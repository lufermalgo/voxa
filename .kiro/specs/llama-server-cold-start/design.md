# llama-server Cold Start — Bugfix Design

## Overview

El bug central es que `LlamaEngine` (y por tanto `llama-server`) se instancia dentro del hilo del pipeline de dictado, en lugar de mantenerse como proceso persistente gestionado por el `AppState`. Esto provoca un bloqueo de ~120s por dictado cuando el servidor no está precargado.

La estrategia de fix es:

1. Extraer la gestión del ciclo de vida de `llama-server` a un componente dedicado (`LlamaManager`) que vive en el `AppState` de Tauri.
2. Iniciar `llama-server` en background durante el `setup()` de la app, antes del primer dictado.
3. Implementar health checks periódicos y auto-restart si el proceso cae.
4. Hacer que el pipeline consuma el `LlamaManager` ya inicializado en lugar de crear un `LlamaEngine` nuevo.
5. Garantizar shutdown limpio del proceso al cerrar la app.

---

## Glossary

- **Bug_Condition (C)**: La condición que activa el bug — `llama-server` no está corriendo cuando el pipeline intenta refinar texto, forzando un cold start de ~120s dentro del hilo del dictado.
- **Property (P)**: El comportamiento correcto — cuando el pipeline llega a la fase de refinamiento LLM, `llama-server` ya está corriendo y listo, y la inferencia tarda 1–3s.
- **Preservation**: Los comportamientos existentes que no deben cambiar: fallback a texto crudo cuando el modelo no existe, perfiles, vocabulario, Whisper, VAD, etc.
- **`LlamaEngine`**: Struct en `src-tauri/src/llama_inference.rs` que encapsula el proceso hijo `llama-server` y el cliente HTTP. Actualmente se instancia dentro del pipeline.
- **`EngineState`**: Struct en `pipeline.rs` que contiene `Mutex<Option<LlamaEngine>>`. El `None` es el estado buggy.
- **`LlamaManager`**: Nuevo componente propuesto que reemplaza el `Mutex<Option<LlamaEngine>>` con gestión activa del ciclo de vida.
- **`AppState`**: Estado global de Tauri, gestionado con `app.manage()`. Accesible desde cualquier comando o hilo.
- **Pre-warm thread**: Hilo en `lib.rs` que intenta inicializar `LlamaEngine` 3s después del arranque. Actualmente falla silenciosamente y no reintenta.
- **Health check**: Petición GET a `http://127.0.0.1:{port}/health` para verificar que `llama-server` está listo.

---

## Bug Details

### Bug Condition

El bug se manifiesta cuando el pipeline de dictado llega a la fase de refinamiento LLM y `engine_state.llama` es `None`. En ese caso, el pipeline intenta crear un nuevo `LlamaEngine` (que lanza `llama-server` y espera hasta 120s a que esté listo) **dentro del hilo del pipeline**, bloqueando el dictado completo.

El pre-warm thread en `lib.rs` intenta evitar esto, pero falla silenciosamente si `LlamaEngine::new()` tarda más de 120s o lanza error — dejando `llama` en `None` para siempre en esa sesión.

**Formal Specification:**
```
FUNCTION isBugCondition(state)
  INPUT: state de tipo AppState en el momento de un dictado
  OUTPUT: boolean

  RETURN state.engine_state.llama.lock() == None
         AND model_path.exists()
         AND server_path.is_some()
END FUNCTION
```

### Examples

- **Caso reportado**: MacBook Air M3, Qwen2.5-1.5B descargado, `llama-server` en `/opt/homebrew/bin/llama-server`. El pre-warm falla o expira → `llama` queda en `None` → cada dictado intenta cold start → timeout a los 120s → `Pipeline total: 122.21s`.
- **Caso 2**: App recién instalada, primer dictado antes de que el pre-warm de 3s complete → `llama` en `None` → cold start en pipeline.
- **Caso 3**: `llama-server` muere durante la sesión (OOM, señal externa) → `llama` queda con un `LlamaEngine` cuyo proceso hijo ya no existe → las peticiones HTTP fallan → el pipeline devuelve texto crudo sin reintentar.
- **Caso edge**: Modelo no descargado → `isBugCondition` retorna `false` (el sistema debe continuar devolviendo texto crudo sin error).

---

## Expected Behavior

### Preservation Requirements

**Unchanged Behaviors:**
- WHEN el modelo LLM no está descargado en disco THEN el sistema CONTINÚA omitiendo `llama-server` y devuelve texto crudo sin error fatal (req. 3.1)
- WHEN `llama-server` no está disponible en el sistema THEN el sistema CONTINÚA devolviendo texto crudo con aviso (req. 3.2)
- WHEN el usuario dicta con un perfil con system prompt THEN el sistema CONTINÚA enviando el texto al LLM y devuelve texto refinado (req. 3.3)
- WHEN el usuario dicta con perfil en modo transcripción cruda THEN el sistema CONTINÚA devolviendo texto sin pasar por el LLM (req. 3.4)
- WHEN Whisper transcribe audio THEN el sistema CONTINÚA funcionando con la misma velocidad independientemente del estado de `llama-server` (req. 3.5)
- WHEN el usuario tiene diccionario personalizado y reemplazos THEN el sistema CONTINÚA aplicándolos antes del LLM (req. 3.6)
- WHEN el sistema detecta silencio THEN el sistema CONTINÚA omitiendo el pipeline completo sin interacción con `llama-server` (req. 3.7)

**Scope:**
Todos los inputs que NO activen la condición del bug (modelo ausente, servidor no disponible, perfil sin prompt, audio silencioso) deben ser completamente no afectados por este fix.

---

## Hypothesized Root Cause

Basado en el análisis del código en `pipeline.rs` y `lib.rs`:

1. **Inicialización lazy dentro del pipeline** (`pipeline.rs`, bloque `if llama_lock.is_none()`): El diseño actual trata `LlamaEngine` como un recurso lazy que se crea on-demand. Esto es correcto para Whisper (carga rápida ~1s) pero catastrófico para `llama-server` (cold start ~120s en el peor caso).

2. **Pre-warm thread sin retry ni supervisión** (`lib.rs`, `std::thread::spawn`): El hilo de pre-calentamiento hace un único intento con `LlamaEngine::new()` que tiene un timeout interno de 120s (240 intentos × 500ms). Si falla, registra el error y termina — sin reintentar, sin notificar al pipeline, sin actualizar ningún estado de "intentando".

3. **`EngineState.llama: Mutex<Option<LlamaEngine>>`**: El tipo `Option<LlamaEngine>` no distingue entre "nunca inicializado", "inicializando", "listo" y "proceso muerto". El pipeline solo comprueba `is_none()`, lo que hace imposible detectar el caso 3 (proceso muerto con `LlamaEngine` en `Some`).

4. **Sin health check continuo**: Una vez que `LlamaEngine` está en `Some`, el pipeline asume que el proceso sigue vivo. Si `llama-server` muere (OOM, señal), las peticiones HTTP fallan pero el `LlamaEngine` sigue en `Some` — el pipeline no reintenta ni reinicia.

5. **Timeout de 120s bloqueante**: `LlamaEngine::new()` bloquea el hilo llamante durante hasta 120s. Cuando se llama desde el pipeline thread, esto bloquea el dictado completo.

---

## Correctness Properties

Property 1: Bug Condition — Pipeline No Inicia llama-server

_For any_ dictado donde el modelo LLM existe en disco y `llama-server` está disponible en el sistema, el pipeline de dictado fijo SHALL completar la fase de refinamiento LLM sin llamar a `LlamaEngine::new()` — porque `llama-server` ya estará corriendo y listo desde el arranque de la app.

**Validates: Requirements 2.1, 2.2**

Property 2: Preservation — Comportamiento Sin Cambio Para Inputs No-Buggy

_For any_ input donde la condición del bug NO se cumple (modelo ausente, servidor no disponible, perfil sin system prompt, audio silencioso, dictado con vocabulario personalizado), el código fijo SHALL producir exactamente el mismo resultado que el código original, preservando todos los comportamientos de fallback y las funcionalidades existentes.

**Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7**

---

## Fix Implementation

### High-Level Design

#### Diagrama de Arquitectura — Ciclo de Vida del Proceso

```
App Startup
    │
    ▼
setup() en lib.rs
    │
    ├─► app.manage(LlamaManager::new(...))
    │       │
    │       └─► Spawn background task:
    │               ├─ Inicia llama-server (LlamaEngine::new)
    │               ├─ Actualiza estado: Initializing → Ready
    │               └─ Inicia health-check loop (cada 10s)
    │
    └─► pipeline::start_pipeline(app, rx)

                    ┌─────────────────────────────────┐
                    │         LlamaManager             │
                    │  ┌─────────────────────────┐    │
                    │  │  state: LlamaState       │    │
                    │  │  ┌──────────────────┐   │    │
                    │  │  │ Unavailable       │   │    │
                    │  │  │ Initializing      │   │    │
                    │  │  │ Ready(LlamaEngine)│   │    │
                    │  │  │ Restarting        │   │    │
                    │  │  └──────────────────┘   │    │
                    │  └─────────────────────────┘    │
                    │  health_check_thread             │
                    │  restart_on_failure()            │
                    └─────────────────────────────────┘
                                    │
                                    │ acquire() → &mut LlamaEngine
                                    │
                    ┌───────────────▼─────────────────┐
                    │         Pipeline Thread          │
                    │  StopRecording event             │
                    │  → Whisper transcribe            │
                    │  → vocab replacement             │
                    │  → llama_manager.refine_text()   │
                    │    (no cold start, ~1-3s)        │
                    └─────────────────────────────────┘

App Shutdown
    │
    ▼
exit_app() / on_window_event
    │
    └─► llama_manager.shutdown()
            └─► process.kill() + process.wait()
```

#### Componentes Involucrados

| Componente | Archivo | Cambio |
|---|---|---|
| `LlamaManager` | `llama_inference.rs` (nuevo) | Nuevo struct que gestiona el ciclo de vida |
| `LlamaState` | `llama_inference.rs` (nuevo) | Enum de estados del proceso |
| `EngineState` | `pipeline.rs` | Eliminar campo `llama`, usar `LlamaManager` |
| `lib.rs` setup | `lib.rs` | Reemplazar pre-warm thread con `LlamaManager::start()` |
| `pipeline.rs` LLM block | `pipeline.rs` | Usar `LlamaManager::refine_text()` en lugar de cold start |
| `commands.rs` exit_app | `commands.rs` | Llamar `llama_manager.shutdown()` antes de `app.exit(0)` |

#### Flujo de Estados de LlamaManager

```
                    ┌─────────────┐
                    │ Unavailable │  ← modelo no existe o server no encontrado
                    └─────────────┘
                           │ modelo existe + server encontrado
                           ▼
                    ┌─────────────┐
                    │Initializing │  ← llama-server arrancando
                    └─────────────┘
                           │ /health OK
                           ▼
                    ┌─────────────┐
                    │    Ready    │  ◄──────────────────┐
                    └─────────────┘                     │
                           │ health check falla          │ restart exitoso
                           ▼                             │
                    ┌─────────────┐                     │
                    │ Restarting  │─────────────────────┘
                    └─────────────┘
```

---

### Low-Level Design

#### 1. Nuevo enum `LlamaState` en `llama_inference.rs`

```rust
pub enum LlamaState {
    /// Modelo no descargado o llama-server no encontrado en el sistema.
    Unavailable,
    /// llama-server está arrancando (proceso lanzado, esperando /health).
    Initializing,
    /// llama-server está listo para recibir peticiones.
    Ready(LlamaEngine),
    /// llama-server cayó y está siendo reiniciado.
    Restarting,
}
```

#### 2. Nuevo struct `LlamaManager` en `llama_inference.rs`

```rust
pub struct LlamaManager {
    state: Arc<Mutex<LlamaState>>,
    model_path: PathBuf,
    server_path: Option<PathBuf>,
    shutdown_flag: Arc<AtomicBool>,
}

impl LlamaManager {
    /// Crea el manager. No lanza el proceso todavía.
    pub fn new(model_path: PathBuf, server_path: Option<PathBuf>) -> Self;

    /// Lanza llama-server en background y arranca el health-check loop.
    /// Llamar desde setup() de lib.rs.
    pub fn start(self: Arc<Self>);

    /// Intenta refinar texto. Si el estado es Ready, llama a LlamaEngine::refine_text().
    /// Si es Initializing, espera hasta MAX_WAIT_SECS con polling.
    /// Si es Unavailable o Restarting (timeout), devuelve Ok(raw_text.to_string()).
    pub fn refine_text(
        &self,
        raw_text: &str,
        system_prompt: &str,
        pre_text: &str,
        post_text: &str,
    ) -> Result<String, String>;

    /// Señaliza shutdown y mata el proceso hijo limpiamente.
    pub fn shutdown(&self);
}
```

**Firma de `start()` — lógica interna:**

```
FUNCTION start(self: Arc<LlamaManager>)
  IF model_path does NOT exist OR server_path is None THEN
    SET state = Unavailable
    RETURN
  END IF

  SET state = Initializing
  SPAWN background_thread:
    result = LlamaEngine::new(model_path, server_path)
    IF result is Ok(engine) THEN
      SET state = Ready(engine)
      LOG "LlamaManager: llama-server ready"
      START health_check_loop(self)
    ELSE
      SET state = Unavailable
      LOG ERROR "LlamaManager: initial start failed: {error}"
    END IF
END FUNCTION
```

**Firma de `health_check_loop()` — lógica interna:**

```
FUNCTION health_check_loop(manager: Arc<LlamaManager>)
  LOOP every HEALTH_CHECK_INTERVAL_SECS (10s):
    IF shutdown_flag is set THEN BREAK

    state = manager.state.lock()
    IF state is NOT Ready THEN CONTINUE

    port = state.Ready.port
    ok = GET http://127.0.0.1:{port}/health → status 200
    IF NOT ok THEN
      SET state = Restarting
      LOG WARN "LlamaManager: health check failed, restarting"
      result = LlamaEngine::new(model_path, server_path)
      IF result is Ok(engine) THEN
        SET state = Ready(engine)
        LOG INFO "LlamaManager: restarted successfully"
      ELSE
        SET state = Unavailable
        LOG ERROR "LlamaManager: restart failed: {error}"
      END IF
    END IF
  END LOOP
END FUNCTION
```

**Firma de `refine_text()` — lógica interna:**

```
FUNCTION refine_text(raw_text, system_prompt, pre_text, post_text)
  IF system_prompt is empty THEN RETURN Ok(raw_text)

  // Esperar a que esté Ready, con timeout
  deadline = now() + MAX_WAIT_FOR_READY_SECS (15s)
  LOOP:
    state = self.state.lock()
    MATCH state:
      Ready(engine) → BREAK (usar engine)
      Unavailable   → RETURN Ok(raw_text)  // fallback silencioso
      Initializing | Restarting →
        IF now() > deadline THEN
          LOG WARN "LlamaManager: timeout waiting for ready, returning raw text"
          RETURN Ok(raw_text)
        END IF
        DROP lock
        SLEEP 200ms
        CONTINUE
    END MATCH
  END LOOP

  RETURN engine.refine_text(raw_text, system_prompt, pre_text, post_text)
END FUNCTION
```

#### 3. Cambios en `LlamaEngine` — exponer `port`

```rust
pub struct LlamaEngine {
    _process: Child,
    pub port: u16,          // hacer pub para que LlamaManager pueda hacer health checks
    client: reqwest::blocking::Client,
}
```

#### 4. Cambios en `EngineState` en `pipeline.rs`

Eliminar el campo `llama`:

```rust
// ANTES:
pub struct EngineState {
    pub whisper: Mutex<Option<whisper_inference::WhisperEngine>>,
    pub llama:   Mutex<Option<llama_inference::LlamaEngine>>,
}

// DESPUÉS:
pub struct EngineState {
    pub whisper: Mutex<Option<whisper_inference::WhisperEngine>>,
    // llama eliminado — gestionado por LlamaManager en AppState
}
```

#### 5. Cambios en `lib.rs` — setup()

Reemplazar el pre-warm thread con `LlamaManager::start()`:

```rust
// ELIMINAR: el bloque std::thread::spawn del pre-warm (líneas ~95-115)

// AÑADIR después de app.manage(model_manager):
let model_manager = app.state::<models::ModelManager>();
let llama_manager = Arc::new(llama_inference::LlamaManager::new(
    model_manager.get_llama_path(),
    model_manager.get_effective_llama_server(),
));
app.manage(Arc::clone(&llama_manager));
llama_manager.start();
```

#### 6. Cambios en `pipeline.rs` — bloque LLM refinement

Reemplazar el bloque `if llama_lock.is_none()` completo:

```rust
// ANTES: ~50 líneas con cold start inline
let refined_text = {
    let mut llama_lock = engine_state.llama.lock().unwrap();
    if llama_lock.is_none() {
        // ... cold start de 120s ...
    } else {
        // ... refine ...
    }
};

// DESPUÉS: ~15 líneas usando LlamaManager
let refined_text = {
    let llama_manager = app.state::<Arc<llama_inference::LlamaManager>>();
    let (system_prompt, profile_name) = resolve_system_prompt(&app, &db_state);
    if system_prompt.is_empty() {
        raw_text.clone()
    } else {
        log::info!("LLM Profile: '{}'", profile_name);
        let t_llm = std::time::Instant::now();
        let result = match llama_manager.refine_text(
            &raw_text, &system_prompt, &cursor_pre, &cursor_post
        ) {
            Ok(r) => r,
            Err(e) => {
                log::error!("LLM refinement failed: {}", e);
                let _ = app.emit("pipeline-error", format!("Refinement Error: {}", e));
                raw_text.clone()
            }
        };
        log::info!(
            "LLM: {:.2}s  in={} chars  out={} chars",
            t_llm.elapsed().as_secs_f64(), raw_text.len(), result.len()
        );
        result
    }
};
```

#### 7. Cambios en `commands.rs` — `exit_app`

```rust
#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    use crate::pipeline::PipelineHandle;
    use std::sync::Arc;
    use crate::llama_inference::LlamaManager;

    // Señalizar shutdown al pipeline
    app.state::<PipelineHandle>()
        .cancelled
        .store(true, std::sync::atomic::Ordering::SeqCst);

    // Shutdown limpio de llama-server
    if let Some(manager) = app.try_state::<Arc<LlamaManager>>() {
        manager.shutdown();
    }

    std::thread::sleep(std::time::Duration::from_millis(200));
    app.exit(0);
}
```

#### 8. Constantes de configuración

```rust
// En llama_inference.rs
const HEALTH_CHECK_INTERVAL_SECS: u64 = 10;
const MAX_WAIT_FOR_READY_SECS: u64 = 15;
const MAX_RESTART_ATTEMPTS: u32 = 3;
```

#### 9. Manejo de errores y edge cases

| Caso | Comportamiento |
|---|---|
| Modelo no descargado al arranque | `LlamaState::Unavailable`, `refine_text()` devuelve texto crudo |
| `llama-server` no encontrado | `LlamaState::Unavailable`, `refine_text()` devuelve texto crudo |
| Arranque lento (>15s para estar listo) | Pipeline devuelve texto crudo con log WARN, servidor sigue arrancando |
| Proceso muerto durante sesión | Health check detecta en ≤10s, reinicia automáticamente |
| Restart falla 3 veces | `LlamaState::Unavailable`, log ERROR, no más reintentos |
| Shutdown mientras Initializing | `shutdown_flag` señalizado, background thread termina limpiamente |
| Puerto ocupado | `find_free_port()` ya maneja esto — sin cambios |

---

## Testing Strategy

### Validation Approach

La estrategia sigue dos fases: primero, confirmar el bug en el código sin fix (exploratory); luego, verificar que el fix funciona y no introduce regresiones (fix checking + preservation checking).

### Exploratory Bug Condition Checking

**Goal**: Demostrar que en el código actual, cuando `engine_state.llama` es `None` al inicio de un dictado, el pipeline intenta `LlamaEngine::new()` inline — confirmando el root cause.

**Test Plan**: Escribir tests que simulen el estado `llama = None` en `EngineState` y verifiquen que el pipeline llama a `LlamaEngine::new()` durante el procesamiento. Ejecutar en código SIN fix para observar el comportamiento buggy.

**Test Cases**:
1. **Cold Start Inline Test**: Con `engine_state.llama = None` y modelo disponible, verificar que el pipeline llama a `LlamaEngine::new()` (will fail to show bug on unfixed code — el test pasa porque el bug existe)
2. **Pre-warm Failure Test**: Simular fallo del pre-warm thread y verificar que `llama` queda en `None` permanentemente (will demonstrate bug on unfixed code)
3. **Repeated Cold Start Test**: Verificar que cada dictado con `llama = None` repite el cold start (will demonstrate bug on unfixed code)
4. **Process Death Test**: Matar el proceso hijo de `LlamaEngine` y verificar que el pipeline no detecta la caída (may demonstrate bug on unfixed code)

**Expected Counterexamples**:
- El pipeline llama a `LlamaEngine::new()` durante el dictado cuando `llama` es `None`
- Posibles causas: inicialización lazy sin gestión de ciclo de vida, pre-warm sin retry, `Option<LlamaEngine>` sin estados intermedios

### Fix Checking

**Goal**: Verificar que para todos los inputs donde la condición del bug se cumple (modelo existe, servidor disponible, `llama` estaba en `None`), el código fijo completa el refinamiento sin cold start inline.

**Pseudocode:**
```
FOR ALL dictation_event WHERE isBugCondition(app_state) DO
  result := pipeline_fixed(dictation_event)
  ASSERT result.llama_new_called == false
  ASSERT result.refinement_time_secs < 5.0
  ASSERT result.refined_text != raw_text OR system_prompt.is_empty()
END FOR
```

### Preservation Checking

**Goal**: Verificar que para todos los inputs donde la condición del bug NO se cumple, el código fijo produce el mismo resultado que el código original.

**Pseudocode:**
```
FOR ALL input WHERE NOT isBugCondition(input) DO
  ASSERT pipeline_original(input) == pipeline_fixed(input)
END FOR
```

**Testing Approach**: Property-based testing es recomendado para preservation checking porque:
- Genera muchos casos de test automáticamente (modelos ausentes, perfiles variados, audio silencioso, etc.)
- Captura edge cases que tests manuales podrían omitir
- Provee garantías fuertes de que el comportamiento es idéntico para todos los inputs no-buggy

**Test Cases**:
1. **Model Absent Preservation**: Verificar que con modelo no descargado, el pipeline devuelve texto crudo — igual que antes del fix
2. **Server Unavailable Preservation**: Verificar que sin `llama-server` en el sistema, el pipeline devuelve texto crudo con aviso
3. **Empty System Prompt Preservation**: Verificar que con perfil sin system prompt, el pipeline devuelve texto crudo sin llamar al LLM
4. **Silent Audio Preservation**: Verificar que con audio silencioso, el pipeline omite todo el procesamiento sin interacción con `LlamaManager`
5. **Vocabulary Replacement Preservation**: Verificar que los reemplazos de vocabulario se aplican antes del LLM, igual que antes
6. **Whisper Speed Preservation**: Verificar que la transcripción STT no se ve afectada por el estado de `LlamaManager`

### Unit Tests

- Test de `LlamaManager::new()` con modelo existente y no existente
- Test de transición de estados: `Initializing → Ready`, `Ready → Restarting → Ready`, `Ready → Unavailable`
- Test de `refine_text()` cuando estado es `Unavailable` (debe devolver texto crudo)
- Test de `refine_text()` cuando estado es `Initializing` con timeout (debe devolver texto crudo después de MAX_WAIT_FOR_READY_SECS)
- Test de `shutdown()` — verificar que el proceso hijo es terminado
- Test de `find_free_port()` — sin cambios, ya funciona

### Property-Based Tests

- Generar estados aleatorios de `LlamaState` y verificar que `refine_text()` nunca panics
- Generar system prompts aleatorios (vacíos, largos, con caracteres especiales) y verificar que el comportamiento de fallback es correcto cuando `Unavailable`
- Generar secuencias aleatorias de dictados y verificar que el estado de `LlamaManager` siempre converge a `Ready` o `Unavailable` (nunca queda en `Initializing` indefinidamente)
- Verificar que para cualquier input con `system_prompt.is_empty()`, `refine_text()` devuelve el texto original sin modificar

### Integration Tests

- Test de flujo completo: arranque de app → `LlamaManager::start()` → primer dictado → refinamiento sin cold start
- Test de recovery: dictado exitoso → matar proceso hijo → health check detecta caída → restart → segundo dictado exitoso
- Test de shutdown: dictado en curso → `exit_app()` → verificar que el proceso hijo es terminado limpiamente
- Test de concurrencia: múltiples dictados simultáneos → verificar que `LlamaManager` serializa correctamente el acceso al `LlamaEngine`

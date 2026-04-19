# Implementation Plan

- [ ] 1. Write bug condition exploration test
  - **Property 1: Bug Condition** - Pipeline Inicia LlamaEngine Inline (Cold Start)
  - **CRITICAL**: Este test DEBE FALLAR en el código sin fix — el fallo confirma que el bug existe
  - **DO NOT attempt to fix the test or the code when it fails**
  - **NOTE**: Este test codifica el comportamiento esperado — validará el fix cuando pase después de la implementación
  - **GOAL**: Demostrar que cuando `engine_state.llama` es `None` y el modelo existe, el pipeline llama a `LlamaEngine::new()` dentro del hilo del dictado
  - **Scoped PBT Approach**: Scope the property to the concrete failing case — `engine_state.llama = None` con modelo y servidor disponibles
  - Condición del bug (de `isBugCondition` en el diseño): `state.engine_state.llama.lock() == None AND model_path.exists() AND server_path.is_some()`
  - Escribir test en `src-tauri/src/llama_inference.rs` (módulo `#[cfg(test)]`) o en un archivo de test separado
  - El test debe verificar que el pipeline intenta `LlamaEngine::new()` cuando `llama` es `None` — esto es el comportamiento buggy
  - Estrategia: mockear/interceptar `LlamaEngine::new()` o verificar que el estado `llama` pasa de `None` a `Some` durante el pipeline (lo cual implica cold start inline)
  - Alternativamente: test de integración que mide el tiempo de la fase LLM cuando `llama` empieza en `None` — debe ser >1s (cold start) vs <1s (ya listo)
  - Ejecutar en código SIN fix
  - **EXPECTED OUTCOME**: Test FALLA o demuestra el comportamiento buggy (cold start inline ocurre)
  - Documentar el contraejemplo encontrado: "Con `llama = None` y modelo disponible, el pipeline llama `LlamaEngine::new()` bloqueando el hilo del dictado"
  - Marcar tarea completa cuando el test esté escrito, ejecutado, y el fallo documentado
  - _Requirements: 1.1, 1.2, 1.3_

- [ ] 2. Write preservation property tests (BEFORE implementing fix)
  - **Property 2: Preservation** - Comportamientos No-Buggy Sin Cambio
  - **IMPORTANT**: Seguir metodología observation-first
  - Observar en código SIN fix: con modelo ausente → `refine_text` devuelve texto crudo
  - Observar en código SIN fix: con `server_path = None` → `refine_text` devuelve texto crudo con log warn
  - Observar en código SIN fix: con `system_prompt` vacío → pipeline devuelve texto crudo sin llamar al LLM
  - Observar en código SIN fix: con audio silencioso → pipeline omite todo el procesamiento
  - Observar en código SIN fix: con vocabulario personalizado → reemplazos aplicados antes del LLM
  - Escribir property-based tests en `src-tauri/src/llama_inference.rs` o archivo de test separado:
    - Para todo `system_prompt` vacío: `refine_text(text, "", _, _)` devuelve `Ok(text)` sin modificar
    - Para todo estado `Unavailable`: `refine_text(...)` devuelve `Ok(raw_text)` (fallback silencioso)
    - Para toda secuencia de inputs con modelo ausente: el pipeline nunca intenta inicializar `LlamaEngine`
  - Ejecutar tests en código SIN fix
  - **EXPECTED OUTCOME**: Tests PASAN (confirman comportamiento baseline a preservar)
  - Marcar tarea completa cuando los tests estén escritos, ejecutados, y pasando en código sin fix
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

- [ ] 3. Fix: Convertir LlamaEngine de recurso lazy a proceso persistente gestionado por LlamaManager

  - [ ] 3.1 Añadir enum `LlamaState` en `llama_inference.rs`
    - Añadir enum público con variantes: `Unavailable`, `Initializing`, `Ready(LlamaEngine)`, `Restarting`
    - Colocar antes de la definición de `LlamaEngine`
    - Derivar `Debug` si es posible (sin `LlamaEngine` en `Ready` si no implementa `Debug`)
    - _Bug_Condition: `isBugCondition` retorna true cuando `llama = None` — este enum reemplaza el `Option<LlamaEngine>` que no distingue estados_
    - _Requirements: 2.1, 2.4_

  - [ ] 3.2 Hacer `pub` el campo `port` en `LlamaEngine`
    - Cambiar `port: u16` a `pub port: u16` en el struct `LlamaEngine`
    - Necesario para que `LlamaManager` pueda construir la URL del health check
    - _Requirements: 2.4_

  - [ ] 3.3 Implementar struct `LlamaManager` con método `new()` en `llama_inference.rs`
    - Añadir constantes: `HEALTH_CHECK_INTERVAL_SECS = 10`, `MAX_WAIT_FOR_READY_SECS = 15`, `MAX_RESTART_ATTEMPTS = 3`
    - Struct con campos: `state: Arc<Mutex<LlamaState>>`, `model_path: PathBuf`, `server_path: Option<PathBuf>`, `shutdown_flag: Arc<AtomicBool>`
    - `new(model_path: PathBuf, server_path: Option<PathBuf>) -> Self` — solo crea el struct, no lanza proceso
    - _Bug_Condition: `isBugCondition` — el manager reemplaza el `Mutex<Option<LlamaEngine>>` en `EngineState`_
    - _Expected_Behavior: `expectedBehavior` — cuando el pipeline llega a refinamiento, `llama-server` ya está corriendo_
    - _Requirements: 2.1, 2.4_

  - [ ] 3.4 Implementar método `start()` en `LlamaManager`
    - Firma: `pub fn start(self: Arc<Self>)`
    - Si `model_path` no existe o `server_path` es `None`: establecer estado `Unavailable` y retornar
    - Si condiciones OK: establecer estado `Initializing`, lanzar background thread
    - Background thread: llamar `LlamaEngine::new()`, si OK → estado `Ready(engine)` + iniciar `health_check_loop`; si Err → estado `Unavailable` + log error
    - _Bug_Condition: reemplaza el `std::thread::spawn` del pre-warm en `lib.rs` que falla silenciosamente_
    - _Requirements: 2.1_

  - [ ] 3.5 Implementar `health_check_loop()` en `LlamaManager`
    - Loop cada `HEALTH_CHECK_INTERVAL_SECS` (10s), salir si `shutdown_flag` está activo
    - Si estado no es `Ready`: continuar (no hacer nada)
    - Si estado es `Ready`: GET `http://127.0.0.1:{port}/health` → si falla: estado `Restarting`, reintentar `LlamaEngine::new()` hasta `MAX_RESTART_ATTEMPTS` veces; si OK → estado `Ready`; si todos fallan → estado `Unavailable`
    - _Bug_Condition: el código actual no tiene health check — proceso muerto no se detecta_
    - _Requirements: 2.4_

  - [ ] 3.6 Implementar método `refine_text()` en `LlamaManager`
    - Firma: `pub fn refine_text(&self, raw_text: &str, system_prompt: &str, pre_text: &str, post_text: &str) -> Result<String, String>`
    - Si `system_prompt` vacío: retornar `Ok(raw_text.to_string())` inmediatamente
    - Loop con deadline `now() + MAX_WAIT_FOR_READY_SECS`: match estado → `Ready(engine)` → usar engine; `Unavailable` → retornar `Ok(raw_text)` (fallback); `Initializing | Restarting` → si timeout → log warn + retornar `Ok(raw_text)`; si no → sleep 200ms + continuar
    - _Expected_Behavior: `expectedBehavior(result)` — refinamiento en 1–3s sin cold start_
    - _Preservation: fallback a texto crudo cuando `Unavailable` — preserva req. 3.1, 3.2_
    - _Requirements: 2.2, 2.3, 3.1, 3.2_

  - [ ] 3.7 Implementar método `shutdown()` en `LlamaManager`
    - Firma: `pub fn shutdown(&self)`
    - Establecer `shutdown_flag = true`
    - Tomar lock del estado, si es `Ready(engine)` → drop explícito del engine (que llama `process.kill()` + `process.wait()` via `Drop`)
    - Establecer estado `Unavailable`
    - _Requirements: 2.5_

  - [ ] 3.8 Eliminar campo `llama` de `EngineState` en `pipeline.rs`
    - Eliminar `pub llama: Mutex<Option<llama_inference::LlamaEngine>>` del struct `EngineState`
    - Eliminar import de `LlamaEngine` si ya no se usa directamente en `pipeline.rs`
    - _Bug_Condition: `EngineState.llama = None` es la condición del bug — eliminar este campo elimina la posibilidad del estado buggy_
    - _Requirements: 2.1_

  - [ ] 3.9 Reemplazar pre-warm thread en `lib.rs` con `LlamaManager::start()`
    - Eliminar el bloque `std::thread::spawn` del pre-warm (líneas ~95-115 en `lib.rs`)
    - Añadir después de `app.manage(model_manager)`:
      ```rust
      let model_manager = app.state::<models::ModelManager>();
      let llama_manager = Arc::new(llama_inference::LlamaManager::new(
          model_manager.get_llama_path(),
          model_manager.get_effective_llama_server(),
      ));
      app.manage(Arc::clone(&llama_manager));
      llama_manager.start();
      ```
    - Añadir import `use std::sync::Arc;` si no existe
    - _Bug_Condition: el pre-warm actual falla silenciosamente y no reintenta — `LlamaManager::start()` gestiona el ciclo de vida correctamente_
    - _Requirements: 2.1_

  - [ ] 3.10 Simplificar bloque LLM en `pipeline.rs` para usar `LlamaManager::refine_text()`
    - Eliminar el bloque completo `let refined_text = { let mut llama_lock = engine_state.llama.lock()... }` (~50 líneas)
    - Reemplazar con el bloque simplificado usando `LlamaManager`:
      ```rust
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
    - Eliminar la función helper `run_llm_refinement()` si ya no se usa
    - Actualizar imports en `pipeline.rs` (eliminar `LlamaEngine`, añadir `Arc` si necesario)
    - _Bug_Condition: el bloque actual llama `LlamaEngine::new()` inline cuando `llama = None` — este bloque lo elimina_
    - _Expected_Behavior: `refine_text()` en `LlamaManager` completa en 1–3s porque el servidor ya está corriendo_
    - _Preservation: `system_prompt.is_empty()` → texto crudo (preserva req. 3.3, 3.4); `Unavailable` → texto crudo (preserva req. 3.1, 3.2)_
    - _Requirements: 2.2, 2.3, 3.1, 3.2, 3.3, 3.4_

  - [ ] 3.11 Actualizar `exit_app` en `commands.rs` para llamar `llama_manager.shutdown()`
    - Añadir imports: `use crate::llama_inference::LlamaManager; use std::sync::Arc;`
    - Antes de `app.exit(0)`, añadir:
      ```rust
      if let Some(manager) = app.try_state::<Arc<LlamaManager>>() {
          manager.shutdown();
      }
      ```
    - _Requirements: 2.5_

  - [ ] 3.12 Verificar que el test de exploración del bug ahora pasa
    - **Property 1: Expected Behavior** - Pipeline NO Inicia LlamaEngine Inline
    - **IMPORTANT**: Re-ejecutar el MISMO test del task 1 — NO escribir un test nuevo
    - El test del task 1 codifica el comportamiento esperado: el pipeline NO llama `LlamaEngine::new()` durante el dictado
    - Cuando este test pasa, confirma que `expectedBehavior` se cumple: refinamiento sin cold start inline
    - Ejecutar el test de exploración del bug del paso 1
    - **EXPECTED OUTCOME**: Test PASA (confirma que el bug está corregido)
    - _Requirements: 2.1, 2.2 — Expected Behavior Properties del diseño_

  - [ ] 3.13 Verificar que los tests de preservación siguen pasando
    - **Property 2: Preservation** - Comportamientos No-Buggy Sin Cambio
    - **IMPORTANT**: Re-ejecutar los MISMOS tests del task 2 — NO escribir tests nuevos
    - Ejecutar los property-based tests de preservación del paso 2
    - **EXPECTED OUTCOME**: Tests PASAN (confirma que no hay regresiones)
    - Confirmar que todos los comportamientos de fallback siguen funcionando después del fix
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

- [ ] 4. Checkpoint — Verificar que todos los tests pasan
  - Ejecutar `cargo test` en `src-tauri/` para confirmar que todos los tests pasan
  - Verificar que el proyecto compila sin errores ni warnings relevantes: `cargo build`
  - Confirmar que los tests de exploración (Property 1) y preservación (Property 2) pasan
  - Verificar manualmente que el primer dictado después del arranque de la app completa la fase LLM en <5s (no 120s)
  - Si surgen dudas o errores inesperados, consultar al usuario antes de continuar
  - Asegurarse de que no quedan referencias a `engine_state.llama` en el código
  - Asegurarse de que el campo `llama` fue eliminado de `EngineState`

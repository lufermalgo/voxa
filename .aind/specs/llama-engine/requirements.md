# Requirements: llama-engine

## Introducción

El llama-engine es el subsistema responsable de gestionar el ciclo de vida del proceso `llama-server` y de refinar el texto transcrito por Whisper usando el LLM local (Qwen2.5-1.5B Q4_K_M). Opera como un servidor HTTP local que arranca bajo demanda y se mantiene vivo durante toda la sesión de la app.

### Estado actual del pipeline

```
Whisper → raw_text → [replacements] → LlamaEngine.refine_text() → refined_text → clipboard
```

El LlamaEngine recibe el `raw_text` y el `system_prompt` del perfil activo, y devuelve el texto refinado. Si falla, el pipeline usa `raw_text` como fallback.

---

## Requirements

### Requirement 1: Arranque bajo demanda

**User Story:** Como usuario, cuando hago mi primer dictado, quiero que el LLM esté listo sin haber tenido que configurar nada manualmente.

#### Acceptance Criteria

1. WHEN el pipeline recibe el primer dictado y `LlamaEngine` no está inicializado, THE system SHALL arrancar `llama-server` automáticamente con el modelo configurado.
2. WHEN `llama-server` no está disponible en el sistema, THE pipeline SHALL omitir el refinement y retornar `raw_text` con un warning en el log.
3. WHEN el modelo `.gguf` no está descargado, THE pipeline SHALL omitir el refinement y retornar `raw_text` con un warning en el log.
4. WHEN el arranque del servidor tarda más de 120 segundos, THE system SHALL abortar el intento, loggear el error, y emitir el evento `pipeline-error` al frontend.

### Requirement 2: Persistencia en sesión

**User Story:** Como usuario, quiero que el LLM no se reinicie entre dictados para que la latencia sea mínima a partir del segundo uso.

#### Acceptance Criteria

1. WHEN `LlamaEngine` ya está inicializado, THE pipeline SHALL reutilizar la instancia existente sin reiniciar el servidor.
2. WHEN la app se cierra, THE system SHALL matar el proceso `llama-server` automáticamente (sin dejar procesos huérfanos).

### Requirement 3: Detección de servidor muerto

**User Story:** Como usuario, si el servidor LLM muere inesperadamente (crash, kill externo), quiero que el próximo dictado lo reinicie automáticamente sin que yo tenga que hacer nada.

#### Acceptance Criteria

1. WHEN el proceso `llama-server` muere externamente entre dictados, THE pipeline SHALL detectarlo antes del siguiente uso mediante un health check.
2. WHEN el servidor está muerto, THE pipeline SHALL descartar la instancia stale y crear una nueva, reiniciando el servidor.
3. WHEN el reinicio falla, THE pipeline SHALL emitir `pipeline-error` al frontend y retornar `raw_text` como fallback.

### Requirement 4: Refinement de texto

**User Story:** Como usuario, quiero que el texto transcrito sea mejorado por el LLM según el perfil activo antes de ser insertado.

#### Acceptance Criteria

1. WHEN el `system_prompt` del perfil activo no está vacío, THE engine SHALL enviar el texto al LLM para refinement.
2. WHEN el `system_prompt` está vacío, THE engine SHALL retornar el `raw_text` sin llamar al LLM.
3. WHEN el LLM produce una respuesta vacía o inválida, THE pipeline SHALL retornar `raw_text` como fallback.
4. WHEN hay contexto de cursor (`pre_text`, `post_text`), THE engine SHALL inyectarlo en el prompt para que el LLM adapte capitalización y tono al documento circundante.
5. THE timeout de la request al LLM SHALL ser de 60 segundos.

### Requirement 5: Errores visibles al usuario

**User Story:** Como usuario, si el LLM falla, quiero saberlo — no quiero recibir el texto sin refinar sin ninguna indicación.

#### Acceptance Criteria

1. WHEN el refinement falla por cualquier razón, THE system SHALL emitir el evento `pipeline-error` al frontend con un mensaje descriptivo.
2. WHEN el servidor retorna un código HTTP de error, THE system SHALL loggear el código y emitir `pipeline-error`.
3. THE fallback SHALL siempre ser `raw_text` — nunca texto vacío.

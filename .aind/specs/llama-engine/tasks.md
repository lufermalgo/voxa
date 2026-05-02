# Tasks — llama-engine

## Done

### BUG-59 — Server dies externally, pipeline silently cae a raw transcription

**Outcome:** Tras un kill externo de llama-server, el siguiente dictado detecta el servidor muerto, lo reinicia automáticamente y produce output refinado.

**Acceptance tests:**

1. Dictado normal → LLM refina correctamente (baseline).
2. `killall llama-server` en terminal.
3. Segundo dictado → log muestra `LlamaEngine: server died externally, will restart` → output es diferente a la transcripción cruda.

**Root cause:** `Mutex<Option<LlamaEngine>>` mantenía `Some(_)` con el proceso hijo muerto. El pipeline asumía `Some` = vivo.

**Fix:** `LlamaEngine::is_alive()` + guard en `pipeline.rs` que resetea a `None` si el servidor no responde.

**PR:** #60 | **Issue:** #59 | **Status:** merged

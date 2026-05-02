# Design: llama-engine

## Enfoque

Un único struct `LlamaEngine` (en `src-tauri/src/llama_inference.rs`) encapsula el proceso hijo y el cliente HTTP. Vive en `EngineState.llama: Mutex<Option<LlamaEngine>>`. El pipeline lo inicializa bajo demanda y lo reutiliza entre dictados.

---

## 1. Estructura

```rust
pub struct LlamaEngine {
    _process: Child,      // proceso llama-server — killed on Drop
    port: u16,            // puerto asignado por el OS al arrancar
    client: reqwest::blocking::Client,
}
```

El puerto se asigna dinámicamente via `TcpListener::bind("127.0.0.1:0")` para evitar conflictos con otros procesos.

---

## 2. Ciclo de vida

### Arranque (`LlamaEngine::new`)

1. Busca puerto libre → spawna `llama-server` con `-ngl 99 --flash-attn --mlock --ctx-size 4096`.
2. Redirige stderr a `/tmp/llama-server.log` para diagnóstico.
3. Polling `/health` cada 500ms hasta 120s → si no responde, mata el proceso y retorna `Err`.
4. En macOS/aarch64: añade `--flash-attn auto --mlock` (Metal optimizations).

### Reutilización

El pipeline usa `Mutex<Option<LlamaEngine>>`:

```
dictado →  lock  →  is_alive()?  →  None: restart  →  refine_text()
                                 →  Some: use as-is →  refine_text()
```

### Detección de servidor muerto (`is_alive`)

```rust
pub fn is_alive(&self) -> bool {
    // GET /health con timeout 2s
    // true si status 200, false en cualquier error
}
```

Llamado antes de cada uso en `pipeline.rs`. Si retorna `false`, el lock se resetea a `None` y el path de arranque lo reinicia.

### Shutdown

`impl Drop for LlamaEngine` → `process.kill() + process.wait()`. Garantiza que no queden procesos huérfanos al cerrar la app.

---

## 3. Prompt construction

Formato ChatML (compatible con Qwen2.5-Instruct):

```
<|im_start|>system
You MUST output in {language} only. Never translate.

{system_prompt}<|im_end|>
<|im_start|>user
{user_message}<|im_end|>
<|im_start|>assistant
```

Cuando hay contexto de cursor:

```
<before_text>{pre}</before_text>
<transcription>{text}</transcription>
<after_text>{post}</after_text>

Output ONLY the formatted transcription...
```

Parámetros de inferencia: `n_predict=1200`, `temperature=0.0`, `cache_prompt=true`, `stream=false`.

Stop tokens: `["<|im_end|>", "<|endoftext|>", "<|im_start|>"]`.

---

## 4. Parámetros del servidor

| Flag | Valor | Razón |
|------|-------|-------|
| `-ngl 99` | offload todo a GPU | Metal en M3 — máximo rendimiento |
| `--flash-attn auto` | activado en aarch64 | ~30% speedup en attention |
| `--mlock` | activado en aarch64 | previene paginación bajo memory pressure |
| `--ctx-size 4096` | 4096 tokens | suficiente para dictados + sistema prompt |
| `--n-parallel 4` | auto (default) | permite requests concurrentes |

**Optimizaciones pendientes** (registradas en tasks.md):

- Reducir `--n-parallel 1` y `--ctx-size 2048` para caso de uso single-user.
- `--cache-type-k q8_0 --cache-type-v q8_0` — reduce KV cache ~50%.
- `--threads 8` — usa todos los cores del M3.

---

## 5. Archivos involucrados

| Archivo | Rol |
|---------|-----|
| `src-tauri/src/llama_inference.rs` | `LlamaEngine` — toda la lógica del servidor y refinement |
| `src-tauri/src/pipeline.rs` | gestión del lock + guard `is_alive` + `run_llm_refinement` |
| `src-tauri/src/lib.rs` | inicialización del `EngineState` en el arranque de la app |

# Bugfix Requirements Document

## Introduction

El pipeline de dictado de Voxa tarda ~122 segundos en completarse porque `llama-server` se inicia desde cero dentro del flujo de cada dictado, en lugar de mantenerse como un proceso persistente listo para recibir peticiones. El STT (Whisper) es rápido (~0.56s), pero el cold start del LLM bloquea el hilo del pipeline durante hasta 120s — y si el servidor no responde en ese tiempo, el dictado falla silenciosamente y devuelve el texto crudo sin refinar.

El código en `lib.rs` incluye un hilo de pre-calentamiento (`Pre-loading LlamaEngine`) que intenta inicializar `LlamaEngine` 3 segundos después del arranque de la app. Sin embargo, si ese pre-calentamiento falla o tarda más de 120s (el timeout actual), el estado `EngineState.llama` queda en `None` y el pipeline vuelve a intentar el cold start en cada dictado subsiguiente, reproduciendo el problema en cada sesión.

El impacto es crítico para la experiencia de usuario: la funcionalidad principal de la app (transformación de texto con perfiles) queda inutilizable en la práctica.

## Bug Analysis

### Current Behavior (Defect)

1.1 WHEN el usuario activa un dictado y `LlamaEngine` no está inicializado THEN el sistema inicia `llama-server` como subproceso dentro del hilo del pipeline, bloqueando el flujo de dictado durante el tiempo de arranque del servidor (~120s en el caso reportado)

1.2 WHEN `llama-server` no responde al endpoint `/health` dentro de los 120 segundos de timeout THEN el sistema mata el proceso, registra el error `LlamaEngine init failed: llama-server failed to become ready within 120s`, y devuelve el texto crudo sin refinar como si el LLM no estuviera disponible

1.3 WHEN el pre-calentamiento de `LlamaEngine` en `lib.rs` falla o expira THEN el sistema deja `EngineState.llama` en `None` y el siguiente dictado vuelve a intentar el cold start completo, repitiendo el bloqueo de 120s en cada dictado

1.4 WHEN `llama-server` está arrancando dentro del pipeline THEN el sistema emite el estado `loading_llama` a la UI pero no ofrece al usuario ninguna forma de cancelar la espera ni información sobre el tiempo estimado

### Expected Behavior (Correct)

2.1 WHEN la aplicación arranca THEN el sistema SHALL iniciar `llama-server` como proceso persistente en background y mantenerlo vivo durante toda la sesión de la app, independientemente de si el usuario ha dictado o no

2.2 WHEN el usuario activa un dictado y `llama-server` ya está corriendo y listo THEN el sistema SHALL completar la fase de refinamiento LLM en 1–3 segundos (solo el tiempo de inferencia), sin ningún overhead de arranque

2.3 WHEN `llama-server` no está listo en el momento del dictado (aún arrancando) THEN el sistema SHALL esperar a que esté disponible o devolver el texto crudo con un aviso claro, sin bloquear el pipeline más de lo necesario

2.4 WHEN el proceso `llama-server` muere inesperadamente durante la sesión THEN el sistema SHALL detectar la caída y reiniciarlo automáticamente en background, sin requerir reinicio de la app

2.5 WHEN la aplicación se cierra THEN el sistema SHALL terminar el proceso `llama-server` de forma limpia (kill + wait), liberando el puerto y los recursos de GPU/memoria

### Unchanged Behavior (Regression Prevention)

3.1 WHEN el modelo LLM no está descargado en disco THEN el sistema SHALL CONTINUE TO omitir la inicialización de `llama-server` y devolver el texto crudo sin error fatal

3.2 WHEN `llama-server` no está disponible en el sistema (binario no encontrado) THEN el sistema SHALL CONTINUE TO omitir la refinación y devolver el texto crudo con el aviso correspondiente

3.3 WHEN el usuario dicta con un perfil activo que tiene system prompt configurado THEN el sistema SHALL CONTINUE TO enviar el texto al LLM y devolver el texto refinado según el perfil

3.4 WHEN el usuario dicta con el perfil en modo transcripción cruda (sin system prompt) THEN el sistema SHALL CONTINUE TO devolver el texto sin pasar por el LLM

3.5 WHEN Whisper transcribe el audio THEN el sistema SHALL CONTINUE TO funcionar con la misma velocidad y calidad independientemente del estado de `llama-server`

3.6 WHEN el usuario tiene configurado el diccionario personalizado y los reemplazos de vocabulario THEN el sistema SHALL CONTINUE TO aplicarlos antes de enviar el texto al LLM

3.7 WHEN el sistema detecta silencio en el audio grabado THEN el sistema SHALL CONTINUE TO omitir el pipeline completo (STT + LLM) sin interacción con `llama-server`

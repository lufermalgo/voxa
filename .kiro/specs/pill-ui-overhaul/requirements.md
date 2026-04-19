# Requirements: pill-ui-overhaul

## Introduction

Este documento define los requisitos funcionales y no funcionales para el rediseño visual de la píldora flotante de Voxa. Los requisitos se derivan del design document (`design.md`) y cierran la brecha entre el design spec (`docs/ui_spec_micro.md`, `docs/design.md`) y la implementación actual en `src/components/RecorderPill.tsx`.

---

## Requirements

### Requirement 1: Fix del bug de layout del warning card

**User Story:** Como usuario que graba audio durante más de 48 segundos, quiero ver el popup de advertencia de límite de grabación, para saber cuánto tiempo me queda antes de que la grabación se detenga automáticamente.

#### Acceptance Criteria

1. **GIVEN** que el estado de grabación es `recording` y `progress >= 0.8` (isWarning = true), **WHEN** se renderiza el componente `RecorderPill`, **THEN** el `WarningCard` debe aparecer visualmente por encima de la píldora en la pantalla.

2. **GIVEN** que `isWarning` cambia de `false` a `true`, **WHEN** Tauri ejecuta `setSize`, **THEN** la ventana debe tener exactamente `300 × 220px` de tamaño lógico.

3. **GIVEN** que `isWarning` cambia de `true` a `false` (grabación cancelada o detenida), **WHEN** Tauri ejecuta `setSize`, **THEN** la ventana debe volver a `300 × 80px`.

4. **GIVEN** que `isWarning = true`, **WHEN** se inspecciona el árbol DOM, **THEN** el elemento `WarningCard` debe aparecer **antes** (posición DOM superior) que el elemento de la píldora en el contenedor flex.

5. **GIVEN** que `isWarning = true`, **WHEN** el usuario ve la pantalla, **THEN** el card debe mostrar el número de segundos restantes (`timeRemaining`) actualizado en tiempo real.

---

### Requirement 2: Píldora con dimensiones y forma correctas

**User Story:** Como usuario de Voxa, quiero que la píldora se vea como un elemento premium de macOS, para que se integre visualmente con apps de alta calidad como Wispr Flow.

#### Acceptance Criteria

1. **GIVEN** que el estado es `recording`, `processing`, `refining`, `done`, `loading`, o `loading_whisper`/`loading_llama`, **WHEN** se renderiza la píldora, **THEN** su altura debe ser `48px` (clase `h-12`).

2. **GIVEN** cualquier estado activo de la píldora, **WHEN** se renderiza, **THEN** el corner radius debe ser `24px` (clase `rounded-[24px]`), no `rounded-voxa` (40px).

3. **GIVEN** el estado `idle`, **WHEN** se renderiza la píldora, **THEN** sus dimensiones deben ser `6px × 40px` (clases `h-[6px] w-[40px]`), sin cambios respecto al estado actual.

---

### Requirement 3: Background Obsidian Glass

**User Story:** Como usuario de Voxa, quiero que la píldora tenga el efecto glassmorphism "Obsidian Glass" del design spec, para que se vea premium y moderna sobre cualquier fondo de pantalla.

#### Acceptance Criteria

1. **GIVEN** que el estado es `recording` (sin warning), **WHEN** se renderiza la píldora, **THEN** el background debe ser `rgba(10, 10, 10, 0.8)` (clase `bg-[#0A0A0A]/80`).

2. **GIVEN** cualquier estado activo de la píldora, **WHEN** se renderiza, **THEN** debe aplicarse `backdrop-filter: blur(40px)` (clase `backdrop-blur-[40px]`).

3. **GIVEN** cualquier estado activo de la píldora, **WHEN** se renderiza, **THEN** debe tener el borde `1px solid rgba(255, 255, 255, 0.1)` (clase `border border-white/10`).

4. **GIVEN** cualquier estado activo de la píldora, **WHEN** se renderiza, **THEN** debe tener la sombra `0 20px 50px rgba(0, 0, 0, 0.5)` (clase `shadow-[0_20px_50px_rgba(0,0,0,0.5)]`).

5. **GIVEN** que el estado es `recording` con `isWarning = true`, **WHEN** se renderiza la píldora, **THEN** el background debe cambiar a `bg-amber-600/80` para indicar el estado de advertencia.

---

### Requirement 4: Waveform con 5 barras

**User Story:** Como usuario grabando audio, quiero ver una visualización de forma de onda limpia y premium, para tener feedback visual de que el micrófono está capturando mi voz.

#### Acceptance Criteria

1. **GIVEN** que el estado es `recording`, **WHEN** se renderiza la waveform, **THEN** debe mostrar exactamente **5 barras** verticales (no 18).

2. **GIVEN** cualquier nivel de audio `[0.0, 1.0]` y cualquier timestamp, **WHEN** se calculan las alturas de las barras, **THEN** cada altura debe estar en el rango `[4px, 16px]`.

3. **GIVEN** que no hay audio (nivel = 0), **WHEN** se renderiza la waveform, **THEN** las barras deben mostrar una animación de "idle breath" sutil (oscilación mínima visible).

4. **GIVEN** que hay audio activo (nivel > 0.3), **WHEN** se renderiza la waveform, **THEN** las barras deben responder visualmente al nivel de audio con alturas variables.

---

### Requirement 5: Animación shimmer en processing

**User Story:** Como usuario esperando que se procese mi dictado, quiero ver una animación de shimmer premium en la píldora, para saber que el sistema está trabajando activamente.

#### Acceptance Criteria

1. **GIVEN** que el estado es `processing` o `refining`, **WHEN** se renderiza la píldora, **THEN** debe mostrar un efecto shimmer de gradiente blanco a 45° que barre de izquierda a derecha.

2. **GIVEN** el estado `processing`, **WHEN** se inspecciona el CSS, **THEN** la animación shimmer debe tener duración `1.5s`, timing `linear`, e iteración `infinite`.

3. **GIVEN** el estado `processing`, **WHEN** se renderiza la píldora, **THEN** el background base debe ser `bg-[#0A0A0A]/80` (no `bg-primary`), con el shimmer como overlay.

---

### Requirement 6: Transiciones de estado fluidas

**User Story:** Como usuario de Voxa, quiero que las transiciones entre estados de la píldora sean suaves y naturales, para que la experiencia se sienta premium y pulida.

#### Acceptance Criteria

1. **GIVEN** cualquier cambio de estado de la píldora, **WHEN** ocurre la transición, **THEN** debe haber una animación de entrada (`animate-in`) con duración mínima de `300ms`.

2. **GIVEN** que el estado cambia de `recording` a `processing`, **WHEN** ocurre la transición, **THEN** no debe haber un flash o parpadeo visible — la transición debe ser continua.

3. **GIVEN** el estado `idle`, **WHEN** se renderiza la píldora, **THEN** debe mostrar una animación de "breath" sutil (`animate-pill-breath`) con duración `3s ease-in-out infinite`.

4. **GIVEN** el estado `done`, **WHEN** han pasado 3 segundos, **THEN** la píldora debe hacer fade-out suave antes de volver al estado `idle`.

---

### Requirement 7: Estado idle con pointer-events-none

**User Story:** Como usuario de macOS, quiero que la píldora en estado idle no interfiera con mis clicks en aplicaciones debajo de ella, para que la experiencia sea no intrusiva.

#### Acceptance Criteria

1. **GIVEN** que el estado es `idle`, **WHEN** se renderiza la píldora, **THEN** el elemento raíz debe tener la clase `pointer-events-none`.

2. **GIVEN** que el estado es `idle`, **WHEN** el usuario hace click en el área de la píldora, **THEN** el click debe pasar al sistema operativo (aplicación debajo).

---

### Requirement 8: Tipografía premium en la píldora

**User Story:** Como usuario de Voxa, quiero que el texto en la píldora siga el design spec de tipografía, para que se vea consistente y profesional.

#### Acceptance Criteria

1. **GIVEN** cualquier estado activo con texto visible, **WHEN** se renderiza la píldora, **THEN** el texto debe ser `10px`, `font-bold`, `uppercase`, con `letter-spacing: 0.2em`.

2. **GIVEN** el estado `done`, **WHEN** se renderiza el icono de check, **THEN** el icono debe ser de color `text-primary` (violeta) en lugar de `text-white`.

---

### Requirement 9: Nuevas keyframes CSS en App.css

**User Story:** Como desarrollador, quiero que las animaciones premium estén definidas como keyframes reutilizables en App.css, para mantener la consistencia del design system.

#### Acceptance Criteria

1. **GIVEN** que se actualiza `App.css`, **WHEN** se inspeccionan las keyframes, **THEN** debe existir `@keyframes shimmer-sweep` con transform de `translateX(-150%)` a `translateX(250%)` con `skewX(-15deg)`.

2. **GIVEN** que se actualiza `App.css`, **WHEN** se inspeccionan las keyframes, **THEN** debe existir `@keyframes pill-breath` con oscilación de opacidad `0.5 → 0.7` y scaleX `1 → 1.05`.

3. **GIVEN** que se actualiza `App.css`, **WHEN** se inspeccionan las keyframes, **THEN** debe existir `@keyframes bar-grow` para las barras de la waveform.

---

### Requirement 10: useAudioLevel refactorizado a 5 barras

**User Story:** Como desarrollador, quiero que el hook `useAudioLevel` retorne exactamente 5 valores de altura, para alinearse con el design spec y reducir la carga de renderizado.

#### Acceptance Criteria

1. **GIVEN** que se refactoriza `useAudioLevel`, **WHEN** se llama al hook, **THEN** debe retornar un array de exactamente **5** números.

2. **GIVEN** cualquier nivel de audio `[0.0, 1.0]` y cualquier timestamp `> 0`, **WHEN** se ejecuta `computeBarHeights`, **THEN** todos los valores del array deben estar en `[4, 16]`.

3. **GIVEN** que `isRecording = false`, **WHEN** se llama al hook, **THEN** debe retornar `[4, 4, 4, 4, 4]` (todos en MIN_HEIGHT).

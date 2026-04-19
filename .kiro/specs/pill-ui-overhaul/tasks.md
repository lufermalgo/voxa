# Tasks: pill-ui-overhaul

## Task List

- [x] 1. Fix bug de layout del warning card (ventana crece hacia arriba)
  - [x] 1.1 Cambiar `PILL_WINDOW_HEIGHT_NORMAL` de 100 a 80 en `RecorderPill.tsx`
  - [x] 1.2 Invertir el orden DOM en el estado `recording`: renderizar `WarningCard` ANTES de la píldora en el JSX (posición DOM superior = visible arriba cuando la ventana crece hacia arriba)
  - [x] 1.3 Optimizar el `useEffect` de `setSize` para evitar llamadas redundantes (comparar con valor previo usando `useRef`)
  - [x] 1.4 Verificar manualmente que el warning card aparece al llegar al 80% del tiempo de grabación

- [x] 2. Rediseño visual de la píldora — dimensiones y glassmorphism
  - [x] 2.1 Cambiar altura de `h-7` (28px) a `h-12` (48px) en todos los estados activos
  - [x] 2.2 Cambiar `rounded-voxa` a `rounded-[24px]` en todos los estados activos
  - [x] 2.3 Reemplazar `bg-primary` por `bg-[#0A0A0A]/80` como background base en estados recording, processing, done, loading
  - [x] 2.4 Agregar `backdrop-blur-[40px]` a todos los estados activos
  - [x] 2.5 Agregar `border border-white/10` a todos los estados activos
  - [x] 2.6 Agregar `shadow-[0_20px_50px_rgba(0,0,0,0.5)]` a todos los estados activos
  - [x] 2.7 Cambiar el color del icono `check_circle` en estado `done` de `text-white` a `text-primary`

- [x] 3. Mejorar la animación de la waveform de detección de voz (18 barras)
  - [x] 3.1 Ajustar `MIN_HEIGHT_PX` de 2 a 3 y `MAX_HEIGHT_PX` de 20 a 18 para un rango más premium
  - [x] 3.2 Refinar el perfil de campana (`BAR_PROFILES`) para que las barras centrales sean más pronunciadas y las extremas más sutiles
  - [x] 3.3 Ajustar `IDLE_AMPLITUDE` para que la animación idle sea claramente visible pero no distractora
  - [x] 3.4 Cambiar el color de las barras de `bg-white` a un gradiente sutil: barras centrales `bg-white`, extremas `bg-white/50`, para dar profundidad visual

- [x] 4. Animación shimmer premium para estado processing
  - [x] 4.1 Agregar `@keyframes shimmer-sweep` a `App.css` (translateX -150% → 250% con skewX -15deg)
  - [x] 4.2 Agregar clase `.animate-shimmer-sweep` a `App.css` (1.5s linear infinite)
  - [x] 4.3 Reemplazar el overlay `bg-white/10 animate-pulse` en estado `processing` por un pseudo-elemento shimmer usando `::before` o un `div` absoluto con `animate-shimmer-sweep`
  - [x] 4.4 Asegurar que el shimmer usa `bg-gradient-to-r from-transparent via-white/15 to-transparent` con `skew-x-[-15deg]`

- [x] 5. Animaciones de idle y transiciones de estado
  - [x] 5.1 Agregar `@keyframes pill-breath` a `App.css` (opacity 0.5→0.7, scaleX 1→1.05, 3s ease-in-out infinite)
  - [x] 5.2 Agregar `@keyframes bar-grow` a `App.css` para las barras de waveform
  - [x] 5.3 Aplicar `animate-pill-breath` al estado `idle` en lugar del `animate-in` estático actual
  - [x] 5.4 Agregar `transition-all duration-500` al wrapper del estado `recording` para transiciones suaves de color (normal → warning)
  - [x] 5.5 Verificar que todas las entradas de estado usan `animate-in fade-in` con duración apropiada

- [x] 6. Escribir property-based tests con fast-check
  - [x] 6.1 Instalar `fast-check` si no está en `package.json`
  - [x] 6.2 Crear `src/hooks/__tests__/useAudioLevel.test.ts` con propiedad: ∀ audioLevel ∈ [0,1], ∀ timeMs > 0 → todas las alturas ∈ [MIN_HEIGHT, MAX_HEIGHT] y length = 18
  - [x] 6.3 Crear `src/hooks/__tests__/useRecordingDuration.test.ts` con propiedad: ∀ elapsed ≥ 0, ∀ maxSeconds > 0 → progress ∈ [0, 1]
  - [x] 6.4 Agregar propiedad en `useRecordingDuration.test.ts`: isWarning = (progress >= 0.8) para valores en el límite (0.79, 0.80, 0.81)
  - [x] 6.5 Crear `src/components/__tests__/RecorderPill.test.tsx` con ejemplo: dado isWarning=true, el WarningCard aparece antes de la píldora en el DOM

- [x] 7. Ajustes finales y limpieza
  - [x] 7.1 Eliminar la clase `.animate-shimmer` antigua de `App.css` (reemplazada por `animate-shimmer-sweep`)
  - [x] 7.2 Verificar que `pointer-events-none` está presente en el estado `idle`
  - [x] 7.3 Revisar que el padding del wrapper en `App.tsx` (`pb-5`) es suficiente para la píldora de 48px (ajustar a `pb-4` si es necesario para mantener el offset de 15px sobre el Dock)
  - [x] 7.4 Ejecutar `npm run build` para verificar que no hay errores de TypeScript ni de Tailwind
  - [x] 7.5 Ejecutar los tests con `npm run test -- --run` para verificar que todos los PBT pasan

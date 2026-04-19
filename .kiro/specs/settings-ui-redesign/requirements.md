# Requirements Document

## Introduction

Voxa es una app de dictado por voz para macOS construida con Tauri + React + Tailwind. El panel de configuración actual (`SettingsPanel.tsx`) contiene 5 secciones: History, Profiles, Dictionary, Models y General. Aunque el diseño ya sigue el sistema "Ethereal Curator" (fondo oscuro `#0A0A0A`, glassmorphism, Voxa Violet `#9D7AFF`, tipografía Inter/Outfit), existen oportunidades concretas de mejora visual: la jerarquía de información puede ser más clara, el uso del espacio puede ser más intencional, y la experiencia por sección puede ser más coherente con el nivel de sofisticación que el branding exige.

Esta propuesta de mejora visual mantiene estrictamente la identidad de marca y el sistema de diseño existentes. No introduce nuevas librerías ni cambia la arquitectura del componente. El objetivo es elevar la calidad visual percibida sin romper nada funcional.

## Glossary

- **Settings_Panel**: El componente `SettingsPanel.tsx` que contiene el panel de configuración completo de Voxa.
- **Sidebar**: La columna izquierda de navegación de 320px con los 5 tabs.
- **Content_Area**: El área principal derecha donde se renderiza el contenido de cada tab.
- **Header**: La barra superior con el logo de Voxa y el subtítulo de la app.
- **Footer**: La barra inferior con la versión de la app.
- **Tab**: Cada una de las 5 secciones navegables (History, Profiles, Dictionary, Models, General).
- **Profile_Card**: Tarjeta que representa un perfil de transformación en la sección Profiles.
- **History_Card**: Tarjeta que representa una transcripción en la sección History.
- **Shortcut_Key**: Elemento visual que muestra una combinación de teclas en la sección General.
- **Dictionary_Table**: Tabla de palabras del diccionario personal en la sección Dictionary.
- **Model_Card**: Tarjeta que representa un modelo de IA en la sección Models.
- **Empty_State**: Estado visual cuando una sección no tiene contenido (ej. historial vacío).
- **Confirm_Modal**: Modal de confirmación para acciones destructivas.
- **Voxa_Violet**: Color de acento primario `#9D7AFF`.
- **Ethereal_Curator**: Sistema de diseño de Voxa basado en glassmorphism, fondo oscuro y tipografía editorial.
- **Active_Tab**: El tab actualmente seleccionado en el Sidebar.
- **Edit_Drawer**: Panel expandible inline para editar un perfil.

---

## Requirements

### Requirement 1: Jerarquía visual del Header

**User Story:** Como usuario de Voxa, quiero que el header del panel de configuración comunique la identidad de la app con mayor impacto visual, para que la experiencia se sienta premium desde el primer momento.

#### Acceptance Criteria

1. THE Settings_Panel SHALL mostrar el logo de Voxa (squircle violeta con 5 barras) en el Header con un tamaño mínimo de 56×56px.
2. THE Settings_Panel SHALL aplicar un sutil gradiente o glow de `Voxa_Violet` detrás del logo en el Header para reforzar la identidad de marca.
3. THE Settings_Panel SHALL mostrar el nombre "Voxa" con tipografía `font-headline` en tamaño mínimo `text-2xl` y el subtítulo en `text-[10px] uppercase tracking-[0.15em]` con opacidad reducida.
4. WHEN el Header se renderiza, THE Settings_Panel SHALL separar visualmente el Header del resto del layout mediante un borde inferior sutil (`border-b border-white/[0.04]`) en lugar de un `glass-panel` genérico.
5. THE Settings_Panel SHALL mantener el Header con altura fija y sin scroll para que permanezca siempre visible.

---

### Requirement 2: Sidebar con navegación más expresiva

**User Story:** Como usuario de Voxa, quiero que la navegación lateral sea más clara y expresiva, para que pueda identificar rápidamente en qué sección estoy y navegar con confianza.

#### Acceptance Criteria

1. WHEN un Tab está activo, THE Sidebar SHALL mostrar el ítem con un fondo `bg-primary/10` y el ícono en color `text-primary` con variante `material-symbols-fill`, diferenciándolo claramente de los ítems inactivos.
2. WHEN un Tab está inactivo, THE Sidebar SHALL mostrar el ícono con opacidad `opacity-40` y transición suave a `opacity-100 text-primary` en hover.
3. THE Sidebar SHALL mostrar el indicador de tab activo como una línea vertical de 2px en el borde izquierdo del ítem (en lugar del punto circular actual), usando `bg-primary` y `rounded-full`.
4. THE Sidebar SHALL aplicar un ancho fijo de 280px (reducido desde 320px) para dar más espacio al Content_Area sin sacrificar legibilidad.
5. THE Sidebar SHALL mostrar el bloque de tip inferior con un diseño más compacto: ícono `lightbulb` de 16px + texto en una sola línea truncada, expandible en hover.
6. WHEN el usuario hace hover sobre el bloque de tip, THE Sidebar SHALL expandir el texto completo con una transición `max-height` suave de 300ms.

---

### Requirement 3: Content Area con mejor uso del espacio

**User Story:** Como usuario de Voxa, quiero que el área de contenido use el espacio de forma más intencional, para que cada sección se sienta organizada y fácil de escanear.

#### Acceptance Criteria

1. THE Content_Area SHALL aplicar padding horizontal de `px-10` y vertical de `py-10` (reducido desde `p-12`) para maximizar el espacio útil sin perder respiración.
2. THE Content_Area SHALL limitar el ancho máximo del contenido a `max-w-2xl` (reducido desde `max-w-3xl`) para mejorar la legibilidad en líneas de texto largas.
3. WHEN se cambia de Tab, THE Content_Area SHALL aplicar una animación de entrada `fade-in slide-in-from-right-2 duration-300` (más sutil que la actual `slide-in-from-right-4 duration-500`).
4. THE Content_Area SHALL mostrar el título de cada sección con `text-lg font-black` (reducido desde `text-xl`) y el subtítulo con `text-xs` para crear una jerarquía más compacta.
5. THE Content_Area SHALL usar `space-y-6` entre cards de una misma sección (reducido desde `space-y-8`) para un ritmo visual más ajustado.

---

### Requirement 4: Sección History con cards más legibles

**User Story:** Como usuario de Voxa, quiero que las tarjetas del historial de transcripciones sean más fáciles de leer y escanear, para que pueda encontrar y revisar mis dictados rápidamente.

#### Acceptance Criteria

1. THE History_Card SHALL mostrar la fecha y hora en la parte superior izquierda con formato `DD/MM/YYYY · HH:MM` usando `font-mono text-[10px] text-on-surface-variant/50`.
2. THE History_Card SHALL mostrar el texto de la transcripción con `text-sm leading-relaxed` y un máximo de 4 líneas visibles (`line-clamp-4`), con un botón "Ver más" si el texto es más largo.
3. WHEN el usuario hace hover sobre una History_Card, THE History_Card SHALL revelar los botones de acción (copiar, editar, eliminar) con una transición `opacity-0 → opacity-100` de 200ms.
4. THE History_Card SHALL aplicar padding de `p-6` (reducido desde `p-8`) y border-radius de `rounded-2xl` (reducido desde `rounded-voxa`) para un aspecto más compacto.
5. WHEN el historial está vacío, THE Empty_State SHALL mostrar el ícono `history` en tamaño `text-6xl` con opacidad `opacity-[0.06]`, el texto "Sin transcripciones" en `text-[10px] uppercase tracking-[0.3em]` y una descripción secundaria en `text-xs text-on-surface-variant/30`.
6. THE Settings_Panel SHALL mostrar el contador de transcripciones junto al título de la sección en formato `(N)` con `text-on-surface-variant/40`.

---

### Requirement 5: Sección Profiles con cards más impactantes

**User Story:** Como usuario de Voxa, quiero que las tarjetas de perfiles de transformación sean más visuales e impactantes, para que pueda identificar y seleccionar mi perfil activo de un vistazo.

#### Acceptance Criteria

1. THE Profile_Card SHALL mostrar el ícono del perfil en un contenedor de 48×48px con `rounded-2xl`, usando `bg-primary/15 text-primary` cuando está activo y `bg-surface-container text-on-surface-variant/40` cuando está inactivo.
2. WHEN un Profile_Card está activo, THE Profile_Card SHALL mostrar un borde izquierdo de 3px en `bg-primary` (accent bar) en lugar del `scale-[1.01]` actual, para indicar selección sin distorsionar el layout.
3. THE Profile_Card SHALL mostrar el prompt del perfil truncado a 1 línea con `line-clamp-1 text-[11px] italic text-on-surface-variant/50`, sin las comillas decorativas actuales.
4. WHEN el Edit_Drawer está abierto, THE Edit_Drawer SHALL usar `rounded-3xl` (reducido desde `rounded-[3rem]`) y padding de `p-8` para un aspecto más integrado con las cards.
5. THE Settings_Panel SHALL mostrar el botón "Crear nuevo perfil" con un diseño de card vacía con borde punteado `border-dashed border-2 border-surface-container-high`, altura fija de 80px y el ícono `add` centrado.
6. WHEN el usuario hace hover sobre el botón de crear perfil, THE Settings_Panel SHALL cambiar el borde a `border-primary/30` y el ícono a `text-primary` con transición de 200ms.

---

### Requirement 6: Sección Dictionary con tabla más refinada

**User Story:** Como usuario de Voxa, quiero que la tabla del diccionario personal sea más limpia y fácil de usar, para que pueda gestionar mis palabras personalizadas sin fricción.

#### Acceptance Criteria

1. THE Dictionary_Table SHALL mostrar las filas con altura mínima de 48px y separación visual mediante `border-b border-on-surface/[0.04]` (más sutil que el actual `border-on-surface/5`).
2. THE Dictionary_Table SHALL mostrar la columna "Word" con `font-bold text-sm text-on-surface` y la columna "Replacement" con un input inline de estilo minimalista (`bg-transparent border-none focus:bg-background/40 rounded-lg px-3 py-1.5`).
3. THE Dictionary_Table SHALL mostrar el contador de usos como un badge `bg-primary/10 text-primary text-[10px] font-black px-2 py-0.5 rounded-full` solo cuando `usage_count > 0`.
4. WHEN el usuario hace hover sobre una fila de la Dictionary_Table, THE Dictionary_Table SHALL revelar el botón de eliminar con transición `opacity-0 → opacity-100` de 150ms.
5. THE Settings_Panel SHALL mostrar el input de nueva palabra y el botón "Añadir" en una barra fija en la parte inferior del área del diccionario, con `sticky bottom-0 bg-background/80 backdrop-blur-xl pt-4`.
6. IF el campo de nueva palabra está vacío, THEN THE Settings_Panel SHALL deshabilitar el botón "Añadir" con `opacity-40 cursor-not-allowed`.

---

### Requirement 7: Sección Models con información más clara

**User Story:** Como usuario de Voxa, quiero que la sección de modelos muestre el estado de cada modelo de forma más clara y visual, para que pueda entender qué está instalado y qué necesita descargarse.

#### Acceptance Criteria

1. THE Model_Card SHALL mostrar el nombre del modelo con `text-sm font-black text-on-surface` y el nombre de archivo con `text-[10px] font-mono text-on-surface-variant/40 mt-0.5`.
2. THE Model_Card SHALL mostrar el estado del modelo como un badge: `bg-primary/10 text-primary` para "Descargado" y `bg-error/10 text-error` para "Faltante", ambos con `text-[9px] font-black uppercase tracking-[0.15em] px-2.5 py-1 rounded-full`.
3. THE Model_Card SHALL mostrar el tamaño del modelo con `text-[11px] font-black text-on-surface-variant/60` alineado a la derecha.
4. WHEN se está descargando un modelo, THE Settings_Panel SHALL mostrar una barra de progreso con altura de 3px, color `bg-primary`, y un glow sutil `shadow-[0_0_8px_rgba(157,122,255,0.4)]`.
5. THE Settings_Panel SHALL mostrar la ruta base de los modelos en un bloque con `font-mono text-[11px] bg-background/60 rounded-xl p-4 border border-surface-container-high/60`, con un botón de copiar al portapapeles inline.
6. THE Settings_Panel SHALL agrupar los modelos bajo un label `text-[10px] font-black uppercase tracking-widest text-on-surface-variant/40` que diga "Modelos de IA instalados".

---

### Requirement 8: Sección General con settings cards más consistentes

**User Story:** Como usuario de Voxa, quiero que las tarjetas de configuración general tengan un diseño más consistente y refinado, para que la sección se sienta cohesiva y fácil de usar.

#### Acceptance Criteria

1. THE Settings_Panel SHALL mostrar cada setting card con padding `p-6` (reducido desde `p-8`), `rounded-2xl` y `bg-surface-container-low/40` sin borde explícito, usando solo la diferencia tonal para delimitar.
2. THE Settings_Panel SHALL mostrar el ícono de cada setting card en un contenedor de 40×40px con `rounded-xl bg-primary/8 text-primary` (reducido desde 48×48px).
3. THE Shortcut_Key SHALL mostrar cada atajo de teclado con `text-lg font-black tracking-tight` y el label en `text-[10px] uppercase tracking-widest text-on-surface-variant/60`.
4. WHEN un Shortcut_Key está en modo captura, THE Shortcut_Key SHALL mostrar un fondo `bg-primary/15 ring-1 ring-primary/30` con un indicador de pulso animado en la esquina superior derecha.
5. THE Settings_Panel SHALL mostrar el toggle de "Auto-detect profile" con un diseño de switch de 52×28px, usando `bg-primary` cuando está activo y `bg-surface-container-high` cuando está inactivo, con la bolita blanca de 20×20px.
6. THE Settings_Panel SHALL mostrar los botones de selección de idioma (ES/EN) con altura de `py-6` (reducido desde `py-8`) y `rounded-2xl`, manteniendo el estilo de selección activa con `bg-primary text-background`.

---

### Requirement 9: Footer más discreto y funcional

**User Story:** Como usuario de Voxa, quiero que el footer del panel sea más discreto y no compita visualmente con el contenido principal, para que la atención permanezca en las secciones de configuración.

#### Acceptance Criteria

1. THE Footer SHALL mostrar la versión de la app con `text-[9px] font-mono text-on-surface-variant/30 tracking-widest` (más sutil que el actual `/60`).
2. THE Footer SHALL tener altura fija de 40px con padding `px-8 py-0` y `border-t border-white/[0.03]` como separador superior.
3. THE Footer SHALL usar `bg-transparent` en lugar de `bg-surface-container-low/60` para no crear una banda visual pesada en la parte inferior.

---

### Requirement 10: Confirm Modal con diseño coherente con el sistema

**User Story:** Como usuario de Voxa, quiero que el modal de confirmación de acciones destructivas sea visualmente coherente con el sistema de diseño Ethereal Curator, para que la experiencia no se sienta interrumpida por un elemento genérico.

#### Acceptance Criteria

1. THE Confirm_Modal SHALL usar `bg-surface-container-low/90 backdrop-blur-2xl` como fondo del panel (en lugar del `bg-surface` plano actual).
2. THE Confirm_Modal SHALL aplicar `rounded-3xl` y `ring-1 ring-white/[0.06]` para integrarse con el lenguaje visual del resto del panel.
3. THE Confirm_Modal SHALL mostrar el título con `text-sm font-black uppercase tracking-widest text-on-surface` y la descripción con `text-xs text-on-surface-variant/70 leading-relaxed`.
4. THE Confirm_Modal SHALL mostrar el botón de confirmación destructiva con `bg-error/90 text-white rounded-2xl` y el botón de cancelar con `bg-on-surface/5 text-on-surface-variant rounded-2xl`.
5. WHEN el Confirm_Modal se abre, THE Confirm_Modal SHALL aparecer con animación `zoom-in-95 fade-in duration-200` sobre un overlay `bg-black/40 backdrop-blur-sm`.

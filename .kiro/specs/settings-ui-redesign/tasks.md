# Implementation Tasks: Settings UI — Editorial Redesign

## Overview

Implementación del rediseño editorial del `SettingsPanel.tsx`. Cada tarea es independiente y puede implementarse sin romper la funcionalidad existente. El orden sigue de afuera hacia adentro: estructura global → secciones individuales.

---

- [ ] 1. Layout global y estructura base
  - [ ] 1.1 Reducir header a 48px con logo 36px inline y versión a la derecha
  - [ ] 1.2 Reducir sidebar a 200px con items compactos (py-2 px-3) y barra izquierda activa
  - [ ] 1.3 Ajustar content area: padding px-10 py-8, eliminar max-w-3xl
  - [ ] 1.4 Reducir footer a 32px transparente con solo versión

- [ ] 2. Sistema de rows y controles inline
  - [ ] 2.1 Crear componente SettingRow (label + control en flex justify-between, min-h-[40px])
  - [ ] 2.2 Crear componente GroupHeader (text-[9px] uppercase tracking-[0.35em] + border-t)
  - [ ] 2.3 Rediseñar Toggle switch a 40×22px
  - [ ] 2.4 Rediseñar Shortcut key a estilo monospace compacto con estado de captura

- [ ] 3. Sección General — rows inline
  - [ ] 3.1 Convertir Audio card a row inline con select compacto
  - [ ] 3.2 Convertir Shortcuts card a rows inline (4 filas, una por shortcut)
  - [ ] 3.3 Convertir Auto-detect toggle a row inline
  - [ ] 3.4 Convertir Language selector a row inline con pills pequeños

- [ ] 4. Sección History — feed tipo log
  - [ ] 4.1 Agrupar transcripciones por fecha (Hoy / Ayer / fecha)
  - [ ] 4.2 Convertir cards a filas de 44px con timestamp + texto truncado + acciones hover
  - [ ] 4.3 Implementar expand/collapse al hacer click en la fila
  - [ ] 4.4 Actualizar empty state a versión compacta centrada

- [ ] 5. Sección Profiles — lista con selector inline
  - [ ] 5.1 Convertir profile cards a filas con radio visual + nombre + prompt preview
  - [ ] 5.2 Reducir edit drawer: rounded-xl, p-5, sin rounded-[3rem]
  - [ ] 5.3 Convertir botón "crear perfil" a link inline compacto

- [ ] 6. Sección Dictionary — tabla ultra-compacta
  - [ ] 6.1 Reducir filas a min-h-[36px] con input de replacement transparente
  - [ ] 6.2 Hacer sticky la barra de añadir palabra (bottom-0, backdrop-blur)

- [ ] 7. Sección Models — lista simple
  - [ ] 7.1 Convertir model cards a filas de 40px con nombre + filename + tamaño + badge
  - [ ] 7.2 Mover path a row inline con botones de copiar y abrir carpeta
  - [ ] 7.3 Reducir progress bar a 3px con glow sutil

- [ ] 8. Confirm Modal — glassmorphism coherente
  - [ ] 8.1 Actualizar modal a rounded-2xl, backdrop-blur-2xl, ring-1 ring-white/[0.06]
  - [ ] 8.2 Ajustar animación de entrada a zoom-in-95 fade-in duration-150

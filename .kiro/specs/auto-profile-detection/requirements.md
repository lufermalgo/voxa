# Requirements: auto-profile-detection

## Introduction

Este documento define los requisitos para la funcionalidad de **auto-selección de perfil de transformación basada en el contexto de la app activa** en Voxa.

Actualmente el usuario debe seleccionar manualmente el perfil de transformación (Elegant, Informal, Code, Custom) antes de dictar. Esta feature detecta automáticamente en qué app o contexto web está trabajando el usuario y selecciona el perfil más adecuado, con feedback visual claro y la posibilidad de hacer override rápido sin ir a Settings.

### Evaluación crítica del approach técnico actual

El backend Rust ya tiene implementación parcial de auto-detección. Sin embargo, se identificaron los siguientes problemas que los requisitos deben corregir:

1. **Inconsistencia semántica en el mapeo de email**: `bundle_id_to_profile_keyword` mapea `com.apple.mail` → "Elegant", pero `domain_to_profile_keyword` mapea `mail.google.com` → "Informal". El email es un contexto formal; debe mapearse consistentemente a "Elegant".

2. **Timing de detección incorrecto**: La detección actual ocurre en `resolve_system_prompt`, que se llama en `StopRecording`. Si el usuario inicia la grabación en VS Code y para en otra app, el perfil detectado puede ser incorrecto. La detección debe ocurrir al **inicio** de la grabación (`StartRecording`), cuando se captura el `FrontmostApp`.

3. **Sin feedback al frontend**: No se emite ningún evento al frontend cuando se auto-detecta un perfil. El usuario no sabe qué perfil se está usando.

4. **La pill no muestra el perfil activo**: No hay indicador visual de qué perfil está seleccionado ni si fue auto-detectado o manual.

5. **Override manual sin UX de acceso rápido**: El único camino para cambiar el perfil es ir a Settings → Profiles, lo cual interrumpe el flujo de trabajo.

---

## Glossary

- **Profile_Detector**: El subsistema (Rust backend) responsable de determinar qué perfil de transformación aplicar según el contexto de la app activa.
- **Active_App**: La aplicación macOS que tenía el foco inmediatamente antes de que el usuario iniciara la grabación.
- **Bundle_ID**: Identificador único de una aplicación macOS (ej: `com.microsoft.vscode`).
- **Domain**: El hostname de la URL activa en el tab del browser (ej: `github.com`).
- **Auto_Profile**: El perfil seleccionado automáticamente por el Profile_Detector basado en el contexto.
- **Manual_Override**: La selección explícita de un perfil por parte del usuario, que tiene prioridad sobre el Auto_Profile.
- **Profile_Pill**: El indicador visual en la RecorderPill que muestra el perfil activo.
- **Profile_Picker**: El selector rápido de perfil accesible desde la RecorderPill sin abrir Settings.
- **Default_Profile**: El perfil configurado por el usuario en Settings como fallback cuando no hay match de contexto.
- **Session**: El período desde que se abre Voxa hasta que se cierra.

---

## Requirements

### Requirement 1: Detección de perfil al inicio de la grabación

**User Story:** Como usuario de Voxa, quiero que el perfil correcto se seleccione automáticamente cuando inicio una grabación, para no tener que cambiar manualmente el perfil según la app en la que estoy trabajando.

#### Acceptance Criteria

1. WHEN el usuario inicia una grabación, THE Profile_Detector SHALL determinar el perfil a usar en ese momento (no al finalizar la grabación).

2. WHEN el usuario inicia una grabación y no existe un Manual_Override activo, THE Profile_Detector SHALL usar el Bundle_ID de la Active_App para determinar el perfil.

3. WHEN la Active_App es un browser conocido y no existe un Manual_Override activo, THE Profile_Detector SHALL usar el Domain del tab activo para determinar el perfil, con prioridad sobre el Bundle_ID del browser.

4. WHEN el Profile_Detector no encuentra un match de Bundle_ID ni de Domain, THE Profile_Detector SHALL usar el Default_Profile configurado por el usuario.

5. WHEN el usuario inicia una grabación, THE Profile_Detector SHALL emitir un evento `profile-detected` al frontend con el nombre del perfil seleccionado y si fue auto-detectado o manual.

---

### Requirement 2: Mapeo de contextos a perfiles

**User Story:** Como usuario de Voxa, quiero que los contextos más comunes se mapeen al perfil correcto, para que el texto dictado tenga el tono adecuado sin configuración manual.

#### Acceptance Criteria

1. WHEN la Active_App tiene Bundle_ID `com.microsoft.vscode`, `com.todesktop.230313mzl4w4u92` (Cursor), `com.apple.dt.xcode`, o cualquier Bundle_ID que comience con `com.jetbrains.`, THE Profile_Detector SHALL seleccionar el perfil "Code".

2. WHEN el Domain activo es `github.com`, `gitlab.com`, `linear.app`, `bitbucket.org`, o termina en `.atlassian.net`, THE Profile_Detector SHALL seleccionar el perfil "Code".

3. WHEN el Domain activo es `claude.ai`, `chat.openai.com`, o `chatgpt.com`, THE Profile_Detector SHALL seleccionar el perfil "Code".

4. WHEN la Active_App tiene Bundle_ID `com.tinyspeck.slackmacgap`, `com.hnc.discord`, `com.microsoft.teams2`, o `ru.keepcoder.telegram`, THE Profile_Detector SHALL seleccionar el perfil "Informal".

5. WHEN el Domain activo termina en `.slack.com`, es `discord.com`, `twitter.com`, o `x.com`, THE Profile_Detector SHALL seleccionar el perfil "Informal".

6. WHEN la Active_App tiene Bundle_ID `com.apple.mail`, `com.microsoft.outlook`, `notion.id`, `com.apple.notes`, `md.obsidian`, o `com.evernote.evernote`, THE Profile_Detector SHALL seleccionar el perfil "Elegant".

7. WHEN el Domain activo es `mail.google.com`, `docs.google.com`, `notion.so`, `coda.io`, o contiene `confluence`, THE Profile_Detector SHALL seleccionar el perfil "Elegant".

8. WHEN el Domain activo contiene `outlook.` o es `outlook.com`, THE Profile_Detector SHALL seleccionar el perfil "Elegant".

> **Nota de diseño**: El email (Gmail, Outlook, Apple Mail) se mapea a "Elegant" — no a "Informal" — porque el email es un contexto de comunicación formal. Esto corrige la inconsistencia del código actual donde `mail.google.com` estaba mapeado a "Informal".

---

### Requirement 3: Indicador visual del perfil activo en la RecorderPill

**User Story:** Como usuario de Voxa, quiero ver qué perfil está activo mientras grabo, para tener confianza de que el texto se procesará con el tono correcto.

#### Acceptance Criteria

1. WHEN el estado de la RecorderPill es `recording`, THE Profile_Pill SHALL mostrar el nombre del perfil activo (auto-detectado o manual).

2. WHEN el perfil fue auto-detectado (no manual), THE Profile_Pill SHALL mostrar un indicador visual que distinga el perfil auto-detectado del manual (ej: un ícono de "auto" o un color diferente).

3. WHEN el estado de la RecorderPill es `idle`, THE Profile_Pill SHALL mostrar el nombre del perfil que se usará en la próxima grabación.

4. WHEN el perfil activo cambia (por auto-detección o por Manual_Override), THE Profile_Pill SHALL actualizar su contenido sin requerir interacción del usuario.

5. IF el nombre del perfil tiene más de 8 caracteres, THEN THE Profile_Pill SHALL truncar el nombre con elipsis para no romper el layout de la píldora.

---

### Requirement 4: Override rápido de perfil desde la RecorderPill

**User Story:** Como usuario de Voxa, quiero poder cambiar el perfil rápidamente desde la píldora sin abrir Settings, para ajustar el tono de mi dictado en segundos.

#### Acceptance Criteria

1. WHEN el usuario hace click en el Profile_Pill durante el estado `idle` o `recording`, THE Profile_Picker SHALL mostrarse como un popover flotante sobre la píldora.

2. WHEN el Profile_Picker está visible, THE Profile_Picker SHALL mostrar todos los perfiles disponibles con su nombre e ícono.

3. WHEN el usuario selecciona un perfil en el Profile_Picker, THE Profile_Detector SHALL establecer ese perfil como Manual_Override para la sesión actual.

4. WHEN el usuario selecciona un perfil en el Profile_Picker, THE Profile_Picker SHALL cerrarse automáticamente.

5. WHEN el usuario selecciona un perfil en el Profile_Picker durante una grabación activa, THE Profile_Detector SHALL usar ese perfil para la grabación en curso.

6. WHEN el Profile_Picker está visible y el usuario hace click fuera de él, THE Profile_Picker SHALL cerrarse sin cambiar el perfil activo.

---

### Requirement 5: Persistencia y prioridad del Manual_Override

**User Story:** Como usuario de Voxa, quiero que mi selección manual de perfil se respete durante toda la sesión, para no tener que re-seleccionarlo en cada grabación.

#### Acceptance Criteria

1. WHEN el usuario establece un Manual_Override, THE Profile_Detector SHALL usar ese perfil en todas las grabaciones subsiguientes de la sesión, ignorando la auto-detección.

2. WHEN la aplicación Voxa se reinicia, THE Profile_Detector SHALL limpiar el Manual_Override y volver a la auto-detección (el override es por sesión, no persistente).

3. WHEN existe un Manual_Override activo, THE Profile_Pill SHALL mostrar el nombre del perfil con un indicador visual que lo distinga del perfil auto-detectado (ej: sin el ícono de "auto").

4. WHEN existe un Manual_Override activo y el usuario quiere volver a la auto-detección, THE Profile_Picker SHALL ofrecer una opción explícita "Auto" para limpiar el override.

---

### Requirement 6: Configuración de auto-detección en Settings

**User Story:** Como usuario de Voxa, quiero poder desactivar la auto-detección de perfil desde Settings, para usar siempre el perfil que yo elija manualmente.

#### Acceptance Criteria

1. THE Settings_Panel SHALL mostrar un toggle "Auto-detect profile" en la sección de perfiles.

2. WHEN el toggle "Auto-detect profile" está desactivado, THE Profile_Detector SHALL usar siempre el perfil marcado como activo en Settings, ignorando el Bundle_ID y el Domain.

3. WHEN el toggle "Auto-detect profile" está activado, THE Profile_Detector SHALL aplicar la lógica de detección automática descrita en los Requisitos 1 y 2.

4. WHEN el usuario cambia el estado del toggle, THE Settings_Panel SHALL persistir el cambio en la base de datos (setting `auto_detect_profile`).

5. WHEN el toggle "Auto-detect profile" está desactivado, THE Profile_Pill SHALL mostrar el perfil activo sin el indicador de "auto".

---

### Requirement 7: Detección de URL del browser activo

**User Story:** Como usuario de Voxa que trabaja en el browser, quiero que Voxa detecte en qué web app estoy para seleccionar el perfil correcto, para que el tono sea adecuado aunque use el mismo browser para todo.

#### Acceptance Criteria

1. WHEN la Active_App es un browser soportado (Safari, Chrome, Brave, Arc, Edge, Firefox), THE Profile_Detector SHALL intentar leer el Domain del tab activo via la macOS Accessibility API.

2. WHEN la lectura del Domain falla (permisos denegados, browser en estado inusual, timeout), THE Profile_Detector SHALL usar el Default_Profile sin mostrar un error al usuario.

3. WHEN la lectura del Domain tiene éxito pero el Domain no tiene un mapeo definido, THE Profile_Detector SHALL usar el Default_Profile.

4. WHEN la Active_App es un browser y el Domain tiene un mapeo definido, THE Profile_Detector SHALL usar el perfil mapeado al Domain con prioridad sobre el Bundle_ID del browser.

5. IF el usuario no ha concedido permisos de Accessibility a Voxa, THEN THE Profile_Detector SHALL funcionar correctamente usando solo Bundle_ID (sin detección de URL), sin crashear ni mostrar errores.

---

### Requirement 8: Rendimiento de la detección

**User Story:** Como usuario de Voxa, quiero que la auto-detección de perfil no añada latencia perceptible al inicio de la grabación, para que la experiencia sea fluida.

#### Acceptance Criteria

1. WHEN el usuario inicia una grabación, THE Profile_Detector SHALL completar la detección de perfil en menos de 100ms.

2. WHEN la detección de URL del browser requiere acceso a la Accessibility API, THE Profile_Detector SHALL ejecutar esa operación de forma que no bloquee el hilo principal de audio.

3. IF la detección de URL del browser tarda más de 50ms, THEN THE Profile_Detector SHALL usar el Default_Profile como fallback sin esperar el resultado.

---

### Requirement 9: Compatibilidad con el flujo de grabación existente

**User Story:** Como usuario de Voxa, quiero que la auto-detección de perfil funcione de forma transparente con el flujo de grabación existente, para que no rompa ninguna funcionalidad actual.

#### Acceptance Criteria

1. WHEN la auto-detección está activa y el usuario graba, THE Pipeline SHALL usar el perfil auto-detectado para la refinación LLM exactamente igual que si el usuario lo hubiera seleccionado manualmente.

2. WHEN la auto-detección está desactivada, THE Pipeline SHALL comportarse exactamente igual que antes de esta feature (usando `active_profile_id` de Settings).

3. WHEN el perfil auto-detectado no existe en la base de datos (ej: el usuario borró el perfil "Code"), THE Profile_Detector SHALL usar el Default_Profile como fallback.

4. THE Profile_Detector SHALL ser compatible con perfiles personalizados creados por el usuario, no solo con los 4 perfiles por defecto.

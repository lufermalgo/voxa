Implementación de un Shortcut Selector en Tauri (macOS, Linux y Windows)
Objetivo

Implementar un selector de atajos de teclado donde el usuario pueda definir la combinación que quiere usar para una funcionalidad, capturar esa combinación desde la interfaz, convertirla a un formato compatible con Tauri, registrarla como global shortcut, y persistirla para reutilizarla entre reinicios.
En Tauri 2, el mecanismo correcto para shortcuts globales es el plugin oficial @tauri-apps/plugin-global-shortcut, que permite registrar, consultar y desregistrar atajos. El plugin funciona en desktop y requiere permisos explícitos en capabilities.

Qué problema estás resolviendo realmente

Hay dos cosas distintas que suelen mezclarse:

Capturar la combinación que el usuario presiona en la UI.
Registrar esa combinación como shortcut global en el sistema.

No conviene intentar resolver ambas cosas con la misma capa.

La captura debe hacerse en el frontend con keydown, usando KeyboardEvent.key y las banderas ctrlKey, altKey, shiftKey y metaKey. KeyboardEvent.keyCode no debe usarse: está obsoleto y además es inconsistente para teclas imprimibles y layouts distintos. KeyboardEvent.key sí refleja la tecla presionada considerando modificadores y layout.

Enfoque recomendado
Arquitectura
Frontend
muestra un input tipo “Press shortcut”
escucha keydown
construye una representación interna del shortcut
la normaliza al formato que espera Tauri
Tauri
registra el shortcut con register(...)
elimina el anterior con unregister(...) si el usuario lo cambia
opcionalmente verifica si ya lo tiene registrado la app con isRegistered(...)
Persistencia
guardar el accelerator seleccionado
restaurarlo al iniciar la app

La persistencia en Tauri 2 se puede resolver bien con el plugin store, que provee un key-value store persistente en archivo, reutilizable entre reinicios.

Decisión importante: usar formato accelerator

Tauri espera shortcuts en formato string como estos:

CommandOrControl+Shift+C
Alt+A

Ese es el formato que usa el plugin oficial en JavaScript para register, unregister e isRegistered. Además, el ejemplo oficial usa CommandOrControl+Shift+C, que es precisamente el alias útil para soportar macOS y Windows/Linux con una sola definición lógica.

Flujo completo
1. El usuario entra en modo captura

Ejemplo: hace click en “Configurar atajo”.

2. El frontend escucha keydown

En ese momento:

haces preventDefault()
ignoras repeticiones (event.repeat)
detectas modificadores
detectas la tecla principal
armas el accelerator
3. Validas la combinación

Reglas recomendadas:

no permitir solo modificadores (Ctrl, Shift, Alt, Meta)
exigir al menos una tecla no modificadora
preferiblemente exigir al menos un modificador para evitar combinaciones débiles como A
bloquear Escape para salir del modo captura
permitir Backspace o Delete como acción de “limpiar shortcut”
4. Re-registras

Cuando el usuario confirma:

desregistras el shortcut anterior
registras el nuevo
lo guardas en persistencia
5. Restauras al iniciar

Lees el valor guardado y lo vuelves a registrar.

Implementación base
1) Instalar plugin de global shortcuts

Tauri documenta el plugin global-shortcut como la vía oficial para registrar atajos globales, y el setup incluye dependencia Rust, guest bindings JS e inicialización en lib.rs.

npm run tauri add global-shortcut

Si haces setup manual, el flujo oficial incluye:

dependencia Rust tauri-plugin-global-shortcut
inicialización del plugin en src-tauri/src/lib.rs
instalación de @tauri-apps/plugin-global-shortcut en el frontend
2) Inicializar el plugin en Rust
// src-tauri/src/lib.rs
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

El patrón anterior sigue el setup oficial del plugin.

3) Habilitar permisos en capabilities

En Tauri 2, los comandos del plugin no quedan expuestos por defecto. Debes habilitar permisos en src-tauri/capabilities/default.json. Para shortcuts globales, lo mínimo razonable es permitir consultar, registrar y desregistrar.

{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "main-capability",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "global-shortcut:allow-is-registered",
    "global-shortcut:allow-register",
    "global-shortcut:allow-unregister"
  ]
}
4) Captura en frontend

La captura debe hacerse con keydown, no con input. KeyboardEvent describe la interacción de teclado; key devuelve la tecla efectiva teniendo en cuenta modificadores y layout. metaKey, ctrlKey, altKey y shiftKey indican si esos modificadores estaban activos.

Representación interna sugerida
type ShortcutDraft = {
  meta: boolean
  ctrl: boolean
  alt: boolean
  shift: boolean
  key: string | null
}
Helpers de captura
function isModifierKey(key: string): boolean {
  return ["Meta", "Control", "Alt", "Shift"].includes(key)
}

function normalizeMainKey(key: string): string | null {
  if (!key) return null

  // Casos especiales
  if (key === " ") return "Space"
  if (key === "Escape") return "Esc"

  // Ignorar teclas puramente modificadoras
  if (isModifierKey(key)) return null

  // Letras
  if (key.length === 1) {
    return key.toUpperCase()
  }

  // Mantener nombres comunes
  return key
}
Construcción del accelerator
function toTauriAccelerator(draft: ShortcutDraft): string | null {
  if (!draft.key) return null

  const parts: string[] = []

  // Estrategia cross-platform:
  // CommandOrControl sirve para macOS y Windows/Linux
  if (draft.meta || draft.ctrl) {
    parts.push("CommandOrControl")
  }

  if (draft.alt) parts.push("Alt")
  if (draft.shift) parts.push("Shift")

  parts.push(draft.key)

  return parts.join("+")
}
Captura desde el input
function captureShortcut(event: KeyboardEvent): string | null {
  event.preventDefault()

  if (event.repeat) return null

  // Escape cancela
  if (event.key === "Escape") {
    return null
  }

  const mainKey = normalizeMainKey(event.key)

  // No aceptar solo modificadores
  if (!mainKey) return null

  const draft: ShortcutDraft = {
    meta: event.metaKey,
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    key: mainKey
  }

  // Regla opcional pero recomendada:
  // exigir al menos un modificador
  const hasModifier =
    draft.meta || draft.ctrl || draft.alt || draft.shift

  if (!hasModifier) {
    return null
  }

  return toTauriAccelerator(draft)
}
5) Registro del shortcut en Tauri

La API JavaScript del plugin permite:

register(shortcuts, handler)
unregister(shortcuts)
unregisterAll()
isRegistered(shortcut)

También hay una limitación importante: isRegistered() solo indica si tu aplicación lo tiene registrado; si otra aplicación ya ocupa ese shortcut, isRegistered() seguirá devolviendo false. Además, la documentación advierte que si el shortcut ya está tomado por otra app, el handler no se disparará. Eso significa que la detección de conflictos con terceros no es confiable solo con isRegistered().

import {
  register,
  unregister,
  isRegistered
} from "@tauri-apps/plugin-global-shortcut"

let currentShortcut: string | null = null

export async function applyShortcut(newShortcut: string) {
  if (currentShortcut) {
    await unregister(currentShortcut)
  }

  await register(newShortcut, (event) => {
    if (event.state === "Pressed") {
      console.log("Shortcut ejecutado:", event.shortcut)
      // dispara aquí tu funcionalidad
    }
  })

  currentShortcut = newShortcut
}
6) Persistencia del shortcut

El plugin store de Tauri 2 provee almacenamiento key-value persistente y puede usarse desde frontend o Rust. La carga y guardado son asíncronos, y el store puede salvarse manualmente o al cierre limpio de la app.

Instalación
npm run tauri add store
Inicialización Rust
// src-tauri/src/lib.rs
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
Uso desde frontend
import { load } from "@tauri-apps/plugin-store"

const store = await load("settings.json", { autoSave: false })

export async function saveShortcut(shortcut: string) {
  await store.set("mainShortcut", shortcut)
  await store.save()
}

export async function readShortcut(): Promise<string | null> {
  return (await store.get<string>("mainShortcut")) ?? null
}
Restaurar al iniciar
export async function restoreShortcutOnStartup() {
  const saved = await readShortcut()
  if (!saved) return

  await applyShortcut(saved)
}
Implementación completa sugerida
import {
  register,
  unregister
} from "@tauri-apps/plugin-global-shortcut"
import { load } from "@tauri-apps/plugin-store"

type ShortcutDraft = {
  meta: boolean
  ctrl: boolean
  alt: boolean
  shift: boolean
  key: string | null
}

let currentShortcut: string | null = null
const storePromise = load("settings.json", { autoSave: false })

function isModifierKey(key: string): boolean {
  return ["Meta", "Control", "Alt", "Shift"].includes(key)
}

function normalizeMainKey(key: string): string | null {
  if (!key) return null
  if (key === " ") return "Space"
  if (key === "Escape") return "Esc"
  if (isModifierKey(key)) return null
  if (key.length === 1) return key.toUpperCase()
  return key
}

function toTauriAccelerator(draft: ShortcutDraft): string | null {
  if (!draft.key) return null

  const parts: string[] = []

  if (draft.meta || draft.ctrl) parts.push("CommandOrControl")
  if (draft.alt) parts.push("Alt")
  if (draft.shift) parts.push("Shift")

  parts.push(draft.key)

  return parts.join("+")
}

export function parseShortcutFromKeydown(event: KeyboardEvent): string | null {
  event.preventDefault()

  if (event.repeat) return null

  if (event.key === "Escape") {
    return null
  }

  const mainKey = normalizeMainKey(event.key)
  if (!mainKey) return null

  const draft: ShortcutDraft = {
    meta: event.metaKey,
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    key: mainKey
  }

  const hasModifier =
    draft.meta || draft.ctrl || draft.alt || draft.shift

  if (!hasModifier) return null

  return toTauriAccelerator(draft)
}

export async function applyShortcut(shortcut: string, handler: () => void) {
  if (currentShortcut) {
    await unregister(currentShortcut)
  }

  await register(shortcut, (event) => {
    if (event.state === "Pressed") {
      handler()
    }
  })

  currentShortcut = shortcut

  const store = await storePromise
  await store.set("mainShortcut", shortcut)
  await store.save()
}

export async function restoreShortcut(handler: () => void) {
  const store = await storePromise
  const saved = await store.get<string>("mainShortcut")

  if (!saved) return

  await register(saved, (event) => {
    if (event.state === "Pressed") {
      handler()
    }
  })

  currentShortcut = saved
}
Ejemplo de componente de UI
let isCapturing = false

const input = document.getElementById("shortcut-input") as HTMLInputElement
const button = document.getElementById("capture-btn") as HTMLButtonElement

button.addEventListener("click", () => {
  isCapturing = true
  input.value = "Press shortcut..."
  input.focus()
})

input.addEventListener("keydown", async (event) => {
  if (!isCapturing) return

  const shortcut = parseShortcutFromKeydown(event)

  if (!shortcut) {
    input.value = "Invalid shortcut"
    return
  }

  input.value = shortcut
  isCapturing = false

  await applyShortcut(shortcut, () => {
    console.log("Acción ejecutada")
  })
})
Problemas reales que debes anticipar
1. Conflictos con el sistema operativo o con otras apps

No asumas que porque el usuario eligió una combinación, esa combinación funcionará.
La propia documentación del plugin indica que si otro proceso ya tomó el shortcut, el handler no se disparará, e isRegistered() no te servirá para detectar que otra aplicación lo posee.

Implicación práctica

Debes diseñar la UX con una mentalidad defensiva:

sugerir combinaciones con modificadores
evitar shortcuts demasiado comunes
permitir reconfigurar fácilmente
mostrar feedback si el shortcut aparentemente no responde
2. Diferencias de teclado y layout

KeyboardEvent.key depende del layout y modificadores. Eso es bueno para capturar lo que el usuario realmente presiona, pero significa que no debes construir lógica basada en keyCode. MDN es explícito en que keyCode está obsoleto y que para teclas físicas o caracteres debes usar code o key según el caso. Para este caso de UX, normalmente key es la mejor opción porque refleja la tecla “visible” para el usuario.

3. Tecla Meta en macOS

metaKey representa la tecla Meta; en Mac corresponde a ⌘ Command. MDN también aclara que algunos sistemas operativos pueden interceptar ciertas teclas, por lo que no siempre todo será detectable en todos los contextos.

Consecuencia

No prometas al usuario que cualquier combinación será capturable o registrable.

Reglas UX que sí recomiendo
Mínimo aceptable
exigir al menos un modificador
no permitir solo modificadores
soportar limpiar shortcut
mostrar una versión legible, por ejemplo:
macOS: ⌘⇧K
Windows/Linux: Ctrl+Shift+K
Recomendado
separar:
valor interno: CommandOrControl+Shift+K
valor visual: ⌘⇧K o Ctrl+Shift+K
guardar siempre el valor interno normalizado
renderizar el valor visual según plataforma
Diseño robusto del dato

No guardes únicamente el texto visible.
Guarda algo así:

{
  "commandPaletteShortcut": {
    "accelerator": "CommandOrControl+Shift+K",
    "display": {
      "macos": "⌘⇧K",
      "default": "Ctrl+Shift+K"
    }
  }
}

Eso te da margen para:

cambiar render visual sin romper registro
migrar shortcuts en futuras versiones
validar mejor por plataforma
Mi recomendación técnica final

La forma correcta de implementarlo en Tauri no es “escuchar teclas y ya”, sino dividir el problema en cuatro capas:

captura UI con keydown
normalización a accelerator
registro/desregistro con @tauri-apps/plugin-global-shortcut
persistencia con @tauri-apps/plugin-store

Ese diseño es más limpio, más portable y más defendible que intentar registrar directamente desde la misma rutina que captura eventos del input. Además, encaja con la forma en que Tauri 2 estructura plugins, permissions y state persistence.

Nota de versión

Este documento está alineado con Tauri 2. Tauri 2 introdujo cambios relevantes respecto a Tauri 1, incluyendo la estructura por plugins/capabilities y el uso del plugin oficial de global shortcuts.
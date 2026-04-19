# Design Document: Settings UI — Editorial Redesign

## Overview

Rediseño visual del `SettingsPanel.tsx` adoptando el **Concepto A — "The Editorial Page"**: tipografía como estructura, rows inline, sin tarjetas ni contenedores. Inspirado en macOS System Settings nativo y Linear, pero con el lenguaje visual de Voxa (Ethereal Curator, Voxa Violet, glassmorphism).

**Principio central:** Cada setting es una fila de 40px. El espacio se gana eliminando contenedores, no reduciendo padding.

---

## 1. Layout Global

### Estructura de la ventana (800×600px)

```
┌─────────────────────────────────────────────────────────┐  48px
│  ◉ Voxa  ·  Voice Intelligence Layer          v1.2.0   │  Header
├──────────┬──────────────────────────────────────────────┤
│          │                                              │
│  200px   │           Content Area (flex-1)              │
│ Sidebar  │           px-10 py-8                         │
│          │                                              │
│          │                                              │
└──────────┴──────────────────────────────────────────────┘  32px Footer
```

### Cambios vs diseño actual

| Zona | Antes | Después |
|------|-------|---------|
| Header | `p-8 glass-panel` (64px+) | `px-8 h-12 border-b border-white/[0.04]` (48px) |
| Sidebar | `w-80 p-8` (320px) | `w-[200px] px-4 py-6` (200px) |
| Content padding | `p-12` | `px-10 py-8` |
| Footer | `p-8 bg-surface-container-low/60` (64px+) | `px-8 h-8 bg-transparent border-t border-white/[0.03]` (32px) |

**Espacio recuperado:** ~120px verticales + 120px horizontales → el content area crece de ~420×472px a ~560×488px.

---

## 2. Header

### Especificación

```jsx
<header className="h-12 flex items-center justify-between px-8 border-b border-white/[0.04] flex-shrink-0">
  <div className="flex items-center gap-3">
    {/* Logo squircle — reducido de 56px a 36px */}
    <div className="w-9 h-9 rounded-[0.6rem] bg-primary flex items-center justify-center shadow-md shadow-primary/20">
      <svg width="20" height="20">...</svg>
    </div>
    <div className="flex items-baseline gap-2">
      <span className="text-sm font-black text-on-surface font-headline leading-none">Voxa</span>
      <span className="text-[9px] font-black text-on-surface-variant/30 uppercase tracking-[0.2em]">
        Voice Intelligence Layer
      </span>
    </div>
  </div>
  <span className="text-[9px] font-mono text-on-surface-variant/20 tracking-widest">v{appVersion}</span>
</header>
```

**Tokens:** Logo 36×36px, `rounded-[0.6rem]`, todo en una línea horizontal.

---

## 3. Sidebar

### Especificación

```jsx
<aside className="w-[200px] flex flex-col py-6 border-r border-white/[0.04] flex-shrink-0">
  <nav className="flex-1 px-3 space-y-0.5">
    {tabs.map(tab => (
      <button
        key={tab.id}
        onClick={() => setActiveTab(tab.id)}
        className={`
          relative w-full flex items-center gap-3 px-3 py-2 rounded-lg
          transition-colors duration-150 text-xs font-semibold
          ${activeTab === tab.id
            ? 'text-primary'
            : 'text-on-surface-variant/50 hover:text-on-surface hover:bg-white/[0.03]'}
        `}
      >
        {/* Barra izquierda activa — reemplaza el punto circular */}
        {activeTab === tab.id && (
          <span className="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-4 bg-primary rounded-full" />
        )}
        <span className={`material-symbols-outlined text-[18px] ${
          activeTab === tab.id ? 'material-symbols-fill' : ''
        }`}>
          {tab.icon}
        </span>
        <span className="tracking-tight">{tab.label}</span>
      </button>
    ))}
  </nav>

  {/* Tip — compacto, sin caja */}
  <div className="px-4 pb-2">
    <p className="text-[9px] text-on-surface-variant/20 leading-relaxed">
      <span className="text-on-surface-variant/40 font-bold">Cmd+L</span> to switch language
    </p>
  </div>
</aside>
```

**Cambios clave:**
- Ancho: 320px → 200px
- Item activo: `bg-surface-container-high` (caja) → barra izquierda 2px + `text-primary`
- Padding por item: `px-6 py-4.5` → `px-3 py-2`
- Tip: caja con borde → texto plano

---

## 4. Sistema de Rows (el cambio más importante)

### Anatomía de una fila

```
┌─────────────────────────────────────────────────────────┐ 40px
│  Label text                              [  Control  ]  │
│  text-sm text-on-surface                 alineado right │
└─────────────────────────────────────────────────────────┘
```

### Clases base

```jsx
// Fila estándar
<div className="flex items-center justify-between py-2.5 min-h-[40px]">
  <span className="text-sm text-on-surface">{label}</span>
  <div className="flex-shrink-0">{control}</div>
</div>

// Group header (separador de sección)
<div className="pt-5 pb-2 mt-1 border-t border-white/[0.04] first:border-t-0 first:pt-0">
  <span className="text-[9px] font-black uppercase tracking-[0.35em] text-on-surface-variant/30">
    {groupName}
  </span>
</div>
```

### Controles inline

#### Select / Dropdown
```jsx
<select className="
  bg-surface-container-high/60 text-on-surface text-xs font-medium
  rounded-lg px-3 py-1.5 appearance-none cursor-pointer
  focus:outline-none focus:ring-1 focus:ring-primary/30
  hover:bg-surface-container-highest/60 transition-colors
  pr-7
">
```

#### Toggle Switch (40×22px)
```jsx
<button
  className={`relative w-10 h-[22px] rounded-full transition-colors flex-shrink-0 ${
    isActive ? 'bg-primary' : 'bg-surface-container-highest'
  }`}
>
  <span className={`absolute top-[3px] w-4 h-4 rounded-full bg-white shadow-sm transition-transform ${
    isActive ? 'translate-x-[22px]' : 'translate-x-[3px]'
  }`} />
</button>
```

#### Shortcut Key
```jsx
<div className="flex items-center gap-2">
  <button
    onClick={() => setCapturing(key)}
    className={`
      font-mono text-sm px-2.5 py-1 rounded-md transition-all
      ${isCapturing
        ? 'bg-primary/15 text-primary ring-1 ring-primary/30 animate-pulse'
        : 'bg-surface-container-high/50 text-on-surface hover:bg-surface-container-highest/60'}
    `}
  >
    {isCapturing ? '...' : displayValue}
  </button>
</div>
```

#### Language Selector (inline en la misma fila)
```jsx
<div className="flex gap-1.5">
  {['es', 'en'].map(lang => (
    <button
      key={lang}
      onClick={() => updateSetting('language', lang)}
      className={`
        px-3 py-1 rounded-full text-[11px] font-black uppercase tracking-wider
        transition-all
        ${settings.language === lang
          ? 'bg-primary text-background'
          : 'text-on-surface-variant/40 hover:text-on-surface hover:bg-white/[0.05]'}
      `}
    >
      {lang === 'es' ? 'ES' : 'EN'}
    </button>
  ))}
</div>
```

---

## 5. Sección General

### Layout completo

```
AUDIO
──────────────────────────────────────────────────────────
Microphone                          [MacBook Pro Mic  ▾]

SHORTCUTS                                    [Reset defaults]
──────────────────────────────────────────────────────────
Push to Talk                              [⌥ Space]
Hands-Free                                    [F5]
Paste                                     [⌘ ⇧ V]
Cancel                                       [Esc]

BEHAVIOR
──────────────────────────────────────────────────────────
Auto-detect Profile                              [●]
Language                                    [ES] [EN]
```

Cada fila: 40px. Total sección: ~280px vs los ~500px actuales.

---

## 6. Sección History — Feed tipo log

### Concepto

Reemplaza la grid de tarjetas por un **feed agrupado por fecha**, como un log de terminal o un chat.

### Layout

```
HOY  ·  3 transcripciones                    [Borrar todo]
──────────────────────────────────────────────────────────
10:42  Necesito revisar el contrato antes del viernes...  [↗][✎][×]
09:15  The quarterly report shows a 23% increase in...    [↗][✎][×]
08:03  Recordar llamar al cliente de Monterrey sobre...   [↗][✎][×]

AYER
──────────────────────────────────────────────────────────
18:30  Hola Voxa, esto es una prueba de dictado...        [↗][✎][×]
```

### Especificación JSX

```jsx
// Group header de fecha
<div className="flex items-center justify-between py-2 mt-4 first:mt-0">
  <span className="text-[9px] font-black uppercase tracking-[0.35em] text-on-surface-variant/30">
    {dateLabel}  ·  {count} transcripciones
  </span>
</div>

// Separador de grupo
<div className="border-t border-white/[0.04]" />

// Fila de transcripción
<div className="group flex items-center gap-3 py-2.5 border-b border-white/[0.03] last:border-0 hover:bg-white/[0.02] transition-colors rounded-sm -mx-2 px-2">
  {/* Timestamp */}
  <span className="font-mono text-[10px] text-on-surface-variant/25 flex-shrink-0 w-10">
    {time}
  </span>

  {/* Texto — expandible al click */}
  <p
    className={`flex-1 text-sm text-on-surface/80 font-medium min-w-0 cursor-pointer ${
      isExpanded ? 'whitespace-pre-wrap' : 'truncate'
    }`}
    onClick={() => toggleExpand(id)}
  >
    {content}
  </p>

  {/* Acciones — visibles en hover */}
  <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0">
    <CopyButton text={content} />
    <button onClick={() => startEdit(id)} className="p-1 rounded text-on-surface-variant/30 hover:text-primary transition-colors">
      <span className="material-symbols-outlined text-[14px]">edit</span>
    </button>
    <button onClick={() => deleteTranscript(id)} className="p-1 rounded text-on-surface-variant/30 hover:text-error transition-colors">
      <span className="material-symbols-outlined text-[14px]">close</span>
    </button>
  </div>
</div>
```

### Estado vacío

```jsx
<div className="py-16 flex flex-col items-center gap-3">
  <span className="material-symbols-outlined text-4xl text-on-surface-variant/[0.06]">history</span>
  <p className="text-[10px] font-black uppercase tracking-[0.3em] text-on-surface-variant/20">
    Sin transcripciones
  </p>
</div>
```

---

## 7. Sección Profiles — Lista con selector inline

### Layout

```
● PROFESSIONAL    "Refine and polish the text..."         [✎]
○ INFORMAL        "Keep it casual and direct..."          [✎]
○ CREATIVE        "Add flair and creativity..."           [✎]
○ CUSTOM          "Your custom instructions..."           [✎]

                                              [+ Nuevo perfil]
```

### Especificación JSX

```jsx
// Fila de perfil
<div
  onClick={() => updateSetting('active_profile_id', profile.id.toString())}
  className={`
    group flex items-center gap-3 py-2.5 border-b border-white/[0.03] last:border-0
    cursor-pointer hover:bg-white/[0.02] transition-colors rounded-sm -mx-2 px-2
  `}
>
  {/* Radio visual */}
  <div className={`w-3.5 h-3.5 rounded-full border flex-shrink-0 flex items-center justify-center transition-all ${
    isActive
      ? 'border-primary bg-primary'
      : 'border-on-surface-variant/20 bg-transparent'
  }`}>
    {isActive && <div className="w-1.5 h-1.5 rounded-full bg-background" />}
  </div>

  {/* Nombre */}
  <span className={`text-xs font-black uppercase tracking-widest flex-shrink-0 w-28 ${
    isActive ? 'text-primary' : 'text-on-surface-variant/50'
  }`}>
    {profile.name}
  </span>

  {/* Prompt preview */}
  <span className="flex-1 text-[11px] italic text-on-surface-variant/30 truncate min-w-0">
    {profile.system_prompt || 'Exact transcription'}
  </span>

  {/* Edit button */}
  <button
    onClick={(e) => { e.stopPropagation(); toggleEdit(profile.id); }}
    className="opacity-0 group-hover:opacity-100 p-1 rounded text-on-surface-variant/30 hover:text-on-surface transition-all flex-shrink-0"
  >
    <span className="material-symbols-outlined text-[14px]">edit</span>
  </button>
</div>

// Edit drawer — compacto, sin rounded-[3rem]
{editingProfileId === profile.id && (
  <div className="ml-6 mt-1 mb-3 p-5 rounded-xl bg-surface-container-high/60 backdrop-blur-xl space-y-4 animate-in slide-in-from-top-2 duration-200">
    {/* Nombre + Icono en una fila */}
    <div className="flex gap-3">
      <input className="flex-1 bg-background/40 rounded-lg px-3 py-2 text-xs text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30" />
      {/* Icon picker compacto */}
    </div>
    {/* Prompt */}
    <textarea rows={3} className="w-full bg-background/40 rounded-lg px-3 py-2 text-xs text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30 resize-none" />
    {/* Formatting mode */}
    <div className="flex gap-2">
      {['plain', 'markdown'].map(mode => (
        <button className={`flex-1 py-1.5 rounded-lg text-[10px] font-black uppercase tracking-wider ${
          isActive ? 'bg-primary text-background' : 'bg-background/40 text-on-surface-variant/50'
        }`}>{mode}</button>
      ))}
    </div>
    {/* Actions */}
    <div className="flex gap-2 pt-1">
      <button className="flex-1 bg-on-surface text-background py-2 rounded-lg text-[10px] font-black uppercase tracking-wider">Save</button>
      {!profile.is_default && <button className="px-4 bg-error/10 text-error py-2 rounded-lg text-[10px] font-black uppercase tracking-wider">Delete</button>}
      <button className="px-4 bg-white/[0.04] text-on-surface-variant/40 py-2 rounded-lg text-[10px] font-black uppercase tracking-wider">Cancel</button>
    </div>
  </div>
)}

// Botón nuevo perfil — inline, sin caja grande
<button className="mt-3 flex items-center gap-2 text-[11px] font-black uppercase tracking-wider text-on-surface-variant/30 hover:text-primary transition-colors py-2">
  <span className="material-symbols-outlined text-[16px]">add</span>
  New profile
</button>
```

---

## 8. Sección Dictionary — Tabla ultra-compacta

### Layout

```
WORD              REPLACEMENT                    USES
──────────────────────────────────────────────────────
Voxa              —                               12  [×]
API               —                                3  [×]
Tauri             —                                1  [×]

[+ Add word...                              ] [Add]
```

### Especificación

```jsx
// Header de tabla — minimalista
<div className="flex items-center gap-4 pb-2 border-b border-white/[0.04]">
  <span className="flex-1 text-[9px] font-black uppercase tracking-[0.3em] text-on-surface-variant/20">Word</span>
  <span className="flex-1 text-[9px] font-black uppercase tracking-[0.3em] text-on-surface-variant/20">Replacement</span>
  <span className="w-10 text-right text-[9px] font-black uppercase tracking-[0.3em] text-on-surface-variant/20">Uses</span>
  <span className="w-4" />
</div>

// Fila de entrada
<div className="group flex items-center gap-4 py-2 border-b border-white/[0.03] last:border-0 min-h-[36px]">
  <span className="flex-1 text-sm font-bold text-on-surface">{entry.word}</span>
  <input
    defaultValue={entry.replacement_word ?? ''}
    placeholder="—"
    className="flex-1 bg-transparent text-xs text-on-surface-variant/60 placeholder:text-on-surface-variant/20 focus:outline-none focus:bg-surface-container-high/40 rounded px-1 py-0.5 transition-colors"
  />
  <span className="w-10 text-right">
    {entry.usage_count > 0
      ? <span className="text-[10px] font-black text-primary/60">{entry.usage_count}</span>
      : <span className="text-on-surface-variant/15 text-[10px]">—</span>
    }
  </span>
  <button className="w-4 opacity-0 group-hover:opacity-100 text-on-surface-variant/20 hover:text-error transition-all">
    <span className="material-symbols-outlined text-[14px]">close</span>
  </button>
</div>

// Barra de añadir — sticky bottom
<div className="sticky bottom-0 pt-3 pb-1 bg-background/80 backdrop-blur-sm flex gap-2 mt-2">
  <input
    placeholder="Add word to dictionary..."
    className="flex-1 bg-surface-container-high/40 rounded-lg px-3 py-2 text-sm text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30 placeholder:text-on-surface-variant/20"
  />
  <button
    disabled={!newWord.trim()}
    className="px-5 bg-primary text-background rounded-lg text-[11px] font-black uppercase tracking-wider disabled:opacity-30 disabled:cursor-not-allowed hover:bg-primary/90 transition-all"
  >
    Add
  </button>
</div>
```

---

## 9. Sección Models — Lista simple

### Layout

```
MODELS PATH
──────────────────────────────────────────────────────────
/Users/user/Library/Application Support/voxa/models  [📋]

AI MODELS
──────────────────────────────────────────────────────────
Whisper Base          whisper-base.bin      142 MB  [●]
Llama 3.2 3B          llama-3.2-3b.gguf    2.1 GB  [●]

                                        [Re-download models]
```

### Especificación

```jsx
// Path row
<div className="flex items-center justify-between py-2.5 border-b border-white/[0.03]">
  <span className="font-mono text-[11px] text-on-surface-variant/40 truncate flex-1 mr-3">
    {modelsInfo.base_path}
  </span>
  <div className="flex gap-2 flex-shrink-0">
    <CopyButton text={modelsInfo.base_path} />
    <button onClick={() => invoke('open_models_folder')} className="p-1 rounded text-on-surface-variant/30 hover:text-primary transition-colors">
      <span className="material-symbols-outlined text-[14px]">folder_open</span>
    </button>
  </div>
</div>

// Model row
<div className="flex items-center gap-4 py-2.5 border-b border-white/[0.03] last:border-0 min-h-[40px]">
  <span className="flex-1 text-sm font-bold text-on-surface">{model.display_name}</span>
  <span className="font-mono text-[10px] text-on-surface-variant/25 hidden sm:block">{model.filename}</span>
  <span className="text-[11px] text-on-surface-variant/40 w-14 text-right">{model.size_mb} MB</span>
  <span className={`text-[9px] font-black uppercase tracking-wider px-2 py-0.5 rounded-full flex-shrink-0 ${
    model.downloaded
      ? 'bg-primary/10 text-primary/70'
      : 'bg-error/10 text-error/70'
  }`}>
    {model.downloaded ? 'Ready' : 'Missing'}
  </span>
</div>

// Progress bar (durante descarga) — 3px, sin contenedor
{isDownloadingModels && downloadProgress && (
  <div className="mt-3 space-y-1.5">
    <div className="flex justify-between text-[9px] font-black uppercase tracking-wider text-primary/60">
      <span>{downloadProgress.model}</span>
      <span>{downloadProgress.progress.toFixed(0)}%</span>
    </div>
    <div className="h-[3px] bg-surface-container-highest rounded-full overflow-hidden">
      <div
        className="h-full bg-primary rounded-full transition-all duration-300 shadow-[0_0_6px_rgba(157,122,255,0.5)]"
        style={{ width: `${downloadProgress.progress}%` }}
      />
    </div>
  </div>
)}

// Re-download button — al final, alineado a la derecha
<div className="flex justify-end pt-4">
  <button className="text-[10px] font-black uppercase tracking-wider text-on-surface-variant/30 hover:text-on-surface transition-colors py-1.5 px-3 rounded-lg hover:bg-white/[0.04]">
    Re-download models
  </button>
</div>
```

---

## 10. Confirm Modal

### Especificación

```jsx
<div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm animate-in fade-in duration-150">
  <div className="bg-surface-container-low/90 backdrop-blur-2xl rounded-2xl ring-1 ring-white/[0.06] p-6 max-w-xs w-full mx-6 space-y-4 animate-in zoom-in-95 duration-200 shadow-2xl">
    <div className="space-y-1.5">
      <h3 className="text-sm font-black text-on-surface uppercase tracking-widest">{title}</h3>
      <p className="text-xs text-on-surface-variant/60 leading-relaxed">{description}</p>
    </div>
    <div className="flex gap-2 justify-end pt-1">
      <button className="px-4 py-2 rounded-lg bg-white/[0.04] text-on-surface-variant/50 text-[10px] font-black uppercase tracking-wider hover:bg-white/[0.07] transition-colors">
        Cancel
      </button>
      <button className="px-4 py-2 rounded-lg bg-error/80 text-white text-[10px] font-black uppercase tracking-wider hover:bg-error/90 transition-colors">
        {confirmLabel}
      </button>
    </div>
  </div>
</div>
```

---

## 11. Footer

```jsx
<footer className="h-8 flex items-center justify-between px-8 border-t border-white/[0.03] flex-shrink-0">
  <span className="text-[9px] font-mono text-on-surface-variant/20 tracking-widest">
    Voxa Engine v{appVersion}
  </span>
</footer>
```

---

## 12. Comparativa de espacio

| Sección | Altura antes | Altura después | Ahorro |
|---------|-------------|----------------|--------|
| Header | ~80px | 48px | 32px |
| Sidebar item (×5) | ~56px cada uno | ~36px cada uno | 100px |
| Setting card General | ~120px | ~40px (row) | 80px por setting |
| History card | ~120px | ~44px (row) | 76px por entrada |
| Profile card | ~96px | ~40px (row) | 56px por perfil |
| Footer | ~64px | 32px | 32px |

**Resultado:** El mismo contenido ocupa ~40% menos espacio vertical. En una ventana de 600px, se pueden mostrar ~50% más items sin scroll.

---

## 13. Animaciones y transiciones

| Elemento | Animación | Duración |
|----------|-----------|----------|
| Tab change | `fade-in slide-in-from-right-1` | 200ms |
| Edit drawer open | `slide-in-from-top-2 fade-in` | 200ms |
| Hover actions | `opacity-0 → opacity-100` | 150ms |
| Toggle switch | `translate-x` | 150ms ease |
| Shortcut capture | `animate-pulse` en el key | continuo |
| Modal open | `zoom-in-95 fade-in` | 150ms |
| Progress bar | `transition-all` | 300ms |

---

## 14. Tokens de color usados

```
bg-background              — fondo base de la ventana
bg-surface-container-high/40-60  — controles inline (selects, inputs)
bg-surface-container-highest/60  — hover states
border-white/[0.03-0.06]   — separadores fantasma
text-on-surface            — texto principal
text-on-surface-variant/30-50  — texto secundario/labels
text-primary               — tab activo, toggle activo, acciones
bg-primary                 — toggle on, botón primario, badge activo
bg-error/10-80             — estados de error/destructivo
```

---

## 15. Notas de implementación

1. **No se cambia la lógica de estado** — solo el JSX de presentación
2. **Todos los `invoke()` y handlers permanecen igual**
3. **El componente `CopyButton` se mantiene sin cambios**
4. **Los `useEffect` y refs no se tocan**
5. La migración puede hacerse sección por sección sin romper funcionalidad

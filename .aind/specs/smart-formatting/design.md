# Design: smart-formatting

## Enfoque

No hay motor de reglas nuevo. La feature entera vive en la capa de prompt: se añaden instrucciones de formato al `system_prompt` existente antes de pasarlo al LLM. El aprendizaje de correcciones extiende ese prompt con hints persistidos en DB.

---

## 1. Cambios de base de datos

### 1.1 Columna `formatting_mode` en `transformation_profiles`

```sql
ALTER TABLE transformation_profiles ADD COLUMN formatting_mode TEXT NOT NULL DEFAULT 'plain';
```

Migración como nueva versión en el array `MIGRATIONS` de `db.rs`.

Valores válidos: `"markdown"` | `"plain"` | `"rich"` (rich = futuro, equivale a `plain` por ahora).

Defaults por perfil:
| Perfil | formatting_mode |
|--------|-----------------|
| Elegant | plain |
| Informal | plain |
| Code | markdown |

### 1.2 Tabla `formatting_hints`

```sql
CREATE TABLE IF NOT EXISTS formatting_hints (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id INTEGER NOT NULL,
    pattern TEXT NOT NULL,       -- descripción del patrón detectado
    hint TEXT NOT NULL,          -- instrucción a inyectar en system_prompt
    frequency INTEGER NOT NULL DEFAULT 1,
    is_promoted INTEGER NOT NULL DEFAULT 0,  -- 1 = promovido a regla permanente
    FOREIGN KEY (profile_id) REFERENCES transformation_profiles(id)
);
```

### 1.3 Struct `Profile` actualizado

```rust
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub system_prompt: String,
    pub icon: Option<String>,
    pub is_default: bool,
    pub formatting_mode: String,  // nuevo
}
```

---

## 2. Construcción del system_prompt compuesto

### 2.1 Función `build_formatting_block(mode: &str, language: &str, hints: &[FormattingHint]) -> String`

Ubicación: nuevo módulo `src-tauri/src/formatting.rs`.

Produce un bloque de texto que se **append** al `system_prompt` base del perfil antes de pasarlo al LLM.

**Estructura del bloque:**
```
FORMATTING RULES (apply after all other instructions):
[reglas estructurales según mode]
[tabla de símbolos — siempre incluida]
[cues estructurales en el idioma configurado]
[hints del usuario si existen]
Return ONLY the final text. No explanations.
```

**Para `plain`:**
- Listas numeradas: `1)`, `2)`, `3)`
- Bullets: `-` (sin markdown)
- Sin negritas/cursivas
- Sí sustitución de símbolos

**Para `markdown`:**
- Listas numeradas: `1.`, `2.`, `3.`
- Bullets: `-`
- Negritas: `**text**`, cursivas: `*text*`
- Sí sustitución de símbolos

### 2.2 Integración en `resolve_system_prompt()`

`pipeline.rs` — `resolve_system_prompt()` pasa de retornar `(String, String)` a retornar `(String, String, String)` → `(composed_prompt, profile_name, formatting_mode)`.

```rust
fn resolve_system_prompt(app, db_state) -> (String, String, String) {
    // ... lógica existente para obtener profile ...
    let hints = db::get_formatting_hints(&conn, profile.id).unwrap_or_default();
    let formatting_block = formatting::build_formatting_block(
        &profile.formatting_mode, 
        &language, 
        &hints
    );
    let composed = format!("{}\n\n{}", profile.system_prompt, formatting_block);
    (composed, profile.name, profile.formatting_mode)
}
```

El `composed` es el string que ya existía como `system_prompt` — sin cambios en la firma de `run_llm_refinement()`.

### 2.3 Token budget (Req 7.2)

El bloque de formatting NO supera 200 tokens en el caso base (sin hints). Con hasta 5 hints promovidos, máximo ~350 tokens. Límite de 512 tokens totales para el bloque de formatting — enforced en `build_formatting_block()` con truncado de hints si es necesario.

---

## 3. Aprendizaje de correcciones (Req 6)

### 3.1 Viabilidad: captura automática

Voxa ya tiene permisos de Accessibility (event_tap). Sin embargo, monitorear el clipboard post-paste para detectar ediciones del usuario es frágil: el clipboard puede cambiar por otras apps, y no hay garantía de capturar la edición dentro de 30 segundos.

**Decisión**: implementar primero el **fallback manual** (Req 6.4). Si en el futuro se confirma viabilidad de Accessibility para post-edit monitoring, se añade como capa opcional.

### 3.2 Flujo fallback (botón Correction en RecorderPill)

1. Post-inserción, la RecorderPill muestra botón "✏ Corrección" durante 30 segundos.
2. Usuario hace clic → modal/input donde pega el texto corregido.
3. Frontend envía Tauri command `submit_correction { profile_id, original_text, corrected_text }`.
4. Backend (`commands.rs`) calcula diff y llama al LLM para derivar un `hint`:
   ```
   system: "You are a formatting assistant. Given an original text and its corrected version, extract ONE formatting rule that was applied. Return a single instruction in imperative form, max 15 words. Return only the instruction."
   user: "Original: {original}\nCorrected: {corrected}"
   ```
5. El hint se persiste en `formatting_hints` con `frequency = 1`.
6. Si el mismo `pattern` ya existe → `frequency += 1`. Si `frequency >= 5` → `is_promoted = 1`.

### 3.3 Inyección de hints en el prompt

`build_formatting_block()` incluye al final los hints del perfil activo:
```
USER PREFERENCES (apply always):
- [hint 1]
- [hint 2]
```
Solo se inyectan hints con `is_promoted = 1` o los 3 más recientes no promovidos (evita prompt bloat).

---

## 4. Frontend

### 4.1 SettingsPanel — selector de `formatting_mode`

Por cada perfil, añadir un `<select>` con opciones `plain` / `markdown` bajo el campo de `system_prompt`.

Tauri command: `update_profile_formatting_mode(profile_id, mode)` → `UPDATE transformation_profiles SET formatting_mode = ?1 WHERE id = ?2`.

### 4.2 RecorderPill — botón Correction

Post-inserción exitosa: mostrar botón "✏" durante 30s (timeout con fade-out). Al click: textarea modal, botón "Enviar corrección". Lógica en `RecorderPill.tsx`, command en `commands.rs`.

---

## 5. Orden de implementación (micro-tasks)

1. **DB**: migración `formatting_mode` + tabla `formatting_hints` + update `Profile` struct + funciones CRUD.
2. **formatting.rs**: `build_formatting_block()` con modo plain/markdown, tabla de símbolos, cues ES/EN.
3. **pipeline.rs**: integrar `build_formatting_block()` en `resolve_system_prompt()`.
4. **commands.rs**: command `submit_correction` + lógica de hint derivation + CRUD hints.
5. **SettingsPanel.tsx**: selector `formatting_mode` por perfil.
6. **RecorderPill.tsx**: botón Correction + modal.
7. **Tests**: unit tests para `build_formatting_block()` (plain vs markdown, con/sin hints, token limit).

---

## 6. No-regresión

- `build_formatting_block()` con texto sin Structural_Cues → LLM recibe las mismas instrucciones de base que antes + el bloque de formato. El bloque instruye explícitamente al LLM a NO añadir formato si no hay cues → output idéntico al actual.
- Si `formatting_mode` es `plain` y no hay cues → output plano, sin diferencia con hoy.
- Fallback existente (raw_text si LLM falla) no cambia.

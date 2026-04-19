# Requirements: smart-formatting

## Introduction

Este documento define los requisitos para **Smart Structural Formatting** en Voxa — la capacidad del sistema de detectar patrones estructurales en el dictado (listas, enumeraciones, símbolos, puntuación) y formatear el texto de salida correctamente, sin que el usuario tenga que reformatear manualmente.

### Estado actual del pipeline

El pipeline de Voxa sigue este flujo:

1. **Whisper** transcribe el audio → `raw_text` (transcripción plana, sin estructura)
2. **Diccionario** (`custom_dictionary`) se pasa a Whisper como `initial_prompt` para mejorar vocabulario
3. **Reemplazos** (`replacement_entries`) hacen sustituciones palabra-por-palabra sobre `raw_text` antes del LLM
4. **LLM (llama)** refina el texto usando el `system_prompt` del perfil activo

El `system_prompt` actual de los perfiles se enfoca en tono (formal, informal, código) pero **no instruye al LLM para detectar estructuras** como listas numeradas, bullets, ni para manejar cues de símbolos/puntuación dictados verbalmente.

### Qué resuelve esta feature

No es un motor de reglas separado — es una mejora al LLM prompt que ya existe, más un mecanismo de aprendizaje basado en correcciones del usuario. El LLM ya procesa cada transcripción; solo necesitamos instruirlo correctamente.

---

## Glossary

- **Structural_Cue**: Palabra o frase en el dictado que indica intención de estructura, no contenido (ej: "punto uno", "primero", "entre comillas", "tenemos tres puntos").
- **Formatting_Rule**: Instrucción en el system_prompt que define cómo el LLM debe transformar un Structural_Cue en formato de salida.
- **Profile_Formatting_Mode**: El modo de formato asociado a un perfil — puede ser `markdown`, `plain`, o `rich` (según la app de destino).
- **Symbol_Cue**: Palabra dictada que representa un símbolo (ej: "arroba" → `@`, "guión" → `-`, "copyright" → `©`).
- **Correction_Signal**: Una edición manual del usuario sobre el texto ya insertado que revela una preferencia de formato no satisfecha.
- **Formatting_Hint**: Instrucción corta derivada de Correction_Signals que se agrega al system_prompt del perfil activo para mejorar futuras transcripciones.

---

## Requirements

### Requirement 1: Detección de listas numeradas

**User Story:** Como usuario de Voxa, cuando dicto "punto uno X, punto dos Y, punto tres Z" o "1 X 2 Y 3 Z", quiero que el output sea una lista numerada correctamente formateada, para no tener que reformatear el texto después.

#### Acceptance Criteria

1. WHEN el `raw_text` contiene cues numerados verbales en español ("punto uno", "primero", "uno -", "paso uno") o en inglés ("point one", "first", "step one"), THE LLM SHALL formatear los ítems como una lista numerada.

2. WHEN el `raw_text` contiene números seguidos de contenido ("1 haz esto 2 haz aquello"), THE LLM SHALL reconocer el patrón como lista numerada y formatear cada número como item.

3. WHEN el perfil activo tiene `formatting_mode = markdown`, THE LLM SHALL usar `1.`, `2.`, `3.` como prefijos de lista.

4. WHEN el perfil activo tiene `formatting_mode = plain`, THE LLM SHALL usar `1)`, `2)`, `3)` o formato equivalente sin markdown.

5. WHEN el `raw_text` contiene solo un ítem numerado (sin secuencia), THE LLM SHALL NOT crear una lista — procesará el texto como prosa normal.

---

### Requirement 2: Detección de listas con bullets

**User Story:** Como usuario de Voxa, cuando dicto "tenemos tres puntos: A, B y C" o "los puntos son: primero X, segundo Y", quiero que el output use bullets/viñetas para cada ítem, para que la estructura sea visualmente clara.

#### Acceptance Criteria

1. WHEN el `raw_text` contiene cues de enumeración ("tenemos N puntos", "los puntos son", "hay tres opciones", "we have N items") seguidos de ítems, THE LLM SHALL formatear los ítems como una lista con bullets.

2. WHEN el `raw_text` usa letras como separadores de ítems ("a) X, b) Y, c) Z" o "a X b Y c Z"), THE LLM SHALL reconocer el patrón como lista con bullets.

3. WHEN el perfil activo tiene `formatting_mode = markdown`, THE LLM SHALL usar `•` o `-` como prefijo de bullet.

4. WHEN el perfil activo tiene `formatting_mode = plain`, THE LLM SHALL usar `-` o espacio con sangría como bullet.

5. WHEN el `raw_text` mezcla numeración y bullets en el mismo dictado, THE LLM SHALL preservar la jerarquía: la secuencia principal es numerada, las sub-listas son bullets.

---

### Requirement 3: Sustitución de símbolos dictados

**User Story:** Como usuario de Voxa, cuando dicto el nombre de un símbolo ("arroba", "guión", "copyright"), quiero que el output contenga el símbolo correcto, para no tener que escribirlo manualmente.

#### Acceptance Criteria

1. WHEN el `raw_text` contiene nombres de símbolos comunes, THE LLM SHALL sustituirlos por el símbolo correspondiente según la tabla:

   | Dictado (ES) | Dictado (EN) | Símbolo |
   |---|---|---|
   | arroba | at sign | `@` |
   | guión, guion | dash, hyphen | `-` |
   | guión bajo | underscore | `_` |
   | punto | dot, period | `.` |
   | dos puntos | colon | `:` |
   | punto y coma | semicolon | `;` |
   | copyright | copyright | `©` |
   | trademark | trademark | `™` |
   | registered | registered | `®` |
   | más | plus | `+` |
   | por, multiplicado por | times | `×` |
   | igual | equals | `=` |
   | mayor que | greater than | `>` |
   | menor que | less than | `<` |
   | ampersand, et | ampersand | `&` |
   | barra | slash | `/` |
   | barra invertida | backslash | `\` |
   | almohadilla, hashtag | hash | `#` |
   | asterisco | asterisk | `*` |
   | porcentaje | percent | `%` |
   | dólar | dollar | `$` |
   | euro | euro | `€` |
   | tilde | tilde | `~` |

2. WHEN la sustitución de un símbolo cambia el significado del texto (ej: "punto" en mitad de una oración puede ser puntuación o el símbolo `.`), THE LLM SHALL usar contexto semántico para decidir si sustituir o no.

3. WHEN el `raw_text` contiene el nombre de un símbolo que no está en la tabla, THE LLM SHALL preservar el texto original sin modificar.

---

### Requirement 4: Cues de puntuación y formato inline

**User Story:** Como usuario de Voxa, cuando dicto "entre comillas X" o "en negritas Y", quiero que el output aplique el formato correcto, para dictar texto con formato sin pausas.

#### Acceptance Criteria

1. WHEN el `raw_text` contiene cues de delimitación ("entre comillas X", "X entre comillas"), THE LLM SHALL rodear el contenido con comillas: `"X"`.

2. WHEN el perfil activo tiene `formatting_mode = markdown` y el `raw_text` contiene cues de énfasis ("en negrita X", "en cursiva X", "en bold X", "en italic X"), THE LLM SHALL aplicar formato markdown: `**X**`, `*X*`.

3. WHEN el perfil activo tiene `formatting_mode = plain`, THE LLM SHALL ignorar los cues de énfasis markdown y preservar el texto sin marcado.

4. WHEN el `raw_text` contiene el cue "nueva línea" o "salto de línea", THE LLM SHALL insertar un salto de línea en ese punto.

5. WHEN el `raw_text` contiene el cue "nuevo párrafo", THE LLM SHALL insertar un doble salto de línea en ese punto.

---

### Requirement 5: Formatting mode por perfil

**User Story:** Como usuario de Voxa, quiero que el comportamiento de formato sea diferente según el perfil activo, para que el output sea adecuado para la app de destino (ej: markdown en VS Code, texto plano en Slack).

#### Acceptance Criteria

1. EACH Profile SHALL tener un atributo `formatting_mode` con valores posibles: `markdown`, `plain`, `rich`.

2. WHEN el perfil activo es "Code", THE default `formatting_mode` SHALL ser `markdown`.

3. WHEN el perfil activo es "Elegant", THE default `formatting_mode` SHALL ser `plain`.

4. WHEN el perfil activo es "Informal", THE default `formatting_mode` SHALL ser `plain`.

5. WHEN el usuario crea un perfil personalizado, THE default `formatting_mode` SHALL ser `plain` a menos que el usuario lo configure explícitamente.

6. THE Settings_Panel SHALL permitir al usuario cambiar el `formatting_mode` de cada perfil.

7. WHEN el `formatting_mode` cambia, THE system_prompt SHALL actualizarse automáticamente para reflejar las nuevas instrucciones de formato.

---

### Requirement 6: Aprendizaje de correcciones del usuario

**User Story:** Como usuario de Voxa, cuando corrijo manualmente el texto insertado, quiero que el sistema aprenda ese patrón y lo aplique automáticamente en futuros dictados similares, para que la app mejore con el uso.

#### Acceptance Criteria

1. WHEN el usuario edita el texto insertado por Voxa en los 30 segundos posteriores a la inserción, THE system SHALL capturar la diferencia entre el texto original y el texto editado como un Correction_Signal.

   > **Nota**: Capturar ediciones post-inserción requiere monitorear el clipboard o teclas después del paste. La viabilidad depende de los permisos de Accessibility ya otorgados a Voxa. Si no es viable, este requisito se implementa como feedback manual (ver criterio 4).

2. WHEN se captura un Correction_Signal que muestra un patrón estructural (ej: el usuario convirtió prosa a lista), THE system SHALL generar un Formatting_Hint y persistirlo en la base de datos asociado al perfil activo.

3. WHEN existen Formatting_Hints para el perfil activo, THE system SHALL inyectarlos en el system_prompt como instrucciones adicionales en las siguientes grabaciones.

4. IF la captura automática de correcciones no es viable por permisos, THEN THE UI SHALL ofrecer un botón de "Corrección" en la RecorderPill post-inserción que permita al usuario pegar el texto corregido manualmente para generar el Correction_Signal.

5. WHEN un Formatting_Hint se genera 5 o más veces con el mismo patrón, THE system SHALL promoverlo a Formatting_Rule permanente en el system_prompt del perfil.

6. WHEN el usuario edita el system_prompt de un perfil en Settings, THE system SHALL preservar las Formatting_Rules aprendidas a menos que el usuario las elimine explícitamente.

---

### Requirement 7: No regresión en el pipeline existente

**User Story:** Como usuario de Voxa, quiero que la mejora de formato no rompa el comportamiento actual del pipeline ni aumente la latencia de forma perceptible.

#### Acceptance Criteria

1. WHEN no se detectan Structural_Cues en el `raw_text`, THE LLM SHALL producir el mismo output que antes de esta feature.

2. WHEN se añaden Formatting_Rules al system_prompt, THE total de tokens del prompt del sistema NO SHALL exceder 512 tokens para mantener la latencia de inferencia dentro de los límites actuales.

3. WHEN el LLM falla en la inferencia por cualquier razón, THE pipeline SHALL usar el `raw_text` como fallback exactamente igual que hoy.

4. WHEN el `formatting_mode` es `plain` y no hay Structural_Cues, THE output SHALL ser texto plano sin ningún marcado markdown.

---

### Requirement 8: Idioma-awareness del formatting

**User Story:** Como usuario de Voxa, quiero que la detección de Structural_Cues funcione en el idioma que estoy dictando, para que no tenga que cambiar cómo dicto según el idioma.

#### Acceptance Criteria

1. WHEN el idioma configurado es `es`, THE LLM SHALL reconocer Structural_Cues en español ("punto uno", "primero", "tenemos N puntos", "entre comillas").

2. WHEN el idioma configurado es `en`, THE LLM SHALL reconocer Structural_Cues en inglés ("point one", "first", "we have N points", "in quotes").

3. WHEN el `raw_text` mezcla español e inglés (code-switching), THE LLM SHALL detectar Structural_Cues en ambos idiomas simultáneamente.

4. THE Formatting_Rules en el system_prompt SHALL incluir ejemplos en el idioma configurado para reducir ambigüedad al LLM.

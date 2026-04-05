# Documento de Requerimientos de Producto
## Aplicación de Dictado por Voz para macOS — Versión Inicial

**Estado:** Borrador para revisión de Producto y Desarrollo  
**Plataforma inicial:** macOS  
**Expansión futura prevista:** Windows  
**Objetivo del documento:** definir el propósito del proyecto y los requerimientos funcionales y no funcionales de la aplicación, sin imponer decisiones técnicas de implementación.

---

## 1. Propósito del proyecto

Desarrollar una aplicación de dictado por voz para macOS que permita al usuario convertir su voz en texto de manera rápida, cómoda y controlada, dentro de su flujo normal de trabajo.

La aplicación debe permitir dos modos principales de uso: **Push to Talk** y **Hands-Free**, y ofrecer al usuario control sobre:

- el micrófono que desea usar,
- el idioma principal de dictado,
- el tipo de salida que desea recibir (transcripción cruda o transformada),
- perfiles personalizados de transformación,
- un diccionario de palabras propio,
- y el acceso a un historial local de transcripciones.

El producto debe estar orientado a productividad real, simplicidad de uso y personalización progresiva, con una primera fase enfocada exclusivamente en macOS.

---

## 2. Objetivos del producto

### 2.1 Objetivo principal
Permitir que el usuario dicte texto con fricción mínima y con una salida suficientemente útil como para integrarse en tareas reales de escritura.

### 2.2 Objetivos específicos
- Reducir el esfuerzo de escritura manual en macOS.
- Permitir dictado configurable según preferencia de interacción.
- Dar al usuario control explícito sobre si desea usar una transcripción cruda o una versión transformada.
- Permitir personalización progresiva de la aplicación mediante diccionario y perfiles de transformación.
- Mantener un historial local de transcripciones reutilizable.
- Preparar la base funcional para una futura expansión a Windows.

---

## 3. Alcance inicial

La versión inicial del producto incluye únicamente:

- Soporte para **macOS**
- Soporte para **2 idiomas iniciales**:
  - Español
  - Inglés
- Selección de micrófono desde los dispositivos detectados por el sistema
- Modos de interacción:
  - Push to Talk
  - Hands-Free
- Configuración de shortcuts o combinaciones de teclas por captura directa del input del usuario
- Transcripción cruda
- Transcripción transformada
- Perfiles de transformación configurables
- Diccionario de palabras personalizado
- Historial local de transcripciones

---

## 4. Fuera de alcance para esta fase

No forma parte obligatoria de esta primera fase:

- soporte para Windows,
- sincronización entre dispositivos,
- colaboración multiusuario,
- cuenta de usuario obligatoria,
- funcionalidades móviles,
- analítica avanzada de uso,
- automatizaciones externas,
- comandos complejos fuera del flujo de dictado y transformación.

---

## 5. Requerimientos funcionales

## 5.1 Plataforma

### RF-001 — Soporte inicial de plataforma
La aplicación debe funcionar inicialmente en **macOS**.

### RF-002 — Preparación para expansión futura
El producto debe definirse funcionalmente de forma que pueda evolucionar posteriormente a Windows sin redefinir el comportamiento principal del usuario.

---

## 5.2 Historial local de transcripciones

### RF-003 — Registro de historial
La aplicación debe guardar localmente el historial de transcripciones generadas por el usuario.

### RF-004 — Consulta del historial
El usuario debe poder visualizar su historial de transcripciones dentro de la aplicación.

### RF-005 — Copia al portapapeles
El usuario debe poder copiar cualquier transcripción almacenada en el historial al portapapeles.

### RF-006 — Eliminación individual
El usuario debe poder eliminar individualmente cualquier transcripción del historial.

### RF-007 — Persistencia local
El historial debe mantenerse disponible localmente entre sesiones, salvo que el usuario lo elimine.

---

## 5.3 Mejora progresiva de la calidad de transcripción

### RF-008 — Aprendizaje progresivo del sistema
La aplicación debe incorporar un mecanismo funcional de mejora progresiva de la calidad de transcripción con el tiempo.

### RF-009 — Personalización basada en uso
La mejora progresiva debe considerar, como mínimo, la personalización derivada de:
- palabras agregadas al diccionario,
- preferencias configuradas por el usuario,
- patrones explícitos de uso aceptados por el usuario.

### RF-010 — Evolución sin pérdida de control
La mejora progresiva no debe eliminar la capacidad del usuario de revisar, corregir o redefinir sus preferencias.

### RF-011 — Transparencia funcional
El producto debe hacer explícito al usuario que la aplicación puede mejorar su salida con el tiempo a partir de la personalización configurada y del uso acumulado.

---

## 5.4 Selección entre transcripción cruda y transformada

### RF-012 — Doble modalidad de salida
La aplicación debe permitir dos tipos de salida de texto:
- **Transcripción cruda**
- **Transcripción transformada**

### RF-013 — Selección explícita de salida
El usuario debe poder seleccionar qué tipo de salida desea usar.

### RF-014 — Persistencia de preferencia
La aplicación debe recordar la preferencia de salida del usuario entre sesiones, salvo modificación manual.

### RF-015 — Diferenciación visible
La interfaz debe diferenciar claramente cuándo el usuario está usando transcripción cruda y cuándo transcripción transformada.

---

## 5.5 Perfiles de transformación

### RF-016 — Perfiles configurables
La aplicación debe permitir configurar perfiles de transformación para modificar el resultado textual a partir de la voz del usuario.

### RF-017 — Base conceptual del perfil
Cada perfil de transformación debe responder a una instrucción o conjunto de instrucciones definidas por el usuario para ajustar la salida transformada.

### RF-018 — Creación de perfiles
El usuario debe poder crear nuevos perfiles de transformación.

### RF-019 — Edición de perfiles
El usuario debe poder editar perfiles de transformación existentes.

### RF-020 — Eliminación de perfiles
El usuario debe poder eliminar perfiles de transformación creados por él.

### RF-021 — Selección de perfil activo
El usuario debe poder seleccionar cuál perfil desea usar para la transcripción transformada.

### RF-022 — Persistencia de perfiles
Los perfiles creados o modificados por el usuario deben conservarse entre sesiones.

### RF-023 — Personalización abierta
La aplicación no debe limitarse a perfiles predefinidos; debe permitir la incorporación de perfiles personalizados adicionales.

---

## 5.6 Diccionario de palabras personalizado

### RF-024 — Diccionario persistente
La aplicación debe contar con un diccionario de palabras personalizado persistente.

### RF-025 — Agregar palabras
El usuario debe poder agregar palabras, términos, nombres o expresiones al diccionario.

### RF-026 — Editar palabras
El usuario debe poder modificar entradas existentes del diccionario.

### RF-027 — Eliminar palabras
El usuario debe poder eliminar entradas del diccionario.

### RF-028 — Uso del diccionario en transcripción
La aplicación debe utilizar el diccionario del usuario para mejorar la calidad y consistencia de la transcripción.

### RF-029 — Orientación a vocabulario específico
El diccionario debe servir especialmente para nombres propios, términos técnicos, marcas, siglas y expresiones de uso frecuente.

---

## 5.7 Selección del micrófono

### RF-030 — Detección de inputs de audio
La aplicación debe detectar los dispositivos de entrada de audio disponibles en el sistema.

### RF-031 — Selección manual de micrófono
El usuario debe poder seleccionar qué micrófono desea usar dentro de la aplicación.

### RF-032 — Visualización clara del dispositivo activo
La aplicación debe mostrar claramente qué micrófono se encuentra seleccionado.

### RF-033 — Persistencia de preferencia de micrófono
La aplicación debe recordar el micrófono preferido del usuario cuando esté disponible.

### RF-034 — Manejo de indisponibilidad
Si el micrófono previamente seleccionado deja de estar disponible, la aplicación debe informar al usuario y permitir una nueva selección.

---

## 5.8 Idiomas soportados

### RF-035 — Idiomas iniciales soportados
La aplicación debe soportar inicialmente los siguientes idiomas:
- Español
- Inglés

### RF-036 — Selección del idioma preferido
El usuario debe poder seleccionar explícitamente el idioma preferido para hablar.

### RF-037 — Coherencia entre habla y salida
Cuando el usuario seleccione un idioma preferido, la aplicación debe generar la transcripción en ese mismo idioma esperado.

### RF-038 — Persistencia del idioma
La preferencia de idioma del usuario debe mantenerse entre sesiones.

---

## 5.9 Modos de interacción

### RF-039 — Soporte para Push to Talk
La aplicación debe ofrecer un modo **Push to Talk**.

### RF-040 — Soporte para Hands-Free
La aplicación debe ofrecer un modo **Hands-Free**.

### RF-041 — Selección de modo
El usuario debe poder elegir cuál modo de interacción desea utilizar.

### RF-042 — Persistencia del modo preferido
La aplicación debe recordar el modo de interacción seleccionado por el usuario.

---

## 5.10 Configuración de shortcuts o combinaciones de teclas

### RF-043 — Configuración de shortcut
La aplicación debe permitir configurar la tecla o combinación de teclas utilizada para activar la interacción de dictado.

### RF-044 — Captura automática de input
La configuración del shortcut no debe depender de ingreso manual de texto; el sistema debe detectar automáticamente la tecla o combinación presionada por el usuario durante el proceso de configuración.

### RF-045 — Shortcut configurable por modo
La aplicación debe permitir definir el atajo correspondiente al menos para la activación principal del dictado.

### RF-046 — Validación básica del shortcut
La aplicación debe validar que el shortcut configurado sea utilizable por la app y comunicar al usuario cuando exista un problema funcional con la combinación seleccionada.

### RF-047 — Reconfiguración
El usuario debe poder cambiar su shortcut en cualquier momento.

---

## 5.11 Administración de preferencias

### RF-048 — Persistencia de configuración
La aplicación debe guardar localmente las preferencias de configuración del usuario.

### RF-049 — Recuperación de preferencias
Al iniciar la aplicación, esta debe recuperar las preferencias previamente configuradas por el usuario.

### RF-050 — Consistencia de configuración
Los cambios realizados por el usuario en idioma, micrófono, modo, shortcut, tipo de salida y perfil activo deben reflejarse de forma consistente en el comportamiento de la aplicación.

---

## 6. Requerimientos no funcionales

## 6.1 Usabilidad

### RNF-001 — Facilidad de configuración
La configuración inicial de la aplicación debe ser comprensible y ejecutable por un usuario no técnico.

### RNF-002 — Claridad de estados
La aplicación debe comunicar claramente al usuario los estados principales del flujo de uso, como mínimo:
- lista para dictar,
- escuchando,
- procesando,
- transcripción disponible,
- error o acción requerida.

### RNF-003 — Bajo esfuerzo operativo
Las acciones más frecuentes del usuario deben requerir la menor cantidad posible de pasos.

---

## 6.2 Persistencia y manejo local de información

### RNF-004 — Almacenamiento local
Las preferencias del usuario, el historial y el diccionario deben almacenarse localmente.

### RNF-005 — Consistencia de datos
La información guardada localmente debe conservarse de forma consistente entre sesiones.

### RNF-006 — Eliminación controlada
Cuando el usuario elimine una transcripción o una entrada del diccionario, esa acción debe reflejarse de forma clara y consistente en la aplicación.

---

## 6.3 Rendimiento percibido

### RNF-007 — Respuesta adecuada al flujo de uso
La aplicación debe responder de forma suficientemente fluida para no romper la experiencia de dictado del usuario.

### RNF-008 — Cambio de configuración sin fricción
Cambios como idioma, micrófono, modo o perfil activo deben aplicarse de manera clara y sin comportamiento ambiguo.

---

## 6.4 Confiabilidad

### RNF-009 — Estabilidad funcional
La aplicación debe comportarse de forma estable durante sesiones repetidas de dictado.

### RNF-010 — Recuperación ante fallos simples
Si ocurre un fallo operativo menor, la aplicación debe informar al usuario y permitir retomar el flujo sin requerir reconfiguración completa.

### RNF-011 — No pérdida silenciosa de información
La aplicación no debe perder silenciosamente el resultado de una transcripción ya generada sin informar al usuario.

---

## 6.5 Control del usuario

### RNF-012 — Preferencias explícitas
La aplicación debe privilegiar configuraciones explícitas del usuario sobre comportamientos automáticos ambiguos.

### RNF-013 — Personalización mantenible
Las opciones de personalización no deben quedar ocultas ni depender de flujos difíciles de descubrir.

### RNF-014 — Transparencia de la transformación
Debe quedar claro para el usuario cuándo el texto mostrado es una transcripción directa y cuándo es una versión transformada.

---

## 6.6 Escalabilidad funcional del producto

### RNF-015 — Preparación para evolución
La definición funcional del producto debe permitir crecer a futuro en:
- más idiomas,
- más perfiles,
- más plataformas,
- y opciones adicionales de personalización,
sin requerir rediseñar el comportamiento base del usuario.

---

## 7. Criterios de aceptación de alto nivel

La primera fase del producto se considerará funcionalmente satisfactoria si el usuario puede:

1. Instalar y usar la app en macOS.
2. Seleccionar su micrófono.
3. Seleccionar su idioma preferido entre español e inglés.
4. Elegir entre Push to Talk y Hands-Free.
5. Configurar la tecla o combinación de teclas mediante captura directa.
6. Obtener una transcripción.
7. Elegir entre salida cruda o transformada.
8. Crear y usar perfiles de transformación.
9. Mantener un diccionario personalizado.
10. Consultar, copiar y eliminar elementos del historial local.

---

## 8. Riesgos funcionales a vigilar

- Confusión del usuario entre “transcripción cruda” y “transcripción transformada”.
- Exceso de complejidad en la configuración de perfiles personalizados.
- Falta de claridad en el proceso de configuración de shortcuts.
- Ambigüedad en la mejora progresiva del sistema si no se comunica correctamente.
- Fricción si el usuario no entiende cómo influye el diccionario en la calidad del resultado.

---

## 9. Resumen ejecutivo

Esta primera versión del producto debe enfocarse en un problema concreto y bien delimitado:  
**permitir dictado por voz configurable y personalizable en macOS, con historial local, selección de idioma, selección de micrófono, modos de interacción flexibles y control explícito sobre la salida textual.**

La prioridad no es abarcar demasiadas funciones, sino construir una base de producto clara, útil y extensible.
# Lessons

1. **Nunca trabajar en main** — crear rama `feat/`, `fix/` o `chore/` desde el branch correcto antes de cualquier edición. El branch base no es siempre `main`; verificar en qué rama está el trabajo activo del equipo antes de crear la rama del feature.

2. **Leer git log/diff antes de editar archivos compartidos** — `git log <archivo>` y `git diff <base> -- <archivo>` revelan cambios intencionales de otros desarrolladores que no son visibles leyendo solo el archivo actual.

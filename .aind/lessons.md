# Lessons

1. **Nunca trabajar en main** — crear rama `feat/`, `fix/` o `chore/` desde el branch correcto antes de cualquier edición. El branch base no es siempre `main`; verificar en qué rama está el trabajo activo del equipo antes de crear la rama del feature.

2. **Leer git log/diff antes de editar archivos compartidos** — `git log <archivo>` y `git diff <base> -- <archivo>` revelan cambios intencionales de otros desarrolladores que no son visibles leyendo solo el archivo actual.

3. **Una rama = un issue = un PR directo a main** — nunca acumular PRs en una rama de integración larga. Ramas de feature de otros devs no son base válida para trabajo propio. Siempre desde main, siempre PR a main.

4. **Usar git worktrees, nunca git checkout en /voxa principal** — Kiro opera desde el directorio principal. Hacer git checkout ahí pisa su working directory. Toda tarea de Claude debe vivir en un worktree separado. Nunca `git checkout` en el directorio raíz del proyecto.

5. **git reset --hard destruye archivos no commiteados** — antes de cualquier reset --hard, verificar si hay archivos modificados sin commit en el working tree (.aind/, specs, etc.). Hacer stash o commit primero.

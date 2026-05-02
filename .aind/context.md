# Project Context

## Decisions

- Voxa: app de dictado por voz con transcripción (Whisper) y refinamiento LLM (Qwen2.5 1.5B)
- LlamaEngine usa patrón `Mutex<Option<LlamaEngine>>` — reinicia server si muere externamente (fix #59)
- Whisper se reinicializa en cada dictado — identificado como overhead ~2.5s a eliminar (#68)
- Pipeline: audio → Whisper STT → LLaMA refinement → clipboard/output
- Worktrees obligatorios para Claude: `.claude/worktrees/issue-{id}/`
- `.aind/tasks.md` global eliminado — tasks solo en `specs/[módulo]/tasks.md`

use crate::db::FormattingHint;

const MAX_FORMATTING_BLOCK_CHARS: usize = 2048; // ~512 tokens

pub fn build_formatting_block(mode: &str, language: &str, hints: &[FormattingHint]) -> String {
    let list_numbered = match mode {
        "markdown" => "Use `1.`, `2.`, `3.` for numbered lists.",
        _ => "Use `1)`, `2)`, `3)` for numbered lists (no markdown).",
    };
    let list_bullets = match mode {
        "markdown" => "Use `-` for bullet lists. Use `**text**` for bold, `*text*` for italic when requested.",
        _ => "Use `-` for bullet lists. No markdown bold/italic.",
    };

    let (cue_numbered, cue_bullets, cue_quote, cue_newline) = match language {
        "en" => (
            "\"point one\", \"first\", \"step one\", \"1 ...\", \"second\", \"third\"",
            "\"we have N points\", \"the points are\", \"there are N options\", \"a) ... b) ...\"",
            "\"in quotes X\"",
            "\"new line\", \"new paragraph\"",
        ),
        _ => (
            "\"punto uno\", \"primero\", \"paso uno\", \"1 ...\", \"segundo\", \"tercero\"",
            "\"tenemos N puntos\", \"los puntos son\", \"hay N opciones\", \"a) ... b) ...\"",
            "\"entre comillas X\"",
            "\"nueva línea\", \"nuevo párrafo\"",
        ),
    };

    let mut block = format!(
        "FORMATTING RULES (apply after all other instructions):\
\n- Numbered list cues ({cue_numbered}): {list_numbered}\
\n- Bullet list cues ({cue_bullets}): {list_bullets}\
\n- Quote cue ({cue_quote}): surround the content with \"\".\
\n- Line break cues ({cue_newline}): insert \\n or \\n\\n accordingly.\
\n- Symbol substitution: replace spoken symbol names with their character (arroba→@, guión→-, guión bajo→_, dos puntos→:, punto y coma→;, copyright→©, trademark→™, más→+, igual→=, mayor que→>, menor que→<, ampersand→&, barra→/, almohadilla→#, asterisco→*, porcentaje→%, dólar→$, euro→€).\
\n- If no structural cues are present, output the text exactly as you would without these rules.\
\n- Return ONLY the final text. No explanations."
    );

    if !hints.is_empty() {
        block.push_str("\nUSER PREFERENCES (apply always):");
        for h in hints {
            let line = format!("\n- {}", h.hint);
            if block.len() + line.len() > MAX_FORMATTING_BLOCK_CHARS {
                break;
            }
            block.push_str(&line);
        }
    }

    block
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_hints() -> Vec<FormattingHint> { vec![] }

    #[test]
    fn plain_mode_no_markdown() {
        let block = build_formatting_block("plain", "es", &no_hints());
        assert!(block.contains("1)"), "plain mode should use 1) not 1.");
        assert!(!block.contains("**text**"), "plain mode should not mention bold markdown");
    }

    #[test]
    fn markdown_mode_uses_dot_list() {
        let block = build_formatting_block("markdown", "es", &no_hints());
        assert!(block.contains("1."), "markdown mode should use 1.");
        assert!(block.contains("**text**"), "markdown mode should mention bold");
    }

    #[test]
    fn english_cues_in_en_language() {
        let block = build_formatting_block("plain", "en", &no_hints());
        assert!(block.contains("point one"), "en language should use English cues");
        assert!(!block.contains("punto uno"), "en language should not use Spanish cues");
    }

    #[test]
    fn hints_injected() {
        let hints = vec![FormattingHint {
            id: 1, profile_id: 1,
            pattern: "test".to_string(),
            hint: "Always use bullet lists".to_string(),
            frequency: 5, is_promoted: true,
        }];
        let block = build_formatting_block("plain", "es", &hints);
        assert!(block.contains("Always use bullet lists"));
    }

    #[test]
    fn token_limit_respected() {
        let many_hints: Vec<FormattingHint> = (0..100).map(|i| FormattingHint {
            id: i, profile_id: 1,
            pattern: format!("pattern{i}"),
            hint: "A".repeat(100),
            frequency: 1, is_promoted: false,
        }).collect();
        let block = build_formatting_block("plain", "es", &many_hints);
        assert!(block.len() <= MAX_FORMATTING_BLOCK_CHARS + 200);
    }
}

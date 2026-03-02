use crate::models::{Glyph, GlyphForm, Position, RenderedGlyph, Token};
use std::collections::HashMap;

/// Parse a multi-line art string into exactly `height` lines, width-normalized.
pub fn parse_art(art: &str, height: usize) -> RenderedGlyph {
    let raw_lines: Vec<&str> = art.split('\n').collect();

    let start = if raw_lines.first().is_some_and(|l| l.is_empty()) { 1 } else { 0 };

    let mut lines: Vec<String> = Vec::with_capacity(height);
    for i in 0..height {
        let idx = start + i;
        if idx < raw_lines.len() {
            lines.push(raw_lines[idx].to_string());
        } else {
            lines.push(String::new());
        }
    }

    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    for line in &mut lines {
        let current = line.chars().count();
        if current < width {
            line.push_str(&" ".repeat(width - current));
        }
    }

    RenderedGlyph { lines }
}

/// Select the appropriate art form for a glyph based on its position.
pub fn get_form<'a>(glyph: &'a Glyph, position: &Position) -> &'a GlyphForm {
    match position {
        Position::Isolated => &glyph.isolated,
        Position::Initial => &glyph.initial,
        Position::Medial => &glyph.medial,
        Position::Final => &glyph.final_form,
    }
}

/// Render tokens with their positional forms into final output lines.
pub fn render(
    tokens: &[Token],
    positions: &[Position],
    glyphs: &HashMap<String, Glyph>,
    height: usize,
    spacing: usize,
) -> Vec<String> {
    let mut rendered: Vec<RenderedGlyph> = Vec::new();

    for (idx, token) in tokens.iter().enumerate() {
        match token {
            Token::Space => {
                let w = 3;
                rendered
                    .push(RenderedGlyph { lines: (0..height).map(|_| " ".repeat(w)).collect() });
            }
            Token::Glyph(name) => {
                let glyph = glyphs.get(name.as_str()).unwrap();
                let form = get_form(glyph, &positions[idx]);
                rendered.push(parse_art(&form.art, height));
            }
        }
    }

    rendered.reverse();

    let spacer = " ".repeat(spacing);
    let mut output = vec![String::new(); height];
    for (gi, rg) in rendered.iter().enumerate() {
        if gi > 0 && spacing > 0 {
            for line in &mut output {
                line.push_str(&spacer);
            }
        }
        for (line_idx, line) in rg.lines.iter().enumerate() {
            output[line_idx].push_str(line);
        }
    }

    output
}

pub fn colorize_line(line: &str) -> String {
    let mut result = String::new();
    for c in line.chars() {
        match c {
            '█' => result.push_str("\x1b[33m█\x1b[0m"),
            '▄' => result.push_str("\x1b[93m▄\x1b[0m"),
            '▀' => result.push_str("\x1b[93m▀\x1b[0m"),
            '•' => result.push_str("\x1b[36m•\x1b[0m"),
            _ => result.push(c),
        }
    }
    result
}

pub fn frame_output(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return vec!["╔══╗".to_string(), "║  ║".to_string(), "╚══╝".to_string()];
    }

    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let inner_width = width + 2;

    let mut result = Vec::new();
    result.push(format!("╔{}╗", "═".repeat(inner_width)));

    for line in lines {
        let padding = " ".repeat(width.saturating_sub(line.chars().count()));
        result.push(format!("║ {}{} ║", line, padding));
    }

    result.push(format!("╚{}╝", "═".repeat(inner_width)));

    result
}

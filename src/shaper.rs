use crate::models::{Glyph, Position, Token};
use std::collections::HashMap;

/// Load glyphs from TOML content. Returns (height, glyph_map).
pub fn load_glyphs(content: &str) -> Result<(usize, HashMap<String, Glyph>), String> {
    let value: toml::Value = content.parse().map_err(|e| format!("TOML parse error: {}", e))?;

    let table = value.as_table().ok_or("TOML must be a top-level table")?;

    let height = table
        .get("height")
        .and_then(|v| v.as_integer())
        .ok_or("'height' field required (integer)")? as usize;

    let mut glyphs = HashMap::new();
    for (key, val) in table {
        if key == "height" {
            continue;
        }
        match val.clone().try_into::<Glyph>() {
            Ok(glyph) => {
                glyphs.insert(key.clone(), glyph);
            }
            Err(e) => {
                eprintln!("Warning: '{}' glyph parse error: {}", key, e);
            }
        }
    }

    Ok((height, glyphs))
}

/// Build a Unicode char → glyph name lookup map.
pub fn build_unicode_map(glyphs: &HashMap<String, Glyph>) -> HashMap<char, String> {
    let mut map = HashMap::new();
    for (name, glyph) in glyphs {
        let chars: Vec<char> = glyph.unicode.chars().collect();
        if chars.len() == 1 {
            map.insert(chars[0], name.clone());
        }
    }
    map
}

/// Normalize an Arabic character
pub fn normalize_char(c: char) -> Option<char> {
    match c {
        '\u{0622}' | '\u{0623}' | '\u{0625}' | '\u{0671}' => Some('\u{0627}'),
        '\u{0629}' => Some('\u{0647}'),
        '\u{064B}' | '\u{064C}' | '\u{064D}' | '\u{064E}' | '\u{064F}' | '\u{0650}'
        | '\u{0651}' | '\u{0652}' | '\u{0670}' => None,
        '\u{0640}' => None,
        _ => Some(c),
    }
}

/// Tokenize input text
pub fn tokenize(
    input: &str,
    unicode_map: &HashMap<char, String>,
    glyphs: &HashMap<String, Glyph>,
) -> Vec<Token> {
    let chars: Vec<char> = input.chars().filter_map(normalize_char).collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == ' ' {
            tokens.push(Token::Space);
            i += 1;
            continue;
        }

        if c == '\u{0644}'
            && i + 1 < chars.len()
            && chars[i + 1] == '\u{0627}'
            && glyphs.contains_key("lam_alif")
        {
            tokens.push(Token::Glyph("lam_alif".to_string()));
            i += 2;
            continue;
        }

        if let Some(name) = unicode_map.get(&c) {
            tokens.push(Token::Glyph(name.clone()));
        } else {
            eprintln!("Warning: unknown character '{}' (U+{:04X}) skipped", c, c as u32);
        }

        i += 1;
    }

    tokens
}

/// Determine the positional form for each token based on neighbor connection rules.
pub fn determine_positions(tokens: &[Token], glyphs: &HashMap<String, Glyph>) -> Vec<Position> {
    let glyph_info: Vec<Option<&Glyph>> = tokens
        .iter()
        .map(|t| match t {
            Token::Glyph(name) => glyphs.get(name.as_str()),
            Token::Space => None,
        })
        .collect();

    tokens
        .iter()
        .enumerate()
        .map(|(idx, token)| match token {
            Token::Space => Position::Isolated,
            Token::Glyph(_) => {
                let current = glyph_info[idx].unwrap();

                let connected_to_prev = idx > 0
                    && glyph_info[idx - 1]
                        .is_some_and(|prev| prev.connects_next && current.connects_prev);

                let connected_to_next = idx + 1 < tokens.len()
                    && glyph_info[idx + 1]
                        .is_some_and(|next| current.connects_next && next.connects_prev);

                match (connected_to_prev, connected_to_next) {
                    (false, false) => Position::Isolated,
                    (false, true) => Position::Initial,
                    (true, true) => Position::Medial,
                    (true, false) => Position::Final,
                }
            }
        })
        .collect()
}

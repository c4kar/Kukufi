use kukufi::models::{Glyph, Position, Token};
use kukufi::renderer::{colorize_line, frame_output, parse_art, render};
use kukufi::shaper::{build_unicode_map, determine_positions, load_glyphs, tokenize};
use std::collections::HashMap;
use std::fs;

fn setup() -> (usize, HashMap<String, Glyph>, HashMap<char, String>) {
    let content = fs::read_to_string("assets/glyphs.toml").expect("assets/glyphs.toml gerekli");
    let (height, glyphs) = load_glyphs(&content).unwrap();
    let unicode_map = build_unicode_map(&glyphs);
    (height, glyphs, unicode_map)
}

#[test]
fn test_all_glyphs_load() {
    let (height, glyphs, _) = setup();
    assert_eq!(height, 8);
    // 28 letters + lam_alif + hamza = 30
    assert!(glyphs.len() >= 30, "Beklenen en az 30, bulunan: {}", glyphs.len());
}

#[test]
fn test_unicode_map_excludes_ligatures() {
    let (_, _, unicode_map) = setup();
    assert_eq!(unicode_map.get(&'\u{0644}').map(|s| s.as_str()), Some("lam"));
    assert_eq!(unicode_map.get(&'\u{0627}').map(|s| s.as_str()), Some("alif"));
}

#[test]
fn test_lam_alif_ligature_detection() {
    let (_, glyphs, unicode_map) = setup();
    let tokens = tokenize("لا", &unicode_map, &glyphs);
    assert_eq!(tokens.len(), 1);
    match &tokens[0] {
        Token::Glyph(name) => assert_eq!(name, "lam_alif"),
        _ => panic!("Beklenen: lam_alif token"),
    }
}

#[test]
fn test_single_letter_isolated() {
    let (_, glyphs, unicode_map) = setup();
    let tokens = tokenize("ا", &unicode_map, &glyphs);
    assert_eq!(tokens.len(), 1);
    let positions = determine_positions(&tokens, &glyphs);
    assert!(matches!(positions[0], Position::Isolated));
}

#[test]
fn test_non_connecting_breaks_chain() {
    let (_, glyphs, unicode_map) = setup();
    let tokens = tokenize("دار", &unicode_map, &glyphs);
    assert_eq!(tokens.len(), 3);
    let positions = determine_positions(&tokens, &glyphs);
    assert!(matches!(positions[0], Position::Isolated));
    assert!(matches!(positions[1], Position::Isolated));
    assert!(matches!(positions[2], Position::Isolated));
}

#[test]
fn test_connecting_word() {
    let (_, glyphs, unicode_map) = setup();
    let tokens = tokenize("بسم", &unicode_map, &glyphs);
    assert_eq!(tokens.len(), 3);
    let positions = determine_positions(&tokens, &glyphs);
    assert!(matches!(positions[0], Position::Initial));
    assert!(matches!(positions[1], Position::Medial));
    assert!(matches!(positions[2], Position::Final));
}

#[test]
fn test_space_breaks_words() {
    let (_, glyphs, unicode_map) = setup();
    let tokens = tokenize("بب بب", &unicode_map, &glyphs);
    assert_eq!(tokens.len(), 5);
    let positions = determine_positions(&tokens, &glyphs);
    assert!(matches!(positions[0], Position::Initial));
    assert!(matches!(positions[1], Position::Final));
    assert!(matches!(positions[2], Position::Isolated));
    assert!(matches!(positions[3], Position::Initial));
    assert!(matches!(positions[4], Position::Final));
}

#[test]
fn test_normalization_alif_variants() {
    let (_, glyphs, unicode_map) = setup();
    for input in &["إ", "أ", "آ"] {
        let tokens = tokenize(input, &unicode_map, &glyphs);
        assert_eq!(tokens.len(), 1, "Giriş: {}", input);
        match &tokens[0] {
            Token::Glyph(name) => assert_eq!(name, "alif", "Giriş: {}", input),
            _ => panic!("Beklenen: alif token, giriş: {}", input),
        }
    }
}

#[test]
fn test_normalization_strips_diacritics() {
    let (_, glyphs, unicode_map) = setup();
    let tokens = tokenize("بِسْمِ", &unicode_map, &glyphs);
    assert_eq!(tokens.len(), 3);
}

#[test]
fn test_art_height_consistent() {
    let (height, glyphs, _) = setup();
    for (name, glyph) in &glyphs {
        for (form_name, form) in [
            ("isolated", &glyph.isolated),
            ("initial", &glyph.initial),
            ("medial", &glyph.medial),
            ("final", &glyph.final_form),
        ] {
            let rendered = parse_art(&form.art, height);
            assert_eq!(
                rendered.lines.len(),
                height,
                "{}:{} yükseklik hatası: {} ≠ {}",
                name,
                form_name,
                rendered.lines.len(),
                height
            );
        }
    }
}

#[test]
fn test_render_output_has_correct_height() {
    let (height, glyphs, unicode_map) = setup();
    let tokens = tokenize("بسم", &unicode_map, &glyphs);
    let positions = determine_positions(&tokens, &glyphs);
    let output = render(&tokens, &positions, &glyphs, height, 0);
    assert_eq!(output.len(), height);
}

#[test]
fn test_empty_input() {
    let (_, glyphs, unicode_map) = setup();
    let tokens = tokenize("", &unicode_map, &glyphs);
    assert!(tokens.is_empty());
}

#[test]
fn test_non_arabic_skipped() {
    let (_, glyphs, unicode_map) = setup();
    let tokens = tokenize("hello", &unicode_map, &glyphs);
    assert!(tokens.is_empty());
}

#[test]
fn test_colorize_line() {
    let input = "█";
    let output = colorize_line(input);
    assert!(output.contains("\x1b[33m"));
    assert!(output.contains("\x1b[0m"));

    let input = "▄▀";
    let output = colorize_line(input);
    assert!(output.contains("\x1b[93m"));
    assert!(output.contains("\x1b[0m"));

    let input = "•";
    let output = colorize_line(input);
    assert!(output.contains("\x1b[36m"));
    assert!(output.contains("\x1b[0m"));

    let input = "█ ▄•▀";
    let output = colorize_line(input);
    assert!(output.contains("\x1b[33m"));
    assert!(output.contains("\x1b[93m"));
    assert!(output.contains("\x1b[36m"));
}

#[test]
fn test_frame_output() {
    let lines: Vec<String> = vec![];
    let framed = frame_output(&lines);
    assert!(framed.len() >= 2);
    assert!(framed[0].starts_with('╔'));
    assert!(framed.last().unwrap().starts_with('╚'));

    let lines = vec!["test".to_string()];
    let framed = frame_output(&lines);
    assert_eq!(framed.len(), 3);
    assert!(framed[0].starts_with('╔'));
    assert!(framed[0].ends_with('╗'));
    assert!(framed[1].contains("test"));
    assert!(framed[1].starts_with('║'));
    assert!(framed[2].starts_with('╚'));

    let lines = vec!["a".to_string(), "b".to_string()];
    let framed = frame_output(&lines);
    assert_eq!(framed.len(), 4);
}

use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, ClearType},
};

// ── TOML Data Structures ──────────────────────────────────────

/// A single positional form of a glyph.
#[derive(Debug, Deserialize, Clone)]
struct GlyphForm {
    art: String,
}

/// A complete glyph entry with 4 positional forms and connection metadata.
#[derive(Debug, Deserialize, Clone)]
struct Glyph {
    unicode: String,
    connects_next: bool,
    connects_prev: bool,
    isolated: GlyphForm,
    initial: GlyphForm,
    medial: GlyphForm,
    #[serde(rename = "final")]
    final_form: GlyphForm,
}

// ── Internal Types ────────────────────────────────────────────

/// Positional form of a letter determined by its neighbors.
enum Position {
    Isolated,
    Initial,
    Medial,
    Final,
}

/// A token in the processed input stream.
enum Token {
    Glyph(String), // glyph name key into the map
    Space,
}

/// Width-normalized, parsed art lines ready for concatenation.
struct RenderedGlyph {
    lines: Vec<String>,
}

// ── Glyph Loading ─────────────────────────────────────────────

/// Load glyphs from TOML content. Returns (height, glyph_map).
fn load_glyphs(content: &str) -> (usize, HashMap<String, Glyph>) {
    let value: toml::Value = content.parse().unwrap_or_else(|e| {
        eprintln!("TOML ayrıştırma hatası: {}", e);
        process::exit(1);
    });

    let table = value.as_table().unwrap_or_else(|| {
        eprintln!("TOML üst düzey bir tablo olmalı");
        process::exit(1);
    });

    let height = table
        .get("height")
        .and_then(|v| v.as_integer())
        .unwrap_or_else(|| {
            eprintln!("'height' alanı gerekli (tamsayı)");
            process::exit(1);
        }) as usize;

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
                eprintln!("Uyarı: '{}' glifi ayrıştırılamadı: {}", key, e);
            }
        }
    }

    (height, glyphs)
}

/// Build a Unicode char → glyph name lookup map.
/// Only single-character unicode entries are included; multi-char entries
/// (like lam_alif "لا") are ligatures handled by the tokenizer.
fn build_unicode_map(glyphs: &HashMap<String, Glyph>) -> HashMap<char, String> {
    let mut map = HashMap::new();
    for (name, glyph) in glyphs {
        let chars: Vec<char> = glyph.unicode.chars().collect();
        if chars.len() == 1 {
            map.insert(chars[0], name.clone());
        }
    }
    map
}

// ── Art Parsing ───────────────────────────────────────────────

/// Parse a multi-line art string into exactly `height` lines, width-normalized.
/// TOML triple-quoted strings start with a newline after ''', so the first
/// element after splitting is empty and must be skipped.
fn parse_art(art: &str, height: usize) -> RenderedGlyph {
    let raw_lines: Vec<&str> = art.split('\n').collect();

    // Skip leading empty line from TOML triple-quote
    let start = if raw_lines.first().map_or(false, |l| l.is_empty()) {
        1
    } else {
        0
    };

    let mut lines: Vec<String> = Vec::with_capacity(height);
    for i in 0..height {
        let idx = start + i;
        if idx < raw_lines.len() {
            lines.push(raw_lines[idx].to_string());
        } else {
            lines.push(String::new());
        }
    }

    // Pad all lines to the same width (max char count)
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    for line in &mut lines {
        let current = line.chars().count();
        if current < width {
            line.push_str(&" ".repeat(width - current));
        }
    }

    RenderedGlyph { lines }
}

// ── Tokenization ──────────────────────────────────────────────

/// Normalize an Arabic character: map alif variants to plain alif,
/// return None for diacritics (tashkeel) that should be stripped.
fn normalize_char(c: char) -> Option<char> {
    match c {
        // Alif variants → plain alif
        '\u{0622}' | // آ  alif madda
        '\u{0623}' | // أ  alif hamza above
        '\u{0625}' | // إ  alif hamza below
        '\u{0671}'   // ٱ  alif wasla
            => Some('\u{0627}'), // ا

        // Tee marbuta → ha
        '\u{0629}' => Some('\u{0647}'), // ة → ه

        // Tashkeel (diacritics) — strip silently
        '\u{064B}' | // fathatan
        '\u{064C}' | // dammatan
        '\u{064D}' | // kasratan
        '\u{064E}' | // fatha
        '\u{064F}' | // damma
        '\u{0650}' | // kasra
        '\u{0651}' | // shadda
        '\u{0652}' | // sukun
        '\u{0670}'   // superscript alif
            => None,

        // Tatweel (kashida) — strip
        '\u{0640}' => None,

        _ => Some(c),
    }
}

/// Tokenize input text: normalize chars, resolve to glyph names, detect lam-alif ligature.
fn tokenize(
    input: &str,
    unicode_map: &HashMap<char, String>,
    glyphs: &HashMap<String, Glyph>,
) -> Vec<Token> {
    // Normalize: map variants, strip diacritics
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

        // Detect lam-alif ligature: ل (U+0644) followed by ا (U+0627)
        if c == '\u{0644}' && i + 1 < chars.len() && chars[i + 1] == '\u{0627}' {
            if glyphs.contains_key("lam_alif") {
                tokens.push(Token::Glyph("lam_alif".to_string()));
                i += 2;
                continue;
            }
        }

        if let Some(name) = unicode_map.get(&c) {
            tokens.push(Token::Glyph(name.clone()));
        } else {
            eprintln!(
                "Uyarı: bilinmeyen karakter '{}' (U+{:04X}) atlandı",
                c, c as u32
            );
        }

        i += 1;
    }

    tokens
}

// ── Positional Form Selection ─────────────────────────────────

/// Determine the positional form for each token based on neighbor connection rules.
///
/// In Arabic logical (reading) order:
///   - "previous" = the letter before in reading order (visually to the RIGHT)
///   - "next"     = the letter after in reading order (visually to the LEFT)
///
/// Connection between two adjacent letters A (prev) and B (current):
///   A connects to B iff A.connects_next && B.connects_prev
fn determine_positions(tokens: &[Token], glyphs: &HashMap<String, Glyph>) -> Vec<Position> {
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

                // Connected to previous letter?
                let connected_to_prev = idx > 0
                    && glyph_info[idx - 1]
                        .map_or(false, |prev| prev.connects_next && current.connects_prev);

                // Connected to next letter?
                let connected_to_next = idx + 1 < tokens.len()
                    && glyph_info[idx + 1]
                        .map_or(false, |next| current.connects_next && next.connects_prev);

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

/// Select the appropriate art form for a glyph based on its position.
fn get_form<'a>(glyph: &'a Glyph, position: &Position) -> &'a GlyphForm {
    match position {
        Position::Isolated => &glyph.isolated,
        Position::Initial => &glyph.initial,
        Position::Medial => &glyph.medial,
        Position::Final => &glyph.final_form,
    }
}

// ── Rendering ─────────────────────────────────────────────────

/// Render tokens with their positional forms into final output lines.
/// Glyphs are reversed for RTL→visual LTR display, then concatenated horizontally.
/// When `spacing > 0`, inserts that many blank columns between each glyph.
fn render(
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
                rendered.push(RenderedGlyph {
                    lines: (0..height).map(|_| " ".repeat(w)).collect(),
                });
            }
            Token::Glyph(name) => {
                let glyph = glyphs.get(name.as_str()).unwrap();
                let form = get_form(glyph, &positions[idx]);
                rendered.push(parse_art(&form.art, height));
            }
        }
    }

    // Reverse for RTL → visual left-to-right display
    rendered.reverse();

    // Concatenate horizontally with optional spacing
    let spacer = " ".repeat(spacing);
    let mut output = vec![String::new(); height];
    for (gi, rg) in rendered.iter().enumerate() {
        if gi > 0 && spacing > 0 {
            for line_idx in 0..height {
                output[line_idx].push_str(&spacer);
            }
        }
        for (line_idx, line) in rg.lines.iter().enumerate() {
            output[line_idx].push_str(line);
        }
    }

    output
}

// ── ANSI Color Support ──────────────────────────────────────────

/// Colorize a line of ASCII art with ANSI colors:
/// - █ blocks → gold/yellow (\x1b[33m)
/// - ▄ and ▀ connectors → light yellow (\x1b[93m)
/// - • dots → cyan (\x1b[36m)
/// - Spaces stay as-is
///   Reset at end with \x1b[0m
fn colorize_line(line: &str) -> String {
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

/// Frame output with Unicode borders.
fn frame_output(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        return vec!["╔══╗".to_string(), "║  ║".to_string(), "╚══╝".to_string()];
    }

    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let inner_width = width + 2; // padding + content

    let mut result = Vec::new();

    // Top border
    result.push(format!("╔{}╗", "═".repeat(inner_width)));

    // Content lines with side borders
    for line in lines {
        let padding = " ".repeat(width.saturating_sub(line.chars().count()));
        result.push(format!("║ {}{} ║", line, padding));
    }

    // Bottom border
    result.push(format!("╚{}╝", "═".repeat(inner_width)));

    result
}

/// Validate all glyphs for structural integrity.
/// Returns (glyph_count, warning_count).
fn validate_glyphs(glyphs: &HashMap<String, Glyph>, height: usize) -> (usize, usize) {
    let mut warnings = 0;
    let mut validated = 0;

    for (name, glyph) in glyphs {
        let forms = [
            ("isolated", &glyph.isolated),
            ("initial", &glyph.initial),
            ("medial", &glyph.medial),
            ("final", &glyph.final_form),
        ];

        let mut widths: Vec<usize> = Vec::new();
        let mut height_ok = true;

        for (form_name, form) in &forms {
            let rendered = parse_art(&form.art, height);

            // Check height
            if rendered.lines.len() != height {
                eprintln!(
                    "Uyarı: {}:{} yetersiz satır sayısı ({} / {})",
                    name,
                    form_name,
                    rendered.lines.len(),
                    height
                );
                height_ok = false;
            }

            // Collect widths
            let w = rendered
                .lines
                .iter()
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(0);
            widths.push(w);
        }

        if !height_ok {
            warnings += 1;
        }

        // Check width consistency across forms
        if let (Some(min_w), Some(max_w)) = (widths.iter().min(), widths.iter().max()) {
            if max_w.saturating_sub(*min_w) > 2 {
                eprintln!("Uyarı: {} form genişlikleri tutarsız: {:?}", name, widths);
                warnings += 1;
            }
        }

        validated += 1;
    }

    (validated, warnings)
}

/// Search for glyphs.toml in current directory or next to the executable.
fn find_glyphs_file() -> String {
    if std::path::Path::new("glyphs.toml").exists() {
        return "glyphs.toml".to_string();
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("glyphs.toml");
            if p.exists() {
                return p.to_string_lossy().to_string();
            }
        }
    }
    "glyphs.toml".to_string()
}

/// Process a single input line: tokenize → position → render.
/// Returns rendered lines (without printing).
fn render_line(
    input: &str,
    unicode_map: &HashMap<char, String>,
    glyphs: &HashMap<String, Glyph>,
    height: usize,
    frame: bool,
    spacing: usize,
) -> Vec<String> {
    let tokens = tokenize(input, unicode_map, glyphs);
    if tokens.is_empty() {
        return Vec::new();
    }

    let positions = determine_positions(&tokens, glyphs);
    let mut output = render(&tokens, &positions, glyphs, height, spacing);

    if frame {
        output = frame_output(&output);
    }

    output
}

/// Print rendered lines to stdout, optionally with color.
fn print_lines(lines: &[String], color: bool) {
    for line in lines {
        if color {
            println!("{}", colorize_line(line));
        } else {
            println!("{}", line);
        }
    }
}

/// Write rendered lines to a file. If `ansi` is true, includes color codes.
fn write_to_file(lines: &[String], path: &str, ansi: bool) {
    let mut content = String::new();
    for line in lines {
        if ansi {
            content.push_str(&colorize_line(line));
        } else {
            content.push_str(line);
        }
        content.push('\n');
    }

    fs::write(path, &content).unwrap_or_else(|e| {
        eprintln!("Dosya yazma hatası ({}): {}", path, e);
        process::exit(1);
    });
    eprintln!("Çıktı dosyaya yazıldı: {}", path);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Load glyphs
    let path = find_glyphs_file();
    let content = fs::read_to_string(&path).unwrap_or_else(|_| {
        // Fallback to embedded glyphs.toml if not found on disk
        include_str!("../glyphs.toml").to_string()
    });
    let (height, glyphs) = load_glyphs(&content);
    let unicode_map = build_unicode_map(&glyphs);

    // Parse global flags
    let mut color = false;
    let mut frame = false;
    let mut spacing: usize = 0;
    let mut output_file: Option<String> = None;
    let mut remaining_args: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--color" => color = true,
            "--frame" => frame = true,
            "--spacing" => {
                i += 1;
                if i < args.len() {
                    spacing = args[i].parse().unwrap_or_else(|_| {
                        eprintln!("--spacing bir tamsayı gerektirir");
                        process::exit(1);
                    });
                } else {
                    eprintln!("--spacing bir değer gerektirir");
                    process::exit(1);
                }
            }
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    output_file = Some(args[i].clone());
                } else {
                    eprintln!("--output bir dosya yolu gerektirir");
                    process::exit(1);
                }
            }
            _ => remaining_args.push(args[i].clone()),
        }
        i += 1;
    }

    // Handle subcommands
    if !remaining_args.is_empty() {
        match remaining_args[0].as_str() {
            "--help" | "-h" => {
                println!("KuKufi — Arapça metni Kûfi hat ASCII sanatına dönüştürür");
                println!();
                println!("Kullanım:");
                println!("  kukufi [bayraklar] <Arapça metin>   Metni Kûfi hat olarak göster");
                println!("  kukufi --interactive                 Canlı önizleme modu");
                println!("  kukufi --list                        Tüm glifleri listele");
                println!("  kukufi --show <glif adı>             Bir glifin 4 formunu göster");
                println!("  kukufi --show-all                    Tüm gliflerin 4 formunu göster");
                println!("  kukufi --validate                    Glif dosyasını doğrula");
                println!("  echo 'بسم' | kukufi [bayraklar]      Stdin'den oku");
                println!();
                println!("Bayraklar:");
                println!("  --color              Çıktıyı ANSI renklerle göster");
                println!("  --frame              Çıktıyı Unicode çerçeve ile sarma");
                println!("  --spacing <N>        Glifler arası N sütun boşluk ekle");
                println!("  --output <dosya>     Çıktıyı dosyaya yaz (.ans uzantısı → renkli)");
                println!();
                println!("Örnekler:");
                println!("  kukufi بسم");
                println!("  kukufi --color \"بسم الله\"");
                println!("  kukufi --frame --color \"بسم\"");
                println!("  kukufi --output basmala.txt \"بسم الله الرحمن الرحيم\"");
                println!("  kukufi --output basmala.ans --color \"بسم\"");
                println!("  kukufi --interactive");
                println!("  kukufi --show ba");
                return;
            }
            "--list" => {
                print_glyph_list(&glyphs);
                return;
            }
            "--show" => {
                if remaining_args.len() < 2 {
                    eprintln!("Kullanım: kukufi --show <glif adı>");
                    eprintln!("Glif listesi için: kukufi --list");
                    process::exit(1);
                }
                let name = &remaining_args[1];
                show_glyph(name, &glyphs, height);
                return;
            }
            "--show-all" => {
                show_all_glyphs(&glyphs, height);
                return;
            }
            "--validate" => {
                let (count, warnings) = validate_glyphs(&glyphs, height);
                println!("{} glif doğrulandı, {} uyarı", count, warnings);
                return;
            }
            "--interactive" | "-i" => {
                interactive_mode(&unicode_map, &glyphs, height, color, frame, spacing);
                return;
            }
            _ => {
                // Argument mode: process the provided text
                let input = remaining_args.join(" ");
                let lines = render_line(&input, &unicode_map, &glyphs, height, frame, spacing);
                if !lines.is_empty() {
                    if let Some(ref out_path) = output_file {
                        let ansi = color || out_path.ends_with(".ans");
                        write_to_file(&lines, out_path, ansi);
                    }
                    print_lines(&lines, color);
                }
            }
        }
    } else {
        // Stdin mode: read lines from pipe/stdin
        let stdin = io::stdin();
        let mut any = false;
        let mut all_lines: Vec<String> = Vec::new();
        for line in stdin.lock().lines() {
            match line {
                Ok(text) => {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        if any {
                            println!();
                            all_lines.push(String::new());
                        }
                        let rendered =
                            render_line(trimmed, &unicode_map, &glyphs, height, frame, spacing);
                        print_lines(&rendered, color);
                        all_lines.extend(rendered);
                        any = true;
                    }
                }
                Err(e) => {
                    eprintln!("Okuma hatası: {}", e);
                    break;
                }
            }
        }
        if !any {
            eprintln!("Kullanım: kukufi <Arapça metin>");
            eprintln!("       veya: echo 'بسم' | kukufi");
            eprintln!("Yardım için: kukufi --help");
            process::exit(1);
        }
        if let Some(ref out_path) = output_file {
            let ansi = color || out_path.ends_with(".ans");
            write_to_file(&all_lines, out_path, ansi);
        }
    }
}

// ── Interactive TUI Mode ──────────────────────────────────────

/// Interactive mode: type Arabic text and see live Kufi preview.
/// Ctrl+C or Esc to exit. Backspace to delete. Enter to clear.
fn interactive_mode(
    unicode_map: &HashMap<char, String>,
    glyphs: &HashMap<String, Glyph>,
    height: usize,
    color: bool,
    frame: bool,
    spacing: usize,
) {
    let mut stdout = io::stdout();

    // Enter raw mode
    terminal::enable_raw_mode().unwrap_or_else(|e| {
        eprintln!("Terminal raw modu etkinleştirilemedi: {}", e);
        process::exit(1);
    });

    execute!(
        stdout,
        terminal::Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )
    .ok();

    let header = "╔══════════════════════════════════════════════════╗";
    let title = "║  KuKufi Canlı Önizleme — Esc/Ctrl+C: Çıkış      ║";
    let footer = "╚══════════════════════════════════════════════════╝";

    // Print header
    execute!(stdout, cursor::MoveTo(0, 0)).ok();
    print!("\x1b[36m{}\r\n{}\r\n{}\x1b[0m\r\n", header, title, footer);
    print!("\r\n");
    stdout.flush().ok();

    let prompt_row: u16 = 4;
    let preview_start_row: u16 = 6;

    let mut input_buf = String::new();

    // Draw initial prompt
    draw_prompt(&mut stdout, prompt_row, &input_buf);
    draw_preview(
        &mut stdout,
        preview_start_row,
        &input_buf,
        unicode_map,
        glyphs,
        height,
        color,
        frame,
        spacing,
    );

    loop {
        if event::poll(std::time::Duration::from_millis(100)).unwrap_or(false) {
            if let Ok(ev) = event::read() {
                match ev {
                    Event::Key(key_event) => {
                        // Exit on Esc or Ctrl+C
                        if key_event.code == KeyCode::Esc
                            || (key_event.code == KeyCode::Char('c')
                                && key_event.modifiers.contains(KeyModifiers::CONTROL))
                        {
                            break;
                        }

                        match key_event.code {
                            KeyCode::Char(c) => {
                                input_buf.push(c);
                            }
                            KeyCode::Backspace => {
                                input_buf.pop();
                            }
                            KeyCode::Enter => {
                                input_buf.clear();
                            }
                            _ => {}
                        }

                        draw_prompt(&mut stdout, prompt_row, &input_buf);
                        draw_preview(
                            &mut stdout,
                            preview_start_row,
                            &input_buf,
                            unicode_map,
                            glyphs,
                            height,
                            color,
                            frame,
                            spacing,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    // Clean up
    terminal::disable_raw_mode().ok();
    execute!(
        stdout,
        cursor::MoveTo(0, preview_start_row + height as u16 + 4)
    )
    .ok();
    println!("\r");
}

fn draw_prompt(stdout: &mut io::Stdout, row: u16, input: &str) {
    execute!(stdout, cursor::MoveTo(0, row)).ok();
    // Clear line
    print!("\x1b[2K");
    print!("  \x1b[1;33mGiriş:\x1b[0m {}", input);
    stdout.flush().ok();
}

fn draw_preview(
    stdout: &mut io::Stdout,
    start_row: u16,
    input: &str,
    unicode_map: &HashMap<char, String>,
    glyphs: &HashMap<String, Glyph>,
    height: usize,
    color: bool,
    frame: bool,
    spacing: usize,
) {
    // Clear preview area
    let clear_lines = height + 4; // extra for frame
    for r in 0..clear_lines {
        execute!(stdout, cursor::MoveTo(0, start_row + r as u16)).ok();
        print!("\x1b[2K");
    }

    if input.trim().is_empty() {
        execute!(stdout, cursor::MoveTo(2, start_row)).ok();
        print!("\x1b[2m(Arapça metin yazın...)\x1b[0m");
        stdout.flush().ok();
        return;
    }

    let lines = render_line(input, unicode_map, glyphs, height, frame, spacing);

    for (i, line) in lines.iter().enumerate() {
        execute!(stdout, cursor::MoveTo(2, start_row + i as u16)).ok();
        if color {
            print!("{}", colorize_line(line));
        } else {
            print!("{}", line);
        }
    }

    stdout.flush().ok();
}

// ── Glyph Inspection Commands ─────────────────────────────────

/// Print a sorted list of all glyphs with their unicode and connection info.
fn print_glyph_list(glyphs: &HashMap<String, Glyph>) {
    let mut names: Vec<&String> = glyphs.keys().collect();
    names.sort();

    println!(
        "{:<12} {:<6} {:<10} {:<10}",
        "Ad", "Harf", "Sağa bağ.", "Sola bağ."
    );
    println!("{}", "─".repeat(42));

    for name in &names {
        let g = &glyphs[*name];
        println!(
            "{:<12} {:<6} {:<10} {:<10}",
            name,
            g.unicode,
            if g.connects_next { "evet" } else { "hayır" },
            if g.connects_prev { "evet" } else { "hayır" },
        );
    }
    println!();
    println!("Toplam: {} glif", glyphs.len());
}

/// Show all 4 positional forms of a single glyph side by side.
fn show_glyph(name: &str, glyphs: &HashMap<String, Glyph>, height: usize) {
    let glyph = match glyphs.get(name) {
        Some(g) => g,
        None => {
            eprintln!("'{}' adında glif bulunamadı.", name);
            eprintln!("Glif listesi için: kukufi --list");
            process::exit(1);
        }
    };

    let forms = [
        ("Müstakil", &glyph.isolated),
        ("Başta   ", &glyph.initial),
        ("Ortada  ", &glyph.medial),
        ("Sonda   ", &glyph.final_form),
    ];

    println!(
        "── {} ({}) ──  connects_next={}, connects_prev={}",
        name, glyph.unicode, glyph.connects_next, glyph.connects_prev
    );
    println!();

    for (label, form) in &forms {
        let rendered = parse_art(&form.art, height);
        println!("  {}:", label);
        for line in &rendered.lines {
            println!("    |{}|", line);
        }
        println!();
    }
}

/// Show all glyphs' 4 forms (for full inspection).
fn show_all_glyphs(glyphs: &HashMap<String, Glyph>, height: usize) {
    let mut names: Vec<&String> = glyphs.keys().collect();
    names.sort();

    for name in &names {
        show_glyph(name, glyphs, height);
        println!();
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (usize, HashMap<String, Glyph>, HashMap<char, String>) {
        let content = fs::read_to_string("glyphs.toml").expect("glyphs.toml gerekli");
        let (height, glyphs) = load_glyphs(&content);
        let unicode_map = build_unicode_map(&glyphs);
        (height, glyphs, unicode_map)
    }

    #[test]
    fn test_all_glyphs_load() {
        let (height, glyphs, _) = setup();
        assert_eq!(height, 8);
        // 28 letters + lam_alif + hamza = 30
        assert!(
            glyphs.len() >= 30,
            "Beklenen en az 30, bulunan: {}",
            glyphs.len()
        );
    }

    #[test]
    fn test_unicode_map_excludes_ligatures() {
        let (_, _, unicode_map) = setup();
        // lam and alif should map individually, not be overwritten by lam_alif
        assert_eq!(
            unicode_map.get(&'\u{0644}').map(|s| s.as_str()),
            Some("lam")
        );
        assert_eq!(
            unicode_map.get(&'\u{0627}').map(|s| s.as_str()),
            Some("alif")
        );
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
        // دار — dal is non-connecting on the right, so alif after dal is isolated
        let tokens = tokenize("دار", &unicode_map, &glyphs);
        assert_eq!(tokens.len(), 3); // dal, alif, ra
        let positions = determine_positions(&tokens, &glyphs);
        // dal: connects_next=false → can't connect to alif on its left
        // So dal gets isolated/final depending on ra before it
        // Reading order: د ا ر  (dal, alif, ra)
        // dal(0): prev=none, next=alif → dal.connects_next=false → Isolated
        // alif(1): prev=dal(connects_next=false) → not connected from prev,
        //          next=ra → alif.connects_next=false → Isolated
        // ra(2): prev=alif(connects_next=false) → not connected, next=none → Isolated
        assert!(matches!(positions[0], Position::Isolated));
        assert!(matches!(positions[1], Position::Isolated));
        assert!(matches!(positions[2], Position::Isolated));
    }

    #[test]
    fn test_connecting_word() {
        let (_, glyphs, unicode_map) = setup();
        // بسم — all connecting: ba(initial) sin(medial) mim(final)
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
        // ba ba space ba ba
        assert_eq!(tokens.len(), 5);
        let positions = determine_positions(&tokens, &glyphs);
        assert!(matches!(positions[0], Position::Initial));
        assert!(matches!(positions[1], Position::Final));
        assert!(matches!(positions[2], Position::Isolated)); // space
        assert!(matches!(positions[3], Position::Initial));
        assert!(matches!(positions[4], Position::Final));
    }

    #[test]
    fn test_normalization_alif_variants() {
        let (_, glyphs, unicode_map) = setup();
        // إ أ آ should all map to alif
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
        // بِسْمِ should tokenize as ba, sin, mim (diacritics stripped)
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
        // Test block character gets gold color
        let input = "█";
        let output = colorize_line(input);
        assert!(output.contains("\x1b[33m"));
        assert!(output.contains("\x1b[0m"));

        // Test connector characters get light yellow
        let input = "▄▀";
        let output = colorize_line(input);
        assert!(output.contains("\x1b[93m"));
        assert!(output.contains("\x1b[0m"));

        // Test dot gets cyan
        let input = "•";
        let output = colorize_line(input);
        assert!(output.contains("\x1b[36m"));
        assert!(output.contains("\x1b[0m"));

        // Test mixed content
        let input = "█ ▄•▀";
        let output = colorize_line(input);
        assert!(output.contains("\x1b[33m")); // block
        assert!(output.contains("\x1b[93m")); // connectors
        assert!(output.contains("\x1b[36m")); // dot
    }

    #[test]
    fn test_frame_output() {
        // Test empty lines
        let lines: Vec<String> = vec![];
        let framed = frame_output(&lines);
        assert!(framed.len() >= 2);
        assert!(framed[0].starts_with('╔'));
        assert!(framed.last().unwrap().starts_with('╚'));

        // Test single line
        let lines = vec!["test".to_string()];
        let framed = frame_output(&lines);
        assert_eq!(framed.len(), 3);
        assert!(framed[0].starts_with('╔'));
        assert!(framed[0].ends_with('╗'));
        assert!(framed[1].contains("test"));
        assert!(framed[1].starts_with('║'));
        assert!(framed[2].starts_with('╚'));

        // Test multi-line
        let lines = vec!["a".to_string(), "b".to_string()];
        let framed = frame_output(&lines);
        assert_eq!(framed.len(), 4);
    }
}

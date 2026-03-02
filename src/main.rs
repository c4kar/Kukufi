use kukufi::cli::{Cli, Commands};
use kukufi::models::Glyph;
use kukufi::renderer::{colorize_line, parse_art};
use kukufi::shaper::{build_unicode_map, load_glyphs};
use kukufi::tui::{interactive_mode, render_line};

use clap::Parser;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, BufRead};
use std::process;

fn print_lines(lines: &[String], color: bool) {
    for line in lines {
        if color {
            println!("{}", colorize_line(line));
        } else {
            println!("{}", line);
        }
    }
}

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

fn find_glyphs_file() -> String {
    if std::path::Path::new("assets/glyphs.toml").exists() {
        return "assets/glyphs.toml".to_string();
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("assets/glyphs.toml");
            if p.exists() {
                return p.to_string_lossy().to_string();
            }
        }
    }
    "assets/glyphs.toml".to_string()
}

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

            let w = rendered.lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
            widths.push(w);
        }

        if !height_ok {
            warnings += 1;
        }

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

fn print_glyph_list(glyphs: &HashMap<String, Glyph>) {
    let mut names: Vec<&String> = glyphs.keys().collect();
    names.sort();

    println!("{:<12} {:<6} {:<10} {:<10}", "Ad", "Harf", "Sağa bağ.", "Sola bağ.");
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

fn show_glyph(name: &str, glyphs: &HashMap<String, Glyph>, height: usize) {
    let glyph = match glyphs.get(name) {
        Some(g) => g,
        None => {
            eprintln!("'{}' adında glif bulunamadı.", name);
            eprintln!("Glif listesi için: kukufi list");
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

fn show_all_glyphs(glyphs: &HashMap<String, Glyph>, height: usize) {
    let mut names: Vec<&String> = glyphs.keys().collect();
    names.sort();

    for name in &names {
        show_glyph(name, glyphs, height);
        println!();
    }
}

fn main() {
    let cli = Cli::parse();

    let path = find_glyphs_file();
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|_| include_str!("../assets/glyphs.toml").to_string());

    let (height, glyphs) = load_glyphs(&content).unwrap_or_else(|e| {
        eprintln!("{}", e);
        process::exit(1);
    });
    let unicode_map = build_unicode_map(&glyphs);

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::List => {
                print_glyph_list(&glyphs);
            }
            Commands::Show { name } => {
                show_glyph(&name, &glyphs, height);
            }
            Commands::ShowAll => {
                show_all_glyphs(&glyphs, height);
            }
            Commands::Validate => {
                let (count, warnings) = validate_glyphs(&glyphs, height);
                println!("{} glif doğrulandı, {} uyarı", count, warnings);
            }
            Commands::Interactive => {
                interactive_mode(&unicode_map, &glyphs, height, cli.color, cli.frame, cli.spacing);
            }
        }
        return;
    }

    if let Some(text) = cli.text {
        if !text.is_empty() {
            let input = text.join(" ");
            let lines = render_line(&input, &unicode_map, &glyphs, height, cli.frame, cli.spacing);
            if !lines.is_empty() {
                if let Some(ref out_path) = cli.output {
                    let ansi = cli.color || out_path.ends_with(".ans");
                    write_to_file(&lines, out_path, ansi);
                }
                print_lines(&lines, cli.color);
            }
            return;
        }
    }

    // Stdin mode
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
                        render_line(trimmed, &unicode_map, &glyphs, height, cli.frame, cli.spacing);
                    print_lines(&rendered, cli.color);
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
        use clap::CommandFactory;
        let mut cmd = Cli::command();
        cmd.print_help().unwrap();
        process::exit(1);
    }

    if let Some(ref out_path) = cli.output {
        let ansi = cli.color || out_path.ends_with(".ans");
        write_to_file(&all_lines, out_path, ansi);
    }
}

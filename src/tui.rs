use crate::models::Glyph;
use crate::renderer::{colorize_line, frame_output, render};
use crate::shaper::{determine_positions, tokenize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::process;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, ClearType},
};

pub fn render_line(
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

pub fn interactive_mode(
    unicode_map: &HashMap<char, String>,
    glyphs: &HashMap<String, Glyph>,
    height: usize,
    color: bool,
    frame: bool,
    spacing: usize,
) {
    let mut stdout = io::stdout();

    terminal::enable_raw_mode().unwrap_or_else(|e| {
        eprintln!("Terminal raw modu etkinleştirilemedi: {}", e);
        process::exit(1);
    });

    execute!(stdout, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0)).ok();

    let header = "╔══════════════════════════════════════════════════╗";
    let title = "║  KuKufi Canlı Önizleme — Esc/Ctrl+C: Çıkış      ║";
    let footer = "╚══════════════════════════════════════════════════╝";

    execute!(stdout, cursor::MoveTo(0, 0)).ok();
    print!("\x1b[36m{}\r\n{}\r\n{}\x1b[0m\r\n", header, title, footer);
    print!("\r\n");
    stdout.flush().ok();

    let prompt_row: u16 = 4;
    let preview_start_row: u16 = 6;

    let mut input_buf = String::new();

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
            if let Ok(Event::Key(key_event)) = event::read() {
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
        }
    }

    terminal::disable_raw_mode().ok();
    execute!(stdout, cursor::MoveTo(0, preview_start_row + height as u16 + 4)).ok();
    println!("\r");
}

fn draw_prompt(stdout: &mut io::Stdout, row: u16, input: &str) {
    execute!(stdout, cursor::MoveTo(0, row)).ok();
    print!("\x1b[2K");
    print!("  \x1b[1;33mGiriş:\x1b[0m {}", input);
    stdout.flush().ok();
}

#[allow(clippy::too_many_arguments)]
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
    let clear_lines = height + 4;
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

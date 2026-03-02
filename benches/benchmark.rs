use criterion::{black_box, criterion_group, criterion_main, Criterion};
use kukufi::renderer::render;
use kukufi::shaper::{build_unicode_map, determine_positions, load_glyphs, tokenize};

fn render_benchmark(c: &mut Criterion) {
    let content = std::fs::read_to_string("assets/glyphs.toml").unwrap_or_default();
    if content.is_empty() {
        return;
    }

    let (height, glyphs) = load_glyphs(&content).unwrap();
    let unicode_map = build_unicode_map(&glyphs);
    let text = "بسم الله الرحمن الرحيم";

    c.bench_function("render basmala", |b| {
        b.iter(|| {
            let tokens = tokenize(black_box(text), &unicode_map, &glyphs);
            let positions = determine_positions(&tokens, &glyphs);
            render(&tokens, &positions, &glyphs, height, 0)
        })
    });
}

criterion_group!(benches, render_benchmark);
criterion_main!(benches);

use serde::Deserialize;

/// A single positional form of a glyph.
#[derive(Debug, Deserialize, Clone)]
pub struct GlyphForm {
    pub art: String,
}

/// A complete glyph entry with 4 positional forms and connection metadata.
#[derive(Debug, Deserialize, Clone)]
pub struct Glyph {
    pub unicode: String,
    pub connects_next: bool,
    pub connects_prev: bool,
    pub isolated: GlyphForm,
    pub initial: GlyphForm,
    pub medial: GlyphForm,
    #[serde(rename = "final")]
    pub final_form: GlyphForm,
}

/// Positional form of a letter determined by its neighbors.
pub enum Position {
    Isolated,
    Initial,
    Medial,
    Final,
}

/// A token in the processed input stream.
pub enum Token {
    Glyph(String), // glyph name key into the map
    Space,
}

/// Width-normalized, parsed art lines ready for concatenation.
pub struct RenderedGlyph {
    pub lines: Vec<String>,
}

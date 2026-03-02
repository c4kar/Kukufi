use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "kukufi")]
#[command(about = "Convert Arabic text into Kufi art ASCII text", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// The Arabic text to convert
    pub text: Option<Vec<String>>,

    /// Use ANSI colors in output
    #[arg(long)]
    pub color: bool,

    /// Wrap output in a Unicode frame
    #[arg(long)]
    pub frame: bool,

    /// Add N columns of spacing between glyphs
    #[arg(long, default_value_t = 0)]
    pub spacing: usize,

    /// Write output to file (.ans extension enables color)
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Interactive live preview mode
    Interactive,
    /// List all available glyphs
    List,
    /// Show all 4 positional forms of a specific glyph
    Show {
        /// Name of the glyph to show
        name: String,
    },
    /// Show all 4 positional forms of all glyphs
    ShowAll,
    /// Validate the glyphs definition file
    Validate,
}

use std::fmt;

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    InvalidInput(String),
    IoError(std::io::Error),
    ParseError(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::NotFound(msg) => write!(f, "Not found: {}", msg),
            AppError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            AppError::IoError(e) => write!(f, "IO error: {}", e),
            AppError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

pub type Result<T> = std::result::Result<T, AppError>;

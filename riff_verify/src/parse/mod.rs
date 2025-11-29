mod swift;

pub use swift::SwiftParser;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("failed to parse source: {message}")]
    SyntaxError { message: String },
    
    #[error("unsupported syntax: {description}")]
    UnsupportedSyntax { description: String },
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

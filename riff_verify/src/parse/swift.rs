use std::path::Path;
use std::sync::Arc;

use crate::ir::VerifyUnit;
use crate::source::SourceFile;
use super::ParseError;

pub struct SwiftParser {
    _private: (),
}

impl SwiftParser {
    pub fn new() -> Result<Self, ParseError> {
        Ok(Self { _private: () })
    }

    pub fn parse_file(&mut self, path: &Path) -> Result<Vec<VerifyUnit>, ParseError> {
        let content = std::fs::read_to_string(path)?;
        self.parse_source(path, &content)
    }

    pub fn parse_source(&mut self, path: &Path, source: &str) -> Result<Vec<VerifyUnit>, ParseError> {
        let source_file = Arc::new(SourceFile::new(path, source));
        self.extract_units(source_file)
    }

    fn extract_units(&self, _source_file: Arc<SourceFile>) -> Result<Vec<VerifyUnit>, ParseError> {
        Ok(vec![])
    }
}

impl Default for SwiftParser {
    fn default() -> Self {
        Self::new().expect("failed to create Swift parser")
    }
}

use std::path::Path;
use std::time::Instant;

use crate::analysis::EffectCollector;
use crate::contract::{ContractLoader, FfiContract};
use crate::parse::{LanguageParser, ParseError, SwiftParser};
use crate::report::VerificationResult;
use crate::rules::{RuleRegistry, Violation};

pub struct Verifier {
    parser: SwiftParser,
    rules: RuleRegistry,
    contract: Option<FfiContract>,
}

#[derive(Debug)]
pub enum VerifyError {
    Parse(ParseError),
    Io(std::io::Error),
}

impl From<ParseError> for VerifyError {
    fn from(err: ParseError) -> Self {
        Self::Parse(err)
    }
}

impl From<std::io::Error> for VerifyError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "parse error: {}", e),
            Self::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for VerifyError {}

impl Verifier {
    pub fn new() -> Result<Self, VerifyError> {
        Ok(Self {
            parser: SwiftParser::new()?,
            rules: RuleRegistry::with_defaults(),
            contract: None,
        })
    }

    pub fn with_rules(rules: RuleRegistry) -> Result<Self, VerifyError> {
        Ok(Self {
            parser: SwiftParser::new()?,
            rules,
            contract: None,
        })
    }

    pub fn with_contract(mut self, contract: FfiContract) -> Self {
        self.contract = Some(contract);
        self
    }

    pub fn with_auto_contract(mut self, source: &str, prefix: &str) -> Self {
        self.contract = Some(ContractLoader::from_swift_source(source, prefix));
        self
    }

    pub fn verify_file(&mut self, path: &Path) -> Result<VerificationResult, VerifyError> {
        let content = std::fs::read_to_string(path)?;
        self.verify_source(path, &content)
    }

    pub fn verify_source(&mut self, path: &Path, source: &str) -> Result<VerificationResult, VerifyError> {
        let start = Instant::now();
        
        let contract = self.contract
            .clone()
            .unwrap_or_else(|| ContractLoader::from_swift_source(source, "riff"));
        
        let units = self.parser.parse_source(path, source)?;
        
        let all_violations: Vec<Violation> = units
            .iter()
            .flat_map(|unit| {
                let trace = EffectCollector::collect(unit);
                self.rules.check_all_with_contract(&trace, &contract)
            })
            .collect();

        let duration = start.elapsed();

        if all_violations.is_empty() {
            Ok(VerificationResult::verified(
                units.len(),
                self.rules.rule_count(),
                duration,
            ))
        } else {
            Ok(VerificationResult::failed(all_violations, duration))
        }
    }

    pub fn verify_generated_swift(&mut self, swift_code: &str) -> Result<VerificationResult, VerifyError> {
        self.verify_source(Path::new("generated.swift"), swift_code)
    }
}

impl Default for Verifier {
    fn default() -> Self {
        Self::new().expect("failed to create verifier")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_balanced_alloc_free() {
        let source = r#"
public func test() {
    let ptr = UnsafeMutablePointer<Int32>.allocate(capacity: 10)
    defer { ptr.deallocate() }
}
"#;
        let mut verifier = Verifier::new().unwrap();
        let result = verifier.verify_generated_swift(source).unwrap();
        
        assert!(result.is_verified(), "Should verify: balanced alloc/free");
    }

    #[test]
    fn test_verify_detects_memory_leak() {
        let source = r#"
public func test() {
    let ptr = UnsafeMutablePointer<Int32>.allocate(capacity: 10)
}
"#;
        let mut verifier = Verifier::new().unwrap();
        let result = verifier.verify_generated_swift(source).unwrap();
        
        assert!(result.is_failed(), "Should detect memory leak");
        assert!(result.error_count() > 0);
    }

    #[test]
    fn test_verify_balanced_retain_release() {
        let source = r#"
public func test() {
    let obj = MyObject()
    let handle = Unmanaged.passRetained(obj).toOpaque()
    Unmanaged<MyObject>.fromOpaque(handle).release()
}
"#;
        let mut verifier = Verifier::new().unwrap();
        let result = verifier.verify_generated_swift(source).unwrap();
        
        assert!(result.is_verified(), "Should verify: balanced retain/release");
    }

    #[test]
    fn test_verify_multiple_functions() {
        let source = r#"
public func allocatesCorrectly() {
    let ptr = UnsafeMutablePointer<Int32>.allocate(capacity: 10)
    defer { ptr.deallocate() }
}

public func alsoCorrect() {
    let ptr = UnsafeMutablePointer<Double>.allocate(capacity: 5)
    defer { ptr.deallocate() }
}
"#;
        let mut verifier = Verifier::new().unwrap();
        let result = verifier.verify_generated_swift(source).unwrap();
        
        assert!(result.is_verified(), "Should verify both functions");
    }
}

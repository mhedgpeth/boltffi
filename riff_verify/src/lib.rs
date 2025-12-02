pub mod source;
pub mod ir;
pub mod parse;
pub mod analysis;
pub mod rules;
pub mod report;
pub mod verifier;

pub use source::{SourceFile, SourceSpan, SourcePosition, LineNumber, ColumnNumber, ByteOffset, ByteLength};
pub use ir::{VerifyUnit, UnitKind, Statement, Expression, VarId, VarName, VarIdGenerator};
pub use parse::{SwiftParser, ParseError};
pub use analysis::{Effect, EffectTrace, EffectEntry, EffectCollector, MemoryState, Capacity};
pub use rules::{Rule, RuleRegistry, Violation, ViolationKind, Severity};
pub use report::{VerificationResult, Reporter, OutputFormat};
pub use verifier::{Verifier, VerifyError};

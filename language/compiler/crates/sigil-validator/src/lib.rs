//! Validator for Sigil programming language
//!
//! Enforces Sigil's "ONE WAY" principle through canonical form validation.
//!
//! # Validation Rules
//!
//! 1. **No Duplicate Declarations**: Each name can only be declared once
//! 2. **No Accumulator Parameters**: Prevents tail-call optimization patterns
//! 3. **Canonical Pattern Matching**: Enforces direct, structural forms
//! 4. **Declaration Ordering**: Types before usage (optional check)

pub mod error;
pub mod canonical;

pub use error::*;
pub use canonical::*;

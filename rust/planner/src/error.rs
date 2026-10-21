//! `FactorioToolsException` (`src/FactorioTools/FactorioToolsException.cs`), its subclass
//! `NoPathBetweenTerminalsException` (`OilField/Models/NoPathBetweenTerminalsException.cs`),
//! and `Result` (`src/FactorioTools/Result.cs`).
//!
//! The C# core has no `try`/`catch`: an exception either ends the plan or travels inside a
//! `Result` until a caller inspects it. Rust's `Result<T, FactorioToolsError>` covers both.
//! Failures the C# raises as `NotSupportedException`, `InvalidCastException` or
//! `IndexOutOfRangeException` are programming errors, not planner outcomes, so the port
//! panics for those instead.

use std::fmt;

use crate::Location;

/// What kind of `FactorioToolsException` this is. The planner tests for
/// `NoPathBetweenTerminalsException` by type (`AddPipes.0.cs`), so it needs its own case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    General,
    /// `NoPathBetweenTerminalsException`, with its `LocationA` and `LocationB`.
    NoPathBetweenTerminals {
        a: Location,
        b: Location,
    },
}

/// `FactorioToolsException`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactorioToolsError {
    pub message: String,
    /// `BadInput`: the input blueprint or options are at fault, not the planner.
    pub bad_input: bool,
    pub kind: ErrorKind,
}

impl FactorioToolsError {
    /// `new FactorioToolsException(message)`.
    pub fn new(message: impl Into<String>) -> Self {
        FactorioToolsError {
            message: message.into(),
            bad_input: false,
            kind: ErrorKind::General,
        }
    }

    /// `new FactorioToolsException(message, badInput)`.
    pub fn with_bad_input(message: impl Into<String>, bad_input: bool) -> Self {
        FactorioToolsError {
            message: message.into(),
            bad_input,
            kind: ErrorKind::General,
        }
    }

    /// `new NoPathBetweenTerminalsException(a, b)`.
    pub fn no_path_between_terminals(a: Location, b: Location) -> Self {
        FactorioToolsError {
            message: "One pumpjack terminal has no path to another terminal. This is likely due to the oil field having an area completed surrounded by pumpjacks.".to_string(),
            bad_input: true,
            kind: ErrorKind::NoPathBetweenTerminals { a, b },
        }
    }

    /// `exception is NoPathBetweenTerminalsException`.
    pub fn is_no_path_between_terminals(&self) -> bool {
        matches!(self.kind, ErrorKind::NoPathBetweenTerminals { .. })
    }
}

impl fmt::Display for FactorioToolsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FactorioToolsError {}

/// `Result<T>`: either the data or the exception.
pub type FtResult<T> = Result<T, FactorioToolsError>;

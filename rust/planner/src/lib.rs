//! Rust port of the FactorioTools oil-field planner (#138), built stage by stage beside
//! the C# in `src/FactorioTools`. Each module names the C# file it copies. Wherever the C#
//! result depends on .NET's ordering or tie-breaking, the port uses `dotnet_compat`.

pub mod collection_ext;
pub mod containers;
pub mod data;
pub mod error;
pub mod grid;
pub mod location;

pub use error::{FactorioToolsError, FtResult};
pub use location::Location;

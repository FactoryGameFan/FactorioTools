//! Rust port of the FactorioTools oil-field planner (#138), from the C# that lived in
//! `src/FactorioTools` until stage 13b. Each module names the C# file it copies. Wherever the C#
//! result depends on .NET's ordering or tie-breaking, the port uses `dotnet_compat`.

pub mod algorithms;
pub mod collection_ext;
pub mod containers;
pub mod context;
pub mod data;
pub mod error;
pub mod grid;
pub mod helpers;
pub mod location;
pub mod models;
pub mod options;
pub mod planner;
pub mod providers;
pub mod quality;
pub mod steps;

pub use error::{FactorioToolsError, FtResult};
pub use location::Location;

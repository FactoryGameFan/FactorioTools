//! `InitializeFLUTE`, from `OilField/Steps/InitializeFLUTE.cs`: the one FLUTE instance the
//! planner shares, built from the degree 6 lookup table the first time it is needed.
//!
//! FLUTE is by Dr. Chris C. N. Chu, Iowa State University,
//! <http://home.engineering.iastate.edu/~cnchu/>, used under his Attribution Assurance
//! License (see the `flute` crate).
//!
//! The C# keeps it in a static property behind a lock; a `OnceLock` does the same. The
//! default build has no `ALLOW_DYNAMIC_FLUTE_DEGREE`, so only degree 6 is supported.

use std::sync::OnceLock;

use flute::{Flute, LookUpTable};

static FLUTE: OnceLock<Flute> = OnceLock::new();

/// `InitializeFLUTE.Execute(lutD)` followed by reading `InitializeFLUTE.FLUTE`.
///
/// Panics for any degree but 6, where the C# throws `ArgumentOutOfRangeException`.
pub fn execute(lut_d: i32) -> &'static Flute {
    if lut_d != 6 {
        panic!("ArgumentOutOfRangeException: Only lookup table of degree 6 is supported.");
    }

    FLUTE.get_or_init(|| Flute::new(LookUpTable::degree6()))
}

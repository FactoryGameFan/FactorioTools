# flute

FLUTE, a rectilinear Steiner minimal tree algorithm, ported to Rust for the oil-field planner (#138).

## Attribution

**FLUTE is by Dr. Chris C. N. Chu, Iowa State University, http://home.engineering.iastate.edu/~cnchu/.**

FLUTE's code and its lookup tables (`data/POWV6.dat`, `data/POST6.dat`) are under Dr. Chu's Attribution Assurance License, in `LICENSE-FLUTE`. Its terms, in short:

- Each time a program built from this code launches, it must prominently show the three lines of attribution above. The planner's site shows them in its footer (`src/vue/src/App.vue`).
- Anyone who intends to use the code commercially must tell the author before doing so.
- The author's name may not be used to promote products built from it.

Read `LICENSE-FLUTE` for the exact wording. These terms apply to this crate as a derivative work, on top of anything else.

This crate is a port of [FluteSharp](https://github.com/joelverhagen/FluteSharp) by Joel Verhagen, which is MIT-licensed (`LICENSE-FluteSharp`) and itself ports FLUTE 3.1's C. FluteSharp's lookup table reader, and so this one, comes from Stefan Mücke's Java port, `Flute.java`.

## What it covers

Only what the planner uses: the degree 6 lookup table and the default accuracy of 3, through `Flute::new(LookUpTable::degree6()).execute(&points)`. The reader accepts the other degrees' files too, but only degree 6 is embedded and checked.

## How it is checked

`oracle/` records FluteSharp's trees for 1,346 inputs, and `tests/fixtures.rs` checks this port gives the same tree, branch for branch, for every one. See `FIXTURES.md`.

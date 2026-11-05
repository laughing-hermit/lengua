//! lengua's calls into the operating system, kept apart from the rest so that the lengua crate can
//! forbid unsafe code outright: any call that needs it is wrapped here in a safe function, with a
//! test on the runner of the system it is for.
//!
//! Nothing is here yet but the spikes in `tests/spikes.rs`: round 1 of the roadmap in todo.md
//! asks the real macOS and Windows runners what the toolchain and the systems do, before the
//! edge's design (round 2) relies on it. Their answers are recorded in ROADMAP.md.

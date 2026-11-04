//! Tables with one cell per target, so that the targets stay one product.
//!
//! A test, a limit or a fact that holds on every target is written once, as a [`PerTarget`]: a
//! struct literal with a field for each target, so adding a target to lengua breaks every such
//! table at compile time until the new target's cell is filled. A cell that doesn't apply to a
//! target says why in its own type (a `Result` or an enum with a reason), never by being left out.
//!
//! To add a target: a variant of [`Profile`] (every `match` on it stops compiling until it
//! is handled), a field here (every table stops compiling until it has a cell), a native runner
//! row in `.github/workflows`, and the conformance tests then run on it by themselves.

use crate::platform::Profile;

/// One value for each target lengua exists on.
#[derive(Debug, Clone, Copy)]
pub struct PerTarget<T> {
    /// Linux on x86-64.
    pub linux_x86_64: T,
    /// Linux on ARM64 (AArch64).
    pub linux_aarch64: T,
}

impl<T> PerTarget<T> {
    /// The cell of `target`.
    #[must_use]
    pub const fn get(&self, target: Profile) -> &T {
        match target {
            Profile::LinuxX86_64 => &self.linux_x86_64,
            Profile::LinuxAarch64 => &self.linux_aarch64,
        }
    }

    /// Every target with its cell, in the order of [`Profile::ALL`].
    pub fn cells(&self) -> impl Iterator<Item = (Profile, &T)> {
        Profile::ALL
            .into_iter()
            .map(move |target| (target, self.get(target)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_has_a_cell_for_every_target() {
        let table = PerTarget {
            linux_x86_64: "x86-64",
            linux_aarch64: "arm64",
        };
        let names: Vec<(&str, &str)> = table
            .cells()
            .map(|(target, cell)| (target.name(), *cell))
            .collect();
        assert_eq!(names, [("linux-x86_64", "x86-64"), ("linux-aarch64", "arm64")]);
        assert_eq!(Profile::ALL.len(), 2);
    }
}

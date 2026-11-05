//! Tables with one cell per target, so that the targets stay one product.
//!
//! A test, a limit or a fact that holds on every target is written once, as a table with a cell
//! for each of them: a [`PerCpu`] when it depends on the CPU alone, which is nearly everything a
//! test holds (a program's assembly, a count of its instructions, the edit that plants a bug), so
//! it is written once however many systems the CPU runs; a [`PerTarget`] when it depends on the
//! system too (the label of a runner, a name a system reports). A table is a struct literal with
//! a field for each CPU or target, so adding one to lengua breaks every such table at compile time
//! until the new cell is filled. A cell that doesn't apply says why in its own type (a `Result` or
//! an enum with a reason), never by being left out.
//!
//! To add a target: a variant of [`Profile`] (every `match` on it stops compiling until it
//! is handled), a field of [`PerTarget`] (its tables stop compiling until they have a cell), a
//! native runner row in `scripts/ci/targets.json`, and the conformance tests then run on it by
//! themselves, every [`PerCpu`] table giving it its CPU's cell.

use crate::isa::Isa;
use crate::platform::Profile;

/// One value for each CPU lengua generates code for.
#[derive(Debug, Clone, Copy)]
pub struct PerCpu<T> {
    /// x86-64.
    pub x86_64: T,
    /// ARM64 (AArch64).
    pub aarch64: T,
}

impl<T> PerCpu<T> {
    /// The cell of `isa`.
    #[must_use]
    pub const fn of(&self, isa: Isa) -> &T {
        match isa {
            Isa::X86_64 => &self.x86_64,
            Isa::Aarch64 => &self.aarch64,
        }
    }

    /// The cell of `target`'s CPU.
    #[must_use]
    pub const fn get(&self, target: Profile) -> &T {
        self.of(target.isa())
    }

    /// Every target with its CPU's cell, in the order of [`Profile::ALL`].
    pub fn cells(&self) -> impl Iterator<Item = (Profile, &T)> {
        Profile::ALL
            .into_iter()
            .map(move |target| (target, self.get(target)))
    }
}

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
        assert_eq!(
            names,
            [("linux-x86_64", "x86-64"), ("linux-aarch64", "arm64")]
        );
        assert_eq!(Profile::ALL.len(), 2);
    }

    /// A CPU's table gives each target its CPU's cell, so a system added to a CPU needs no cell.
    #[test]
    fn a_cpus_cell_serves_every_target_of_that_cpu() {
        let table = PerCpu {
            x86_64: "x86-64",
            aarch64: "arm64",
        };
        for (target, cell) in table.cells() {
            assert_eq!(*cell, *table.of(target.isa()), "{}", target.name());
        }
        assert_eq!(*table.get(Profile::LinuxX86_64), "x86-64");
        assert_eq!(*table.get(Profile::LinuxAarch64), "arm64");
    }
}

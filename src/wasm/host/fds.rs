//! The descriptors a program holds, each number fixed by its role: 0, 1 and 2 standard input,
//! output and error; 3 the working folder, a preopen named `.`, only when the idea names files;
//! 4 the listener, for a server; 5 and up the connections lengua made, in the idea's order; then
//! the UDP port the idea receives datagrams on, when it names one. What the program opens or
//! accepts takes the lowest number free after those. A number in no use gives `BADF`.

use std::collections::BTreeMap;

use super::files::{Folder, Open};
use super::net::{Listener, Socket};
use super::open::Datagrams;
use crate::wasm::wasi::errno;

/// Standard input's descriptor.
pub const STDIN: u32 = 0;
/// Standard output's.
pub const STDOUT: u32 = 1;
/// Standard error's.
pub const STDERR: u32 = 2;
/// The working folder's, a preopen named `.`.
pub const FOLDER: u32 = 3;
/// The listener's, for a server.
pub const LISTENER: u32 = 4;
/// The first connection's.
pub const FIRST_CONNECTION: u32 = 5;

/// What a descriptor is.
pub enum Descriptor {
    /// Standard input.
    Stdin,
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
    /// A folder: the working folder, or one beneath it the program opened.
    Folder(Folder),
    /// A file the program opened.
    File(Box<Open>),
    /// The listener a server's connections come in on.
    Listener(Box<Listener>),
    /// A connection: one lengua made, one the program accepted, or one it opened itself.
    Socket(Box<Socket>),
    /// A socket for UDP datagrams: the port the idea receives on, which lengua bound, or one the
    /// program opened to a computer its idea grants.
    Datagram(Box<Datagrams>),
}

/// The descriptors a program holds.
pub struct Table {
    /// Each descriptor, by its number.
    slots: BTreeMap<u32, Descriptor>,
    /// The lowest number a descriptor the program opens may take.
    first: u32,
}

impl Table {
    /// A table with standard input, output and error, whose next descriptors start at `first`.
    #[must_use]
    pub fn new(first: u32) -> Self {
        let slots = BTreeMap::from([
            (STDIN, Descriptor::Stdin),
            (STDOUT, Descriptor::Stdout),
            (STDERR, Descriptor::Stderr),
        ]);
        Self { slots, first }
    }

    /// Puts `descriptor` at `fd`, for the ones whose numbers are fixed.
    pub fn put(&mut self, fd: u32, descriptor: Descriptor) {
        self.slots.insert(fd, descriptor);
    }

    /// Adds `descriptor` at the lowest free number from the first a program's may take.
    ///
    /// # Errors
    ///
    /// `MFILE` when the program holds as many as every system lets it.
    pub fn add(&mut self, descriptor: Descriptor) -> Result<u32, u16> {
        if u64::try_from(self.slots.len()).unwrap_or(u64::MAX) >= crate::runner::OPEN_FILES {
            return Err(errno::MFILE);
        }
        let mut fd = self.first;
        while self.slots.contains_key(&fd) {
            fd = fd.checked_add(1).ok_or(errno::MFILE)?;
        }
        self.slots.insert(fd, descriptor);
        Ok(fd)
    }

    /// The descriptor `fd`, if it's open.
    #[must_use]
    pub fn get(&self, fd: u32) -> Option<&Descriptor> {
        self.slots.get(&fd)
    }

    /// The descriptor `fd`, to change, if it's open.
    pub fn get_mut(&mut self, fd: u32) -> Option<&mut Descriptor> {
        self.slots.get_mut(&fd)
    }

    /// Closes `fd`, giving back what it was.
    pub fn remove(&mut self, fd: u32) -> Option<Descriptor> {
        self.slots.remove(&fd)
    }

    /// Every open descriptor, by number.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &Descriptor)> {
        self.slots.iter().map(|(fd, descriptor)| (*fd, descriptor))
    }

    /// How many are open.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether none is.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_descriptor_takes_the_lowest_free_number_after_the_fixed_ones() {
        let mut table = Table::new(FIRST_CONNECTION + 2);
        assert_eq!(table.len(), 3);
        assert!(matches!(table.get(STDIN), Some(Descriptor::Stdin)));
        assert!(
            table.get(FOLDER).is_none(),
            "no folder unless the idea names files"
        );
        assert_eq!(table.add(Descriptor::Stdout), Ok(7));
        assert_eq!(table.add(Descriptor::Stdout), Ok(8));
        assert!(table.remove(7).is_some());
        assert_eq!(table.add(Descriptor::Stderr), Ok(7));
        assert!(table.remove(STDIN).is_some());
        assert!(table.get(STDIN).is_none());
        assert!(!table.is_empty());
    }

    #[test]
    fn a_program_holds_as_many_descriptors_as_every_system_lets_it() {
        let mut table = Table::new(FIRST_CONNECTION);
        let most = usize::try_from(crate::runner::OPEN_FILES).expect("a count");
        while table.len() < most {
            table.add(Descriptor::Stdout).expect("room");
        }
        assert_eq!(table.add(Descriptor::Stdout), Err(errno::MFILE));
    }
}

//! x86-64 machine code, read with `iced-x86`: which instructions of a stretch of code branch on a
//! condition, jump or call, divide, or take the address of data in the code.

use iced_x86::{Decoder, DecoderOptions, FlowControl, Instruction, Mnemonic};

use super::{Made, Read, Seen};

/// What `instruction` is, when the rule has a word for it.
fn made(instruction: &Instruction) -> Option<Made> {
    if instruction.is_invalid() {
        return Some(Made::Unread);
    }
    match instruction.flow_control() {
        // `jcc`, `jrcxz`, `loop` and its kin.
        FlowControl::ConditionalBranch => return Some(Made::Branch),
        FlowControl::IndirectBranch => return Some(Made::IndirectJump),
        FlowControl::Call => return Some(Made::Call),
        FlowControl::IndirectCall => return Some(Made::IndirectCall),
        // A transaction's start, which goes one of two ways: the engine makes none.
        FlowControl::XbeginXabortXend => return Some(Made::Unread),
        _ => {}
    }
    match instruction.mnemonic() {
        Mnemonic::Div
        | Mnemonic::Idiv
        | Mnemonic::Divss
        | Mnemonic::Divsd
        | Mnemonic::Divps
        | Mnemonic::Divpd
        | Mnemonic::Vdivss
        | Mnemonic::Vdivsd
        | Mnemonic::Vdivps
        | Mnemonic::Vdivpd
        | Mnemonic::Sqrtss
        | Mnemonic::Sqrtsd
        | Mnemonic::Sqrtps
        | Mnemonic::Sqrtpd
        | Mnemonic::Vsqrtss
        | Mnemonic::Vsqrtsd
        | Mnemonic::Vsqrtps
        | Mnemonic::Vsqrtpd => Some(Made::Division),
        // The address of data in the code, in a register: a table's first byte, which an index
        // is then added to. A load of a constant at such an address, with no index, isn't this.
        Mnemonic::Lea if instruction.is_ip_rel_memory_operand() => Some(Made::Table),
        _ => None,
    }
}

/// Reads `code`, which starts at an instruction's first byte, `at` bytes into the engine's
/// code: how many instructions it holds, and each the rule has a word for. It stops at the
/// first bytes that are no instruction, since nothing says where the next one starts.
pub fn read(code: &[u8], at: usize) -> Read {
    let mut decoder = Decoder::new(64, code, DecoderOptions::NONE);
    let mut read = Read::default();
    while decoder.can_decode() {
        let position = decoder.position();
        let instruction = decoder.decode();
        read.instructions += 1;
        if let Some(made) = made(&instruction) {
            read.seen.push(Seen {
                at: at.saturating_add(position),
                made,
                name: format!("{:?}", instruction.mnemonic()).to_lowercase(),
            });
        }
        if instruction.is_invalid() {
            break;
        }
    }
    read
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the rule sees in `code`.
    fn seen(code: &[u8]) -> Vec<(usize, Made, String)> {
        read(code, 0)
            .seen
            .into_iter()
            .map(|seen| (seen.at, seen.made, seen.name))
            .collect()
    }

    /// Each kind of instruction the rule has a word for is read as that kind, from its bytes.
    #[test]
    fn each_kind_is_read_from_its_bytes() {
        let kinds: [(&[u8], Made, &str); 11] = [
            // jb +0
            (&[0x72, 0x00], Made::Branch, "jb"),
            // je +0, with a 32-bit distance
            (&[0x0F, 0x84, 0x00, 0x00, 0x00, 0x00], Made::Branch, "je"),
            // jrcxz +0
            (&[0xE3, 0x00], Made::Branch, "jrcxz"),
            // jmp rax
            (&[0xFF, 0xE0], Made::IndirectJump, "jmp"),
            // call +0
            (&[0xE8, 0x00, 0x00, 0x00, 0x00], Made::Call, "call"),
            // call rax
            (&[0xFF, 0xD0], Made::IndirectCall, "call"),
            // div ecx
            (&[0xF7, 0xF1], Made::Division, "div"),
            // idiv rcx
            (&[0x48, 0xF7, 0xF9], Made::Division, "idiv"),
            // divsd xmm0, xmm1
            (&[0xF2, 0x0F, 0x5E, 0xC1], Made::Division, "divsd"),
            // lea rax, [rip+0]
            (
                &[0x48, 0x8D, 0x05, 0x00, 0x00, 0x00, 0x00],
                Made::Table,
                "lea",
            ),
            // push es, which 64-bit code doesn't have
            (&[0x06], Made::Unread, "invalid"),
        ];
        for (code, made, name) in kinds {
            assert_eq!(seen(code), [(0, made, name.to_owned())], "{code:02x?}");
        }
    }

    /// Straight code has nothing the rule has a word for: arithmetic, a conditional move, a
    /// load through registers, a constant loaded from the code with no index, an address made
    /// from registers, a jump that no condition decides, and a return.
    #[test]
    fn straight_code_has_nothing_to_refuse() {
        let code: &[u8] = &[
            0x31, 0xC8, // xor eax, ecx
            0x0F, 0x42, 0xC1, // cmovb eax, ecx
            0x03, 0x04, 0x37, // add eax, [rdi+rsi]
            0xF3, 0x0F, 0x6F, 0x05, 0x00, 0x00, 0x00, 0x00, // movdqu xmm0, [rip+0]
            0x48, 0x8D, 0x04, 0x11, // lea rax, [rcx+rdx]
            0x0F, 0xAF, 0xC1, // imul eax, ecx
            0xEB, 0x00, // jmp +0
            0xC3, // ret
        ];
        let read = read(code, 0);
        assert_eq!(read.instructions, 8);
        assert!(read.seen.is_empty(), "{:?}", read.seen);
    }

    /// An instruction is found where it is, counted from where the code starts in the engine's;
    /// and reading stops at bytes that are no instruction.
    #[test]
    fn an_instruction_is_found_at_its_place() {
        // xor eax, ecx; jb +0
        assert_eq!(
            read(&[0x31, 0xC8, 0x72, 0x00], 100)
                .seen
                .iter()
                .map(|seen| seen.at)
                .collect::<Vec<_>>(),
            [102]
        );
        // push es; jb +0: the jump after what can't be read isn't looked for.
        let after = read(&[0x06, 0x72, 0x00], 0);
        assert_eq!(after.seen.len(), 1);
        assert_eq!(after.seen[0].made, Made::Unread);
    }
}

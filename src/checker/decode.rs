//! Reads the linked executable back and matches it to main.s instruction by instruction,
//! operands included, so the step from main.s to machine code is a checked translation.

use std::collections::BTreeMap;

use iced_x86::{
    CpuidFeature, Decoder, DecoderOptions, EncodingKind, FlowControl, Formatter, Instruction,
    IntelFormatter, MemorySize, OpKind, Register, RoundingControl,
};
use object::{Object, ObjectSection, ObjectSymbol, SectionKind, SymbolKind};

use super::source::{
    Function as SourceFunction, Instruction as SourceInstruction, Item, Label, Memory, Operand,
    Source,
};
use super::{Rule, Violation};
use crate::platform::CpuFeatures;

/// A function in the executable.
#[derive(Debug, Clone)]
pub struct MachineFunction {
    /// Its address.
    pub address: u64,
    /// Its size in bytes.
    pub size: u64,
    /// Its decoded instructions.
    pub instructions: Vec<Instruction>,
}

/// A data symbol in the executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineData {
    /// Its address.
    pub address: u64,
    /// Its size in bytes.
    pub size: u64,
    /// Whether its section is writable.
    pub writable: bool,
    /// Its initial bytes (empty for zero-filled data).
    pub bytes: Vec<u8>,
}

/// The executable's functions and data, by name.
#[derive(Debug, Clone, Default)]
pub struct Machine {
    /// Functions.
    pub functions: BTreeMap<String, MachineFunction>,
    /// Data symbols.
    pub data: BTreeMap<String, MachineData>,
}

/// Reads the functions and data symbols of a linked ELF executable and decodes the functions.
///
/// # Errors
///
/// Fails when the file isn't the kind of executable lengua builds.
pub fn read(executable: &[u8]) -> Result<Machine, String> {
    let file = object::File::parse(executable)
        .map_err(|error| format!("can't read the executable: {error}"))?;
    let mut machine = Machine::default();
    let mut text_ranges = Vec::new();
    for symbol in file.symbols() {
        let Ok(name) = symbol.name() else { continue };
        if name.is_empty() || symbol.size() == 0 {
            continue;
        }
        let Some(section) = symbol
            .section_index()
            .and_then(|index| file.section_by_index(index).ok())
        else {
            continue;
        };
        let start = symbol
            .address()
            .checked_sub(section.address())
            .ok_or("a symbol lies outside its section")?;
        let data = section
            .data()
            .map_err(|error| format!("can't read section data: {error}"))?;
        let bytes = usize::try_from(start)
            .ok()
            .zip(usize::try_from(symbol.size()).ok())
            .and_then(|(start, size)| data.get(start..start.checked_add(size)?))
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        match symbol.kind() {
            SymbolKind::Text => {
                text_ranges.push((symbol.address(), symbol.size(), section.size()));
                let mut decoder =
                    Decoder::with_ip(64, &bytes, symbol.address(), DecoderOptions::NONE);
                let mut instructions = Vec::new();
                while decoder.can_decode() {
                    let instruction = decoder.decode();
                    if instruction.is_invalid() {
                        return Err(format!(
                            "{name} holds bytes that aren't an instruction at {:#x}",
                            instruction.ip()
                        ));
                    }
                    instructions.push(instruction);
                }
                machine.functions.insert(
                    name.to_owned(),
                    MachineFunction {
                        address: symbol.address(),
                        size: symbol.size(),
                        instructions,
                    },
                );
            }
            SymbolKind::Data => {
                let writable = matches!(
                    section.kind(),
                    SectionKind::Data | SectionKind::UninitializedData
                );
                machine.data.insert(
                    name.to_owned(),
                    MachineData {
                        address: symbol.address(),
                        size: symbol.size(),
                        writable,
                        bytes,
                    },
                );
            }
            _ => {}
        }
    }
    // Every byte of code must belong to a function, so nothing runs that wasn't checked.
    let covered: u64 = text_ranges.iter().map(|(_, size, _)| size).sum();
    let section_size = text_ranges.first().map_or(0, |(_, _, size)| *size);
    if covered != section_size {
        return Err(format!(
            "the code section holds {section_size} bytes but the functions cover {covered}"
        ));
    }
    Ok(machine)
}

/// One main.s instruction and the machine instruction it became.
#[derive(Debug, Clone)]
pub struct Step<'a> {
    /// The source line.
    pub source: &'a SourceInstruction,
    /// The decoded instruction.
    pub machine: Instruction,
}

/// A local label and the address it names.
#[derive(Debug, Clone)]
pub struct LabelAt<'a> {
    /// The label.
    pub label: &'a Label,
    /// The index of the step that follows it.
    pub index: usize,
    /// Its address.
    pub address: u64,
}

/// A function with its source and machine instructions matched up.
#[derive(Debug, Clone)]
pub struct MappedFunction<'a> {
    /// The source function.
    pub source: &'a SourceFunction,
    /// Its first address.
    pub address: u64,
    /// The address just past its end.
    pub end: u64,
    /// The instructions, in order.
    pub steps: Vec<Step<'a>>,
    /// The local labels, in order.
    pub labels: Vec<LabelAt<'a>>,
}

impl MappedFunction<'_> {
    /// The index of the instruction at `address`.
    #[must_use]
    pub fn index_of(&self, address: u64) -> Option<usize> {
        self.steps
            .binary_search_by_key(&address, |step| step.machine.ip())
            .ok()
    }

    /// The name of the function.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.source.name
    }

    /// The label right before the instruction at `index`, if any.
    #[must_use]
    pub fn label_before(&self, index: usize) -> Option<&LabelAt<'_>> {
        self.labels.iter().find(|label| label.index == index)
    }
}

/// main.s and the executable, matched up.
#[derive(Debug, Clone)]
pub struct Program<'a> {
    /// The parsed source.
    pub source: &'a Source,
    /// The functions, in source order.
    pub functions: Vec<MappedFunction<'a>>,
    /// Data symbols by name.
    pub data: BTreeMap<String, MachineData>,
}

impl Program<'_> {
    /// A function by name.
    #[must_use]
    pub fn function(&self, name: &str) -> Option<&MappedFunction<'_>> {
        self.functions
            .iter()
            .find(|function| function.name() == name)
    }

    /// The function starting at `address`.
    #[must_use]
    pub fn function_at(&self, address: u64) -> Option<&MappedFunction<'_>> {
        self.functions
            .iter()
            .find(|function| function.address == address)
    }

    /// The data symbol containing `address`.
    #[must_use]
    pub fn data_at(&self, address: u64) -> Option<(&str, &MachineData)> {
        self.data
            .iter()
            .find(|(_, data)| address >= data.address && address < data.address + data.size)
            .map(|(name, data)| (name.as_str(), data))
    }
}

/// Integer instructions and `syscall`, the part of the dialect that dates from dialect 1.
const ALLOWED: &[&str] = &[
    "mov", "movzx", "movsx", "movsxd", "lea", "push", "pop", "xchg", "bswap", "movbe", "cmovo",
    "cmovno", "cmovb", "cmovae", "cmove", "cmovne", "cmovbe", "cmova", "cmovs", "cmovns", "cmovp",
    "cmovnp", "cmovl", "cmovge", "cmovle", "cmovg", "add", "adc", "sub", "sbb", "neg", "inc",
    "dec", "mul", "imul", "div", "idiv", "cbw", "cwde", "cdqe", "cwd", "cdq", "cqo", "adcx",
    "adox", "mulx", "and", "or", "xor", "not", "test", "cmp", "shl", "shr", "sar", "rol", "ror",
    "shld", "shrd", "bt", "bts", "btr", "btc", "bsf", "bsr", "seto", "setno", "setb", "setae",
    "sete", "setne", "setbe", "seta", "sets", "setns", "setp", "setnp", "setl", "setge", "setle",
    "setg", "andn", "bextr", "blsi", "blsmsk", "blsr", "tzcnt", "bzhi", "pdep", "pext", "rorx",
    "sarx", "shlx", "shrx", "lzcnt", "popcnt", "jmp", "jo", "jno", "jb", "jae", "je", "jne", "jbe",
    "ja", "js", "jns", "jp", "jnp", "jl", "jge", "jle", "jg", "call", "ret", "syscall", "nop",
];

/// Integer vector instructions, added in dialect 2: SSE2 through SSE4.2, AVX, AVX2 and AVX-512,
/// each usable only when this CPU has it, with AVX-512 write masks, zeroing and broadcasts. Left
/// out on purpose: aligned and non-temporal moves (`movdqa`, `movntdq`), the AVX2 masked moves
/// (`vpmaskmovd`), gathers, scatters and MMX.
const VECTOR: &[&str] = &[
    // Unaligned loads and stores, and moves between general-purpose and vector registers.
    "movdqu",
    "vmovdqu",
    "vmovdqu8",
    "vmovdqu16",
    "vmovdqu32",
    "vmovdqu64",
    "lddqu",
    "vlddqu",
    "movd",
    "vmovd",
    "movq",
    "vmovq",
    // Arithmetic.
    "paddb",
    "paddw",
    "paddd",
    "paddq",
    "paddsb",
    "paddsw",
    "paddusb",
    "paddusw",
    "psubb",
    "psubw",
    "psubd",
    "psubq",
    "psubsb",
    "psubsw",
    "psubusb",
    "psubusw",
    "pmullw",
    "pmulhw",
    "pmulhuw",
    "pmulld",
    "pmuludq",
    "pmuldq",
    "pmaddwd",
    "pmaddubsw",
    "pmulhrsw",
    "psadbw",
    "pavgb",
    "pavgw",
    "pabsb",
    "pabsw",
    "pabsd",
    "psignb",
    "psignw",
    "psignd",
    "pminub",
    "pminuw",
    "pminud",
    "pminsb",
    "pminsw",
    "pminsd",
    "pmaxub",
    "pmaxuw",
    "pmaxud",
    "pmaxsb",
    "pmaxsw",
    "pmaxsd",
    "phaddw",
    "phaddd",
    "phaddsw",
    "phsubw",
    "phsubd",
    "phsubsw",
    "phminposuw",
    "mpsadbw",
    "vpaddb",
    "vpaddw",
    "vpaddd",
    "vpaddq",
    "vpaddsb",
    "vpaddsw",
    "vpaddusb",
    "vpaddusw",
    "vpsubb",
    "vpsubw",
    "vpsubd",
    "vpsubq",
    "vpsubsb",
    "vpsubsw",
    "vpsubusb",
    "vpsubusw",
    "vpmullw",
    "vpmulhw",
    "vpmulhuw",
    "vpmulld",
    "vpmullq",
    "vpmuludq",
    "vpmuldq",
    "vpmaddwd",
    "vpmaddubsw",
    "vpmulhrsw",
    "vpsadbw",
    "vdbpsadbw",
    "vpavgb",
    "vpavgw",
    "vpabsb",
    "vpabsw",
    "vpabsd",
    "vpabsq",
    "vpsignb",
    "vpsignw",
    "vpsignd",
    "vpminub",
    "vpminuw",
    "vpminud",
    "vpminuq",
    "vpminsb",
    "vpminsw",
    "vpminsd",
    "vpminsq",
    "vpmaxub",
    "vpmaxuw",
    "vpmaxud",
    "vpmaxuq",
    "vpmaxsb",
    "vpmaxsw",
    "vpmaxsd",
    "vpmaxsq",
    "vphaddw",
    "vphaddd",
    "vphaddsw",
    "vphsubw",
    "vphsubd",
    "vphsubsw",
    "vphminposuw",
    "vmpsadbw",
    // Logic.
    "pand",
    "pandn",
    "por",
    "pxor",
    "vpand",
    "vpandn",
    "vpor",
    "vpxor",
    "vpandd",
    "vpandq",
    "vpandnd",
    "vpandnq",
    "vpord",
    "vporq",
    "vpxord",
    "vpxorq",
    "vpternlogd",
    "vpternlogq",
    // Compares, into vector registers, mask registers or flags.
    "pcmpeqb",
    "pcmpeqw",
    "pcmpeqd",
    "pcmpeqq",
    "pcmpgtb",
    "pcmpgtw",
    "pcmpgtd",
    "pcmpgtq",
    "vpcmpeqb",
    "vpcmpeqw",
    "vpcmpeqd",
    "vpcmpeqq",
    "vpcmpgtb",
    "vpcmpgtw",
    "vpcmpgtd",
    "vpcmpgtq",
    "vpcmpb",
    "vpcmpub",
    "vpcmpw",
    "vpcmpuw",
    "vpcmpd",
    "vpcmpud",
    "vpcmpq",
    "vpcmpuq",
    "vptestmb",
    "vptestmw",
    "vptestmd",
    "vptestmq",
    "vptestnmb",
    "vptestnmw",
    "vptestnmd",
    "vptestnmq",
    "ptest",
    "vptest",
    "pcmpestri",
    "pcmpestri64",
    "pcmpestrm",
    "pcmpestrm64",
    "pcmpistri",
    "pcmpistrm",
    "vpcmpestri",
    "vpcmpestri64",
    "vpcmpestrm",
    "vpcmpestrm64",
    "vpcmpistri",
    "vpcmpistrm",
    // Shifts and rotates.
    "psllw",
    "pslld",
    "psllq",
    "psrlw",
    "psrld",
    "psrlq",
    "psraw",
    "psrad",
    "pslldq",
    "psrldq",
    "vpsllw",
    "vpslld",
    "vpsllq",
    "vpsrlw",
    "vpsrld",
    "vpsrlq",
    "vpsraw",
    "vpsrad",
    "vpsraq",
    "vpslldq",
    "vpsrldq",
    "vpsllvw",
    "vpsllvd",
    "vpsllvq",
    "vpsrlvw",
    "vpsrlvd",
    "vpsrlvq",
    "vpsravw",
    "vpsravd",
    "vpsravq",
    "vprold",
    "vprolq",
    "vprord",
    "vprorq",
    "vprolvd",
    "vprolvq",
    "vprorvd",
    "vprorvq",
    "vpshldw",
    "vpshldd",
    "vpshldq",
    "vpshrdw",
    "vpshrdd",
    "vpshrdq",
    "vpshldvw",
    "vpshldvd",
    "vpshldvq",
    "vpshrdvw",
    "vpshrdvd",
    "vpshrdvq",
    // Shuffles, permutes, packs, unpacks and blends.
    "pshufb",
    "pshufd",
    "pshufhw",
    "pshuflw",
    "palignr",
    "punpcklbw",
    "punpcklwd",
    "punpckldq",
    "punpcklqdq",
    "punpckhbw",
    "punpckhwd",
    "punpckhdq",
    "punpckhqdq",
    "packsswb",
    "packssdw",
    "packuswb",
    "packusdw",
    "pblendw",
    "pblendvb",
    "vpshufb",
    "vpshufd",
    "vpshufhw",
    "vpshuflw",
    "vpalignr",
    "vpunpcklbw",
    "vpunpcklwd",
    "vpunpckldq",
    "vpunpcklqdq",
    "vpunpckhbw",
    "vpunpckhwd",
    "vpunpckhdq",
    "vpunpckhqdq",
    "vpacksswb",
    "vpackssdw",
    "vpackuswb",
    "vpackusdw",
    "vpblendw",
    "vpblendvb",
    "vpblendd",
    "vpermb",
    "vpermw",
    "vpermd",
    "vpermq",
    "vpblendmb",
    "vpblendmw",
    "vpblendmd",
    "vpblendmq",
    "vpcompressb",
    "vpcompressw",
    "vpcompressd",
    "vpcompressq",
    "vpexpandb",
    "vpexpandw",
    "vpexpandd",
    "vpexpandq",
    "vperm2i128",
    "vpermi2b",
    "vpermi2w",
    "vpermi2d",
    "vpermi2q",
    "vpermt2b",
    "vpermt2w",
    "vpermt2d",
    "vpermt2q",
    "valignd",
    "valignq",
    "vpmultishiftqb",
    "vpshufbitqmb",
    "vinserti128",
    "vinserti32x4",
    "vinserti64x2",
    "vinserti32x8",
    "vinserti64x4",
    "vextracti128",
    "vextracti32x4",
    "vextracti64x2",
    "vextracti32x8",
    "vextracti64x4",
    "vpbroadcastb",
    "vpbroadcastw",
    "vpbroadcastd",
    "vpbroadcastq",
    "vbroadcasti128",
    "vbroadcasti32x2",
    "vbroadcasti32x4",
    "vbroadcasti64x2",
    "vbroadcasti32x8",
    "vbroadcasti64x4",
    // Lanes in and out of general-purpose registers, and byte masks.
    "pextrb",
    "pextrw",
    "pextrd",
    "pextrq",
    "pinsrb",
    "pinsrw",
    "pinsrd",
    "pinsrq",
    "pmovmskb",
    "vpextrb",
    "vpextrw",
    "vpextrd",
    "vpextrq",
    "vpinsrb",
    "vpinsrw",
    "vpinsrd",
    "vpinsrq",
    "vpmovmskb",
    // Widening and narrowing.
    "pmovzxbw",
    "pmovzxbd",
    "pmovzxbq",
    "pmovzxwd",
    "pmovzxwq",
    "pmovzxdq",
    "pmovsxbw",
    "pmovsxbd",
    "pmovsxbq",
    "pmovsxwd",
    "pmovsxwq",
    "pmovsxdq",
    "vpmovzxbw",
    "vpmovzxbd",
    "vpmovzxbq",
    "vpmovzxwd",
    "vpmovzxwq",
    "vpmovzxdq",
    "vpmovsxbw",
    "vpmovsxbd",
    "vpmovsxbq",
    "vpmovsxwd",
    "vpmovsxwq",
    "vpmovsxdq",
    "vpmovwb",
    "vpmovdb",
    "vpmovqb",
    "vpmovdw",
    "vpmovqw",
    "vpmovqd",
    "vpmovswb",
    "vpmovsdb",
    "vpmovsqb",
    "vpmovsdw",
    "vpmovsqw",
    "vpmovsqd",
    "vpmovuswb",
    "vpmovusdb",
    "vpmovusqb",
    "vpmovusdw",
    "vpmovusqw",
    "vpmovusqd",
    // AVX-512 mask registers.
    "kmovb",
    "kmovw",
    "kmovd",
    "kmovq",
    "kandb",
    "kandw",
    "kandd",
    "kandq",
    "kandnb",
    "kandnw",
    "kandnd",
    "kandnq",
    "korb",
    "korw",
    "kord",
    "korq",
    "kxorb",
    "kxorw",
    "kxord",
    "kxorq",
    "kxnorb",
    "kxnorw",
    "kxnord",
    "kxnorq",
    "knotb",
    "knotw",
    "knotd",
    "knotq",
    "kortestb",
    "kortestw",
    "kortestd",
    "kortestq",
    "ktestb",
    "ktestw",
    "ktestd",
    "ktestq",
    "kshiftlb",
    "kshiftlw",
    "kshiftld",
    "kshiftlq",
    "kshiftrb",
    "kshiftrw",
    "kshiftrd",
    "kshiftrq",
    "kaddb",
    "kaddw",
    "kaddd",
    "kaddq",
    "kunpckbw",
    "kunpckwd",
    "kunpckdq",
    "vpmovm2b",
    "vpmovm2w",
    "vpmovm2d",
    "vpmovm2q",
    "vpmovb2m",
    "vpmovw2m",
    "vpmovd2m",
    "vpmovq2m",
    // Bit counts, conflicts and dot products.
    "vpopcntb",
    "vpopcntw",
    "vpopcntd",
    "vpopcntq",
    "vplzcntd",
    "vplzcntq",
    "vpconflictd",
    "vpconflictq",
    "vpdpbusd",
    "vpdpbusds",
    "vpdpwssd",
    "vpdpwssds",
    // The rest.
    "vzeroupper",
    "vzeroall",
    "crc32",
    // Hashing, checksums and ciphers.
    "aesenc",
    "aesenclast",
    "aesdec",
    "aesdeclast",
    "aesimc",
    "aeskeygenassist",
    "vaesenc",
    "vaesenclast",
    "vaesdec",
    "vaesdeclast",
    "vaesimc",
    "vaeskeygenassist",
    "sha1rnds4",
    "sha1nexte",
    "sha1msg1",
    "sha1msg2",
    "sha256rnds2",
    "sha256msg1",
    "sha256msg2",
    "pclmulqdq",
    "vpclmulqdq",
    "gf2p8affineqb",
    "gf2p8affineinvqb",
    "gf2p8mulb",
    "vgf2p8affineqb",
    "vgf2p8affineinvqb",
    "vgf2p8mulb",
];

/// Double-precision floating point, scalar and packed, from SSE2 to AVX-512. Left out on purpose:
/// half precision, the `rcp`/`rsqrt` approximations (their results differ between CPU makers),
/// `dpps`/`dppd`, `ldmxcsr` (rounding stays nearest, ties to even) and x87.
const FLOAT: &[&str] = &[
    "movsd",
    "vmovsd",
    "movupd",
    "vmovupd",
    "movhpd",
    "vmovhpd",
    "movlpd",
    "vmovlpd",
    "movddup",
    "vmovddup",
    "movmskpd",
    "vmovmskpd",
    "vbroadcastsd",
    "vbroadcastf128",
    "addsd",
    "vaddsd",
    "subsd",
    "vsubsd",
    "mulsd",
    "vmulsd",
    "divsd",
    "vdivsd",
    "sqrtsd",
    "vsqrtsd",
    "minsd",
    "vminsd",
    "maxsd",
    "vmaxsd",
    "addpd",
    "vaddpd",
    "subpd",
    "vsubpd",
    "mulpd",
    "vmulpd",
    "divpd",
    "vdivpd",
    "sqrtpd",
    "vsqrtpd",
    "minpd",
    "vminpd",
    "maxpd",
    "vmaxpd",
    "haddpd",
    "vhaddpd",
    "hsubpd",
    "vhsubpd",
    "addsubpd",
    "vaddsubpd",
    "andpd",
    "vandpd",
    "andnpd",
    "vandnpd",
    "orpd",
    "vorpd",
    "xorpd",
    "vxorpd",
    "comisd",
    "vcomisd",
    "ucomisd",
    "vucomisd",
    "cmpsd",
    "vcmpsd",
    "cmppd",
    "vcmppd",
    "cvtsi2sd",
    "vcvtsi2sd",
    "cvttsd2si",
    "vcvttsd2si",
    "cvtsd2si",
    "vcvtsd2si",
    "cvtdq2pd",
    "vcvtdq2pd",
    "cvtpd2dq",
    "vcvtpd2dq",
    "cvttpd2dq",
    "vcvttpd2dq",
    "vcvtqq2pd",
    "vcvtpd2qq",
    "vcvttpd2qq",
    "vcvtusi2sd",
    "vcvtsd2usi",
    "vcvttsd2usi",
    "vcvtudq2pd",
    "vcvtuqq2pd",
    "vcvtpd2udq",
    "vcvttpd2udq",
    "vcvtpd2uqq",
    "vcvttpd2uqq",
    "roundsd",
    "vroundsd",
    "roundpd",
    "vroundpd",
    "vrndscalesd",
    "vrndscalepd",
    "unpcklpd",
    "vunpcklpd",
    "unpckhpd",
    "vunpckhpd",
    "shufpd",
    "vshufpd",
    "blendpd",
    "vblendpd",
    "blendvpd",
    "vblendvpd",
    "vpermilpd",
    "vpermpd",
    "vperm2f128",
    "vinsertf128",
    "vinsertf64x2",
    "vinsertf64x4",
    "vextractf128",
    "vextractf64x2",
    "vextractf64x4",
    "vshuff64x2",
    "vpermi2pd",
    "vpermt2pd",
    "vblendmpd",
    "vcompresspd",
    "vexpandpd",
];

/// Single-precision floating point, the same operations as [`FLOAT`] on 32-bit numbers, and the
/// conversions between the two precisions.
const SINGLE: &[&str] = &[
    "movss",
    "vmovss",
    "movups",
    "vmovups",
    "movhps",
    "vmovhps",
    "movlps",
    "vmovlps",
    "movhlps",
    "vmovhlps",
    "movlhps",
    "vmovlhps",
    "movshdup",
    "vmovshdup",
    "movsldup",
    "vmovsldup",
    "movmskps",
    "vmovmskps",
    "vbroadcastss",
    "vbroadcastf32x2",
    "vbroadcastf32x4",
    "vbroadcastf32x8",
    "addss",
    "vaddss",
    "subss",
    "vsubss",
    "mulss",
    "vmulss",
    "divss",
    "vdivss",
    "sqrtss",
    "vsqrtss",
    "minss",
    "vminss",
    "maxss",
    "vmaxss",
    "addps",
    "vaddps",
    "subps",
    "vsubps",
    "mulps",
    "vmulps",
    "divps",
    "vdivps",
    "sqrtps",
    "vsqrtps",
    "minps",
    "vminps",
    "maxps",
    "vmaxps",
    "haddps",
    "vhaddps",
    "hsubps",
    "vhsubps",
    "addsubps",
    "vaddsubps",
    "andps",
    "vandps",
    "andnps",
    "vandnps",
    "orps",
    "vorps",
    "xorps",
    "vxorps",
    "comiss",
    "vcomiss",
    "ucomiss",
    "vucomiss",
    "cmpss",
    "vcmpss",
    "cmpps",
    "vcmpps",
    "cvtsi2ss",
    "vcvtsi2ss",
    "cvttss2si",
    "vcvttss2si",
    "cvtss2si",
    "vcvtss2si",
    "cvtss2sd",
    "vcvtss2sd",
    "cvtsd2ss",
    "vcvtsd2ss",
    "cvtps2pd",
    "vcvtps2pd",
    "cvtpd2ps",
    "vcvtpd2ps",
    "cvtdq2ps",
    "vcvtdq2ps",
    "cvtps2dq",
    "vcvtps2dq",
    "cvttps2dq",
    "vcvttps2dq",
    "vcvtqq2ps",
    "vcvtps2qq",
    "vcvttps2qq",
    "vcvtusi2ss",
    "vcvtss2usi",
    "vcvttss2usi",
    "vcvtudq2ps",
    "vcvtuqq2ps",
    "vcvtps2udq",
    "vcvttps2udq",
    "vcvtps2uqq",
    "vcvttps2uqq",
    "roundss",
    "vroundss",
    "roundps",
    "vroundps",
    "vrndscaless",
    "vrndscaleps",
    "unpcklps",
    "vunpcklps",
    "unpckhps",
    "vunpckhps",
    "shufps",
    "vshufps",
    "blendps",
    "vblendps",
    "blendvps",
    "vblendvps",
    "insertps",
    "vinsertps",
    "extractps",
    "vextractps",
    "vpermilps",
    "vpermps",
    "vinsertf32x4",
    "vinsertf32x8",
    "vextractf32x4",
    "vextractf32x8",
    "vshuff32x4",
    "vpermi2ps",
    "vpermt2ps",
    "vblendmps",
    "vcompressps",
    "vexpandps",
];

/// Fused multiply-add: `a × b + c` rounded once, for both precisions. The formulas spell it
/// `fma(a, b, c)`, since it rounds differently from a multiply and then an add.
const FUSED: &[&str] = &[
    "vfmadd132sd",
    "vfmadd213sd",
    "vfmadd231sd",
    "vfmadd132ss",
    "vfmadd213ss",
    "vfmadd231ss",
    "vfmadd132pd",
    "vfmadd213pd",
    "vfmadd231pd",
    "vfmadd132ps",
    "vfmadd213ps",
    "vfmadd231ps",
    "vfmsub132sd",
    "vfmsub213sd",
    "vfmsub231sd",
    "vfmsub132ss",
    "vfmsub213ss",
    "vfmsub231ss",
    "vfmsub132pd",
    "vfmsub213pd",
    "vfmsub231pd",
    "vfmsub132ps",
    "vfmsub213ps",
    "vfmsub231ps",
    "vfnmadd132sd",
    "vfnmadd213sd",
    "vfnmadd231sd",
    "vfnmadd132ss",
    "vfnmadd213ss",
    "vfnmadd231ss",
    "vfnmadd132pd",
    "vfnmadd213pd",
    "vfnmadd231pd",
    "vfnmadd132ps",
    "vfnmadd213ps",
    "vfnmadd231ps",
    "vfnmsub132sd",
    "vfnmsub213sd",
    "vfnmsub231sd",
    "vfnmsub132ss",
    "vfnmsub213ss",
    "vfnmsub231ss",
    "vfnmsub132pd",
    "vfnmsub213pd",
    "vfnmsub231pd",
    "vfnmsub132ps",
    "vfnmsub213ps",
    "vfnmsub231ps",
    "vfmaddsub132pd",
    "vfmaddsub213pd",
    "vfmaddsub231pd",
    "vfmaddsub132ps",
    "vfmaddsub213ps",
    "vfmaddsub231ps",
    "vfmsubadd132pd",
    "vfmsubadd213pd",
    "vfmsubadd231pd",
    "vfmsubadd132ps",
    "vfmsubadd213ps",
    "vfmsubadd231ps",
];

/// Moves that need aligned memory, and what to write instead.
const ALIGNED_MOVES: &[(&str, &str)] = &[
    ("movdqa", "movdqu"),
    ("vmovdqa", "vmovdqu"),
    ("vmovdqa32", "vmovdqu32"),
    ("vmovdqa64", "vmovdqu64"),
    ("movaps", "movdqu"),
    ("movapd", "movdqu"),
    ("vmovaps", "vmovdqu"),
    ("vmovapd", "vmovdqu"),
];

/// Alternative spellings the assembler accepts, and the name the decoder gives them.
const ALIASES: &[(&str, &str)] = &[
    ("jz", "je"),
    ("jnz", "jne"),
    ("jc", "jb"),
    ("jnae", "jb"),
    ("jnc", "jae"),
    ("jnb", "jae"),
    ("jna", "jbe"),
    ("jnbe", "ja"),
    ("jnge", "jl"),
    ("jnl", "jge"),
    ("jng", "jle"),
    ("jnle", "jg"),
    ("jpe", "jp"),
    ("jpo", "jnp"),
    ("sal", "shl"),
    ("cmovz", "cmove"),
    ("cmovnz", "cmovne"),
    ("cmovc", "cmovb"),
    ("cmovnae", "cmovb"),
    ("cmovnc", "cmovae"),
    ("cmovnb", "cmovae"),
    ("cmovna", "cmovbe"),
    ("cmovnbe", "cmova"),
    ("cmovnge", "cmovl"),
    ("cmovnl", "cmovge"),
    ("cmovng", "cmovle"),
    ("cmovnle", "cmovg"),
    ("cmovpe", "cmovp"),
    ("cmovpo", "cmovnp"),
    ("setz", "sete"),
    ("setnz", "setne"),
    ("setc", "setb"),
    ("setnae", "setb"),
    ("setnc", "setae"),
    ("setnb", "setae"),
    ("setna", "setbe"),
    ("setnbe", "seta"),
    ("setnge", "setl"),
    ("setnl", "setge"),
    ("setng", "setle"),
    ("setnle", "setg"),
    ("setpe", "setp"),
    ("setpo", "setnp"),
];

/// The decoder's name for an instruction, in lowercase.
#[must_use]
pub fn mnemonic(instruction: &Instruction) -> String {
    format!("{:?}", instruction.mnemonic()).to_ascii_lowercase()
}

fn canonical(source_mnemonic: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == source_mnemonic)
        .map_or(source_mnemonic, |(_, canonical)| canonical)
}

/// The instruction as the decoder reads it, for messages.
#[must_use]
pub fn display(instruction: &Instruction) -> String {
    let mut formatter = IntelFormatter::new();
    formatter
        .options_mut()
        .set_space_after_operand_separator(true);
    let mut text = String::new();
    formatter.format(instruction, &mut text);
    text
}

/// The CPU features an instruction needs beyond the x86-64 baseline, checked against this CPU.
fn missing_feature(instruction: &Instruction, features: CpuFeatures) -> Option<&'static str> {
    let needs = |present: bool, name: &'static str| (!present).then_some(name);
    instruction
        .cpuid_features()
        .iter()
        .find_map(|feature| match feature {
            // Every x86-64 CPU has these, SSE and SSE2 included.
            CpuidFeature::INTEL8086
            | CpuidFeature::INTEL186
            | CpuidFeature::INTEL286
            | CpuidFeature::INTEL386
            | CpuidFeature::INTEL486
            | CpuidFeature::X64
            | CpuidFeature::CMOV
            | CpuidFeature::SYSCALL
            | CpuidFeature::SSE
            | CpuidFeature::SSE2 => None,
            CpuidFeature::BMI1 => needs(features.bmi1, "BMI1"),
            CpuidFeature::BMI2 => needs(features.bmi2, "BMI2"),
            CpuidFeature::POPCNT => needs(features.popcnt, "POPCNT"),
            CpuidFeature::LZCNT => needs(features.lzcnt, "LZCNT"),
            CpuidFeature::ADX => needs(features.adx, "ADX"),
            CpuidFeature::MOVBE => needs(features.movbe, "MOVBE"),
            CpuidFeature::SSE3 => needs(features.sse3, "SSE3"),
            CpuidFeature::SSSE3 => needs(features.ssse3, "SSSE3"),
            CpuidFeature::SSE4_1 => needs(features.sse4_1, "SSE4.1"),
            CpuidFeature::SSE4_2 => needs(features.sse4_2, "SSE4.2"),
            CpuidFeature::AVX => needs(features.avx, "AVX"),
            CpuidFeature::AVX2 => needs(features.avx2, "AVX2"),
            CpuidFeature::FMA => needs(features.fma, "FMA"),
            CpuidFeature::AVX512F => needs(features.avx512f, "AVX-512F"),
            CpuidFeature::AVX512BW => needs(features.avx512bw, "AVX-512BW"),
            CpuidFeature::AVX512CD => needs(features.avx512cd, "AVX-512CD"),
            CpuidFeature::AVX512DQ => needs(features.avx512dq, "AVX-512DQ"),
            CpuidFeature::AVX512VL => needs(features.avx512vl, "AVX-512VL"),
            CpuidFeature::AVX512_VBMI => needs(features.avx512vbmi, "AVX-512 VBMI"),
            CpuidFeature::AVX512_VBMI2 => needs(features.avx512vbmi2, "AVX-512 VBMI2"),
            CpuidFeature::AVX512_VNNI => needs(features.avx512vnni, "AVX-512 VNNI"),
            CpuidFeature::AVX512_BITALG => needs(features.avx512bitalg, "AVX-512 BITALG"),
            CpuidFeature::AVX512_VPOPCNTDQ => needs(features.avx512vpopcntdq, "AVX-512 VPOPCNTDQ"),
            CpuidFeature::AVX_VNNI => needs(features.avxvnni, "AVX-VNNI"),
            CpuidFeature::AES => needs(features.aes, "AES-NI"),
            CpuidFeature::VAES => needs(features.vaes, "VAES"),
            CpuidFeature::SHA => needs(features.sha, "SHA"),
            CpuidFeature::PCLMULQDQ => needs(features.pclmulqdq, "PCLMULQDQ"),
            CpuidFeature::VPCLMULQDQ => needs(features.vpclmulqdq, "VPCLMULQDQ"),
            CpuidFeature::GFNI => needs(features.gfni, "GFNI"),
            _ => Some("an extension outside the dialect"),
        })
}

/// Matches every source function to its machine code. Returns `None` when they don't match,
/// after recording why: then nothing else can be trusted.
pub fn map<'a>(
    source: &'a Source,
    machine: &Machine,
    features: CpuFeatures,
    violations: &mut Vec<Violation>,
) -> Option<Program<'a>> {
    let before = violations.len();
    let mut functions = Vec::new();
    for function in &source.functions {
        let Some(code) = machine.functions.get(&function.name) else {
            violations.push(Violation {
                rule: Rule::Dialect,
                line: Some(function.line),
                problem: format!("{} isn't in the executable", function.name),
                fix: "give it `.type` and `.size` lines and keep it in .text".into(),
            });
            continue;
        };
        if let Some(mapped) = map_function(function, code, violations) {
            functions.push(mapped);
        }
    }
    if violations.len() > before {
        return None;
    }
    let program = Program {
        source,
        functions,
        data: machine.data.clone(),
    };
    for function in &program.functions {
        for step in &function.steps {
            if let Err(problem) = same_instruction(step, function, &program, machine) {
                violations.push(Violation {
                    rule: Rule::Dialect,
                    line: Some(step.source.line),
                    problem: format!(
                        "`{}` became `{}` in the machine code: {problem}",
                        step.source.text,
                        display(&step.machine)
                    ),
                    fix: "write the instruction so it means what the machine does; constants \
                          must come from the block at the top and data must be addressed as \
                          [rip + name]"
                        .into(),
                });
            }
            check_instruction(step, features, violations);
        }
    }
    (violations.len() == before).then_some(program)
}

fn map_function<'a>(
    function: &'a SourceFunction,
    code: &MachineFunction,
    violations: &mut Vec<Violation>,
) -> Option<MappedFunction<'a>> {
    let sources: Vec<&SourceInstruction> = function.instructions().collect();
    let first_difference = sources
        .iter()
        .zip(&code.instructions)
        .position(|(source, machine)| canonical(&source.mnemonic) != mnemonic(machine));
    if sources.len() != code.instructions.len() || first_difference.is_some() {
        let index = first_difference.unwrap_or(sources.len().min(code.instructions.len()));
        let (line, found) = match (sources.get(index), code.instructions.get(index)) {
            (Some(source), Some(machine)) => (
                source.line,
                format!("`{}` became `{}`", source.text, display(machine)),
            ),
            (Some(source), None) => (
                source.line,
                format!("`{}` has no machine instruction", source.text),
            ),
            (None, Some(machine)) => (
                function.line,
                format!("the machine code has an extra `{}`", display(machine)),
            ),
            (None, None) => (function.line, String::new()),
        };
        violations.push(Violation {
            rule: Rule::Dialect,
            line: Some(line),
            problem: format!(
                "main.s and the machine code of {} don't match line for line: {found}",
                function.name
            ),
            fix: "write exactly one plain instruction per line, with no prefixes or directives \
                  between instructions"
                .into(),
        });
        return None;
    }
    let steps: Vec<Step<'a>> = sources
        .into_iter()
        .zip(&code.instructions)
        .map(|(source, machine)| Step {
            source,
            machine: *machine,
        })
        .collect();
    let end = code.address + code.size;
    let mut labels = Vec::new();
    let mut index = 0;
    for item in &function.items {
        match item {
            Item::Instruction(_) => index += 1,
            Item::Label(label) => labels.push(LabelAt {
                label,
                index,
                address: steps.get(index).map_or(end, |step| step.machine.ip()),
            }),
        }
    }
    Some(MappedFunction {
        source: function,
        address: code.address,
        end,
        steps,
        labels,
    })
}

/// Compares each operand as written with the operand the decoder found.
fn same_instruction(
    step: &Step<'_>,
    function: &MappedFunction<'_>,
    program: &Program<'_>,
    machine: &Machine,
) -> Result<(), String> {
    let instruction = &step.machine;
    let count = instruction.op_count();
    if usize::try_from(count).ok() != Some(step.source.operands.len()) {
        return Err(format!(
            "it has {} operands as written and {count} in the machine code",
            step.source.operands.len()
        ));
    }
    for (index, operand) in (0..count).zip(&step.source.operands) {
        let kind = instruction.op_kind(index);
        let position = index + 1;
        match operand {
            Operand::Register(register) => {
                if kind != OpKind::Register || instruction.op_register(index) != *register {
                    return Err(format!(
                        "operand {position} isn't the register {register:?}"
                    ));
                }
            }
            Operand::Immediate { value, .. } => {
                let bits = immediate_bits(kind).ok_or_else(|| {
                    format!("operand {position} isn't a number in the machine code")
                })?;
                let mask = if bits == 64 {
                    u64::MAX
                } else {
                    (1_u64 << bits) - 1
                };
                // Reinterpret the value's bits, as the assembler does.
                let written = u64::from_ne_bytes(value.to_ne_bytes());
                if written & mask != instruction.immediate(index) & mask {
                    return Err(format!(
                        "operand {position} is {value} as written but {} in the machine code",
                        instruction.immediate(index)
                    ));
                }
            }
            Operand::Memory(memory) => {
                if kind != OpKind::Memory {
                    return Err(format!(
                        "operand {position} isn't memory in the machine code"
                    ));
                }
                same_memory(memory, instruction, program)
                    .map_err(|problem| format!("operand {position}: {problem}"))?;
            }
            Operand::Target(name) => {
                if !matches!(
                    kind,
                    OpKind::NearBranch16 | OpKind::NearBranch32 | OpKind::NearBranch64
                ) {
                    return Err(format!(
                        "operand {position} isn't a jump target in the machine code"
                    ));
                }
                let expected = function
                    .labels
                    .iter()
                    .find(|label| label.label.name == *name)
                    .map(|label| label.address)
                    .or_else(|| machine.functions.get(name).map(|code| code.address))
                    .ok_or_else(|| {
                        format!(
                            "`{name}` isn't a label in {} or a function",
                            function.name()
                        )
                    })?;
                if instruction.near_branch_target() != expected {
                    return Err(format!("operand {position} doesn't land on {name}"));
                }
            }
        }
    }
    let mask = step.source.mask.unwrap_or(Register::None);
    if instruction.op_mask() != mask {
        return Err(format!(
            "the write mask is {:?} in the machine code",
            instruction.op_mask()
        ));
    }
    if instruction.zeroing_masking() != step.source.zeroing {
        return Err("{z} differs in the machine code".into());
    }
    Ok(())
}

fn same_memory(
    memory: &Memory,
    instruction: &Instruction,
    program: &Program<'_>,
) -> Result<(), String> {
    let base = memory.base.unwrap_or(Register::None);
    let index = memory.index.unwrap_or(Register::None);
    if instruction.memory_base() != base {
        return Err(format!(
            "the base register is {:?} in the machine code",
            instruction.memory_base()
        ));
    }
    if instruction.memory_index() != index
        || (memory.index.is_some() && instruction.memory_index_scale() != memory.scale)
    {
        return Err("the index or scale differs in the machine code".into());
    }
    let displacement = u64::from_ne_bytes(memory.displacement.to_ne_bytes());
    let expected = match &memory.symbol {
        Some(symbol) => program
            .data
            .get(symbol)
            .map(|data| data.address.wrapping_add(displacement))
            .ok_or_else(|| format!("`{symbol}` isn't a data symbol"))?,
        None => displacement,
    };
    if instruction.memory_displacement64() != expected {
        return Err("the address differs in the machine code".into());
    }
    if memory.broadcast != instruction.is_broadcast() {
        return Err("the broadcast ({1toN}) differs in the machine code".into());
    }
    if let Some(size) = memory.size {
        let decoded = instruction.memory_size();
        if decoded != MemorySize::Unknown && decoded.size() != usize::from(size) {
            return Err(format!(
                "it accesses {} bytes in the machine code",
                decoded.size()
            ));
        }
    }
    Ok(())
}

fn immediate_bits(kind: OpKind) -> Option<u32> {
    match kind {
        OpKind::Immediate8 | OpKind::Immediate8_2nd => Some(8),
        OpKind::Immediate16 | OpKind::Immediate8to16 => Some(16),
        OpKind::Immediate32 | OpKind::Immediate8to32 => Some(32),
        OpKind::Immediate64 | OpKind::Immediate8to64 | OpKind::Immediate32to64 => Some(64),
        _ => None,
    }
}

/// Checks the machine instruction itself: the instruction set, CPU features, prefixes and
/// addressing.
fn check_instruction(step: &Step<'_>, features: CpuFeatures, violations: &mut Vec<Violation>) {
    let instruction = &step.machine;
    let line = Some(step.source.line);
    let name = mnemonic(instruction);
    if let Some((_, unaligned)) = ALIGNED_MOVES.iter().find(|(aligned, _)| *aligned == name) {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!("`{name}` faults unless its memory is aligned"),
            fix: format!(
                "use `{unaligned}`: the dialect moves vectors with unaligned loads and stores"
            ),
        });
        return;
    }
    if instruction.is_string_instruction() {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!(
                "`{}` is a string instruction, which isn't in the dialect",
                step.source.text
            ),
            fix: "use a bounded loop; the floating-point movsd and cmpsd take xmm registers".into(),
        });
        return;
    }
    if ![ALLOWED, VECTOR, FLOAT, SINGLE, FUSED]
        .iter()
        .any(|list| list.contains(&name.as_str()))
    {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!(
                "`{name}` isn't in dialect 2, which allows integer, vector and floating-point \
                 instructions"
            ),
            fix: "use the instructions listed in references/power-of-10.md: no half precision, \
                  rcp/rsqrt approximations, x87, MMX, string instructions, gathers or scatters"
                .into(),
        });
        return;
    }
    if let Some(feature) = missing_feature(instruction, features) {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!("`{name}` needs {feature}, which this CPU doesn't have"),
            fix: "use instructions this CPU has (status lists its features); SSE2 vectors work \
                  on every x86-64 CPU"
                .into(),
        });
    }
    if (0..instruction.op_count()).any(|index| {
        instruction.op_kind(index) == OpKind::Register && instruction.op_register(index).is_mm()
    }) {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: "MMX registers aren't in the dialect".into(),
            fix: "use xmm registers".into(),
        });
    }
    if instruction.is_vsib() {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!(
                "`{}` gathers or scatters through a vector of addresses, which can't be \
                 bounds-checked",
                step.source.text
            ),
            fix: "load the elements with indexed, bounds-checked accesses".into(),
        });
    }
    if instruction.rounding_control() != RoundingControl::None
        || instruction.suppress_all_exceptions()
    {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!(
                "`{}` overrides rounding or exceptions, which aren't in the dialect",
                step.source.text
            ),
            fix: "leave rounding to the default: nearest, ties to even".into(),
        });
    }
    let has_memory =
        (0..instruction.op_count()).any(|index| instruction.op_kind(index) == OpKind::Memory);
    if has_memory
        && instruction.encoding() == EncodingKind::Legacy
        && instruction.memory_size().size() == 16
        && !matches!(name.as_str(), "movdqu" | "lddqu")
    {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!(
                "`{name}` reads 16 bytes of memory in its SSE form, which faults unless the \
                 address is a multiple of 16"
            ),
            fix: format!(
                "load the bytes into a register with movdqu first, or use the VEX form `v{name}` \
                 when the CPU has AVX"
            ),
        });
    }
    if instruction.segment_prefix() != Register::None {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: "segment registers aren't in the dialect".into(),
            fix: "address memory through general-purpose registers or [rip + name]".into(),
        });
    }
    if matches!(
        instruction.flow_control(),
        FlowControl::IndirectBranch | FlowControl::IndirectCall
    ) {
        violations.push(Violation {
            rule: Rule::Indirection,
            line,
            problem: format!("`{}` jumps through a register or memory", step.source.text),
            fix: "jump and call only to labels and function names; choose between targets with \
                  compare-and-jump chains"
                .into(),
        });
    }
    if has_memory
        && instruction.memory_base() == Register::None
        && !instruction.is_ip_rel_memory_operand()
    {
        violations.push(Violation {
            rule: Rule::Dialect,
            line,
            problem: format!(
                "`{}` reads or writes a fixed address ({})",
                step.source.text,
                display(instruction)
            ),
            fix: "use a register base or [rip + name]; a constant used before its .equ or a \
                  `. - name` turns into a memory address"
                .into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn decode(bytes: &[u8]) -> Instruction {
        Decoder::with_ip(64, bytes, 0x1000, DecoderOptions::NONE).decode()
    }

    #[test]
    fn iced_treats_syscall_as_a_call() {
        // The control-flow graph relies on this: syscall returns unless it's an exit.
        let syscall = decode(&[0x0f, 0x05]);
        assert_eq!(mnemonic(&syscall), "syscall");
        assert_eq!(syscall.flow_control(), FlowControl::Call);
    }

    #[test]
    fn names_mnemonics_like_the_source() {
        assert_eq!(mnemonic(&decode(&[0x74, 0x00])), "je");
        assert_eq!(mnemonic(&decode(&[0x48, 0x99])), "cqo");
        assert_eq!(canonical("jz"), "je");
        assert_eq!(canonical("sal"), "shl");
        assert_eq!(canonical("mov"), "mov");
    }

    #[test]
    fn gates_integer_extensions_on_the_cpu() {
        // shlx rax, rbx, rcx
        let shlx = decode(&[0xc4, 0xe2, 0xf1, 0xf7, 0xc3]);
        assert_eq!(mnemonic(&shlx), "shlx");
        assert_eq!(missing_feature(&shlx, CpuFeatures::all()), None);
        let without_bmi2 = CpuFeatures {
            bmi2: false,
            ..CpuFeatures::all()
        };
        assert_eq!(missing_feature(&shlx, without_bmi2), Some("BMI2"));
        assert_eq!(
            missing_feature(&decode(&[0x0f, 0x05]), CpuFeatures::default()),
            None
        );
    }

    #[test]
    fn every_listed_instruction_is_one_the_decoder_names() {
        let known: BTreeSet<String> = iced_x86::Mnemonic::values()
            .map(|mnemonic| format!("{mnemonic:?}").to_ascii_lowercase())
            .collect();
        let aligned = ALIGNED_MOVES.iter().map(|(aligned, _)| *aligned);
        for name in ALLOWED
            .iter()
            .chain(VECTOR)
            .chain(FLOAT)
            .copied()
            .chain(aligned)
        {
            assert!(known.contains(name), "{name} isn't an iced-x86 mnemonic");
        }
        for (alias, canonical) in ALIASES {
            assert!(known.contains(*canonical), "{alias} → {canonical}");
        }
    }

    #[test]
    fn gates_vector_extensions_on_the_cpu() {
        // paddb xmm0, xmm1: SSE2, in every x86-64 CPU.
        let paddb = decode(&[0x66, 0x0f, 0xfc, 0xc1]);
        assert_eq!(mnemonic(&paddb), "paddb");
        assert_eq!(missing_feature(&paddb, CpuFeatures::default()), None);
        // pshufb xmm0, xmm1: SSSE3.
        let pshufb = decode(&[0x66, 0x0f, 0x38, 0x00, 0xc1]);
        assert_eq!(
            missing_feature(&pshufb, CpuFeatures::default()),
            Some("SSSE3")
        );
        // vpaddb ymm0, ymm1, ymm2: AVX2.
        let vpaddb = decode(&[0xc5, 0xf5, 0xfc, 0xc2]);
        assert_eq!(mnemonic(&vpaddb), "vpaddb");
        assert_eq!(missing_feature(&vpaddb, CpuFeatures::all()), None);
        let without_avx2 = CpuFeatures {
            avx2: false,
            ..CpuFeatures::all()
        };
        assert_eq!(missing_feature(&vpaddb, without_avx2), Some("AVX2"));
    }
}

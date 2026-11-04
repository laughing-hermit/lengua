.arch armv8-a

// CRC-32C in plain integer code, a byte at a time through a table of 256 words, so it runs on every
// ARM64 computer, with or without the CRC32 instructions (an extension before ARMv8.1). The form
// that uses crc32cx and pmull, from the very same spec, is
// plugin/skills/compile/assets/linux-aarch64/fast/crc32c-crc.

// System calls (Linux AArch64)
.equ SYS_READ, 63
.equ SYS_WRITE, 64
.equ SYS_EXIT, 93
// File descriptors
.equ STDIN, 0
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_IO_FAILED, 2
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
// fill's constants
.equ MAX_FILL_READS, 1099511627776
.equ READ_TO_END, -1
// CRC-32C starts from all ones and ends inverted.
.equ CRC_START, 0xFFFFFFFF
// Sizes and limits: standard input is folded in pieces of INPUT_LEN bytes, a byte at a step
.equ INPUT_LEN, 65536
.equ BYTE_STEPS, INPUT_LEN + 1
.equ TABLE_ENTRIES, 256
.equ MAX_FULLS, 1099511627776
// Printing the checksum
.equ MAX_WRITES, 64
.equ MAX_DIGITS, 19
.equ NEWLINE, 10
.equ OUT_BUF_LEN, 21

.section .rodata

// The CRC-32C of each byte value: the byte through eight steps of the reflected polynomial
// 0x82f63b78.
.type crc_table, @object
crc_table:
    .long 0x00000000, 0xf26b8303, 0xe13b70f7, 0x1350f3f4
    .long 0xc79a971f, 0x35f1141c, 0x26a1e7e8, 0xd4ca64eb
    .long 0x8ad958cf, 0x78b2dbcc, 0x6be22838, 0x9989ab3b
    .long 0x4d43cfd0, 0xbf284cd3, 0xac78bf27, 0x5e133c24
    .long 0x105ec76f, 0xe235446c, 0xf165b798, 0x030e349b
    .long 0xd7c45070, 0x25afd373, 0x36ff2087, 0xc494a384
    .long 0x9a879fa0, 0x68ec1ca3, 0x7bbcef57, 0x89d76c54
    .long 0x5d1d08bf, 0xaf768bbc, 0xbc267848, 0x4e4dfb4b
    .long 0x20bd8ede, 0xd2d60ddd, 0xc186fe29, 0x33ed7d2a
    .long 0xe72719c1, 0x154c9ac2, 0x061c6936, 0xf477ea35
    .long 0xaa64d611, 0x580f5512, 0x4b5fa6e6, 0xb93425e5
    .long 0x6dfe410e, 0x9f95c20d, 0x8cc531f9, 0x7eaeb2fa
    .long 0x30e349b1, 0xc288cab2, 0xd1d83946, 0x23b3ba45
    .long 0xf779deae, 0x05125dad, 0x1642ae59, 0xe4292d5a
    .long 0xba3a117e, 0x4851927d, 0x5b016189, 0xa96ae28a
    .long 0x7da08661, 0x8fcb0562, 0x9c9bf696, 0x6ef07595
    .long 0x417b1dbc, 0xb3109ebf, 0xa0406d4b, 0x522bee48
    .long 0x86e18aa3, 0x748a09a0, 0x67dafa54, 0x95b17957
    .long 0xcba24573, 0x39c9c670, 0x2a993584, 0xd8f2b687
    .long 0x0c38d26c, 0xfe53516f, 0xed03a29b, 0x1f682198
    .long 0x5125dad3, 0xa34e59d0, 0xb01eaa24, 0x42752927
    .long 0x96bf4dcc, 0x64d4cecf, 0x77843d3b, 0x85efbe38
    .long 0xdbfc821c, 0x2997011f, 0x3ac7f2eb, 0xc8ac71e8
    .long 0x1c661503, 0xee0d9600, 0xfd5d65f4, 0x0f36e6f7
    .long 0x61c69362, 0x93ad1061, 0x80fde395, 0x72966096
    .long 0xa65c047d, 0x5437877e, 0x4767748a, 0xb50cf789
    .long 0xeb1fcbad, 0x197448ae, 0x0a24bb5a, 0xf84f3859
    .long 0x2c855cb2, 0xdeeedfb1, 0xcdbe2c45, 0x3fd5af46
    .long 0x7198540d, 0x83f3d70e, 0x90a324fa, 0x62c8a7f9
    .long 0xb602c312, 0x44694011, 0x5739b3e5, 0xa55230e6
    .long 0xfb410cc2, 0x092a8fc1, 0x1a7a7c35, 0xe811ff36
    .long 0x3cdb9bdd, 0xceb018de, 0xdde0eb2a, 0x2f8b6829
    .long 0x82f63b78, 0x709db87b, 0x63cd4b8f, 0x91a6c88c
    .long 0x456cac67, 0xb7072f64, 0xa457dc90, 0x563c5f93
    .long 0x082f63b7, 0xfa44e0b4, 0xe9141340, 0x1b7f9043
    .long 0xcfb5f4a8, 0x3dde77ab, 0x2e8e845f, 0xdce5075c
    .long 0x92a8fc17, 0x60c37f14, 0x73938ce0, 0x81f80fe3
    .long 0x55326b08, 0xa759e80b, 0xb4091bff, 0x466298fc
    .long 0x1871a4d8, 0xea1a27db, 0xf94ad42f, 0x0b21572c
    .long 0xdfeb33c7, 0x2d80b0c4, 0x3ed04330, 0xccbbc033
    .long 0xa24bb5a6, 0x502036a5, 0x4370c551, 0xb11b4652
    .long 0x65d122b9, 0x97baa1ba, 0x84ea524e, 0x7681d14d
    .long 0x2892ed69, 0xdaf96e6a, 0xc9a99d9e, 0x3bc21e9d
    .long 0xef087a76, 0x1d63f975, 0x0e330a81, 0xfc588982
    .long 0xb21572c9, 0x407ef1ca, 0x532e023e, 0xa145813d
    .long 0x758fe5d6, 0x87e466d5, 0x94b49521, 0x66df1622
    .long 0x38cc2a06, 0xcaa7a905, 0xd9f75af1, 0x2b9cd9f2
    .long 0xff56bd19, 0x0d3d3e1a, 0x1e6dcdee, 0xec064eed
    .long 0xc38d26c4, 0x31e6a5c7, 0x22b65633, 0xd0ddd530
    .long 0x0417b1db, 0xf67c32d8, 0xe52cc12c, 0x1747422f
    .long 0x49547e0b, 0xbb3ffd08, 0xa86f0efc, 0x5a048dff
    .long 0x8ecee914, 0x7ca56a17, 0x6ff599e3, 0x9d9e1ae0
    .long 0xd3d3e1ab, 0x21b862a8, 0x32e8915c, 0xc083125f
    .long 0x144976b4, 0xe622f5b7, 0xf5720643, 0x07198540
    .long 0x590ab964, 0xab613a67, 0xb831c993, 0x4a5a4a90
    .long 0x9e902e7b, 0x6cfbad78, 0x7fab5e8c, 0x8dc0dd8f
    .long 0xe330a81a, 0x115b2b19, 0x020bd8ed, 0xf0605bee
    .long 0x24aa3f05, 0xd6c1bc06, 0xc5914ff2, 0x37faccf1
    .long 0x69e9f0d5, 0x9b8273d6, 0x88d28022, 0x7ab90321
    .long 0xae7367ca, 0x5c18e4c9, 0x4f48173d, 0xbd23943e
    .long 0xf36e6f75, 0x0105ec76, 0x12551f82, 0xe03e9c81
    .long 0x34f4f86a, 0xc69f7b69, 0xd5cf889d, 0x27a40b9e
    .long 0x79b737ba, 0x8bdcb4b9, 0x988c474d, 0x6ae7c44e
    .long 0xbe2da0a5, 0x4c4623a6, 0x5f16d052, 0xad7d5351
.size crc_table, . - crc_table

.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.section .bss

// One piece of standard input, as it comes.
.type input, @object
input:
    .zero INPUT_LEN
.size input, . - input

// The checksum's digits and a newline.
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
.globl _start

// Folds standard input into the checksum a piece at a time, as the pieces come, and prints it.
.type _start, @function
_start:
    mov w21, #CRC_START
    mov x19, #MAX_FULLS
.Lstart_full:  // bound: 1099511627776
    mov x0, #STDIN
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, #INPUT_LEN
    mov w3, #READ_TO_END
    bl fill
    tbnz x0, #63, .Lstart_failed
    cbz x0, .Lstart_print
    cmp x0, #INPUT_LEN
    b.hi .Lstart_assert_count
    mov x20, x0
    mov w0, w21
    adrp x1, input
    add x1, x1, :lo12:input
    mov x2, x20
    bl crc_bytes
    mov w21, w0
    cmp x20, #INPUT_LEN
    b.lo .Lstart_print
    subs x19, x19, #1
    b.ne .Lstart_full
    b .Lstart_assert_fulls
.Lstart_print:
    mvn w0, w21
    bl write_i64
    tbnz x0, #63, .Lstart_failed
    mov w0, #EXIT_OK
    b .Lstart_exit
.Lstart_failed:
    mov w0, #EXIT_IO_FAILED
.Lstart_exit:
    mov x8, #SYS_EXIT
    svc #0
.Lstart_assert_count:
    mov w0, #1
    bl assert_fail
.Lstart_assert_fulls:
    mov w0, #2
    bl assert_fail
.size _start, . - _start

// Folds bytes into a CRC-32C state, a byte at a time: the state's low byte xor the byte picks a
// word of the table, which is xored with the state moved down a byte.
// In: w0 = the state; x1 = the bytes; x2 = how many, up to INPUT_LEN.
// Out: w0 = the new state.
.type crc_bytes, @function
crc_bytes:
    cbz x1, .Lcrc_bytes_assert_bytes
    cmp x2, #INPUT_LEN
    b.hi .Lcrc_bytes_assert_length
    adrp x9, crc_table
    add x9, x9, :lo12:crc_table
    mov x3, #0
    mov x4, #BYTE_STEPS & 0xffff
    movk x4, #BYTE_STEPS >> 16, lsl #16
.Lcrc_bytes_loop:  // bound: 65537
    cmp x3, x2
    b.hs .Lcrc_bytes_done
    cmp x3, #INPUT_LEN
    b.hs .Lcrc_bytes_assert_index
    ldrb w5, [x1, x3]
    eor w5, w5, w0
    and w5, w5, #255
    cmp x5, #TABLE_ENTRIES
    b.hs .Lcrc_bytes_assert_table
    ldr w6, [x9, x5, lsl #2]
    eor w0, w6, w0, lsr #8
    add x3, x3, #1
    subs x4, x4, #1
    b.ne .Lcrc_bytes_loop
.Lcrc_bytes_done:
    ret
.Lcrc_bytes_assert_bytes:
    mov w0, #3
    bl assert_fail
.Lcrc_bytes_assert_length:
    mov w0, #4
    bl assert_fail
.Lcrc_bytes_assert_index:
    mov w0, #5
    bl assert_fail
.Lcrc_bytes_assert_table:
    mov w0, #13
    bl assert_fail
.size crc_bytes, . - crc_bytes

// Writes a whole number in decimal, then a newline, to standard output.
// In: x0 = the number, never -9223372036854775808.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_i64, @function
write_i64:
    stp x29, x30, [sp, #-16]!
    mov x9, x0
    cmp x0, #0
    b.ge .Lwrite_i64_digits
    negs x0, x0
    b.vs .Lwrite_i64_assert_min
.Lwrite_i64_digits:
    adrp x1, out_buf
    add x1, x1, :lo12:out_buf
    mov w10, #NEWLINE
    strb w10, [x1, #OUT_BUF_LEN - 1]
    mov x2, #OUT_BUF_LEN - 1
    mov x11, #10
    mov x3, #MAX_DIGITS
.Lwrite_i64_digit:  // bound: 19
    udiv x12, x0, x11
    msub x13, x12, x11, x0
    add w13, w13, #'0'
    sub x2, x2, #1
    cmp x2, #OUT_BUF_LEN
    b.hs .Lwrite_i64_assert_digit_index
    strb w13, [x1, x2]
    mov x0, x12
    cbz x0, .Lwrite_i64_sign
    subs x3, x3, #1
    b.ne .Lwrite_i64_digit
    b .Lwrite_i64_assert_digits
.Lwrite_i64_sign:
    tbz x9, #63, .Lwrite_i64_output
    sub x2, x2, #1
    cmp x2, #OUT_BUF_LEN
    b.hs .Lwrite_i64_assert_sign_index
    mov w10, #'-'
    strb w10, [x1, x2]
.Lwrite_i64_output:
    add x1, x1, x2
    mov x10, #OUT_BUF_LEN
    sub x2, x10, x2
    mov x0, #STDOUT
    bl write_all
    tbnz x0, #63, .Lwrite_i64_return
    mov x0, #0
.Lwrite_i64_return:
    ldp x29, x30, [sp], #16
    ret
.Lwrite_i64_assert_min:
    mov w0, #6
    bl assert_fail
.Lwrite_i64_assert_digit_index:
    mov w0, #7
    bl assert_fail
.Lwrite_i64_assert_digits:
    mov w0, #8
    bl assert_fail
.Lwrite_i64_assert_sign_index:
    mov w0, #9
    bl assert_fail
.size write_i64, . - write_i64

// Writes x2 bytes from x1 to file descriptor x0, retrying partial writes.
// In: x0 = the file descriptor, x1 = the bytes, x2 = how many, at least 1.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_all, @function
write_all:
    cbz x1, .Lwrite_all_assert_null
    cbz x2, .Lwrite_all_assert_empty
    mov x9, x0
    mov x10, #MAX_WRITES
.Lwrite_all_loop:  // bound: 64
    mov x0, x9
    mov x8, #SYS_WRITE
    svc #0
    tbnz x0, #63, .Lwrite_all_return
    cmp x0, x2
    b.hi .Lwrite_all_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Lwrite_all_done
    subs x10, x10, #1
    b.ne .Lwrite_all_loop
    mov x0, #ERROR_IO
    b .Lwrite_all_return
.Lwrite_all_done:
    mov x0, #0
.Lwrite_all_return:
    ret
.Lwrite_all_assert_null:
    mov w0, #10
    bl assert_fail
.Lwrite_all_assert_empty:
    mov w0, #11
    bl assert_fail
.Lwrite_all_assert_count:
    mov w0, #12
    bl assert_fail
.size write_all, . - write_all

// Reads from file descriptor x0 into the x2 bytes at x1 until they're full, the input ends,
// or, when w3 is a byte (0 to 255), the last byte read is w3; with w3 = -1 it reads to the
// end. However the input arrives, it reads the same.
// In: x0 = the file descriptor, x1 = the buffer, x2 = its size, at least 1, w3 = the end
// byte or -1.
// Out: x0 = how many bytes it read, or a negative error code.
// returns: result
.type fill, @function
fill:
    cbz x1, .Lfill_assert_null
    cbz x2, .Lfill_assert_empty
    mov x9, x0
    mov x10, x2
    mov w11, w3
    mov x12, #MAX_FILL_READS
.Lfill_loop:  // bound: 1099511627776
    mov x0, x9
    mov x8, #SYS_READ
    svc #0
    tbnz x0, #63, .Lfill_return
    cbz x0, .Lfill_done
    cmp x0, x2
    b.hi .Lfill_assert_count
    add x1, x1, x0
    subs x2, x2, x0
    b.eq .Lfill_done
    ldurb w13, [x1, #-1]
    cmp w13, w11
    b.eq .Lfill_done
    subs x12, x12, #1
    b.ne .Lfill_loop
    mov x0, #ERROR_IO
    b .Lfill_return
.Lfill_done:
    sub x0, x10, x2
.Lfill_return:
    ret
.Lfill_assert_null:
    mov w0, #990
    bl assert_fail
.Lfill_assert_empty:
    mov w0, #991
    bl assert_fail
.Lfill_assert_count:
    mov w0, #992
    bl assert_fail
.size fill, . - fill

// lengua's assertion handler. check compares it word for word: copy it exactly.
// In: w0 = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
// standard error and ends the program, every thread of it, with code 134.
.type assert_fail, @function
assert_fail:
    sub sp, sp, #32
    adrp x1, assert_message
    add x1, x1, :lo12:assert_message
    ldp x2, x3, [x1]
    stp x2, x3, [sp]
    ldr x2, [x1, #16]
    str x2, [sp, #16]
    ldr w2, [x1, #24]
    str w2, [sp, #24]
    ldrb w2, [x1, #28]
    strb w2, [sp, #28]
    mov w2, #100
    udiv w3, w0, w2
    msub w0, w3, w2, w0
    add w3, w3, #48
    strb w3, [sp, #18]
    mov w2, #10
    udiv w3, w0, w2
    msub w0, w3, w2, w0
    add w3, w3, #48
    strb w3, [sp, #19]
    add w0, w0, #48
    strb w0, [sp, #20]
    mov x0, #2
    mov x1, sp
    mov x2, #29
    mov x8, #64
    svc #0
    mov x0, #134
    mov x8, #94
    svc #0
.size assert_fail, . - assert_fail

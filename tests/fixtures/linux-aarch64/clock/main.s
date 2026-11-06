.arch armv8-a

// The clock of the day's time
.equ CLOCK_REALTIME, 0
// File descriptors
.equ STDOUT, 1
// Exit codes
.equ EXIT_OK, 0
.equ EXIT_WRITE_FAILED, 2
.equ EXIT_CLOCK_FAILED, 3
// Error codes returned by functions marked "returns: result"
.equ ERROR_IO, -5
.equ MAX_WRITES, 64
.equ NEWLINE, 10
.equ COLON, 58
// A timespec, seconds then nanoseconds, and the line "hh:mm:ss\n"
.equ TIMESPEC_LEN, 16
.equ OUT_BUF_LEN, 9
// The clock's units
.equ SECONDS_PER_DAY, 86400
.equ SECONDS_PER_HOUR, 3600
.equ SECONDS_PER_MINUTE, 60
.equ HOURS_PER_DAY, 24
.equ MINUTES_PER_HOUR, 60
.equ NANOS_PER_SECOND, 1000000000
.equ DECIMAL_BASE, 10

.section .bss

// What the clock says: seconds since 1970, then nanoseconds.
.type timespec, @object
timespec:
    .zero TIMESPEC_LEN
.size timespec, . - timespec

// The line it prints, "hh:mm:ss\n".
.type out_buf, @object
out_buf:
    .zero OUT_BUF_LEN
.size out_buf, . - out_buf

.text
// Reads the clock and prints the time of day in UTC.
.type main, @function
main:
    mov x0, #CLOCK_REALTIME
    adrp x1, timespec
    add x1, x1, :lo12:timespec
    bl os_clock
    tbnz x0, #63, .Lmain_clock_failed
    adrp x9, timespec
    add x9, x9, :lo12:timespec
    ldr x10, [x9, #8]
    mov x11, #NANOS_PER_SECOND & 0xffff
    movk x11, #NANOS_PER_SECOND >> 16, lsl #16
    cmp x10, x11
    b.hs .Lmain_assert_nanos
    ldr x0, [x9]
    tbnz x0, #63, .Lmain_assert_seconds
    bl print_time
    tbnz x0, #63, .Lmain_write_failed
    mov w0, #EXIT_OK
    b .Lmain_exit
.Lmain_clock_failed:
    mov w0, #EXIT_CLOCK_FAILED
    b .Lmain_exit
.Lmain_write_failed:
    mov w0, #EXIT_WRITE_FAILED
.Lmain_exit:
    bl os_exit
.Lmain_assert_nanos:
    mov w0, #1
    bl assert_fail
.Lmain_assert_seconds:
    mov w0, #2
    bl assert_fail
.size main, . - main

// Prints the time of day a count of seconds since 1970 says, in UTC, as "hh:mm:ss".
// In: x0 = the seconds, at least 0.
// Out: x0 = 0, or a negative error code.
// returns: result
.type print_time, @function
print_time:
    stp x29, x30, [sp, #-16]!
    adrp x1, out_buf
    add x1, x1, :lo12:out_buf
    mov x9, #SECONDS_PER_DAY & 0xffff
    movk x9, #SECONDS_PER_DAY >> 16, lsl #16
    udiv x10, x0, x9
    msub x11, x10, x9, x0
    mov x9, #SECONDS_PER_HOUR
    udiv x12, x11, x9
    msub x13, x12, x9, x11
    cmp x12, #HOURS_PER_DAY
    b.hs .Lprint_time_assert_hours
    mov x9, #DECIMAL_BASE
    udiv x14, x12, x9
    msub x15, x14, x9, x12
    add w14, w14, #'0'
    strb w14, [x1]
    add w15, w15, #'0'
    strb w15, [x1, #1]
    mov w14, #COLON
    strb w14, [x1, #2]
    mov x9, #SECONDS_PER_MINUTE
    udiv x12, x13, x9
    msub x13, x12, x9, x13
    cmp x12, #MINUTES_PER_HOUR
    b.hs .Lprint_time_assert_minutes
    mov x9, #DECIMAL_BASE
    udiv x14, x12, x9
    msub x15, x14, x9, x12
    add w14, w14, #'0'
    strb w14, [x1, #3]
    add w15, w15, #'0'
    strb w15, [x1, #4]
    mov w14, #COLON
    strb w14, [x1, #5]
    cmp x13, #SECONDS_PER_MINUTE
    b.hs .Lprint_time_assert_seconds
    udiv x14, x13, x9
    msub x15, x14, x9, x13
    add w14, w14, #'0'
    strb w14, [x1, #6]
    add w15, w15, #'0'
    strb w15, [x1, #7]
    mov w14, #NEWLINE
    strb w14, [x1, #8]
    mov x0, #STDOUT
    mov x2, #OUT_BUF_LEN
    bl write_all
    tbnz x0, #63, .Lprint_time_return
    mov x0, #0
.Lprint_time_return:
    ldp x29, x30, [sp], #16
    ret
.Lprint_time_assert_hours:
    mov w0, #3
    bl assert_fail
.Lprint_time_assert_minutes:
    mov w0, #4
    bl assert_fail
.Lprint_time_assert_seconds:
    mov w0, #5
    bl assert_fail
.size print_time, . - print_time

// Writes x2 bytes from x1 to file descriptor x0, retrying partial writes.
// In: x0 = the file descriptor, x1 = the bytes, x2 = how many, at least 1.
// Out: x0 = 0, or a negative error code.
// returns: result
.type write_all, @function
write_all:
    stp x29, x30, [sp, #-16]!
    cbz x1, .Lwrite_all_assert_null
    cbz x2, .Lwrite_all_assert_empty
    mov x9, x0
    mov x10, #MAX_WRITES
.Lwrite_all_loop:  // bound: 64
    mov x0, x9
    bl os_write
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
    ldp x29, x30, [sp], #16
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


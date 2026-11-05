# The x86-64 library: functions a program may call without defining them, written once for every
# operating system because they reach the OS only through the operations (os_write, os_exit, ...).
# src/edge/mod.rs says how this file is read: a line `# == name: needs` starts a snippet, which is
# written when main.s calls `name` or another snippet needs it.

# == assert_fail: os_write os_exit
.section .rodata
.type assert_message, @object
assert_message:
    .ascii "lengua: assertion 000 failed\n"
.size assert_message, . - assert_message

.text
.type assert_fail, @function
# lengua's assertion handler.
# In: edi = the assertion's number, 1 to 999. Prints "lengua: assertion NNN failed" to
# standard error and ends the program, every thread of it, with code 134.
assert_fail:
    sub rsp, 40
    mov rax, qword ptr [rip + assert_message]
    mov qword ptr [rsp], rax
    mov rax, qword ptr [rip + assert_message + 8]
    mov qword ptr [rsp + 8], rax
    mov rax, qword ptr [rip + assert_message + 16]
    mov qword ptr [rsp + 16], rax
    mov eax, dword ptr [rip + assert_message + 24]
    mov dword ptr [rsp + 24], eax
    mov al, byte ptr [rip + assert_message + 28]
    mov byte ptr [rsp + 28], al
    mov eax, edi
    xor edx, edx
    mov ecx, 100
    div ecx
    add al, 48
    mov byte ptr [rsp + 18], al
    mov eax, edx
    xor edx, edx
    mov ecx, 10
    div ecx
    add al, 48
    mov byte ptr [rsp + 19], al
    add dl, 48
    mov byte ptr [rsp + 20], dl
    mov edi, 2
    mov rsi, rsp
    mov edx, 29
    call os_write
    mov edi, 134
    call os_exit
.size assert_fail, . - assert_fail

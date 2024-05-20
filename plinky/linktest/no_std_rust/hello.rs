#![no_std]

fn write(fd: u64, content: &str) {
    let content = content.as_bytes();
    unsafe {
        core::arch::asm!(
            "push rbx",
            "mov rax, 4",
            "mov rbx, {fd}",
            "mov rcx, {ptr}",
            "mov rdx, {len}",
            "int 0x80",
            "pop rbx",
            fd = in(reg) fd,
            ptr = in(reg) content.as_ptr(),
            len = in(reg) content.len(),
            out("rax") _,
            // rbx is used by llvm, so we have to push and pop it rather than clobbering it.
            out("rcx") _,
            out("rdx") _
        );
    }
}

fn exit(code: u64) -> ! {
    unsafe {
        core::arch::asm!(
            "mov rax, 1",
            "mov rbx, {code}",
            "int 0x80",
            code = in(reg) code,
            options(noreturn)
        );
    }
}

#[no_mangle]
pub fn _start() -> ! {
    write(1, "Hello world\n");
    exit(0);
}

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

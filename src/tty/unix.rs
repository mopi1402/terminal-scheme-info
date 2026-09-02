use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, RawFd};
use std::os::raw::c_int;

use super::REPLY_TIMEOUT;

/// `struct termios` and the flags we touch, per platform. Declared by hand so
/// the binary carries no dependency; the layouts are those of the C library.
#[cfg(any(target_os = "macos", target_os = "ios"))]
mod sys {
    pub type Flag = u64;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Termios {
        pub c_iflag: Flag,
        pub c_oflag: Flag,
        pub c_cflag: Flag,
        pub c_lflag: Flag,
        pub c_cc: [u8; 20],
        pub c_ispeed: Flag,
        pub c_ospeed: Flag,
    }

    pub const ECHO: Flag = 0x0000_0008;
    pub const ICANON: Flag = 0x0000_0100;
    pub const ISIG: Flag = 0x0000_0080;
    pub const VMIN: usize = 16;
    pub const VTIME: usize = 17;
}

/// Linux with the generic termios ABI (x86, arm, aarch64, riscv, s390x, ...).
/// mips, powerpc and sparc lay `struct termios` out differently and are not supported.
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "powerpc",
        target_arch = "powerpc64",
        target_arch = "sparc",
        target_arch = "sparc64",
    ))
))]
mod sys {
    pub type Flag = u32;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Termios {
        pub c_iflag: Flag,
        pub c_oflag: Flag,
        pub c_cflag: Flag,
        pub c_lflag: Flag,
        pub c_line: u8,
        pub c_cc: [u8; 32],
        pub c_ispeed: Flag,
        pub c_ospeed: Flag,
    }

    pub const ECHO: Flag = 0o000_010;
    pub const ICANON: Flag = 0o000_002;
    pub const ISIG: Flag = 0o000_001;
    pub const VMIN: usize = 6;
    pub const VTIME: usize = 5;
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    all(
        target_os = "linux",
        not(any(
            target_arch = "mips",
            target_arch = "mips64",
            target_arch = "powerpc",
            target_arch = "powerpc64",
            target_arch = "sparc",
            target_arch = "sparc64",
        ))
    )
)))]
compile_error!("terminal-scheme-info supports macOS, Linux (generic termios ABI) and Windows");

const TCSANOW: c_int = 0;

unsafe extern "C" {
    fn tcgetattr(fd: c_int, termios: *mut sys::Termios) -> c_int;
    fn tcsetattr(fd: c_int, action: c_int, termios: *const sys::Termios) -> c_int;
}

pub struct Tty {
    file: File,
    saved: sys::Termios,
}

impl Tty {
    pub fn open() -> Option<Tty> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .ok()?;
        let fd = file.as_raw_fd();

        let mut saved = MaybeUninit::<sys::Termios>::uninit();
        // SAFETY: `saved` is a valid, writable `struct termios` of the platform layout.
        if unsafe { tcgetattr(fd, saved.as_mut_ptr()) } != 0 {
            return None;
        }
        // SAFETY: tcgetattr returned 0, so it filled the struct.
        let saved = unsafe { saved.assume_init() };

        // No echo (the answer must not show up in the prompt), no line buffering
        // (the answer has no newline), no signals (a Ctrl-C in this window must
        // not kill us before the mode is restored). A read returns as soon as
        // anything arrives, or empty after REPLY_TIMEOUT of silence: the
        // termios timer itself, no poll(), which macOS does not support on /dev/tty.
        let mut raw = saved;
        raw.c_lflag &= !(sys::ECHO | sys::ICANON | sys::ISIG);
        raw.c_cc[sys::VMIN] = 0;
        raw.c_cc[sys::VTIME] = (REPLY_TIMEOUT.as_millis() / 100).clamp(1, 255) as u8;
        // SAFETY: `raw` is a valid `struct termios`.
        if unsafe { tcsetattr(fd, TCSANOW, &raw) } != 0 {
            return None;
        }
        Some(Tty { file, saved })
    }

    fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all(bytes)
    }

    /// Appends whatever the terminal sends next. `Ok(false)` when nothing came
    /// within REPLY_TIMEOUT, or the tty is gone.
    pub fn read(&mut self, buf: &mut Vec<u8>) -> io::Result<bool> {
        let mut chunk = [0u8; 256];
        let n = loop {
            match self.file.read(&mut chunk) {
                Ok(n) => break n,
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(err),
            }
        };
        buf.extend_from_slice(&chunk[..n]);
        Ok(n > 0)
    }
}

impl Drop for Tty {
    fn drop(&mut self) {
        // SAFETY: `saved` is the struct tcgetattr gave us.
        unsafe { tcsetattr(self.fd(), TCSANOW, &self.saved) };
    }
}

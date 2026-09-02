//! Windows console, through kernel32 declared by hand.
//!
//! Not yet verified on a real Windows machine: written against the Win32
//! documentation, compiled blind. Windows Terminal answers OSC 10/11 from 1.22;
//! conhost answers DA1 only, so the sentinel makes it a clean no-op there.

use std::ffi::c_void;
use std::io;

use super::REPLY_TIMEOUT;

type Handle = *mut c_void;
type Bool = i32;
type Dword = u32;

#[repr(C)]
#[derive(Clone, Copy)]
struct KeyEventRecord {
    key_down: Bool,
    repeat_count: u16,
    virtual_key_code: u16,
    virtual_scan_code: u16,
    unicode_char: u16,
    control_key_state: Dword,
}

/// `INPUT_RECORD`: a 16-bit tag, padding, then a 16-byte union of which we
/// only ever read the key event member (the other members are the same size).
#[repr(C)]
#[derive(Clone, Copy)]
struct InputRecord {
    event_type: u16,
    _padding: u16,
    key: KeyEventRecord,
}

const INVALID_HANDLE_VALUE: Handle = usize::MAX as Handle;
const GENERIC_READ: Dword = 0x8000_0000;
const GENERIC_WRITE: Dword = 0x4000_0000;
const FILE_SHARE_READ: Dword = 0x1;
const FILE_SHARE_WRITE: Dword = 0x2;
const OPEN_EXISTING: Dword = 3;

const ENABLE_PROCESSED_INPUT: Dword = 0x0001;
const ENABLE_LINE_INPUT: Dword = 0x0002;
const ENABLE_ECHO_INPUT: Dword = 0x0004;
const ENABLE_VIRTUAL_TERMINAL_INPUT: Dword = 0x0200;
const ENABLE_VIRTUAL_TERMINAL_PROCESSING: Dword = 0x0004;

const WAIT_OBJECT_0: Dword = 0;
const KEY_EVENT: u16 = 0x0001;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: Dword,
        share: Dword,
        security: *const c_void,
        disposition: Dword,
        flags: Dword,
        template: Handle,
    ) -> Handle;
    fn CloseHandle(handle: Handle) -> Bool;
    fn GetConsoleMode(handle: Handle, mode: *mut Dword) -> Bool;
    fn SetConsoleMode(handle: Handle, mode: Dword) -> Bool;
    fn WriteFile(
        handle: Handle,
        buf: *const c_void,
        len: Dword,
        written: *mut Dword,
        overlapped: *mut c_void,
    ) -> Bool;
    fn WaitForSingleObject(handle: Handle, millis: Dword) -> Dword;
    fn ReadConsoleInputW(
        handle: Handle,
        buf: *mut InputRecord,
        len: Dword,
        read: *mut Dword,
    ) -> Bool;
}

struct Console {
    handle: Handle,
    saved_mode: Dword,
}

impl Console {
    fn open(name: &str, mode: impl FnOnce(Dword) -> Dword) -> Option<Console> {
        let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
        // SAFETY: `wide` is a NUL-terminated UTF-16 string; other arguments are plain values.
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut saved_mode = 0;
        // SAFETY: valid handle, valid out pointer.
        if unsafe { GetConsoleMode(handle, &mut saved_mode) } == 0 {
            // SAFETY: the handle is ours to close.
            unsafe { CloseHandle(handle) };
            return None;
        }
        let console = Console { handle, saved_mode };
        // SAFETY: valid handle.
        if unsafe { SetConsoleMode(handle, mode(saved_mode)) } == 0 {
            return None; // `console` drops here and closes the handle
        }
        Some(console)
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        // SAFETY: the handle is ours; restoring the mode we read is always valid.
        unsafe {
            SetConsoleMode(self.handle, self.saved_mode);
            CloseHandle(self.handle);
        }
    }
}

pub struct Tty {
    input: Console,
    output: Console,
}

impl Tty {
    pub fn open() -> Option<Tty> {
        // Bytes in, no echo, no line editing, no Ctrl-C processing.
        let input = Console::open("CONIN$", |mode| {
            (mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT))
                | ENABLE_VIRTUAL_TERMINAL_INPUT
        })?;
        // Escape sequences out, so the query reaches the terminal.
        let output = Console::open("CONOUT$", |mode| mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING)?;
        Some(Tty { input, output })
    }

    pub fn write_all(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            let mut written = 0;
            // SAFETY: valid handle, `bytes` is a live slice, `written` a valid out pointer.
            let ok = unsafe {
                WriteFile(
                    self.output.handle,
                    bytes.as_ptr().cast(),
                    bytes.len() as Dword,
                    &mut written,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 || written == 0 {
                return Err(io::Error::last_os_error());
            }
            bytes = &bytes[written as usize..];
        }
        Ok(())
    }

    /// Appends whatever the console sends next. `Ok(false)` when nothing came
    /// within REPLY_TIMEOUT.
    pub fn read(&mut self, buf: &mut Vec<u8>) -> io::Result<bool> {
        let millis = REPLY_TIMEOUT.as_millis().min(u128::from(Dword::MAX - 1)) as Dword;
        // SAFETY: valid handle.
        if unsafe { WaitForSingleObject(self.input.handle, millis) } != WAIT_OBJECT_0 {
            return Ok(false);
        }
        let mut records = [InputRecord {
            event_type: 0,
            _padding: 0,
            key: KeyEventRecord {
                key_down: 0,
                repeat_count: 0,
                virtual_key_code: 0,
                virtual_scan_code: 0,
                unicode_char: 0,
                control_key_state: 0,
            },
        }; 64];
        let mut read = 0;
        // SAFETY: valid handle, `records` holds `len` writable records, `read` is a valid out pointer.
        let ok = unsafe {
            ReadConsoleInputW(
                self.input.handle,
                records.as_mut_ptr(),
                records.len() as Dword,
                &mut read,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        // With virtual terminal input on, the reply arrives one ASCII byte per
        // key-down record. Anything else (focus, mouse, resize) is dropped.
        for record in &records[..read as usize] {
            if record.event_type == KEY_EVENT
                && record.key.key_down != 0
                && (1..0x80).contains(&record.key.unicode_char)
            {
                buf.push(record.key.unicode_char as u8);
            }
        }
        Ok(true)
    }
}

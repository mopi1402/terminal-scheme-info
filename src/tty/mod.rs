//! The controlling terminal, opened directly (stdin and stdout are usually a
//! pipe when we run inside `$(...)`), switched to a mode where the answer is
//! neither echoed nor line-buffered, and restored on drop.
//!
//! Each platform module exposes the same `Tty` type:
//!
//! - `Tty::open() -> Option<Tty>`: `None` when there is no terminal at all.
//! - `Tty::write_all(&mut self, &[u8]) -> io::Result<()>`
//! - `Tty::read(&mut self, &mut Vec<u8>) -> io::Result<bool>`: appends what has
//!   arrived, `Ok(false)` after `REPLY_TIMEOUT` of silence or at end of file.

use std::time::Duration;

/// How long one read waits for the terminal. A live terminal answers within a
/// few milliseconds, plus one network round trip over SSH; this only bites when
/// the tty is attached to nothing that speaks VT at all.
pub const REPLY_TIMEOUT: Duration = Duration::from_millis(500);

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::Tty;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::Tty;

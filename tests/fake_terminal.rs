//! End-to-end check against a fake terminal.
//!
//! Runs the binary inside a pseudo-terminal, plays the terminal's part (answers
//! OSC 11, OSC 10 and DA1 the way xterm does), and checks what the binary
//! prints, that nothing leaks back into the terminal, that the tty mode is
//! restored, and how long the exchange takes. Unix only: Windows has no pty
//! we could drive from here.

#![cfg(unix)]

use std::ffi::{CStr, CString, c_void};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::raw::{c_char, c_int, c_short};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_terminal-scheme-info");
const ST: &[u8] = b"\x1b\\";
const REQUEST: &[u8] = b"\x1b]11;?\x1b\\\x1b]10;?\x1b\\\x1b[c";

#[cfg(target_os = "linux")]
type Nfds = std::os::raw::c_ulong;
#[cfg(not(target_os = "linux"))]
type Nfds = std::os::raw::c_uint;

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}
const POLLIN: c_short = 0x0001;
const O_RDWR: c_int = 2;
const F_SETFD: c_int = 2;
const FD_CLOEXEC: c_int = 1;

#[cfg_attr(target_os = "linux", link(name = "util"))]
unsafe extern "C" {
    fn openpty(
        master: *mut c_int,
        slave: *mut c_int,
        name: *mut c_char,
        termios: *const c_void,
        winsize: *const c_void,
    ) -> c_int;
    fn ptsname(fd: c_int) -> *mut c_char;
    fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    fn poll(fds: *mut PollFd, nfds: Nfds, timeout: c_int) -> c_int;
    fn setsid() -> c_int;
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn dup2(from: c_int, to: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
}

struct Scenario {
    name: &'static str,
    /// What the terminal answers; `None` for a terminal that never answers.
    replies: Option<Vec<u8>>,
    expected: &'static str,
}

struct Run {
    request: Vec<u8>,
    stdout: String,
    leaked: Vec<u8>,
    finished: bool,
    elapsed: Duration,
    echo_restored: bool,
}

/// A pty pair. `master` is our side (the terminal), `slave` the child's tty.
struct Pty {
    master: File,
    _slave: File, // kept open so the pty does not hang up when the child exits
    slave_path: CString,
}

impl Pty {
    fn open() -> io::Result<Pty> {
        let (mut master, mut slave) = (0, 0);
        // SAFETY: two valid out pointers; the rest may be null.
        if unsafe {
            openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openpty gave us these descriptors, they are ours to own.
        let (master, slave) = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
        for file in [&master, &slave] {
            // SAFETY: plain fcntl on a descriptor we own.
            unsafe { fcntl(file.as_raw_fd(), F_SETFD, FD_CLOEXEC) };
        }
        // SAFETY: ptsname returns a static NUL-terminated string (copied right away).
        let slave_path = unsafe { CStr::from_ptr(ptsname(master.as_raw_fd())) }.to_owned();
        Ok(Pty {
            master,
            _slave: slave,
            slave_path,
        })
    }

    /// Reads what the child's side has sent, waiting at most `timeout`.
    /// Returns `false` when nothing came (or the pty hung up).
    fn read_timeout(&mut self, buf: &mut Vec<u8>, timeout: Duration) -> bool {
        read_timeout(&mut self.master, buf, timeout)
    }

    /// Spawns `sh` with this pty as its controlling terminal, running the
    /// binary then `cat`. `sh` stays the session leader for the whole run: on
    /// BSD systems the tty is revoked when the leader exits, so the binary
    /// itself must not be that leader. stdout is piped back to us as the
    /// shell's `$(...)` would do.
    fn spawn(&self, args: &[&str]) -> io::Result<Child> {
        assert!(!BIN.contains('\''), "binary path must be single-quotable");
        let script = format!("'{BIN}' {}; echo {DONE}; exec cat", args.join(" "));
        let slave_path = self.slave_path.clone();
        let mut command = Command::new("sh");
        command
            .args(["-c", &script])
            .env("TERM", "xterm-256color")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // SAFETY: only async-signal-safe calls, no allocation, between fork and exec.
        unsafe {
            command.pre_exec(move || {
                if setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }
                // A session leader with no controlling terminal acquires the
                // first tty it opens.
                let tty = open(slave_path.as_ptr(), O_RDWR);
                if tty < 0 {
                    return Err(io::Error::last_os_error());
                }
                dup2(tty, 0);
                dup2(tty, 2);
                close(tty);
                Ok(())
            });
        }
        command.spawn()
    }
}

/// Marker `sh` prints once the binary has exited.
const DONE: &str = "__terminal_scheme_info_done__";

fn read_timeout<R: Read + AsRawFd>(from: &mut R, buf: &mut Vec<u8>, timeout: Duration) -> bool {
    let mut pollfd = PollFd {
        fd: from.as_raw_fd(),
        events: POLLIN,
        revents: 0,
    };
    // SAFETY: one valid pollfd.
    if unsafe { poll(&mut pollfd, 1, timeout.as_millis() as c_int) } <= 0 {
        return false;
    }
    let mut chunk = [0u8; 512];
    match from.read(&mut chunk) {
        Ok(n) if n > 0 => {
            buf.extend_from_slice(&chunk[..n]);
            true
        }
        _ => false,
    }
}

fn run(scenario: &Scenario) -> io::Result<Run> {
    let mut pty = Pty::open()?;
    let started = Instant::now();
    let mut child = pty.spawn(&["query", "zsh"])?;
    let mut stdout_pipe = child.stdout.take().expect("stdout is piped");

    let mut request = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    while !request.ends_with(b"\x1b[c") && Instant::now() < deadline {
        pty.read_timeout(&mut request, Duration::from_millis(50));
    }
    if let Some(replies) = &scenario.replies {
        pty.master.write_all(replies)?;
    }

    // Until sh prints the marker, anything coming back from the pty is an echo
    // of our reply leaking into the terminal.
    let mut leaked = Vec::new();
    let mut stdout = Vec::new();
    let done = format!("{DONE}\n").into_bytes();
    let deadline = Instant::now() + Duration::from_millis(1500);
    while !stdout.ends_with(&done) && Instant::now() < deadline {
        pty.read_timeout(&mut leaked, Duration::from_millis(1));
        read_timeout(&mut stdout_pipe, &mut stdout, Duration::from_millis(1));
    }
    let elapsed = started.elapsed();
    let finished = stdout.ends_with(&done);
    if finished {
        stdout.truncate(stdout.len() - done.len());
    }

    // With the tty mode restored, echo is back on: a byte we type comes back
    // (cat is now reading the tty, so the line discipline is live).
    let mut echoed = Vec::new();
    if finished {
        pty.master.write_all(b"x")?;
        pty.read_timeout(&mut echoed, Duration::from_millis(100));
    }
    child.kill()?;
    child.wait()?;

    Ok(Run {
        request,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        leaked,
        finished,
        elapsed,
        echo_restored: echoed == b"x",
    })
}

fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "dark theme, ST terminators",
            replies: Some(
                [
                    b"\x1b]11;rgb:2828/2c2c/3434",
                    ST,
                    b"\x1b]10;rgb:ffff/ffff/ffff",
                    ST,
                    b"\x1b[?62;1;4c",
                ]
                .concat(),
            ),
            expected: "export TERMINAL_BACKGROUND='#282C34'\n\
                       export TERMINAL_FOREGROUND='#FFFFFF'\n\
                       export TERMINAL_COLOR_SCHEME='dark'\n",
        },
        Scenario {
            name: "light theme, BEL terminators",
            replies: Some(
                b"\x1b]11;rgb:fdfd/f6f6/e3e3\x07\x1b]10;rgb:5757/6e6e/7575\x07\x1b[?1;2c".to_vec(),
            ),
            expected: "export TERMINAL_BACKGROUND='#FDF6E3'\n\
                       export TERMINAL_FOREGROUND='#576E75'\n\
                       export TERMINAL_COLOR_SCHEME='light'\n",
        },
        Scenario {
            name: "mute terminal (DA1 only)",
            replies: Some(b"\x1b[?1;0c".to_vec()),
            expected: "",
        },
        Scenario {
            name: "dead terminal (no answer at all)",
            replies: None,
            expected: "",
        },
    ]
}

#[test]
fn behaves_like_a_terminal_would_expect() {
    let mut failures = Vec::new();
    for scenario in scenarios() {
        let run = run(&scenario).unwrap_or_else(|err| panic!("{}: {err}", scenario.name));
        let mut problems = Vec::new();
        if run.request != REQUEST {
            problems.push(format!(
                "unexpected query {:?}",
                run.request.escape_ascii().to_string()
            ));
        }
        if run.stdout != scenario.expected {
            problems.push(format!(
                "stdout {:?}, expected {:?}",
                run.stdout, scenario.expected
            ));
        }
        if !run.leaked.is_empty() {
            problems.push(format!(
                "leaked into the terminal: {:?}",
                run.leaked.escape_ascii().to_string()
            ));
        }
        if !run.finished {
            problems.push("the binary did not exit in time".to_string());
        }
        if !run.echo_restored {
            problems.push("tty mode not restored (no echo after exit)".to_string());
        }
        let ms = run.elapsed.as_millis();
        match scenario.replies {
            None if !(450..=1000).contains(&ms) => {
                problems.push(format!("dead-terminal timeout took {ms} ms"))
            }
            Some(_) if ms > 100 => problems.push(format!("took {ms} ms")),
            _ => {}
        }
        eprintln!(
            "[{}] {}: {ms} ms",
            if problems.is_empty() { "ok" } else { "FAIL" },
            scenario.name
        );
        for problem in &problems {
            eprintln!("       {problem}");
        }
        if !problems.is_empty() {
            failures.push(scenario.name);
        }
    }
    assert!(failures.is_empty(), "failed scenarios: {failures:?}");
}

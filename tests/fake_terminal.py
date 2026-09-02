#!/usr/bin/env python3
"""End-to-end check against a fake terminal.

Runs the binary inside a pseudo-terminal, plays the terminal's part (answers
OSC 11, OSC 10 and DA1 the way xterm does), and checks what the binary prints,
that nothing leaks back into the terminal, that the tty mode is restored, and
how long the exchange takes. Python standard library only.

    python3 tests/fake_terminal.py target/release/terminal-scheme-info
"""
import os
import pty
import select
import sys
import termios
import time

ST = b"\x1b\\"
DA1_REQUEST = b"\x1b[c"

SCENARIOS = [
    # name, terminal replies (None = terminal never answers), expected stdout
    (
        "dark theme, ST terminators",
        b"\x1b]11;rgb:2828/2c2c/3434" + ST + b"\x1b]10;rgb:ffff/ffff/ffff" + ST + b"\x1b[?62;1;4c",
        "export TERMINAL_BACKGROUND='#282C34'\n"
        "export TERMINAL_FOREGROUND='#FFFFFF'\n"
        "export TERMINAL_COLOR_SCHEME='dark'\n",
    ),
    (
        "light theme, BEL terminators",
        b"\x1b]11;rgb:fdfd/f6f6/e3e3\x07\x1b]10;rgb:5757/6e6e/7575\x07\x1b[?1;2c",
        "export TERMINAL_BACKGROUND='#FDF6E3'\n"
        "export TERMINAL_FOREGROUND='#576E75'\n"
        "export TERMINAL_COLOR_SCHEME='light'\n",
    ),
    ("mute terminal (DA1 only)", b"\x1b[?1;0c", ""),
    ("dead terminal (no answer at all)", None, ""),
]


def run(binary, replies):
    out_r, out_w = os.pipe()
    pid, master = pty.fork()
    if pid == 0:  # child: stdout to the pipe, the pty stays the controlling terminal
        os.close(out_r)
        os.dup2(out_w, 1)
        os.close(out_w)
        os.environ["TERM"] = "xterm-256color"
        os.execvp(binary, [binary, "query", "zsh"])
    os.close(out_w)

    started = time.monotonic()
    seen = b""
    while DA1_REQUEST not in seen:
        ready, _, _ = select.select([master], [], [], 2.0)
        if not ready:
            raise SystemExit("the binary never sent its query")
        seen += os.read(master, 1024)
    if replies is not None:
        os.write(master, replies)

    # Anything the pty sends back now would be an echo of our reply: a leak.
    leaked = b""
    status = None
    deadline = time.monotonic() + 1.5
    while status is None and time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], 0.005)
        if ready:
            try:
                leaked += os.read(master, 1024)
            except OSError:
                pass
        done, status = os.waitpid(pid, os.WNOHANG)
        if done == 0:
            status = None
    elapsed = time.monotonic() - started
    if status is None:
        os.kill(pid, 9)
        raise SystemExit("the binary did not exit")
    output = b""
    while True:
        chunk = os.read(out_r, 4096)
        if not chunk:
            break
        output += chunk
    os.close(out_r)
    try:
        echo_restored = bool(termios.tcgetattr(master)[3] & termios.ECHO)
    except termios.error:
        echo_restored = None  # the slave is gone, nothing to check
    os.close(master)
    return seen, output.decode(), leaked, os.waitstatus_to_exitcode(status), elapsed, echo_restored


def main():
    binary = sys.argv[1] if len(sys.argv) > 1 else "target/release/terminal-scheme-info"
    failures = 0
    for name, replies, expected in SCENARIOS:
        seen, output, leaked, code, elapsed, echo_restored = run(binary, replies)
        problems = []
        if seen != b"\x1b]11;?" + ST + b"\x1b]10;?" + ST + DA1_REQUEST:
            problems.append(f"unexpected query {seen!r}")
        if output != expected:
            problems.append(f"stdout {output!r}, expected {expected!r}")
        if leaked:
            problems.append(f"leaked into the terminal: {leaked!r}")
        if code != 0:
            problems.append(f"exit code {code}")
        if echo_restored is False:
            problems.append("ECHO not restored on the tty")
        if replies is None and not 0.45 <= elapsed <= 1.0:
            problems.append(f"dead-terminal timeout took {elapsed * 1000:.0f} ms")
        if replies is not None and elapsed > 0.1:
            problems.append(f"took {elapsed * 1000:.0f} ms")
        status = "ok" if not problems else "FAIL"
        print(f"[{status}] {name}: {elapsed * 1000:.1f} ms")
        for problem in problems:
            print(f"       {problem}")
        failures += bool(problems)
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()

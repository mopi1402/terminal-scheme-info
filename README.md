# terminal-scheme-info

A terminal has no alpha channel. A program that wants to draw translucent
bands, washes or dimmed backgrounds has to blend every shade against the real
background colour, and today it has no simple way to learn it. The terminal
does tell it, through OSC 11 (and OSC 10 for the foreground), but only the
process that owns the tty can ask and read the answer. A subprocess, a hook, a
script run by another tool cannot.

`terminal-scheme-info` asks once, when the shell starts, and hands the answer
to the whole session as environment variables, the way `TERM`, `COLORTERM` and
`COLORFGBG` already are:

```sh
TERMINAL_BACKGROUND='#282C34'
TERMINAL_FOREGROUND='#FFFFFF'
TERMINAL_COLOR_SCHEME='dark'
```

Everything started in that session inherits them and reads three variables
without knowing anything about terminals.

- One static binary, no dependencies, a couple of milliseconds all in.
- Linux, macOS, Windows (Windows Terminal 1.22 and later answers OSC 11).
- Silent when there is no terminal (scripts, cron, `TERM=dumb`) or when the
  terminal does not answer, so nothing ever leaks into the prompt.

## Install

Download the binary for your platform from the
[releases page](https://github.com/mopi1402/terminal-scheme-info/releases)
(Linux x86_64 and aarch64, static; macOS Intel and Apple silicon; Windows
x86_64 and arm64), put it somewhere on your `PATH`, or build it yourself:

```sh
cargo install --path .
```

Then add the line to your shell startup file:

```sh
terminal-scheme-info install            # detects the shell from $SHELL
terminal-scheme-info install fish       # or name it: zsh, bash, fish, powershell
terminal-scheme-info install zsh ~/.config/zsh/.zshrc   # or name the file too
```

`uninstall` takes the same arguments and removes the block again. If you
manage the file yourself, `terminal-scheme-info init <shell>` prints the
line(s) to add:

| Shell      | Startup file                                          | Line                                             |
| ---------- | ----------------------------------------------------- | ------------------------------------------------ |
| zsh        | `$ZDOTDIR/.zshrc`, else `~/.zshrc`                    | `eval "$(terminal-scheme-info query zsh)"`       |
| bash       | `~/.bashrc`                                           | `eval "$(terminal-scheme-info query bash)"`      |
| fish       | `$XDG_CONFIG_HOME/fish/config.fish`                   | `terminal-scheme-info query fish \| source`      |
| PowerShell | `$PROFILE` (PowerShell 7, current user, current host) | three lines, see `init powershell`               |

On macOS, bash login shells read `~/.bash_profile`, not `~/.bashrc`; make sure
the former sources the latter.

## How it works

The binary opens the controlling terminal directly (`/dev/tty`, or `CONIN$` /
`CONOUT$` on Windows), turns echo and line buffering off, and writes three
queries in one go: OSC 11 (background), OSC 10 (foreground) and DA1 (primary
device attributes). Every VT terminal answers DA1, and answers come back in
order, so the DA1 reply is a sentinel: once it is in, the colour replies are
either in as well or never coming. A terminal that does not know OSC 11 costs
one round trip, not a timeout. A 500 ms timeout remains for a tty attached to
nothing that speaks VT at all.

`TERMINAL_COLOR_SCHEME` is `dark` when the background's CIE lightness (L*)
is below 50, `light` otherwise. Without a background answer it falls back to
the inverted foreground.

## Terminals

One real query in each terminal: macOS from a developer machine, Linux and
Windows from the `terminals` workflow on GitHub's runners. The time is the
whole exchange and mostly measures how fast the terminal answers.

### macOS 26.2

| Terminal | Version | Answers OSC 10/11 | Time |
| --- | --- | --- | --- |
| Alacritty | 0.17.0 | yes | 3 ms |
| Tabby | 1.0.235 | yes | 5 ms |
| Ghostty | 1.3.1 | yes | 7 ms |
| WezTerm | 20240203-110809 | yes | 7 ms |
| Terminal.app | 2.15 | yes | 10 to 26 ms |
| iTerm2 | 3.6.11 | yes | 21 ms |
| kitty | 0.48.2 | yes | 63 ms |
| Rio | 0.5.27 | yes | 73 ms |
| Hyper | 3.4.1 | yes | 168 ms |
| Contour | 0.7.0 | yes | 310 ms |
| Wave | 0.14.5 | not checked: it replaces ZDOTDIR, the script cannot drive it | |

### Linux

Ubuntu 24.04.4 LTS, GitHub runner, Xvfb with software rendering
(`terminals` workflow, run 33684398426).

| Terminal | Version | Answers OSC 10/11 | Time |
| --- | --- | --- | --- |
| xterm | 390 | yes | 2 ms |
| Konsole | 23.08.5 | yes | 2 ms |
| rxvt-unicode | 9.31 | yes | 2 ms |
| st (suckless) | 0.9 | yes | 2 ms |
| LXTerminal | 0.4.0 | yes | 2 ms |
| WezTerm | 20240203-110809 | yes | 5 ms |
| GNOME Terminal | 3.52.0 | yes | 9 ms |
| MATE Terminal | 1.26.1 | yes | 10 ms |
| Terminator | 2.1.3 | yes | 16 ms |
| xfce4-terminal | 1.1.3 | yes | 22 ms |
| Alacritty | 0.13.2 | yes | 140 ms (software OpenGL) |
| Tilix | 1.9.6 | yes | 199 ms |
| QTerminal | 1.4.0 | no: answers DA1 only, the variables stay unset | 29 ms |
| kitty | 0.32.2 | no answer within the 500 ms timeout under Xvfb; answers in 63 ms on macOS | 502 ms |

### Windows

Windows Server 2025 (10.0.26100), GitHub runner, `terminals` workflow
(run 33686011217). Windows Terminal is the portable build of the latest
release.

| Terminal | Version | Answers OSC 10/11 | Time |
| --- | --- | --- | --- |
| Windows Terminal | 1.24 | yes | 21 ms |
| conhost (legacy console) | 10.0.26100 | no: answers DA1 only, the variables stay unset | 32 ms |
| mintty (Git for Windows) | 3.8.3 | no: silent | 13 ms |
| Alacritty, WezTerm | Chocolatey packages | did not start on the runner | |

### Multiplexers

tmux and screen answer DA1 but not OSC 10/11: inside them the variables stay
unset. Start the multiplexer from a shell that already has them and they are
inherited.

## Limits

- The values are those of the terminal when the shell started. A theme change
  is seen by the next session.
- Keys typed during the few milliseconds of the query are consumed by it.

## Development

```sh
cargo build --release
cargo test
```

`cargo test` runs the unit tests and `tests/fake_terminal.rs`, an end-to-end
check in a pseudo-terminal: it plays the terminal's part, answers the queries
like xterm would, and checks the output, that nothing is echoed back, that the
tty mode is restored, and how long the exchange takes (Unix only).

`scripts/try-terminals.sh` (macOS), `scripts/try-terminals-linux.sh` and
`scripts/try-terminals.ps1` (Windows) run one real query in every installed
terminal emulator and print the tables above; the `terminals` workflow runs
the Linux and Windows ones on GitHub's runners.

The Windows implementation is exercised for real by the `terminals` workflow
(Windows Terminal on a Windows Server 2025 runner); the pty test itself is
Unix only.

## License

MIT

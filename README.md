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

Checked with a real query, on macOS, through `scripts/try-terminals.sh`
(the time is the whole exchange, dominated by how fast the terminal answers):

| Terminal | Answers OSC 10/11 | Time |
| --- | --- | --- |
| Alacritty 0.17 | yes | 3 ms |
| Tabby 1.0 | yes | 5 ms |
| Ghostty | yes | 7 ms |
| WezTerm 20240203 | yes | 7 ms |
| Terminal.app (macOS 26) | yes | 10 ms |
| iTerm2 | yes | 21 ms |
| kitty 0.48 | yes | 63 ms |
| Rio 0.5 | yes | 73 ms |
| Hyper 3.4 | yes | 168 ms |
| Contour 0.7 | yes | 310 ms |
| Windows Terminal 1.22+ | yes, per its release notes | not yet measured |
| Wave | not yet checked (it replaces ZDOTDIR, the script cannot drive it) | |
| tmux, screen | no: DA1 only, the variables stay unset inside | |

## Limits

- The values are those of the terminal when the shell started. A theme change
  is seen by the next session.
- Multiplexers (tmux, screen) answer DA1 but usually not OSC 11: inside them
  the variables are not set. Start the multiplexer from a shell that already
  has them and they are inherited.
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

`scripts/try-terminals.sh` (macOS) runs one real query in every installed
terminal emulator and prints what each one answered; that is where the table
above comes from.

The Windows implementation is written against the Win32 documentation and has
not yet been run on a Windows machine.

## License

MIT

//! terminal-scheme-info: ask the terminal for its colours once, at shell
//! startup, and hand them to the session as environment variables.

mod color;
mod query;
mod shell;
mod tty;

use std::path::PathBuf;
use std::process::ExitCode;

use shell::{Outcome, Shell};

const NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
Expose the terminal's colours to the shell session as environment variables.

Usage:
  terminal-scheme-info [query [SHELL]]         print the assignments for SHELL (default: bash/zsh syntax)
  terminal-scheme-info init [SHELL]            print the line(s) to put in the shell startup file
  terminal-scheme-info install [SHELL] [FILE]  add them to the startup file (default: the shell's own)
  terminal-scheme-info uninstall [SHELL] [FILE]
  terminal-scheme-info --help | --version

SHELL is one of zsh, bash, fish, powershell; detected from $SHELL when omitted.

Variables:
  TERMINAL_BACKGROUND    #RRGGBB
  TERMINAL_FOREGROUND    #RRGGBB
  TERMINAL_COLOR_SCHEME  dark | light

`query` prints nothing and exits 0 when there is no terminal, TERM is dumb,
or the terminal does not answer OSC 10/11. The values are those of the
terminal when the shell started; a theme change is seen by the next session.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut args = args.iter().map(String::as_str);
    let result = match args.next() {
        None | Some("query") => query(args.next()),
        Some("init") => init(args.next()),
        Some("install") => install(args.next(), args.next()),
        Some("uninstall") => uninstall(args.next(), args.next()),
        Some("-h" | "--help" | "help") => {
            println!("{NAME} {VERSION}\n{USAGE}");
            Ok(())
        }
        Some("-V" | "--version") => {
            println!("{NAME} {VERSION}");
            Ok(())
        }
        Some(other) => Err(format!("unknown command `{other}`\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{NAME}: {message}");
            ExitCode::from(2)
        }
    }
}

fn query(shell: Option<&str>) -> Result<(), String> {
    let shell = match shell {
        Some(name) => parse_shell(name)?,
        None => Shell::Bash,
    };
    if let Some(palette) = query::query() {
        print!("{}", shell.render(&palette));
    }
    Ok(())
}

fn init(shell: Option<&str>) -> Result<(), String> {
    print!("{}", resolve_shell(shell)?.init_snippet());
    Ok(())
}

fn install(shell: Option<&str>, file: Option<&str>) -> Result<(), String> {
    let shell = resolve_shell(shell)?;
    let file = resolve_file(shell, file)?;
    match shell::install(shell, &file).map_err(|err| format!("{}: {err}", file.display()))? {
        Outcome::Changed => println!(
            "Added to {}. Open a new terminal to use it.",
            file.display()
        ),
        Outcome::Unchanged => println!("Already present in {}.", file.display()),
    }
    Ok(())
}

fn uninstall(shell: Option<&str>, file: Option<&str>) -> Result<(), String> {
    let shell = resolve_shell(shell)?;
    let file = resolve_file(shell, file)?;
    match shell::uninstall(&file).map_err(|err| format!("{}: {err}", file.display()))? {
        Outcome::Changed => println!("Removed from {}.", file.display()),
        Outcome::Unchanged => println!("Nothing to remove in {}.", file.display()),
    }
    Ok(())
}

fn parse_shell(name: &str) -> Result<Shell, String> {
    Shell::parse(name).ok_or_else(|| format!("unknown shell `{name}` (expected {})", Shell::NAMES))
}

fn resolve_shell(name: Option<&str>) -> Result<Shell, String> {
    match name {
        Some(name) => parse_shell(name),
        None => Shell::detect().ok_or_else(|| {
            format!(
                "cannot detect the shell from $SHELL; pass one of {}",
                Shell::NAMES
            )
        }),
    }
}

fn resolve_file(shell: Shell, file: Option<&str>) -> Result<PathBuf, String> {
    match file {
        Some(path) => Ok(PathBuf::from(path)),
        None => shell.rc_path().ok_or_else(|| {
            "cannot locate the home directory; pass the startup file explicitly".to_string()
        }),
    }
}

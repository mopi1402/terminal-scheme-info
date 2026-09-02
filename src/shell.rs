//! Everything shell-specific: how a variable is assigned, where the startup
//! file lives, the line to put in it, and putting it there (and taking it out).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::query::Palette;

const BIN: &str = env!("CARGO_PKG_NAME");
const MARK_BEGIN: &str = "# >>> terminal-scheme-info >>>";
const MARK_END: &str = "# <<< terminal-scheme-info <<<";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // PowerShell is the shell's name
pub enum Shell {
    Zsh,
    Bash,
    Fish,
    PowerShell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Changed,
    Unchanged,
}

impl Shell {
    pub const NAMES: &'static str = "zsh, bash, fish, powershell";

    pub fn parse(name: &str) -> Option<Shell> {
        match name {
            "zsh" => Some(Shell::Zsh),
            "bash" => Some(Shell::Bash),
            "fish" => Some(Shell::Fish),
            "powershell" | "pwsh" => Some(Shell::PowerShell),
            _ => None,
        }
    }

    /// From `$SHELL`; PowerShell on Windows when that is unset.
    pub fn detect() -> Option<Shell> {
        let from_env = std::env::var_os("SHELL")
            .and_then(|shell| Shell::parse(Path::new(&shell).file_name()?.to_str()?));
        from_env.or(if cfg!(windows) {
            Some(Shell::PowerShell)
        } else {
            None
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Shell::Zsh => "zsh",
            Shell::Bash => "bash",
            Shell::Fish => "fish",
            Shell::PowerShell => "powershell",
        }
    }

    /// One exported assignment, newline-terminated. `value` is always one of
    /// our own `#RRGGBB` or `dark`/`light` strings, so single quotes are safe.
    fn assignment(self, name: &str, value: &str) -> String {
        match self {
            Shell::Zsh | Shell::Bash => format!("export {name}='{value}'\n"),
            Shell::Fish => format!("set -gx {name} '{value}'\n"),
            Shell::PowerShell => format!("$env:{name} = '{value}'\n"),
        }
    }

    /// The assignments for the whole palette, ready to be evaluated.
    pub fn render(self, palette: &Palette) -> String {
        let mut out = String::new();
        if let Some(bg) = palette.background {
            out.push_str(&self.assignment("TERMINAL_BACKGROUND", &bg.hex()));
        }
        if let Some(fg) = palette.foreground {
            out.push_str(&self.assignment("TERMINAL_FOREGROUND", &fg.hex()));
        }
        if let Some(scheme) = palette.scheme() {
            out.push_str(&self.assignment("TERMINAL_COLOR_SCHEME", scheme.as_str()));
        }
        out
    }

    /// What the startup file has to run, without the markers.
    pub fn init_snippet(self) -> String {
        match self {
            Shell::Zsh | Shell::Bash => format!("eval \"$({BIN} query {})\"\n", self.name()),
            Shell::Fish => format!("{BIN} query fish | source\n"),
            Shell::PowerShell => format!(
                "$__tsi = & {BIN} query powershell\n\
                 if ($__tsi) {{ Invoke-Expression ($__tsi -join \"`n\") }}\n\
                 Remove-Variable __tsi\n"
            ),
        }
    }

    /// The startup file, following each shell's conventions:
    /// `$ZDOTDIR/.zshrc`, `~/.bashrc`, `$XDG_CONFIG_HOME/fish/config.fish`,
    /// PowerShell 7's `$PROFILE` (CurrentUserCurrentHost).
    pub fn rc_path(self) -> Option<PathBuf> {
        let home = home_dir()?;
        let path = match self {
            Shell::Zsh => std::env::var_os("ZDOTDIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.clone())
                .join(".zshrc"),
            Shell::Bash => home.join(".bashrc"),
            Shell::Fish => config_dir(&home).join("fish").join("config.fish"),
            Shell::PowerShell if cfg!(windows) => home
                .join("Documents")
                .join("PowerShell")
                .join("Microsoft.PowerShell_profile.ps1"),
            Shell::PowerShell => config_dir(&home)
                .join("powershell")
                .join("Microsoft.PowerShell_profile.ps1"),
        };
        Some(path)
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

fn config_dir(home: &Path) -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
}

/// Appends the marked block to `file` (created if needed). Idempotent.
pub fn install(shell: Shell, file: &Path) -> io::Result<Outcome> {
    let current = read_or_empty(file)?;
    if current.contains(MARK_BEGIN) {
        return Ok(Outcome::Unchanged);
    }
    let mut next = current;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&format!(
        "{MARK_BEGIN}\n{}{MARK_END}\n",
        shell.init_snippet()
    ));
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(file, next)?;
    Ok(Outcome::Changed)
}

/// Removes the marked block from `file`, markers included. Idempotent.
pub fn uninstall(file: &Path) -> io::Result<Outcome> {
    let current = read_or_empty(file)?;
    let mut next = String::with_capacity(current.len());
    let mut inside = false;
    for line in current.split_inclusive('\n') {
        match line.trim_end() {
            MARK_BEGIN => inside = true,
            MARK_END => inside = false,
            _ if !inside => next.push_str(line),
            _ => {}
        }
    }
    if next == current {
        return Ok(Outcome::Unchanged);
    }
    fs::write(file, next)?;
    Ok(Outcome::Changed)
}

fn read_or_empty(file: &Path) -> io::Result<String> {
    match fs::read_to_string(file) {
        Ok(content) => Ok(content),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb;

    fn palette() -> Palette {
        Palette {
            background: Rgb::parse("#282C34"),
            foreground: Rgb::parse("#FFFFFF"),
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tsi-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn renders_each_dialect() {
        assert_eq!(
            Shell::Zsh.render(&palette()),
            "export TERMINAL_BACKGROUND='#282C34'\nexport TERMINAL_FOREGROUND='#FFFFFF'\nexport TERMINAL_COLOR_SCHEME='dark'\n"
        );
        assert_eq!(
            Shell::Fish.render(&palette()),
            "set -gx TERMINAL_BACKGROUND '#282C34'\nset -gx TERMINAL_FOREGROUND '#FFFFFF'\nset -gx TERMINAL_COLOR_SCHEME 'dark'\n"
        );
        assert_eq!(
            Shell::PowerShell.render(&palette()),
            "$env:TERMINAL_BACKGROUND = '#282C34'\n$env:TERMINAL_FOREGROUND = '#FFFFFF'\n$env:TERMINAL_COLOR_SCHEME = 'dark'\n"
        );
        assert_eq!(Shell::Bash.render(&Palette::default()), "");
    }

    #[test]
    fn install_appends_once_and_uninstall_restores() {
        let file = scratch("zshrc");
        fs::write(&file, "alias ll='ls -l'").unwrap(); // no trailing newline on purpose

        assert_eq!(install(Shell::Zsh, &file).unwrap(), Outcome::Changed);
        assert_eq!(install(Shell::Zsh, &file).unwrap(), Outcome::Unchanged);
        let installed = fs::read_to_string(&file).unwrap();
        assert_eq!(
            installed,
            "alias ll='ls -l'\n# >>> terminal-scheme-info >>>\neval \"$(terminal-scheme-info query zsh)\"\n# <<< terminal-scheme-info <<<\n"
        );

        assert_eq!(uninstall(&file).unwrap(), Outcome::Changed);
        assert_eq!(uninstall(&file).unwrap(), Outcome::Unchanged);
        assert_eq!(fs::read_to_string(&file).unwrap(), "alias ll='ls -l'\n");
    }

    #[test]
    fn install_creates_missing_file_and_directories() {
        let file = scratch("nested/dir/config.fish");
        let _ = fs::remove_dir_all(file.parent().unwrap().parent().unwrap());
        assert_eq!(install(Shell::Fish, &file).unwrap(), Outcome::Changed);
        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains("terminal-scheme-info query fish | source\n"));
        assert_eq!(uninstall(&file).unwrap(), Outcome::Changed);
        assert_eq!(fs::read_to_string(&file).unwrap(), "");
    }

    #[test]
    fn uninstall_leaves_a_missing_file_alone() {
        let file = scratch("never-existed");
        let _ = fs::remove_file(&file);
        assert_eq!(uninstall(&file).unwrap(), Outcome::Unchanged);
        assert!(!file.exists());
    }

    #[test]
    fn parses_shell_names() {
        assert_eq!(Shell::parse("pwsh"), Some(Shell::PowerShell));
        assert_eq!(Shell::parse("tcsh"), None);
    }
}

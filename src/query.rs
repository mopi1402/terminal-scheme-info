//! The exchange with the terminal: ask for the colours, wait for the answers,
//! stop as soon as the terminal has said everything it will say.

use std::time::Instant;

use crate::color::{Rgb, Scheme};
use crate::tty::{REPLY_TIMEOUT, Tty};

/// OSC 11 (background), OSC 10 (foreground), then DA1 as a sentinel. Every VT
/// terminal answers DA1, and answers arrive in order, so once the DA1 reply is
/// in we know the colour replies are either in as well or never coming.
const REQUEST: &[u8] = b"\x1b]11;?\x1b\\\x1b]10;?\x1b\\\x1b[c";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Palette {
    pub background: Option<Rgb>,
    pub foreground: Option<Rgb>,
}

impl Palette {
    /// Decided from the background; failing that, from the foreground inverted.
    pub fn scheme(&self) -> Option<Scheme> {
        self.background
            .map(|bg| bg.scheme())
            .or_else(|| self.foreground.map(|fg| fg.scheme().inverse()))
    }

    fn parse(reply: &[u8]) -> Option<Palette> {
        let palette = Palette {
            background: osc_color(reply, b"11"),
            foreground: osc_color(reply, b"10"),
        };
        (palette.background.is_some() || palette.foreground.is_some()).then_some(palette)
    }
}

/// `None` when there is no terminal to ask or it did not answer.
pub fn query() -> Option<Palette> {
    if std::env::var_os("TERM").is_some_and(|term| term == "dumb") {
        return None;
    }
    let mut tty = Tty::open()?;
    tty.write_all(REQUEST).ok()?;

    // Each read waits REPLY_TIMEOUT for the terminal; the deadline caps the
    // whole exchange should something keep the line busy (someone typing).
    let deadline = Instant::now() + 2 * REPLY_TIMEOUT;
    let mut reply = Vec::with_capacity(128);
    while !has_da1_reply(&reply)
        && Instant::now() < deadline
        && tty.read(&mut reply).unwrap_or(false)
    {}
    Palette::parse(&reply)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The colour carried by the `ESC ] <code> ; <spec> (BEL | ST)` reply, if present.
fn osc_color(reply: &[u8], code: &[u8]) -> Option<Rgb> {
    let prefix = [b"\x1b]", code, b";"].concat();
    let start = find(reply, &prefix)? + prefix.len();
    let spec = &reply[start..];
    let end = spec.iter().position(|&b| b == 0x07 || b == 0x1b)?;
    Rgb::parse(std::str::from_utf8(&spec[..end]).ok()?)
}

/// A complete `ESC [ ? <params> c` is somewhere in the buffer.
fn has_da1_reply(reply: &[u8]) -> bool {
    let mut rest = reply;
    while let Some(i) = find(rest, b"\x1b[?") {
        let params = &rest[i + 3..];
        let n = params
            .iter()
            .take_while(|&&b| b.is_ascii_digit() || b == b';')
            .count();
        match params.get(n) {
            Some(b'c') => return true,
            None => return false,
            Some(_) => rest = &params[n..],
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(hex: &str) -> Option<Rgb> {
        Rgb::parse(hex)
    }

    #[test]
    fn parses_a_full_reply_with_st_terminators() {
        let reply =
            b"\x1b]11;rgb:2828/2c2c/3434\x1b\\\x1b]10;rgb:ffff/ffff/ffff\x1b\\\x1b[?62;1;4c";
        assert!(has_da1_reply(reply));
        let palette = Palette::parse(reply).unwrap();
        assert_eq!(palette.background, rgb("#282C34"));
        assert_eq!(palette.foreground, rgb("#FFFFFF"));
        assert_eq!(palette.scheme(), Some(Scheme::Dark));
    }

    #[test]
    fn parses_bel_terminators_and_any_order() {
        let reply = b"\x1b]10;rgb:0000/0000/0000\x07\x1b]11;rgb:fdfd/f6f6/e3e3\x07\x1b[?1;2c";
        let palette = Palette::parse(reply).unwrap();
        assert_eq!(palette.background, rgb("#FDF6E3"));
        assert_eq!(palette.foreground, rgb("#000000"));
        assert_eq!(palette.scheme(), Some(Scheme::Light));
    }

    #[test]
    fn mute_terminal_answers_only_da1() {
        let reply = b"\x1b[?1;0c";
        assert!(has_da1_reply(reply));
        assert_eq!(Palette::parse(reply), None);
    }

    #[test]
    fn partial_da1_keeps_waiting() {
        assert!(!has_da1_reply(
            b"\x1b]11;rgb:2828/2c2c/3434\x1b\\\x1b[?62;1"
        ));
        assert!(!has_da1_reply(b"\x1b]11;rgb:2828/2c2c/3434\x1b\\"));
        assert!(!has_da1_reply(b""));
    }

    #[test]
    fn unterminated_colour_is_not_a_colour() {
        assert_eq!(osc_color(b"\x1b]11;rgb:2828/2c2c/34", b"11"), None);
    }

    #[test]
    fn scheme_falls_back_to_the_inverted_foreground() {
        let palette = Palette::parse(b"\x1b]10;rgb:ffff/ffff/ffff\x07\x1b[?1c").unwrap();
        assert_eq!(palette.background, None);
        assert_eq!(palette.scheme(), Some(Scheme::Dark));
    }
}

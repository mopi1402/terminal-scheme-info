//! Colour values as terminals report them, and the dark/light decision.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    Dark,
    Light,
}

impl Scheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Scheme::Dark => "dark",
            Scheme::Light => "light",
        }
    }

    pub fn inverse(self) -> Scheme {
        match self {
            Scheme::Dark => Scheme::Light,
            Scheme::Light => Scheme::Dark,
        }
    }
}

impl Rgb {
    /// Parses the colour specifications a terminal may answer with (X11 syntax):
    /// `rgb:RRRR/GGGG/BBBB` with 1 to 4 hex digits per channel, the `rgba:` variant
    /// (alpha ignored), and `#RGB` / `#RRGGBB` / `#RRRGGGBBB` / `#RRRRGGGGBBBB`.
    pub fn parse(spec: &str) -> Option<Rgb> {
        let spec = spec.trim();
        if !spec.is_ascii() {
            return None;
        }
        if let Some(hex) = spec.strip_prefix('#') {
            let width = hex.len() / 3;
            if hex.len() % 3 != 0 || width == 0 {
                return None;
            }
            return Some(Rgb {
                r: channel(&hex[..width])?,
                g: channel(&hex[width..2 * width])?,
                b: channel(&hex[2 * width..])?,
            });
        }
        let body = spec
            .strip_prefix("rgb:")
            .or_else(|| spec.strip_prefix("rgba:"))?;
        let mut parts = body.split('/');
        let rgb = Rgb {
            r: channel(parts.next()?)?,
            g: channel(parts.next()?)?,
            b: channel(parts.next()?)?,
        };
        Some(rgb)
    }

    /// `#RRGGBB`, upper case.
    pub fn hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    /// Dark below a CIE L* of 50, light at or above it.
    pub fn scheme(&self) -> Scheme {
        if self.lightness() < 50.0 {
            Scheme::Dark
        } else {
            Scheme::Light
        }
    }

    /// CIE L* (0 = black, 100 = white), from the sRGB channels.
    fn lightness(&self) -> f64 {
        fn linear(channel: u8) -> f64 {
            let c = f64::from(channel) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        let y = 0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b);
        if y <= 0.008856 {
            903.3 * y
        } else {
            116.0 * y.cbrt() - 16.0
        }
    }
}

/// One channel of 1 to 4 hex digits, scaled to 8 bits.
fn channel(digits: &str) -> Option<u8> {
    let width = digits.len();
    if width == 0 || width > 4 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    let max = (1u32 << (4 * width)) - 1;
    Some(((value * 255 + max / 2) / max) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_x11_four_digit_channels() {
        assert_eq!(Rgb::parse("rgb:2828/2c2c/3434").unwrap().hex(), "#282C34");
        assert_eq!(Rgb::parse("rgb:ffff/ffff/ffff").unwrap().hex(), "#FFFFFF");
        assert_eq!(Rgb::parse("rgb:0000/0000/0000").unwrap().hex(), "#000000");
    }

    #[test]
    fn parses_short_channels_and_hash_forms() {
        assert_eq!(Rgb::parse("rgb:f/0/8").unwrap().hex(), "#FF0088");
        assert_eq!(Rgb::parse("rgb:28/2c/34").unwrap().hex(), "#282C34");
        assert_eq!(Rgb::parse("rgb:fff/000/888").unwrap().hex(), "#FF0088");
        assert_eq!(Rgb::parse("#282c34").unwrap().hex(), "#282C34");
        assert_eq!(Rgb::parse("#f08").unwrap().hex(), "#FF0088");
        assert_eq!(
            Rgb::parse("rgba:1d1d/1f1f/2121/ffff").unwrap().hex(),
            "#1D1F21"
        );
    }

    #[test]
    fn rejects_garbage() {
        for bad in [
            "",
            "rgb:",
            "rgb:12/34",
            "rgb:12/34/zz",
            "rgb:12345/1/1",
            "#12345",
            "#",
            "rgb:é/0/0",
        ] {
            assert_eq!(Rgb::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn classifies_common_backgrounds() {
        let dark = ["#000000", "#282C34", "#002B36", "#1D1F21", "#2E3440"];
        let light = ["#FFFFFF", "#FDF6E3", "#EFF1F5", "#F8F8F8"];
        for hex in dark {
            assert_eq!(Rgb::parse(hex).unwrap().scheme(), Scheme::Dark, "{hex}");
        }
        for hex in light {
            assert_eq!(Rgb::parse(hex).unwrap().scheme(), Scheme::Light, "{hex}");
        }
    }
}

//! Named + hex colors for terminal emphasis.
//!
//! Colors ride on border/line/arrow cells only — text stays default for
//! readability. ANSI codes are emitted at render time, after layout, so they
//! never participate in width math.

use serde::de::{self, Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

/// A terminal color: one of the 16 ANSI base colors (theme-tuned, safe on
/// light and dark backgrounds) or a 24-bit RGB value (truecolor terminals).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    /// 24-bit RGB, emitted as `38;2;r;g;b` (truecolor terminals).
    Hex(u8, u8, u8),
}

/// Common aliases that have no direct ANSI-16 slot map to fixed RGB values.
const ORANGE: (u8, u8, u8) = (255, 140, 0);
const PURPLE: (u8, u8, u8) = (160, 32, 240);
const BROWN: (u8, u8, u8) = (165, 105, 30);

impl Color {
    /// Parses a color name or `#rrggbb` / `#rgb` hex string.
    ///
    /// Accepts the 16 ANSI names, common aliases (`grey`/`gray`,
    /// `brightblack`, `orange`, `purple`, `brown`), and hex literals.
    /// Unknown names return `None` (color silently ignored, matching how
    /// unparseable `classDef` props are treated today).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some(hex) = s.strip_prefix('#') {
            return parse_hex(hex);
        }
        match s.to_ascii_lowercase().as_str() {
            "black" => Some(Self::Black),
            "red" => Some(Self::Red),
            "green" => Some(Self::Green),
            "yellow" => Some(Self::Yellow),
            "blue" => Some(Self::Blue),
            "magenta" => Some(Self::Magenta),
            "cyan" => Some(Self::Cyan),
            "white" => Some(Self::White),
            "grey" | "gray" | "brightblack" => Some(Self::BrightBlack),
            "brightred" => Some(Self::BrightRed),
            "brightgreen" => Some(Self::BrightGreen),
            "brightyellow" => Some(Self::BrightYellow),
            "brightblue" => Some(Self::BrightBlue),
            "brightmagenta" => Some(Self::BrightMagenta),
            "brightcyan" => Some(Self::BrightCyan),
            "brightwhite" => Some(Self::BrightWhite),
            "orange" => Some(Self::Hex(ORANGE.0, ORANGE.1, ORANGE.2)),
            "purple" | "violet" => Some(Self::Hex(PURPLE.0, PURPLE.1, PURPLE.2)),
            "brown" => Some(Self::Hex(BROWN.0, BROWN.1, BROWN.2)),
            _ => None,
        }
    }

    /// ANSI SGR foreground parameters for this color, e.g. `"31"` or
    /// `"38;2;255;140;0"`.
    #[must_use]
    pub fn sgr(&self) -> String {
        match self {
            Self::Black => "30".into(),
            Self::Red => "31".into(),
            Self::Green => "32".into(),
            Self::Yellow => "33".into(),
            Self::Blue => "34".into(),
            Self::Magenta => "35".into(),
            Self::Cyan => "36".into(),
            Self::White => "37".into(),
            Self::BrightBlack => "90".into(),
            Self::BrightRed => "91".into(),
            Self::BrightGreen => "92".into(),
            Self::BrightYellow => "93".into(),
            Self::BrightBlue => "94".into(),
            Self::BrightMagenta => "95".into(),
            Self::BrightCyan => "96".into(),
            Self::BrightWhite => "97".into(),
            Self::Hex(r, g, b) => format!("38;2;{r};{g};{b}"),
        }
    }

    /// Wraps `text` in SGR color + reset codes.
    #[must_use]
    pub fn paint(&self, text: &str) -> String {
        format!("\u{1b}[{}m{text}\u{1b}[0m", self.sgr())
    }

    /// Strips ANSI SGR sequences — proves color output is layout-identical.
    #[must_use]
    pub fn strip_ansi(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }
}

fn parse_hex(hex: &str) -> Option<Color> {
    // #rgb expands to #rrggbb (CSS convention)
    if hex.len() == 3 {
        let vals: Vec<u8> = hex
            .chars()
            .map(|c| u8::from_str_radix(&c.to_string(), 16).ok())
            .collect::<Option<_>>()?;
        let expand = |v: u8| v * 17;
        return Some(Color::Hex(
            expand(vals[0]),
            expand(vals[1]),
            expand(vals[2]),
        ));
    }
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Hex(r, g, b))
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = match self {
            Self::Hex(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            other => {
                // Round-trip via the canonical name: find it by SGR params.

                [
                    ("black", Self::Black),
                    ("red", Self::Red),
                    ("green", Self::Green),
                    ("yellow", Self::Yellow),
                    ("blue", Self::Blue),
                    ("magenta", Self::Magenta),
                    ("cyan", Self::Cyan),
                    ("white", Self::White),
                    ("grey", Self::BrightBlack),
                    ("brightred", Self::BrightRed),
                    ("brightgreen", Self::BrightGreen),
                    ("brightyellow", Self::BrightYellow),
                    ("brightblue", Self::BrightBlue),
                    ("brightmagenta", Self::BrightMagenta),
                    ("brightcyan", Self::BrightCyan),
                    ("brightwhite", Self::BrightWhite),
                ]
                .into_iter()
                .find(|(_, c)| c.sgr() == other.sgr())
                .map_or_else(|| "grey".to_string(), |(n, _)| n.to_string())
            }
        };
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ColorVisitor;
        impl Visitor<'_> for ColorVisitor {
            type Value = Color;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a color name or #rrggbb hex string")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Color, E> {
                Color::parse(v).ok_or_else(|| E::custom(format!("unknown color: {v}")))
            }
        }
        deserializer.deserialize_str(ColorVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_named_colors() {
        assert_eq!(Color::parse("red"), Some(Color::Red));
        assert_eq!(Color::parse("RED"), Some(Color::Red));
        assert_eq!(Color::parse("grey"), Some(Color::BrightBlack));
        assert_eq!(Color::parse("gray"), Some(Color::BrightBlack));
        assert_eq!(Color::parse("bright_white"), None); // underscores not supported
        assert_eq!(Color::parse("brightwhite"), Some(Color::BrightWhite));
        assert_eq!(Color::parse("orange"), Some(Color::Hex(255, 140, 0)));
        assert_eq!(Color::parse("nonsense"), None);
    }

    #[test]
    fn parse_hex_colors() {
        assert_eq!(Color::parse("#ff8800"), Some(Color::Hex(255, 136, 0)));
        assert_eq!(Color::parse("#f80"), Some(Color::Hex(255, 136, 0)));
        assert_eq!(Color::parse("#f8"), None);
        assert_eq!(Color::parse("#zzzzzz"), None);
    }

    #[test]
    fn sgr_codes() {
        assert_eq!(Color::Red.sgr(), "31");
        assert_eq!(Color::BrightBlack.sgr(), "90");
        assert_eq!(Color::Hex(1, 2, 3).sgr(), "38;2;1;2;3");
        assert_eq!(Color::Green.paint("ab"), "\u{1b}[32mab\u{1b}[0m");
    }

    #[test]
    fn serde_roundtrip() {
        let c = Color::Red;
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "\"red\"");
        assert_eq!(serde_json::from_str::<Color>(&json).unwrap(), c);
        let h = Color::Hex(255, 136, 0);
        assert_eq!(serde_json::from_str::<Color>("\"#ff8800\"").unwrap(), h);
    }
}

use ratatui::style::Color;

/// Semantic colour slots used by every view. Views never name raw colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    /// Titles, labels, informational status.
    pub accent: Color,
    /// Byte sizes.
    pub size: Color,
    /// Selection counts, mode indicator, warnings.
    pub warn: Color,
    pub ok: Color,
    pub danger: Color,
    /// De-emphasised text (paths, hints).
    pub dim: Color,
    /// Filled part of gauges.
    pub gauge: Color,
    /// Backgrounds: highlighted row, gauge trough.
    pub surface: Color,
    pub text: Color,
    /// Category bars on the dashboard.
    pub palette: [Color; 6],
}

pub const DEFAULT: Theme = Theme {
    name: "default",
    accent: Color::Cyan,
    size: Color::Magenta,
    warn: Color::Yellow,
    ok: Color::Green,
    danger: Color::Red,
    dim: Color::DarkGray,
    gauge: Color::Blue,
    surface: Color::DarkGray,
    text: Color::White,
    palette: [
        Color::Cyan,
        Color::Magenta,
        Color::Yellow,
        Color::Green,
        Color::Blue,
        Color::Red,
    ],
};

pub const NORD: Theme = Theme {
    name: "nord",
    accent: Color::Rgb(0x88, 0xC0, 0xD0),
    size: Color::Rgb(0xB4, 0x8E, 0xAD),
    warn: Color::Rgb(0xEB, 0xCB, 0x8B),
    ok: Color::Rgb(0xA3, 0xBE, 0x8C),
    danger: Color::Rgb(0xBF, 0x61, 0x6A),
    dim: Color::Rgb(0x61, 0x6E, 0x88),
    gauge: Color::Rgb(0x5E, 0x81, 0xAC),
    surface: Color::Rgb(0x3B, 0x42, 0x52),
    text: Color::Rgb(0xEC, 0xEF, 0xF4),
    palette: [
        Color::Rgb(0x88, 0xC0, 0xD0),
        Color::Rgb(0xB4, 0x8E, 0xAD),
        Color::Rgb(0xEB, 0xCB, 0x8B),
        Color::Rgb(0xA3, 0xBE, 0x8C),
        Color::Rgb(0x81, 0xA1, 0xC1),
        Color::Rgb(0xD0, 0x87, 0x70),
    ],
};

pub const GRUVBOX: Theme = Theme {
    name: "gruvbox",
    accent: Color::Rgb(0x83, 0xA5, 0x98),
    size: Color::Rgb(0xD3, 0x86, 0x9B),
    warn: Color::Rgb(0xFA, 0xBD, 0x2F),
    ok: Color::Rgb(0xB8, 0xBB, 0x26),
    danger: Color::Rgb(0xFB, 0x49, 0x34),
    dim: Color::Rgb(0x92, 0x83, 0x74),
    gauge: Color::Rgb(0x45, 0x85, 0x88),
    surface: Color::Rgb(0x3C, 0x38, 0x36),
    text: Color::Rgb(0xEB, 0xDB, 0xB2),
    palette: [
        Color::Rgb(0x83, 0xA5, 0x98),
        Color::Rgb(0xD3, 0x86, 0x9B),
        Color::Rgb(0xFA, 0xBD, 0x2F),
        Color::Rgb(0xB8, 0xBB, 0x26),
        Color::Rgb(0xFE, 0x80, 0x19),
        Color::Rgb(0x8E, 0xC0, 0x7C),
    ],
};

pub const DRACULA: Theme = Theme {
    name: "dracula",
    accent: Color::Rgb(0x8B, 0xE9, 0xFD),
    size: Color::Rgb(0xFF, 0x79, 0xC6),
    warn: Color::Rgb(0xF1, 0xFA, 0x8C),
    ok: Color::Rgb(0x50, 0xFA, 0x7B),
    danger: Color::Rgb(0xFF, 0x55, 0x55),
    dim: Color::Rgb(0x62, 0x72, 0xA4),
    gauge: Color::Rgb(0xBD, 0x93, 0xF9),
    surface: Color::Rgb(0x44, 0x47, 0x5A),
    text: Color::Rgb(0xF8, 0xF8, 0xF2),
    palette: [
        Color::Rgb(0x8B, 0xE9, 0xFD),
        Color::Rgb(0xFF, 0x79, 0xC6),
        Color::Rgb(0xF1, 0xFA, 0x8C),
        Color::Rgb(0x50, 0xFA, 0x7B),
        Color::Rgb(0xBD, 0x93, 0xF9),
        Color::Rgb(0xFF, 0xB8, 0x6C),
    ],
};

/// Single-hue theme for monochrome terminals and colour-blind users;
/// meaning is carried by text and bold weight instead.
pub const MONO: Theme = Theme {
    name: "mono",
    accent: Color::White,
    size: Color::White,
    warn: Color::White,
    ok: Color::White,
    danger: Color::White,
    dim: Color::Gray,
    gauge: Color::Gray,
    surface: Color::DarkGray,
    text: Color::White,
    palette: [Color::White; 6],
};

pub const ALL: [Theme; 5] = [DEFAULT, NORD, GRUVBOX, DRACULA, MONO];

impl Default for Theme {
    fn default() -> Self {
        DEFAULT
    }
}

impl Theme {
    pub fn by_name(name: &str) -> Option<Theme> {
        ALL.iter()
            .copied()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }

    pub fn names() -> Vec<&'static str> {
        ALL.iter().map(|t| t.name).collect()
    }

    /// The theme after this one, wrapping around.
    pub fn next(self) -> Theme {
        let idx = ALL.iter().position(|t| t.name == self.name).unwrap_or(0);
        ALL[(idx + 1) % ALL.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_name_is_case_insensitive_and_rejects_unknown() {
        assert_eq!(Theme::by_name("Nord").map(|t| t.name), Some("nord"));
        assert!(Theme::by_name("solarized").is_none());
    }

    #[test]
    fn next_cycles_through_every_theme_and_wraps() {
        let mut t = DEFAULT;
        let mut seen = vec![t.name];
        for _ in 1..ALL.len() {
            t = t.next();
            seen.push(t.name);
        }
        assert_eq!(seen, Theme::names());
        assert_eq!(t.next().name, "default");
    }
}

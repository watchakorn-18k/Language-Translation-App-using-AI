use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    pub const ALL: &'static [ThemeChoice] =
        &[ThemeChoice::System, ThemeChoice::Dark, ThemeChoice::Light];

    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "System",
            ThemeChoice::Dark => "Dark",
            ThemeChoice::Light => "Light",
        }
    }

    pub fn from_label(label: &str) -> Self {
        match label.trim().to_ascii_lowercase().as_str() {
            "dark" => ThemeChoice::Dark,
            "light" => ThemeChoice::Light,
            _ => ThemeChoice::System,
        }
    }

    pub fn is_dark(self, system_is_dark: bool) -> bool {
        match self {
            ThemeChoice::Dark => true,
            ThemeChoice::Light => false,
            ThemeChoice::System => system_is_dark,
        }
    }
}

/// Graphite neutral ramp tokens (unslop dark + light palettes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub page: u32,
    pub card: u32,
    pub raised: u32,
    pub border_hairline: u32,
    pub border_subtle: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub accent: u32,
    pub primary_btn_bg: u32,
    pub primary_btn_fg: u32,
}

impl Palette {
    pub const DARK: Self = Self {
        page: 0x141415,
        card: 0x19191b,
        raised: 0x232325,
        border_hairline: 0xededed14,
        border_subtle: 0xededed1f,
        text_primary: 0xededed,
        text_secondary: 0xa1a1a6,
        text_muted: 0x85858b,
        accent: 0xd4973b,
        primary_btn_bg: 0xededed,
        primary_btn_fg: 0x141415,
    };

    pub const LIGHT: Self = Self {
        page: 0xfafafa,
        card: 0xffffff,
        raised: 0xf4f4f5,
        border_hairline: 0x17171714,
        border_subtle: 0x17171724,
        text_primary: 0x171717,
        text_secondary: 0x525252,
        text_muted: 0x737373,
        accent: 0xb47318,
        primary_btn_bg: 0x171717,
        primary_btn_fg: 0xffffff,
    };

    pub fn for_choice(choice: ThemeChoice, system_is_dark: bool) -> Self {
        if choice.is_dark(system_is_dark) {
            Self::DARK
        } else {
            Self::LIGHT
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_theme_labels_and_resolves_palettes() {
        assert_eq!(ThemeChoice::from_label("Dark"), ThemeChoice::Dark);
        assert_eq!(ThemeChoice::from_label("light"), ThemeChoice::Light);
        assert_eq!(ThemeChoice::from_label("anything"), ThemeChoice::System);
        assert_eq!(
            serde_json::to_string(&ThemeChoice::Dark).unwrap(),
            "\"dark\""
        );
        assert_eq!(Palette::for_choice(ThemeChoice::Dark, false), Palette::DARK);
        assert_eq!(
            Palette::for_choice(ThemeChoice::Light, true),
            Palette::LIGHT
        );
        assert_eq!(
            Palette::for_choice(ThemeChoice::System, true),
            Palette::DARK
        );
        assert_eq!(
            Palette::for_choice(ThemeChoice::System, false),
            Palette::LIGHT
        );
    }
}

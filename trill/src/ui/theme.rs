//! Trill's iced `Theme` builder. The dark/light palettes are
//! custom — not iced's stock Light/Dark — so they live here
//! rather than picking from `iced::Theme::DARK` etc. Used by
//! `App::theme` (live theme for the window chrome) and by any
//! `view` function that needs to derive a theme-aware style
//! before iced has handed it the live `Theme` (e.g. markdown
//! link color in the About panel).
//!
//! The config-independent constants and decorations the app draws
//! with sit at the top of this file — the palettes below anchor on
//! them.

use crate::config;
use iced::Theme;

/// Trill's brand green, the color the legacy egui app ran. It anchors
/// `success` in both palettes so "it worked" stays green no matter
/// which accent the chrome wears; the chrome accent itself comes from
/// [`accent_color`] (dinosaur orange by default).
pub const TRILL_GREEN: iced::Color =
    iced::Color::from_rgb(0x4c as f32 / 255.0, 0xaf as f32 / 255.0, 0x50 as f32 / 255.0);

/// The chrome color, as sRGB bytes, for a [`config::AccentColor`]
/// choice per theme mode. Dark mode wants bright accents that glow
/// against the charcoal; light mode needs deeper shades of the same
/// identity or the frames/glows wash out on cream (the LC restyle
/// learned this with its pale PET cyan — hence the deeper
/// `LIGHT_CYAN` twin). Dinosaur orange is the exception: it is vivid
/// enough to glow on charcoal and still read on cream, so it gets one
/// shade for both.
///
/// A [`config::AccentColor::Custom`] choice ignores `dark` — the user
/// picked exactly that color, so honor it in both themes.
pub fn accent_rgb(accent: config::AccentColor, custom: [u8; 3], dark: bool) -> [u8; 3] {
    match (accent, dark) {
        (config::AccentColor::DinosaurOrange, _) => [0xff, 0x5a, 0x00],
        // The blue bomber's azure — bright enough to glow on
        // charcoal; cobalt in daylight.
        (config::AccentColor::MegaManBlue, true) => [0x4d, 0xa6, 0xff],
        (config::AccentColor::MegaManBlue, false) => [0x14, 0x5c, 0xc2],
        // Crimson, a step deeper than the danger red so alarms still
        // read a notch hotter than the chrome.
        (config::AccentColor::BluesRed, true) => [0xef, 0x40, 0x56],
        (config::AccentColor::BluesRed, false) => [0xb7, 0x1c, 0x30],
        (config::AccentColor::RollPink, true) => [0xff, 0x6e, 0xa8],
        (config::AccentColor::RollPink, false) => [0xc2, 0x2f, 0x6d],
        // GutsMan's metallic amber-gold — deeper than the selection
        // gold, which deliberately stays gold alongside it. Light
        // mode runs bronze (bright gold on cream has no contrast).
        (config::AccentColor::GutsManYellow, true) => [0xe6, 0xb4, 0x22],
        (config::AccentColor::GutsManYellow, false) => [0x96, 0x71, 0x18],
        // Bass's aura violet — bright enough to glow on charcoal;
        // royal purple in daylight for the same reason as the blues.
        (config::AccentColor::BassPurple, true) => [0xae, 0x6f, 0xf5],
        (config::AccentColor::BassPurple, false) => [0x6a, 0x35, 0xb5],
        (config::AccentColor::Custom, _) => custom,
    }
}

pub fn accent_color(accent: config::AccentColor, custom: [u8; 3], dark: bool) -> iced::Color {
    to_color(accent_rgb(accent, custom, dark))
}

/// An sRGB byte triple as an iced color.
pub fn to_color([r, g, b]: [u8; 3]) -> iced::Color {
    iced::Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

/// RGB bytes to `(hue, saturation, value)` with every component in
/// `0.0..=1.0`. Hue is 0 for greys (the picker treats that as "no
/// preference").
pub fn rgb_to_hsv([r, g, b]: [u8; 3]) -> (f32, f32, f32) {
    let (r, g, b) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };
    let saturation = if max <= f32::EPSILON { 0.0 } else { delta / max };
    (hue, saturation, max)
}

/// `(hue, saturation, value)` to RGB bytes, each component clamped to
/// `0.0..=1.0`. Hue wraps.
pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> [u8; 3] {
    let hue = hue.rem_euclid(1.0);
    let saturation = saturation.clamp(0.0, 1.0);
    let value = value.clamp(0.0, 1.0);
    let chroma = value * saturation;
    let x = chroma * (1.0 - ((hue * 6.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (hue * 6.0) as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = value - chroma;
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ]
}

pub fn theme_for(config: &config::Config) -> Theme {
    // Trill palettes — these aren't tweaks of iced's stock Light /
    // Dark anymore. The dark variant keeps the Battle Network
    // Legacy Collection structure (glowing panel frames, gold
    // selection, the drawn cyberworld backdrop) but runs it in the
    // accent on the original deep navy — a bluer "PET blue" base was
    // tried twice and clashes with the accent chrome. Light is a warm
    // cream + slate set tuned to feel like the same UI under daylight,
    // not a separate identity.
    match config.theme {
        config::ThemeMode::Light => Theme::custom(
            "Trill Light".to_string(),
            iced::theme::Palette {
                background: iced::Color::from_rgb(0xf3 as f32 / 255.0, 0xee as f32 / 255.0, 0xdc as f32 / 255.0),
                text: iced::Color::from_rgb(0x14 as f32 / 255.0, 0x22 as f32 / 255.0, 0x34 as f32 / 255.0),
                primary: accent_color(config.accent, config.custom_accent_rgb(), false),
                // Success stays semantically green no matter what
                // the chrome runs — "it worked" shouldn't turn red
                // because the user picked Proto Red.
                success: TRILL_GREEN,
                warning: iced::Color::from_rgb(0xb7 as f32 / 255.0, 0x7e as f32 / 255.0, 0x33 as f32 / 255.0),
                danger: iced::Color::from_rgb(0xd1 as f32 / 255.0, 0x3a as f32 / 255.0, 0x3a as f32 / 255.0),
            },
        ),
        config::ThemeMode::Dark => Theme::custom(
            "Trill Dark".to_string(),
            iced::theme::Palette {
                // Neutral charcoal, the faintest hair cool. The
                // base went navy ("too blue" next to the green
                // chrome), then green-black ("way too green
                // everywhere") — the lesson both times: the base
                // shouldn't carry the accent's hue at all, just sit
                // dark and let the accent chrome and gold selection
                // be the color. Still darker than stock iced Dark
                // (0x2b2d31) so the accent actually glows.
                background: iced::Color::from_rgb(0x0e as f32 / 255.0, 0x10 as f32 / 255.0, 0x11 as f32 / 255.0),
                // Neutral off-white to match — any tinted white
                // (the old cyan, then green) casts its hue onto
                // every surface mixed from it.
                text: iced::Color::from_rgb(0xec as f32 / 255.0, 0xee as f32 / 255.0, 0xed as f32 / 255.0),
                primary: accent_color(config.accent, config.custom_accent_rgb(), true),
                // See the Light arm — success is always green.
                success: TRILL_GREEN,
                warning: iced::Color::from_rgb(0xff as f32 / 255.0, 0xb5 as f32 / 255.0, 0x47 as f32 / 255.0),
                danger: iced::Color::from_rgb(0xff as f32 / 255.0, 0x52 as f32 / 255.0, 0x52 as f32 / 255.0),
            },
        ),
    }
}

/// The markdown widget's style for the given theme. iced's
/// `markdown::Style::from(theme)` only derives colors — it leaves the body
/// font at `Font::DEFAULT` (the system sans-serif) and code at the system
/// monospace, ignoring the app's bundled Noto faces. Pin them to our fonts
/// so READMEs / the About panel match the rest of the UI.
pub fn markdown_style(theme: &Theme) -> iced::widget::markdown::Style {
    let mut style = iced::widget::markdown::Style::from(theme);
    style.font = crate::ui::style::DEFAULT_FONT;
    style.inline_code_font = crate::ui::style::MONOSPACE_FONT;
    style.code_block_font = crate::ui::style::MONOSPACE_FONT;
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_accent_is_used_verbatim_in_both_modes() {
        assert_eq!(
            accent_rgb(config::AccentColor::Custom, [0x12, 0x34, 0x56], true),
            [0x12, 0x34, 0x56]
        );
        assert_eq!(
            accent_rgb(config::AccentColor::Custom, [0x12, 0x34, 0x56], false),
            [0x12, 0x34, 0x56]
        );
    }

    #[test]
    fn hsv_round_trips_through_rgb() {
        for rgb in [
            [0xff, 0x5a, 0x00],
            [0x00, 0xcc, 0x88],
            [0x12, 0x34, 0x56],
            [0xff, 0xff, 0xff],
        ] {
            let (h, s, v) = rgb_to_hsv(rgb);
            assert_eq!(hsv_to_rgb(h, s, v), rgb);
        }
        // Grey has no hue; both extremes survive the trip.
        assert_eq!(hsv_to_rgb(0.0, 0.0, 0.0), [0, 0, 0]);
    }
}

//! General pane: profile, interface, storage, updates, patches.

use super::*;
// Explicit: macros reached only through the glob above are ambiguous.
use sweeten::widget::{column, row};

/// A [`config::ThemeMode`] as a pick_list [`Choice`], labeled in the
/// UI language (mirrors [`crate::i18n::LanguageChoice`]).
fn theme_choice(lang: &LanguageIdentifier, mode: config::ThemeMode) -> Choice<config::ThemeMode> {
    Choice::new(
        mode,
        match mode {
            config::ThemeMode::Dark => t!(lang, "settings-theme-dark"),
            config::ThemeMode::Light => t!(lang, "settings-theme-light"),
        },
    )
}

/// A [`config::AccentColor`] as a pick_list [`Choice`], labeled in
/// the UI language.
fn accent_choice(lang: &LanguageIdentifier, accent: config::AccentColor) -> Choice<config::AccentColor> {
    Choice::new(
        accent,
        match accent {
            config::AccentColor::DinosaurOrange => t!(lang, "settings-accent-dinosaur-orange"),
            config::AccentColor::MegaManBlue => t!(lang, "settings-accent-rockman-blue"),
            config::AccentColor::BluesRed => t!(lang, "settings-accent-blues-red"),
            config::AccentColor::RollPink => t!(lang, "settings-accent-roll-pink"),
            config::AccentColor::GutsManYellow => t!(lang, "settings-accent-gutsman-yellow"),
            config::AccentColor::BassPurple => t!(lang, "settings-accent-forte-purple"),
            config::AccentColor::Custom => t!(lang, "settings-accent-custom"),
        },
    )
}

/// The custom accent's live preview chip, sitting next to its hex
/// field.
fn accent_swatch<'a>(rgb: [u8; 3]) -> Element<'a, Message> {
    container(Space::new().width(Length::Fixed(26.0)).height(Length::Fixed(26.0)))
        .style(move |theme: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(crate::ui::theme::to_color(rgb))),
            border: iced::Border {
                color: iced::Color {
                    a: 0.5,
                    ..theme.palette().text
                },
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
}

pub(super) fn settings_general<'a>(
    lang: &'a LanguageIdentifier,
    config: &'a config::Config,
    state: &'a State,
) -> Element<'a, Message> {
    let mut groups: Vec<Element<'a, Message>> = vec![
        settings_group(
            t!(lang, "settings-group-profile"),
            vec![
                option_row::<Message>(
                    t!(lang, "settings-nickname"),
                    text_input("", config.nickname.as_deref().unwrap_or(""))
                        .on_input(Message::NicknameChanged)
                        .padding(STANDARD_PADDING)
                        .width(Length::Fixed(240.0))
                        .style(widgets::chunky_text_input),
                ),
                option_row(
                    t!(lang, "settings-streamer-mode"),
                    toggle(config.streamer_mode, Message::ToggleStreamerMode),
                ),
            ],
        ),
        settings_group(
            t!(lang, "settings-group-interface"),
            vec![
                option_row::<Message>(t!(lang, "settings-language"), {
                    // Build the picker options as `LanguageChoice`
                    // wrappers — they Display the endonym from each
                    // locale's `LANGUAGE` Fluent key instead of the
                    // bare locale code.
                    let options: Vec<crate::i18n::LanguageChoice> = SUPPORTED_LANGS
                        .iter()
                        .map(|id| crate::i18n::LanguageChoice::new(id.clone()))
                        .collect();
                    let selected = options.iter().find(|c| c.id == config.language).cloned();
                    widgets::focus_picker(options, selected, |c: crate::i18n::LanguageChoice| {
                        Message::LanguageSelected(c.id)
                    })
                }),
                option_row::<Message>(t!(lang, "settings-theme"), {
                    let options = vec![
                        theme_choice(lang, config::ThemeMode::Dark),
                        theme_choice(lang, config::ThemeMode::Light),
                    ];
                    let selected = options.iter().find(|c| c.value == config.theme).cloned();
                    widgets::focus_picker(options, selected, |c: Choice<config::ThemeMode>| {
                        Message::ThemeChanged(c.value)
                    })
                }),
                option_row::<Message>(t!(lang, "settings-accent"), {
                    let options = vec![
                        accent_choice(lang, config::AccentColor::DinosaurOrange),
                        accent_choice(lang, config::AccentColor::MegaManBlue),
                        accent_choice(lang, config::AccentColor::BluesRed),
                        accent_choice(lang, config::AccentColor::RollPink),
                        accent_choice(lang, config::AccentColor::GutsManYellow),
                        accent_choice(lang, config::AccentColor::BassPurple),
                        accent_choice(lang, config::AccentColor::Custom),
                    ];
                    let selected = options.iter().find(|c| c.value == config.accent).cloned();
                    widgets::focus_picker(options, selected, |c: Choice<config::AccentColor>| {
                        Message::AccentChanged(c.value)
                    })
                }),
            ],
        ),
    ];

    // The picker only appears once the accent picker runs in Custom:
    // presets already say exactly which color the chrome wears, and
    // the extra controls would just be noise above them.
    if config.accent == config::AccentColor::Custom {
        let rgb = config.custom_accent_rgb();
        let hex = state
            .custom_accent_hex
            .clone()
            .unwrap_or_else(|| config.custom_accent.clone());
        // Prefer the picker's own HSV so hue/saturation survive a
        // trip through black or grey; fall back to deriving them
        // from RGB after a hex edit or config load.
        let hsv = state
            .custom_accent_hsv
            .filter(|&(h, s, v)| crate::ui::theme::hsv_to_rgb(h, s, v) == rgb)
            .unwrap_or_else(|| crate::ui::theme::rgb_to_hsv(rgb));
        groups.push(settings_group(
            t!(lang, "settings-group-custom-accent"),
            vec![
                option_row::<Message>(
                    t!(lang, "settings-custom-accent-hex"),
                    row![
                        accent_swatch(rgb),
                        text_input("#RRGGBB", &hex)
                            .on_input(Message::CustomAccentHexChanged)
                            .padding(STANDARD_PADDING)
                            .width(Length::Fixed(120.0))
                            .style(widgets::chunky_text_input),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                ),
                container(
                    column![
                        text(t!(lang, "settings-custom-accent-picker-hint"))
                            .size(TEXT_CAPTION)
                            .style(widgets::muted_text_style),
                        widgets::color_picker(hsv, Message::CustomAccentHsvChanged),
                    ]
                    .spacing(6),
                )
                .padding([4, 10])
                .into(),
            ],
        ));
    }

    groups.extend([
        settings_group(
            t!(lang, "settings-group-storage"),
            vec![option_row::<Message>(
                t!(lang, "settings-data-folder"),
                row![
                    // The path is supporting detail next to its
                    // Change action, so it rides muted at caption
                    // size instead of competing with the row label.
                    text(config.data_path.to_string_lossy().into_owned())
                        .size(TEXT_CAPTION)
                        .style(widgets::muted_text_style),
                    crate::ui::focus::on_press(
                        Message::OpenDataFolderPicker,
                        button(text(t!(lang, "settings-data-folder-change")))
                            .on_press(Message::OpenDataFolderPicker)
                            .padding(STANDARD_PADDING)
                            .style(widgets::neutral),
                    ),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )],
        ),
        settings_group(
            t!(lang, "settings-group-patches"),
            vec![
                option_row::<Message>(
                    t!(lang, "settings-patch-repo"),
                    text_input("", &config.patch_repo)
                        .on_input(Message::PatchRepoChanged)
                        .padding(STANDARD_PADDING)
                        .width(Length::Fixed(380.0))
                        .style(widgets::chunky_text_input),
                ),
                option_row(
                    t!(lang, "settings-enable-patch-autoupdate"),
                    toggle(config.enable_patch_autoupdate, Message::TogglePatchAutoupdate),
                ),
            ],
        ),
        settings_group(
            t!(lang, "settings-group-updates"),
            vec![
                option_row(
                    t!(lang, "settings-enable-updater"),
                    toggle(config.enable_updater, Message::ToggleEnableUpdater),
                ),
                option_row(
                    t!(lang, "settings-allow-prerelease-upgrades"),
                    toggle(config.allow_prerelease_upgrades, Message::ToggleAllowPrereleaseUpgrades),
                ),
            ],
        ),
    ]);

    column(groups).spacing(24).padding(style::PANE_PADDING).into()
}

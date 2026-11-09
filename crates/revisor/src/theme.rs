//! The theme of the interface (TASK-34, ADR-0014): wxWidgets paints every window light,
//! dark or as Windows is, by the setting «Тема», and a contrast theme of Windows always wins
//! over it. A new theme takes effect at once, in every window.

use std::cell::Cell;

use revisor_core::settings::Theme;
use wxdragon::app::set_appearance;
use wxdragon::appearance::Appearance;

use crate::win32;

thread_local! {
    /// The appearance last asked of wxWidgets, so it is asked again only when it changes:
    /// asking repaints every window, and once there are windows it is itself a change of
    /// the system colours, which brings the main window back to [`apply`].
    static APPLIED: Cell<Option<Appearance>> = const { Cell::new(None) };
}

/// The appearance wxWidgets is asked for: the theme chosen, unless a contrast theme of
/// Windows is on (#6). Then light, which for wxWidgets means the colours of the system, that
/// is, the contrast theme's: its dark theme has fixed colours of its own.
pub(crate) fn appearance_of(theme: Theme, high_contrast: bool) -> Appearance {
    if high_contrast {
        return Appearance::Light;
    }
    match theme {
        Theme::SameAsWindows => Appearance::System,
        Theme::Light => Appearance::Light,
        Theme::Dark => Appearance::Dark,
    }
}

/// Makes the interface show `theme`, or the contrast theme of Windows while one is on:
/// before the first window is made at the start, and at once in every window later.
pub fn apply(theme: Theme) {
    let appearance = appearance_of(theme, win32::theme::high_contrast());
    if APPLIED.with(|applied| applied.replace(Some(appearance))) == Some(appearance) {
        return;
    }
    // A Windows older than 10 version 1903 has no dark mode for wxWidgets to use: the
    // interface then stays light, which is all the setting can do there.
    let _ = set_appearance(appearance);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_picks_the_appearance() {
        // #1, #2: «Як у Windows» follows Windows, the others hold whatever Windows does.
        assert_eq!(
            appearance_of(Theme::SameAsWindows, false),
            Appearance::System
        );
        assert_eq!(appearance_of(Theme::Light, false), Appearance::Light);
        assert_eq!(appearance_of(Theme::Dark, false), Appearance::Dark);
    }

    #[test]
    fn a_contrast_theme_of_windows_wins_over_every_setting() {
        // #6: never the dark colours of wxWidgets over the colours of a contrast theme.
        for theme in Theme::ALL {
            assert_eq!(appearance_of(theme, true), Appearance::Light);
        }
    }
}

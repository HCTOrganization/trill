//! Controller focus navigation for the menu screens.
//!
//! Every control that wants to be reachable registers itself with
//! [`crate::ui::focus::focusable`] while the view is built; this module
//! owns the index that walks that list and the press-edge detection that
//! moves it. The d-pad (or whatever the user bound to it) nudges the
//! ring, A activates the focused control, and left/right adjust it in
//! place when it offers an adjustment (a picker or a slider). The
//! shoulders are the tab strip's own shortcut: L/R walk the four tabs,
//! wrapping at the ends.
//!
//! The Play tab's link-code strip keeps first refusal on input: START
//! opens its modal, and while that modal is up it owns the d-pad
//! entirely (it edits digits with it), so the ring and the shoulders are
//! left alone.

use super::{App, Message, Tab};
use crate::platform::input::{Event, HeldState, MappedKey, Mapping};
use crate::ui::focus;
use iced::Task;

/// Which navigation actions a held-state currently satisfies, read off
/// the user's key bindings.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct NavActions {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    a: bool,
    l: bool,
    r: bool,
}

impl NavActions {
    fn read(mapping: &Mapping, held: &HeldState) -> Self {
        let on = |key: MappedKey| mapping.slot(key).iter().any(|p| held.is_active(p));
        Self {
            up: on(MappedKey::Up),
            down: on(MappedKey::Down),
            left: on(MappedKey::Left),
            right: on(MappedKey::Right),
            a: on(MappedKey::A),
            l: on(MappedKey::L),
            r: on(MappedKey::R),
        }
    }
}

/// The tab `step` positions along the nav strip from `tab`, wrapping at
/// either end. The strip lays the tabs out in declaration order, so the
/// enum order is the walking order.
fn nearby_tab(tab: Tab, step: isize) -> Tab {
    const TABS: [Tab; 4] = [Tab::Play, Tab::Replays, Tab::Patches, Tab::Settings];
    let index = TABS.iter().position(|t| *t == tab).unwrap_or(0) as isize;
    TABS[(index + step).rem_euclid(TABS.len() as isize) as usize]
}

impl App {
    /// Whether the tabs shell (and so the focus ring) is what's on
    /// screen — no session, no results card, no first-run welcome.
    pub(super) fn menu_on_screen(&self) -> bool {
        self.config.nickname.is_some() && !self.session.is_active() && self.session.results.is_none()
    }

    /// Drain SDL's gamepad queue. Main-thread only, so the tick arrives
    /// as a message and the drain happens here.
    pub(super) fn poll_gamepad(&mut self) -> Task<Message> {
        let mut tasks = Vec::new();
        while let Some(event) = gamepad_facade::next_event() {
            if let Some(event) = crate::platform::input_capture::Input::Gamepad(&event).to_event() {
                tasks.push(self.menu_input(event));
            }
        }
        Task::batch(tasks)
    }

    /// Route one physical input event: the Play tab's code strip first,
    /// then the focus ring.
    pub(super) fn menu_input(&mut self, event: Event) -> Task<Message> {
        let mut task = Task::none();
        if self.tab == Tab::Play {
            if let Some(effect) = self
                .play
                .link_code_input(&event, &self.config, &self.scanners, &self.loadout)
            {
                task = Task::batch([task, self.apply_play_effect(effect)]);
            }
        }
        // The code modal edits digits with the d-pad; leave the ring out
        // of it while it's up.
        if self.play.link_code_modal_open() {
            return task;
        }

        self.menu_held.apply(&event);
        let now = NavActions::read(&self.config.input_mapping, &self.menu_held);
        let before = std::mem::replace(&mut self.nav_actions, now);
        // Press edge only: a held key must not run the ring away.
        let pressed = |action: bool, was: bool| action && !was;

        // Shoulders walk the tab strip, the way they do on the console —
        // a whole screen at a time, independent of where the ring sits.
        if pressed(now.r, before.r) {
            return Task::batch([task, Task::done(Message::TabSelected(nearby_tab(self.tab, 1)))]);
        }
        if pressed(now.l, before.l) {
            return Task::batch([task, Task::done(Message::TabSelected(nearby_tab(self.tab, -1)))]);
        }
        if pressed(now.right, before.right) {
            if let Some(cmd) = self.step_horizontal(true) {
                return Task::batch([task, Task::done(cmd.into_message())]);
            }
            return task;
        }
        if pressed(now.left, before.left) {
            if let Some(cmd) = self.step_horizontal(false) {
                return Task::batch([task, Task::done(cmd.into_message())]);
            }
            return task;
        }
        if pressed(now.down, before.down) {
            focus::nudge(&mut self.focus, 1);
        } else if pressed(now.up, before.up) {
            focus::nudge(&mut self.focus, -1);
        } else if pressed(now.a, before.a) {
            if let Some(cmd) = focus::activation(self.focus) {
                return Task::batch([task, Task::done(cmd.into_message())]);
            }
        }
        task
    }

    /// A horizontal step: hand it to the focused control if it adjusts
    /// in place (a picker or slider), otherwise walk the ring. Returns
    /// the control's command when it took the step.
    fn step_horizontal(&mut self, forward: bool) -> Option<focus::Command> {
        if let Some(cmd) = focus::adjustment(self.focus, forward) {
            return Some(cmd);
        }
        focus::nudge(&mut self.focus, if forward { 1 } else { -1 });
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shoulders_walk_the_tab_strip_and_wrap() {
        assert_eq!(nearby_tab(Tab::Play, 1), Tab::Replays);
        assert_eq!(nearby_tab(Tab::Replays, 1), Tab::Patches);
        assert_eq!(nearby_tab(Tab::Patches, 1), Tab::Settings);
        assert_eq!(nearby_tab(Tab::Settings, 1), Tab::Play);
        assert_eq!(nearby_tab(Tab::Play, -1), Tab::Settings);
    }
}

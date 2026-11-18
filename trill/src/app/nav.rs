//! Controller focus navigation for the menu screens.
//!
//! Every control that wants to be reachable registers itself with
//! [`crate::ui::focus::focusable`] while the view is built; this module
//! owns the index that walks that list and the press-edge detection that
//! moves it. The d-pad nudges the ring, A activates the focused control,
//! and left/right adjust it in place when it offers an adjustment (a
//! picker or a slider). The shoulders are the tab strip's own shortcut:
//! L/R walk the four tabs, wrapping at the ends.
//!
//! All of it is pad-only. The bindings it reads are shared with the
//! keyboard (arrows, letters), and treating a keypress as a menu action
//! on every screen would make the keyboard fight the pad for control of
//! the UI. Keyboard input still reaches the widgets that ask for it —
//! text fields, the code-entry modal — but it never moves this ring.
//!
//! The Play tab's link-code strip keeps first refusal on input: START
//! opens its modal, and while that modal is up it owns the d-pad
//! entirely (it edits digits with it), so the ring and the shoulders are
//! left alone.

use super::{App, Message, Tab};
use crate::platform::input::{Event, HeldState, MappedKey, Mapping};
use crate::ui::focus;
use iced::Task;

/// How long B has to be held on the lobby band to leave the connection
/// attempt. Long enough that a stray press during play can't drop the
/// match, short enough that it doesn't feel stuck — and the Leave
/// button next to it shows the hold filling up.
pub(super) const CANCEL_HOLD: std::time::Duration = std::time::Duration::from_secs(3);

/// An in-flight B hold on the lobby band.
#[derive(Clone, Copy)]
pub(super) struct CancelHold {
    /// Matches the timer task's token, so releasing B (which clears the
    /// hold) makes the already-scheduled sleep a no-op when it fires.
    token: u64,
    started: iced::time::Instant,
}

impl CancelHold {
    /// 0.0 at the press, 1.0 when the hold has run its full length.
    pub(super) fn progress(&self, now: iced::time::Instant) -> f32 {
        (now.duration_since(self.started).as_secs_f32() / CANCEL_HOLD.as_secs_f32()).clamp(0.0, 1.0)
    }
}

/// Which navigation actions a held-state currently satisfies, read off
/// the user's key bindings.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct NavActions {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    a: bool,
    b: bool,
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
            b: on(MappedKey::B),
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

/// Whether an event may drive the menu ring: pad events only. Keys are
/// dropped so a binding shared with the keyboard can't take the menu
/// over — see the module docs.
fn drives_ring(event: &Event) -> bool {
    !matches!(event, Event::Key { .. })
}

impl App {
    /// Whether the tabs shell (and so the focus ring) is what's on
    /// screen — no session, no results card, no first-run welcome.
    pub(super) fn menu_on_screen(&self) -> bool {
        self.config.nickname.is_some() && !self.session.is_active() && self.session.results.is_none()
    }

    /// 0.0–1.0 progress of the leave hold, for the lobby band's Leave
    /// button. `None` when no hold is running.
    pub(super) fn cancel_hold_progress(&self) -> Option<f32> {
        self.cancel_hold.map(|hold| hold.progress(iced::time::Instant::now()))
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
    ///
    /// The ring itself is **pad-only**. Its bindings reuse the keys the
    /// keyboard's own widgets want — arrows for lists, letters for
    /// shortcut keys — and stealing those on every menu screen made the
    /// keyboard fight the pad. So a keyboard event stops here; it can
    /// still drive the code-entry modal (a modal surface that wants
    /// typing), and every widget that takes keyboard input directly is
    /// untouched.
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
        if !drives_ring(&event) {
            return task;
        }
        self.menu_held.apply(&event);
        let now = NavActions::read(&self.config.input_mapping, &self.menu_held);
        let before = std::mem::replace(&mut self.nav_actions, now);
        // Press edge only: a held key must not run the ring away.
        let pressed = |action: bool, was: bool| action && !was;

        // B held on the lobby band leaves the connection attempt. It is
        // a hold rather than a press because leaving mid-handshake costs
        // the match, and a thumb resting on B shouldn't end it.
        if self.lobby_on_screen() {
            if pressed(now.b, before.b) {
                return Task::batch([task, self.begin_cancel_hold()]);
            }
            if !now.b && before.b {
                self.cancel_hold = None;
            }
        }

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

    /// Start the B hold that leaves the lobby band, and schedule the
    /// timer that ends it. The token is what makes an early release
    /// stick: releasing clears `cancel_hold` and bumps the counter, so
    /// the pending sleep finds no hold (or a different one) when it
    /// fires.
    fn begin_cancel_hold(&mut self) -> Task<Message> {
        let token = self.cancel_hold_token.wrapping_add(1);
        self.cancel_hold_token = token;
        self.cancel_hold = Some(CancelHold {
            token,
            started: iced::time::Instant::now(),
        });
        Task::perform(
            async move {
                tokio::time::sleep(CANCEL_HOLD).await;
                Message::CancelHoldElapsed(token)
            },
            std::convert::identity,
        )
    }

    /// The hold's timer fired: disconnect if that hold is still the live
    /// one, then drop it. A no-op for a hold that was released first.
    pub(super) fn cancel_hold_elapsed(&mut self, token: u64) -> Task<Message> {
        match self.cancel_hold {
            Some(hold) if hold.token == token => {
                self.cancel_hold = None;
                self.netplay.disconnect();
            }
            _ => {}
        }
        Task::none()
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

    #[test]
    fn the_cancel_hold_fills_over_its_length() {
        let start = iced::time::Instant::now();
        let hold = CancelHold {
            token: 1,
            started: start,
        };
        assert_eq!(hold.progress(start), 0.0);
        assert!((hold.progress(start + CANCEL_HOLD / 2) - 0.5).abs() < 0.01);
        assert_eq!(hold.progress(start + CANCEL_HOLD * 2), 1.0);
    }

    #[test]
    fn only_pad_events_drive_the_ring() {
        use crate::platform::input::{Event, GamepadButton};
        use gamepad_facade::Id as GamepadId;

        let key = Event::Key {
            physical: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::ArrowDown),
            pressed: true,
        };
        let button = Event::Button {
            id: GamepadId(0),
            button: GamepadButton::DPadDown,
            pressed: true,
        };
        assert!(!drives_ring(&key));
        assert!(drives_ring(&button));
    }
}

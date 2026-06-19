//! Live emulator-session machinery: state struct, per-session
//! Message + update + view + subscription. Owned by App as
//! `session: session::State` and routed via `Message::Session(_)`.
//!
//! The Play / Replays tabs are responsible for STARTING sessions
//! (they construct an ActiveSession via [`build_playback`] /
//! [`spawn_singleplayer`] and stuff it into `state.active`); this
//! module handles everything that happens after.

pub mod pvp;
pub mod replay;
pub mod singleplayer;
pub mod view;

use crate::anim;
use crate::app::Scanners;
use crate::audio;
use crate::config;
use crate::game;
use crate::i18n::t;
use crate::patch;
use crate::save_view;
use crate::selection;
use crate::style::{self, TEXT_BODY, TEXT_CAPTION};
use crate::video::framebuffer::Effect;
use crate::widgets;
use iced::widget::canvas::{self, Canvas, Frame, LineCap, Path, Stroke};
use iced::widget::space::horizontal as horizontal_space;
use iced::widget::{button, container, stack, text};
use iced::{mouse, Alignment, Color, Element, Fill, Length, Point, Rectangle, Renderer, Theme};
use lucide_icons::Icon;
use tango_pvp::battle::{suggest_frame_delay, MAX_FRAME_DELAY, MIN_FRAME_DELAY};
use unic_langid::LanguageIdentifier;

/// Create the mgba core every session boots from: a GBA core with audio-sync
/// on, its video buffer enabled, and `rom` loaded. Callers then load the save
/// (which differs per session — RW file vs in-memory SRAM dump) and install
/// their own traps.
pub(crate) fn new_gba_core(rom: &[u8]) -> anyhow::Result<mgba::core::Core> {
    let mut core = mgba::core::Core::new_gba(
        "tango",
        &mgba::core::Options {
            audio_sync: true,
            ..Default::default()
        },
    )?;
    core.enable_video_buffer();
    core.as_mut().load_rom(mgba::vfile::VFile::from_vec(rom.to_vec()))?;
    Ok(core)
}

/// At most one of these can be active at a time: replay playback, or
/// single-player. The two variants share enough surface (vbuf,
/// close-request) that the view + tick loop wrap them uniformly.
pub enum ActiveSession {
    Replay(replay::ReplaySession),
    SinglePlayer(singleplayer::SinglePlayerSession),
    PvP(pvp::PvpSession),
}

impl ActiveSession {
    pub fn request_close(&self) {
        match self {
            Self::Replay(s) => s.request_close(),
            Self::SinglePlayer(s) => s.request_close(),
            Self::PvP(s) => s.request_close(),
        }
    }

    /// True once the session has ended on its own — currently used
    /// by PvP so a peer-disconnect / comm error tears the session
    /// view down automatically instead of leaving the user staring
    /// at a frozen frame.
    pub fn is_ended(&self) -> bool {
        match self {
            Self::Replay(_) | Self::SinglePlayer(_) => false,
            Self::PvP(s) => s.is_ended(),
        }
    }

    pub fn as_replay(&self) -> Option<&replay::ReplaySession> {
        match self {
            Self::Replay(s) => Some(s),
            _ => None,
        }
    }

    /// Local-perspective Game registration for this session. Used by
    /// the session view to pull per-game chrome (background image,
    /// logo) into the emulator pane.
    pub fn local_game(&self) -> &'static crate::game::Game {
        match self {
            Self::Replay(s) => s.game(),
            Self::SinglePlayer(s) => s.game(),
            Self::PvP(s) => s.game(),
        }
    }
}

/// Per-session UI state. App holds `session: State`; the Play and
/// Replays tabs swap an `ActiveSession` into `active` to start a
/// session, then [`State::update`] handles the rest until [`Close`]
/// clears it.
/// One per-frame snapshot of the live PvP telemetry, retained in a short ring
/// buffer ([`State::metric_history`]) so the match-settings popover can draw a
/// sparkline per metric. `round` is `None` between rounds, when no
/// skew/lead/depth reading exists; when present it is `(skew, depth, lead)`.
#[derive(Clone, Copy)]
pub struct MetricSample {
    pub tps: f32,
    pub fps_target: f32,
    pub ping_ms: u128,
    pub round: Option<(i32, u32, i32)>,
}

impl MetricSample {
    /// Read the current telemetry off a live PvP session. Called once per
    /// emulator frame from the [`Message::UpdateFramebuffer`] handler.
    fn capture(pvp: &pvp::PvpSession) -> Self {
        Self {
            tps: pvp.tps(),
            fps_target: pvp.fps_target(),
            // Raw latest ping (not the median) — the sparkline is a live
            // display, so it should track the true per-frame reading and
            // show spikes. The median feeds only the frame-delay suggestion.
            ping_ms: pvp.latency_raw().map_or(0, |d| d.as_millis()),
            round: pvp.round_stats().map(|s| (s.skew, s.depth, s.lead)),
        }
    }
}

/// How many frames of telemetry the sparklines retain (~3 s at 60 fps).
const METRIC_HISTORY_LEN: usize = 180;

pub struct State {
    /// Permanent iced ↔ emu-thread wake handle. Cloned into each
    /// active session at construction so its frame callback (and
    /// PvP end-detection wires) can `notify_one()` whenever a new
    /// frame lands or `is_ended` could flip. The [`subscription`]
    /// `.notified().await`s on this single Notify across the
    /// program's lifetime — no per-session re-keying needed.
    pub frame_notify: std::sync::Arc<tokio::sync::Notify>,
    /// Shared GBA framebuffer. The active session's frame callback
    /// `copy_from_slice`s mgba's video buffer into this Mutex once
    /// per emu vblank; the [`Message::UpdateFramebuffer`] handler
    /// locks it, clones the bytes, and rebuilds
    /// [`State::current_frame`]. Pre-sized to GBA dimensions and
    /// reused across sessions — saves the per-session
    /// `Arc<Mutex<Vec<u8>>>` allocation dance and lets the handler
    /// read straight off `State` without dispatching through
    /// `ActiveSession`.
    pub vbuf: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    pub active: Option<ActiveSession>,
    /// PvP-only: the opponent's save-view side panel, shown when
    /// they haven't blinded their setup. Defaults to hidden; user
    /// opens it via the edge handle. The drawer slides in from the
    /// screen edge and the edge handle rides its moving inner edge.
    pub opponent_panel: anim::Overlay,
    /// PvP-only: the local player's save-view side panel. Defaults
    /// to hidden; user toggles it via the red toolbar button. Slides
    /// the same way as [`opponent_panel`](Self::opponent_panel).
    pub self_panel: anim::Overlay,
    /// Combined keyboard + gamepad held state. Updated from
    /// the input event stream; the user's Mapping resolves it
    /// into mgba joyflags each event.
    pub input_held: crate::input::HeldState,
    /// Last value of `mapping.speed_up_held(...)` so we can
    /// detect the falling/rising edge and only call set_speed
    /// when it actually flips.
    pub speed_up_engaged: bool,
    /// In-session Settings overlay. Toggled by the Settings
    /// icon in the status bar (`Message::OpenSettings`) and the
    /// "back to session" button on the overlay itself
    /// (`Message::CloseSettings`). The emulator keeps running
    /// underneath; we just swap what `App::view` renders.
    pub settings: anim::Overlay,
    /// PvP-only: the "are you sure?" modal that gates the
    /// Disconnect item in the options menu. Disconnect tears the
    /// session down mid-match (same as Close), so the confirm
    /// keeps a stray click from costing the user a real game.
    pub disconnect: anim::Overlay,
    /// PvP-only: the match-settings popover, anchored above the
    /// telemetry plate (instrument panel) and toggled by clicking it.
    /// Holds the live frame-delay control (moved here from the footer).
    /// Mutually exclusive with the options menu.
    pub match_settings: anim::Overlay,
    /// Latest GBA framebuffer (post upscale filter), presented by the
    /// [`crate::video::framebuffer`] shader widget. Refreshed in
    /// [`Message::UpdateFramebuffer`] (which the session subscription
    /// fires once per emulator vblank). `None` between sessions and
    /// before the first frame lands.
    pub current_frame: Option<crate::video::framebuffer::Frame>,
    /// Monotonic counter stamped into each [`current_frame`] so the
    /// framebuffer pipeline can skip re-uploading when the same frame
    /// is presented twice (a UI redraw with no new emu frame).
    pub frame_revision: u64,
    /// Rolling window of PvP telemetry snapshots (newest at the back),
    /// sampled once per frame from the [`Message::UpdateFramebuffer`] handler
    /// and drawn as sparklines in the match-settings popover. Capped at
    /// [`METRIC_HISTORY_LEN`]; cleared whenever the active session is not a
    /// live PvP match.
    pub metric_history: std::collections::VecDeque<MetricSample>,
    /// Replay-only: scrub-bar interaction state (drag preview, the
    /// floating hover thumbnail, and the bookkeeping that ties them to
    /// the running playback session). Inert outside a replay session.
    pub scrub: replay::Scrub,
    /// Wall-clock of the last cursor movement over the session
    /// view — drives the floating controls' auto-hide. Bumped by
    /// [`Message::MouseMoved`] and on session start
    /// ([`State::wake_controls`]).
    pub last_mouse_move: std::time::Instant,
    /// Cursor is currently over the floating controls bar — pins
    /// it visible regardless of the idle timer.
    pub controls_hovered: bool,
    /// Show/hide transition for the floating controls bar. Synced
    /// after every update: shown while the mouse moved recently,
    /// the cursor rests on the bar, any overlay is open, a scrub
    /// is in flight, or a replay is paused. Unlike the [`anim::Overlay`]
    /// fields above it has no companion bool — its target is recomputed
    /// from those inputs each update rather than toggled by a handler.
    pub controls_anim: anim::Transition,
}

impl Default for State {
    fn default() -> Self {
        Self {
            frame_notify: std::sync::Arc::new(tokio::sync::Notify::new()),
            vbuf: std::sync::Arc::new(std::sync::Mutex::new(vec![
                0u8;
                // Raw BGR555 from mgba: 2 bytes/pixel. The framebuffer shader
                // expands it to RGB on the GPU (see `video::framebuffer`).
                (mgba::gba::SCREEN_WIDTH * mgba::gba::SCREEN_HEIGHT * 2)
                    as usize
            ])),
            active: None,
            opponent_panel: anim::Overlay::new(false),
            self_panel: anim::Overlay::new(false),
            input_held: crate::input::HeldState::default(),
            speed_up_engaged: false,
            settings: anim::Overlay::new(false),
            disconnect: anim::Overlay::new(false),
            match_settings: anim::Overlay::new(false),
            current_frame: None,
            frame_revision: 0,
            metric_history: std::collections::VecDeque::new(),
            scrub: replay::Scrub::default(),
            last_mouse_move: std::time::Instant::now(),
            controls_hovered: false,
            controls_anim: anim::Transition::new(true),
        }
    }
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// True iff a session is running. Drives main.rs's view routing.
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }
}

/// Messages the session pane emits + handles. All variants are
/// inert when `state.active` is `None`.
#[derive(Debug, Clone)]
pub enum Message {
    /// Close the session and return to the previous tab.
    Close,
    /// Cursor moved anywhere over the session view. Resets the
    /// floating controls' idle timer.
    MouseMoved,
    /// Cursor entered (`true`) / left (`false`) the floating
    /// controls bar. While inside, the bar never auto-hides.
    ControlsHovered(bool),
    /// Raw input event from the keyboard or a gamepad. The
    /// handler updates the held-state set, resolves the user's
    /// Mapping into joyflags, and pushes them to the active
    /// session. Speed-up uses the same mechanism (edge-
    /// detected).
    Input(InputEvent),
    /// Toggle play/pause on a replay session. No-op for single-player.
    TogglePlay,
    /// Scrub-bar drag in progress — fires per tick change while the
    /// button is held. Pauses playback and blits the nearest prefetched
    /// snapshot's framebuffer as an instant preview; the exact seek
    /// waits for [`Message::ScrubCommit`]. Replay-only.
    ScrubPreview(u32),
    /// Scrub-bar drag released. Fires the real (asynchronous) seek to
    /// the last previewed tick and resumes playback if it was running
    /// when the drag started. Replay-only.
    ScrubCommit(u32),
    /// Cursor moved onto / along the scrub bar (`Some`) or off it
    /// (`None`) without a button held. Drives the floating keyframe
    /// thumbnail above the bar. Replay-only.
    ScrubHover(Option<replay::scrubber::HoverInfo>),
    /// Set the playback speed factor (1.0 = realtime). Replay-only.
    SetSpeed(f32),
    /// PvP-only: the match-settings frame-delay slider moved. Live-sets this
    /// side's local frame delay on the running session; the App also persists it
    /// to config. No peer coordination — it's purely a local display lag.
    SetFrameDelay(u32),
    /// PvP-only: open/close the match-settings popover anchored on the
    /// telemetry plate (instrument panel). Mutually exclusive with the
    /// options menu.
    ToggleMatchSettings,
    /// User pressed Esc inside a session. Dismisses whichever overlay
    /// is on top (settings modal, disconnect confirm, match-settings
    /// popover) if any; otherwise does nothing — Esc never tears the
    /// session down (closing a session is an explicit button action).
    /// Routed here rather than from the InputCapture so the decision
    /// sees the current overlay state.
    EscPressed,
    /// Show the "really disconnect?" modal. PvP-only; picked from
    /// the options menu's Disconnect item, which also dismisses
    /// the popover.
    OpenDisconnectConfirm,
    /// Dismiss the disconnect confirm without disconnecting (the
    /// Cancel button + the modal backdrop both fire this).
    CloseDisconnectConfirm,
    /// Show/hide the opponent's setup side panel. PvP-only.
    ToggleOpponentPanel,
    /// Show/hide the local player's save-view panel. PvP-only.
    ToggleSelfPanel,
    /// User interacted with the opponent's save-view (tab swap,
    /// folder-group toggle, hover, …). PvP-only.
    OpponentSaveViewAction(save_view::Action),
    /// Mirror of [`OpponentSaveViewAction`] for the local panel.
    SelfSaveViewAction(save_view::Action),
    /// Show the in-session Settings overlay. The emulator keeps
    /// running; only the visible body swaps. Replaces the
    /// legacy in-game pause menu.
    OpenSettings,
    /// Hide the in-session Settings overlay (the "back to
    /// session" button on the overlay's header).
    CloseSettings,
    /// One emulator frame has landed, or `is_ended` could have
    /// flipped (PvP peer-end / disconnect / grace-timeout). The
    /// handler rebuilds the framebuffer from the active
    /// session's vbuf into [`State::current_frame`] and tears
    /// the session down if it's now ended. Fired by the session
    /// subscription, which wakes on [`State::frame_notify`] —
    /// `notify_one()`'d by both the frame callback and the PvP
    /// end-detection wires.
    UpdateFramebuffer,
    /// Click-swallower for modal panel chrome — keeps presses
    /// on the panel's inert regions from falling through to the
    /// dismiss-on-press backdrop layer beneath it.
    NoOp,
}

/// Atomic input event we feed to the mapping resolver. Carries
/// the raw key/button/axis info so the session layer can drive
/// both joyflags and the speed-up edge detector.
#[derive(Debug, Clone)]
pub enum InputEvent {
    Key {
        physical: iced::keyboard::key::Physical,
        pressed: bool,
    },
    Button {
        button: crate::input::GamepadButton,
        pressed: bool,
    },
    Axis {
        axis: crate::input::GamepadAxis,
        value: f32,
    },
    /// Controller dropped — clear all gamepad state so
    /// disconnected buttons don't read as still-held.
    GamepadDisconnected,
}

impl State {
    /// Apply a session message to the state. Returns the iced Task
    /// that should be scheduled (always Task::none today — kept for
    /// API parity with the other tabs).
    pub fn update(&mut self, msg: Message, mapping: &crate::input::Mapping, video_filter: &str) -> iced::Task<Message> {
        let task = self.update_inner(msg, mapping, video_filter);
        // Mirror each overlay's bool into its transition in one
        // place — handlers above flip them freely and the
        // animations follow, including the multi-flip paths (Esc
        // peeling, mutual-exclusion closes).
        let now = iced::time::Instant::now();
        self.settings.sync(now);
        self.disconnect.sync(now);
        self.match_settings.sync(now);
        self.self_panel.sync(now);
        self.opponent_panel.sync(now);
        // Floating controls auto-hide. The per-frame
        // UpdateFramebuffer messages re-run this, so the idle
        // timer expires without needing its own timer source; a
        // paused replay (no frames) pins the bar visible anyway.
        let replay_paused = self
            .active
            .as_ref()
            .and_then(ActiveSession::as_replay)
            .map_or(false, |r| r.is_paused());
        // The telemetry panel (match_settings) deliberately
        // doesn't count: it lives in the permanently-visible
        // top-right indicator, independent of the HUD controls,
        // so leaving the graph open shouldn't pin the chips up.
        let overlay_open = self.settings.shown() || self.disconnect.shown();
        let show_controls = self.controls_hovered
            || overlay_open
            || replay_paused
            || self.scrub.preview.is_some()
            || self.last_mouse_move.elapsed() < CONTROLS_HIDE_AFTER;
        self.controls_anim.set(show_controls, now);
        task
    }

    /// Reset the floating controls' idle timer — called by the App
    /// when a session starts so the bar greets the user visible
    /// even if the mouse hasn't moved in a while. Also clears the
    /// hover pin: closing a session removes its widgets without
    /// any `on_exit` firing (the cursor is usually ON the close
    /// button), and a latched `controls_hovered` would pin the
    /// next session's chrome on screen permanently.
    pub fn wake_controls(&mut self) {
        self.last_mouse_move = std::time::Instant::now();
        self.controls_hovered = false;
    }

    fn update_inner(
        &mut self,
        msg: Message,
        mapping: &crate::input::Mapping,
        video_filter: &str,
    ) -> iced::Task<Message> {
        match msg {
            Message::Close => {
                if let Some(s) = self.active.as_ref() {
                    s.request_close();
                }
                self.active = None;
                self.current_frame = None;
                self.controls_hovered = false;
                self.disconnect.close();
                self.match_settings.close();
                self.scrub.clear();
            }
            Message::Input(ev) => {
                match ev {
                    InputEvent::Key { physical, pressed } => self.input_held.set_key(physical, pressed),
                    InputEvent::Button { button, pressed } => self.input_held.set_button(button, pressed),
                    InputEvent::Axis { axis, value } => self.input_held.set_axis(axis, value),
                    InputEvent::GamepadDisconnected => self.input_held.clear_gamepad(),
                }
                let joyflags = mapping.to_mgba_keys(&self.input_held);
                match self.active.as_ref() {
                    Some(ActiveSession::SinglePlayer(s)) => s.set_joyflags(joyflags),
                    Some(ActiveSession::PvP(s)) => s.set_joyflags(joyflags),
                    _ => {}
                }
                // Speed-up: only fire set_speed on the rising or
                // falling edge so we don't spam mgba's audio
                // sync target with no-op writes.
                let now_engaged = mapping.speed_up_held(&self.input_held);
                if now_engaged != self.speed_up_engaged {
                    self.speed_up_engaged = now_engaged;
                    let factor = if now_engaged { 4.0 } else { 1.0 };
                    match self.active.as_ref() {
                        Some(ActiveSession::SinglePlayer(s)) => s.set_speed(factor),
                        Some(ActiveSession::Replay(s)) => s.set_speed(factor),
                        // PvP runs at fixed EXPECTED_FPS.
                        Some(ActiveSession::PvP(_)) | None => {}
                    }
                }
            }
            Message::TogglePlay => {
                if let Some(s) = self.active.as_ref().and_then(ActiveSession::as_replay) {
                    if s.seek_will_resume() {
                        // An in-flight seek is about to resume playback,
                        // so the button shows "Pause" — honor the press
                        // as one: land the seek, stay paused.
                        s.cancel_seek_resume();
                    } else {
                        // Play at end-of-replay: rewind to start and
                        // play through again. Mirrors the behaviour you
                        // get on any media player — "play" on a finished
                        // track restarts it. The seek is asynchronous, so
                        // resuming is deferred to the chase landing —
                        // unpausing here would run frames off the end
                        // before the rewind starts.
                        let paused = s.is_paused();
                        if paused && s.current_tick() >= s.total_ticks() {
                            s.seek_to(0, true);
                        } else {
                            s.set_paused(!paused);
                        }
                    }
                }
            }
            Message::ScrubPreview(target) => {
                if let Some(s) = self.active.as_ref().and_then(ActiveSession::as_replay) {
                    self.scrub.drag(target, s);
                }
                // The drag blits its keyframes to the main screen —
                // the floating hover thumbnail is redundant under it.
                self.scrub.hover = None;
            }
            Message::ScrubCommit(target) => {
                if let Some(s) = self.active.as_ref().and_then(ActiveSession::as_replay) {
                    s.seek_to(target, self.scrub.resume);
                }
                self.scrub.end_drag();
            }
            Message::ScrubHover(hover) => {
                self.scrub.hover = hover;
                if let Some(s) = self.active.as_ref().and_then(ActiveSession::as_replay) {
                    self.scrub.refresh_thumb(s);
                }
            }
            Message::SetSpeed(factor) => {
                match self.active.as_ref() {
                    Some(ActiveSession::Replay(s)) => s.set_speed(factor),
                    Some(ActiveSession::SinglePlayer(s)) => s.set_speed(factor),
                    Some(ActiveSession::PvP(_)) => {
                        // PvP runs at fixed EXPECTED_FPS so both sides
                        // stay in sync — no speed control.
                    }
                    None => {}
                }
            }
            Message::SetFrameDelay(d) => {
                // Purely local frame delay — apply straight to the running
                // PvP session. Config persistence happens in the App's
                // `Message::Session` handler (it owns config).
                if let Some(ActiveSession::PvP(s)) = self.active.as_ref() {
                    s.set_frame_delay(d);
                }
            }
            Message::ToggleMatchSettings => {
                // PvP-only: applied by the signal indicator.
                if let Some(ActiveSession::PvP(_)) = self.active.as_ref() {
                    self.match_settings.toggle();
                }
            }
            Message::EscPressed => {
                // Peel overlays off top-down: the settings modal, then
                // the disconnect confirm, then the match-settings
                // popover. Esc stops there — it no longer tears the
                // session down (replay/SP back-out and PvP disconnect
                // are explicit button actions now).
                if self.settings.shown() {
                    self.settings.close();
                } else if self.disconnect.shown() {
                    self.disconnect.close();
                } else if self.match_settings.shown() {
                    self.match_settings.close();
                }
            }
            Message::OpenDisconnectConfirm => {
                self.disconnect.open();
            }
            Message::CloseDisconnectConfirm => {
                self.disconnect.close();
            }
            Message::MouseMoved => {
                self.last_mouse_move = std::time::Instant::now();
            }
            Message::ControlsHovered(h) => {
                self.controls_hovered = h;
            }
            Message::NoOp => {}
            Message::ToggleOpponentPanel => {
                self.opponent_panel.toggle();
            }
            Message::ToggleSelfPanel => {
                self.self_panel.toggle();
            }
            Message::OpponentSaveViewAction(action) => {
                if let Some(ActiveSession::PvP(s)) = self.active.as_mut() {
                    let sv_task = s.opponent_save_view.apply(&action);
                    return sv_task.map(Message::OpponentSaveViewAction);
                }
            }
            Message::SelfSaveViewAction(action) => {
                if let Some(ActiveSession::PvP(s)) = self.active.as_mut() {
                    let sv_task = s.local_save_view.apply(&action);
                    return sv_task.map(Message::SelfSaveViewAction);
                }
            }
            Message::OpenSettings => {
                self.settings.open();
            }
            Message::CloseSettings => {
                self.settings.close();
            }
            Message::UpdateFramebuffer => {
                // Telemetry snapshot for the popover sparklines, captured while
                // the session is borrowed below and pushed afterward. `None`
                // (no live PvP match) clears the history so a fresh match — or
                // a return to SP/replay — starts the charts clean.
                let mut sample = None;
                if let Some(session) = self.active.as_ref() {
                    // PvP self-closes when the per-game match-end
                    // hook + peer-end handshake (or grace timeout)
                    // are both satisfied. The end-detection paths
                    // each call `notify_one()` so this branch fires
                    // even after the emu thread has paused.
                    if session.is_ended() {
                        self.active = None;
                        self.current_frame = None;
                        self.controls_hovered = false;
                        self.disconnect.close();
                        self.match_settings.close();
                    } else {
                        // Upload the native frame as-is; the selected effect
                        // magnifies it on the GPU at draw time.
                        let pixels = self.vbuf.lock().unwrap().clone();
                        self.frame_revision = self.frame_revision.wrapping_add(1);
                        self.current_frame = Some(crate::video::framebuffer::Frame {
                            pixels: std::sync::Arc::new(pixels),
                            width: replay::SCREEN_WIDTH,
                            height: replay::SCREEN_HEIGHT,
                            revision: self.frame_revision,
                            effect: crate::video::effects::effect_for(video_filter),
                        });
                        if let ActiveSession::PvP(pvp) = session {
                            sample = Some(MetricSample::capture(pvp));
                        }
                    }
                }
                match sample {
                    Some(s) => {
                        self.metric_history.push_back(s);
                        while self.metric_history.len() > METRIC_HISTORY_LEN {
                            self.metric_history.pop_front();
                        }
                    }
                    None => self.metric_history.clear(),
                }
            }
        }
        iced::Task::none()
    }
}

/// Per-emulator-frame wake stream. Yields
/// [`Message::UpdateFramebuffer`] each time someone fires
/// `notify_one()` on [`State::frame_notify`] — the per-frame
/// callback for new vbuf data, and the PvP end-detection wires
/// (peer-end packet, peer disconnect, grace timeout) for
/// state-transition checks. Always-on across the program's
/// lifetime; parks silently with no active session because
/// nothing fires the notify. Keyboard input still flows through
/// [`crate::input_capture`] — see that module's docs for why the
/// subscription path is too laggy for joypad state.
pub fn subscription(state: &State) -> iced::Subscription<Message> {
    iced::Subscription::run_with(
        FrameTag {
            notify: state.frame_notify.clone(),
        },
        build_frame_stream,
    )
}

/// Stable subscription identity. The hash is a constant string so
/// iced keeps the same stream alive across view rebuilds; the
/// `notify` payload carries the actual wake handle through to
/// [`build_frame_stream`].
struct FrameTag {
    notify: std::sync::Arc<tokio::sync::Notify>,
}

impl std::hash::Hash for FrameTag {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "session-frame".hash(h);
    }
}

fn build_frame_stream(tag: &FrameTag) -> impl futures::Stream<Item = Message> {
    let notify = tag.notify.clone();
    futures::stream::unfold(notify, |notify| async move {
        notify.notified().await;
        Some((Message::UpdateFramebuffer, notify))
    })
}

/// Optional iced texture handle for a Game's background art. Pulls
/// the TGA out of the appropriate BNLC volume's shared `exe.dat` and
/// caches the decoded iced `Handle` per game. `None` whenever Steam
/// / BNLC / the target entry can't be read — caller drops the
/// background widget instead of degrading to a placeholder.
fn background_handle(game: &'static crate::game::Game) -> Option<iced::widget::image::Handle> {
    use std::collections::HashMap;
    use std::sync::LazyLock;
    static CACHE: LazyLock<std::sync::Mutex<HashMap<usize, Option<iced::widget::image::Handle>>>> =
        LazyLock::new(Default::default);
    let key = game as *const _ as usize;
    if let Some(cached) = CACHE.lock().unwrap().get(&key).cloned() {
        return cached;
    }
    let bg = game.background;
    let path = format!("exe/data/bg/{}", bg.tga);
    let handle = crate::bnlc::get(bg.volume)
        .and_then(|b| b.read_shared_file(&path))
        .and_then(|bytes| {
            // TGA has no magic prefix, so the image crate's
            // auto-detect refuses to guess it. Pass the format
            // explicitly — every shared-archive background is TGA.
            image::load_from_memory_with_format(&bytes, image::ImageFormat::Tga)
                .inspect_err(|e| log::warn!("bnlc bg {:?}/{}: decode: {e}", bg.volume, bg.tga))
                .ok()
        })
        .map(|img| {
            let rgba = img.into_rgba8();
            let (w, h) = rgba.dimensions();
            iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
        });
    CACHE.lock().unwrap().insert(key, handle.clone());
    handle
}

/// Decodes the user's custom emulator-border image into an iced
/// `Handle`, caching the result per file path (the path rarely
/// changes, so re-decoding on every frame would be wasteful). The
/// cache keys on the path string and tracks the last-modified time so
/// replacing the file at the same path picks up the new image.
/// `None` whenever the path can't be read or decoded — the caller
/// then falls back to the plain black backdrop.
fn custom_border_handle(path: &std::path::Path) -> Option<iced::widget::image::Handle> {
    use std::collections::HashMap;
    use std::sync::LazyLock;
    type Stamp = Option<std::time::SystemTime>;
    static CACHE: LazyLock<std::sync::Mutex<HashMap<std::path::PathBuf, (Stamp, Option<iced::widget::image::Handle>)>>> =
        LazyLock::new(Default::default);
    let modified: Stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    if let Some((stamp, cached)) = CACHE.lock().unwrap().get(path).cloned() {
        if stamp == modified {
            return cached;
        }
    }
    let handle = image::open(path)
        .inspect_err(|e| log::warn!("custom border {}: decode: {e}", path.display()))
        .ok()
        .map(|img| {
            let rgba = img.into_rgba8();
            let (w, h) = rgba.dimensions();
            iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
        });
    CACHE
        .lock()
        .unwrap()
        .insert(path.to_path_buf(), (modified, handle.clone()));
    handle
}

/// Builds the backdrop behind the emulator framebuffer for the current
/// border preference. A custom MP4 is rendered through the
/// [`crate::video::border`] shader primitive (NOT the `image` widget,
/// which flickers when fed a fresh handle every frame); BNLC art and
/// still custom images go through `image`; everything else is plain
/// black.
fn border_backdrop<'a>(
    session: &'a ActiveSession,
    border_preference: u8,
    border_image_path: Option<&std::path::Path>,
) -> Element<'a, Message> {
    if border_preference == 1 {
        if let Some(path) = border_image_path {
            if is_video_border(path) {
                return match video_border_handle(path) {
                    Some(frame) => iced::widget::shader::Shader::new(crate::video::border::Program::new(frame))
                        .width(Fill)
                        .height(Fill)
                        .into(),
                    // No frame decoded yet (cold start / decode failure).
                    None => black_backdrop(),
                };
            }
        }
    }
    // `0` (BNLC): per-game art. `1` (Custom): a still image. `2`
    // (Disable) or anything else: no art.
    let handle = match border_preference {
        0 => background_handle(session.local_game()),
        1 => border_image_path.and_then(custom_border_handle),
        _ => None,
    };
    match handle {
        Some(h) => iced::widget::image(h)
            .width(Fill)
            .height(Fill)
            .content_fit(iced::ContentFit::Cover)
            .into(),
        None => black_backdrop(),
    }
}

/// A plain opaque-black backdrop element filling its parent.
fn black_backdrop<'a>() -> Element<'a, Message> {
    container(iced::widget::Space::new().width(Fill).height(Fill))
        .style(|_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::BLACK)),
            ..Default::default()
        })
        .into()
}

/// True when the path's extension marks it as an MP4 video handled by
/// the ffmpeg border decoder. Case-insensitive.
fn is_video_border(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
}

/// Locates the bundled `ffmpeg`/`ffmpeg.exe` (sitting next to our own
/// binary), falling back to a bare `ffmpeg` on `PATH`. Mirrors
/// `tango_pvp::replay::export`'s resolver so both find the same
/// shipped binary.
fn resolve_ffmpeg_path() -> std::path::PathBuf {
    let mut p = std::env::current_exe()
        .ok()
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.join("ffmpeg"))
        .unwrap_or_else(|| "ffmpeg".into());
    p.set_extension(std::env::consts::EXE_EXTENSION);
    if p.exists() {
        p
    } else {
        "ffmpeg".into()
    }
}

/// A running ffmpeg-backed border-video decoder. ffmpeg loops the
/// source forever (`-stream_loop -1`) at native frame rate (`-re`)
/// with audio dropped (`-an`), piping fixed-size raw RGBA frames to a
/// reader thread that publishes the most-recent frame into `latest`.
/// The in-session redraw loop samples `latest` each frame, so the
/// border animates at whatever rate iced is already repainting.
struct VideoBorder {
    path: std::path::PathBuf,
    latest: std::sync::Arc<std::sync::Mutex<Option<crate::video::border::Frame>>>,
    /// Set to stop the decoder (e.g. the source path changed). The
    /// reader thread checks it each frame, then kills ffmpeg and exits.
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Set by the reader thread once it has exited (idle-timed-out,
    /// hit EOF, or was stopped) so the next lookup restarts cleanly.
    finished: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Bumped on every lookup. When the border stops being rendered
    /// (session closed), the reader self-terminates after a short
    /// idle window so we don't leave ffmpeg running in the background.
    last_access: std::sync::Arc<std::sync::Mutex<std::time::Instant>>,
}

/// How long the decoder keeps running after the border stops being
/// rendered before it tears ffmpeg down.
const VIDEO_BORDER_IDLE: std::time::Duration = std::time::Duration::from_secs(2);

/// Memory budget for caching a border clip's decoded RGBA frames. A clip
/// whose frames fit under this is looped seamlessly from memory (no ffmpeg
/// restart); a larger one falls back to respawn-streaming (a brief gap at
/// the loop point).
const VIDEO_BORDER_CACHE_BUDGET: usize = 512 * 1024 * 1024;

/// Returns the latest decoded frame of the looping border video at
/// `path`, lazily (re)starting the single shared ffmpeg decoder when
/// the requested path changes or the previous one exited.
///
/// To avoid the black "blink" on every decoder (re)start, the most
/// recent decoded frame is persisted per-path in `LAST` and returned
/// as a fallback whenever the live decoder hasn't produced a frame yet
/// (cold start, idle resume, loop boundary). Only a never-before-seen
/// path returns `None` (→ plain black backdrop) until its first frame.
fn video_border_handle(path: &std::path::Path) -> Option<crate::video::border::Frame> {
    use std::sync::atomic::Ordering;
    use std::sync::LazyLock;
    static ACTIVE: LazyLock<std::sync::Mutex<Option<VideoBorder>>> = LazyLock::new(Default::default);
    // Survives `ACTIVE` being swapped out, so a restart shows the
    // previous frame instead of flashing black.
    static LAST: LazyLock<std::sync::Mutex<Option<(std::path::PathBuf, crate::video::border::Frame)>>> =
        LazyLock::new(Default::default);

    let mut guard = ACTIVE.lock().unwrap();
    let alive = matches!(guard.as_ref(), Some(vb) if vb.path == path && !vb.finished.load(Ordering::Relaxed));
    if !alive {
        // Signal any stale decoder to tear down its ffmpeg, then start
        // a fresh one. The probe + spawn happen on the decoder thread,
        // so this never blocks the UI thread.
        if let Some(vb) = guard.as_ref() {
            vb.stop.store(true, Ordering::Relaxed);
        }
        *guard = start_video_border(path);
    }
    let fresh = guard.as_ref().and_then(|vb| {
        *vb.last_access.lock().unwrap() = std::time::Instant::now();
        vb.latest.lock().unwrap().clone()
    });

    let mut last = LAST.lock().unwrap();
    match fresh {
        Some(handle) => {
            *last = Some((path.to_path_buf(), handle.clone()));
            Some(handle)
        }
        // No live frame yet — reuse the last frame decoded for this
        // same path so a restart doesn't blink to black.
        None => match last.as_ref() {
            Some((p, h)) if p == path => Some(h.clone()),
            _ => None,
        },
    }
}

/// Sets up the shared decoder state and spawns the reader thread that
/// owns the ffmpeg subprocess(es). Returns `None` only if the thread
/// itself can't be spawned; ffmpeg launch/probe failures are handled
/// on the thread (the border just stays on its black/last-frame
/// fallback).
fn start_video_border(path: &std::path::Path) -> Option<VideoBorder> {
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    let latest = Arc::new(Mutex::new(None));
    let stop = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    let last_access = Arc::new(Mutex::new(std::time::Instant::now()));

    let vb = VideoBorder {
        path: path.to_path_buf(),
        latest: latest.clone(),
        stop: stop.clone(),
        finished: finished.clone(),
        last_access: last_access.clone(),
    };

    let path_owned = path.to_path_buf();
    let spawned = std::thread::Builder::new()
        .name("border-video".into())
        .spawn(move || {
            run_video_border(&path_owned, &latest, &stop, &last_access);
            finished.store(true, std::sync::atomic::Ordering::Relaxed);
        });
    if let Err(e) = spawned {
        log::warn!("custom border video {}: thread spawn: {e}", path.display());
        return None;
    }
    Some(vb)
}

/// Decoder thread body: probe the dimensions once, play the first pass
/// straight from ffmpeg while caching its frames, then loop. If the whole
/// clip fits in [`VIDEO_BORDER_CACHE_BUDGET`], the loop replays the cached
/// frames in memory at the source frame rate — seamless, with no ffmpeg
/// restart and so no gap at the loop point. Oversized clips fall back to
/// respawning the ffmpeg pass each time it ends (a brief gap). Exits when
/// stopped (path changed) or after the border has gone unrendered for
/// `VIDEO_BORDER_IDLE` (session closed).
fn run_video_border(
    path: &std::path::Path,
    latest: &std::sync::Mutex<Option<crate::video::border::Frame>>,
    stop: &std::sync::atomic::AtomicBool,
    last_access: &std::sync::Mutex<std::time::Instant>,
) {
    use std::sync::atomic::Ordering;
    use std::time::{Duration, Instant};
    // Globally monotonic so every emitted frame (across loops and different
    // videos) has a unique revision — the shader pipeline skips re-uploading
    // a revision it already holds.
    static NEXT_REVISION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    let (width, height) = match probe_video_dimensions(path) {
        Ok(dims) => dims,
        Err(e) => {
            log::warn!("custom border video {}: probe: {e}", path.display());
            return;
        }
    };
    let frame_len = width as usize * height as usize * 4;
    if frame_len == 0 {
        return;
    }

    let stopped =
        || stop.load(Ordering::Relaxed) || last_access.lock().unwrap().elapsed() > VIDEO_BORDER_IDLE;
    let publish = |pixels: std::sync::Arc<Vec<u8>>| {
        *latest.lock().unwrap() = Some(crate::video::border::Frame {
            pixels,
            width,
            height,
            revision: NEXT_REVISION.fetch_add(1, Ordering::Relaxed),
        });
    };

    // Phase 1: first pass — play straight from ffmpeg (paced by `-re`),
    // caching frames + their timing until the budget is hit.
    let mut cache: Vec<std::sync::Arc<Vec<u8>>> = Vec::new();
    let mut cache_ok = true;
    let mut cache_bytes = 0usize;
    let mut first_frame: Option<Instant> = None;
    let mut last_frame = Instant::now();
    let (_, eof) = border_stream_pass(path, frame_len, stop, last_access, |px| {
        let now = Instant::now();
        first_frame.get_or_insert(now);
        last_frame = now;
        publish(px.clone());
        if cache_ok {
            cache_bytes += frame_len;
            if cache_bytes > VIDEO_BORDER_CACHE_BUDGET {
                cache_ok = false;
                cache = Vec::new(); // too big — drop and stream instead
            } else {
                cache.push(px);
            }
        }
    });

    if stopped() {
        return;
    }

    // Phase 2a: the whole clip fits — loop it from memory, seamlessly.
    if cache_ok && eof && cache.len() > 1 {
        let span = last_frame.saturating_duration_since(first_frame.unwrap_or(last_frame));
        let interval = span
            .checked_div(cache.len() as u32 - 1)
            .filter(|d| !d.is_zero())
            .unwrap_or(Duration::from_millis(33));
        loop {
            for px in &cache {
                if stopped() {
                    return;
                }
                publish(px.clone());
                std::thread::sleep(interval);
            }
        }
    }

    // Phase 2b: too large to cache — respawn the pass each time it ends.
    let mut dry_passes = 0u32;
    while !stopped() {
        let (got_frame, _) = border_stream_pass(path, frame_len, stop, last_access, |px| publish(px));
        if got_frame {
            dry_passes = 0;
        } else {
            // Two empty passes running means ffmpeg can't decode the file;
            // stop rather than respawn-spin forever.
            dry_passes += 1;
            if dry_passes >= 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
}

/// Runs one ffmpeg decode pass for `path`, invoking `on_frame` with each
/// fixed-size raw RGBA frame. Returns `(got_any_frame, reached_eof)`. Ends
/// early (without `eof`) when stopped or idle.
fn border_stream_pass(
    path: &std::path::Path,
    frame_len: usize,
    stop: &std::sync::atomic::AtomicBool,
    last_access: &std::sync::Mutex<std::time::Instant>,
    mut on_frame: impl FnMut(std::sync::Arc<Vec<u8>>),
) -> (bool, bool) {
    use std::io::Read;
    use std::sync::atomic::Ordering;

    let mut child = match spawn_border_stream(path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("custom border video {}: ffmpeg spawn: {e}", path.display());
            return (false, false);
        }
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return (false, false);
    };
    let mut reader = std::io::BufReader::new(stdout);
    let mut buf = vec![0u8; frame_len];
    let mut got = false;
    let mut eof = false;
    loop {
        if stop.load(Ordering::Relaxed) || last_access.lock().unwrap().elapsed() > VIDEO_BORDER_IDLE {
            break;
        }
        // Fixed-size read keeps frame boundaries aligned; an error here is
        // EOF/short read, which ends this pass.
        if reader.read_exact(&mut buf).is_err() {
            eof = true;
            break;
        }
        on_frame(std::sync::Arc::new(buf.clone()));
        got = true;
    }
    let _ = child.kill();
    let _ = child.wait();
    (got, eof)
}

/// Spawns one muted, native-rate ffmpeg pass that decodes `path` once and
/// writes fixed-size raw RGBA frames to stdout. No `-stream_loop`: looping
/// is handled by the caller (from the in-memory cache, or by respawning),
/// which makes it independent of ffmpeg's loop behaviour.
fn spawn_border_stream(path: &std::path::Path) -> std::io::Result<std::process::Child> {
    let mut cmd = std::process::Command::new(resolve_ffmpeg_path());
    cmd.args(["-hide_banner", "-loglevel", "error", "-re"])
        .arg("-i")
        .arg(path)
        // `-an`: never decode/route audio — the border is silent.
        .args(["-an", "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd.spawn()
}

/// One-shot probe of a video's pixel dimensions: decode its first
/// frame to a single self-contained BMP (read to EOF, so no stream
/// splitting) and read the size off the decoded image.
fn probe_video_dimensions(path: &std::path::Path) -> anyhow::Result<(u32, u32)> {
    let mut cmd = std::process::Command::new(resolve_ffmpeg_path());
    cmd.args(["-hide_banner", "-loglevel", "error"])
        .arg("-i")
        .arg(path)
        .args(["-an", "-frames:v", "1", "-f", "image2pipe", "-vcodec", "bmp", "pipe:1"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let out = cmd.output()?;
    anyhow::ensure!(out.status.success(), "ffmpeg probe exited {:?}", out.status);
    let img = image::load_from_memory_with_format(&out.stdout, image::ImageFormat::Bmp)?;
    Ok((img.width(), img.height()))
}

/// How long the cursor has to sit still before the floating
/// controls slide away.
const CONTROLS_HIDE_AFTER: std::time::Duration = std::time::Duration::from_millis(2500);

/// Expand an mgba-native BGR555 framebuffer (one little-endian `u16`
/// per pixel — see [`State`]'s `vbuf`) to an RGBA8 image handle for
/// the hover thumbnail, via dataview's shared conversion — the same
/// table that renders ROM sprites/palettes and replay video exports,
/// and the CPU twin of the GPU decode in `video/effects/common.wgsl`.
/// At 240×160 it's cheap, and it only runs when the hovered keyframe
/// changes.
fn thumbnail_handle(framebuffer: &[u8]) -> iced::widget::image::Handle {
    let mut rgba = vec![0u8; framebuffer.len() * 2];
    tango_dataview::rom::bgr555_to_rgba8(framebuffer, &mut rgba);
    iced::widget::image::Handle::from_rgba(replay::SCREEN_WIDTH, replay::SCREEN_HEIGHT, rgba)
}

/// Decode a `.t5replay`, resolve both sides' ROM (+ optional
/// patch) from the scanners, and spin up a playback session bound to
/// the shared audio binder. Ready to drop straight into the app's
/// `session` slot.
pub fn build_playback(
    scanners: &Scanners,
    config: &config::Config,
    audio_binder: &audio::LateBinder,
    frame_notify: std::sync::Arc<tokio::sync::Notify>,
    vbuf: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    path: &std::path::Path,
) -> anyhow::Result<replay::ReplaySession> {
    let f = std::fs::File::open(path)?;
    let replay = std::sync::Arc::new(tango_pvp::replay::Replay::decode(f)?);
    let patches_path = config.patches_path();
    let resolve_rom = |side: Option<&tango_pvp::replay::metadata::Side>| -> anyhow::Result<(
        &'static game::Game,
        std::sync::Arc<Vec<u8>>,
    )> {
        let gi = side
            .and_then(|s| s.game_info.as_ref())
            .ok_or_else(|| anyhow::anyhow!("replay side has no game info"))?;
        let variant = u8::try_from(gi.rom_variant)
            .map_err(|_| anyhow::anyhow!("variant {} out of range", gi.rom_variant))?;
        let entry = tango_gamedb::find_by_family_and_variant(&gi.rom_family, variant)
            .ok_or_else(|| anyhow::anyhow!("unknown rom {}/{}", gi.rom_family, gi.rom_variant))?;
        let g = game::from_gamedb_entry(entry).ok_or_else(|| {
            anyhow::anyhow!("no impl for {}/{}", gi.rom_family, gi.rom_variant)
        })?;
        let rom = scanners
            .roms
            .read()
            .get(&entry)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("rom for {}/{} not scanned", gi.rom_family, gi.rom_variant))?;
        let rom = if let Some(patch_info) = gi.patch.as_ref() {
            let v = semver::Version::parse(&patch_info.version)?;
            patch::apply_patch_from_disk(&rom, entry, &patches_path, &patch_info.name, &v)?
        } else {
            rom
        };
        Ok((g, std::sync::Arc::new(rom)))
    };

    let (local_game, local_rom) = resolve_rom(replay.metadata.local_side.as_ref())?;
    let (remote_game, remote_rom) = resolve_rom(replay.metadata.remote_side.as_ref())?;
    replay::ReplaySession::new(
        local_game,
        local_rom,
        remote_game,
        remote_rom,
        replay,
        audio_binder,
        frame_notify,
        vbuf,
    )
}

/// Build the live PvP session from the netplay handoff data
/// plus the local selection + scanners. Async because
/// PvpSession::new awaits the lobby loop's receiver handoff,
/// and because remote-side rom resolution might apply a patch.
pub async fn spawn_pvp(
    scanners: Scanners,
    config: config::Config,
    audio_binder: audio::LateBinder,
    frame_notify: std::sync::Arc<tokio::sync::Notify>,
    vbuf: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    local_game: crate::rom::GameRef,
    local_patch: Option<(String, semver::Version)>,
    pre_match: crate::netplay::PreMatchData,
) -> anyhow::Result<pvp::PvpSession> {
    let local_game_impl =
        game::from_gamedb_entry(local_game).ok_or_else(|| anyhow::anyhow!("no impl for local game"))?;
    let local_rom_raw = scanners
        .roms
        .read()
        .get(&local_game)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("local rom not scanned"))?;
    let local_rom_bytes = if let Some((name, version)) = local_patch.as_ref() {
        patch::apply_patch_from_disk(&local_rom_raw, local_game, &config.patches_path(), name, version)?
    } else {
        local_rom_raw
    };

    // Remote-side game + rom. Falls back to the local game if
    // the remote's GameInfo is missing, but a Compatible verdict
    // would have caught that.
    let remote_gi = pre_match
        .remote_settings
        .game_info
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("remote settings missing game info"))?;
    let remote_game =
        tango_gamedb::find_by_family_and_variant(&remote_gi.family_and_variant.0, remote_gi.family_and_variant.1)
            .ok_or_else(|| anyhow::anyhow!("unknown remote rom"))?;
    let remote_game_impl =
        game::from_gamedb_entry(remote_game).ok_or_else(|| anyhow::anyhow!("no impl for remote game"))?;
    let remote_rom_raw = scanners
        .roms
        .read()
        .get(&remote_game)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("remote rom not scanned"))?;
    let remote_rom_bytes = if let Some(p) = remote_gi.patch.as_ref() {
        patch::apply_patch_from_disk(
            &remote_rom_raw,
            remote_game,
            &config.patches_path(),
            &p.name,
            &p.version,
        )?
    } else {
        remote_rom_raw
    };

    // Build the opponent's Loaded only if they didn't blind their
    // setup — otherwise we don't have visibility into their save.
    // Loaded parses chip/navi/navicust assets from the rom + wram,
    // so the session pane can render them with the same widgets we
    // use for the local side.
    let opponent_loaded = if !pre_match.remote_settings.blind_setup {
        let remote_save = remote_game
            .parse_save(&pre_match.remote_save_data)
            .map_err(|e| anyhow::anyhow!("parse remote save: {e:?}"))?;
        // `remote_rom_bytes` is already the patched image we run in the
        // session, so resolve the matching `rom_overrides` + charset and
        // hand both straight to `from_patched_rom` — no second BPS apply.
        let applied_patch = remote_gi.patch.as_ref().and_then(|p| {
            let patches = scanners.patches.read();
            let version_meta = patches.get(&p.name)?.versions.get(&p.version).cloned()?;
            Some(crate::selection::AppliedPatch {
                name: p.name.clone(),
                version: p.version.clone(),
                version_meta,
            })
        });
        Some(crate::selection::Loaded::from_patched_rom(
            remote_game,
            remote_rom_bytes.clone(),
            std::path::PathBuf::new(),
            remote_save,
            applied_patch,
        ))
    } else {
        None
    };

    // Build the local-side Loaded so the in-session "my setup"
    // toggle can render the same save-view we use for the
    // opponent panel.
    let local_loaded = {
        let local_save = local_game
            .parse_save(&pre_match.local_save_data)
            .map_err(|e| anyhow::anyhow!("parse local save: {e:?}"))?;
        // Same as the opponent side: `local_rom_bytes` is already
        // patched, so layer the overrides on via `from_patched_rom`
        // instead of re-applying the BPS patch.
        let applied_patch = local_patch.as_ref().and_then(|(name, version)| {
            let patches = scanners.patches.read();
            let version_meta = patches.get(name)?.versions.get(version).cloned()?;
            Some(crate::selection::AppliedPatch {
                name: name.clone(),
                version: version.clone(),
                version_meta,
            })
        });
        Some(crate::selection::Loaded::from_patched_rom(
            local_game,
            local_rom_bytes.clone(),
            std::path::PathBuf::new(),
            local_save,
            applied_patch,
        ))
    };

    pvp::PvpSession::new(
        local_game_impl,
        std::sync::Arc::new(local_rom_bytes),
        remote_game_impl,
        std::sync::Arc::new(remote_rom_bytes),
        pre_match,
        // Presentation delay is purely local — read straight from config (clamped
        // to the supported range), not negotiated with the peer.
        config
            .frame_delay
            .clamp(tango_pvp::battle::MIN_FRAME_DELAY, tango_pvp::battle::MAX_FRAME_DELAY),
        config.disable_bgm_in_pvp,
        &config.replays_path(),
        &audio_binder,
        opponent_loaded,
        local_loaded,
        frame_notify,
        vbuf,
    )
    .await
}

/// Boot the supplied selection in single-player mode. Caller must
/// already have a complete (game + rom + save) Loaded — there's no
/// fallback for missing pieces, so the Play button is responsible for
/// gating.
pub fn spawn_singleplayer(
    scanners: &Scanners,
    config: &config::Config,
    audio_binder: &audio::LateBinder,
    frame_notify: std::sync::Arc<tokio::sync::Notify>,
    vbuf: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    loaded: &selection::Loaded,
) -> anyhow::Result<singleplayer::SinglePlayerSession> {
    let game = game::from_gamedb_entry(loaded.game)
        .ok_or_else(|| anyhow::anyhow!("no game impl for {:?}", loaded.game.family_and_variant()))?;
    // Loaded stashes the *parsed* ROM (assets), not the raw bytes —
    // grab them back from the scanner and re-apply the patch if any so
    // the emulator sees the same image it would in the legacy app.
    let raw = scanners
        .roms
        .read()
        .get(&loaded.game)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("rom not in scanner cache"))?;
    let rom_bytes = if let Some(p) = loaded.patch.as_ref() {
        patch::apply_patch_from_disk(&raw, loaded.game, &config.patches_path(), &p.name, &p.version)?
    } else {
        raw
    };
    singleplayer::SinglePlayerSession::new(
        game,
        std::sync::Arc::new(rom_bytes),
        &loaded.save_path,
        audio_binder,
        frame_notify,
        vbuf,
    )
}

/// Convert a tick count (60 Hz GBA frames) into `m:ss` for the scrub
/// bar's wallclock labels.
pub fn format_tick(tick: u32) -> String {
    let total_s = tick / 60;
    let m = total_s / 60;
    let s = total_s % 60;
    format!("{m}:{s:02}")
}

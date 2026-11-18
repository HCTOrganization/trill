//! The Play tab: everything in one place. The full loadout selector
//! (family / save / patch pickers + save management, the latter in
//! [`save_manage`]) and the save viewer/editor fill the body, and a
//! netplay band rides the bottom —
//! the link-code strip + Fight CTA when idle, the full lobby
//! ([`lobby`]) once a connection attempt is in flight. The save view
//! stays on screen through it all, so what you're bringing to the
//! match is always visible (and switchable) even mid-lobby. The
//! selection state itself is App-level ([`Selection`])
//! so the lobby settings-resend sees every change made here live.

pub mod loadout_strip;
mod lobby;
mod save_manage;

pub use save_manage::SaveAction;

use crate::i18n::t;
use crate::library::Catalog;
use crate::library::{game, rom};
use crate::netplay::randomcode::LINK_CODE_DIGITS;
use crate::ui::style::{self, STANDARD_PADDING, TEXT_BODY, TEXT_CAPTION, TEXT_TITLE};
use crate::ui::widgets;
use crate::{config, selection};
use iced::widget::{button, container, text, Space};
use iced::{Alignment, Element, Fill, Length};
use lucide_icons::Icon;
use sweeten::widget::{column, row, text_input};
use trill_library::loadout::Selection;
use unic_langid::LanguageIdentifier;

// ---------- Messages ----------

#[derive(Debug, Clone)]
pub enum Message {
    /// Loadout strip interaction. Routed by the App to the shared
    /// [`Selection`] — never reaches [`State::update`].
    Loadout(loadout_strip::Message),
    SaveEditor(std::sync::Arc<dyn trill_gamesupport::SaveEditorMessage>),

    /// Close it and drop the draft (B, the header's X, or a click on
    /// the backdrop).
    LinkCodeModalClose,
    /// Put the cursor on this digit (clicking one).
    LinkCodeCursorSet(usize),
    /// Refill the field from a fresh random code.
    LinkCodeRandomize,
    /// Accept the draft: the connection starts from here (see
    /// [`State::confirm_link_code`]).
    LinkCodeConfirm,
    /// Copy plain text to the clipboard — the lobby's copy-link-code
    /// button. Carries the real code even when streamer mode masks it
    /// on screen.
    CopyText(String),
    /// The strip's Fight CTA: opens the code-entry modal (the pad's
    /// START does the same). The connection itself comes from the
    /// modal's OK — see [`Message::LinkCodeConfirm`].
    FightPressed,
    Disconnect,
    /// Lobby UI: user picked a different match type. App routes
    /// this through Effect::SetMatchType so the resend
    /// machinery picks it up.
    SetMatchType((u8, u8)),
    /// Lobby UI: user dragged the frame-delay slider, OR pressed
    /// the "suggest" button (which dispatches a value computed from the
    /// `lobby.latency_counter` median). Routes to the shared `config.frame_delay`
    /// (same store the Settings-tab slider writes), not lobby-local state.
    SetFrameDelay(u32),
    /// Lobby UI: user toggled the blind-setup checkbox.
    SetBlindSetup(bool),
    /// Lobby UI: user pressed Ready. App loads the local
    /// save's raw SRAM, builds a NegotiatedState, and
    /// commits the local save.
    Ready,
    /// Lobby UI: user pressed Unready (Ready button while
    /// already committed). Sends an Uncommit packet.
    Unready,
    /// Soft-disable sentinel for widgets that don't accept a
    /// `None` handler in iced 0.14 (pick_list). The lobby
    /// reroutes match-type changes here while the committed
    /// terms are locked (failure / handoff), and the selector
    /// strip does the same during handoff, so the controls
    /// render inert without touching layout. The update handler
    /// drops it.
    Noop,

    SaveOpenFolder,
    /// Open an arbitrary folder in the OS file manager. Used by
    /// the no-saves / no-roms empty-state cards to give the user
    /// a one-click jump into the right directory.
    OpenSavesFolder(std::path::PathBuf),
    SaveDuplicateStart,
    SaveDuplicateDraftChanged(String),
    SaveDuplicateConfirm,
    SaveRenameStart,
    SaveRenameDraftChanged(String),
    SaveRenameConfirm,
    SaveDeleteStart,
    SaveDeleteConfirm,
    SaveActionCancel,
    SaveNewStart,
    SaveNewDraftChanged(String),
    /// (target variant, template name) — a template option fixes both,
    /// since the family's variants each ship their own templates.
    SaveNewTemplateSelected(rom::GameRef, String),
    SaveNewConfirm,
}

// ---------- Play tab state ----------

pub struct State {
    link_code: String,
    /// Draft for the code-entry modal — `Some` while it's open. The
    /// strip itself takes no typing; the modal is how a pad-only
    /// player enters a code, and how anyone else does it without a
    /// keyboard.
    link_code_modal: Option<LinkCodeModal>,
    /// Physical inputs held right now on the Play tab. The modal acts
    /// on press *edges* off this, so one press of a bound key steps
    /// once instead of spinning while held.
    link_held: crate::platform::input::HeldState,
    /// Which link-code actions were active at the last input event —
    /// the previous side of [`State::link_held`]'s edge detection.
    link_actions: LinkActions,
    /// A link code Fight auto-generated for an empty input, parked here
    /// instead of `link_code` while the lobby is up: the outgoing strip
    /// renders for the first half of the band morph, so writing the input
    /// immediately would flash the code on screen before the lobby's
    /// connection line (which masks it in streamer mode) gets to debut it.
    /// [`restore_generated_link_code`](Self::restore_generated_link_code)
    /// moves it into the input when the lobby band leaves, so a retry
    /// re-hosts the same code.
    pending_generated_code: Option<String>,
    /// Inline state for the save-management actions (rename / delete).
    save_action: SaveAction,
    /// Entrance glide for the whole save-view pane under the selector
    /// strip, played by the App when the *family* changes — a family
    /// switch replaces the entire bottom of the tab. A save switch
    /// within the family plays the smaller entrance the save view
    /// itself owns instead, which only moves the panes under its
    /// sub-tab strip.
    save_body_enter: crate::ui::anim::Enter,
    /// Fade-through swap for the save-action row: the picker row
    /// morphs into whichever rename / delete / create form opens
    /// ([`State::save_action`]) and back.
    save_form: crate::ui::anim::Transition,
    /// The form that was open before `save_action` reset to
    /// `None`, frozen so the swap's exit half has something to
    /// render (the live form — including the rename draft — is
    /// already gone).
    save_action_exit: SaveAction,
}

impl Default for State {
    fn default() -> Self {
        Self {
            link_code: String::new(),
            link_code_modal: None,
            link_held: crate::platform::input::HeldState::default(),
            link_actions: LinkActions::default(),
            pending_generated_code: None,
            save_action: SaveAction::None,
            save_body_enter: crate::ui::anim::Enter::default(),
            save_form: crate::ui::anim::Transition::swap(false),
            save_action_exit: SaveAction::None,
        }
    }
}

/// The link-code entry modal's draft: the eight digits plus which cell
/// the cursor is on. `cursor == LINK_CODE_DIGITS` is the "past the last
/// digit" stop — the field is full and OK is the highlighted target, so
/// typing all eight digits with the keyboard leaves the player one press
/// from confirming (and Left/Backspace walk back into the field).
/// Digits are stored numerically so stepping is one wrap-around add, and
/// rendering is a `char` conversion.
struct LinkCodeModal {
    digits: [u8; LINK_CODE_DIGITS],
    cursor: usize,
    /// The last OK found a blocked code and swapped in the replacement.
    /// The modal stays open on the replacement (and says so) so the
    /// player can see — and share — the code a second OK will dial.
    /// Any edit clears it.
    replaced: bool,
}

/// The cursor stop that means "OK is the target" — one past the digits.
const OK_STOP: usize = LINK_CODE_DIGITS;

impl LinkCodeModal {
    /// Seed the draft from the strip's current code (all zeros when it
    /// has none, or holds a `/`-command the field can't represent).
    fn from_code(code: &str) -> Self {
        let mut modal = Self {
            digits: [0u8; LINK_CODE_DIGITS],
            cursor: 0,
            replaced: false,
        };
        if let Some(code) = crate::netplay::randomcode::canonical(code.trim()) {
            modal.set_code(&code);
        }
        modal
    }

    fn code(&self) -> String {
        self.digits.iter().map(|d| char::from(b'0' + d)).collect()
    }

    /// Overwrite the field from a canonical eight-digit code.
    fn set_code(&mut self, code: &str) {
        for (slot, byte) in self.digits.iter_mut().zip(code.bytes()) {
            *slot = byte.saturating_sub(b'0').min(9);
        }
    }

    /// Whether the draft is still the untouched all-zero field — the
    /// modal's "no code picked yet" state, which dials a random code.
    fn is_blank(&self) -> bool {
        self.digits.iter().all(|d| *d == 0)
    }

    /// Whether OK is the highlighted target (the field has been filled
    /// and the cursor walked off its end).
    fn ok_focused(&self) -> bool {
        self.cursor >= OK_STOP
    }

    /// Step the digit under the cursor, wrapping 9 ↔ 0 so a fat finger
    /// never dead-ends at the field's edge. Nothing to step once the
    /// cursor sits on OK.
    fn step(&mut self, by: i8) {
        if self.cursor >= OK_STOP {
            return;
        }
        let digit = i16::from(self.digits[self.cursor]) + i16::from(by);
        self.digits[self.cursor] = digit.rem_euclid(10) as u8;
        self.replaced = false;
    }

    fn move_cursor(&mut self, by: i8) {
        let cursor = self.cursor as i16 + i16::from(by);
        self.cursor = cursor.clamp(0, OK_STOP as i16) as usize;
    }

    /// Type a numeral straight into the field, the way a text box would:
    /// it lands on the cursor cell and the cursor steps to the next one,
    /// ending on the OK stop after the last digit.
    fn type_digit(&mut self, digit: u8) {
        if self.cursor >= OK_STOP {
            return;
        }
        self.digits[self.cursor] = digit.min(9);
        self.cursor = (self.cursor + 1).min(OK_STOP);
        self.replaced = false;
    }

    /// Backspace: clear the previous cell and step back onto it. From
    /// the OK stop that is the last digit, so the first Backspace after
    /// a full code reopens the field instead of confirming anything.
    fn erase(&mut self) {
        self.cursor = self.cursor.saturating_sub(1).min(LINK_CODE_DIGITS - 1);
        self.digits[self.cursor] = 0;
        self.replaced = false;
    }

    /// Fill the field from a fresh random code, cursor back at the
    /// start so the next press adjusts the first digit.
    fn randomize(&mut self) {
        self.set_code(&crate::netplay::randomcode::generate());
        self.cursor = 0;
        self.replaced = false;
    }
}

/// Which link-code actions a held-state currently satisfies. Read from
/// the user's key bindings, so the modal follows whatever the player
/// mapped their d-pad / A / B / X / START to.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct LinkActions {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    a: bool,
    b: bool,
    x: bool,
    start: bool,
}

impl LinkActions {
    fn read(mapping: &crate::platform::input::Mapping, held: &crate::platform::input::HeldState) -> Self {
        use crate::platform::input::MappedKey as K;
        let on = |key: K| mapping.slot(key).iter().any(|p| held.is_active(p));
        Self {
            up: on(K::Up),
            down: on(K::Down),
            left: on(K::Left),
            right: on(K::Right),
            a: on(K::A),
            b: on(K::B),
            x: on(K::X),
            start: on(K::Start),
        }
    }
}

/// Side-effects bubble-up. Mirrors the [`crate::tabs::replays::Effect`]
/// convention: pure UI-state mutations happen inside
/// [`State::update`]; anything that requires App-level
/// collaborators (session host, file system, clipboard) comes back
/// as an `Effect` for the caller to interpret.
#[derive(Debug)]
pub enum Effect {
    /// Kick off netplay. The `LinkIdent` variant tells the app
    /// handler whether to route via matchmaking signaling or direct
    /// TCP transport. `copy_code` is `Some` when the modal's OK found
    /// an untouched all-zero field and generated a code on the spot —
    /// the App puts it straight on the clipboard so the host can paste
    /// it to their opponent without hunting for the lobby's copy
    /// button.
    Connect {
        ident: crate::netplay::LinkIdent,
        copy_code: Option<String>,
    },
    /// Leave the lobby / cancel a connection attempt.
    Disconnect,
    /// Lobby match-type picker moved. App records it and resends Settings.
    SetMatchType((u8, u8)),
    /// Lobby "blind my setup" toggled. App records it, persists the
    /// choice, and resends Settings.
    SetBlindSetup(bool),
    /// Lobby Un-ready — drop our commitment and tell the peer.
    Unready,
    /// Lobby frame-delay slider moved. App persists `config.frame_delay`; it's
    /// this side's local frame delay (snapshotted into the match at
    /// start, not negotiated with the peer), so there's nothing live to update.
    SetFrameDelay(u32),
    /// Lobby Ready — App reads the local save SRAM and commits it.
    ReadyWithSave,
    /// `open::that(_)` on a file or folder.
    OpenPath(std::path::PathBuf),
    /// Reveal in the OS file manager with the file selected.
    RevealPath(std::path::PathBuf),
    /// Copy plain text to the clipboard.
    CopyText(String),
    /// Copy HTML to the clipboard with a plain-text alternative.
    CopyHtml { text: String, html: String },
    /// Copy a raster image to the clipboard.
    CopyImage(image::RgbaImage),
    /// User pressed Play → start a single-player session from the
    /// current selection.
    StartSinglePlayer,
    /// User pressed Training → start a training session (single-player
    /// ride + do-nothing dummy controller) from the current selection.
    StartTraining,
    /// Duplicate the currently-selected save file as `new_stem` (no
    /// extension; the handler keeps the source's).
    SaveDuplicate { new_stem: String },
    /// Rename the currently-selected save to `new_stem` (no
    /// extension; rename_save adds `.sav`).
    SaveRename { new_stem: String },
    /// Delete the currently-selected save file.
    SaveDelete,
    /// Create a fresh save in the saves dir from a bundled
    /// template.
    SaveNew {
        name: String,
        template: String,
        /// The concrete variant to create the save for (a family can
        /// have several owned-ROM variants). The handler looks up this
        /// game's template and adopts it as the loadout's game.
        game: rom::GameRef,
    },
    /// Task returned from the save view's `ui.update`. Generic pipe
    /// so save_editor-internal side effects (e.g. the scroll-to-top
    /// snap on tab change) flow through without per-feature
    /// Effect variants.
    SaveEditorTask(iced::Task<Message>),
    /// Global save editor committed: the edit session already staged
    /// everything into the in-memory save and serialized it — write
    /// `sram` to the loaded save's path on disk.
    SaveEditCommit { sram: Vec<u8> },
    /// Global save editor: discard all staged edits, reloading the on-disk
    /// original.
    SaveEditCancel,
}

impl State {
    /// Apply a tab message. See [`crate::tabs::replays::Effect`]
    /// for the side-effect surface convention.
    pub fn update(
        &mut self,
        msg: Message,
        scanners: &Catalog,
        config: &config::Config,
        loaded: Option<&mut selection::LoadedSave>,
        loadout: &Selection,
    ) -> Option<Effect> {
        let action_before = self.save_action.clone();
        let effect = self.update_inner(msg, scanners, config, loaded, loadout);
        let now = iced::time::Instant::now();
        // Freeze the closing form (with its draft) for the
        // save-action swap's exit half to render.
        if self.save_action == SaveAction::None && action_before != SaveAction::None {
            self.save_action_exit = action_before;
        }
        self.save_form.set(self.save_action != SaveAction::None, now);
        effect
    }

    /// Move a Fight-generated link code into the input. The App calls this
    /// the moment the lobby band leaves (failure, cancel, or handoff into a
    /// match), so the returning strip shows what was hosted and a retry
    /// re-hosts the same code — without the code ever rendering in the
    /// input on the way *into* the lobby.
    pub fn restore_generated_link_code(&mut self) {
        if let Some(code) = self.pending_generated_code.take() {
            if self.link_code.trim().is_empty() {
                self.link_code = code;
            }
        }
    }

    /// Prefill the link-code input from an external source — the CLI
    /// `Join <code>` argument or a `trill://join/<code>` deep link.
    /// Shorter digit strings are canonicalized so the field matches what
    /// we'll actually dial; anything else is kept as typed.
    pub fn adopt_link_code(&mut self, code: String) {
        self.link_code = crate::netplay::randomcode::canonical(code.trim()).unwrap_or(code);
    }

    /// Play the family-switch entrance: a family change replaces the
    /// entire bottom of the tab, so the whole save-view pane under the
    /// selector strip glides in.
    pub fn animate_family_switch(&mut self, now: iced::time::Instant) {
        self.save_body_enter.start(now);
    }

    /// Drop the tracked held inputs. The Play tab's input wrapper is
    /// their only source, so when it leaves the tree (tab switch,
    /// session start) a release it would have seen never arrives and
    /// the key would read as stuck — clearing stops that from
    /// swallowing the next press after coming back.
    pub fn forget_held(&mut self) {
        self.link_held = crate::platform::input::HeldState::default();
        self.link_actions = LinkActions::default();
    }

    /// Open the code-entry modal, seeded from the strip's current code.
    fn open_link_code_modal(&mut self) {
        self.link_code_modal = Some(LinkCodeModal::from_code(&self.link_code));
    }

    /// Whether the code-entry modal is up. While it is, it owns the
    /// d-pad — the App's focus ring stays out of its way.
    pub fn link_code_modal_open(&self) -> bool {
        self.link_code_modal.is_some()
    }

    /// Fold one physical input event into the strip's edge detection.
    /// Called by the App for every captured input event while the Play
    /// tab is on screen. With the modal closed only START acts (it
    /// opens the modal — the pad's way in); open, numerals/backspace
    /// type straight into the field, the d-pad edits digit/cursor, A or
    /// START accepts, and B dismisses. Returns the connect effect when
    /// an accept lands.
    pub fn link_code_input(
        &mut self,
        ev: &crate::platform::input::Event,
        config: &config::Config,
        scanners: &Catalog,
        loadout: &Selection,
    ) -> Option<Effect> {
        // A held key repeats its press, and a repeat must not retype
        // into the field — so note whether this key was already down
        // before folding the event in.
        let repeat = match ev {
            crate::platform::input::Event::Key {
                physical,
                pressed: true,
            } => self.link_held.is_active(&crate::platform::input::PhysicalInput::Key(
                crate::platform::input::KeyPhysical(*physical),
            )),
            _ => false,
        };
        self.link_held.apply(ev);
        let now = LinkActions::read(&config.input_mapping, &self.link_held);
        let before = std::mem::replace(&mut self.link_actions, now);
        // Press edge only: a held key must not spin the code.
        let pressed = |action: bool, was: bool| action && !was;

        if self.link_code_modal.is_none() {
            if pressed(now.start, before.start) {
                self.open_link_code_modal();
            }
            return None;
        }
        if !repeat {
            if let crate::platform::input::Event::Key {
                physical,
                pressed: true,
            } = ev
            {
                if let Some(digit) = typed_digit(physical) {
                    if let Some(modal) = self.link_code_modal.as_mut() {
                        modal.type_digit(digit);
                    }
                    return None;
                }
                if is_backspace(physical) {
                    if let Some(modal) = self.link_code_modal.as_mut() {
                        modal.erase();
                    }
                    return None;
                }
            }
        }
        if pressed(now.b, before.b) {
            self.link_code_modal = None;
            return None;
        }
        if pressed(now.x, before.x) {
            if let Some(modal) = self.link_code_modal.as_mut() {
                modal.randomize();
            }
            return None;
        }
        if let Some(modal) = self.link_code_modal.as_mut() {
            if pressed(now.up, before.up) {
                modal.step(1);
            }
            if pressed(now.down, before.down) {
                modal.step(-1);
            }
            if pressed(now.left, before.left) {
                modal.move_cursor(-1);
            }
            if pressed(now.right, before.right) {
                modal.move_cursor(1);
            }
        }
        if pressed(now.a, before.a) || pressed(now.start, before.start) {
            return self.confirm_link_code(scanners, loadout);
        }
        None
    }

    /// The modal's OK. A blocked code is swapped for its replacement and
    /// the modal stays open on the result (with the tip explaining why),
    /// so the player sees — and can share — the code they will actually
    /// host; a second OK dials it. An untouched all-zero field means
    /// "just get me a lobby" and dials a fresh random code, as the empty
    /// strip used to.
    fn confirm_link_code(&mut self, scanners: &Catalog, loadout: &Selection) -> Option<Effect> {
        let modal = self.link_code_modal.as_ref()?;
        let typed = modal.code();
        let canonical = crate::netplay::randomcode::canonical(&typed)?;
        if canonical != typed {
            if let Some(modal) = self.link_code_modal.as_mut() {
                modal.set_code(&canonical);
                modal.cursor = 0;
                modal.replaced = true;
            }
            // Keep the strip in step with what the modal now shows, so
            // closing and reopening doesn't quietly revert to the
            // blocked number that was typed.
            self.link_code = canonical;
            return None;
        }
        // The strip's CTA is disabled until the patch is ready, but
        // START can open the modal regardless — don't commit to a match
        // this side can't run.
        if !loadout.patch_ready(scanners) {
            return None;
        }
        let generated = self.link_code_modal.as_ref().is_some_and(LinkCodeModal::is_blank);
        let code = if generated {
            crate::netplay::randomcode::generate()
        } else {
            canonical
        };
        self.link_code_modal = None;
        if generated {
            // Never render a Fight-generated code in the outgoing strip
            // on the way into the lobby: park it and let the band swap
            // hand it back when the lobby leaves (`restore_generated_link_code`).
            self.pending_generated_code = Some(code.clone());
            crate::ui::copy_feedback::flash(lobby::LINK_CODE_FLASH_KEY);
        } else {
            self.link_code = code.clone();
        }
        Some(Effect::Connect {
            ident: crate::netplay::LinkIdent::Matchmaking(code.clone()),
            copy_code: generated.then_some(code),
        })
    }

    fn update_inner(
        &mut self,
        msg: Message,
        scanners: &Catalog,
        config: &config::Config,
        loaded: Option<&mut selection::LoadedSave>,
        loadout: &Selection,
    ) -> Option<Effect> {
        match msg {
            // Routed to the shared Selection at App level before this
            // dispatch is reached.
            Message::Loadout(_) => None,
            Message::LinkCodeModalClose => {
                self.link_code_modal = None;
                None
            }
            Message::LinkCodeCursorSet(index) => {
                if let Some(modal) = self.link_code_modal.as_mut() {
                    modal.cursor = index.min(LINK_CODE_DIGITS - 1);
                }
                None
            }
            Message::LinkCodeRandomize => {
                if let Some(modal) = self.link_code_modal.as_mut() {
                    modal.randomize();
                }
                None
            }
            Message::LinkCodeConfirm => self.confirm_link_code(scanners, loadout),
            Message::CopyText(s) => {
                // The lobby's copy-link-code button is the only
                // sender — light its "Copied!" flash as the text
                // heads for the clipboard.
                crate::ui::copy_feedback::flash(lobby::LINK_CODE_FLASH_KEY);
                Some(Effect::CopyText(s))
            }
            Message::FightPressed => {
                // The CTA opens the entry modal; the code (and the
                // connection) come from its OK.
                self.open_link_code_modal();
                None
            }
            Message::Noop => None,
            Message::Disconnect => Some(Effect::Disconnect),
            Message::SetMatchType(mt) => Some(Effect::SetMatchType(mt)),
            Message::SetFrameDelay(d) => Some(Effect::SetFrameDelay(d)),
            Message::SetBlindSetup(v) => Some(Effect::SetBlindSetup(v)),
            Message::Ready => Some(Effect::ReadyWithSave),
            Message::Unready => Some(Effect::Unready),
            Message::SaveEditor(msg) => {
                // `ui.update` folds the message into save-view state —
                // staging edits straight into the loaded data — and
                // hands back what's left for the App: clipboard copies,
                // launches, the committed SRAM. Everything else flows
                // through as a generic save-view-internal task.
                let data = loaded?;
                let (sv_task, outcome) = data.editor.update(&config.language, data, &*msg);
                match outcome {
                    Some(trill_gamesupport::SaveEditorEvent::CopyText(s)) => Some(Effect::CopyText(s)),
                    Some(trill_gamesupport::SaveEditorEvent::CopyHtml { text, html }) => {
                        Some(Effect::CopyHtml { text, html })
                    }
                    Some(trill_gamesupport::SaveEditorEvent::CopyImage(img)) => Some(Effect::CopyImage(img)),
                    Some(trill_gamesupport::SaveEditorEvent::Play) => Some(Effect::StartSinglePlayer),
                    Some(trill_gamesupport::SaveEditorEvent::Training) => Some(Effect::StartTraining),
                    Some(trill_gamesupport::SaveEditorEvent::Commit { sram }) => Some(Effect::SaveEditCommit { sram }),
                    Some(trill_gamesupport::SaveEditorEvent::Cancel) => Some(Effect::SaveEditCancel),
                    None => Some(Effect::SaveEditorTask(sv_task.map(Message::SaveEditor))),
                }
            }
            m @ (Message::SaveOpenFolder
            | Message::OpenSavesFolder(_)
            | Message::SaveDuplicateStart
            | Message::SaveDuplicateDraftChanged(_)
            | Message::SaveDuplicateConfirm
            | Message::SaveRenameStart
            | Message::SaveRenameDraftChanged(_)
            | Message::SaveRenameConfirm
            | Message::SaveDeleteStart
            | Message::SaveDeleteConfirm
            | Message::SaveActionCancel
            | Message::SaveNewStart
            | Message::SaveNewDraftChanged(_)
            | Message::SaveNewTemplateSelected(..)
            | Message::SaveNewConfirm) => self.update_save_manage(m, scanners, config, loadout),
        }
    }
}

/// The netplay-side inputs the Play tab needs to render its bottom
/// band, bundled so [`State::view`] doesn't take the App's netplay
/// internals as loose positional arguments.
pub struct LobbyBandCtx<'a> {
    pub phase: &'a crate::netplay::Phase,
    pub lobby: &'a crate::netplay::LobbyState,
    /// Derived ready-ladder projection (who's committed / match
    /// starting), same per-frame vintage as `lobby`.
    pub ready: crate::netplay::ReadyView,
    /// True between "both sides exchanged StartMatch" and the PvP
    /// session taking over: the selector strip goes inert and the
    /// lobby shows its "Starting match…" chrome.
    pub handoff_pending: bool,
    /// Two-phase swap between the bottom bands (link-code strip ↔
    /// lobby), driven by the App (which sees the netplay phase
    /// flip): first half sinks + dissolves the outgoing band,
    /// second half rises the incoming one out of the page surface.
    pub swap: &'a crate::ui::anim::Transition,
    /// The lobby's last live state, frozen by the App on the
    /// frame the band left — the exiting band renders from
    /// this so the verdict (e.g. the failure banner) doesn't
    /// flash to the idle handshake line mid-dissolve.
    pub exit_snapshot: Option<&'a (
        crate::netplay::Phase,
        crate::netplay::LobbyState,
        crate::netplay::ReadyView,
    )>,
    /// 0.0–1.0 of a B-hold that will leave the lobby, so the Leave
    /// button can show it filling up. `None` when B isn't held.
    pub cancel_hold: Option<f32>,
}

impl State {
    pub fn view<'a>(
        &'a self,
        lang: &'a LanguageIdentifier,
        scanners: &'a Catalog,
        loadout: &'a Selection,
        loaded: Option<&'a selection::LoadedSave>,
        streamer_mode: bool,
        config: &'a config::Config,
        downloads: &'a crate::library::patch::Downloads,
        // The startup scan hasn't landed yet, so empty scanners mean
        // "not read yet" rather than "nothing installed".
        scanning: bool,
        band: LobbyBandCtx<'a>,
    ) -> Element<'a, Message> {
        let now = iced::time::Instant::now();
        let mut save_body = self.body(
            lang,
            scanners,
            loadout,
            loaded,
            streamer_mode,
            config,
            scanning,
            band.phase,
        );
        // A family switch replaces the entire bottom of the tab, so
        // the whole pane glides in (the App starts `save_body_enter`
        // when `loadout.family()` flips); save switches within the
        // family animate inside the save view instead.
        if let Some(p) = self.save_body_enter.progress(now) {
            save_body = crate::ui::anim::slide_in(save_body, p, iced::Vector::new(0.0, 20.0));
        }

        // Selector strip + save-view body live inside a single
        // PANE_GAP-padded column so every pane in that area shares
        // the same inset from the window edges and gap from one
        // another. The hud_scanline_bottom + bottom band sit OUTSIDE
        // that padding so they remain edge-to-edge bottom bars.
        // The strip goes inert during the handoff window: the PvP
        // session is being built from the committed state and
        // selection changes would only confuse.
        //
        // It's absent entirely until the startup scan lands: there is
        // nothing to select from yet, so the pickers would offer empty
        // lists and then have the restored selection replace them under
        // the user. The scanning card holds the pane on its own.
        let mut inner = column![].spacing(style::PANE_GAP).padding(style::PANE_GAP).height(Fill);
        if !scanning {
            inner = inner.push(self.selector_strip(lang, scanners, loadout, config, downloads, band.handoff_pending));
        }
        let inner = inner.push(save_body);

        let mut col = column![].width(Fill).height(Fill);
        col = col.push(inner);
        // While a netplay attempt is in flight (Connecting /
        // Negotiating / Lobby, sticky Failed, handoff) the lobby IS
        // the bottom band — it carries the versus cards, match
        // settings, and the verdict/cancel/ready chrome, while the
        // save view above stays visible. Otherwise the normal bottom
        // strip handles the link code + Fight CTA.
        //
        // The swap between them is a two-phase morph on
        // `bottom_swap`'s unified timeline: the outgoing band sinks
        // while dissolving into the page surface, then the incoming
        // one rises out of it — so the code strip reads as turning
        // into the lobby and back, rather than one vanishing and the
        // other arriving. The bottom HUD scanline is grouped into the
        // moving band so it rides the motion instead of staying
        // pinned above it.
        let (render_lobby, swap) = crate::ui::anim::swap_phase(band.swap, now);
        let bottom: Element<'a, Message> = if render_lobby {
            // While the band is on its way OUT, the live phase has
            // already gone Idle (and the lobby may be wiped) — use
            // the snapshot the App froze on the band's last live
            // frame so the verdict doesn't flash mid-dissolve.
            let (band_phase, band_lobby, band_ready) = if !band.swap.shown() {
                band.exit_snapshot
                    .map(|(p, l, r)| (p, l, *r))
                    .unwrap_or((band.phase, band.lobby, band.ready))
            } else {
                (band.phase, band.lobby, band.ready)
            };
            // Synthesize the local side's Settings from the current
            // loadout so the "You" slot fills in immediately —
            // pre-Lobby phases haven't populated `lobby.local` yet,
            // but everything it needs is already on hand locally.
            // Same builder the netplay loop uses to ship settings on
            // the wire, so the visible info during the handshake
            // exactly matches what gets sent.
            let local_fallback = loadout_strip::local_settings(loadout, config, band.lobby);
            lobby::Lobby {
                lang,
                state: band_lobby,
                ready: band_ready,
                phase: band_phase,
                local_game: loadout.game(),
                scanners,
                has_save: loadout.is_playable(),
                local_fallback,
                streamer_mode,
                handoff_pending: band.handoff_pending,
                frame_delay: config.frame_delay,
                downloads,
                cancel_hold: band.cancel_hold,
            }
            .view()
        } else {
            self.bottom_strip(lang, streamer_mode, loadout.patch_ready(scanners))
        };
        let mut group: Element<'a, Message> = column![widgets::hud_scanline_bottom(), bottom].width(Fill).into();
        if let Some(phase) = swap {
            let dist = if render_lobby { 48.0 } else { 24.0 };
            group =
                crate::ui::anim::swap_transform(group, phase, iced::Vector::new(0.0, dist), |theme: &iced::Theme| {
                    theme.palette().background
                });
        }
        col = col.push(group);
        // The code-entry modal floats over the whole tab, but only
        // while the strip (not the lobby) is the bottom band — the
        // lobby has already taken the code.
        let body: Element<'a, Message> = col.into();
        match self.link_code_modal_view(lang, streamer_mode).filter(|_| !render_lobby) {
            Some(modal) => iced::widget::stack![body, modal].into(),
            None => body,
        }
    }

    /// Picks between the save view, an empty-state hint, or a "pick a
    /// save" hint based on what the user has installed and selected.
    fn body<'a>(
        &'a self,
        lang: &'a LanguageIdentifier,
        scanners: &'a Catalog,
        loadout: &'a Selection,
        loaded: Option<&'a selection::LoadedSave>,
        streamer_mode: bool,
        config: &'a config::Config,
        scanning: bool,
        netplay_phase: &'a crate::netplay::Phase,
    ) -> Element<'a, Message> {
        // The startup scan is still running. Gated on the scan itself,
        // not on any one scanner being empty: the four phases publish as
        // they finish, so between ROMs landing and saves landing an
        // emptiness test reads as "you own games but no saves" — and
        // with the strip hidden there'd be nothing on screen to fix it
        // with. One state until the whole scan is in. Same card shape as
        // the empty states below, so landing on the real one doesn't
        // move anything.
        if scanning {
            return empty_state_card(
                t!(lang, "empty-scanning-title"),
                vec![t!(lang, "empty-scanning-body")],
                None,
            );
        }
        // No ROMs at all: explain where to put them.
        if scanners.roms.read().is_empty() {
            let roms_path = config.roms_path();
            return empty_state_card(
                t!(lang, "empty-no-roms-title"),
                vec![t!(lang, "empty-no-roms-body"), roms_path.display().to_string()],
                Some((t!(lang, "save-open-folder"), roms_path)),
            );
        }
        // Family selected but no save files anywhere in it.
        if let Some(family) = loadout.family() {
            let saves = scanners.saves.read();
            let has_saves =
                game::games_in_family(family).any(|g| saves.get(&g).map(|v| !v.is_empty()).unwrap_or(false));
            if !has_saves && loadout.save().is_none() {
                let saves_path = config.saves_path();
                return empty_state_card(
                    t!(lang, "empty-no-saves-title"),
                    vec![t!(lang, "empty-no-saves-body"), saves_path.display().to_string()],
                    Some((t!(lang, "save-open-folder"), saves_path)),
                );
            }
        }
        self.save_editor(
            lang,
            loaded,
            streamer_mode,
            netplay_phase,
            loadout.patch_ready(scanners),
        )
    }

    fn selector_strip<'a>(
        &'a self,
        lang: &'a LanguageIdentifier,
        scanners: &'a Catalog,
        loadout: &'a Selection,
        config: &'a config::Config,
        downloads: &'a crate::library::patch::Downloads,
        inert: bool,
    ) -> Element<'a, Message> {
        // Inert during the PvP handoff window — the loadout pickers
        // reroute to Noop so a mid-spawn selection change can't
        // contradict the committed state, without the strip changing
        // shape.
        let gate = move |m: loadout_strip::Message| if inert { Message::Noop } else { Message::Loadout(m) };
        let game_row: Element<'a, Message> =
            loadout_strip::game_row(loadout, lang, scanners, config, downloads).map(gate);
        let save_picker: Element<'a, Message> =
            Element::from(loadout_strip::save_picker(loadout, lang, scanners, config).width(Length::Fill)).map(gate);
        let save_row = self.save_action_row(lang, scanners, loadout, save_picker);

        container(column![game_row, save_row].spacing(6))
            .padding(style::PANE_PADDING)
            .width(Fill)
            .style(widgets::pane)
            .into()
    }

    fn save_editor<'a>(
        &'a self,
        lang: &'a LanguageIdentifier,
        loaded: Option<&'a selection::LoadedSave>,
        streamer_mode: bool,
        netplay_phase: &'a crate::netplay::Phase,
        patch_ready: bool,
    ) -> Element<'a, Message> {
        let playable = matches!(netplay_phase, crate::netplay::Phase::Idle) && patch_ready;
        let Some(loaded) = loaded else {
            return container(text(t!(lang, "play-no-selection")).size(TEXT_BODY))
                .center(Fill)
                .into();
        };
        // Play button is the singleplayer entry point — disabled
        // whenever a netplay attempt is anywhere in flight so it
        // can't fight with the lobby for the same save/emulator slot,
        // and while the selected patch is still downloading, since the
        // session would otherwise boot the game unpatched.
        let play_button = Some(playable);
        loaded
            .editor
            .view(
                lang,
                loaded,
                streamer_mode,
                play_button,
                true,
                // The save editor is always available (no longer experimental).
                true,
            )
            .map(Message::SaveEditor)
    }

    /// The idle bottom band: the Fight CTA alone. The lobby band
    /// replaces this strip for every in-flight netplay phase, so this
    /// strip is pure "pick a code and fight" — singleplayer lives in the
    /// save view's Play button. There is no text box: the CTA opens the
    /// entry modal ([`State::link_code_modal_view`]), and the pad's
    /// START does the same. A code left untouched in the modal (the
    /// all-zero field) is fair game for "just get me a lobby": OK then
    /// generates one on the spot and never renders it here on the way
    /// in — it debuts in the lobby's connection line and only lands in
    /// this strip when the lobby band leaves. The muted line at the left
    /// reports what a retry would dial, masked in streamer mode (which
    /// the modal masks too — typed, pad-entered, and restored-generated
    /// codes are all scrapeable off a stream otherwise).
    ///
    /// `patch_ready` is false while the selected patch is still being
    /// downloaded: committing to a match then would negotiate a setup we
    /// can't actually run, so the CTA waits alongside the download the
    /// version slot is already reporting.
    fn bottom_strip<'a>(
        &'a self,
        lang: &'a LanguageIdentifier,
        streamer_mode: bool,
        patch_ready: bool,
    ) -> Element<'a, Message> {
        const BOTTOM_SIZE: f32 = 15.0;
        const BOTTOM_CTA_PAD: [f32; 2] = [10.0, 22.0];
        let trimmed = self.link_code.trim();
        let fight_button: Element<'a, Message> = {
            // Same chrome as the lobby's Ready button — both are
            // "commit to a match" CTAs. ready_button_style for
            // ReadyPalette::Idle falls back to neutral when the
            // button is disabled, so the not-patch-ready case
            // renders as a plain greyed-out pill without a
            // separate branch here.
            let label = row![
                Icon::Swords.widget().size(BOTTOM_SIZE),
                text(t!(lang, "play-fight")).size(BOTTOM_SIZE),
            ]
            .spacing(8)
            .align_y(Alignment::Center);
            let mut btn = button(label)
                .padding(BOTTOM_CTA_PAD)
                .height(Length::Fixed(crate::ui::style::BAR_CONTROL_HEIGHT))
                .style(|theme: &iced::Theme, status| ready_button_style(theme, status, ReadyPalette::Idle));
            if patch_ready {
                btn = btn.on_press(Message::FightPressed);
            }
            crate::ui::focus::on_press(Message::FightPressed, btn)
        };
        // What OK will dial, for the next attempt: the code the last
        // attempt used, or the hint that opens the modal while there
        // isn't one yet.
        let (status, status_style): (String, fn(&iced::Theme) -> iced::widget::text::Style) = if trimmed.is_empty() {
            (t!(lang, "play-link-code-start"), widgets::muted_text_style)
        } else if streamer_mode {
            (
                t!(lang, "lobby-link-code", code = "•".repeat(LINK_CODE_DIGITS)),
                widgets::muted_text_style,
            )
        } else {
            (
                t!(lang, "lobby-link-code", code = self.link_code.clone()),
                |theme: &iced::Theme| iced::widget::text::Style {
                    color: Some(theme.palette().text),
                },
            )
        };
        let status: Element<'a, Message> = text(status).size(TEXT_CAPTION).style(status_style).into();

        container(
            row![status, iced::widget::space::horizontal(), fight_button]
                .spacing(10)
                .align_y(Alignment::Center)
                .padding([10, 8]),
        )
        .width(Fill)
        .style(widgets::hud_bar)
        .into()
    }

    /// The code-entry modal: an 8-digit field the pad (or keyboard)
    /// edits in place, with OK and Random, over the shared dim wash.
    /// Purely presentational — every action arrives as a message, and
    /// the physical side is [`State::link_code_input`].
    ///
    /// Streamer mode masks every cell: the point of that mode is that a
    /// code never reaches the stream, and typing one in a visible field
    /// would leak it just as surely as showing the strip did.
    fn link_code_modal_view<'a>(
        &'a self,
        lang: &'a LanguageIdentifier,
        streamer_mode: bool,
    ) -> Option<Element<'a, Message>> {
        const DIGIT_SIZE: f32 = 26.0;
        const CELL_W: f32 = 40.0;
        let modal = self.link_code_modal.as_ref()?;

        let mut cells = row![].spacing(6).align_y(Alignment::Center);
        for (index, digit) in modal.digits.iter().enumerate() {
            let active = index == modal.cursor;
            let shown = if streamer_mode {
                "•".to_string()
            } else {
                digit.to_string()
            };
            let cell = container(text(shown).size(DIGIT_SIZE))
                .width(Length::Fixed(CELL_W))
                .height(Length::Fixed(CELL_W * 1.3))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |theme: &iced::Theme| link_code_cell(theme, active));
            cells = cells.push(
                button(cell)
                    .padding(0)
                    .style(widgets::flat)
                    .on_press(Message::LinkCodeCursorSet(index)),
            );
        }

        // OK takes the highlight when the cursor has walked off the end
        // of the field: the same accent treatment a focused digit cell
        // gets, plus a ring, so "press A / Enter to confirm" is visible
        // without a legend lookup.
        let ok_focused = modal.ok_focused();
        let ok = button(text(t!(lang, "play-link-code-ok")).size(TEXT_BODY))
            .padding([10.0, 18.0])
            .style(move |theme: &iced::Theme, status| link_code_ok(theme, status, ok_focused))
            .on_press(Message::LinkCodeConfirm);
        let randomize = button(
            row![
                Icon::Dices.widget().size(TEXT_BODY),
                text(t!(lang, "play-link-code-randomize")).size(TEXT_BODY),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([10.0, 14.0])
        .style(widgets::neutral)
        .on_press(Message::LinkCodeRandomize);

        let panel = container(
            column![
                row![
                    text(t!(lang, "play-link-code-title")).size(TEXT_TITLE),
                    iced::widget::space::horizontal(),
                    widgets::icon_button_styled(
                        Icon::X,
                        t!(lang, "playback-close"),
                        Some(Message::LinkCodeModalClose),
                        [4.0, 8.0],
                        widgets::window_close,
                    ),
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .width(Fill),
                row![cells, ok, randomize].spacing(12).align_y(Alignment::Center),
                text(t!(lang, "play-link-code-hint"))
                    .size(TEXT_CAPTION)
                    .style(widgets::muted_text_style),
                // The all-zero field doubles as "no code picked yet",
                // which OK turns into a random one — say so, or that
                // code looks like a deliberate choice. The line keeps
                // its height either way so the panel doesn't resize
                // the moment the field leaves the all-zero state.
                container(
                    text(if modal.is_blank() {
                        t!(lang, "play-link-code-blank")
                    } else {
                        String::new()
                    })
                    .size(TEXT_CAPTION)
                    .style(widgets::muted_text_style),
                )
                .height(Length::Fixed(TEXT_CAPTION * 1.5)),
                // What each pad button does, in the pad's own glyphs —
                // letters on filled discs, so they read as controller
                // buttons rather than keyboard keys.
                row![
                    pad_legend("A", t!(lang, "play-link-code-confirm")),
                    pad_legend("B", t!(lang, "play-link-code-cancel")),
                    pad_legend("X", t!(lang, "play-link-code-randomize")),
                ]
                .spacing(16)
                .align_y(Alignment::Center),
            ]
            .spacing(18)
            .align_x(Alignment::Center),
        )
        .padding(24)
        .width(Length::Fill)
        .max_width(700.0)
        .style(widgets::panel);

        // A blocked code is swapped in place rather than dialled, so say
        // so and leave the replacement on screen — the player has to
        // share the code they'll actually host.
        let panel: Element<'a, Message> = if modal.replaced {
            let tip = container(
                text(t!(lang, "play-link-code-replaced"))
                    .size(TEXT_CAPTION)
                    .style(widgets::primary_text_style),
            )
            .padding([8.0, 12.0])
            .width(Length::Fill)
            .style(|theme: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(theme.extended_palette().background.weak.color)),
                border: iced::Border {
                    radius: widgets::tech_radius(8.0),
                    width: 1.0,
                    color: theme.palette().primary,
                },
                ..Default::default()
            });
            let column = column![panel, tip].spacing(12).align_x(Alignment::Center);
            container(column).padding(0).width(Length::Fill).max_width(700.0).into()
        } else {
            panel.into()
        };

        Some(widgets::modal_layer(
            panel,
            0.55,
            Message::Noop,
            Some(Message::LinkCodeModalClose),
        ))
    }
}

/// The modal's OK button. At rest it is the plain primary CTA; once the
/// cursor reaches it (the field has been filled), it takes the accent
/// treatment the focused digit cell gets, plus a ring and glow — the
/// signal that A / Enter confirms.
fn link_code_ok(
    theme: &iced::Theme,
    status: iced::widget::button::Status,
    focused: bool,
) -> iced::widget::button::Style {
    let mut style = widgets::primary_button(theme, status);
    if focused {
        let primary = theme.palette().primary;
        style.border = iced::Border {
            radius: widgets::tech_radius(10.0),
            width: 2.0,
            color: widgets::mix(primary, iced::Color::WHITE, 0.65),
        };
        style.shadow = iced::Shadow {
            color: iced::Color { a: 0.8, ..primary },
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 18.0,
        };
    }
    style
}

/// One cell of the code modal: the active digit sits on the accent,
/// the rest on the quiet plate so the cursor reads at a glance.
fn link_code_cell(theme: &iced::Theme, active: bool) -> iced::widget::container::Style {
    let p = theme.extended_palette();
    let primary = theme.palette().primary;
    if active {
        iced::widget::container::Style {
            background: Some(iced::Background::Color(primary)),
            text_color: Some(widgets::on_accent(primary)),
            border: iced::Border {
                radius: widgets::tech_radius(8.0),
                width: 2.0,
                color: widgets::mix(primary, iced::Color::WHITE, 0.45),
            },
            ..Default::default()
        }
    } else {
        iced::widget::container::Style {
            background: Some(iced::Background::Color(p.background.weak.color)),
            text_color: Some(theme.palette().text),
            border: iced::Border {
                radius: widgets::tech_radius(8.0),
                width: 1.0,
                color: p.background.strong.color,
            },
            ..Default::default()
        }
    }
}

/// The numeral a physical key types into the code field — the number
/// row and the numpad both count, so a keyboard player can enter a code
/// the way they would in any text box.
fn typed_digit(key: &iced::keyboard::key::Physical) -> Option<u8> {
    use iced::keyboard::key::{Code, Physical};
    let Physical::Code(code) = key else {
        return None;
    };
    Some(match code {
        Code::Digit0 | Code::Numpad0 => 0,
        Code::Digit1 | Code::Numpad1 => 1,
        Code::Digit2 | Code::Numpad2 => 2,
        Code::Digit3 | Code::Numpad3 => 3,
        Code::Digit4 | Code::Numpad4 => 4,
        Code::Digit5 | Code::Numpad5 => 5,
        Code::Digit6 | Code::Numpad6 => 6,
        Code::Digit7 | Code::Numpad7 => 7,
        Code::Digit8 | Code::Numpad8 => 8,
        Code::Digit9 | Code::Numpad9 => 9,
        _ => return None,
    })
}

fn is_backspace(key: &iced::keyboard::key::Physical) -> bool {
    matches!(
        key,
        iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Backspace)
    )
}

/// One pad-button legend entry: the button's letter on a filled disc,
/// then what it does. The disc is what makes it read as a controller
/// button — a bare `A` would look like a keyboard key.
fn pad_legend<'a>(button: &'static str, label: String) -> Element<'a, Message> {
    const DISC: f32 = 20.0;
    row![
        container(text(button).size(TEXT_CAPTION))
            .width(Length::Fixed(DISC))
            .height(Length::Fixed(DISC))
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(pad_disc),
        text(label).size(TEXT_CAPTION).style(widgets::muted_text_style),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

/// The disc behind a [`pad_legend`] letter: lifted plate fill with
/// whichever ink stays readable on it, and a full radius so it reads as
/// a circle rather than a chip.
fn pad_disc(theme: &iced::Theme) -> iced::widget::container::Style {
    let fill = widgets::plate_lift(theme);
    iced::widget::container::Style {
        background: Some(iced::Background::Color(fill)),
        text_color: Some(widgets::on_accent(fill)),
        border: iced::Border {
            radius: iced::border::Radius::from(10.0),
            width: 1.0,
            color: theme.extended_palette().background.strong.color,
        },
        ..Default::default()
    }
}

// ---------- Save-action form helpers ----------

/// Centered card used for the no-roms / no-saves hints. Title is
/// rendered larger, body lines stack underneath in muted text.
/// When `folder` is provided, appends an "Open Folder" button —
/// usually the same path as the body's last line, so the user
/// can click straight through instead of copy-pasting it into
/// their file manager.
fn empty_state_card(
    title: String,
    body_lines: Vec<String>,
    open_folder: Option<(String, std::path::PathBuf)>,
) -> Element<'static, Message> {
    let mut col = column![
        // Lucide "info" glyph sized up so the card has a clear
        // visual anchor — without it the empty state was just a
        // floating title + paragraph, which read as a flash of
        // text rather than a deliberate placeholder.
        Icon::Info.widget().size(28.0),
        text(title).size(TEXT_TITLE),
    ]
    .spacing(10)
    .align_x(Alignment::Center);
    for line in body_lines {
        col = col.push(text(line).size(TEXT_CAPTION).style(widgets::muted_text_style));
    }
    if let Some((label, path)) = open_folder {
        col = col.push(Space::new().height(4)).push(widgets::labeled_icon_button(
            Icon::Folder,
            label,
            Message::OpenSavesFolder(path),
            STANDARD_PADDING,
            widgets::neutral,
        ));
    }
    container(container(col.padding(28).max_width(520)).style(widgets::panel))
        .padding(24)
        .center(Fill)
        .into()
}

// ---------- Commit-button styling (Fight + Ready) ----------

/// Which ready-button state we're painting. Drives
/// [`ready_button_style`]'s color choice.
#[derive(Clone, Copy)]
enum ReadyPalette {
    /// Pre-commit; the action is "ready up". Accent (primary) so
    /// it reads as the call-to-action in the strip.
    Idle,
    /// Locally committed; the action is "unready". Neutral / gray —
    /// the commitment isn't a celebration to surface in green;
    /// what matters is the user can un-commit.
    Committed,
    /// Both committed; match is spinning up. Rendered as a passive
    /// indicator: muted background, no click target, no border.
    /// Caller sets `on_press = None` to match the disabled look.
    Starting,
}

/// Custom style for the lobby's Ready toggle. Three discrete
/// moods — each one its own visual register so a glance at the
/// button tells the whole story of "where are we in the
/// handshake".
///
/// * Idle — primary_button on steroids: brighter gradient, huge
///   primary glow, chunky 2 px border. This is the moment the user is
///   supposed to slam the button, so it has to feel hot.
/// * Committed — neutral beveled plate. We've ack'd locally and are
///   waiting on the peer; the only useful action is to take it back,
///   which is not a celebration.
/// * Starting — flat muted badge. Both sides committed; the button is
///   now purely a status indicator with no click target.
fn ready_button_style(theme: &iced::Theme, status: button::Status, palette: ReadyPalette) -> button::Style {
    let p = theme.extended_palette();
    let primary = theme.palette().primary;
    match palette {
        ReadyPalette::Starting => button::Style {
            background: Some(iced::Background::Color(p.background.weak.color)),
            text_color: widgets::muted_color(theme),
            border: iced::Border {
                radius: widgets::tech_radius(10.0),
                width: 1.0,
                color: p.background.strong.color,
            },
            ..Default::default()
        },
        ReadyPalette::Committed => {
            // Defer to the shared beveled neutral so the
            // un-ready toggle looks like a sibling of the other
            // chunky neutral buttons in the lobby strip.
            crate::ui::widgets::neutral(theme, status)
        }
        ReadyPalette::Idle => {
            // Disabled state defers to the standard neutral
            // button so it reads as a plainly-greyed-out button
            // — the dim-primary-fill version this used to
            // render looked like a corrupted variant of the
            // lit-up state rather than a disabled affordance.
            if matches!(status, button::Status::Disabled) {
                return crate::ui::widgets::neutral(theme, status);
            }
            // Inline expansion of the battle-button kernel with
            // every dial cranked: bigger glow, brighter top stop,
            // 2 px border so the button reads as a console
            // affordance rather than a CSS rectangle.
            let lighter = widgets::mix(primary, iced::Color::WHITE, 0.30);
            let darker = widgets::mix(primary, iced::Color::BLACK, 0.25);
            let (top, bottom, glow_alpha, offset_y, blur) = match status {
                button::Status::Hovered => (
                    widgets::mix(lighter, iced::Color::WHITE, 0.18),
                    widgets::mix(primary, iced::Color::WHITE, 0.05),
                    0.95,
                    8.0,
                    28.0,
                ),
                button::Status::Pressed => (darker, widgets::mix(darker, iced::Color::BLACK, 0.12), 0.35, 2.0, 14.0),
                button::Status::Disabled => unreachable!("handled above"),
                button::Status::Active => (lighter, darker, 0.75, 6.0, 22.0),
            };
            button::Style {
                background: Some(iced::Background::Gradient(iced::Gradient::Linear(
                    iced::gradient::Linear::new(0.0)
                        .add_stop(0.0, top)
                        .add_stop(1.0, bottom),
                ))),
                text_color: widgets::on_accent(primary),
                border: iced::Border {
                    radius: widgets::tech_radius(10.0),
                    width: 2.0,
                    color: widgets::mix(primary, iced::Color::WHITE, 0.45),
                },
                shadow: iced::Shadow {
                    color: iced::Color {
                        a: glow_alpha,
                        ..primary
                    },
                    offset: iced::Vector::new(0.0, offset_y),
                    blur_radius: blur,
                },
                snap: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(digits: &str) -> LinkCodeModal {
        let mut modal = LinkCodeModal::from_code("");
        for byte in digits.bytes() {
            modal.type_digit(byte - b'0');
        }
        modal
    }

    #[test]
    fn typing_a_full_code_lands_on_ok() {
        let modal = typed("12345678");
        assert_eq!(modal.code(), "12345678");
        assert_eq!(modal.cursor, OK_STOP);
        assert!(modal.ok_focused());
    }

    #[test]
    fn a_partial_code_keeps_the_cursor_in_the_field() {
        let modal = typed("1234");
        assert_eq!(modal.cursor, 4);
        assert!(!modal.ok_focused());
    }

    #[test]
    fn the_ok_stop_ignores_further_typing_and_stepping() {
        let mut modal = typed("12345678");
        modal.type_digit(9);
        assert_eq!(modal.code(), "12345678");
        modal.step(1);
        assert_eq!(modal.code(), "12345678");
        assert!(modal.ok_focused());
    }

    #[test]
    fn left_or_backspace_walks_back_into_a_full_field() {
        let mut walked = typed("12345678");
        walked.move_cursor(-1);
        assert_eq!(walked.cursor, LINK_CODE_DIGITS - 1);
        assert_eq!(walked.code(), "12345678", "walking back keeps the digits");

        let mut erased = typed("12345678");
        erased.erase();
        assert_eq!(erased.cursor, LINK_CODE_DIGITS - 1);
        assert_eq!(erased.code(), "12345670", "backspace clears the last cell");
        assert!(!erased.ok_focused());

        // Re-typing that digit fills the field and returns to OK.
        erased.type_digit(8);
        assert_eq!(erased.code(), "12345678");
        assert!(erased.ok_focused());
    }

    #[test]
    fn the_cursor_cannot_walk_past_ok() {
        let mut modal = typed("12345678");
        modal.move_cursor(1);
        assert_eq!(modal.cursor, OK_STOP);
        modal.move_cursor(-9);
        assert_eq!(modal.cursor, 0);
    }
}

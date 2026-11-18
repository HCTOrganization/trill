//! Controller focus navigation for the menu screens.
//!
//! There is no built-in focus model for widgets in iced — only text
//! inputs hold keyboard focus. So this module keeps its own: the views
//! call [`focusable`] around every control that should take part, in
//! the order they appear, and the App moves a single index through that
//! list with the pad's d-pad. A is "press the focused thing"; left/right
//! adjust it first if it offers an adjustment (a picker or slider),
//! otherwise they move the ring too. Navigation is pad-only — the shell
//! drops keyboard events before they reach the ring, so a key bound for
//! the pad can't take over the menu.
//!
//! Why a thread-local registry rather than plumbing a handle through
//! every view: the ring has to be registered from a dozen view
//! functions across the tabs, and threading a borrow through all of
//! them would touch far more than it buys. Everything here is
//! `!Sync` and used only from the UI thread — the shell installs the
//! frame in [`begin_frame`] at the top of `App::view` and the input
//! side reads it back from `App::update`. Nothing is shared across
//! threads, and nothing survives a frame except the focus index, which
//! the App owns.
//!
//! Wrapping a control changes nothing about how it behaves under the
//! mouse: the wrapper lays out and forwards events exactly like its
//! child, and only paints when it is the focused item.

use std::cell::RefCell;

use iced::advanced::renderer;
use iced::advanced::renderer::Renderer as _;
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{layout, overlay, Clipboard, Layout, Shell, Widget};
use iced::{mouse, Background, Border, Color, Element, Event, Length, Rectangle, Size, Vector};

use super::widgets;
use crate::app::Message as AppMessage;
use crate::tabs;

/// A control's pad action, in whichever message vocabulary it was built
/// with. The tabs render their own message types and the shell maps them
/// at the boundary, so a tab-local control registers its own message here
/// and the shell converts once, at dispatch.
#[derive(Clone)]
pub enum Command {
    App(AppMessage),
    Play(tabs::play::Message),
    Loadout(tabs::play::loadout_strip::Message),
    Replays(tabs::replays::Message),
    Patches(tabs::patches::Message),
    Settings(tabs::settings::Message),
}

impl Command {
    /// The shell-level message this command dispatches.
    pub fn into_message(self) -> AppMessage {
        match self {
            Self::App(msg) => msg,
            Self::Play(msg) => AppMessage::Play(msg),
            Self::Loadout(msg) => AppMessage::Play(tabs::play::Message::Loadout(msg)),
            Self::Replays(msg) => AppMessage::Replays(msg),
            Self::Patches(msg) => AppMessage::Patches(msg),
            Self::Settings(msg) => AppMessage::Settings(msg),
        }
    }
}

impl From<tabs::play::loadout_strip::Message> for Command {
    fn from(msg: tabs::play::loadout_strip::Message) -> Self {
        Self::Loadout(msg)
    }
}

impl From<AppMessage> for Command {
    fn from(msg: AppMessage) -> Self {
        Self::App(msg)
    }
}

impl From<tabs::play::Message> for Command {
    fn from(msg: tabs::play::Message) -> Self {
        Self::Play(msg)
    }
}

impl From<tabs::replays::Message> for Command {
    fn from(msg: tabs::replays::Message) -> Self {
        Self::Replays(msg)
    }
}

impl From<tabs::patches::Message> for Command {
    fn from(msg: tabs::patches::Message) -> Self {
        Self::Patches(msg)
    }
}

impl From<tabs::settings::Message> for Command {
    fn from(msg: tabs::settings::Message) -> Self {
        Self::Settings(msg)
    }
}

/// What the focused control does with the pad's buttons. `activate` is
/// A; `left`/`right` let one control consume horizontal input for
/// itself (a picker stepping through its options, a slider stepping its
/// value) instead of letting it move the ring.
#[derive(Clone, Default)]
pub struct Actions {
    pub activate: Option<Command>,
    pub left: Option<Command>,
    pub right: Option<Command>,
}

impl Actions {
    /// A plain "press me" control: A activates it.
    pub fn activate(msg: impl Into<Command>) -> Self {
        Self {
            activate: Some(msg.into()),
            ..Self::default()
        }
    }

    /// A control left/right adjust in place. A is left alone (pickers
    /// and sliders have no single "press").
    pub fn adjust(left: impl Into<Command>, right: impl Into<Command>) -> Self {
        Self {
            activate: None,
            left: Some(left.into()),
            right: Some(right.into()),
        }
    }
}

/// One frame's worth of registered controls. Cleared at
/// [`begin_frame`], appended to by each [`focusable`] in view-build
/// order.
#[derive(Default)]
struct Frame {
    focus: Option<usize>,
    actions: Vec<Actions>,
}

thread_local! {
    static FRAME: RefCell<Frame> = RefCell::new(Frame::default());
}

/// Start a view build: adopt the App's current focus and forget last
/// frame's controls.
pub fn begin_frame(focus: Option<usize>) {
    FRAME.with(|frame| {
        let mut frame = frame.borrow_mut();
        frame.focus = focus;
        frame.actions.clear();
    });
}

/// How many controls the last built frame registered.
pub fn len() -> usize {
    FRAME.with(|frame| frame.borrow().actions.len())
}

/// The focused control's "press" message, if it has one.
pub fn activation(focus: Option<usize>) -> Option<Command> {
    let index = focus?;
    FRAME.with(|frame| frame.borrow().actions.get(index)?.activate.clone())
}

/// The focused control's message for a horizontal step, if it adjusts
/// in place. `forward` is +1 (right).
pub fn adjustment(focus: Option<usize>, forward: bool) -> Option<Command> {
    let index = focus?;
    FRAME.with(|frame| {
        let frame = frame.borrow();
        let actions = frame.actions.get(index)?;
        if forward {
            actions.right.clone()
        } else {
            actions.left.clone()
        }
    })
}

/// Move the ring by `step` through the frame's controls, wrapping at
/// either end so a short list never dead-ends. From "nothing focused",
/// forward enters at the top and backward at the bottom.
pub fn nudge(focus: &mut Option<usize>, step: isize) {
    let count = len() as isize;
    if count == 0 {
        *focus = None;
        return;
    }
    *focus = Some(match *focus {
        Some(index) => (index as isize + step).rem_euclid(count) as usize,
        None if step >= 0 => 0,
        None => (count - 1) as usize,
    });
}

/// Forget the ring — the screen it belonged to is gone.
pub fn clear(focus: &mut Option<usize>) {
    *focus = None;
}

/// Whether the control built with `index` is the focused one. Read at
/// draw time so the ring follows the index without a rebuild.
fn is_focused(index: usize) -> bool {
    FRAME.with(|frame| frame.borrow().focus == Some(index))
}

/// Register `content` as one stop on the focus ring, and draw the focus
/// ring around it while it is focused.
pub fn focusable<'a, M: Clone + 'a>(actions: Actions, content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    let index = FRAME.with(|frame| {
        let mut frame = frame.borrow_mut();
        frame.actions.push(actions);
        frame.actions.len() - 1
    });
    Element::new(Focusable {
        index,
        content: content.into(),
    })
}

/// [`focusable`] for the common case: a control that just wants A.
pub fn on_press<'a, M: Clone + 'a>(msg: impl Into<Command>, content: impl Into<Element<'a, M>>) -> Element<'a, M> {
    focusable(Actions::activate(msg), content)
}

/// The wrapper itself. Deliberately a transparent pass-through: layout,
/// events, mouse interaction and overlays all belong to the child, and
/// the only thing this adds is a stroke painted over the child's
/// bounds while focused.
struct Focusable<'a, M> {
    index: usize,
    content: Element<'a, M>,
}

impl<'a, M: Clone + 'a> Widget<M, iced::Theme, iced::Renderer> for Focusable<'a, M> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(&tree.children[0], renderer, theme, style, layout, cursor, viewport);

        if !is_focused(self.index) {
            return;
        }
        // Two rings: a dark one just outside the accent one, so the
        // outline stays visible on both light plates and on the accent
        // itself (a single accent stroke disappears on an accent-filled
        // button, and several of these controls are accent-filled).
        let bounds = layout.bounds();
        for (grow, color) in [
            (2.0, Color::from_rgba(0.0, 0.0, 0.0, 0.55)),
            (0.0, theme.palette().primary),
        ] {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x - grow,
                        y: bounds.y - grow,
                        width: bounds.width + grow * 2.0,
                        height: bounds.height + grow * 2.0,
                    },
                    border: Border {
                        radius: widgets::tech_radius(8.0),
                        width: 2.0,
                        color,
                    },
                    ..renderer::Quad::default()
                },
                Background::Color(Color::TRANSPARENT),
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(&mut tree.children[0], layout, renderer, viewport, translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        begin_frame(None);
    }

    fn push(actions: Actions) {
        FRAME.with(|frame| frame.borrow_mut().actions.push(actions));
    }

    #[test]
    fn nudge_enters_and_wraps() {
        reset();
        for _ in 0..3 {
            push(Actions::activate(AppMessage::NoOp));
        }

        let mut focus = None;
        nudge(&mut focus, 1);
        assert_eq!(focus, Some(0));
        nudge(&mut focus, -1);
        assert_eq!(focus, Some(2));
        nudge(&mut focus, 1);
        assert_eq!(focus, Some(0));
    }

    #[test]
    fn nudge_backward_from_nothing_enters_at_the_end() {
        reset();
        push(Actions::activate(AppMessage::NoOp));
        push(Actions::activate(AppMessage::NoOp));

        let mut focus = None;
        nudge(&mut focus, -1);
        assert_eq!(focus, Some(1));
    }

    #[test]
    fn empty_frame_clears_the_ring() {
        reset();
        let mut focus = Some(3);
        nudge(&mut focus, 1);
        assert_eq!(focus, None);
    }

    #[test]
    fn activation_and_adjustment_read_the_focused_control() {
        reset();
        push(Actions::activate(AppMessage::Quit));
        push(Actions::adjust(
            AppMessage::TabSelected(crate::app::Tab::Replays),
            AppMessage::Quit,
        ));

        assert!(matches!(activation(Some(0)), Some(Command::App(AppMessage::Quit))));
        assert!(activation(Some(1)).is_none());
        assert!(adjustment(Some(1), false).is_some());
        assert!(adjustment(Some(1), true).is_some());
        assert!(adjustment(Some(0), true).is_none());
        assert!(activation(Some(9)).is_none());
    }

    #[test]
    fn tab_commands_map_to_their_shell_message() {
        let cmd = Command::from(tabs::play::Message::Noop);
        assert!(matches!(cmd.into_message(), AppMessage::Play(_)));
    }
}

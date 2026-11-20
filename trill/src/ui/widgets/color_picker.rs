//! Inline HSV color picker for the custom accent setting. The
//! saturation/value square and the hue strip share one canvas, so a
//! drag that starts in either region keeps its intent even when the
//! pointer strays outside the widget.

use crate::ui::theme::hsv_to_rgb;
use iced::widget::canvas::gradient::Linear;
use iced::widget::canvas::{self, Canvas, Frame, Path, Stroke, Style};
use iced::{mouse, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

const HEIGHT: f32 = 196.0;
const HUE_HEIGHT: f32 = 16.0;
const GAP: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drag {
    SaturationValue,
    Hue,
}

#[derive(Default)]
struct PickerState {
    drag: Option<Drag>,
}

struct Picker<'a, M> {
    hsv: (f32, f32, f32),
    on_change: Box<dyn Fn((f32, f32, f32)) -> M + 'a>,
}

/// An HSV picker. `on_change` receives the new `(hue, saturation,
/// value)` when the user clicks or drags either region.
pub fn color_picker<'a, M: Clone + 'a>(
    hsv: (f32, f32, f32),
    on_change: impl Fn((f32, f32, f32)) -> M + 'a,
) -> Element<'a, M> {
    Canvas::new(Picker {
        hsv,
        on_change: Box::new(on_change),
    })
    .width(Length::Fixed(340.0))
    .height(Length::Fixed(HEIGHT))
    .into()
}

/// The square on top, the hue strip at the bottom.
fn regions(size: Size) -> (Rectangle, Rectangle) {
    let square_height = (size.height - HUE_HEIGHT - GAP).max(1.0);
    let square = Rectangle::new(Point::ORIGIN, Size::new(size.width, square_height));
    let hue = Rectangle::new(
        Point::new(0.0, size.height - HUE_HEIGHT),
        Size::new(size.width, HUE_HEIGHT),
    );
    (square, hue)
}

fn hue_color(hue: f32) -> Color {
    let [r, g, b] = hsv_to_rgb(hue, 1.0, 1.0);
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

fn local_position(cursor: mouse::Cursor, bounds: Rectangle) -> Option<Point> {
    cursor.position().map(|p| Point::new(p.x - bounds.x, p.y - bounds.y))
}

impl<M: Clone> Picker<'_, M> {
    fn pick(&self, size: Size, drag: Drag, position: Point) -> (f32, f32, f32) {
        let (square, hue) = regions(size);
        match drag {
            Drag::SaturationValue => (
                self.hsv.0,
                ((position.x - square.x) / square.width).clamp(0.0, 1.0),
                (1.0 - (position.y - square.y) / square.height).clamp(0.0, 1.0),
            ),
            Drag::Hue => (
                ((position.x - hue.x) / hue.width).clamp(0.0, 1.0),
                self.hsv.1,
                self.hsv.2,
            ),
        }
    }
}

impl<M: Clone> canvas::Program<M> for Picker<'_, M> {
    type State = PickerState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<M>> {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let position = local_position(cursor, bounds)?;
                let (square, hue) = regions(bounds.size());
                let drag = if square.contains(position) {
                    Drag::SaturationValue
                } else if hue.contains(position) {
                    Drag::Hue
                } else {
                    return None;
                };
                state.drag = Some(drag);
                Some(canvas::Action::publish((self.on_change)(self.pick(bounds.size(), drag, position))).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let drag = state.drag?;
                let position = local_position(cursor, bounds)?;
                Some(canvas::Action::publish((self.on_change)(self.pick(bounds.size(), drag, position))).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.drag.take().map(|_| canvas::Action::capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let (square, hue) = regions(bounds.size());
        let (h, s, v) = self.hsv;

        // Saturation/value square: the pure hue, then white from the
        // left edge (saturation) and black from the bottom (value).
        frame.fill_rectangle(square.position(), square.size(), hue_color(h));
        frame.fill_rectangle(
            square.position(),
            square.size(),
            Linear::new(square.position(), Point::new(square.x + square.width, square.y))
                .add_stop(0.0, Color::WHITE)
                .add_stop(1.0, Color { a: 0.0, ..Color::WHITE }),
        );
        frame.fill_rectangle(
            square.position(),
            square.size(),
            Linear::new(square.position(), Point::new(square.x, square.y + square.height))
                .add_stop(0.0, Color { a: 0.0, ..Color::BLACK })
                .add_stop(1.0, Color::BLACK),
        );

        // Hue strip: red → yellow → green → cyan → blue → magenta → red.
        let hue_gradient = (0..=6).fold(
            Linear::new(hue.position(), Point::new(hue.x + hue.width, hue.y)),
            |gradient, step| gradient.add_stop(step as f32 / 6.0, hue_color(step as f32 / 6.0)),
        );
        frame.fill_rectangle(hue.position(), hue.size(), hue_gradient);

        let border = Color {
            a: 0.35,
            ..theme.palette().text
        };
        for region in [square, hue] {
            frame.stroke_rectangle(
                region.position(),
                region.size(),
                Stroke {
                    style: Style::Solid(border),
                    width: 1.0,
                    ..Stroke::default()
                },
            );
        }

        // Ring-shaped cursors: black under white so they stay legible
        // on both light and dark ends of the square / strip.
        let cursor = Point::new(square.x + s * square.width, square.y + (1.0 - v) * square.height);
        let ring = Path::circle(cursor, 6.0);
        frame.stroke(
            &ring,
            Stroke {
                style: Style::Solid(Color { a: 0.8, ..Color::BLACK }),
                width: 4.0,
                ..Stroke::default()
            },
        );
        frame.stroke(
            &ring,
            Stroke {
                style: Style::Solid(Color::WHITE),
                width: 2.0,
                ..Stroke::default()
            },
        );

        let x = hue.x + h * hue.width;
        let marker = Path::new(|path| {
            path.move_to(Point::new(x, hue.y + 1.0));
            path.line_to(Point::new(x, hue.y + hue.height - 1.0));
        });
        frame.stroke(
            &marker,
            Stroke {
                style: Style::Solid(Color { a: 0.8, ..Color::BLACK }),
                width: 4.0,
                ..Stroke::default()
            },
        );
        frame.stroke(
            &marker,
            Stroke {
                style: Style::Solid(Color::WHITE),
                width: 2.0,
                ..Stroke::default()
            },
        );

        vec![frame.into_geometry()]
    }

    fn mouse_interaction(&self, state: &Self::State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if state.drag.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

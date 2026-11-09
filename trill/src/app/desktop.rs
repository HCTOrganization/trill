//! Window lifecycle, clipboard, and file manager.

use super::{App, Message};

/// Allow PvP's bounded Goodbye send to finish before the runtime exits.
const PVP_EXIT_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(1500);

pub(super) fn copy_html_to_clipboard(text: String, html: String) {
    tokio::task::spawn_blocking(move || match arboard::Clipboard::new() {
        Ok(mut cb) => {
            if let Err(e) = cb.set_html(html.as_str(), Some(text.as_str())) {
                log::warn!("clipboard set_html failed: {e}");
                // Keep the established plain-text copy useful even on a
                // clipboard backend that cannot publish HTML.
                if let Err(e) = cb.set_text(text) {
                    log::warn!("clipboard set_text fallback failed: {e}");
                }
            }
        }
        Err(e) => log::warn!("clipboard open failed: {e}"),
    });
}

pub(super) fn copy_image_to_clipboard(img: image::RgbaImage) {
    let (width, height) = (img.width() as usize, img.height() as usize);
    let bytes = img.into_raw();
    tokio::task::spawn_blocking(move || match arboard::Clipboard::new() {
        Ok(mut cb) => {
            let data = arboard::ImageData {
                width,
                height,
                bytes: bytes.into(),
            };
            if let Err(e) = cb.set_image(data) {
                log::warn!("clipboard set_image failed: {e}");
            }
        }
        Err(e) => log::warn!("clipboard open failed: {e}"),
    });
}

/// Open a path in the OS file manager / default handler, logging on failure.
/// Shared by the per-tab `OpenPath` effects.
pub(super) fn open_path(path: impl AsRef<std::path::Path>) -> iced::Task<Message> {
    let path = path.as_ref();
    if let Err(e) = open::that(path) {
        log::error!("open {}: {e}", path.display());
    }
    iced::Task::none()
}

/// Open an external link with the OS's default handler.
pub(super) fn open_url(url: &str) -> iced::Task<Message> {
    if let Err(e) = open::that(url) {
        log::warn!("open url {url}: {e}");
    }
    iced::Task::none()
}

/// Reveal a file in the OS file manager with the file itself selected,
/// rather than opening its containing folder anonymously. Shared by the
/// per-tab `RevealPath` effects (replays, saves).
pub(super) fn reveal_path(path: impl AsRef<std::path::Path>) -> iced::Task<Message> {
    // opener::reveal blocks until the platform helper finishes; run it off
    // the update loop so a wedged file manager can't stall the UI.
    let path = path.as_ref().to_path_buf();
    std::thread::spawn(move || {
        if let Err(e) = opener::reveal(&path) {
            log::error!("reveal {}: {e}", path.display());
        }
    });
    iced::Task::none()
}

impl App {
    pub(super) fn update_window(&mut self, id: iced::window::Id, ev: iced::window::Event) -> iced::Task<Message> {
        match ev {
            iced::window::Event::Opened { .. } => {
                // The window came up, so the geometry we
                // launched with is presentable — disarm the
                // safe mode `main::restore_window_size` armed
                // on disk before the window was built.
                if self.config.window_geometry_unverified {
                    self.config.window_geometry_unverified = false;
                    self.persist_config();
                }
                // What this window's DPI scale is decides how
                // large it may get before its surface outgrows
                // the GPU — see `WindowScaleQueried`. Nothing
                // else about the opened size is second-guessed:
                // a window bigger than the monitor it landed on
                // is a legitimate thing to want (spanning two
                // displays, say), and the only size we're in a
                // position to refuse is one that can't be drawn
                // at all.
                return iced::window::scale_factor(id).map(move |scale| Message::WindowScaleQueried { id, scale });
            }
            iced::window::Event::Resized(size) => {
                // The Resized size could be either a user-driven
                // resize or the result of maximize/unmaximize.
                // We need is_maximized to decide whether to keep
                // it as the restore size, so query it and finish
                // the bookkeeping in WindowMaximizedQueried.
                return iced::window::is_maximized(id)
                    .map(move |maximized| Message::WindowMaximizedQueried { size, maximized });
            }
            iced::window::Event::Moved(point) => {
                // Only remember position while fullscreen.
                // Entering fullscreen parks the window at its
                // monitor's origin and fires Moved (with
                // fullscreen already set, see C::Fullscreen) —
                // so the persisted value identifies the
                // fullscreen monitor for the next launch.
                // Windowed positions are deliberately not
                // persisted: restoring an exact x/y is janky on
                // multi-monitor setups (saved coords can land
                // off-screen or on the wrong display).
                if self.config.fullscreen {
                    self.config.last_window_position = Some((point.x, point.y));
                    self.persist_config();
                }
            }
            iced::window::Event::CloseRequested => {
                return self.quit();
            }
            _ => {}
        }
        iced::Task::none()
    }

    pub(super) fn quit(&mut self) -> iced::Task<Message> {
        if self.exit_pending {
            return iced::Task::none();
        }
        // Complete any queued config write before the runtime
        // tears down (Drop also flushes, as the backstop for the
        // window-close exit path).
        self.config_writer.flush();
        if let Some(done) = self.session.request_app_close() {
            self.exit_pending = true;
            iced::Task::perform(
                async move {
                    let _ = tokio::time::timeout(PVP_EXIT_TIMEOUT, done.cancelled()).await;
                },
                |_| Message::Exit,
            )
        } else {
            iced::exit()
        }
    }
}

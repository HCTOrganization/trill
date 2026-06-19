//! Emulator-border / backdrop art. Builds the layer that sits behind
//! the game framebuffer in [`super::view::emulator_body`] per the
//! user's `border_preference`:
//!
//! - `0` (BNLC): the per-game background TGA pulled from the installed
//!   BNLC shared archive.
//! - `1` (Custom): a user-chosen still image, or — for an `.mp4` — a
//!   looping video decoded through a bundled ffmpeg subprocess and
//!   presented via the [`crate::video::border`] shader.
//! - anything else (Disable / missing art): a plain black backdrop.
//!
//! Decoded stills are cached per source; the video decoder runs on its
//! own thread, publishing the latest frame for the redraw loop to
//! sample, and self-terminates a couple seconds after the border stops
//! being rendered (session closed).

use super::*;

/// Builds the backdrop behind the emulator framebuffer for the current
/// border preference. A custom MP4 is rendered through the
/// [`crate::video::border`] shader primitive (NOT the `image` widget,
/// which flickers when fed a fresh handle every frame); BNLC art and
/// still custom images go through `image`; everything else is plain
/// black.
///
/// PvP is the exception: it runs single-core (the one live mgba thread
/// drives the netcode, the rollback fast-forward, AND rendering on a
/// tight real-time budget). A live MP4 decode — a continuous ffmpeg
/// pass plus per-frame multi-MB frame clones and GPU uploads — steals
/// enough of that budget to blow the rollback/prediction window and
/// desync the match. So during PvP an MP4 border is shown as a static
/// first frame (decoded once, off-thread) instead of an animated one.
pub(super) fn border_backdrop<'a>(
    session: &'a ActiveSession,
    border_preference: u8,
    border_image_path: Option<&std::path::Path>,
) -> Element<'a, Message> {
    if border_preference == 1 {
        if let Some(path) = border_image_path {
            if is_video_border(path) {
                // PvP: static first frame only — never the live decoder.
                if matches!(session, ActiveSession::PvP(_)) {
                    return match video_still_handle(path) {
                        Some(h) => iced::widget::image(h)
                            .width(Fill)
                            .height(Fill)
                            .content_fit(iced::ContentFit::Cover)
                            .into(),
                        // Not decoded yet (cold start) / decode failure.
                        None => black_backdrop(),
                    };
                }
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

/// True when the path's extension marks it as an MP4 video handled by
/// the ffmpeg border decoder. Case-insensitive.
fn is_video_border(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
}

/// A static iced `Handle` for the first frame of a border video, for the
/// PvP path where running the live decoder would starve the single
/// real-time core (see [`border_backdrop`]). The decode runs once on a
/// throwaway thread so it never blocks the UI thread; the result (a
/// successful `Handle` or `None` on failure) is cached per path. Returns
/// `None` until the first decode lands — the caller then shows a plain
/// black backdrop for those first frames.
fn video_still_handle(path: &std::path::Path) -> Option<iced::widget::image::Handle> {
    use std::collections::{HashMap, HashSet};
    use std::sync::LazyLock;
    // `Some(None)` is a cached decode *failure* — distinct from "not yet
    // decoded" (absent), so we don't respawn a doomed decode every frame.
    static CACHE: LazyLock<std::sync::Mutex<HashMap<std::path::PathBuf, Option<iced::widget::image::Handle>>>> =
        LazyLock::new(Default::default);
    static IN_FLIGHT: LazyLock<std::sync::Mutex<HashSet<std::path::PathBuf>>> = LazyLock::new(Default::default);

    if let Some(cached) = CACHE.lock().unwrap().get(path).cloned() {
        return cached;
    }
    // First sighting of this path — kick off a one-shot decode. The
    // in-flight guard keeps later frames from spawning duplicates while
    // it runs.
    if IN_FLIGHT.lock().unwrap().insert(path.to_path_buf()) {
        let path_owned = path.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name("border-still".into())
            .spawn(move || {
                let handle = decode_video_first_frame(&path_owned);
                CACHE.lock().unwrap().insert(path_owned.clone(), handle);
                IN_FLIGHT.lock().unwrap().remove(&path_owned);
            });
        if spawned.is_err() {
            IN_FLIGHT.lock().unwrap().remove(path);
        }
    }
    None
}

/// Decodes a video's first frame to an iced `Handle` via a one-shot
/// ffmpeg pass (first frame → BMP → RGBA). `None` on any spawn / decode
/// failure. Used by [`video_still_handle`].
fn decode_video_first_frame(path: &std::path::Path) -> Option<iced::widget::image::Handle> {
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
    let out = cmd
        .output()
        .inspect_err(|e| log::warn!("custom border video {}: still ffmpeg spawn: {e}", path.display()))
        .ok()?;
    if !out.status.success() {
        log::warn!("custom border video {}: still ffmpeg exited {:?}", path.display(), out.status);
        return None;
    }
    let img = image::load_from_memory_with_format(&out.stdout, image::ImageFormat::Bmp)
        .inspect_err(|e| log::warn!("custom border video {}: still decode: {e}", path.display()))
        .ok()?;
    let rgba = img.into_rgba8();
    let (w, h) = rgba.dimensions();
    Some(iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw()))
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

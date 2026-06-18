//! One-shot WAV playback. Decodes a PCM `.wav` blob (typically an
//! `include_bytes!`'d voice clip) into stereo `i16` frames and plays
//! it through its OWN dedicated SDL stream — deliberately separate
//! from the [`crate::audio::LateBinder`]/session path so a startup
//! jingle can't fight a game session for the single binder slot.
//!
//! Lifetime: the returned [`Player`] owns the SDL stream. Keep it
//! alive (e.g. in `App`) for the duration of playback; once the clip
//! finishes the stream just emits silence until the `Player` drops.

use crate::audio::{self, Stream, NUM_CHANNELS};
use crate::config::ThemeColor;

/// The voice clip to play at launch for the given accent color.
///
/// Every entry currently points at the same placeholder
/// (`voice/ja/00.wav`); swap individual arms to per-color clips as
/// they're recorded. Bytes are embedded at build time so they ship
/// in the binary.
pub fn voice_for_theme(color: ThemeColor) -> &'static [u8] {
    // Placeholder shared by every color until per-theme clips exist.
    const PLACEHOLDER: &[u8] = include_bytes!("../voice/ja/trill_yellow.wav");
    match color {
        ThemeColor::TrillYellow => PLACEHOLDER,
        ThemeColor::PegasusBlue => PLACEHOLDER,
        ThemeColor::SoniaPink => PLACEHOLDER,
        ThemeColor::ZerkerGrey => PLACEHOLDER,
        ThemeColor::NinjaGreen => PLACEHOLDER,
        ThemeColor::SaurianOrange => PLACEHOLDER,
        ThemeColor::RoguePurple => PLACEHOLDER,
        ThemeColor::AceBlack => PLACEHOLDER,
        ThemeColor::JokerRed => PLACEHOLDER,
        ThemeColor::SpeakiBrown => PLACEHOLDER,
    }
}

/// The voice clip played when a netplay match starts. Fixed (not
/// accent-dependent), but localized: Japanese, Korean, and Chinese
/// (both scripts) get the Japanese clip; every other language gets
/// the English one. Embedded at build time.
pub fn match_start_voice(lang: &unic_langid::LanguageIdentifier) -> &'static [u8] {
    const JA: &[u8] = include_bytes!("../voice/ja/00.wav");
    const EN: &[u8] = include_bytes!("../voice/en/00.wav");
    // Region/script don't matter here — zh-CN and zh-TW both take JA —
    // so we only look at the primary language subtag.
    match lang.language.as_str() {
        "ja" | "ko" | "zh" => JA,
        _ => EN,
    }
}

/// A decoded PCM wav, ready to be streamed once.
struct WavStream {
    /// Interleaved stereo frames at the wav's native sample rate.
    samples: Vec<[i16; NUM_CHANNELS]>,
    pos: usize,
}

impl Stream for WavStream {
    fn fill(&mut self, buf: &mut [[i16; NUM_CHANNELS]]) -> usize {
        let remaining = self.samples.len().saturating_sub(self.pos);
        let n = remaining.min(buf.len());
        if n > 0 {
            buf[..n].copy_from_slice(&self.samples[self.pos..self.pos + n]);
            self.pos += n;
        }
        // After exhaustion `n` is 0 — the SDL callback pads with
        // silence, so the clip plays exactly once.
        n
    }
}

/// Minimal RIFF/WAVE PCM decoder. Handles 16-bit mono/stereo PCM
/// (`audioFormat == 1`), which is all the bundled voice clips use.
/// Returns the native sample rate plus the decoded stereo frames.
fn decode_wav(bytes: &[u8]) -> anyhow::Result<(u32, Vec<[i16; NUM_CHANNELS]>)> {
    let read_u16 = |o: usize| -> u16 { u16::from_le_bytes([bytes[o], bytes[o + 1]]) };
    let read_u32 =
        |o: usize| -> u32 { u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]) };

    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        anyhow::bail!("not a RIFF/WAVE file");
    }

    let mut channels: u16 = 0;
    let mut sample_rate: u32 = 0;
    let mut bits: u16 = 0;
    let mut audio_format: u16 = 0;
    let mut data: Option<&[u8]> = None;

    // Walk the chunk list looking for `fmt ` and `data`.
    let mut pos = 12usize;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = read_u32(pos + 4) as usize;
        let body = pos + 8;
        if body + size > bytes.len() {
            break;
        }
        match id {
            b"fmt " => {
                audio_format = read_u16(body);
                channels = read_u16(body + 2);
                sample_rate = read_u32(body + 4);
                bits = read_u16(body + 14);
            }
            b"data" => {
                data = Some(&bytes[body..body + size]);
            }
            _ => {}
        }
        // Chunks are word-aligned: odd sizes carry a pad byte.
        pos = body + size + (size & 1);
    }

    if audio_format != 1 {
        anyhow::bail!("unsupported wav format {audio_format} (only PCM)");
    }
    if bits != 16 {
        anyhow::bail!("unsupported bit depth {bits} (only 16-bit)");
    }
    if channels != 1 && channels != 2 {
        anyhow::bail!("unsupported channel count {channels}");
    }
    let data = data.ok_or_else(|| anyhow::anyhow!("wav has no data chunk"))?;

    let frame_count = data.len() / (2 * channels as usize);
    let mut samples = Vec::with_capacity(frame_count);
    for f in 0..frame_count {
        let base = f * 2 * channels as usize;
        let l = i16::from_le_bytes([data[base], data[base + 1]]);
        let r = if channels == 2 {
            i16::from_le_bytes([data[base + 2], data[base + 3]])
        } else {
            l // upmix mono to both channels
        };
        samples.push([l, r]);
    }

    Ok((sample_rate, samples))
}

/// Owns the dedicated SDL stream playing the clip. Drop to stop.
pub struct Player {
    _backend: audio::sdl::Backend,
}

/// Decode `wav_bytes` and start playing it once on its own SDL
/// stream. The clip plays at its native sample rate (SDL resamples
/// to the device); keep the returned [`Player`] alive for playback.
pub fn play(wav_bytes: &[u8]) -> anyhow::Result<Player> {
    let (sample_rate, samples) = decode_wav(wav_bytes)?;
    let stream = WavStream { samples, pos: 0 };
    let backend = audio::sdl::Backend::new_at(stream, sample_rate as i32)?;
    log::info!("audio: playing one-shot voice clip at {sample_rate} Hz");
    Ok(Player { _backend: backend })
}

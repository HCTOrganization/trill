//! What an export should produce: codecs, quality, geometry, timebase.
//!
//! Everything here is the caller's choice, decided before anything is
//! encoded. What comes *out* of the encoders is [`crate::packet`].
//!
//! The codec list is deliberately short. Every codec costs a sample
//! entry in one muxer, a track header in the other and a command line in
//! the backend, so the ones here are the ones an export actually asks
//! for: H.264 with AAC in MP4, and raw RGB with PCM in Matroska for the
//! archival stop. The raw pair bypasses a platform encoder entirely, so
//! native and browser exports produce the same bytes without asking an
//! integer-rate codec to describe a fractional hardware clock.

use crate::error::check;
use crate::mux::Container;

/// Video codecs a backend can produce and a muxer can carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    H264 {
        quality: H264Quality,
    },
    /// Tightly packed RGB24 frames. Matroska carries these as
    /// `V_UNCOMPRESSED` with an RGB24 FourCC.
    RawRgb24,
}

/// How hard the H.264 encoder should work. Quality-targeted rather than
/// bitrate-targeted where it can be: a screen capture at a fixed size
/// gives predictable output from a quality floor, where a bitrate target
/// would swing with the content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum H264Quality {
    /// Mathematically lossless (`libx264rgb -qp 0`), which keeps the
    /// frames in RGB — so no color conversion happens and nothing is
    /// lost, at the cost of a container that carries RGB H.264.
    Lossless,
    /// x264's constant-rate-factor scale: lower is better, 18 is a good
    /// default, 51 is the floor.
    Crf(u8),
    /// Target bitrate in bits per second. What WebCodecs takes — the
    /// browser's encoders have no quality-targeted mode.
    Bitrate(u32),
}

/// Audio codecs a backend can produce and a muxer can carry: one lossy,
/// one lossless.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioCodec {
    /// Per-track bitrate in bits per second.
    Aac {
        bitrate: u32,
    },
    Flac,
    /// Interleaved signed 16-bit little-endian PCM.
    PcmS16Le,
}

/// A sample clock as an exact rational number of frames per second.
///
/// Audio codecs and operating-system APIs usually take a whole-Hz rate,
/// but hardware does not have to produce one: the DS SPU runs at
/// 33,513,982 / 1,024 Hz. Keeping the fraction also gives raw PCM packets
/// an integer timebase: `numerator` ticks per second and `denominator`
/// ticks per sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl SampleRate {
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self { numerator, denominator }
    }

    pub const fn integer(hz: u32) -> Self {
        Self::new(hz, 1)
    }

    pub fn as_f64(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }

    pub const fn integer_hz(self) -> Option<u32> {
        if self.denominator == 1 {
            Some(self.numerator)
        } else {
            None
        }
    }
}

impl From<u32> for SampleRate {
    fn from(value: u32) -> Self {
        Self::integer(value)
    }
}

/// Samples per AAC frame with `frameLengthFlag` clear, which is what
/// every encoder here produces. Also what those encoders prime with —
/// see [`crate::AudioTrackInfo::codec_delay_samples`].
pub(crate) const AAC_SAMPLES_PER_FRAME: u64 = 1024;

/// Color signalling, in the ISO/IEC 23091-2 code points that both MP4's
/// `colr` box and Matroska's colour element use verbatim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorInfo {
    pub primaries: u8,
    pub transfer: u8,
    pub matrix: u8,
    /// True for full-range (0–255) luma, false for the 16–235 video
    /// range. An emulator framebuffer is full-range sRGB, and tagging it
    /// as video-range is what makes an export look more saturated than
    /// the emulator did.
    pub full_range: bool,
}

impl ColorInfo {
    /// Full-range sRGB: BT.709 primaries and matrix with the IEC
    /// 61966-2-1 (sRGB) transfer function.
    pub const SRGB_FULL: ColorInfo = ColorInfo {
        primaries: 1,
        transfer: 13,
        matrix: 1,
        full_range: true,
    };

    /// Full-range sRGB stored as RGB rather than converted to YCbCr.
    pub const SRGB_RGB_FULL: ColorInfo = ColorInfo {
        primaries: 1,
        transfer: 13,
        matrix: 0,
        full_range: true,
    };
}

/// The video half of an export.
#[derive(Clone, Debug)]
pub struct VideoSettings {
    /// Which codec, and how hard to work at it — the quality knobs live
    /// on the codec, because they differ per codec.
    pub codec: VideoCodec,
    /// Size of the frames the caller will push.
    pub width: u32,
    pub height: u32,
    /// Integer nearest-neighbor upscale applied before encoding, so
    /// players that smooth-scale can't blur pixel art. 1 encodes frames
    /// at their input size.
    ///
    /// Which side scales is the backend's business: ffmpeg scales in its
    /// own filtergraph (keeping the frame pipe small — at 10× the
    /// pre-scaled bytes would be hundreds of megabytes a second),
    /// WebCodecs has no scaler so that backend expands in Rust. Either
    /// way callers push frames at the input size.
    pub scale: u32,
    /// Frames between forced keyframes. Bounds seek granularity, and on
    /// Matroska it bounds cluster length too.
    pub keyframe_interval: u32,
    /// Ticks per second for this track's timestamps.
    pub timescale: u32,
    /// One frame's length in `timescale` ticks. Constant — an emulator
    /// export has no variable frame timing — and stated here rather than
    /// as a frame rate so the backend can ask its encoder for exactly
    /// this timebase and get whole ticks back.
    pub frame_duration: u64,
    /// Color tags for the container. An H.264 encoder additionally writes
    /// them into the bitstream's VUI, which is what decoders actually
    /// read; this is for players that trust the container.
    pub color: Option<ColorInfo>,
}

impl VideoSettings {
    /// Encoded frame width: the input size times the scale.
    pub fn output_width(&self) -> u32 {
        self.width * self.scale
    }

    pub fn output_height(&self) -> u32 {
        self.height * self.scale
    }

    /// Bytes in one RGBA frame at the input size.
    pub fn frame_bytes(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }
}

/// The audio half of an export. One [`AudioSettings`] covers every audio
/// track — a two-sided export encodes both sides the same way.
#[derive(Clone, Debug)]
pub struct AudioSettings {
    /// Which codec, with its bitrate if it has one.
    pub codec: AudioCodec,
    pub sample_rate: SampleRate,
    pub channels: u8,
}

/// Everything an export needs that isn't the frames themselves.
#[derive(Clone, Debug)]
pub struct Settings {
    pub video: VideoSettings,
    pub audio: AudioSettings,
    pub container: Container,
    /// How many audio tracks to open. A two-sided layout carries one per
    /// side.
    pub audio_tracks: usize,
    /// Put the index in front of the media, so a player reading the file
    /// in order can start before it has all of it — MP4's `faststart`.
    ///
    /// The index can only be built once the stream ends, so reaching that
    /// layout costs one pass over the finished file to make room for it.
    /// Matroska ignores this: it always writes a seek index at the head.
    pub faststart: bool,
}

impl Settings {
    /// Check the codec, container and geometry choices, so a bad
    /// combination fails before an encoder is spawned rather than at the
    /// first packet.
    pub fn validate(&self) -> crate::Result<()> {
        check!(self.audio_tracks > 0, "an export needs at least one audio track");
        check!(self.video.scale >= 1, "scale must be at least 1");
        check!(
            self.video.width > 0 && self.video.height > 0,
            "an export needs a nonzero frame size"
        );
        check!(self.video.frame_duration > 0, "frame duration must be nonzero");
        check!(self.video.timescale > 0, "timescale must be nonzero");
        check!(
            self.audio.sample_rate.numerator > 0 && self.audio.sample_rate.denominator > 0,
            "sample rate must be nonzero"
        );
        check!(self.audio.channels > 0, "an export needs at least one audio channel");
        let raw_video = matches!(self.video.codec, VideoCodec::RawRgb24);
        let raw_audio = matches!(self.audio.codec, AudioCodec::PcmS16Le);
        check!(raw_video == raw_audio, "raw RGB and PCM must be selected together");
        check!(
            !raw_video || self.video.scale == 1,
            "raw RGB cannot be scaled by an encoder"
        );
        check!(
            raw_audio || self.audio.sample_rate.integer_hz().is_some(),
            "encoded audio requires an integer sample rate"
        );
        self.container.accepts(self.video.codec, self.audio.codec)?;
        Ok(())
    }
}

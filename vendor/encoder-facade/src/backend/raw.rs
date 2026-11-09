//! Encoder-free packets for Matroska's uncompressed archival profile.
//!
//! The caller already supplies RGBA8 frames and signed 16-bit PCM, so
//! encoding would only obscure the hardware clock this profile exists to
//! preserve. RGB drops the framebuffer's unused alpha byte; PCM is copied
//! little-endian. Packet timestamps stay integer by counting one sample as
//! `SampleRate::denominator` ticks in a `SampleRate::numerator` timebase.

use crate::{AudioCodec, Backend, Canceller, Packet, Settings, VideoCodec, VIDEO_TRACK};

pub struct RawBackend {
    video_frame_duration: u64,
    keyframe_interval: u32,
    audio_channels: usize,
    sample_tick_duration: u32,
    video_frames: u64,
    audio_frames: Vec<u64>,
    ready: Vec<(usize, Packet)>,
    flushing: bool,
}

impl RawBackend {
    pub fn open(settings: &Settings, canceller: &Canceller) -> crate::Result<Self> {
        settings.validate()?;
        canceller.check()?;
        debug_assert!(matches!(settings.video.codec, VideoCodec::RawRgb24));
        debug_assert!(matches!(settings.audio.codec, AudioCodec::PcmS16Le));
        Ok(Self {
            video_frame_duration: settings.video.frame_duration,
            keyframe_interval: settings.video.keyframe_interval.max(1),
            audio_channels: settings.audio.channels as usize,
            sample_tick_duration: settings.audio.sample_rate.denominator,
            video_frames: 0,
            audio_frames: vec![0; settings.audio_tracks],
            ready: Vec::new(),
            flushing: false,
        })
    }
}

impl Backend for RawBackend {
    fn submit_video(&mut self, frame: &[u8]) -> crate::Result<()> {
        let mut rgb = Vec::with_capacity(frame.len() / 4 * 3);
        for pixel in frame.chunks_exact(4) {
            rgb.extend_from_slice(&pixel[..3]);
        }
        let index = self.video_frames;
        self.ready.push((
            VIDEO_TRACK,
            Packet {
                pts: index.saturating_mul(self.video_frame_duration),
                duration: self.video_frame_duration,
                keyframe: index.is_multiple_of(self.keyframe_interval as u64),
                data: rgb,
            },
        ));
        self.video_frames += 1;
        Ok(())
    }

    fn submit_audio(&mut self, track: usize, samples: &[i16]) -> crate::Result<()> {
        let frames = samples.len() / self.audio_channels;
        let next = self
            .audio_frames
            .get_mut(track)
            .ok_or_else(|| crate::Error::internal(format!("no raw audio track {track}")))?;
        let pts = next.saturating_mul(self.sample_tick_duration as u64);
        let duration = (frames as u64).saturating_mul(self.sample_tick_duration as u64);
        *next += frames as u64;
        let mut data = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            data.extend_from_slice(&sample.to_le_bytes());
        }
        self.ready.push((
            track + 1,
            Packet {
                pts,
                duration,
                keyframe: true,
                data,
            },
        ));
        Ok(())
    }

    fn poll(&mut self) -> crate::Result<Vec<(usize, Packet)>> {
        Ok(std::mem::take(&mut self.ready))
    }

    fn codec_private(&self, track: usize) -> Option<Vec<u8>> {
        (track <= self.audio_frames.len()).then(Vec::new)
    }

    fn codec_delay_samples(&self, _track: usize) -> u64 {
        0
    }

    fn begin_flush(&mut self) -> crate::Result<()> {
        self.flushing = true;
        Ok(())
    }

    fn poll_flush(&mut self) -> crate::Result<bool> {
        Ok(self.flushing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioSettings, ColorInfo, Container, SampleRate, VideoSettings};

    const DS_SAMPLE_RATE: SampleRate = SampleRate::new(33_513_982, 1_024);

    fn settings() -> Settings {
        Settings {
            video: VideoSettings {
                codec: VideoCodec::RawRgb24,
                width: 2,
                height: 1,
                scale: 1,
                keyframe_interval: 60,
                timescale: 16_777_216,
                frame_duration: 280_896,
                color: Some(ColorInfo::SRGB_RGB_FULL),
            },
            audio: AudioSettings {
                codec: AudioCodec::PcmS16Le,
                sample_rate: DS_SAMPLE_RATE,
                channels: 2,
            },
            container: Container::Matroska,
            audio_tracks: 1,
            faststart: false,
        }
    }

    #[test]
    fn raw_packets_preserve_rgb_pcm_and_the_fractional_clock() {
        let mut backend = RawBackend::open(&settings(), &Canceller::new()).unwrap();
        backend
            .submit_video(&[0x10, 0x20, 0x30, 0xff, 0x40, 0x50, 0x60, 0x80])
            .unwrap();
        backend.submit_audio(0, &[1, -2, 3, -4]).unwrap();
        backend.submit_audio(0, &[5, -6]).unwrap();

        let packets = backend.poll().unwrap();
        assert_eq!(packets.len(), 3);
        assert_eq!(packets[0].0, VIDEO_TRACK);
        assert_eq!(packets[0].1.data, [0x10, 0x20, 0x30, 0x40, 0x50, 0x60]);

        let first_audio = &packets[1].1;
        assert_eq!(first_audio.pts, 0);
        assert_eq!(first_audio.duration, 2 * DS_SAMPLE_RATE.denominator as u64);
        assert_eq!(first_audio.data, [1, 0, 0xfe, 0xff, 3, 0, 0xfc, 0xff]);

        let second_audio = &packets[2].1;
        assert_eq!(second_audio.pts, 2 * DS_SAMPLE_RATE.denominator as u64);
        assert_eq!(second_audio.duration, DS_SAMPLE_RATE.denominator as u64);
        assert_eq!(second_audio.data, [5, 0, 0xfa, 0xff]);
    }
}

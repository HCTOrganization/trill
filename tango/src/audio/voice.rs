//! Voice playback for match ready state. Plays a simple audio cue
//! when both players are ready and the match is about to start.
//!
//! Voice files are located at `tango/src/voice/` and are named by
//! language:
//! - ja.wav for Japanese (ja-JP)
//! - en.wav for English and all other languages
//! - Chinese (zh-CN, zh-TW) and Korean (ko-KR) use ja.wav

use crate::audio;
use std::sync::Arc;
use unic_langid::LanguageIdentifier;

/// Simple PCM WAV file reader. Handles the minimal WAV format needed
/// for simple voice clips.
struct WavFile {
    /// Raw PCM i16 samples in stereo (NUM_CHANNELS)
    samples: Vec<[i16; audio::NUM_CHANNELS]>,
    sample_rate: u32,
}

impl WavFile {
    /// Load a WAV file from bytes. Returns (sample_rate, samples).
    /// Panics if the WAV format is not supported.
    fn from_bytes(data: &[u8]) -> anyhow::Result<Self> {
        if data.len() < 44 {
            anyhow::bail!("WAV file too small");
        }

        // Check RIFF header
        if &data[0..4] != b"RIFF" {
            anyhow::bail!("not a RIFF file");
        }

        // Check WAVE format
        if &data[8..12] != b"WAVE" {
            anyhow::bail!("not a WAVE file");
        }

        // Find fmt chunk
        let mut pos = 12;
        let mut sample_rate = 44100u32;
        let mut channels = 2u16;
        let mut bits_per_sample = 16u16;

        while pos + 8 < data.len() {
            let chunk_id = &data[pos..pos + 4];
            let chunk_size = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]) as usize;
            pos += 8;

            if chunk_id == b"fmt " {
                if pos + 16 > data.len() {
                    anyhow::bail!("fmt chunk too small");
                }
                // Audio format (1 = PCM)
                let audio_format = u16::from_le_bytes([data[pos], data[pos + 1]]);
                if audio_format != 1 {
                    anyhow::bail!("unsupported audio format: {}", audio_format);
                }
                channels = u16::from_le_bytes([data[pos + 2], data[pos + 3]]);
                sample_rate = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]]);
                bits_per_sample = u16::from_le_bytes([data[pos + 14], data[pos + 15]]);
            } else if chunk_id == b"data" {
                if bits_per_sample != 16 {
                    anyhow::bail!("unsupported bits per sample: {}", bits_per_sample);
                }
                if channels != 2 {
                    anyhow::bail!("unsupported channel count: {}", channels);
                }

                let data_start = pos;
                let sample_count = chunk_size / (channels as usize * bits_per_sample as usize / 8);
                let mut samples = Vec::with_capacity(sample_count);

                for i in 0..sample_count {
                    let offset = data_start + i * 4;
                    if offset + 4 <= data.len() {
                        let left = i16::from_le_bytes([data[offset], data[offset + 1]]);
                        let right = i16::from_le_bytes([data[offset + 2], data[offset + 3]]);
                        samples.push([left, right]);
                    }
                }

                return Ok(WavFile { samples, sample_rate });
            }

            pos += chunk_size;
            if pos % 2 == 1 {
                pos += 1; // Align to even boundary
            }
        }

        anyhow::bail!("no data chunk found in WAV file")
    }
}

/// A voice clip stream that plays a WAV file once.
pub struct VoiceClip {
    samples: Arc<Vec<[i16; audio::NUM_CHANNELS]>>,
    position: usize,
}

impl VoiceClip {
    fn new(samples: Vec<[i16; audio::NUM_CHANNELS]>) -> Self {
        Self {
            samples: Arc::new(samples),
            position: 0,
        }
    }

    /// Check if playback has finished
    pub fn is_finished(&self) -> bool {
        self.position >= self.samples.len()
    }
}

impl audio::Stream for VoiceClip {
    fn fill(&mut self, buf: &mut [[i16; audio::NUM_CHANNELS]]) -> usize {
        let remaining = self.samples.len() - self.position;
        let to_copy = remaining.min(buf.len());

        if to_copy > 0 {
            buf[..to_copy].copy_from_slice(&self.samples[self.position..self.position + to_copy]);
            self.position += to_copy;
        }

        // Pad remaining with silence
        for i in to_copy..buf.len() {
            buf[i] = [0, 0];
        }

        to_copy
    }
}

/// Get the voice file path for the given language.
pub fn get_voice_file_path(lang: &LanguageIdentifier) -> &'static str {
    // Map languages to voice files
    // ja-JP, zh-CN, zh-TW, ko-KR: ja.wav
    // All other languages: en.wav
    match (lang.language.as_str(), lang.region.as_ref().map(|r| r.as_str())) {
        ("ja", _) => "ja.wav",
        ("zh", Some("CN")) | ("zh", Some("TW")) => "ja.wav",
        ("ko", Some("KR")) => "ja.wav",
        _ => "en.wav",
    }
}

/// Load a voice file from the embedded resources
pub fn load_voice_file(filename: &str) -> anyhow::Result<VoiceClip> {
    let data = match filename {
        "ja.wav" => include_bytes!("../voice/ja.wav"),
        "en.wav" => include_bytes!("../voice/en.wav"),
        _ => anyhow::bail!("unknown voice file: {}", filename),
    };

    let wav = WavFile::from_bytes(data)?;
    Ok(VoiceClip::new(wav.samples))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_file_selection() {
        use std::str::FromStr;
        assert_eq!(get_voice_file_path(&LanguageIdentifier::from_str("ja-JP").unwrap()), "ja.wav");
        assert_eq!(get_voice_file_path(&LanguageIdentifier::from_str("en-US").unwrap()), "en.wav");
        assert_eq!(get_voice_file_path(&LanguageIdentifier::from_str("zh-CN").unwrap()), "ja.wav");
        assert_eq!(get_voice_file_path(&LanguageIdentifier::from_str("zh-TW").unwrap()), "ja.wav");
        assert_eq!(get_voice_file_path(&LanguageIdentifier::from_str("ko-KR").unwrap()), "ja.wav");
        assert_eq!(get_voice_file_path(&LanguageIdentifier::from_str("fr-FR").unwrap()), "en.wav");
    }
}

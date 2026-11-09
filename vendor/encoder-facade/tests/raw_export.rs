//! Byte-level coverage for the encoder-free Matroska archival profile.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use encoder_facade::{
    AudioCodec, AudioSettings, Canceller, ColorInfo, Container, Output, SampleRate, Session, Settings, VideoCodec,
    VideoSettings,
};
use matroska_demuxer::{Frame, MatroskaFile, TrackType};

const DS_SAMPLE_RATE: SampleRate = SampleRate::new(33_513_982, 1_024);

fn settings(sample_rate: SampleRate) -> Settings {
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
            sample_rate,
            channels: 2,
        },
        container: Container::Matroska,
        audio_tracks: 1,
        faststart: false,
    }
}

fn export(sample_rate: SampleRate) -> Vec<u8> {
    let mut session = Session::new(settings(sample_rate), &Canceller::new()).unwrap();
    let mut output = Output::new(Cursor::new(Vec::new()));

    session
        .write_video(&[0x10, 0x20, 0x30, 0xff, 0x40, 0x50, 0x60, 0x80])
        .unwrap();
    session.write_audio(0, &[1, -2, 3, -4]).unwrap();
    output.append(&session.take_output()).unwrap();

    session
        .write_video(&[0x70, 0x80, 0x90, 0x00, 0xa0, 0xb0, 0xc0, 0xff])
        .unwrap();
    session.write_audio(0, &[5, -6]).unwrap();
    output.append(&session.take_output()).unwrap();

    session.begin_finish().unwrap();
    let fixups = session
        .poll_finish(&[])
        .unwrap()
        .expect("raw backend finishes immediately");
    output.append(&session.take_output()).unwrap();
    output.finish(&fixups).unwrap().into_inner()
}

fn inspect(sample_rate: SampleRate) {
    let bytes = export(sample_rate);
    let mut file = MatroskaFile::open(Cursor::new(bytes)).unwrap();
    let video = file
        .tracks()
        .iter()
        .find(|track| track.track_type() == TrackType::Video)
        .unwrap();
    let audio = file
        .tracks()
        .iter()
        .find(|track| track.track_type() == TrackType::Audio)
        .unwrap();

    assert_eq!(video.codec_id().trim_end_matches('\0'), "V_UNCOMPRESSED");
    assert_eq!(video.video().unwrap().pixel_width().get(), 2);
    assert_eq!(video.video().unwrap().pixel_height().get(), 1);
    assert_eq!(audio.codec_id().trim_end_matches('\0'), "A_PCM/INT/LIT");
    assert_eq!(audio.audio().unwrap().sampling_frequency(), sample_rate.as_f64());
    assert_eq!(audio.audio().unwrap().channels().get(), 2);
    assert_eq!(audio.audio().unwrap().bit_depth().unwrap().get(), 16);

    let video_track = video.track_number().get();
    let audio_track = audio.track_number().get();
    let mut video_packets = Vec::new();
    let mut audio_packets = Vec::new();
    let mut frame = Frame::default();
    while file.next_frame(&mut frame).unwrap() {
        if frame.track == video_track {
            video_packets.push(frame.data.clone());
        } else if frame.track == audio_track {
            audio_packets.push(frame.data.clone());
        }
    }

    assert_eq!(
        video_packets,
        [
            vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60],
            vec![0x70, 0x80, 0x90, 0xa0, 0xb0, 0xc0],
        ]
    );
    assert_eq!(
        audio_packets,
        [vec![1, 0, 0xfe, 0xff, 3, 0, 0xfc, 0xff], vec![5, 0, 0xfa, 0xff]]
    );
}

#[test]
fn ds_raw_export_preserves_fractional_sample_rate_and_payloads() {
    inspect(DS_SAMPLE_RATE);
}

#[test]
fn gba_raw_export_preserves_integer_sample_rate_and_payloads() {
    inspect(SampleRate::integer(32_768));
}

fn ffmpeg() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("ENCODER_FACADE_TEST_FFMPEG")?);
    path.exists().then_some(path)
}

fn decode(ffmpeg: &Path, file: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(file)
        .args(args)
        .output()
        .expect("run ffmpeg");
    assert!(
        output.status.success(),
        "ffmpeg rejected {}: {}",
        file.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn ffmpeg_decodes_the_raw_payloads_without_changing_them() {
    let Some(ffmpeg) = ffmpeg() else {
        eprintln!("skipping: set ENCODER_FACADE_TEST_FFMPEG to an ffmpeg binary");
        return;
    };
    let path = std::env::temp_dir().join("encoder-facade-raw-rgb-pcm.mkv");
    std::fs::write(&path, export(DS_SAMPLE_RATE)).unwrap();

    let video = decode(
        &ffmpeg,
        &path,
        &[
            "-map",
            "0:v:0",
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ],
    );
    assert_eq!(
        video,
        [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xa0, 0xb0, 0xc0,]
    );

    let audio = decode(
        &ffmpeg,
        &path,
        &["-map", "0:a:0", "-f", "s16le", "-acodec", "pcm_s16le", "-"],
    );
    assert_eq!(audio, [1, 0, 0xfe, 0xff, 3, 0, 0xfc, 0xff, 5, 0, 0xfa, 0xff]);
}

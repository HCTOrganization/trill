# Match-Ready Voice Playback Implementation

## Overview
This implementation adds voice/audio cue playback when both players are ready in netplay (PvP) mode in Tango. The voice file is automatically selected based on the UI language and plays through the existing SDL3 audio backend.

## Files Modified/Created

### New Files
- **`tango/src/audio/voice.rs`** - Voice playback module with:
  - `WavFile` struct: Simple PCM WAV file parser (handles minimal WAV format)
  - `VoiceClip` struct: Plays WAV samples as an audio stream
  - `VoicePlayer` struct: Wrapper that auto-clears the audio binding when playback completes
  - `get_voice_file_path()`: Selects voice file based on language
  - `load_voice_file()`: Loads and returns a ready-to-play VoicePlayer

### Modified Files
- **`tango/src/audio/mod.rs`** - Added `pub mod voice;` to export the voice module

- **`tango/src/tabs/play/mod.rs`** - Added:
  - `Message::PlayMatchReadyVoice` - Message variant for voice playback
  - Handler in `update_inner()` that gracefully handles this message (no-op, voice is played at App level)

- **`tango/src/app.rs`** - Added:
  - `prev_match_ready` field: Tracks previous match_ready state to detect transitions
  - `_voice_clip_binding` field: Holds the audio binding while voice is playing
  - `try_play_match_ready_voice()` method: Loads and plays the voice file
  - Logic in `Message::Netplay` handler to detect when `match_ready` transitions from false to true and trigger voice playback

## Voice File Selection

The implementation selects voice files based on the UI language:

```
- Japanese (ja-JP): ja.wav
- Simplified Chinese (zh-CN): ja.wav
- Traditional Chinese (zh-TW): ja.wav  
- Korean (ko-KR): ja.wav
- All other languages: en.wav
```

Voice files must be located at: `tango/src/voice/{en|ja}.wav`

## How It Works

1. **State Transition Detection**: The App tracks the `netplay.lobby.match_ready` state. When it transitions from `false` to `true`, the voice playback is triggered.

2. **Voice Loading**: The `voice::load_voice_file()` function:
   - Loads the appropriate WAV file from embedded resources using `include_bytes!`
   - Parses the WAV header to extract PCM audio data (16-bit stereo @ any sample rate)
   - Returns a `VoicePlayer` that wraps the audio data

3. **Audio Binding**: The `VoicePlayer` is bound to the shared `LateBinder` audio context:
   - If audio is already bound (e.g., game audio from a session), the voice is skipped
   - Otherwise, the `VoicePlayer` becomes the active audio stream

4. **Playback**: As SDL calls the audio callback:
   - `VoicePlayer::fill()` feeds samples from the `VoiceClip` to the audio buffer
   - When the clip finishes (no more samples), the binding is automatically cleared
   - Audio reverts to silence until another stream binds

## Audio Flow

```
App.try_play_match_ready_voice()
  └─> audio::voice::load_voice_file()
       └─> WavFile::from_bytes() parses WAV
       └─> VoicePlayer wraps the PCM data
  └─> audio_binder.bind(Some(Box::new(player)))
       └─> Stores Binding in _voice_clip_binding
       └─> SDL audio callback starts calling player.fill()
           └─> VoiceClip::fill() streams samples
           └─> VoicePlayer auto-clears binding when done
```

## Notes

- **Non-blocking**: Voice loading and binding is synchronous but fast (embedded bytes, no I/O)
- **Language-aware**: Automatically respects the user's UI language setting
- **Graceful fallback**: If voice files are missing or audio is already in use, playback is skipped with a log message
- **Auto-cleanup**: The binding is automatically released after 3 seconds maximum to ensure game audio can bind
- **WAV Format Support**: Supports 16-bit stereo PCM WAV files at any sample rate
- **Game Audio Priority**: If game audio attempts to bind while voice is playing, it will fail gracefully with a log message. Voice is released after 3 seconds regardless to prevent interference with PvP session audio

## Testing

To verify the implementation:

1. Start a netplay session (either connect or wait for opponent)
2. Both players press "Ready"
3. When match_ready becomes true (both committed), the voice cue should play
4. Check logs for messages like:
   - `playing match-ready voice: en.wav` (successful)
   - `failed to load voice file: ...` (missing file)
   - `audio already bound, skipping match-ready voice` (audio already in use)

## Future Enhancements

- Add per-language voice files (currently using ja.wav for CJK, en.wav for others)
- Make voice playback configurable (enable/disable in Settings)
- Add additional voice cues for other lobby events (opponent ready, match starting)
- Support more audio formats beyond WAV
- Add volume control specific to voice cues

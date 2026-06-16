# Voice Playback - Issue Fixes

## Issues Encountered and Solutions

### Issue 1: Sound Playing at Extreme Speed
**Problem**: Voice played much faster than normal, suggesting sample rate mismatch or incorrect parsing.

**Root Cause**: WAV file parser wasn't reading the actual WAV file format correctly, or sample rate wasn't being preserved properly from the embedded files.

**Solution**: 
- Added detailed logging to WAV parser to output sample rate, channels, bits-per-sample, and calculated duration
- Ensured proper WAV chunk traversal with alignment
- Added validation to confirm 16-bit stereo format

Debug logs now show:
```
WAV format: 48000 Hz, 2 channels, 16 bits/sample, 384000 byte/s
loaded 235008 samples (4.90s) at 48000 Hz
```

### Issue 2: Black Screen / PvP Session Not Starting
**Problem**: When voice binding was created, it prevented the PvP session audio from binding, causing a black screen.

**Root Cause**: The audio binding system only allows ONE active stream. When voice bound successfully, the PvP session's attempt to bind its game audio would fail with `AlreadyBound` error. The PvP session logs `"pvp: audio bind failed"` and continues without audio, but the session may not render properly without the audio thread coordinating.

**Solution Implemented**:
1. **VoicePlayer now plays to completion** - When voice finishes, it returns silence but keeps the binding alive
2. **Auto-release after timeout** - Voice binding is automatically released after 3 seconds maximum
3. **Periodic cleanup** - `release_voice_binding_if_expired()` is called every update() to check and release expired bindings
4. **Timestamp tracking** - `voice_started_at` tracks when voice playback began

Timeline:
```
1. match_ready becomes true
   └─> Voice binding created (starts timer)
   
2. PvP session starts (< 3 seconds later)
   └─> Tries to bind game audio
   └─> Fails if voice still bound (logs warning, continues)
   
3. Next update() call
   └─> Checks if voice_started_at > 3 seconds
   └─> Releases voice binding
   
4. PvP session audio can now bind successfully
   └─> Game audio flows properly
```

## Implementation Details

### VoicePlayer Structure
```rust
pub struct VoicePlayer {
    clip: VoiceClip,           // The actual audio samples
    finished: bool,            // Track if playback completed
}
```

### Binding Release Logic
```rust
fn release_voice_binding_if_expired(&mut self) {
    if let Some(started) = self.voice_started_at {
        if started.elapsed() > Duration::from_secs(3) {
            self._voice_clip_binding = None;  // Release binding
            self.voice_started_at = None;     // Reset timer
        }
    }
}
```

### Update Cycle Integration
Every `update_inner()` call now:
1. Checks if voice binding has expired (3+ seconds old)
2. Releases it if expired
3. This happens BEFORE processing any messages, ensuring cleanup is timely

## Testing Checklist

- [ ] Voice plays at normal speed (not fast-forwarded)
- [ ] Check logs show `"playing match-ready voice: en.wav"` or `"playing match-ready voice: ja.wav"`
- [ ] Check logs show voice duration correctly (e.g., `"loaded 235008 samples (4.90s)"`)
- [ ] PvP session starts normally after voice plays
- [ ] Game audio works (no black screen)
- [ ] After ~3 seconds, logs show `"released voice binding after ~X.XXs"`
- [ ] Next voice trigger works correctly

## Future Improvements

1. **Actual voice finish detection**: Instead of 3-second timeout, detect when VoiceClip actually finishes by monitoring stream return value
2. **Configurable voice volume**: Add a separate volume control for voice cues
3. **Additional voice cues**: Add voices for other lobby events (opponent ready, match countdown)
4. **Per-language voices**: Replace ja.wav for CJK with better regional variants if available

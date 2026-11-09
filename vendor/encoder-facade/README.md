# encoder-facade

Video/audio encoding behind one API, with two backends feeding one set
of Rust muxers.

```text
  frames + samples ──► Backend ──► Packet ──► Session ──► Muxer ──► bytes
```

Only the encoding is platform-specific. `FfmpegBackend` runs a
subprocess per stream and reads back the fragmented MP4 each one writes;
`WebCodecsBackend` drives the browser's encoders on wasm32. Timing,
interleaving, containers and chapters are shared, and both containers
(MP4 and Matroska) are written here in Rust — so a browser needs no
muxer of its own, and a desktop needs ffmpeg only to encode.

Two seams make that composable: a `Packet` is one encoded access unit
timestamped in its track's own integer timebase, so a new encoder plugs
in by implementing `Backend`, and a new container by implementing
`mux::Muxer`.

Extracted from [tango](https://github.com/tangobattle/tango), which
renders replays through it.

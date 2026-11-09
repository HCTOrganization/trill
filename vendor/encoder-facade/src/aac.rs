//! The AAC configuration, in the form the containers want it.
//!
//! [`Backend::codec_private`](crate::Backend::codec_private) promises an
//! `AudioSpecificConfig` for an AAC track: the two bytes of audio object
//! type, sampling frequency index and channel configuration that MP4
//! nests in its `esds` and Matroska stores verbatim as `CodecPrivate`.
//! The ffmpeg backend reads exactly that out of its encoder's sample
//! entry. A browser is asked for it through WebCodecs, and one of them
//! answers with something bigger — so this is where an answer is turned
//! back into the config, and where a config is checked before a muxer
//! writes a track around it.

/// MPEG-4 descriptor tags (ISO/IEC 14496-1 §7.2.2).
const ES_DESCRIPTOR: u8 = 0x03;
const DECODER_CONFIG: u8 = 0x04;
const DECODER_SPECIFIC_INFO: u8 = 0x05;

/// Fixed fields at the head of a DecoderConfigDescriptor, before the
/// descriptors nested in it: object type indication, stream type and
/// flags, buffer size, then the maximum and average bitrates.
const DECODER_CONFIG_HEADER: usize = 1 + 1 + 3 + 4 + 4;

/// The `AudioSpecificConfig` inside what an encoder reported as its
/// configuration.
///
/// WebCodecs' AAC registration says `decoderConfig.description` *is* the
/// config, and Chrome's is: `11 90` for 48 kHz stereo AAC-LC. WebKit
/// reports AudioToolbox's magic cookie instead — the whole ES_Descriptor
/// chain with the config buried in its DecoderSpecificInfo — and the
/// first two bytes of that are `03 80`, which read as audio object type
/// 0 at 22050 Hz with no channels. Nothing rejects it, either: the file
/// muxes and plays, carrying an audio track no decoder will touch, which
/// is a video that is simply silent.
///
/// So a descriptor chain is unwrapped here, at the one seam that knows
/// it came from a browser. Anything that isn't a chain already is the
/// config and comes back untouched — including a future WebKit that
/// reports what the registration asks for.
pub(crate) fn specific_config(reported: Vec<u8>) -> Vec<u8> {
    match descriptor(&reported, DECODER_SPECIFIC_INFO) {
        Some(config) if !config.is_empty() => config.to_vec(),
        _ => reported,
    }
}

/// The three fields a two-byte `AudioSpecificConfig` carries.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SpecificConfig {
    pub object_type: u8,
    pub frequency_index: u8,
    pub channels: u8,
}

/// Read a config's leading two bytes: 5 bits of audio object type, 4 of
/// sampling frequency index, 4 of channel configuration.
///
/// `None` for a config those two bytes can't describe on their own —
/// object type 0 (none) or 31 (escape to a wider field), frequency index
/// 15 (escape to an explicit 24-bit rate), or a channel configuration of
/// 0 (the layout is in the bitstream's own program config element).
/// Callers mux the three fields, so a config they can't state is one to
/// refuse rather than round off.
pub(crate) fn parse(config: &[u8]) -> Option<SpecificConfig> {
    let [a, b, ..] = config else { return None };
    let parsed = SpecificConfig {
        object_type: a >> 3,
        frequency_index: ((a & 0b111) << 1) | (b >> 7),
        channels: (b >> 3) & 0b1111,
    };
    let usable = parsed.object_type != 0
        && parsed.object_type != 31
        && parsed.frequency_index != 15
        && (1..=7).contains(&parsed.channels);
    usable.then_some(parsed)
}

/// The payload of the first `tag` descriptor in an MPEG-4 descriptor
/// chain, looking inside the two descriptors that nest others.
///
/// `None` when `data` isn't a descriptor chain at all — which is how a
/// bare config is told apart from a cookie carrying one. A config is too
/// short to be mistaken for a chain: two bytes leave nothing for a
/// descriptor's payload once its tag and length are read.
fn descriptor(data: &[u8], tag: u8) -> Option<&[u8]> {
    let mut rest = data;
    while let Some((&here, after_tag)) = rest.split_first() {
        let (length, body) = descriptor_length(after_tag)?;
        let payload = body.get(..length)?;
        if here == tag {
            return Some(payload);
        }
        let nested = match here {
            ES_DESCRIPTOR => es_descriptor_payload(payload),
            DECODER_CONFIG => payload.get(DECODER_CONFIG_HEADER..),
            _ => None,
        };
        if let Some(found) = nested.and_then(|nested| descriptor(nested, tag)) {
            return Some(found);
        }
        rest = &body[length..];
    }
    None
}

/// A descriptor's length: seven bits per byte, high bit set on all but
/// the last, up to four bytes. Returns the length and what follows it.
fn descriptor_length(data: &[u8]) -> Option<(usize, &[u8])> {
    let mut length = 0usize;
    for at in 0..4 {
        let &byte = data.get(at)?;
        length = (length << 7) | (byte & 0x7f) as usize;
        if byte & 0x80 == 0 {
            return Some((length, &data[at + 1..]));
        }
    }
    None
}

/// What follows an ES_Descriptor's own header: its 16-bit id and flags
/// byte, plus whichever optional fields those flags announce.
fn es_descriptor_payload(payload: &[u8]) -> Option<&[u8]> {
    let &flags = payload.get(2)?;
    let mut at = 3;
    if flags & 0x80 != 0 {
        at += 2; // depends on another stream
    }
    if flags & 0x40 != 0 {
        at += 1 + *payload.get(at)? as usize; // a length-prefixed URL
    }
    if flags & 0x20 != 0 {
        at += 2; // has its own clock reference stream
    }
    payload.get(at..)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What Safari 26 reports for `mp4a.40.2` at 48 kHz stereo: the
    /// AudioToolbox magic cookie, with `11 90` inside it.
    const SAFARI_COOKIE: &[u8] = &[
        0x03, 0x80, 0x80, 0x80, 0x22, 0x00, 0x00, 0x00, 0x04, 0x80, 0x80, 0x80, 0x14, 0x40, 0x14, 0x00, 0x18, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x80, 0x80, 0x80, 0x02, 0x11, 0x90, 0x06, 0x80, 0x80,
        0x80, 0x01, 0x02,
    ];

    /// The config itself, which is what Chrome reports directly.
    const CONFIG: &[u8] = &[0x11, 0x90];

    #[test]
    fn a_magic_cookie_yields_the_config_inside_it() {
        assert_eq!(specific_config(SAFARI_COOKIE.to_vec()), CONFIG);
    }

    #[test]
    fn a_bare_config_is_left_alone() {
        assert_eq!(specific_config(CONFIG.to_vec()), CONFIG);
    }

    #[test]
    fn the_config_reads_as_what_was_asked_for() {
        assert_eq!(
            parse(CONFIG),
            Some(SpecificConfig {
                object_type: 2,     // AAC-LC
                frequency_index: 3, // 48000 Hz
                channels: 2,
            })
        );
    }

    /// The regression this module exists for: the first two bytes of the
    /// cookie parse into a track nothing can play, so they must not pass
    /// for a config.
    #[test]
    fn the_head_of_a_cookie_is_not_a_config() {
        assert_eq!(parse(&SAFARI_COOKIE[..2]), None);
    }

    #[test]
    fn nothing_shorter_than_a_config_is_one() {
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&[0x11]), None);
    }
}

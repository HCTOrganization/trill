use super::arm_core;
use super::sync;
use super::timing;

pub const SCREEN_WIDTH: u32 = mgba_sys::GBA_VIDEO_HORIZONTAL_PIXELS as u32;
pub const SCREEN_HEIGHT: u32 = mgba_sys::GBA_VIDEO_VERTICAL_PIXELS as u32;

/// Canonical BGR555 → RGBA8 expansion, indexed by the 15-bit value
/// (`r | g << 5 | b << 10`). Built at compile time so every consumer of
/// mGBA framebuffers shares identical colors at one lookup per pixel.
static BGR555_RGBA8_LUT: [[u8; 4]; 0x8000] = {
    let mut arr = [[0u8; 4]; 0x8000];
    let mut i = 0usize;
    while i < 0x8000 {
        let r = ((i & 0x1f) * 0xff / 0x1f) as u8;
        let g = (((i >> 5) & 0x1f) * 0xff / 0x1f) as u8;
        let b = (((i >> 10) & 0x1f) * 0xff / 0x1f) as u8;
        arr[i] = [r, g, b, 0xff];
        i += 1;
    }
    arr
};

/// Convert a `BGR5` framebuffer — what `COLOR_16_BIT` builds emit: one
/// little-endian `u16` per pixel holding the GBA-native 15-bit color —
/// into RGBA8.
///
/// `src` is 2 bytes per pixel and `dst` 4 bytes per pixel; conversion
/// runs over whole pixels and stops when either buffer is exhausted.
/// Alpha is forced opaque. Bit 15 is unused in GBA BGR555 (mGBA emits
/// 0), so it's masked off — which also proves the LUT index in range.
pub fn bgr555_to_rgba8(src: &[u8], dst: &mut [u8]) {
    for (s, d) in src.chunks_exact(2).zip(dst.chunks_exact_mut(4)) {
        let v = u16::from_le_bytes([s[0], s[1]]) & 0x7fff;
        d.copy_from_slice(&BGR555_RGBA8_LUT[v as usize]);
    }
}

/// The GBA board, viewed in place: a `#[repr(transparent)]` wrapper over
/// the C struct, only ever handed out as `&Gba` / `&mut Gba` borrowed
/// from a [`Core`](crate::core::Core).
#[repr(transparent)]
pub struct Gba(pub(super) mgba_sys::GBA);

impl Gba {
    pub fn cpu(&self) -> &arm_core::ArmCore {
        unsafe { &*(self.0.cpu as *const arm_core::ArmCore) }
    }

    pub fn cpu_mut(&mut self) -> &mut arm_core::ArmCore {
        unsafe { &mut *(self.0.cpu as *mut arm_core::ArmCore) }
    }

    pub fn timing(&self) -> &timing::Timing {
        unsafe { &*(&self.0.timing as *const _ as *const timing::Timing) }
    }

    pub fn master_volume(&self) -> i32 {
        self.0.audio.masterVolume
    }

    pub fn set_master_volume(&mut self, volume: i32) {
        self.0.audio.masterVolume = volume;
    }

    /// Set the video frameskip. A large value makes the GBA video module skip
    /// `drawScanline` + `finishFrame` outright (both gated on
    /// `frameskipCounter <= 0` in gba/video.c), so the core advances game logic
    /// without rasterizing. Used on the headless fast-forward and shadow cores,
    /// whose pixels are never shown — the display core re-renders from the save
    /// states they capture, and VRAM/IO are driven by the CPU, not the renderer.
    /// `frameskip` isn't part of the serialized state, so loading a save state
    /// (e.g. one captured on a rendering core) won't clear it.
    pub fn set_frameskip(&mut self, frameskip: i32) {
        self.0.video.frameskip = frameskip;
        self.0.video.frameskipCounter = frameskip;
    }

    pub fn sync(&self) -> Option<&sync::Sync> {
        let sync_ptr = self.0.sync;
        if sync_ptr.is_null() {
            None
        } else {
            Some(unsafe { &*(sync_ptr as *const sync::Sync) })
        }
    }

    pub fn sync_mut(&mut self) -> Option<&mut sync::Sync> {
        let sync_ptr = self.0.sync;
        if sync_ptr.is_null() {
            None
        } else {
            Some(unsafe { &mut *(sync_ptr as *mut sync::Sync) })
        }
    }

    /// The raw C struct, for state surgery the bindings don't cover
    /// (mgba-siolink's snapshot side-channels).
    pub fn as_raw(&mut self) -> *mut mgba_sys::GBA {
        &mut self.0
    }
}

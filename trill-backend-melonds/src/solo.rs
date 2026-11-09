//! One DS booted alone, as the seam's [`Console`](trill_match::Console).

use crate::link::{input_of, rtc_parts, DsSide, Screens};

/// One DS booted alone, as the seam's [`Console`](trill_match::Console):
/// a single core, not a pair with an idle seat.
pub struct SoloConsole {
    inner: melonds_rollback::Solo,
    /// The screens this ride's frames carry — the whole console, since
    /// a cart played alone is played the way it shipped. Held rather
    /// than assumed so the composition reads off the same selection a
    /// linked pair's does.
    screens: Screens,
}

impl SoloConsole {
    /// Boot one console. `rtc` pins the cart clock, exactly as a
    /// match's negotiated clock does.
    pub fn new(rom: &[u8], save: Option<&[u8]>, rtc: std::time::SystemTime) -> Result<Self, melonds::Error> {
        Ok(SoloConsole {
            inner: melonds_rollback::Solo::new(rom, save, rtc_parts(rtc))?,
            screens: Screens::BOTH,
        })
    }
}

impl trill_match::Console for SoloConsole {
    fn tick(&mut self, input: trill_match::HostInput) -> Result<(), trill_match::Error> {
        self.inner.tick(input_of(input));
        Ok(())
    }

    fn side(&mut self) -> Box<dyn trill_match::Side + '_> {
        Box::new(DsSide(self.inner.side(), self.screens))
    }
}

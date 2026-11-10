//! Link-code generator. Produces a uniformly random numeric code for a
//! matchmaking rendezvous. Digits are easy to read out and retype, and
//! shorter input is accepted and left-padded with `0` (see
//! [`LinkIdent::parse`](crate::LinkIdent::parse)).

use rand::Rng;

/// Digits in a matchmaking link code. Also the length a typed code is
/// padded up to.
pub const LINK_CODE_DIGITS: usize = 8;

/// Digit sequences never used as-is: values that read as something
/// unpleasant out loud, paired with the value that replaces them wherever
/// they appear in a code. Written in hex so the source doesn't spell them
/// out either.
const BLOCKED_CODES: [(u64, u64); 7] = [
    (0x2304, 0x2305),
    (0x1_1B4B, 0x1_1B4C),
    (0x1_BF52, 0x1_BF53),
    (0xE_3F0B, 0xE_3F0C),
    (0x1D_4B42, 0x1D_4B43),
    (0x12F_81AC, 0x12F_81AD),
    (0x174_D093, 0x174_D094),
];

/// Codes a blocked match may be swapped for wholesale, half the time.
const WISHED_CODES: [u64; 9] = [
    19_871_217, 20_010_321, 20_030_429, 20_030_717, 20_061_214, 20_160_624, 20_260_327, 20_261_108, 72_794_137,
];

/// Chance a blocked match swaps the whole code for a wished one, instead
/// of only replacing the offending digits.
const WISHED_CHANCE: f64 = 0.5;

/// Whether `code` contains any blocked digit sequence.
fn contains_blocked(code: &str) -> bool {
    BLOCKED_CODES
        .iter()
        .any(|&(blocked, _)| code.contains(&blocked.to_string()))
}

/// Wished codes that don't themselves carry a blocked sequence: the random
/// branch must never hand one back.
fn clean_wished_codes() -> Vec<String> {
    WISHED_CODES
        .iter()
        .map(|&code| format!("{code:0width$}", width = LINK_CODE_DIGITS))
        .filter(|code| !contains_blocked(code))
        .collect()
}

/// Replace blocked digit sequences in `code`. Half the time the whole code
/// becomes one of the wished codes instead.
fn substitute(code: String) -> String {
    if !contains_blocked(&code) {
        return code;
    }

    let mut rng = rand::thread_rng();
    let wished = clean_wished_codes();
    if !wished.is_empty() && rng.gen_bool(WISHED_CHANCE) {
        return wished[rng.gen_range(0..wished.len())].clone();
    }

    let mut code = code;
    for &(blocked, replacement) in &BLOCKED_CODES {
        code = code.replace(&blocked.to_string(), &replacement.to_string());
    }
    // A replacement can't normally reintroduce a blocked sequence, but the
    // whole point is to never emit one: fall back to a wished code if it did.
    if contains_blocked(&code) && !wished.is_empty() {
        return wished[rng.gen_range(0..wished.len())].clone();
    }
    code
}

/// A fresh random link code: exactly [`LINK_CODE_DIGITS`] digits, with
/// leading zeros preserved. Never contains a blocked sequence.
pub fn generate() -> String {
    let code = rand::thread_rng().gen_range(0..10u64.pow(LINK_CODE_DIGITS as u32));
    substitute(format!("{code:0width$}", width = LINK_CODE_DIGITS))
}

/// Canonicalize a typed matchmaking code: 1..=[`LINK_CODE_DIGITS`] ASCII
/// digits, left-padded with `0` to exactly [`LINK_CODE_DIGITS`]. A blocked
/// digit sequence in the result is replaced — half the time by swapping
/// the whole code for a wished code. `None` for anything else — malformed
/// input is never reinterpreted.
pub fn canonical(input: &str) -> Option<String> {
    if input.is_empty() || input.len() > LINK_CODE_DIGITS || !input.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let code: u64 = input.parse().ok()?;
    Some(substitute(format!("{code:0width$}", width = LINK_CODE_DIGITS)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_codes_are_eight_digits() {
        for _ in 0..64 {
            let code = generate();
            assert_eq!(code.len(), LINK_CODE_DIGITS);
            assert!(code.bytes().all(|b| b.is_ascii_digit()));
        }
    }

    #[test]
    fn canonical_pads_short_codes_and_rejects_bad_input() {
        assert_eq!(canonical("1234").as_deref(), Some("00001234"));
        assert_eq!(canonical("00001234").as_deref(), Some("00001234"));
        assert_eq!(canonical("0").as_deref(), Some("00000000"));
        assert!(canonical("").is_none());
        assert!(canonical("123456789").is_none());
        assert!(canonical("12a4").is_none());
        assert!(canonical("-1234").is_none());
    }

    #[test]
    fn blocked_sequences_are_replaced_wherever_they_appear() {
        let wished = clean_wished_codes();
        for &(blocked, replacement) in &BLOCKED_CODES {
            let pattern = blocked.to_string();
            let replacement = replacement.to_string();
            let room = LINK_CODE_DIGITS - pattern.len();
            for lead in 0..=room {
                let code = format!("{}{pattern}{}", "3".repeat(lead), "0".repeat(room - lead));
                let in_place = format!("{}{replacement}{}", "3".repeat(lead), "0".repeat(room - lead));
                let out = canonical(&code).expect("eight digits is submittable");
                assert_eq!(out.len(), LINK_CODE_DIGITS);
                assert!(!contains_blocked(&out), "{out} still carries a blocked sequence");
                assert!(
                    out == in_place || wished.contains(&out),
                    "{out} is neither {in_place} nor a wished code"
                );
            }
        }
    }

    #[test]
    fn padding_can_expose_a_blocked_sequence() {
        let wished = clean_wished_codes();
        for &(blocked, replacement) in &BLOCKED_CODES {
            let padded = format!("{blocked:0width$}", width = LINK_CODE_DIGITS);
            let in_place = padded.replacen(&blocked.to_string(), &replacement.to_string(), 1);
            let out = canonical(&blocked.to_string()).expect("a bare sequence is submittable");
            assert_ne!(out, padded);
            assert!(!contains_blocked(&out), "{out} still carries a blocked sequence");
            assert!(out == in_place || wished.contains(&out));
        }
    }

    #[test]
    fn generated_codes_never_contain_a_blocked_sequence() {
        for _ in 0..1_000 {
            assert!(!contains_blocked(&generate()));
        }
    }

    #[test]
    fn wished_codes_are_clean() {
        let wished = clean_wished_codes();
        assert!(!wished.is_empty());
        assert!(wished
            .iter()
            .all(|code| code.len() == LINK_CODE_DIGITS && !contains_blocked(code)));
    }

    #[test]
    fn blocked_matches_use_both_branches() {
        let wished = clean_wished_codes();
        let &(blocked, replacement) = BLOCKED_CODES
            .iter()
            .find(|&&(blocked, _)| blocked.to_string().len() < LINK_CODE_DIGITS)
            .expect("a sequence shorter than a code");
        let pattern = blocked.to_string();
        let pad = "0".repeat(LINK_CODE_DIGITS - pattern.len());
        let code = format!("{pattern}{pad}");
        let in_place = format!("{replacement}{pad}");
        assert!(
            !wished.contains(&in_place),
            "pick a code whose in-place result differs from every wished code"
        );

        let (mut saw_in_place, mut saw_wished) = (false, false);
        for _ in 0..128 {
            let out = canonical(&code).expect("valid code");
            assert!(!contains_blocked(&out));
            saw_in_place |= out == in_place;
            saw_wished |= wished.contains(&out);
        }
        assert!(saw_in_place, "the in-place branch never ran");
        assert!(saw_wished, "the wished-code branch never ran");
    }
}

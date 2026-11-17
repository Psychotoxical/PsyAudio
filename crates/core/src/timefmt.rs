//! Durations and encoding speed in words.

/// Seconds as words. Splits into days, hours, minutes and seconds
/// (/3600, %24, /60), shows the largest non-zero unit and, when `verbose` or that unit is below 4,
/// the next unit after a space. Singular for exactly 1.
pub fn long_duration(total: u64, verbose: bool) -> String {
    let hours_all = total / 3600;
    let rest = total % 3600;
    let (d, h, m, s) = (hours_all / 24, hours_all % 24, rest / 60, rest % 60);
    let unit = |n: u64, one: &str, many: &str| crate::i18n::tf(crate::i18n::t(if n == 1 { one } else { many }), &[("0", n.to_string())]);
    let parts = [
        (d, unit(d, "{0} day", "{0} days")),
        (h, unit(h, "{0} hour", "{0} hours")),
        (m, unit(m, "{0} minute", "{0} minutes")),
        (s, unit(s, "{0} second", "{0} seconds")),
    ];
    match parts.iter().position(|(n, _)| *n > 0) {
        None => unit(0, "{0} second", "{0} seconds"),
        Some(3) => parts[3].1.clone(),
        Some(i) => {
            let mut out = parts[i].1.clone();
            if verbose || parts[i].0 < 4 {
                out.push(' ');
                out.push_str(&parts[i + 1].1);
            }
            out
        }
    }
}

/// Audio length / elapsed time (elapsed 0 counts as 1);
/// up to 4× shown with one decimal, above that rounded (+0.5) to a whole number.
pub fn encoding_speed(audio_secs: f64, elapsed_secs: f64) -> String {
    let e = if elapsed_secs == 0.0 { 1.0 } else { elapsed_secs };
    let x = audio_secs / e;
    let n = if x <= 4.0 { format!("{x:.1}") } else { format!("{}", (x + 0.5) as i64) };
    crate::i18n::tf(crate::i18n::t("{0}× encoding speed"), &[("0", n)])
}

#[cfg(test)]
mod finish_tests {
    use super::*;
    #[test]
    fn duration_and_speed_wording() {
        assert_eq!(long_duration(0, true), "0 seconds");
        assert_eq!(long_duration(1, true), "1 second");
        assert_eq!(long_duration(75, false), "1 minute 15 seconds");
        assert_eq!(long_duration(5 * 60 + 9, false), "5 minutes");
        assert_eq!(long_duration(5 * 60 + 9, true), "5 minutes 9 seconds");
        assert_eq!(long_duration(90_000, true), "1 day 1 hour");
        assert_eq!(encoding_speed(300.0, 100.0), "3.0× encoding speed");
        assert_eq!(encoding_speed(300.0, 40.0), "8× encoding speed");
    }
}

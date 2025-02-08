use std::cmp::{max, min};
use std::iter::zip;

/// Check whether two strings are similar using the Jaro similarity algorithm.
///
/// https://en.wikipedia.org/wiki/Jaro%E2%80%93Winkler_distance#Jaro_similarity
pub fn jaro_similarity(lhs: &str, rhs: &str) -> f64 {
    let lhs_len = lhs.chars().count();
    let rhs_len = rhs.chars().count();

    if lhs_len == 0 && rhs_len == 0 {
        return 1.0; // Same length of 0, they match
    } else if lhs_len == 0 || rhs_len == 0 {
        return 0.0; // One is empty, they don't match
    }

    // How much to check around the current char.
    let match_distance = (max(lhs_len, rhs_len) / 2).saturating_sub(1);

    let mut matches = 0;
    let mut lhs_matches = vec![false; lhs_len];
    let mut rhs_matches = vec![false; rhs_len];

    // Cound the number of matches.
    for (lhs_i, lhs_chr) in lhs.chars().enumerate() {
        let min_bound = max(0, lhs_i.saturating_sub(match_distance));
        let max_bound = min(rhs_len, lhs_i + match_distance + 1);

        for (rhs_i, rhs_chr) in rhs.chars().enumerate().take(max_bound) {
            // Find a char within bounds in rhs that has not been matched yet and matches.
            if rhs_i >= min_bound && !rhs_matches[rhs_i] && lhs_chr == rhs_chr {
                lhs_matches[lhs_i] = true;
                rhs_matches[rhs_i] = true;
                matches += 1;
                break;
            }
        }
    }

    if matches == 0 {
        return 0.0;
    }

    // Count the number of transpositions.
    let lhs_iter = lhs.chars().zip(lhs_matches.iter().copied()).filter(|(_, matched)| *matched);
    let rhs_iter = rhs.chars().zip(rhs_matches.iter().copied()).filter(|(_, matched)| *matched);
    let transpositions = zip(lhs_iter, rhs_iter)
        .filter(|((lhs_chr, _lhs_filter), (rhs_chr, _rhs_filter))| *lhs_chr != *rhs_chr)
        .count();

    (1.0 / 3.0)
        * (matches as f64 / lhs_len as f64
            + matches as f64 / rhs_len as f64
            + (matches as f64 - transpositions as f64 / 2.0) / matches as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaro_similarity() {
        assert_float_eq(jaro_similarity("martha", "marhta"), 0.944);
        assert_float_eq(jaro_similarity("jellyfish", "smellyfish"), 0.896);
    }

    #[track_caller]
    fn assert_float_eq(lhs: f64, rhs: f64) {
        let delta = lhs - rhs;
        assert!(delta.abs() < 0.001, "{lhs} != {rhs}");
    }
}

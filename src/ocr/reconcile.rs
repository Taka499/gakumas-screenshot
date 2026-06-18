//! Checksum-based reconstruction of per-character scores corrupted by the
//! overlapping-million OCR failure (see docs/EXECPLAN_OVERLAP_SCORE_RECOVERY.md).
//!
//! When two adjacent per-character scores are both >= 1,000,000 the game renders
//! them so close that the right number's leading "1" overlaps the left number's
//! last digit. OCR then drops the right "1" and may misread the left number's
//! units digit. The screen also shows an isolated stage total and a bonus badge,
//! and the game guarantees the exact identity
//!
//!     stage_total = c1 + c2 + c3 + floor(max(c1, c2, c3) / 5)
//!
//! (the bonus is `floor(max/5)`), so `reconcile_stage` reconstructs the true
//! scores from the total alone via a small exhaustive search, using the bonus
//! only as an optional cross-check.

/// Confidence of a stage's reconstructed scores.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Recovery {
    /// Read cleanly (or the checksum confirmed the raw read with no edits).
    Ok,
    /// One or more values were repaired and the result is trusted.
    Repaired,
    /// Ambiguous or unverifiable: stored as best-effort, needs human review.
    Flagged,
}

/// A per-character score is never 2,000,000 or larger (invariant 1).
const MAX_SCORE: u32 = 2_000_000;

/// Reconstructs one stage's three per-character scores from the OCR'd scores
/// plus the optional stage total and bonus.
///
/// See the module docs and the ExecPlan for the full algorithm. In brief:
/// validate the total/bonus (step 0), then if the total is usable run an
/// exhaustive small search over per-slot candidate values (raw, optionally
/// +1,000,000 to restore a dropped leading "1", each with its units digit
/// replaced 0..=9), keep combinations that satisfy the checksum exactly, and
/// pick the one with the lowest *corruption-aware* cost. With no usable total,
/// fall back to a conservative structural-only pass.
pub fn reconcile_stage(
    ocr_scores: [u32; 3],
    total: Option<u32>,
    bonus: Option<u32>,
) -> ([u32; 3], Recovery) {
    let total_provided = total.is_some();

    // --- Step 0: validate inputs before trusting them. ---
    let max_raw = ocr_scores.iter().copied().max().unwrap_or(0);
    let total_ok = total.filter(|&t| t <= 9_999_999 && t >= max_raw && t > 0);
    let bonus_ok = bonus.filter(|&b| {
        b < 1_000_000 && total_ok.map_or(true, |t| b < t)
    });

    let Some(total) = total_ok else {
        return structural_only(ocr_scores, total_provided, bonus_ok);
    };

    // --- Steps 2–3: build candidates and collect checksum-satisfying combos. ---
    let cand = [
        candidates(ocr_scores[0]),
        candidates(ocr_scores[1]),
        candidates(ocr_scores[2]),
    ];

    let mut solutions: Vec<([u32; 3], u32)> = Vec::new(); // (combo, cost)
    for &a in &cand[0] {
        for &b in &cand[1] {
            for &c in &cand[2] {
                let max = a.max(b).max(c);
                if a + b + c + max / 5 == total {
                    let combo = [a, b, c];
                    solutions.push((combo, cost(combo, ocr_scores)));
                }
            }
        }
    }

    // --- Step 5 (zero solutions): the total was subtly wrong; flag. ---
    if solutions.is_empty() {
        return (ocr_scores, Recovery::Flagged);
    }

    // --- Step 4: pick the minimum-cost combo, using the bonus to break ties. ---
    let min_cost = solutions.iter().map(|&(_, c)| c).min().unwrap();
    let mut best: Vec<[u32; 3]> = solutions
        .iter()
        .filter(|&&(_, c)| c == min_cost)
        .map(|&(combo, _)| combo)
        .collect();

    if best.len() > 1 {
        if let Some(b) = bonus_ok {
            let corroborated: Vec<[u32; 3]> = best
                .iter()
                .copied()
                .filter(|combo| derived_bonus(*combo) == b)
                .collect();
            if !corroborated.is_empty() {
                best = corroborated;
            }
        }
    }

    // Deterministic order so a residual tie returns a stable best-effort value.
    best.sort_unstable();
    let chosen = best[0];

    let tie = best.len() > 1;
    let bonus_disagrees = bonus_ok.map_or(false, |b| derived_bonus(chosen) != b);

    let recovery = if tie || bonus_disagrees {
        Recovery::Flagged
    } else if min_cost == 0 {
        Recovery::Ok
    } else {
        Recovery::Repaired
    };

    (chosen, recovery)
}

/// `floor(max(combo) / 5)` — the bonus the game would render for this combo.
fn derived_bonus(combo: [u32; 3]) -> u32 {
    combo.iter().copied().max().unwrap_or(0) / 5
}

/// Builds the candidate value set for one slot (ExecPlan step 2).
///
/// Always includes the raw value. If the raw is a plausible victim of a dropped
/// leading "1" (>= 1000 and < 1,000,000), also includes raw + 1,000,000. For
/// every base >= 100,000, includes the ten units-digit variants (covers the
/// corrupted left-units digit). Generated variants are capped to < 2,000,000
/// (invariant 1); the raw is always kept. A dash slot (0) contributes only {0}.
fn candidates(v: u32) -> Vec<u32> {
    if v == 0 {
        return vec![0];
    }

    let mut bases = vec![v];
    if (1_000..1_000_000).contains(&v) {
        bases.push(v + 1_000_000);
    }

    let mut out = vec![v];
    for &b in &bases {
        if b < MAX_SCORE {
            out.push(b);
        }
        if b >= 100_000 {
            let floor10 = (b / 10) * 10;
            for d in 0..=9u32 {
                let variant = floor10 + d;
                if variant < MAX_SCORE {
                    out.push(variant);
                }
            }
        }
    }

    out.sort_unstable();
    out.dedup();
    out
}

/// Corruption-aware cost of a reconstructed combo relative to the raw OCR
/// (ExecPlan step 4). NOT a plain edit count: per invariant 3, an overlap
/// restores a leading million on the RIGHT operand of a junction and corrupts
/// only the units digit of its LEFT neighbour. So:
///   - +1 for each slot given a restored leading million,
///   - a units-digit change costs +1 only when the slot is immediately LEFT of
///     a restored slot (the expected victim), else +3.
fn cost(chosen: [u32; 3], raw: [u32; 3]) -> u32 {
    let restored = [
        raw[0] < 1_000_000 && chosen[0] >= 1_000_000,
        raw[1] < 1_000_000 && chosen[1] >= 1_000_000,
        raw[2] < 1_000_000 && chosen[2] >= 1_000_000,
    ];

    let mut c = 0u32;
    for i in 0..3 {
        if restored[i] {
            c += 1;
        }
        if chosen[i] % 10 != raw[i] % 10 {
            let left_of_restored = i + 1 < 3 && restored[i + 1];
            c += if left_of_restored { 1 } else { 3 };
        }
    }
    c
}

/// Structural-only fallback when no usable total is available (ExecPlan step 6).
///
/// Without the checksum we cannot recover from the numeric values alone — the
/// lost-million markers (a leading-zero group, or provenance of an over-split
/// token) live in the OCR *text*, which is not available here — so the raw
/// scores are returned as best-effort. The flag distinguishes the two reasons we
/// got here: a total that was *provided but rejected* as garbage is suspicious
/// (Flagged), as is a present bonus that disagrees with the raw maximum; an
/// absent total with a corroborating (or absent) bonus is treated as a clean
/// read (Ok).
fn structural_only(
    ocr_scores: [u32; 3],
    total_provided: bool,
    bonus_ok: Option<u32>,
) -> ([u32; 3], Recovery) {
    let bonus_disagrees = bonus_ok.map_or(false, |b| derived_bonus(ocr_scores) != b);
    let recovery = if total_provided || bonus_disagrees {
        Recovery::Flagged
    } else {
        Recovery::Ok
    };
    (ocr_scores, recovery)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- The four real samples (ground truth from the ExecPlan). ---

    #[test]
    fn test_reconcile_102842_one_junction_all_three_million() {
        let (scores, rec) =
            reconcile_stage([1172669, 161196, 1093518], Some(3661912), Some(234533));
        assert_eq!(scores, [1172665, 1161196, 1093518]);
        assert_eq!(rec, Recovery::Repaired);
    }

    #[test]
    fn test_reconcile_003_mode_b_overflow_dash() {
        let (scores, rec) = reconcile_stage([1327534, 151661, 0], Some(2744700), Some(265506));
        assert_eq!(scores, [1327533, 1151661, 0]);
        assert_eq!(rec, Recovery::Repaired);
    }

    #[test]
    fn test_reconcile_005_leading_zero_victim() {
        let (scores, rec) = reconcile_stage([1083344, 62741, 0], Some(2362759), Some(216669));
        assert_eq!(scores, [1083349, 1062741, 0]);
        assert_eq!(rec, Recovery::Repaired);
    }

    #[test]
    fn test_reconcile_102623_regression_guard() {
        // Already-correct read; the checksum confirms it with zero edits.
        let (scores, rec) =
            reconcile_stage([912127, 1171024, 1004816], Some(3322171), Some(234204));
        assert_eq!(scores, [912127, 1171024, 1004816]);
        assert_eq!(rec, Recovery::Ok);
    }

    // --- Total-only tier: recovery must work without the bonus. ---

    #[test]
    fn test_reconcile_total_only_recovers_exactly() {
        let (scores, rec) = reconcile_stage([1327534, 151661, 0], Some(2744700), None);
        assert_eq!(scores, [1327533, 1151661, 0]);
        assert_eq!(rec, Recovery::Repaired);
    }

    // --- Cost model: the asymmetric cost breaks a plain-edit tie. ---

    #[test]
    fn test_asymmetric_cost_breaks_unit_trade_tie() {
        // Both combos satisfy the checksum for total 2,744,700 at plain edit
        // count 2 (restore + one units edit). The asymmetric cost charges the
        // units edit on the left neighbour (slot 0) only 1, but on the restored
        // slot itself (slot 1) 3, so the correct combo wins.
        let raw = [1327534, 151661, 0];
        let correct = [1327533, 1151661, 0]; // edits slot 0 units
        let wrong = [1327534, 1151660, 0]; // edits slot 1 units

        // Both genuinely satisfy the checksum (the tie is real).
        for combo in [correct, wrong] {
            let max = combo.iter().copied().max().unwrap();
            assert_eq!(combo[0] + combo[1] + combo[2] + max / 5, 2744700);
        }
        // Plain edit count ties them; the asymmetric cost does not.
        assert!(cost(correct, raw) < cost(wrong, raw));

        let (scores, rec) = reconcile_stage(raw, Some(2744700), None);
        assert_eq!(scores, correct);
        assert_eq!(rec, Recovery::Repaired);
    }

    // --- Over-detection guards: bad inputs flag, never silently corrupt. ---

    #[test]
    fn test_over_detected_bonus_ignored() {
        // 8-digit bonus → demoted to None in step 0; the total alone still
        // recovers the correct scores.
        let (scores, rec) =
            reconcile_stage([1327534, 151661, 0], Some(2744700), Some(23545335));
        assert_eq!(scores, [1327533, 1151661, 0]);
        assert_eq!(rec, Recovery::Repaired);
    }

    #[test]
    fn test_over_detected_total_rejected() {
        // 8-digit total → demoted to None; drops to structural-only. Must not
        // emit any checksum-derived (units-edited or million-restored) value.
        let (scores, rec) =
            reconcile_stage([1327534, 151661, 0], Some(27447007), Some(265506));
        assert_eq!(scores, [1327534, 151661, 0]);
        assert_eq!(rec, Recovery::Flagged);
    }

    #[test]
    fn test_unreachable_total_flags() {
        // A wrong total that no candidate combination can satisfy degrades to a
        // flagged best-effort read (the exact-checksum requirement is the final
        // guard). Note: an off-by-one total (2,744,701) is NOT usable here — it
        // is exactly satisfiable by the restore-only combo with slot 0's units
        // left uncorrected, so a compensating units error hides it. We use a
        // clearly-unreachable total instead.
        let (scores, rec) = reconcile_stage([1327534, 151661, 0], Some(2744600), None);
        assert_eq!(scores, [1327534, 151661, 0]);
        assert_eq!(rec, Recovery::Flagged);
    }

    #[test]
    fn test_off_by_one_total_is_satisfiable() {
        // Documents the compensating-error limitation: total off by +1 plus the
        // uncorrected +1 units error in slot 0 cancel, so this is "recovered"
        // (to the OCR'd, units-uncorrected value) rather than flagged.
        let (scores, _rec) = reconcile_stage([1327534, 151661, 0], Some(2744701), None);
        assert_eq!(scores, [1327534, 1151661, 0]);
    }

    #[test]
    fn test_no_total_clean_read_is_ok() {
        // No total provided and the bonus corroborates the raw maximum → Ok.
        let (scores, rec) =
            reconcile_stage([912127, 1171024, 1004816], None, Some(234204));
        assert_eq!(scores, [912127, 1171024, 1004816]);
        assert_eq!(rec, Recovery::Ok);
    }

    #[test]
    fn test_candidates_include_restore_and_units() {
        // Leading-zero victim: 62741 must yield 1,062,741 as a candidate.
        let c = candidates(62741);
        assert!(c.contains(&62741));
        assert!(c.contains(&1062741));
        // Units variants exist around the restored base.
        assert!(c.contains(&1062740) && c.contains(&1062749));
        // Capped below 2,000,000.
        assert!(c.iter().all(|&x| x < MAX_SCORE));
    }
}

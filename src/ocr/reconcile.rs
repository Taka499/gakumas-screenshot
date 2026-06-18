//! Checksum-based reconstruction of per-character scores corrupted by the
//! overlapping-million OCR failure (see docs/EXECPLAN_OVERLAP_SCORE_RECOVERY.md).
//!
//! The solver itself (`reconcile_stage`) is added in milestone M3. This module
//! currently provides the `Recovery` confidence flag used by `StageReadout`.

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

mod aggregation;
mod analyzer;
mod candidates;
mod contradiction;
mod extractor;
mod normalization;
mod reporting;
mod scoring;
mod segmentation;
mod semantic_matching;

pub use extractor::{ClaimRequest, EvidenceError, EvidenceExtractor, EvidenceInput};

#[cfg(test)]
use contradiction::contradiction_detected;
#[cfg(test)]
use normalization::normalize_text;

pub fn crate_status() -> &'static str {
    "evidence ready"
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

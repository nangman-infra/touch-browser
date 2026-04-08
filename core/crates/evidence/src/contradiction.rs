use std::collections::BTreeSet;

use crate::normalization::{
    anchor_tokens, contains_token_sequence, normalize_text, token_overlap_ratio, tokenize_all,
    tokenize_significant,
};

pub(crate) fn contradiction_detected(normalized_claim: &str, block_text: &str) -> bool {
    let normalized_block = normalize_text(block_text);

    if normalized_claim.is_empty() || normalized_block.is_empty() {
        return false;
    }

    CONTRADICTION_PATTERNS.iter().any(|pattern| {
        contradiction_matches_pattern(normalized_claim, &normalized_block, block_text, pattern)
    })
}

fn contains_phrase(text: &str, phrase: &str) -> bool {
    contains_token_sequence(text, phrase)
}

fn contradiction_matches_pattern(
    normalized_claim: &str,
    normalized_block: &str,
    raw_block: &str,
    pattern: &ContradictionPattern,
) -> bool {
    let claim_polarity = polarity_state(normalized_claim, pattern);

    if matches!(claim_polarity, PolarityState::None | PolarityState::Both) {
        return false;
    }

    let (same_phrase, opposite_phrase) = match claim_polarity {
        PolarityState::Positive => (pattern.positive, pattern.negative),
        PolarityState::Negative => (pattern.negative, pattern.positive),
        PolarityState::None | PolarityState::Both => return false,
    };

    let opposite_claim = normalized_claim.replacen(same_phrase, opposite_phrase, 1);
    if opposite_claim != normalized_claim && contains_phrase(normalized_block, &opposite_claim) {
        return true;
    }

    let claim_context_tokens = phrase_context_tokens(normalized_claim, same_phrase);
    let anchor_tokens = contradiction_anchor_tokens(normalized_claim, pattern);
    if claim_context_tokens.is_empty() && anchor_tokens.is_empty() {
        return false;
    }

    split_normalized_segments(raw_block)
        .into_iter()
        .any(|segment| {
            if !matches_opposite_polarity(claim_polarity, polarity_state(&segment, pattern)) {
                return false;
            }

            if !claim_context_tokens.is_empty() {
                return phrase_context_overlap(&claim_context_tokens, &segment, opposite_phrase);
            }

            token_overlap_ratio(&anchor_tokens, &tokenize_significant(&segment)) >= 0.6
        })
}

fn contradiction_anchor_tokens(
    normalized_claim: &str,
    pattern: &ContradictionPattern,
) -> Vec<String> {
    let polarity_tokens = tokenize_all(pattern.positive)
        .into_iter()
        .chain(tokenize_all(pattern.negative))
        .collect::<BTreeSet<_>>();

    anchor_tokens(
        &tokenize_significant(normalized_claim)
            .into_iter()
            .filter(|token| !polarity_tokens.contains(token))
            .collect::<Vec<_>>(),
    )
}

fn split_normalized_segments(text: &str) -> Vec<String> {
    text.split(['.', '!', '?', ';', ':', '\n', '\r'])
        .map(normalize_text)
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn phrase_context_overlap(claim_context_tokens: &[String], segment: &str, phrase: &str) -> bool {
    let segment_context_tokens = phrase_context_tokens(segment, phrase);
    if segment_context_tokens.is_empty() {
        return false;
    }

    token_overlap_ratio(claim_context_tokens, &segment_context_tokens) >= 0.5
}

fn matches_opposite_polarity(claim: PolarityState, support: PolarityState) -> bool {
    matches!(
        (claim, support),
        (PolarityState::Positive, PolarityState::Negative)
            | (PolarityState::Negative, PolarityState::Positive)
    )
}

fn polarity_state(text: &str, pattern: &ContradictionPattern) -> PolarityState {
    let positive_spans = phrase_match_spans(text, pattern.positive);
    let negative_spans = phrase_match_spans(text, pattern.negative);

    let has_negative = !negative_spans.is_empty();
    let has_positive = positive_spans.iter().any(|positive_span| {
        !negative_spans
            .iter()
            .any(|negative_span| span_contains(*negative_span, *positive_span))
    });

    match (has_positive, has_negative) {
        (true, true) => PolarityState::Both,
        (true, false) => PolarityState::Positive,
        (false, true) => PolarityState::Negative,
        (false, false) => PolarityState::None,
    }
}

fn phrase_match_spans(text: &str, phrase: &str) -> Vec<(usize, usize)> {
    let text_tokens = tokenize_all(text);
    let phrase_tokens = tokenize_all(phrase);

    if phrase_tokens.is_empty() || text_tokens.len() < phrase_tokens.len() {
        return Vec::new();
    }

    text_tokens
        .windows(phrase_tokens.len())
        .enumerate()
        .filter_map(|(index, window)| {
            (window == phrase_tokens.as_slice()).then_some((index, index + phrase_tokens.len()))
        })
        .collect()
}

fn phrase_context_tokens(text: &str, phrase: &str) -> Vec<String> {
    let tokens = tokenize_all(text);
    let phrase_tokens = tokenize_all(phrase);

    if phrase_tokens.is_empty() || tokens.len() < phrase_tokens.len() {
        return Vec::new();
    }

    let mut context_tokens = BTreeSet::new();

    for (_start, end) in phrase_match_spans(text, phrase) {
        let context_end = (end + 4).min(tokens.len());

        for token in &tokens[end..context_end] {
            if CONTRADICTION_CONTEXT_STOP_WORDS.contains(&token.as_str()) {
                continue;
            }
            if token.len() < 3 {
                continue;
            }
            context_tokens.insert(token.clone());
        }
    }

    context_tokens.into_iter().collect()
}

fn span_contains(container: (usize, usize), inner: (usize, usize)) -> bool {
    container.0 <= inner.0 && container.1 >= inner.1
}

const CONTRADICTION_CONTEXT_STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "as", "at", "be", "been", "being", "by", "for", "from", "in",
    "into", "is", "its", "of", "on", "that", "their", "this", "those", "to", "with",
];

struct ContradictionPattern {
    positive: &'static str,
    negative: &'static str,
}

const fn contradiction_pattern(
    positive: &'static str,
    negative: &'static str,
) -> ContradictionPattern {
    ContradictionPattern { positive, negative }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PolarityState {
    None,
    Positive,
    Negative,
    Both,
}

const CONTRADICTION_PATTERNS: &[ContradictionPattern] = &[
    contradiction_pattern("available", "not available"),
    contradiction_pattern("available", "unavailable"),
    contradiction_pattern("required", "not required"),
    contradiction_pattern("allowed", "not allowed"),
    contradiction_pattern("supported", "not supported"),
    contradiction_pattern("enabled", "not enabled"),
    contradiction_pattern("enabled", "disabled"),
    contradiction_pattern("synchronous", "asynchronous"),
    contradiction_pattern("blocking", "non blocking"),
    contradiction_pattern("mutable", "immutable"),
    contradiction_pattern("stateful", "stateless"),
    contradiction_pattern("encrypted", "unencrypted"),
];

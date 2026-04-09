use std::collections::BTreeSet;

use touch_browser_contracts::{SnapshotBlockKind, SnapshotBlockRole};

use crate::CandidateBlock;

pub(crate) fn apply_budget(
    budget: usize,
    candidates: &mut [CandidateBlock],
) -> Vec<CandidateBlock> {
    let estimated_tokens = candidates
        .iter()
        .map(|candidate| candidate.token_cost)
        .sum::<usize>();
    if estimated_tokens <= budget {
        return candidates.to_vec();
    }

    let mut ranked = candidates
        .iter()
        .enumerate()
        .collect::<Vec<(usize, &CandidateBlock)>>();
    ranked.sort_by(|(left_index, left), (right_index, right)| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.order.cmp(&right.order))
            .then_with(|| left_index.cmp(right_index))
    });

    let mut selected_indices = BTreeSet::new();
    let mut emitted_tokens = 0usize;
    select_navigation_candidates(
        &ranked,
        navigation_budget(budget),
        &mut selected_indices,
        &mut emitted_tokens,
    );
    select_main_text_candidates(
        candidates,
        budget,
        main_text_budget(budget),
        &mut selected_indices,
        &mut emitted_tokens,
    );
    select_ranked_candidates(&ranked, budget, &mut selected_indices, &mut emitted_tokens);

    candidates
        .iter()
        .enumerate()
        .filter(|(index, _)| selected_indices.contains(index))
        .map(|(_, candidate)| candidate.clone())
        .collect()
}

fn navigation_budget(budget: usize) -> usize {
    if budget < 32 {
        0
    } else {
        budget.div_ceil(4).max(24).min(budget / 2)
    }
}

fn main_text_budget(budget: usize) -> usize {
    if budget < 48 {
        0
    } else {
        budget.div_ceil(3).max(96).min(budget / 2)
    }
}

fn select_navigation_candidates(
    ranked: &[(usize, &CandidateBlock)],
    nav_budget: usize,
    selected_indices: &mut BTreeSet<usize>,
    emitted_tokens: &mut usize,
) {
    if nav_budget == 0 {
        return;
    }

    for (index, candidate) in ranked {
        if !is_navigation_candidate(candidate)
            || exceeds_budget(*emitted_tokens, candidate.token_cost, nav_budget)
        {
            continue;
        }

        *emitted_tokens += candidate.token_cost;
        selected_indices.insert(*index);

        if *emitted_tokens >= nav_budget {
            break;
        }
    }
}

fn select_ranked_candidates(
    ranked: &[(usize, &CandidateBlock)],
    budget: usize,
    selected_indices: &mut BTreeSet<usize>,
    emitted_tokens: &mut usize,
) {
    for (index, candidate) in ranked {
        if selected_indices.contains(index)
            || exceeds_budget(*emitted_tokens, candidate.token_cost, budget)
        {
            continue;
        }

        *emitted_tokens += candidate.token_cost;
        selected_indices.insert(*index);

        if *emitted_tokens >= budget {
            break;
        }
    }
}

fn select_main_text_candidates(
    candidates: &[CandidateBlock],
    budget: usize,
    text_budget: usize,
    selected_indices: &mut BTreeSet<usize>,
    emitted_tokens: &mut usize,
) {
    if text_budget == 0 {
        return;
    }

    let mut text_emitted_tokens = 0usize;
    for (index, candidate) in candidates.iter().enumerate() {
        if !is_primary_text_candidate(candidate)
            || selected_indices.contains(&index)
            || exceeds_budget(text_emitted_tokens, candidate.token_cost, text_budget)
            || exceeds_budget(*emitted_tokens, candidate.token_cost, budget)
        {
            continue;
        }

        text_emitted_tokens += candidate.token_cost;
        *emitted_tokens += candidate.token_cost;
        selected_indices.insert(index);

        if text_emitted_tokens >= text_budget {
            break;
        }
    }
}

fn exceeds_budget(emitted_tokens: usize, token_cost: usize, budget: usize) -> bool {
    emitted_tokens + token_cost > budget && emitted_tokens > 0
}

fn is_navigation_candidate(candidate: &CandidateBlock) -> bool {
    (candidate.zone != "main"
        && matches!(
            candidate.kind,
            SnapshotBlockKind::Link | SnapshotBlockKind::Button | SnapshotBlockKind::Input
        ))
        || matches!(
            candidate.role,
            SnapshotBlockRole::PrimaryNav
                | SnapshotBlockRole::SecondaryNav
                | SnapshotBlockRole::Cta
                | SnapshotBlockRole::FormControl
        )
}

fn is_primary_text_candidate(candidate: &CandidateBlock) -> bool {
    candidate.zone == "main"
        && matches!(candidate.kind, SnapshotBlockKind::Text)
        && matches!(candidate.role, SnapshotBlockRole::Content)
}

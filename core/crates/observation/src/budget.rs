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
            || exceeds_budget(
                *emitted_tokens,
                candidate.token_cost,
                nav_budget,
                selected_indices,
            )
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
            || exceeds_budget(
                *emitted_tokens,
                candidate.token_cost,
                budget,
                selected_indices,
            )
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

fn exceeds_budget(
    emitted_tokens: usize,
    token_cost: usize,
    budget: usize,
    selected_indices: &BTreeSet<usize>,
) -> bool {
    emitted_tokens + token_cost > budget && !selected_indices.is_empty()
}

fn is_navigation_candidate(candidate: &CandidateBlock) -> bool {
    matches!(
        candidate.kind,
        SnapshotBlockKind::Link | SnapshotBlockKind::Button | SnapshotBlockKind::Input
    ) || matches!(
        candidate.role,
        SnapshotBlockRole::PrimaryNav
            | SnapshotBlockRole::SecondaryNav
            | SnapshotBlockRole::Cta
            | SnapshotBlockRole::FormControl
    )
}

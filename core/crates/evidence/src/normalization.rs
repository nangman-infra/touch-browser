use std::{collections::BTreeSet, sync::OnceLock};

use ferrous_opencc::{config::BuiltinConfig, OpenCC};

use crate::ClaimRequest;

pub(crate) struct ClaimAnalysisInput {
    pub(crate) claim_tokens: Vec<String>,
    pub(crate) claim_cross_lingual_tokens: Vec<String>,
    pub(crate) claim_sequence_tokens: Vec<String>,
    pub(crate) claim_numeric_tokens: Vec<String>,
    pub(crate) claim_anchor_tokens: Vec<String>,
    pub(crate) claim_cross_lingual_anchor_tokens: Vec<String>,
    pub(crate) claim_qualifier_tokens: Vec<String>,
    pub(crate) normalized_claim: String,
    pub(crate) claim_contains_cjk: bool,
}

pub(crate) fn build_claim_analysis_input(claim: &ClaimRequest) -> ClaimAnalysisInput {
    let normalized_claim = normalize_text(&claim.statement);
    let claim_sequence_tokens = split_normalized_tokens(&normalized_claim);
    let claim_tokens = tokenize_significant(&claim.statement);
    let claim_cross_lingual_tokens = tokenize_cross_lingual_search(&claim.statement);
    ClaimAnalysisInput {
        claim_cross_lingual_tokens: claim_cross_lingual_tokens.clone(),
        claim_sequence_tokens,
        claim_numeric_tokens: numeric_tokens(&claim.statement),
        claim_anchor_tokens: anchor_tokens(&claim_tokens),
        claim_cross_lingual_anchor_tokens: anchor_tokens(&claim_cross_lingual_tokens),
        claim_qualifier_tokens: qualifier_tokens(&claim.statement),
        normalized_claim,
        claim_contains_cjk: claim.statement.chars().any(is_cjk_character),
        claim_tokens,
    }
}

pub(crate) fn claim_is_low_signal_noise(statement: &str, claim_tokens: &[String]) -> bool {
    let char_count = statement.chars().count();
    if char_count < 240 {
        return false;
    }

    let significant_tokens = claim_tokens
        .iter()
        .filter(|token| token.chars().count() >= 2)
        .collect::<Vec<_>>();
    if significant_tokens.len() < 18 {
        return false;
    }

    let unique_tokens = significant_tokens
        .iter()
        .map(|token| token.as_str())
        .collect::<BTreeSet<_>>();
    let unique_ratio = unique_tokens.len() as f64 / significant_tokens.len() as f64;
    let max_repetition = unique_tokens
        .iter()
        .map(|token| {
            significant_tokens
                .iter()
                .filter(|candidate| candidate.as_str() == *token)
                .count()
        })
        .max()
        .unwrap_or(0);
    let max_consecutive_repetition = max_consecutive_repetition(claim_tokens);
    let anchor_density =
        anchor_tokens(claim_tokens).len() as f64 / significant_tokens.len().max(1) as f64;

    unique_ratio <= 0.45
        || max_repetition >= 8
        || max_consecutive_repetition >= 6
        || (significant_tokens.len() >= 24 && anchor_density <= 0.22)
}

pub(crate) fn token_overlap_ratio(claim_tokens: &[String], block_tokens: &[String]) -> f64 {
    if claim_tokens.is_empty() {
        return 0.0;
    }

    let block_token_set = block_tokens.iter().cloned().collect::<BTreeSet<_>>();
    let matched = claim_tokens
        .iter()
        .filter(|claim_token| {
            block_token_set
                .iter()
                .any(|block_token| tokens_match(claim_token, block_token))
        })
        .count();

    matched as f64 / claim_tokens.len() as f64
}

pub(crate) fn tokenize_significant(text: &str) -> Vec<String> {
    split_normalized_tokens(&normalize_text(text))
        .into_iter()
        .flat_map(|token| expand_semantic_tokens(&token, true))
        .filter(|token| is_significant_token(token))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn numeric_tokens(text: &str) -> Vec<String> {
    normalize_text(text)
        .split_whitespace()
        .map(|token| token.replace(',', ""))
        .filter(|token| {
            !token.is_empty() && token.chars().all(|character| character.is_ascii_digit())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn tokenize_all(text: &str) -> Vec<String> {
    split_normalized_tokens(&normalize_text(text))
        .into_iter()
        .flat_map(|token| expand_semantic_tokens(&token, false))
        .filter(|token| !token.is_empty())
        .collect()
}

pub(crate) fn tokenize_cross_lingual_search(text: &str) -> Vec<String> {
    split_normalized_tokens(&normalize_text(text))
        .into_iter()
        .flat_map(cross_lingual_search_variants)
        .filter(|token| is_significant_token(token))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn normalize_text(text: &str) -> String {
    let normalized_source = normalize_chinese_variants(text);
    let mut normalized = String::with_capacity(text.len());
    let mut previous_kind = CharacterKind::Separator;

    for character in normalized_source
        .chars()
        .flat_map(|character| character.to_lowercase())
    {
        let current_kind = classify_character(character);
        if matches!(
            (previous_kind, current_kind),
            (CharacterKind::AsciiAlnum, CharacterKind::Cjk)
                | (CharacterKind::Cjk, CharacterKind::AsciiAlnum)
        ) && !normalized.ends_with(' ')
        {
            normalized.push(' ');
        }

        if matches!(current_kind, CharacterKind::AsciiAlnum | CharacterKind::Cjk) {
            normalized.push(character);
        } else {
            normalized.push(' ');
        }

        previous_kind = current_kind;
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn tokens_match(left: &str, right: &str) -> bool {
    if !left.is_ascii() || !right.is_ascii() {
        return left == right || left.contains(right) || right.contains(left);
    }

    left == right
        || (left.len() >= 4 && right.starts_with(left))
        || (right.len() >= 4 && left.starts_with(right))
}

pub(crate) fn contains_token_sequence(text: &str, phrase: &str) -> bool {
    let text_tokens = tokenize_all(text);
    let phrase_tokens = tokenize_all(phrase);
    if phrase_tokens.is_empty() || text_tokens.len() < phrase_tokens.len() {
        return false;
    }

    text_tokens
        .windows(phrase_tokens.len())
        .any(|window| window == phrase_tokens.as_slice())
}

pub(crate) fn anchor_tokens(claim_tokens: &[String]) -> Vec<String> {
    claim_tokens
        .iter()
        .filter(|token| token.len() >= 5)
        .filter(|token| !ANCHOR_STOP_WORDS.contains(&token.as_str()))
        .filter(|token| !QUALIFIER_TOKENS.contains(&token.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn qualifier_tokens(text: &str) -> Vec<String> {
    tokenize_all(text)
        .into_iter()
        .filter(|token| QUALIFIER_TOKENS.contains(&token.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn claim_mentions_version_or_release(claim_tokens: &[String]) -> bool {
    claim_tokens
        .iter()
        .any(|token| RELEASE_NOISE_TOKENS.contains(&token.as_str()) || is_version_like_token(token))
}

pub(crate) fn is_version_like_token(token: &str) -> bool {
    if let Some(rest) = token.strip_prefix('v') {
        return !rest.is_empty()
            && rest
                .chars()
                .all(|character| character.is_ascii_digit() || character == '.');
    }

    token.chars().filter(|character| *character == '.').count() >= 1
        && token
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
}

fn normalize_chinese_variants(text: &str) -> String {
    if !should_fold_chinese_variants(text) {
        return text.to_string();
    }

    chinese_t2s_converter()
        .map(|converter| converter.convert(text))
        .unwrap_or_else(|| text.to_string())
}

fn chinese_t2s_converter() -> Option<&'static OpenCC> {
    static CONVERTER: OnceLock<Option<OpenCC>> = OnceLock::new();

    CONVERTER
        .get_or_init(|| OpenCC::from_config(BuiltinConfig::T2s).ok())
        .as_ref()
}

fn should_fold_chinese_variants(text: &str) -> bool {
    text.chars().any(is_han_character)
        && !text.chars().any(is_japanese_kana_character)
        && !text.chars().any(is_hangul_character)
}

fn stem_token(token: &str) -> String {
    if !token.is_ascii() {
        return token.to_string();
    }

    let mut stemmed = token.to_string();

    for suffix in ["ing", "ed", "ly", "es", "s"] {
        if stemmed.len() > suffix.len() + 2 && stemmed.ends_with(suffix) {
            stemmed.truncate(stemmed.len() - suffix.len());
            break;
        }
    }

    stemmed
}

fn is_significant_token(token: &str) -> bool {
    if token.chars().all(|character| character.is_ascii_digit()) {
        return true;
    }

    if contains_cjk(token) {
        return token.chars().count() >= 2;
    }

    token.len() >= 3 && !STOP_WORDS.contains(&token)
}

fn split_normalized_tokens(normalized: &str) -> Vec<String> {
    normalized
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
}

fn max_consecutive_repetition(tokens: &[String]) -> usize {
    let mut max_run = 0usize;
    let mut current_run = 0usize;
    let mut previous: Option<&str> = None;

    for token in tokens {
        let token = token.as_str();
        if previous == Some(token) {
            current_run += 1;
        } else {
            current_run = 1;
            previous = Some(token);
        }
        max_run = max_run.max(current_run);
    }

    max_run
}

fn expand_semantic_tokens(token: &str, significant_only: bool) -> Vec<String> {
    let mut expanded = BTreeSet::new();
    let stemmed = stem_token(token);
    if !stemmed.is_empty() {
        expanded.insert(stemmed);
    }

    if contains_cjk(token) {
        for width in [2usize, 3usize] {
            for gram in cjk_ngrams(token, width) {
                if !significant_only || gram.chars().count() >= 2 {
                    expanded.insert(gram);
                }
            }
        }
    }

    expanded.into_iter().collect()
}

fn cross_lingual_search_variants(token: String) -> Vec<String> {
    let mut expanded = BTreeSet::new();
    let stemmed = stem_token(&token);

    if !contains_cjk(&token) {
        if !stemmed.is_empty() {
            expanded.insert(stemmed);
        }
        return expanded.into_iter().collect();
    }

    for gloss in cross_lingual_glosses(&token) {
        expanded.insert(gloss.to_string());
        let gloss_stem = stem_token(gloss);
        if !gloss_stem.is_empty() {
            expanded.insert(gloss_stem);
        }
    }

    expanded.into_iter().collect()
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(is_cjk_character)
}

fn is_cjk_character(character: char) -> bool {
    matches!(
        character as u32,
        0x3040..=0x30ff
            | 0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xac00..=0xd7af
            | 0xf900..=0xfaff
    )
}

fn is_han_character(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
    )
}

fn is_japanese_kana_character(character: char) -> bool {
    matches!(character as u32, 0x3040..=0x30ff)
}

fn is_hangul_character(character: char) -> bool {
    matches!(character as u32, 0xac00..=0xd7af)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CharacterKind {
    AsciiAlnum,
    Cjk,
    Separator,
}

fn classify_character(character: char) -> CharacterKind {
    if is_cjk_character(character) {
        CharacterKind::Cjk
    } else if character.is_ascii_alphanumeric() {
        CharacterKind::AsciiAlnum
    } else {
        CharacterKind::Separator
    }
}

fn cjk_ngrams(token: &str, width: usize) -> Vec<String> {
    let characters = token.chars().collect::<Vec<_>>();
    if characters.len() < width {
        return Vec::new();
    }

    (0..=characters.len() - width)
        .map(|index| characters[index..index + width].iter().collect::<String>())
        .collect()
}

fn cross_lingual_glosses(token: &str) -> &'static [&'static str] {
    if token.starts_with("제공") {
        return &["provides"];
    }
    if token.starts_with("지원") {
        return &["supports"];
    }
    if token.starts_with("인터페이스") {
        return &["interface"];
    }
    if token.starts_with("네트워크") {
        return &["network"];
    }
    if token.starts_with("요청") {
        return &["request", "fetching"];
    }
    if token.starts_with("리소스") {
        return &["resources"];
    }
    if token.starts_with("가져오") {
        return &["fetching"];
    }
    if token.starts_with("사용") {
        return &["used"];
    }
    if token.starts_with("문서") {
        return &["documentation"];
    }
    if token.starts_with("검색") {
        return &["search"];
    }
    if token.starts_with("추출") {
        return &["extract"];
    }
    if token.starts_with("세션") {
        return &["session"];
    }
    if token.starts_with("증거") || token.starts_with("근거") {
        return &["evidence"];
    }
    if token.starts_with("언어") {
        return &["language"];
    }
    if token.starts_with("컴파일") {
        return &["compiled"];
    }
    if token.starts_with("인터프리") || token.starts_with("해석") {
        return &["interpreted"];
    }
    if token.contains("提供") {
        return &["provides"];
    }
    if token.contains("支持") {
        return &["supports"];
    }
    if token.contains("接口") {
        return &["interface"];
    }
    if token.contains("网络") {
        return &["network"];
    }
    if token.contains("请求") {
        return &["request", "fetching"];
    }
    if token.contains("资源") {
        return &["resources"];
    }
    if token.contains("获取") || token.contains("抓取") {
        return &["fetching"];
    }
    if token.contains("文档") {
        return &["documentation"];
    }
    if token.contains("搜索") {
        return &["search"];
    }
    if token.contains("提取") {
        return &["extract"];
    }
    if token.contains("证据") || token.contains("依据") {
        return &["evidence"];
    }
    if token.contains("语言") {
        return &["language"];
    }
    if token.contains("编译") {
        return &["compiled"];
    }
    if token.contains("解释") {
        return &["interpreted"];
    }

    &[]
}

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "your", "must", "now", "are",
    "all", "per", "there", "page", "include", "includes", "includ", "contain", "contains", "list",
    "built", "flow", "runtime", "plan", "touch", "browser", "both", "modern", "feature", "app",
    "model", "style",
];

const ANCHOR_STOP_WORDS: &[&str] = &[
    "support",
    "avail",
    "available",
    "feature",
    "features",
    "modern",
    "app",
    "apps",
    "model",
    "provid",
    "service",
    "system",
    "platform",
];

const QUALIFIER_TOKENS: &[&str] = &[
    "all",
    "every",
    "fully",
    "native",
    "global",
    "worldwide",
    "only",
    "always",
    "never",
    "entire",
];

const RELEASE_NOISE_TOKENS: &[&str] = &[
    "upgrade",
    "upgrading",
    "changelog",
    "release",
    "releases",
    "remix",
    "migration",
    "migrat",
];

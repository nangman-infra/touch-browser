use std::{
    env,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

use crate::scoring::{semantic_similarity_bonus, ScoredCandidate};

const DEFAULT_EMBEDDING_MODEL_ID: &str = "Xenova/multilingual-e5-small";
const DEFAULT_NLI_MODEL_ID: &str = "Xenova/nli-deberta-v3-xsmall";
const SEMANTIC_RERANK_LIMIT: usize = 12;
const NLI_RERANK_LIMIT: usize = 5;
const STRONG_NLI_ENTAILMENT: f64 = 0.82;
const STRONG_NLI_CONTRADICTION: f64 = 0.94;
const STRONG_NLI_MARGIN: f64 = 0.20;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub(crate) struct NliScore {
    pub(crate) contradiction: f64,
    pub(crate) entailment: f64,
    pub(crate) neutral: f64,
}

pub(crate) fn rerank_candidates_with_semantic(
    claim_text: &str,
    candidates: &mut [ScoredCandidate<'_>],
) {
    let rerank_count = candidates.len().min(SEMANTIC_RERANK_LIMIT);
    if rerank_count == 0 {
        return;
    }

    let Some(model_root) = resolved_embedding_model_root() else {
        return;
    };

    let Some(runner_command) = embedding_runner_command() else {
        return;
    };
    let model_id = embedding_model_id();
    let Some(embeddings) = run_embedding_batch_with_backend(
        &embedding_request_texts(claim_text, candidates, rerank_count),
        &model_root,
        &runner_command,
        &model_id,
    ) else {
        return;
    };
    if embeddings.len() != rerank_count + 1 {
        return;
    }

    let claim_vector = &embeddings[0];
    if vector_is_zero(claim_vector) {
        return;
    }

    for (candidate, candidate_vector) in candidates
        .iter_mut()
        .take(rerank_count)
        .zip(embeddings.iter().skip(1))
    {
        let Some(semantic_similarity) = cosine_similarity(claim_vector, candidate_vector) else {
            continue;
        };
        let semantic_bonus =
            semantic_similarity_bonus(semantic_similarity, candidate.lexical_overlap);
        candidate.score = (candidate.score + semantic_bonus).min(1.0);
        candidate.signals.semantic_similarity = Some(semantic_similarity);
        candidate.signals.semantic_boost = Some(semantic_bonus);
    }

    sort_candidates_by_score(candidates);
}

pub(crate) fn rerank_candidates_with_nli(claim_text: &str, candidates: &mut [ScoredCandidate<'_>]) {
    let rerank_count = candidates.len().min(NLI_RERANK_LIMIT);
    if rerank_count == 0 {
        return;
    }

    let Some(model_root) = resolved_nli_model_root() else {
        return;
    };
    let Some(runner_command) = nli_runner_command() else {
        return;
    };
    let model_id = nli_model_id();
    let candidate_texts = candidates
        .iter()
        .take(rerank_count)
        .map(|candidate| candidate.text.clone())
        .collect::<Vec<_>>();
    let Some(results) = run_nli_batch_with_backend(
        claim_text,
        &candidate_texts,
        &model_root,
        &runner_command,
        &model_id,
    ) else {
        return;
    };

    for (candidate, nli) in candidates.iter_mut().take(rerank_count).zip(results.iter()) {
        apply_nli_reranking(candidate, nli);
    }

    sort_candidates_by_score(candidates);
}

pub(crate) fn score_nli_pairs(pairs: &[(String, String)]) -> Option<Vec<NliScore>> {
    if pairs.is_empty() {
        return Some(Vec::new());
    }

    let model_root = resolved_nli_model_root()?;
    let runner_command = nli_runner_command()?;
    let model_id = nli_model_id();
    let request = NliBatchRequest {
        model_id: model_id.clone(),
        pairs: pairs
            .iter()
            .map(|(premise, hypothesis)| NliPairRequest {
                premise: premise.clone(),
                hypothesis: hypothesis.clone(),
            })
            .collect(),
    };
    let request_body = serde_json::to_vec(&request).ok()?;

    let output = run_json_runner(
        &runner_command,
        &[
            ("TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH", &model_root),
            ("TOUCH_BROWSER_EVIDENCE_NLI_MODEL_ID", Path::new(&model_id)),
        ],
        &request_body,
    )?;

    let response: NliBatchResponse = serde_json::from_slice(&output).ok()?;
    Some(response.results)
}

fn sort_candidates_by_score(candidates: &mut [ScoredCandidate<'_>]) {
    candidates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn resolved_embedding_model_root() -> Option<PathBuf> {
    if env::var("TOUCH_BROWSER_EVIDENCE_DISABLE_LIVE_MODELS")
        .ok()
        .as_deref()
        == Some("1")
    {
        return None;
    }

    #[cfg(test)]
    if env::var_os("TOUCH_BROWSER_EVIDENCE_ENABLE_LIVE_MODELS").is_none()
        && env::var_os("TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_PATH").is_none()
        && env::var_os("TOUCH_BROWSER_EVIDENCE_EMBEDDING_RUNNER").is_none()
    {
        return None;
    }

    if let Some(model_path) = env::var_os("TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_PATH") {
        let model_root = PathBuf::from(model_path);
        return model_root.is_dir().then_some(model_root);
    }

    let home = env::var_os("HOME").map(PathBuf::from)?;
    let default_root = default_embedding_model_root_from_home(&home);
    default_root
        .join(".ready.json")
        .is_file()
        .then_some(default_root)
}

fn default_embedding_model_root_from_home(home: &Path) -> PathBuf {
    home.join(".touch-browser/models/evidence/embedding")
}

fn embedding_model_id() -> String {
    env::var("TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL_ID.to_string())
}

fn embedding_runner_command() -> Option<String> {
    if let Ok(command) = env::var("TOUCH_BROWSER_EVIDENCE_EMBEDDING_RUNNER") {
        if !command.trim().is_empty() {
            return Some(command);
        }
    }

    default_embedding_runner_script().map(|path| {
        format!(
            "{} {}",
            shell_escape_text(&runner_node_executable()),
            shell_escape(&path)
        )
    })
}

fn default_embedding_runner_script() -> Option<PathBuf> {
    if let Some(runtime_script) = runtime_resource_root()
        .map(|root| root.join("scripts/evidence-embedding-runner.mjs"))
        .filter(|path| path.is_file())
    {
        return Some(runtime_script);
    }

    let current_dir_script = std::env::current_dir()
        .ok()
        .map(|dir| dir.join("scripts/evidence-embedding-runner.mjs"));
    if let Some(path) = current_dir_script.filter(|path| path.is_file()) {
        return Some(path);
    }

    let build_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../scripts/evidence-embedding-runner.mjs");
    build_path.is_file().then_some(build_path)
}

fn resolved_nli_model_root() -> Option<PathBuf> {
    if env::var("TOUCH_BROWSER_EVIDENCE_DISABLE_LIVE_MODELS")
        .ok()
        .as_deref()
        == Some("1")
    {
        return None;
    }

    #[cfg(test)]
    if env::var_os("TOUCH_BROWSER_EVIDENCE_ENABLE_LIVE_MODELS").is_none()
        && env::var_os("TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH").is_none()
        && env::var_os("TOUCH_BROWSER_EVIDENCE_NLI_RUNNER").is_none()
    {
        return None;
    }

    if let Some(model_path) = env::var_os("TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH") {
        let model_root = PathBuf::from(model_path);
        return model_root.is_dir().then_some(model_root);
    }

    let home = env::var_os("HOME").map(PathBuf::from)?;
    let default_root = home.join(".touch-browser/models/evidence/nli");
    default_root
        .join(".ready.json")
        .is_file()
        .then_some(default_root)
}

fn nli_model_id() -> String {
    env::var("TOUCH_BROWSER_EVIDENCE_NLI_MODEL_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_NLI_MODEL_ID.to_string())
}

fn nli_runner_command() -> Option<String> {
    if let Ok(command) = env::var("TOUCH_BROWSER_EVIDENCE_NLI_RUNNER") {
        if !command.trim().is_empty() {
            return Some(command);
        }
    }

    default_nli_runner_script().map(|path| {
        format!(
            "{} {}",
            shell_escape_text(&runner_node_executable()),
            shell_escape(&path)
        )
    })
}

fn default_nli_runner_script() -> Option<PathBuf> {
    if let Some(runtime_script) = runtime_resource_root()
        .map(|root| root.join("scripts/evidence-nli-runner.mjs"))
        .filter(|path| path.is_file())
    {
        return Some(runtime_script);
    }

    let current_dir_script = std::env::current_dir()
        .ok()
        .map(|dir| dir.join("scripts/evidence-nli-runner.mjs"));
    if let Some(path) = current_dir_script.filter(|path| path.is_file()) {
        return Some(path);
    }

    let build_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../scripts/evidence-nli-runner.mjs");
    build_path.is_file().then_some(build_path)
}

fn repo_root_for_runner() -> PathBuf {
    if let Some(runtime_root) = runtime_resource_root() {
        return runtime_root;
    }

    std::env::current_dir()
        .ok()
        .filter(|dir| dir.join(".git").exists())
        .or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .canonicalize()
                .ok()
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

fn embedding_request_texts(
    claim_text: &str,
    candidates: &[ScoredCandidate<'_>],
    rerank_count: usize,
) -> Vec<String> {
    let mut texts = Vec::with_capacity(rerank_count + 1);
    texts.push(prefix_query_text(claim_text));
    texts.extend(
        candidates
            .iter()
            .take(rerank_count)
            .map(|candidate| prefix_passage_text(&candidate.text)),
    );
    texts
}

fn run_embedding_batch_with_backend(
    texts: &[String],
    model_root: &Path,
    runner_command: &str,
    model_id: &str,
) -> Option<Vec<Vec<f32>>> {
    let request = EmbeddingBatchRequest {
        model_id: model_id.to_string(),
        texts: texts.to_vec(),
    };
    let request_body = serde_json::to_vec(&request).ok()?;

    let output = run_json_runner(
        runner_command,
        &[
            ("TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_PATH", model_root),
            (
                "TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_ID",
                Path::new(model_id),
            ),
        ],
        &request_body,
    )?;

    let response: EmbeddingBatchResponse = serde_json::from_slice(&output).ok()?;
    Some(response.embeddings)
}

fn run_nli_batch_with_backend(
    claim_text: &str,
    candidate_texts: &[String],
    model_root: &Path,
    runner_command: &str,
    model_id: &str,
) -> Option<Vec<NliScore>> {
    let request = NliBatchRequest {
        model_id: model_id.to_string(),
        pairs: candidate_texts
            .iter()
            .map(|candidate_text| NliPairRequest {
                premise: candidate_text.clone(),
                hypothesis: claim_text.to_string(),
            })
            .collect(),
    };
    let request_body = serde_json::to_vec(&request).ok()?;

    let output = run_json_runner(
        runner_command,
        &[
            ("TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH", model_root),
            ("TOUCH_BROWSER_EVIDENCE_NLI_MODEL_ID", Path::new(model_id)),
        ],
        &request_body,
    )?;

    let response: NliBatchResponse = serde_json::from_slice(&output).ok()?;
    Some(response.results)
}

fn run_json_runner(
    runner_command: &str,
    envs: &[(&str, &Path)],
    request_body: &[u8],
) -> Option<Vec<u8>> {
    let mut command = Command::new("sh");
    command
        .args(["-lc", runner_command])
        .current_dir(repo_root_for_runner())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (key, value) in envs {
        command.env(key, value);
    }

    let mut child = command.spawn().ok()?;

    {
        let stdin = child.stdin.as_mut()?;
        stdin.write_all(request_body).ok()?;
    }
    let _ = child.stdin.take();

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }

    Some(output.stdout)
}

fn apply_nli_reranking(candidate: &mut ScoredCandidate<'_>, nli: &NliScore) {
    if has_strong_nli_contradiction(nli) && candidate.score >= 0.40 {
        candidate.contradictory = true;
        candidate.score = (candidate.score + 0.05).min(1.0);
        candidate.signals.nli_entailment = Some(nli.entailment);
        candidate.signals.nli_contradiction = Some(nli.contradiction);
        return;
    }

    if has_strong_nli_entailment(nli) {
        candidate.score = (candidate.score + 0.18).min(1.0);
        candidate.exact_support = true;
        candidate.signals.exact_support = true;
    }
    candidate.signals.nli_entailment = Some(nli.entailment);
    candidate.signals.nli_contradiction = Some(nli.contradiction);
}

pub(crate) fn has_strong_nli_contradiction(nli: &NliScore) -> bool {
    let contradiction_margin = nli.contradiction - nli.entailment.max(nli.neutral);
    nli.contradiction >= STRONG_NLI_CONTRADICTION && contradiction_margin >= STRONG_NLI_MARGIN
}

pub(crate) fn has_strong_nli_entailment(nli: &NliScore) -> bool {
    let entailment_margin = nli.entailment - nli.contradiction.max(nli.neutral);
    nli.entailment >= STRONG_NLI_ENTAILMENT && entailment_margin >= STRONG_NLI_MARGIN
}

fn prefix_query_text(text: &str) -> String {
    format!("query: {}", text.trim())
}

fn prefix_passage_text(text: &str) -> String {
    format!("passage: {}", text.trim())
}

fn shell_escape(path: &Path) -> String {
    let rendered = path.display().to_string();
    shell_escape_text(&rendered)
}

fn shell_escape_text(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn runner_node_executable() -> String {
    if let Some(explicit_node) =
        env::var_os("TOUCH_BROWSER_NODE_EXECUTABLE").filter(|value| !value.is_empty())
    {
        return PathBuf::from(explicit_node).display().to_string();
    }

    if let Some(runtime_root) = runtime_resource_root() {
        let bundled_node = runtime_root.join("node/bin/node");
        if bundled_node.is_file() {
            return bundled_node.display().to_string();
        }
    }

    "node".to_string()
}

fn runtime_resource_root() -> Option<PathBuf> {
    if let Some(explicit_root) =
        env::var_os("TOUCH_BROWSER_RESOURCE_ROOT").filter(|value| !value.is_empty())
    {
        return Some(canonical_or_raw(PathBuf::from(explicit_root)));
    }

    let current_exe = env::current_exe().ok()?;
    let bundled_runtime = current_exe
        .parent()
        .and_then(Path::parent)
        .map(|path| path.join("runtime"));
    if let Some(runtime_root) = bundled_runtime.filter(|path| path.exists()) {
        return Some(canonical_or_raw(runtime_root));
    }

    current_exe
        .parent()
        .map(|path| path.join("runtime"))
        .filter(|path| path.exists())
        .map(canonical_or_raw)
}

fn canonical_or_raw(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f64> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }

    let (dot, left_norm, right_norm) = left.iter().zip(right.iter()).fold(
        (0.0f64, 0.0f64, 0.0f64),
        |(dot, left_norm, right_norm), (left, right)| {
            let left = f64::from(*left);
            let right = f64::from(*right);
            (
                dot + (left * right),
                left_norm + (left * left),
                right_norm + (right * right),
            )
        },
    );

    let denominator = left_norm.sqrt() * right_norm.sqrt();
    (denominator > f64::EPSILON).then_some((dot / denominator).clamp(-1.0, 1.0))
}

fn vector_is_zero(vector: &[f32]) -> bool {
    vector.iter().all(|value| value.abs() <= f32::EPSILON)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EmbeddingBatchRequest {
    model_id: String,
    texts: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbeddingBatchResponse {
    embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NliBatchRequest {
    model_id: String,
    pairs: Vec<NliPairRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NliPairRequest {
    premise: String,
    hypothesis: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NliBatchResponse {
    results: Vec<NliScore>,
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    use touch_browser_contracts::{
        SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotEvidence, SourceType,
    };

    use super::{
        apply_nli_reranking, canonical_or_raw, cosine_similarity,
        default_embedding_model_root_from_home, default_embedding_runner_script,
        default_nli_runner_script, embedding_request_texts, repo_root_for_runner,
        run_embedding_batch_with_backend, run_nli_batch_with_backend, runner_node_executable,
        vector_is_zero, NliScore,
    };
    use crate::scoring::{semantic_similarity_bonus, CandidateMatchSignals, ScoredCandidate};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn cosine_similarity_requires_non_zero_vectors() {
        assert_eq!(cosine_similarity(&[], &[]), None);
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]), None);
        assert!(vector_is_zero(&[0.0, 0.0]));
        assert!(!vector_is_zero(&[0.0, 0.2]));
    }

    #[test]
    fn default_embedding_root_resolves_under_touch_browser_home() {
        let home = Path::new("/tmp/touch-browser-home");

        assert_eq!(
            default_embedding_model_root_from_home(home),
            PathBuf::from("/tmp/touch-browser-home/.touch-browser/models/evidence/embedding")
        );
    }

    #[test]
    fn embedding_runner_script_prefers_explicit_runtime_root() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_root = temporary_directory("embedding-runtime");
        let runner_script = runtime_root.join("scripts/evidence-embedding-runner.mjs");
        fs::create_dir_all(
            runner_script
                .parent()
                .expect("runner script parent should exist"),
        )
        .expect("runner script parent should be created");
        fs::write(&runner_script, "export {};\n").expect("runner script should be written");

        let previous = std::env::var_os("TOUCH_BROWSER_RESOURCE_ROOT");
        std::env::set_var("TOUCH_BROWSER_RESOURCE_ROOT", &runtime_root);

        assert_eq!(
            default_embedding_runner_script(),
            Some(canonical_or_raw(runner_script))
        );

        restore_env("TOUCH_BROWSER_RESOURCE_ROOT", previous);
    }

    #[test]
    fn nli_runner_script_prefers_explicit_runtime_root() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_root = temporary_directory("nli-runtime");
        let runner_script = runtime_root.join("scripts/evidence-nli-runner.mjs");
        fs::create_dir_all(
            runner_script
                .parent()
                .expect("runner script parent should exist"),
        )
        .expect("runner script parent should be created");
        fs::write(&runner_script, "export {};\n").expect("runner script should be written");

        let previous = std::env::var_os("TOUCH_BROWSER_RESOURCE_ROOT");
        std::env::set_var("TOUCH_BROWSER_RESOURCE_ROOT", &runtime_root);

        assert_eq!(
            default_nli_runner_script(),
            Some(canonical_or_raw(runner_script))
        );

        restore_env("TOUCH_BROWSER_RESOURCE_ROOT", previous);
    }

    #[test]
    fn runner_node_executable_prefers_bundled_runtime_node() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_root = temporary_directory("bundled-node");
        let bundled_node = runtime_root.join("node/bin/node");
        fs::create_dir_all(
            bundled_node
                .parent()
                .expect("bundled node parent should exist"),
        )
        .expect("bundled node parent should be created");
        fs::write(&bundled_node, "#!/bin/sh\n").expect("bundled node should be written");

        let previous_runtime_root = std::env::var_os("TOUCH_BROWSER_RESOURCE_ROOT");
        let previous_node = std::env::var_os("TOUCH_BROWSER_NODE_EXECUTABLE");
        std::env::set_var("TOUCH_BROWSER_RESOURCE_ROOT", &runtime_root);
        std::env::remove_var("TOUCH_BROWSER_NODE_EXECUTABLE");

        assert_eq!(
            runner_node_executable(),
            canonical_or_raw(bundled_node).display().to_string()
        );

        restore_env("TOUCH_BROWSER_RESOURCE_ROOT", previous_runtime_root);
        restore_env("TOUCH_BROWSER_NODE_EXECUTABLE", previous_node);
    }

    #[test]
    fn runner_working_directory_prefers_explicit_runtime_root() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let runtime_root = temporary_directory("runner-working-root");
        let previous = std::env::var_os("TOUCH_BROWSER_RESOURCE_ROOT");
        std::env::set_var("TOUCH_BROWSER_RESOURCE_ROOT", &runtime_root);

        assert_eq!(
            repo_root_for_runner(),
            canonical_or_raw(runtime_root.clone())
        );

        restore_env("TOUCH_BROWSER_RESOURCE_ROOT", previous);
    }

    #[test]
    fn semantic_reranking_boosts_more_similar_candidate() {
        let temp_dir = temporary_directory("embedding-rerank");
        let model_root = temp_dir.join("model-root");
        fs::create_dir_all(&model_root).expect("model root should exist");
        let script_path = temp_dir.join("mock-embedding-runner.mjs");
        fs::write(
            &script_path,
            r#"
let body = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => (body += chunk));
process.stdin.on('end', () => {
  const request = JSON.parse(body);
  const embeddings = request.texts.map((text, index) => {
    if (index === 0) return [1, 0, 0];
    if (text.includes('documentation')) return [0.98, 0.01, 0];
    return [0.2, 0.9, 0];
  });
  process.stdout.write(JSON.stringify({ embeddings }));
});
"#,
        )
        .expect("mock runner should write");

        let runner_command = format!("node {}", script_path.display());

        let mut candidates = vec![
            scored_candidate("b1", "domains are maintained for documentation", 0.34, 0.18),
            scored_candidate("b2", "tokio spawns asynchronous tasks", 0.35, 0.18),
        ];
        let embeddings = run_embedding_batch_with_backend(
            &embedding_request_texts("도메인은 문서화 목적으로 유지된다", &candidates, 2),
            &model_root,
            &runner_command,
            "test-embedding-model",
        )
        .expect("embedding batch should succeed");
        assert_eq!(embeddings.len(), 3);
        let claim_vector = &embeddings[0];
        for (candidate, vector) in candidates.iter_mut().zip(embeddings.iter().skip(1)) {
            let semantic_similarity =
                cosine_similarity(claim_vector, vector).expect("cosine similarity");
            let semantic_bonus =
                semantic_similarity_bonus(semantic_similarity, candidate.lexical_overlap);
            candidate.score = (candidate.score + semantic_bonus).min(1.0);
        }
        candidates.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        assert_eq!(candidates[0].block.id, "b1");
        assert!(candidates[0].score > candidates[1].score);
    }

    #[test]
    fn nli_reranking_boosts_strong_entailment_candidates() {
        let mut candidate = scored_candidate(
            "b1",
            "domains are maintained for documentation purposes",
            0.41,
            0.41,
        );

        apply_nli_reranking(
            &mut candidate,
            &NliScore {
                contradiction: 0.04,
                entailment: 0.88,
                neutral: 0.08,
            },
        );

        assert!(candidate.score > 0.5);
        assert!(candidate.exact_support);
        assert!(!candidate.contradictory);
    }

    #[test]
    fn nli_reranking_marks_strong_contradiction_candidates() {
        let mut candidate = scored_candidate("b1", "the default value is 3 seconds", 0.52, 0.30);

        apply_nli_reranking(
            &mut candidate,
            &NliScore {
                contradiction: 0.97,
                entailment: 0.01,
                neutral: 0.02,
            },
        );

        assert!(candidate.contradictory);
    }

    #[test]
    fn run_embedding_batch_reads_json_response_from_mock_runner() {
        let temp_dir = temporary_directory("embedding-runner");
        let model_root = temp_dir.join("model-root");
        fs::create_dir_all(&model_root).expect("model root should exist");
        let script_path = temp_dir.join("mock-embedding-runner.mjs");
        fs::write(
            &script_path,
            r#"
let body = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => (body += chunk));
process.stdin.on('end', () => {
  const request = JSON.parse(body);
  process.stdout.write(JSON.stringify({
    embeddings: request.texts.map((_text, index) => [index, 1, 0])
  }));
});
"#,
        )
        .expect("mock runner should write");

        let results = run_embedding_batch_with_backend(
            &["query: one".to_string(), "passage: two".to_string()],
            &model_root,
            &format!("node {}", script_path.display()),
            "test-embedding-model",
        )
        .expect("embedding batch should succeed");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], vec![0.0, 1.0, 0.0]);
        assert_eq!(results[1], vec![1.0, 1.0, 0.0]);
    }

    #[test]
    fn run_nli_batch_reads_json_response_from_mock_runner() {
        let temp_dir = temporary_directory("nli-runner");
        let model_root = temp_dir.join("model-root");
        fs::create_dir_all(&model_root).expect("model root should exist");
        let script_path = temp_dir.join("mock-runner.mjs");
        fs::write(
            &script_path,
            r#"
let body = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => (body += chunk));
process.stdin.on('end', () => {
  const request = JSON.parse(body);
  process.stdout.write(JSON.stringify({
    results: request.pairs.map((_pair, index) => ({
      contradiction: index === 0 ? 0.96 : 0.02,
      entailment: index === 1 ? 0.88 : 0.01,
      neutral: 0.03
    }))
  }));
});
"#,
        )
        .expect("mock runner should write");

        let results = run_nli_batch_with_backend(
            "claim text",
            &["first".to_string(), "second".to_string()],
            &model_root,
            &format!("node {}", script_path.display()),
            "test-nli-model",
        )
        .expect("nli batch should succeed");

        assert_eq!(results.len(), 2);
        assert!(results[0].contradiction > 0.9);
        assert!(results[1].entailment > 0.8);
    }

    fn temporary_directory(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("touch-browser-{name}-{nonce}"));
        fs::create_dir_all(&directory).expect("temp dir should exist");
        directory
    }

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    fn scored_candidate<'a>(
        id: &str,
        text: &str,
        score: f64,
        lexical_overlap: f64,
    ) -> ScoredCandidate<'a> {
        let block = Box::leak(Box::new(SnapshotBlock {
            version: "1.0.0".to_string(),
            id: id.to_string(),
            kind: SnapshotBlockKind::Text,
            stable_ref: format!("ref:{id}"),
            role: SnapshotBlockRole::Content,
            text: text.to_string(),
            attributes: Default::default(),
            evidence: SnapshotEvidence {
                source_url: "https://example.com".to_string(),
                source_type: SourceType::Http,
                dom_path_hint: Some("html > body > main > p".to_string()),
                byte_range_start: None,
                byte_range_end: None,
            },
        }));

        ScoredCandidate {
            block,
            candidate_index: 0,
            text: text.to_string(),
            score,
            lexical_overlap,
            contradictory: false,
            exact_support: false,
            signals: CandidateMatchSignals {
                lexical_overlap,
                contextual_overlap: lexical_overlap,
                numeric_alignment: None,
                exact_support: false,
                semantic_similarity: None,
                semantic_boost: None,
                nli_entailment: None,
                nli_contradiction: None,
            },
        }
    }
}

use std::{
    env,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};

use fasttext::FastText;
use serde::{Deserialize, Serialize};

use crate::scoring::ScoredCandidate;

const DEFAULT_NLI_MODEL_ID: &str = "Xenova/nli-deberta-v3-xsmall";
const NLI_RERANK_LIMIT: usize = 5;
const STRONG_NLI_ENTAILMENT: f64 = 0.82;
const STRONG_NLI_CONTRADICTION: f64 = 0.94;
const STRONG_NLI_MARGIN: f64 = 0.20;

#[derive(Default)]
pub(crate) struct SemanticScoringContext {
    claim_vector: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub(crate) struct NliScore {
    pub(crate) contradiction: f64,
    pub(crate) entailment: f64,
    pub(crate) neutral: f64,
}

pub(crate) fn build_semantic_scoring_context(claim_text: &str) -> SemanticScoringContext {
    let claim_vector = active_backend()
        .and_then(|backend| backend.sentence_vector(claim_text).ok())
        .filter(|vector| !vector_is_zero(vector));

    SemanticScoringContext { claim_vector }
}

pub(crate) fn semantic_similarity(
    context: &SemanticScoringContext,
    candidate_text: &str,
) -> Option<f64> {
    let claim_vector = context.claim_vector.as_ref()?;
    let backend = active_backend()?;
    let candidate_vector = backend
        .sentence_vector(candidate_text)
        .ok()
        .filter(|vector| !vector_is_zero(vector))?;

    cosine_similarity(claim_vector, &candidate_vector)
}

pub(crate) fn rerank_candidates_with_nli(claim_text: &str, candidates: &mut [ScoredCandidate<'_>]) {
    let rerank_count = candidates.len().min(NLI_RERANK_LIMIT);
    if rerank_count == 0 {
        return;
    }

    let Some(model_root) = resolved_nli_model_root() else {
        return;
    };
    let candidate_texts = candidates
        .iter()
        .take(rerank_count)
        .map(|candidate| candidate.text.clone())
        .collect::<Vec<_>>();
    let Some(results) = run_nli_batch(claim_text, &candidate_texts, &model_root) else {
        return;
    };

    for (candidate, nli) in candidates.iter_mut().take(rerank_count).zip(results.iter()) {
        apply_nli_reranking(candidate, nli);
    }

    candidates.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn active_backend() -> Option<&'static FastTextBackend> {
    match backend_state() {
        SemanticBackendState::FastText(backend) => Some(backend),
        SemanticBackendState::Disabled | SemanticBackendState::Unavailable => None,
    }
}

fn backend_state() -> &'static SemanticBackendState {
    static BACKEND: OnceLock<SemanticBackendState> = OnceLock::new();

    BACKEND.get_or_init(load_backend_from_env)
}

fn load_backend_from_env() -> SemanticBackendState {
    if let Some(model_path) = env::var_os("TOUCH_BROWSER_EVIDENCE_FASTTEXT_MODEL_PATH") {
        let model_path = PathBuf::from(model_path);
        if !model_path.is_file() {
            return SemanticBackendState::Unavailable;
        }

        return match FastTextBackend::load(&model_path) {
            Ok(backend) => SemanticBackendState::FastText(backend),
            Err(_) => SemanticBackendState::Unavailable,
        };
    }

    let Some(model_path) = default_model_candidates()
        .into_iter()
        .find(|candidate| candidate.is_file())
    else {
        return SemanticBackendState::Disabled;
    };

    match FastTextBackend::load(&model_path) {
        Ok(backend) => SemanticBackendState::FastText(backend),
        Err(_) => SemanticBackendState::Unavailable,
    }
}

fn default_model_candidates() -> Vec<PathBuf> {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };

    default_model_candidates_from_home(&home)
}

fn default_model_candidates_from_home(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".touch-browser/models/evidence/fasttext/cc.en.300.bin"),
        home.join(".touch-browser/models/evidence/fasttext.bin"),
    ]
}

fn resolved_nli_model_root() -> Option<PathBuf> {
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

    default_nli_runner_script().map(|path| format!("node {}", shell_escape(&path)))
}

fn default_nli_runner_script() -> Option<PathBuf> {
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

fn run_nli_batch(
    claim_text: &str,
    candidate_texts: &[String],
    model_root: &Path,
) -> Option<Vec<NliScore>> {
    let runner_command = nli_runner_command()?;
    let request = NliBatchRequest {
        model_id: nli_model_id(),
        pairs: candidate_texts
            .iter()
            .map(|candidate_text| NliPairRequest {
                premise: candidate_text.clone(),
                hypothesis: claim_text.to_string(),
            })
            .collect(),
    };
    let request_body = serde_json::to_vec(&request).ok()?;

    let mut child = Command::new("sh")
        .args(["-lc", &runner_command])
        .current_dir(repo_root_for_runner())
        .env("TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH", model_root)
        .env("TOUCH_BROWSER_EVIDENCE_NLI_MODEL_ID", &request.model_id)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;

    {
        let stdin = child.stdin.as_mut()?;
        stdin.write_all(&request_body).ok()?;
    }
    let _ = child.stdin.take();

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }

    let response: NliBatchResponse = serde_json::from_slice(&output.stdout).ok()?;
    Some(response.results)
}

fn apply_nli_reranking(candidate: &mut ScoredCandidate<'_>, nli: &NliScore) {
    let contradiction_margin = nli.contradiction - nli.entailment.max(nli.neutral);
    if nli.contradiction >= STRONG_NLI_CONTRADICTION
        && contradiction_margin >= STRONG_NLI_MARGIN
        && candidate.score >= 0.40
    {
        candidate.contradictory = true;
        candidate.score = (candidate.score + 0.05).min(1.0);
        return;
    }

    let entailment_margin = nli.entailment - nli.contradiction.max(nli.neutral);
    if nli.entailment >= STRONG_NLI_ENTAILMENT && entailment_margin >= STRONG_NLI_MARGIN {
        candidate.score = (candidate.score + 0.18).min(1.0);
        candidate.exact_support = true;
    }
}

fn shell_escape(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\"'\"'"))
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

enum SemanticBackendState {
    Disabled,
    Unavailable,
    FastText(FastTextBackend),
}

struct FastTextBackend {
    model: FastText,
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

impl FastTextBackend {
    fn load(model_path: &Path) -> Result<Self, String> {
        let Some(model_path) = model_path.to_str() else {
            return Err(format!(
                "fastText model path `{}` is not valid UTF-8.",
                model_path.display()
            ));
        };

        let mut model = FastText::new();
        model.load_model(model_path)?;
        Ok(Self { model })
    }

    fn sentence_vector(&self, text: &str) -> Result<Vec<f32>, String> {
        self.model.get_sentence_vector(text)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use fasttext::{Args, FastText, ModelName};
    use touch_browser_contracts::{
        SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotEvidence, SourceType,
    };

    use super::{
        apply_nli_reranking, cosine_similarity, default_model_candidates_from_home, run_nli_batch,
        vector_is_zero, FastTextBackend, NliScore,
    };
    use crate::scoring::ScoredCandidate;

    #[test]
    fn cosine_similarity_requires_non_zero_vectors() {
        assert_eq!(cosine_similarity(&[], &[]), None);
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]), None);
        assert!(vector_is_zero(&[0.0, 0.0]));
        assert!(!vector_is_zero(&[0.0, 0.2]));
    }

    #[test]
    fn fasttext_backend_scores_same_domain_paraphrases_similarly() {
        let model = train_test_model(
            "domains-docs",
            &[
                "domains are maintained for documentation purposes docs kept for examples",
                "documentation examples use reserved domains for docs",
                "reserved domains are kept for documentation and examples",
                "domains kept for docs are useful in documentation",
            ],
        );
        let backend = FastTextBackend { model };

        let paraphrase_similarity = cosine_similarity(
            &backend
                .sentence_vector("domains are kept for docs")
                .expect("claim vector"),
            &backend
                .sentence_vector("domains are maintained for documentation purposes")
                .expect("support vector"),
        )
        .expect("cosine similarity");

        let unrelated_similarity = cosine_similarity(
            &backend
                .sentence_vector("domains are kept for docs")
                .expect("claim vector"),
            &backend
                .sentence_vector("tokio runtime spawns many tasks")
                .expect("support vector"),
        )
        .expect("cosine similarity");

        assert!(
            paraphrase_similarity > unrelated_similarity,
            "expected paraphrase similarity {} to exceed unrelated similarity {}",
            paraphrase_similarity,
            unrelated_similarity
        );
    }

    #[test]
    fn default_model_candidates_resolve_under_touch_browser_home() {
        let home = Path::new("/tmp/touch-browser-home");
        let candidates = default_model_candidates_from_home(home);

        assert_eq!(
            candidates,
            vec![
                PathBuf::from(
                    "/tmp/touch-browser-home/.touch-browser/models/evidence/fasttext/cc.en.300.bin"
                ),
                PathBuf::from(
                    "/tmp/touch-browser-home/.touch-browser/models/evidence/fasttext.bin"
                ),
            ]
        );
    }

    #[test]
    fn nli_reranking_boosts_strong_entailment_candidates() {
        let mut candidate = scored_candidate(
            "b1",
            "domains are maintained for documentation purposes",
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
        let mut candidate = scored_candidate("b1", "the default value is 3 seconds", 0.52);

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

        let previous_runner = env::var("TOUCH_BROWSER_EVIDENCE_NLI_RUNNER").ok();
        env::set_var(
            "TOUCH_BROWSER_EVIDENCE_NLI_RUNNER",
            format!("node {}", script_path.display()),
        );

        let results = run_nli_batch(
            "claim text",
            &["first".to_string(), "second".to_string()],
            &model_root,
        )
        .expect("nli batch should succeed");

        if let Some(previous_runner) = previous_runner {
            env::set_var("TOUCH_BROWSER_EVIDENCE_NLI_RUNNER", previous_runner);
        } else {
            env::remove_var("TOUCH_BROWSER_EVIDENCE_NLI_RUNNER");
        }

        assert_eq!(results.len(), 2);
        assert!(results[0].contradiction > 0.9);
        assert!(results[1].entailment > 0.8);
    }

    fn train_test_model(name: &str, corpus_lines: &[&str]) -> FastText {
        let temp_dir = temporary_directory(name);
        let corpus_path = temp_dir.join("corpus.txt");
        let output_base = temp_dir.join("model");
        fs::write(&corpus_path, corpus_lines.join("\n")).expect("test corpus should write");

        let mut args = Args::new();
        args.set_input(corpus_path.to_str().expect("corpus path utf8"))
            .expect("set input");
        args.set_output(output_base.to_str().expect("output path utf8"))
            .expect("set output");
        args.set_model(ModelName::SG);
        args.set_epoch(80);
        args.set_lr(0.15);
        args.set_dim(32);
        args.set_thread(1);
        args.set_min_count(1);
        args.set_word_ngrams(2);
        args.set_bucket(10_000);
        args.set_minn(2);
        args.set_maxn(4);
        args.set_verbose(0);

        let mut model = FastText::new();
        model
            .train(&args)
            .expect("fastText test model should train");
        model
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

    fn scored_candidate<'a>(id: &str, text: &str, score: f64) -> ScoredCandidate<'a> {
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
            contradictory: false,
            exact_support: false,
        }
    }
}

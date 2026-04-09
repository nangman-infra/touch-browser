use std::{
    env,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use fasttext::FastText;

#[derive(Default)]
pub(crate) struct SemanticScoringContext {
    claim_vector: Option<Vec<f32>>,
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
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use fasttext::{Args, FastText, ModelName};

    use super::{
        cosine_similarity, default_model_candidates_from_home, vector_is_zero, FastTextBackend,
    };

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
}

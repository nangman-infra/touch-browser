use std::{
    env,
    path::{Path, PathBuf},
};

use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

pub(crate) fn slot_timestamp(slot: usize, seconds: usize) -> String {
    let hour = slot / 60;
    let minute = slot % 60;
    format!("2026-03-14T{hour:02}:{minute:02}:{seconds:02}+09:00")
}

pub(crate) fn current_timestamp() -> String {
    let now = OffsetDateTime::now_utc();
    if let Ok(offset) = UtcOffset::current_local_offset() {
        if let Ok(local) = now.to_offset(offset).format(&Rfc3339) {
            return local;
        }
    }

    now.format(&Rfc3339)
        .expect("utc RFC3339 timestamp should format")
}

pub(crate) fn is_fixture_target(target: &str) -> bool {
    target.starts_with("fixture://")
}

pub(crate) fn repo_root() -> PathBuf {
    if let Some(explicit_root) =
        env::var_os("TOUCH_BROWSER_REPO_ROOT").filter(|value| !value.is_empty())
    {
        return canonical_or_raw(PathBuf::from(explicit_root));
    }

    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    if manifest_root.exists() {
        return canonical_or_raw(manifest_root);
    }

    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub(crate) fn resource_root() -> PathBuf {
    resource_root_from(None, bundled_resource_root(), repo_root())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn is_bundled_runtime() -> bool {
    bundled_resource_root().is_some()
}

pub(crate) fn data_root() -> PathBuf {
    if let Some(explicit_root) =
        env::var_os("TOUCH_BROWSER_DATA_ROOT").filter(|value| !value.is_empty())
    {
        return canonical_or_raw(PathBuf::from(explicit_root));
    }

    data_root_from(
        None,
        bundled_resource_root().is_some(),
        env::var_os("HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from),
        repo_root(),
    )
}

pub(crate) fn node_executable() -> String {
    if let Some(explicit_node) =
        env::var_os("TOUCH_BROWSER_NODE_EXECUTABLE").filter(|value| !value.is_empty())
    {
        return PathBuf::from(explicit_node).display().to_string();
    }

    node_executable_from(None, &resource_root())
}

fn resource_root_from(
    explicit_root: Option<PathBuf>,
    bundled_root: Option<PathBuf>,
    fallback_repo_root: PathBuf,
) -> PathBuf {
    explicit_root
        .or(bundled_root)
        .map(canonical_or_raw)
        .unwrap_or(fallback_repo_root)
}

fn data_root_from(
    explicit_root: Option<PathBuf>,
    bundled_runtime: bool,
    home_root: Option<PathBuf>,
    fallback_repo_root: PathBuf,
) -> PathBuf {
    if let Some(explicit_root) = explicit_root {
        return canonical_or_raw(explicit_root);
    }

    if bundled_runtime {
        if let Some(home_root) = home_root {
            return canonical_or_raw(home_root.join(".touch-browser"));
        }
    }

    fallback_repo_root.join("output")
}

fn node_executable_from(explicit_node: Option<PathBuf>, resource_root: &Path) -> String {
    if let Some(explicit_node) = explicit_node {
        return explicit_node.display().to_string();
    }

    let bundled_node = resource_root.join("node/bin/node");
    if bundled_node.is_file() {
        return bundled_node.display().to_string();
    }

    "node".to_string()
}

fn canonical_or_raw(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn bundled_resource_root() -> Option<PathBuf> {
    if let Some(explicit_root) =
        env::var_os("TOUCH_BROWSER_RESOURCE_ROOT").filter(|value| !value.is_empty())
    {
        return Some(canonical_or_raw(PathBuf::from(explicit_root)));
    }

    if let Ok(current_exe) = env::current_exe() {
        if let Some(bundle_root) = current_exe.parent().and_then(Path::parent) {
            let runtime_dir = bundle_root.join("runtime");
            if runtime_dir.exists() {
                return Some(canonical_or_raw(runtime_dir));
            }
        }

        if let Some(exe_dir) = current_exe.parent() {
            let runtime_dir = exe_dir.join("runtime");
            if runtime_dir.exists() {
                return Some(canonical_or_raw(runtime_dir));
            }
        }
    }

    None
}

pub(crate) fn usage() -> String {
    [
        "Usage:",
        "  Stable research commands:",
        "  touch-browser search <query> [--engine google|brave] [--headed] [--profile-dir <path>] [--budget <tokens>] [--session-file <path>]",
        "  touch-browser search-open-result --rank <number> [--prefer-official] [--engine google|brave] [--session-file <path>] [--headed]",
        "  touch-browser search-open-top [--limit <count>] [--engine google|brave] [--session-file <path>] [--headed]",
        "  touch-browser update [--check] [--version <tag>]",
        "  touch-browser uninstall [--purge-data] [--purge-all] --yes",
        "  touch-browser open <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser snapshot <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser compact-view <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser read-view <target> [--browser] [--headed] [--main-only] [--budget <tokens>] [--session-file <path>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser extract <target> --claim <statement> [--claim <statement> ...] [--verifier-command <shell-command>] [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser policy <target> [--browser] [--headed] [--budget <tokens>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "    Target commands use HTTP first by default and automatically retry with browser rendering when the page looks JS-dependent. Use --browser to force browser-backed capture.",
        "  touch-browser session-snapshot --session-file <path>",
        "  touch-browser session-compact --session-file <path>",
        "  touch-browser session-extract [--session-file <path>] [--engine google|brave] --claim <statement> [--claim <statement> ...] [--verifier-command <shell-command>]",
        "  touch-browser session-read --session-file <path> [--main-only]",
        "  touch-browser session-synthesize --session-file <path> [--note-limit <count>] [--format json|markdown]",
        "  touch-browser follow --session-file <path> --ref <stable-ref> [--headed]",
        "  touch-browser paginate --session-file <path> --direction next|prev [--headed]",
        "  touch-browser expand --session-file <path> --ref <stable-ref> [--headed]",
        "  touch-browser browser-replay --session-file <path>",
        "  touch-browser session-close --session-file <path>",
        "  touch-browser telemetry-summary",
        "  touch-browser telemetry-recent [--limit <count>]",
        "  touch-browser replay <scenario-name>",
        "  touch-browser memory-summary [--steps <even-number>]",
        "  touch-browser serve",
        "  Experimental supervised commands:",
        "  touch-browser refresh --session-file <path> [--headed]",
        "  touch-browser checkpoint --session-file <path>",
        "  touch-browser session-policy --session-file <path>",
        "  touch-browser session-profile --session-file <path>",
        "  touch-browser set-profile --session-file <path> --profile research-read-only|research-restricted|interactive-review|interactive-supervised-auth|interactive-supervised-write",
        "  touch-browser approve --session-file <path> --risk challenge|mfa|auth|high-risk-write [--risk ...]",
        "  touch-browser click --session-file <path> --ref <stable-ref> [--headed] [--ack-risk challenge|mfa|auth|high-risk-write ...]",
        "  touch-browser type --session-file <path> --ref <stable-ref> --value <text> [--headed] [--sensitive] [--ack-risk challenge|mfa|auth ...]",
        "  touch-browser submit --session-file <path> --ref <stable-ref> [--headed] [--ack-risk challenge|mfa|auth|high-risk-write ...]",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        canonical_or_raw, data_root, data_root_from, is_bundled_runtime, node_executable,
        node_executable_from, resource_root, resource_root_from,
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temporary_directory(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("touch-browser-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temporary directory should exist");
        path
    }

    #[test]
    fn resource_root_prefers_explicit_env_override() {
        let temp_dir = temporary_directory("resource-root");
        assert_eq!(
            resource_root_from(Some(temp_dir.clone()), None, PathBuf::from("/repo")),
            canonical_or_raw(temp_dir)
        );
    }

    #[test]
    fn node_executable_prefers_bundled_node_under_resource_root() {
        let temp_dir = temporary_directory("node-runtime");
        let bundled_node = temp_dir.join("node/bin/node");
        fs::create_dir_all(
            bundled_node
                .parent()
                .expect("bundled node parent should exist"),
        )
        .expect("bundled node parent should be created");
        fs::write(&bundled_node, "#!/bin/sh\n").expect("bundled node placeholder should exist");

        assert_eq!(
            PathBuf::from(node_executable_from(None, &temp_dir))
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(node_executable_from(None, &temp_dir)))
                .display()
                .to_string(),
            canonical_or_raw(bundled_node).display().to_string()
        );
    }

    #[test]
    fn data_root_prefers_explicit_env_override() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp_dir = temporary_directory("data-root");
        let previous = std::env::var_os("TOUCH_BROWSER_DATA_ROOT");
        std::env::set_var("TOUCH_BROWSER_DATA_ROOT", &temp_dir);

        assert_eq!(data_root(), canonical_or_raw(temp_dir.clone()));

        restore_env("TOUCH_BROWSER_DATA_ROOT", previous);
    }

    #[test]
    fn bundled_runtime_flag_is_false_without_bundle_env() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var_os("TOUCH_BROWSER_RESOURCE_ROOT");
        std::env::remove_var("TOUCH_BROWSER_RESOURCE_ROOT");

        assert!(!is_bundled_runtime());

        restore_env("TOUCH_BROWSER_RESOURCE_ROOT", previous);
    }

    #[test]
    fn data_root_uses_touch_browser_home_for_bundled_runtime() {
        let home_root = temporary_directory("bundled-home");

        assert_eq!(
            data_root_from(None, true, Some(home_root.clone()), PathBuf::from("/repo")),
            canonical_or_raw(home_root.join(".touch-browser"))
        );
    }

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    #[allow(dead_code)]
    fn _path_exists(path: &Path) -> bool {
        path.exists()
    }
}

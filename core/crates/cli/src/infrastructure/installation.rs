use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::interface::{
    cli_error::CliError,
    cli_support::{current_timestamp, data_root},
};

#[allow(dead_code)]
const DEFAULT_RELEASE_REPOSITORY: &str = "nangman-infra/touch-browser";
const DEFAULT_RELEASES_API_ROOT: &str = "https://api.github.com/repos/nangman-infra/touch-browser";
#[allow(dead_code)]
const MANAGED_INSTALL_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedInstallManifest {
    pub(crate) schema_version: u8,
    pub(crate) repository: String,
    pub(crate) version: String,
    pub(crate) platform: String,
    pub(crate) arch: String,
    pub(crate) bundle_name: String,
    pub(crate) data_root: String,
    pub(crate) install_root: String,
    pub(crate) managed_bundle_root: String,
    pub(crate) current_symlink: String,
    pub(crate) command_link: String,
    pub(crate) installed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReleaseTarget {
    pub(crate) version: String,
    pub(crate) html_url: String,
    pub(crate) tarball_asset: ReleaseAsset,
    pub(crate) checksum_asset: ReleaseAsset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReleaseAsset {
    pub(crate) name: String,
    pub(crate) download_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UpdateInstallResult {
    pub(crate) manifest: ManagedInstallManifest,
    pub(crate) release: ReleaseTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UninstallResult {
    pub(crate) removed_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct BundleIdentity {
    pub(crate) bundle_name: String,
    pub(crate) version: String,
    pub(crate) platform: String,
    pub(crate) arch: String,
}

#[derive(Debug, Deserialize)]
struct GitHubReleaseResponse {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
}

pub(crate) fn managed_install_root_from(resolved_data_root: &Path) -> PathBuf {
    resolved_data_root.join("install")
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn managed_versions_root_from(resolved_data_root: &Path) -> PathBuf {
    managed_install_root_from(resolved_data_root).join("versions")
}

pub(crate) fn managed_staging_root_from(resolved_data_root: &Path) -> PathBuf {
    managed_install_root_from(resolved_data_root).join("staging")
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn managed_current_symlink_from(resolved_data_root: &Path) -> PathBuf {
    managed_install_root_from(resolved_data_root).join("current")
}

pub(crate) fn install_manifest_path() -> PathBuf {
    install_manifest_path_from(&data_root())
}

pub(crate) fn install_manifest_path_from(resolved_data_root: &Path) -> PathBuf {
    managed_install_root_from(resolved_data_root).join("install-manifest.json")
}

pub(crate) fn shell_manifest_path_from(resolved_data_root: &Path) -> PathBuf {
    managed_install_root_from(resolved_data_root).join("install-manifest.env")
}

pub(crate) fn load_install_manifest() -> Result<ManagedInstallManifest, CliError> {
    load_install_manifest_from(&install_manifest_path())
}

pub(crate) fn load_install_manifest_from(path: &Path) -> Result<ManagedInstallManifest, CliError> {
    let raw = fs::read_to_string(path).map_err(|source| CliError::IoPath {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| CliError::JsonPath {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn require_managed_install_manifest() -> Result<ManagedInstallManifest, CliError> {
    load_install_manifest().map_err(|error| match error {
        CliError::IoPath { .. } => CliError::Usage(
            "Managed standalone install metadata was not found. Install a standalone bundle with install.sh first.".to_string(),
        ),
        other => other,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_bundle_name(bundle_name: &str) -> Result<BundleIdentity, CliError> {
    let Some(stripped) = bundle_name.strip_prefix("touch-browser-") else {
        return Err(CliError::Usage(format!(
            "Unsupported bundle name `{bundle_name}`."
        )));
    };

    let (rest, arch) = split_once_from_end(stripped, '-').ok_or_else(|| {
        CliError::Usage(format!(
            "Could not determine architecture from `{bundle_name}`."
        ))
    })?;
    let (version, platform) = split_once_from_end(rest, '-').ok_or_else(|| {
        CliError::Usage(format!(
            "Could not determine platform from `{bundle_name}`."
        ))
    })?;

    if version.trim().is_empty() || platform.trim().is_empty() || arch.trim().is_empty() {
        return Err(CliError::Usage(format!(
            "Bundle name `{bundle_name}` is incomplete."
        )));
    }

    Ok(BundleIdentity {
        bundle_name: bundle_name.to_string(),
        version: version.to_string(),
        platform: platform.to_string(),
        arch: arch.to_string(),
    })
}

pub(crate) fn normalize_requested_version(version: &str) -> String {
    if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

pub(crate) fn fetch_release_target(
    current_install: &ManagedInstallManifest,
    requested_version: Option<&str>,
) -> Result<ReleaseTarget, CliError> {
    let endpoint = match requested_version {
        Some(version) => format!(
            "{}/releases/tags/{}",
            release_api_root(),
            normalize_requested_version(version)
        ),
        None => format!("{}/releases/latest", release_api_root()),
    };
    let response = github_client()?.get(&endpoint).send()?.error_for_status()?;
    let response = serde_json::from_str::<GitHubReleaseResponse>(&response.text()?)?;

    release_target_from_response(&response, &current_install.platform, &current_install.arch)
}

fn release_target_from_response(
    release: &GitHubReleaseResponse,
    platform: &str,
    arch: &str,
) -> Result<ReleaseTarget, CliError> {
    let expected_prefix = format!("touch-browser-{}-{platform}-{arch}", release.tag_name);
    let tarball_name = format!("{expected_prefix}.tar.gz");
    let checksum_name = format!("{expected_prefix}.sha256");

    let tarball_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == tarball_name)
        .map(|asset| ReleaseAsset {
            name: asset.name.clone(),
            download_url: asset.browser_download_url.clone(),
        })
        .ok_or_else(|| {
            CliError::Usage(format!(
                "Release {} does not contain asset `{tarball_name}`.",
                release.tag_name
            ))
        })?;
    let checksum_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == checksum_name)
        .map(|asset| ReleaseAsset {
            name: asset.name.clone(),
            download_url: asset.browser_download_url.clone(),
        })
        .ok_or_else(|| {
            CliError::Usage(format!(
                "Release {} does not contain asset `{checksum_name}`.",
                release.tag_name
            ))
        })?;

    Ok(ReleaseTarget {
        version: release.tag_name.clone(),
        html_url: release.html_url.clone(),
        tarball_asset,
        checksum_asset,
    })
}

pub(crate) fn install_release(
    current_install: &ManagedInstallManifest,
    release: &ReleaseTarget,
) -> Result<UpdateInstallResult, CliError> {
    let data_root_path = PathBuf::from(&current_install.data_root);
    let staging_root = managed_staging_root_from(&data_root_path).join(format!(
        "{}-{}",
        release.version,
        current_timestamp().replace(':', "-")
    ));
    fs::create_dir_all(&staging_root)?;

    let tarball_path = staging_root.join(&release.tarball_asset.name);
    let checksum_path = staging_root.join(&release.checksum_asset.name);

    download_to_path(&release.tarball_asset.download_url, &tarball_path)?;
    download_to_path(&release.checksum_asset.download_url, &checksum_path)?;
    verify_checksum(&tarball_path, &checksum_path)?;

    extract_tarball_into(&tarball_path, &staging_root)?;
    let bundle_name = bundle_name_from_asset(&release.tarball_asset.name)?;
    let bundle_root = staging_root.join(&bundle_name);
    if !bundle_root.is_dir() {
        return Err(CliError::Usage(format!(
            "Expected extracted bundle at {}.",
            bundle_root.display()
        )));
    }

    let command_link = PathBuf::from(&current_install.command_link);
    run_install_script(&bundle_root, &data_root_path, command_link.parent())?;

    let updated_manifest =
        load_install_manifest_from(&install_manifest_path_from(&data_root_path))?;
    let _ = fs::remove_dir_all(&staging_root);

    Ok(UpdateInstallResult {
        manifest: updated_manifest,
        release: release.clone(),
    })
}

pub(crate) fn uninstall_managed_install(
    current_install: &ManagedInstallManifest,
    purge_data: bool,
    purge_all: bool,
) -> Result<UninstallResult, CliError> {
    let data_root_path = PathBuf::from(&current_install.data_root);
    let command_link = PathBuf::from(&current_install.command_link);
    let install_root = PathBuf::from(&current_install.install_root);

    let mut removed_paths = Vec::new();
    remove_path_if_exists(&command_link, &mut removed_paths)?;
    remove_path_if_exists(&install_root, &mut removed_paths)?;

    if purge_data {
        remove_path_if_exists(&data_root_path.join("browser-search"), &mut removed_paths)?;
        remove_path_if_exists(&data_root_path.join("pilot"), &mut removed_paths)?;
    }
    if purge_all {
        remove_path_if_exists(&data_root_path.join("models"), &mut removed_paths)?;
    }

    remove_path_if_exists(
        &shell_manifest_path_from(&data_root_path),
        &mut removed_paths,
    )?;
    remove_empty_directories_up_to(&data_root_path, data_root_path.parent());

    Ok(UninstallResult { removed_paths })
}

pub(crate) fn github_client() -> Result<Client, CliError> {
    Client::builder()
        .user_agent(format!(
            "touch-browser-updater/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(CliError::from)
}

pub(crate) fn release_api_root() -> String {
    std::env::var("TOUCH_BROWSER_UPDATE_API_ROOT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_RELEASES_API_ROOT.to_string())
}

fn download_to_path(url: &str, destination: &Path) -> Result<(), CliError> {
    let response = github_client()?
        .get(url)
        .send()?
        .error_for_status()?
        .bytes()?;
    fs::write(destination, response.as_ref()).map_err(|source| CliError::IoPath {
        path: destination.display().to_string(),
        source,
    })?;
    Ok(())
}

fn verify_checksum(tarball_path: &Path, checksum_path: &Path) -> Result<(), CliError> {
    let expected = parse_checksum_file(checksum_path)?;
    let actual = file_sha256(tarball_path)?;
    if expected != actual {
        return Err(CliError::Usage(format!(
            "Checksum verification failed for {}.",
            tarball_path.display()
        )));
    }
    Ok(())
}

fn parse_checksum_file(checksum_path: &Path) -> Result<String, CliError> {
    let raw = fs::read_to_string(checksum_path).map_err(|source| CliError::IoPath {
        path: checksum_path.display().to_string(),
        source,
    })?;
    raw.split_whitespace()
        .next()
        .map(ToString::to_string)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CliError::Usage(format!(
                "Checksum file {} is empty.",
                checksum_path.display()
            ))
        })
}

fn file_sha256(path: &Path) -> Result<String, CliError> {
    let bytes = fs::read(path).map_err(|source| CliError::IoPath {
        path: path.display().to_string(),
        source,
    })?;
    let mut digest = Sha256::new();
    digest.update(bytes);
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn extract_tarball_into(tarball_path: &Path, destination: &Path) -> Result<(), CliError> {
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(tarball_path)
        .arg("-C")
        .arg(destination)
        .status()?;
    if !status.success() {
        return Err(CliError::Usage(format!(
            "Failed to extract release asset {}.",
            tarball_path.display()
        )));
    }
    Ok(())
}

fn run_install_script(
    bundle_root: &Path,
    data_root_path: &Path,
    install_dir: Option<&Path>,
) -> Result<(), CliError> {
    let install_script = bundle_root.join("install.sh");
    if !install_script.is_file() {
        return Err(CliError::Usage(format!(
            "Standalone bundle is missing install.sh at {}.",
            install_script.display()
        )));
    }

    let mut command = Command::new(&install_script);
    command.env("TOUCH_BROWSER_DATA_ROOT", data_root_path);
    if let Some(install_dir) = install_dir {
        command.env("TOUCH_BROWSER_INSTALL_DIR", install_dir);
    }
    command.arg(bundle_root);

    let status = command.status()?;
    if !status.success() {
        return Err(CliError::Usage(format!(
            "Install script failed for bundle {}.",
            bundle_root.display()
        )));
    }
    Ok(())
}

fn bundle_name_from_asset(asset_name: &str) -> Result<String, CliError> {
    asset_name
        .strip_suffix(".tar.gz")
        .map(ToString::to_string)
        .ok_or_else(|| CliError::Usage(format!("Unsupported asset name `{asset_name}`.")))
}

fn remove_path_if_exists(path: &Path, removed_paths: &mut Vec<String>) -> Result<(), CliError> {
    if !path.exists() {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(path).map_err(|source| CliError::IoPath {
        path: path.display().to_string(),
        source,
    })?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path).map_err(|source| CliError::IoPath {
            path: path.display().to_string(),
            source,
        })?;
    } else {
        fs::remove_file(path).map_err(|source| CliError::IoPath {
            path: path.display().to_string(),
            source,
        })?;
    }
    removed_paths.push(path.display().to_string());
    Ok(())
}

fn remove_empty_directories_up_to(start: &Path, stop: Option<&Path>) {
    let mut current = Some(start.to_path_buf());
    while let Some(path) = current {
        if stop.is_some_and(|stop| stop == path) {
            break;
        }
        match fs::read_dir(&path) {
            Ok(entries) => {
                if entries.count() != 0 {
                    break;
                }
                let parent = path.parent().map(Path::to_path_buf);
                let _ = fs::remove_dir(&path);
                current = parent;
            }
            _ => break,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn split_once_from_end(value: &str, delimiter: char) -> Option<(&str, &str)> {
    let index = value.rfind(delimiter)?;
    Some((&value[..index], &value[index + 1..]))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        install_manifest_path_from, load_install_manifest_from, managed_current_symlink_from,
        managed_install_root_from, managed_staging_root_from, managed_versions_root_from,
        normalize_requested_version, parse_bundle_name, release_target_from_response,
        uninstall_managed_install, ManagedInstallManifest,
    };

    fn temporary_directory(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("touch-browser-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temporary directory should exist");
        path
    }

    fn sample_manifest(data_root: &Path, command_link: &Path) -> ManagedInstallManifest {
        ManagedInstallManifest {
            schema_version: 1,
            repository: "nangman-infra/touch-browser".to_string(),
            version: "v0.1.1".to_string(),
            platform: "macos".to_string(),
            arch: "arm64".to_string(),
            bundle_name: "touch-browser-v0.1.1-macos-arm64".to_string(),
            data_root: data_root.display().to_string(),
            install_root: data_root.join("install").display().to_string(),
            managed_bundle_root: data_root
                .join("install/versions/touch-browser-v0.1.1-macos-arm64")
                .display()
                .to_string(),
            current_symlink: data_root.join("install/current").display().to_string(),
            command_link: command_link.display().to_string(),
            installed_at: "2026-04-11T00:00:00+09:00".to_string(),
        }
    }

    #[test]
    fn managed_install_paths_live_under_data_root() {
        let data_root = temporary_directory("managed-install");
        assert_eq!(
            managed_install_root_from(&data_root),
            data_root.join("install")
        );
        assert_eq!(
            managed_versions_root_from(&data_root),
            data_root.join("install/versions")
        );
        assert_eq!(
            managed_staging_root_from(&data_root),
            data_root.join("install/staging")
        );
        assert_eq!(
            managed_current_symlink_from(&data_root),
            data_root.join("install/current")
        );
        assert_eq!(
            install_manifest_path_from(&data_root),
            data_root.join("install/install-manifest.json")
        );
    }

    #[test]
    fn bundle_name_parser_keeps_hyphenated_versions() {
        let parsed = parse_bundle_name("touch-browser-release-first-local-macos-arm64")
            .expect("bundle name should parse");
        assert_eq!(parsed.version, "release-first-local");
        assert_eq!(parsed.platform, "macos");
        assert_eq!(parsed.arch, "arm64");
    }

    #[test]
    fn install_manifest_round_trips_from_json() {
        let data_root = temporary_directory("install-manifest");
        let manifest_path = install_manifest_path_from(&data_root);
        let manifest = sample_manifest(&data_root, Path::new("/opt/homebrew/bin/touch-browser"));
        fs::create_dir_all(
            manifest_path
                .parent()
                .expect("manifest parent should exist"),
        )
        .expect("manifest parent should be created");
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("manifest should serialize"),
        )
        .expect("manifest should write");

        assert_eq!(
            load_install_manifest_from(&manifest_path).expect("manifest should reload"),
            manifest
        );
    }

    #[test]
    fn normalize_requested_version_adds_prefix_only_when_missing() {
        assert_eq!(normalize_requested_version("0.1.2"), "v0.1.2");
        assert_eq!(normalize_requested_version("v0.1.2"), "v0.1.2");
    }

    #[test]
    fn release_target_selector_uses_platform_specific_assets() {
        let response = serde_json::from_value::<super::GitHubReleaseResponse>(serde_json::json!({
            "tag_name": "v0.1.2",
            "html_url": "https://example.test/releases/v0.1.2",
            "assets": [
                {
                    "name": "touch-browser-v0.1.2-macos-arm64.tar.gz",
                    "browser_download_url": "https://example.test/macos.tar.gz"
                },
                {
                    "name": "touch-browser-v0.1.2-macos-arm64.sha256",
                    "browser_download_url": "https://example.test/macos.sha256"
                },
                {
                    "name": "touch-browser-v0.1.2-linux-x86_64.tar.gz",
                    "browser_download_url": "https://example.test/linux.tar.gz"
                }
            ]
        }))
        .expect("release response should parse");

        let release = release_target_from_response(&response, "macos", "arm64")
            .expect("release should resolve");
        assert_eq!(release.version, "v0.1.2");
        assert_eq!(
            release.tarball_asset.name,
            "touch-browser-v0.1.2-macos-arm64.tar.gz"
        );
        assert_eq!(
            release.checksum_asset.name,
            "touch-browser-v0.1.2-macos-arm64.sha256"
        );
    }

    #[test]
    fn uninstall_without_purge_keeps_user_data_directories() {
        let data_root = temporary_directory("managed-uninstall-keep");
        let command_root = temporary_directory("managed-uninstall-command");
        let command_link = command_root.join("touch-browser");
        let manifest = sample_manifest(&data_root, &command_link);

        fs::create_dir_all(data_root.join("install/versions")).expect("install root should exist");
        fs::create_dir_all(data_root.join("browser-search")).expect("search data should exist");
        fs::create_dir_all(data_root.join("pilot")).expect("pilot data should exist");
        fs::create_dir_all(data_root.join("models")).expect("model data should exist");
        fs::write(data_root.join("install/versions/current.txt"), "bundle")
            .expect("install marker should write");
        fs::write(&command_link, "shim").expect("command shim should write");
        fs::write(data_root.join("browser-search/state.json"), "{}")
            .expect("search state should write");
        fs::write(data_root.join("pilot/telemetry.sqlite"), "sqlite")
            .expect("pilot db should write");
        fs::write(data_root.join("models/model.bin"), "model").expect("model cache should write");

        let result =
            uninstall_managed_install(&manifest, false, false).expect("uninstall should succeed");

        assert_eq!(result.removed_paths.len(), 2);
        assert!(result
            .removed_paths
            .contains(&command_link.display().to_string()));
        assert!(result
            .removed_paths
            .contains(&data_root.join("install").display().to_string()));
        assert!(!command_link.exists());
        assert!(!data_root.join("install").exists());
        assert!(data_root.join("browser-search").exists());
        assert!(data_root.join("pilot").exists());
        assert!(data_root.join("models").exists());
    }

    #[test]
    fn uninstall_with_purge_all_removes_install_and_all_managed_data() {
        let data_root = temporary_directory("managed-uninstall-purge");
        let command_root = temporary_directory("managed-uninstall-command-purge");
        let command_link = command_root.join("touch-browser");
        let manifest = sample_manifest(&data_root, &command_link);

        fs::create_dir_all(data_root.join("install/versions")).expect("install root should exist");
        fs::create_dir_all(data_root.join("browser-search")).expect("search data should exist");
        fs::create_dir_all(data_root.join("pilot")).expect("pilot data should exist");
        fs::create_dir_all(data_root.join("models")).expect("model data should exist");
        fs::write(data_root.join("install/versions/current.txt"), "bundle")
            .expect("install marker should write");
        fs::write(&command_link, "shim").expect("command shim should write");
        fs::write(data_root.join("browser-search/state.json"), "{}")
            .expect("search state should write");
        fs::write(data_root.join("pilot/telemetry.sqlite"), "sqlite")
            .expect("pilot db should write");
        fs::write(data_root.join("models/model.bin"), "model").expect("model cache should write");

        let result =
            uninstall_managed_install(&manifest, true, true).expect("purge should succeed");

        assert!(result
            .removed_paths
            .contains(&command_link.display().to_string()));
        assert!(result
            .removed_paths
            .contains(&data_root.join("install").display().to_string()));
        assert!(result
            .removed_paths
            .contains(&data_root.join("browser-search").display().to_string()));
        assert!(result
            .removed_paths
            .contains(&data_root.join("pilot").display().to_string()));
        assert!(result
            .removed_paths
            .contains(&data_root.join("models").display().to_string()));
        assert!(!command_link.exists());
        assert!(!data_root.exists());
    }
}

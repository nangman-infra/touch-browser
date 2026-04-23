use std::{
    collections::HashMap,
    env, fs,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tungstenite::{connect, Message};

use crate::{
    application::ports::{BrowserDownloadEvidence, BrowserSnapshotReference},
    infrastructure::browser_models::{
        PlaywrightClickParams, PlaywrightClickResult, PlaywrightDownloadEvidence,
        PlaywrightExpandParams, PlaywrightExpandResult, PlaywrightFollowParams,
        PlaywrightFollowResult, PlaywrightLoadDiagnostics, PlaywrightPaginateParams,
        PlaywrightPaginateResult, PlaywrightSnapshotParams, PlaywrightSnapshotResult,
        PlaywrightSubmitParams, PlaywrightSubmitResult, PlaywrightTypeParams, PlaywrightTypeResult,
    },
    interface::cli_error::CliError,
};

const DEVTOOLS_TIMEOUT: Duration = Duration::from_secs(10);
const READY_TIMEOUT: Duration = Duration::from_secs(8);
const ACTION_SETTLE: Duration = Duration::from_millis(650);
const POST_LOAD_SETTLE_MIN: Duration = Duration::from_millis(500);
const POST_LOAD_SETTLE_TIMEOUT: Duration = Duration::from_millis(1_200);
const POST_LOAD_SETTLE_STABLE: Duration = Duration::from_millis(250);
const POST_LOAD_SETTLE_POLL: Duration = Duration::from_millis(100);
const DOWNLOAD_WAIT_TIMEOUT: Duration = Duration::from_millis(2_500);
const SEARCH_PROFILE_MARKER: &str = ".touch-browser-search-profile.json";
const SEARCH_MANUAL_RECOVERY_TIMEOUT: Duration = Duration::from_secs(300);
const SEARCH_MANUAL_RECOVERY_POLL: Duration = Duration::from_millis(750);
const SEARCH_BROWSER_FALLBACK_VERSION: &str = "146.0.0.0";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DevtoolsTarget {
    web_socket_debugger_url: String,
}

#[derive(Debug)]
struct CdpPageState {
    final_url: String,
    title: String,
    visible_text: String,
    html: String,
    html_length: usize,
    link_count: usize,
    button_count: usize,
    input_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpFrameDescriptor {
    id: String,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CdpFrameTreeNode {
    frame: CdpFrameDescriptor,
    #[serde(default)]
    child_frames: Vec<CdpFrameTreeNode>,
}

#[derive(Debug)]
struct CdpActionDetails {
    target_text: String,
    target_href: Option<String>,
    clicked_text: Option<String>,
    download: Option<BrowserDownloadEvidence>,
    typed_length: Option<usize>,
    page: Option<usize>,
}

#[derive(Debug)]
struct CdpFrameState {
    frame_id: String,
    final_url: String,
    title: String,
    visible_text: String,
    html: String,
    link_count: usize,
    button_count: usize,
    input_count: usize,
}

#[derive(Debug)]
pub(crate) enum CdpActionKind {
    Follow,
    Click,
    Type {
        value: String,
    },
    Submit {
        prefill: Vec<CdpTypePrefill>,
    },
    Paginate {
        direction: String,
        current_page: usize,
    },
    Expand,
}

#[derive(Debug)]
pub(crate) struct CdpTypePrefill {
    pub(crate) target: BrowserSnapshotReference,
    pub(crate) value: String,
}

pub(crate) fn cdp_adapter_enabled() -> bool {
    cdp_adapter_name_enabled(env::var("TOUCH_BROWSER_BROWSER_ADAPTER").ok().as_deref())
}

fn cdp_adapter_name_enabled(value: Option<&str>) -> bool {
    value
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "cdp-rust" | "rust-cdp" | "cdp"
            )
        })
        .unwrap_or(false)
}

pub(crate) fn invoke_cdp_snapshot(
    params: PlaywrightSnapshotParams,
) -> Result<PlaywrightSnapshotResult, CliError> {
    let mut browser = CdpBrowser::launch(
        params
            .context_dir
            .as_deref()
            .or(params.profile_dir.as_deref()),
        params.headless,
        params.search_identity,
    )?;
    browser.load(params.url.as_deref(), params.html.as_deref())?;
    browser.maybe_await_manual_search_recovery(params.search_identity, params.manual_recovery)?;
    let state = browser.capture_state()?;
    Ok(PlaywrightSnapshotResult {
        status: "ok".to_string(),
        mode: "cdp-rust-browser".to_string(),
        source: params.url.unwrap_or_else(|| "inline-html".to_string()),
        final_url: state.final_url,
        title: state.title,
        visible_text: state.visible_text,
        html: state.html,
        html_length: state.html_length,
        link_count: state.link_count,
        button_count: state.button_count,
        input_count: state.input_count,
        diagnostics: cdp_load_diagnostics("cdp-rust-load"),
    })
}

pub(crate) fn invoke_cdp_follow(
    params: PlaywrightFollowParams,
) -> Result<PlaywrightFollowResult, CliError> {
    let details = run_cdp_action(
        params.url.as_deref(),
        params.html.as_deref(),
        params
            .context_dir
            .as_deref()
            .or(params.profile_dir.as_deref()),
        params.headless,
        cdp_target_from_ref(
            params.target_ref.clone(),
            params.target_text.clone(),
            params.target_href.clone(),
            params.target_tag_name.clone(),
            params.target_dom_path_hint.clone(),
            params.target_ordinal_hint,
            None,
            None,
        ),
        CdpActionKind::Follow,
    )?;
    Ok(PlaywrightFollowResult {
        status: "ok".to_string(),
        method: "browser.follow".to_string(),
        limited_dynamic_action: true,
        followed_ref: params.target_ref,
        target_text: details.action.target_text,
        target_href: details.action.target_href,
        clicked_text: details
            .action
            .clicked_text
            .unwrap_or_else(|| params.target_text.clone()),
        final_url: details.state.final_url,
        title: details.state.title,
        visible_text: details.state.visible_text,
        html: details.state.html,
        diagnostics: cdp_load_diagnostics("cdp-rust-action"),
    })
}

pub(crate) fn invoke_cdp_click(
    params: PlaywrightClickParams,
) -> Result<PlaywrightClickResult, CliError> {
    let details = run_cdp_action(
        params.url.as_deref(),
        params.html.as_deref(),
        params
            .context_dir
            .as_deref()
            .or(params.profile_dir.as_deref()),
        params.headless,
        cdp_target_from_ref(
            params.target_ref.clone(),
            params.target_text.clone(),
            params.target_href.clone(),
            params.target_tag_name.clone(),
            params.target_dom_path_hint.clone(),
            params.target_ordinal_hint,
            None,
            None,
        ),
        CdpActionKind::Click,
    )?;
    Ok(PlaywrightClickResult {
        status: "ok".to_string(),
        method: "browser.click".to_string(),
        limited_dynamic_action: false,
        clicked_ref: params.target_ref,
        target_text: details.action.target_text,
        target_href: details.action.target_href,
        clicked_text: details
            .action
            .clicked_text
            .unwrap_or_else(|| params.target_text.clone()),
        download: details.action.download.map(playwright_download_evidence),
        final_url: details.state.final_url,
        title: details.state.title,
        visible_text: details.state.visible_text,
        html: details.state.html,
        diagnostics: cdp_load_diagnostics("cdp-rust-action"),
    })
}

pub(crate) fn invoke_cdp_type(
    params: PlaywrightTypeParams,
) -> Result<PlaywrightTypeResult, CliError> {
    let value = params.value.clone();
    let details = run_cdp_action(
        params.url.as_deref(),
        params.html.as_deref(),
        params
            .context_dir
            .as_deref()
            .or(params.profile_dir.as_deref()),
        params.headless,
        cdp_target_from_ref(
            params.target_ref.clone(),
            params.target_text.clone(),
            None,
            params.target_tag_name.clone(),
            params.target_dom_path_hint.clone(),
            params.target_ordinal_hint,
            params.target_name.clone(),
            params.target_input_type.clone(),
        ),
        CdpActionKind::Type { value },
    )?;
    Ok(PlaywrightTypeResult {
        status: "ok".to_string(),
        method: "browser.type".to_string(),
        limited_dynamic_action: false,
        typed_ref: params.target_ref,
        target_text: details.action.target_text,
        typed_length: details.action.typed_length.unwrap_or(params.value.len()),
        final_url: details.state.final_url,
        title: details.state.title,
        visible_text: details.state.visible_text,
        html: details.state.html,
        diagnostics: cdp_load_diagnostics("cdp-rust-action"),
    })
}

pub(crate) fn invoke_cdp_submit(
    params: PlaywrightSubmitParams,
) -> Result<PlaywrightSubmitResult, CliError> {
    let prefill = params
        .prefill
        .into_iter()
        .map(|prefill| CdpTypePrefill {
            target: cdp_target_from_ref(
                prefill.target_ref,
                prefill.target_text.unwrap_or_default(),
                None,
                prefill.target_tag_name,
                prefill.target_dom_path_hint,
                prefill.target_ordinal_hint,
                prefill.target_name,
                prefill.target_input_type,
            ),
            value: prefill.value,
        })
        .collect();
    let details = run_cdp_action(
        params.url.as_deref(),
        params.html.as_deref(),
        params
            .context_dir
            .as_deref()
            .or(params.profile_dir.as_deref()),
        params.headless,
        cdp_target_from_ref(
            params.target_ref.clone(),
            params.target_text.clone(),
            None,
            params.target_tag_name.clone(),
            params.target_dom_path_hint.clone(),
            params.target_ordinal_hint,
            None,
            None,
        ),
        CdpActionKind::Submit { prefill },
    )?;
    Ok(PlaywrightSubmitResult {
        status: "ok".to_string(),
        method: "browser.submit".to_string(),
        limited_dynamic_action: false,
        submitted_ref: params.target_ref,
        target_text: details.action.target_text,
        final_url: details.state.final_url,
        title: details.state.title,
        visible_text: details.state.visible_text,
        html: details.state.html,
    })
}

pub(crate) fn invoke_cdp_paginate(
    params: PlaywrightPaginateParams,
) -> Result<PlaywrightPaginateResult, CliError> {
    let direction = params.direction.clone();
    let details = run_cdp_action(
        params.url.as_deref(),
        params.html.as_deref(),
        params
            .context_dir
            .as_deref()
            .or(params.profile_dir.as_deref()),
        params.headless,
        cdp_target_from_ref(
            format!("pagination:{direction}"),
            direction.clone(),
            None,
            None,
            None,
            None,
            None,
            None,
        ),
        CdpActionKind::Paginate {
            direction,
            current_page: params.current_page,
        },
    )?;
    Ok(PlaywrightPaginateResult {
        status: "ok".to_string(),
        method: "browser.paginate".to_string(),
        limited_dynamic_action: true,
        page: details.action.page.unwrap_or(params.current_page),
        clicked_text: details
            .action
            .clicked_text
            .unwrap_or_else(|| params.direction.clone()),
        final_url: details.state.final_url,
        title: details.state.title,
        visible_text: details.state.visible_text,
        html: details.state.html,
    })
}

pub(crate) fn invoke_cdp_expand(
    params: PlaywrightExpandParams,
) -> Result<PlaywrightExpandResult, CliError> {
    let details = run_cdp_action(
        params.url.as_deref(),
        params.html.as_deref(),
        params
            .context_dir
            .as_deref()
            .or(params.profile_dir.as_deref()),
        params.headless,
        cdp_target_from_ref(
            params.target_ref.clone(),
            params.target_text.clone(),
            None,
            params.target_tag_name.clone(),
            params.target_dom_path_hint.clone(),
            params.target_ordinal_hint,
            None,
            None,
        ),
        CdpActionKind::Expand,
    )?;
    Ok(PlaywrightExpandResult {
        status: "ok".to_string(),
        method: "browser.expand".to_string(),
        limited_dynamic_action: true,
        expanded_ref: params.target_ref,
        target_text: details.action.target_text,
        clicked_text: details
            .action
            .clicked_text
            .unwrap_or_else(|| params.target_text.clone()),
        final_url: details.state.final_url,
        title: details.state.title,
        visible_text: details.state.visible_text,
        html: details.state.html,
    })
}

struct CdpActionRun {
    action: CdpActionDetails,
    state: CdpPageState,
}

fn run_cdp_action(
    url: Option<&str>,
    html: Option<&str>,
    context_dir: Option<&str>,
    headless: bool,
    target: BrowserSnapshotReference,
    action: CdpActionKind,
) -> Result<CdpActionRun, CliError> {
    let search_identity = context_dir.is_some_and(has_search_identity_marker);
    let mut browser = CdpBrowser::launch(context_dir, headless, search_identity)?;
    browser.load(url, html)?;
    let known_downloads = browser.list_download_files()?;
    let details = browser.execute_action(&target, &action)?;
    thread::sleep(ACTION_SETTLE);
    browser.wait_until_ready()?;
    browser.wait_for_dynamic_settle()?;
    let download = browser.capture_new_download(&known_downloads)?;
    let state = browser.capture_state()?;
    Ok(CdpActionRun {
        action: CdpActionDetails {
            download,
            ..details
        },
        state,
    })
}

#[allow(clippy::too_many_arguments)]
fn cdp_target_from_ref(
    target_ref: String,
    text: String,
    href: Option<String>,
    tag_name: Option<String>,
    dom_path_hint: Option<String>,
    ordinal_hint: Option<usize>,
    name: Option<String>,
    input_type: Option<String>,
) -> BrowserSnapshotReference {
    BrowserSnapshotReference {
        target_ref,
        text,
        href,
        tag_name,
        dom_path_hint,
        ordinal_hint,
        name,
        input_type,
        sensitive: false,
    }
}

fn cdp_load_diagnostics(strategy: &str) -> PlaywrightLoadDiagnostics {
    PlaywrightLoadDiagnostics {
        wait_strategy: strategy.to_string(),
        wait_budget_ms: Some(READY_TIMEOUT.as_millis() as usize),
        wait_consumed_ms: None,
        wait_stop_reason: Some("document-ready".to_string()),
    }
}

struct CdpBrowser {
    child: Child,
    socket: tungstenite::WebSocket<tungstenite::stream::MaybeTlsStream<std::net::TcpStream>>,
    next_id: u64,
    frame_context_ids: HashMap<String, i64>,
    user_data_dir: PathBuf,
    cleanup_user_data_dir: bool,
    download_dir: Option<PathBuf>,
}

impl CdpBrowser {
    fn launch(
        context_dir: Option<&str>,
        headless: bool,
        search_identity: bool,
    ) -> Result<Self, CliError> {
        let executable = resolve_browser_executable()?;
        let (user_data_dir, cleanup_user_data_dir) = match context_dir {
            Some(dir) => (PathBuf::from(dir), false),
            None => (temporary_directory("touch-browser-cdp-profile")?, true),
        };
        fs::create_dir_all(&user_data_dir)?;
        if search_identity && context_dir.is_some() {
            write_search_identity_marker(&user_data_dir)?;
        }
        let _ = fs::remove_file(user_data_dir.join("DevToolsActivePort"));

        let mut command = Command::new(executable);
        command
            .arg("--remote-debugging-port=0")
            .arg(format!("--user-data-dir={}", user_data_dir.display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg("--disable-popup-blocking")
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if search_identity {
            command
                .arg("--disable-blink-features=AutomationControlled")
                .arg("--disable-dev-shm-usage")
                .arg(format!("--lang={}", search_locale()));
        }
        if headless {
            command.arg("--headless=new").arg("--disable-gpu");
        }

        let child = command.spawn().map_err(|source| {
            CliError::Adapter(format!(
                "failed to launch Chromium for CDP adapter: {source}"
            ))
        })?;
        let port = wait_for_devtools_port(&user_data_dir)?;
        let target = create_devtools_target(port)?;
        let (socket, _) = connect(target.web_socket_debugger_url.as_str()).map_err(|source| {
            CliError::Adapter(format!("CDP websocket connect failed: {source}"))
        })?;

        let mut browser = Self {
            child,
            socket,
            next_id: 1,
            frame_context_ids: HashMap::new(),
            user_data_dir,
            cleanup_user_data_dir,
            download_dir: None,
        };
        browser.call("Page.enable", json!({}))?;
        browser.call("Runtime.enable", json!({}))?;
        browser.configure_downloads()?;
        browser.install_dom_instrumentation()?;
        if search_identity {
            browser.install_search_identity()?;
        }
        Ok(browser)
    }

    fn load(&mut self, url: Option<&str>, html: Option<&str>) -> Result<(), CliError> {
        if let Some(html) = html {
            self.call("Page.navigate", json!({ "url": "about:blank" }))?;
            self.wait_until_ready()?;
            self.evaluate_void(&format!(
                "document.open();document.write({});document.close();",
                serde_json::to_string(html)?
            ))?;
            self.wait_until_ready()?;
            self.wait_for_dynamic_settle()?;
            return Ok(());
        }

        if let Some(url) = url {
            self.call("Page.navigate", json!({ "url": url }))?;
            self.wait_until_ready()?;
            self.wait_for_dynamic_settle()?;
            return Ok(());
        }

        self.wait_until_ready()?;
        self.wait_for_dynamic_settle()
    }

    fn wait_until_ready(&mut self) -> Result<(), CliError> {
        let started_at = Instant::now();
        while started_at.elapsed() < READY_TIMEOUT {
            let ready = self.evaluate_value("document.readyState")?;
            if matches!(ready.as_str(), Some("interactive" | "complete")) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err(CliError::Adapter(
            "CDP adapter timed out waiting for document readiness.".to_string(),
        ))
    }

    fn wait_for_dynamic_settle(&mut self) -> Result<(), CliError> {
        let started_at = Instant::now();
        let mut last_signature = self.page_signature()?;
        let mut changed_at = started_at;

        while started_at.elapsed() < POST_LOAD_SETTLE_TIMEOUT {
            thread::sleep(POST_LOAD_SETTLE_POLL);
            let signature = self.page_signature()?;
            if signature != last_signature {
                last_signature = signature;
                changed_at = Instant::now();
            }
            if started_at.elapsed() >= POST_LOAD_SETTLE_MIN
                && changed_at.elapsed() >= POST_LOAD_SETTLE_STABLE
            {
                return Ok(());
            }
        }

        Ok(())
    }

    fn page_signature(&mut self) -> Result<String, CliError> {
        let value = self.evaluate_value(
            r#"
(() => {
  const text = document.body ? String(document.body.innerText || document.body.textContent || "") : "";
  const html = document.documentElement ? document.documentElement.outerHTML : "";
  return `${text.length}:${html.length}:${text.slice(0, 128)}:${text.slice(-128)}`;
})()
"#,
        )?;
        Ok(value.as_str().unwrap_or_default().to_string())
    }

    fn maybe_await_manual_search_recovery(
        &mut self,
        search_identity: bool,
        manual_recovery: bool,
    ) -> Result<(), CliError> {
        if !search_identity || !manual_recovery {
            return Ok(());
        }
        if !self.looks_like_search_challenge()? {
            return Ok(());
        }

        let started_at = Instant::now();
        let timeout = search_manual_recovery_timeout();
        while started_at.elapsed() < timeout {
            thread::sleep(SEARCH_MANUAL_RECOVERY_POLL);
            let _ = self.wait_until_ready();
            if !self.looks_like_search_challenge()? {
                return Ok(());
            }
        }
        Ok(())
    }

    fn looks_like_search_challenge(&mut self) -> Result<bool, CliError> {
        let state = self.capture_state()?;
        Ok(looks_like_search_challenge(
            &state.final_url,
            &state.title,
            &state.visible_text,
        ))
    }

    fn configure_downloads(&mut self) -> Result<(), CliError> {
        let download_dir = temporary_directory("touch-browser-cdp-downloads")?;
        let download_path = download_dir.display().to_string();
        let params = json!({
            "behavior": "allow",
            "downloadPath": download_path,
            "eventsEnabled": true,
        });
        let _ = self.call("Browser.setDownloadBehavior", params.clone());
        let _ = self.call("Page.setDownloadBehavior", params);
        self.download_dir = Some(download_dir);
        Ok(())
    }

    fn install_search_identity(&mut self) -> Result<(), CliError> {
        let identity = search_identity_payload();
        self.call("Network.enable", json!({}))?;
        self.call(
            "Network.setUserAgentOverride",
            json!({
                "userAgent": &identity.user_agent,
                "acceptLanguage": identity.languages.join(","),
                "platform": &identity.navigator_platform,
                "userAgentMetadata": {
                    "brands": &identity.user_agent_brands,
                    "fullVersionList": &identity.user_agent_brands,
                    "platform": &identity.user_agent_data_platform,
                    "platformVersion": &identity.platform_version,
                    "architecture": &identity.architecture,
                    "model": "",
                    "mobile": false,
                    "bitness": &identity.bitness,
                    "wow64": false
                }
            }),
        )?;
        let timezone = search_timezone();
        if !timezone.is_empty() {
            let _ = self.call(
                "Emulation.setTimezoneOverride",
                json!({ "timezoneId": timezone }),
            );
        }
        self.call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": build_search_identity_script(&identity)? }),
        )?;
        Ok(())
    }

    fn install_dom_instrumentation(&mut self) -> Result<(), CliError> {
        self.call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": DOM_INSTRUMENTATION_SCRIPT }),
        )?;
        self.evaluate_void(DOM_INSTRUMENTATION_SCRIPT)?;
        Ok(())
    }

    fn capture_state(&mut self) -> Result<CdpPageState, CliError> {
        let frames = self.frame_descriptors()?;
        let mut states = Vec::new();
        if let Some(main_frame) = frames.first() {
            if let Ok(state) = self.capture_main_frame_state(main_frame) {
                states.push(state);
            }
        }
        for frame in frames.iter().skip(1) {
            if let Ok(state) = self.capture_frame_state(frame) {
                states.push(state);
            }
        }
        let Some(main_state) = states.first() else {
            return Err(CliError::Adapter(
                "CDP adapter could not capture any frame state.".to_string(),
            ));
        };
        let frame_suffix = states
            .iter()
            .skip(1)
            .enumerate()
            .map(|(index, state)| {
                format!(
                    r#"<section data-touch-browser-frame="{index}" data-touch-browser-frame-id="{}" data-touch-browser-frame-url="{}">{}</section>"#,
                    escape_html_attribute(&state.frame_id),
                    escape_html_attribute(&state.final_url),
                    state.html
                )
            })
            .collect::<Vec<_>>()
            .join("");
        let html = format!("{}{}", main_state.html, frame_suffix);
        Ok(CdpPageState {
            final_url: main_state.final_url.clone(),
            title: main_state.title.clone(),
            visible_text: normalize_browser_text(
                &states
                    .iter()
                    .map(|state| state.visible_text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            html_length: html.len(),
            html,
            link_count: states.iter().map(|state| state.link_count).sum(),
            button_count: states.iter().map(|state| state.button_count).sum(),
            input_count: states.iter().map(|state| state.input_count).sum(),
        })
    }

    fn execute_action(
        &mut self,
        target: &BrowserSnapshotReference,
        action: &CdpActionKind,
    ) -> Result<CdpActionDetails, CliError> {
        let payload = json!({
            "target": {
                "text": target.text,
                "href": target.href,
                "tagName": target.tag_name,
                "domPathHint": target.dom_path_hint,
                "ordinalHint": target.ordinal_hint,
                "name": target.name,
                "inputType": target.input_type,
            },
            "action": action_payload(action),
        });
        let frames = self.frame_descriptors()?;
        let probe_expression = action_expression(ACTION_PROBE_SCRIPT, &payload)?;
        let best_frame = select_best_action_frame(self, &frames, &probe_expression);
        let perform_expression = action_expression(ACTION_PERFORM_SCRIPT, &payload)?;
        let value = perform_action_in_best_frame(
            self,
            &frames,
            best_frame.as_ref(),
            &perform_expression,
            action,
            target,
        )?;
        Ok(CdpActionDetails {
            target_text: value
                .get("targetText")
                .and_then(Value::as_str)
                .unwrap_or(&target.text)
                .to_string(),
            target_href: value
                .get("targetHref")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            clicked_text: value
                .get("clickedText")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            download: None,
            typed_length: value
                .get("typedLength")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
            page: value
                .get("page")
                .and_then(Value::as_u64)
                .map(|value| value as usize),
        })
    }

    fn evaluate_void(&mut self, expression: &str) -> Result<(), CliError> {
        self.evaluate_value(expression).map(|_| ())
    }

    fn frame_descriptors(&mut self) -> Result<Vec<CdpFrameDescriptor>, CliError> {
        let value = self.call("Page.getFrameTree", json!({}))?;
        let tree: CdpFrameTreeNode = serde_json::from_value(
            value
                .get("frameTree")
                .cloned()
                .ok_or_else(|| CliError::Adapter("CDP frame tree is missing.".to_string()))?,
        )?;
        let mut frames = Vec::new();
        flatten_frame_tree(&tree, &mut frames);
        Ok(frames)
    }

    fn capture_frame_state(
        &mut self,
        frame: &CdpFrameDescriptor,
    ) -> Result<CdpFrameState, CliError> {
        let value = self.evaluate_frame_value(&frame.id, &format!("({FRAME_STATE_SCRIPT})()"))?;
        self.parse_frame_state(frame, value)
    }

    fn capture_main_frame_state(
        &mut self,
        frame: &CdpFrameDescriptor,
    ) -> Result<CdpFrameState, CliError> {
        let value = self.evaluate_value(&format!("({FRAME_STATE_SCRIPT})()"))?;
        self.parse_frame_state(frame, value)
    }

    fn parse_frame_state(
        &self,
        frame: &CdpFrameDescriptor,
        value: Value,
    ) -> Result<CdpFrameState, CliError> {
        let final_url = value
            .get("finalUrl")
            .and_then(Value::as_str)
            .unwrap_or(&frame.url)
            .to_string();
        Ok(CdpFrameState {
            frame_id: frame.id.clone(),
            final_url,
            title: value
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            visible_text: value
                .get("visibleText")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            html: value
                .get("html")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            link_count: value
                .get("linkCount")
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize,
            button_count: value
                .get("buttonCount")
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize,
            input_count: value
                .get("inputCount")
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize,
        })
    }

    fn list_download_files(&self) -> Result<Vec<PathBuf>, CliError> {
        let Some(download_dir) = self.download_dir.as_ref() else {
            return Ok(Vec::new());
        };
        let mut files = Vec::new();
        for entry in fs::read_dir(download_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            }
        }
        Ok(files)
    }

    fn capture_new_download(
        &self,
        known_files: &[PathBuf],
    ) -> Result<Option<BrowserDownloadEvidence>, CliError> {
        let deadline = Instant::now() + DOWNLOAD_WAIT_TIMEOUT;

        loop {
            for candidate in self.list_download_files()? {
                if known_files.iter().any(|known| known == &candidate) {
                    continue;
                }
                if candidate
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("crdownload"))
                {
                    continue;
                }

                return persist_download_evidence(&candidate);
            }

            if Instant::now() >= deadline {
                return Ok(None);
            }
            thread::sleep(POST_LOAD_SETTLE_POLL);
        }
    }

    fn evaluate_value(&mut self, expression: &str) -> Result<Value, CliError> {
        self.evaluate_with_context(expression, None)
    }

    fn evaluate_frame_value(
        &mut self,
        frame_id: &str,
        expression: &str,
    ) -> Result<Value, CliError> {
        let context_id = if let Some(context_id) = self.frame_context_ids.get(frame_id).copied() {
            context_id
        } else {
            self.call(
                "Page.createIsolatedWorld",
                json!({
                    "frameId": frame_id,
                    "worldName": "__touch_browser",
                    "grantUniveralAccess": true,
                }),
            )?
            .get("executionContextId")
            .and_then(Value::as_i64)
            .ok_or_else(|| {
                CliError::Adapter(format!(
                    "CDP isolated world did not provide an execution context for frame `{frame_id}`."
                ))
            })?
        };
        self.evaluate_with_context(expression, Some(context_id))
    }

    fn evaluate_with_context(
        &mut self,
        expression: &str,
        context_id: Option<i64>,
    ) -> Result<Value, CliError> {
        let mut params = json!({
            "expression": expression,
            "awaitPromise": true,
            "returnByValue": true,
        });
        if let Some(context_id) = context_id {
            params["contextId"] = json!(context_id);
        }
        let result = self.call("Runtime.evaluate", params)?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(CliError::Adapter(format!(
                "CDP runtime exception: {}",
                exception
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown exception")
            )));
        }
        Ok(result
            .get("result")
            .and_then(|result| result.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, CliError> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        self.socket
            .send(Message::Text(request.to_string().into()))
            .map_err(|source| CliError::Adapter(format!("CDP send failed: {source}")))?;

        loop {
            let message = self
                .socket
                .read()
                .map_err(|source| CliError::Adapter(format!("CDP read failed: {source}")))?;
            let text = match message {
                Message::Text(text) => text.to_string(),
                Message::Binary(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                _ => continue,
            };
            let response: Value = serde_json::from_str(&text)?;
            self.record_protocol_event(&response);
            if response.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = response.get("error") {
                return Err(CliError::Adapter(format!(
                    "CDP method {method} failed: {}",
                    error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown CDP error")
                )));
            }
            return Ok(response.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn record_protocol_event(&mut self, message: &Value) {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return;
        };
        match method {
            "Runtime.executionContextCreated" => {
                if let Some((frame_id, context_id)) = extract_default_execution_context(message) {
                    self.frame_context_ids.insert(frame_id, context_id);
                }
            }
            "Runtime.executionContextDestroyed" => {
                if let Some(context_id) = message
                    .get("params")
                    .and_then(|params| params.get("executionContextId"))
                    .and_then(Value::as_i64)
                {
                    self.frame_context_ids
                        .retain(|_, existing_context_id| *existing_context_id != context_id);
                }
            }
            "Runtime.executionContextsCleared" => {
                self.frame_context_ids.clear();
            }
            _ => {}
        }
    }
}

impl Drop for CdpBrowser {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if self.cleanup_user_data_dir {
            let _ = fs::remove_dir_all(&self.user_data_dir);
        }
        if let Some(download_dir) = self.download_dir.as_ref() {
            let _ = fs::remove_dir_all(download_dir);
        }
    }
}

fn persist_download_evidence(path: &Path) -> Result<Option<BrowserDownloadEvidence>, CliError> {
    if !path.is_file() {
        return Ok(None);
    }

    let suggested_filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download.bin")
        .to_string();
    let persisted_dir = temporary_directory("touch-browser-cdp-download-copy")?;
    let persisted_path = persisted_dir.join(&suggested_filename);
    fs::copy(path, &persisted_path)?;
    let metadata = fs::metadata(&persisted_path)?;

    Ok(Some(BrowserDownloadEvidence {
        completed: true,
        suggested_filename,
        path: Some(persisted_path.display().to_string()),
        byte_length: Some(metadata.len()),
        sha256: Some(sha256_file(&persisted_path)?),
        failure: None,
    }))
}

fn playwright_download_evidence(download: BrowserDownloadEvidence) -> PlaywrightDownloadEvidence {
    PlaywrightDownloadEvidence {
        completed: download.completed,
        suggested_filename: download.suggested_filename,
        path: download.path,
        byte_length: download.byte_length,
        sha256: download.sha256,
        failure: download.failure,
    }
}

fn sha256_file(path: &Path) -> Result<String, CliError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn action_payload(action: &CdpActionKind) -> Value {
    match action {
        CdpActionKind::Follow => json!({ "kind": "follow" }),
        CdpActionKind::Click => json!({ "kind": "click" }),
        CdpActionKind::Type { value } => json!({ "kind": "type", "value": value }),
        CdpActionKind::Submit { prefill } => json!({
            "kind": "submit",
            "prefill": prefill.iter().map(|entry| json!({
                "target": {
                    "text": entry.target.text,
                    "href": entry.target.href,
                    "tagName": entry.target.tag_name,
                    "domPathHint": entry.target.dom_path_hint,
                    "ordinalHint": entry.target.ordinal_hint,
                    "name": entry.target.name,
                    "inputType": entry.target.input_type,
                },
                "value": entry.value,
            })).collect::<Vec<_>>()
        }),
        CdpActionKind::Paginate {
            direction,
            current_page,
        } => json!({
            "kind": "paginate",
            "direction": direction,
            "currentPage": current_page,
        }),
        CdpActionKind::Expand => json!({ "kind": "expand" }),
    }
}

fn action_expression(script: &str, payload: &Value) -> Result<String, CliError> {
    Ok(format!(
        "({})(JSON.parse({}))",
        script,
        serde_json::to_string(&payload.to_string())?
    ))
}

fn action_probe_score(value: &Value) -> i64 {
    value
        .get("score")
        .and_then(Value::as_i64)
        .unwrap_or_default()
}

fn probe_action_frame(
    browser: &mut CdpBrowser,
    frame: &CdpFrameDescriptor,
    probe_expression: &str,
    main_frame_id: Option<&str>,
) -> Option<(CdpFrameDescriptor, i64)> {
    let value = if main_frame_id.is_some_and(|id| id == frame.id) {
        browser.evaluate_value(probe_expression).ok()?
    } else {
        browser
            .evaluate_frame_value(&frame.id, probe_expression)
            .ok()?
    };
    let score = action_probe_score(&value);
    (score > 0).then(|| (frame.clone(), score))
}

fn select_best_action_frame(
    browser: &mut CdpBrowser,
    frames: &[CdpFrameDescriptor],
    probe_expression: &str,
) -> Option<CdpFrameDescriptor> {
    let main_frame_id = frames.first().map(|frame| frame.id.as_str());
    let mut best_match: Option<(CdpFrameDescriptor, i64)> = None;

    for frame in frames {
        let Some(candidate) = probe_action_frame(browser, frame, probe_expression, main_frame_id)
        else {
            continue;
        };
        if best_match
            .as_ref()
            .is_none_or(|(_, current_score)| candidate.1 > *current_score)
        {
            best_match = Some(candidate);
        }
    }

    best_match.map(|(frame, _)| frame)
}

fn perform_action_in_best_frame(
    browser: &mut CdpBrowser,
    frames: &[CdpFrameDescriptor],
    best_frame: Option<&CdpFrameDescriptor>,
    perform_expression: &str,
    action: &CdpActionKind,
    target: &BrowserSnapshotReference,
) -> Result<Value, CliError> {
    let main_frame_id = frames.first().map(|frame| frame.id.as_str());
    if let Some(frame) = best_frame {
        if main_frame_id.is_some_and(|id| id == frame.id) {
            return browser.evaluate_value(perform_expression);
        }
        return browser.evaluate_frame_value(&frame.id, perform_expression);
    }
    if matches!(action, CdpActionKind::Follow) && target.href.is_some() {
        return browser.evaluate_value(perform_expression);
    }
    Err(CliError::Adapter(format!(
        "CDP action target was not found for `{}`.",
        target.text
    )))
}

#[derive(Debug)]
struct SearchIdentityPayload {
    languages: Vec<String>,
    user_agent: String,
    browser_version: String,
    user_agent_brands: Vec<Value>,
    navigator_platform: String,
    user_agent_data_platform: String,
    architecture: String,
    bitness: String,
    platform_version: String,
    web_gl_vendor: String,
    web_gl_renderer: String,
}

fn has_search_identity_marker(context_dir: &str) -> bool {
    Path::new(context_dir).join(SEARCH_PROFILE_MARKER).is_file()
}

fn write_search_identity_marker(context_dir: &Path) -> Result<(), CliError> {
    fs::write(
        context_dir.join(SEARCH_PROFILE_MARKER),
        serde_json::to_string_pretty(&json!({ "profile": "search", "version": 1 }))?,
    )?;
    Ok(())
}

fn search_manual_recovery_timeout() -> Duration {
    env::var("TOUCH_BROWSER_SEARCH_MANUAL_RECOVERY_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or(SEARCH_MANUAL_RECOVERY_TIMEOUT)
}

fn looks_like_search_challenge(final_url: &str, title: &str, visible_text: &str) -> bool {
    let combined = format!(
        "{} {} {}",
        final_url.to_ascii_lowercase(),
        title.to_ascii_lowercase(),
        visible_text.to_ascii_lowercase()
    );
    let signals = [
        "captcha",
        "recaptcha",
        "confirm you're not a robot",
        "i'm not a robot",
        "unusual traffic",
        "traffic verification",
        "verify you are human",
        "verify you're human",
        "robot check",
        "human checkpoint",
        "drag the slider",
        "security check",
        "비정상적인 트래픽",
        "로봇이 아닙니다",
    ];
    combined.contains("/sorry/") || signals.iter().any(|signal| combined.contains(signal))
}

fn search_identity_payload() -> SearchIdentityPayload {
    let architecture = if env::consts::ARCH.starts_with("arm") {
        "arm".to_string()
    } else {
        "x86".to_string()
    };
    let bitness = if env::consts::ARCH.contains("64") || env::consts::ARCH == "arm64" {
        "64".to_string()
    } else {
        "32".to_string()
    };
    let platform = search_platform_profile(&architecture, &bitness);
    let browser_version = resolve_search_browser_version()
        .unwrap_or_else(|| SEARCH_BROWSER_FALLBACK_VERSION.to_string());
    let user_agent = env::var("TOUCH_BROWSER_SEARCH_USER_AGENT").unwrap_or_else(|_| {
        build_search_user_agent(&browser_version, &platform.user_agent_fragment)
    });
    let major_version = browser_version
        .split('.')
        .next()
        .unwrap_or("146")
        .to_string();
    SearchIdentityPayload {
        languages: search_languages(),
        user_agent,
        browser_version,
        user_agent_brands: vec![
            json!({ "brand": "Not=A?Brand", "version": "99" }),
            json!({ "brand": "Chromium", "version": major_version }),
            json!({ "brand": "Google Chrome", "version": major_version }),
        ],
        navigator_platform: platform.navigator_platform,
        user_agent_data_platform: platform.user_agent_data_platform,
        architecture,
        bitness,
        platform_version: platform.platform_version,
        web_gl_vendor: platform.web_gl_vendor,
        web_gl_renderer: platform.web_gl_renderer,
    }
}

#[derive(Debug)]
struct SearchPlatformProfile {
    navigator_platform: String,
    user_agent_fragment: String,
    user_agent_data_platform: String,
    platform_version: String,
    web_gl_vendor: String,
    web_gl_renderer: String,
}

fn search_platform_profile(architecture: &str, bitness: &str) -> SearchPlatformProfile {
    match env::consts::OS {
        "windows" => windows_search_platform_profile(architecture, bitness),
        "linux" => linux_search_platform_profile(architecture),
        _ => macos_search_platform_profile(architecture),
    }
}

fn windows_search_platform_profile(architecture: &str, bitness: &str) -> SearchPlatformProfile {
    SearchPlatformProfile {
        navigator_platform: "Win32".to_string(),
        user_agent_fragment: if architecture == "arm" {
            "Windows NT 10.0; Win64; ARM64".to_string()
        } else if bitness == "64" {
            "Windows NT 10.0; Win64; x64".to_string()
        } else {
            "Windows NT 10.0".to_string()
        },
        user_agent_data_platform: "Windows".to_string(),
        platform_version: "15.0.0".to_string(),
        web_gl_vendor: "Google Inc. (Microsoft)".to_string(),
        web_gl_renderer: if architecture == "arm" {
            "ANGLE (Qualcomm Adreno Direct3D11 vs_5_0 ps_5_0)".to_string()
        } else {
            "ANGLE (Intel, Intel(R) UHD Graphics Direct3D11 vs_5_0 ps_5_0)".to_string()
        },
    }
}

fn linux_search_platform_profile(architecture: &str) -> SearchPlatformProfile {
    SearchPlatformProfile {
        navigator_platform: if architecture == "arm" {
            "Linux armv8l".to_string()
        } else {
            "Linux x86_64".to_string()
        },
        user_agent_fragment: if architecture == "arm" {
            "X11; Linux aarch64".to_string()
        } else {
            "X11; Linux x86_64".to_string()
        },
        user_agent_data_platform: "Linux".to_string(),
        platform_version: "6.0.0".to_string(),
        web_gl_vendor: "Google Inc. (Linux)".to_string(),
        web_gl_renderer: if architecture == "arm" {
            "ANGLE (ARM Mali OpenGL ES)".to_string()
        } else {
            "ANGLE (Intel, Mesa Intel(R) Graphics OpenGL)".to_string()
        },
    }
}

fn macos_search_platform_profile(architecture: &str) -> SearchPlatformProfile {
    SearchPlatformProfile {
        navigator_platform: "MacIntel".to_string(),
        user_agent_fragment: if architecture == "arm" {
            "Macintosh; ARM Mac OS X 14_0_0".to_string()
        } else {
            "Macintosh; Intel Mac OS X 10_15_7".to_string()
        },
        user_agent_data_platform: "macOS".to_string(),
        platform_version: "14.0.0".to_string(),
        web_gl_vendor: "Intel Inc.".to_string(),
        web_gl_renderer: if architecture == "arm" {
            "Apple GPU".to_string()
        } else {
            "Intel Iris OpenGL Engine".to_string()
        },
    }
}

fn build_search_user_agent(version: &str, platform_fragment: &str) -> String {
    format!(
        "Mozilla/5.0 ({platform_fragment}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{version} Safari/537.36"
    )
}

fn resolve_search_browser_version() -> Option<String> {
    if let Ok(version) = env::var("TOUCH_BROWSER_SEARCH_CHROME_VERSION") {
        if !version.trim().is_empty() {
            return Some(version);
        }
    }
    let executable = resolve_browser_executable().ok()?;
    let output = Command::new(executable).arg("--version").output().ok()?;
    let combined = format!(
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    extract_chrome_version(&combined)
}

fn extract_chrome_version(raw: &str) -> Option<String> {
    raw.split_whitespace()
        .find(|part| part.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .map(|part| {
            part.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.')
                .to_string()
        })
        .filter(|part| !part.is_empty())
}

fn search_locale() -> String {
    env::var("TOUCH_BROWSER_SEARCH_LOCALE")
        .or_else(|_| env::var("LANG"))
        .ok()
        .map(|value| {
            value
                .trim_end_matches(".UTF-8")
                .trim_end_matches(".utf8")
                .replace('_', "-")
        })
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "en-US".to_string())
}

fn search_languages() -> Vec<String> {
    let locale = search_locale();
    let primary = locale.split('-').next().unwrap_or("en").to_string();
    let mut languages = vec![locale.clone()];
    for candidate in [primary, "en-US".to_string(), "en".to_string()] {
        if !languages.iter().any(|existing| existing == &candidate) {
            languages.push(candidate);
        }
    }
    languages
}

fn search_timezone() -> String {
    env::var("TOUCH_BROWSER_SEARCH_TIMEZONE").unwrap_or_default()
}

fn build_search_identity_script(identity: &SearchIdentityPayload) -> Result<String, CliError> {
    let payload = json!({
        "languages": &identity.languages,
        "userAgent": &identity.user_agent,
        "browserVersion": &identity.browser_version,
        "userAgentBrands": &identity.user_agent_brands,
        "navigatorPlatform": &identity.navigator_platform,
        "userAgentDataPlatform": &identity.user_agent_data_platform,
        "architecture": &identity.architecture,
        "bitness": &identity.bitness,
        "platformVersion": &identity.platform_version,
        "webGlVendor": &identity.web_gl_vendor,
        "webGlRenderer": &identity.web_gl_renderer,
    });
    Ok(format!(
        r#"
(() => {{
  const payload = {};
  const defineGetter = (target, key, value) => {{
    try {{ Object.defineProperty(target, key, {{ configurable: true, get: () => value }}); }} catch {{}}
  }};
  defineGetter(navigator, "webdriver", undefined);
  defineGetter(navigator, "userAgent", payload.userAgent);
  defineGetter(navigator, "language", payload.languages[0] || "en-US");
  defineGetter(navigator, "languages", payload.languages);
  defineGetter(navigator, "platform", payload.navigatorPlatform);
  defineGetter(navigator, "vendor", "Google Inc.");
  defineGetter(navigator, "plugins", [{{ name: "Chrome PDF Plugin" }}, {{ name: "Chrome PDF Viewer" }}]);
  if ("userAgentData" in navigator) {{
    const userAgentData = {{
      brands: payload.userAgentBrands,
      mobile: false,
      platform: payload.userAgentDataPlatform,
      getHighEntropyValues: async () => ({{
        brands: payload.userAgentBrands,
        fullVersionList: payload.userAgentBrands,
        mobile: false,
        platform: payload.userAgentDataPlatform,
        platformVersion: payload.platformVersion,
        architecture: payload.architecture,
        bitness: payload.bitness,
        model: "",
        uaFullVersion: payload.browserVersion
      }}),
      toJSON: () => ({{
        brands: payload.userAgentBrands,
        mobile: false,
        platform: payload.userAgentDataPlatform
      }})
    }};
    defineGetter(navigator, "userAgentData", userAgentData);
  }}
  window.chrome = window.chrome || {{ runtime: {{}} }};
  const patchWebGl = (prototype) => {{
    if (!prototype || !prototype.getParameter) return;
    const original = prototype.getParameter;
    prototype.getParameter = function(parameter) {{
      if (parameter === 37445) return payload.webGlVendor;
      if (parameter === 37446) return payload.webGlRenderer;
      return original.call(this, parameter);
    }};
  }};
  patchWebGl(window.WebGLRenderingContext && window.WebGLRenderingContext.prototype);
  patchWebGl(window.WebGL2RenderingContext && window.WebGL2RenderingContext.prototype);
  if (navigator.permissions && typeof navigator.permissions.query === "function") {{
    const originalQuery = navigator.permissions.query.bind(navigator.permissions);
    navigator.permissions.query = (parameters) => {{
      if (parameters && parameters.name === "notifications") {{
        return Promise.resolve({{
          name: "notifications",
          state: Notification.permission,
          onchange: null,
          addEventListener() {{}},
          removeEventListener() {{}},
          dispatchEvent() {{ return false; }}
        }});
      }}
      return originalQuery(parameters);
    }};
  }}
}})();
"#,
        serde_json::to_string(&payload)?
    ))
}

fn create_devtools_target(port: u16) -> Result<DevtoolsTarget, CliError> {
    let client = reqwest::blocking::Client::new();
    let url = format!("http://127.0.0.1:{port}/json/new?about:blank");
    let started_at = Instant::now();
    let mut last_error = None;
    while started_at.elapsed() < DEVTOOLS_TIMEOUT {
        match client
            .put(&url)
            .send()
            .and_then(|response| response.error_for_status())
        {
            Ok(response) => {
                let raw = response.text()?;
                return serde_json::from_str(&raw).map_err(CliError::Json);
            }
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    Err(CliError::Adapter(format!(
        "CDP adapter could not create a DevTools page target: {}",
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "timed out waiting for /json/new".to_string())
    )))
}

fn wait_for_devtools_port(user_data_dir: &Path) -> Result<u16, CliError> {
    let active_port_path = user_data_dir.join("DevToolsActivePort");
    let started_at = Instant::now();
    while started_at.elapsed() < DEVTOOLS_TIMEOUT {
        if let Ok(raw) = fs::read_to_string(&active_port_path) {
            if let Some(port) = raw.lines().next().and_then(|line| line.parse().ok()) {
                return Ok(port);
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(CliError::Adapter(format!(
        "CDP adapter could not find DevToolsActivePort at {}",
        active_port_path.display()
    )))
}

fn resolve_browser_executable() -> Result<PathBuf, CliError> {
    for key in [
        "TOUCH_BROWSER_CDP_BROWSER",
        "TOUCH_BROWSER_SEARCH_CHROME_EXECUTABLE",
        "TOUCH_BROWSER_CHROME_EXECUTABLE",
        "CHROME",
        "CHROMIUM",
    ] {
        if let Some(path) = env::var_os(key).filter(|value| !value.is_empty()) {
            let path = PathBuf::from(path);
            if path.is_file() {
                return Ok(path);
            }
        }
    }

    for path in common_browser_paths() {
        if path.is_file() {
            return Ok(path);
        }
    }

    for key in ["PLAYWRIGHT_BROWSERS_PATH", "TOUCH_BROWSER_RESOURCE_ROOT"] {
        if let Some(root) = env::var_os(key).filter(|value| !value.is_empty()) {
            if let Some(path) = find_chromium_executable(&PathBuf::from(root), 5) {
                return Ok(path);
            }
        }
    }

    Err(CliError::Adapter(
        "CDP adapter could not locate Chrome/Chromium. Set TOUCH_BROWSER_CDP_BROWSER to an executable path.".to_string(),
    ))
}

fn common_browser_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/bin/chromium"),
        PathBuf::from("/usr/bin/chromium-browser"),
        PathBuf::from("/snap/bin/chromium"),
    ]
}

fn find_chromium_executable(root: &Path, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 || !root.is_dir() {
        return None;
    }
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_chromium_executable_name(&path) {
            return Some(path);
        }
        if path.is_dir() {
            if let Some(found) = find_chromium_executable(&path, max_depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn is_chromium_executable_name(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    matches!(name, "chrome" | "chromium" | "Chromium" | "chrome.exe")
}

fn temporary_directory(prefix: &str) -> Result<PathBuf, CliError> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|source| CliError::Adapter(format!("system clock error: {source}")))?
        .as_nanos();
    let path = env::temp_dir().join(format!("{prefix}-{unique}"));
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn flatten_frame_tree(tree: &CdpFrameTreeNode, frames: &mut Vec<CdpFrameDescriptor>) {
    frames.push(tree.frame.clone());
    for child in &tree.child_frames {
        flatten_frame_tree(child, frames);
    }
}

fn normalize_browser_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn escape_html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn extract_default_execution_context(message: &Value) -> Option<(String, i64)> {
    let context = message
        .get("params")
        .and_then(|params| params.get("context"))?;
    let context_id = context.get("id").and_then(Value::as_i64)?;
    let aux_data = context.get("auxData")?;
    let frame_id = aux_data.get("frameId").and_then(Value::as_str)?;
    let is_default = aux_data
        .get("isDefault")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !is_default {
        return None;
    }
    Some((frame_id.to_string(), context_id))
}

const DOM_INSTRUMENTATION_SCRIPT: &str = r#"
(() => {
  if (globalThis.__touchBrowserDomInstrumentationInstalled) {
    return;
  }
  const originalAttachShadow = Element.prototype.attachShadow;
  Element.prototype.attachShadow = function(init) {
    const root = originalAttachShadow.call(this, init);
    if (init && init.mode === "closed") {
      try {
        Object.defineProperty(this, "__touchBrowserClosedShadowRoot", {
          configurable: true,
          enumerable: false,
          writable: true,
          value: root,
        });
      } catch {
        this.__touchBrowserClosedShadowRoot = root;
      }
    }
    return root;
  };
  globalThis.__touchBrowserDomInstrumentationInstalled = true;
})();
"#;

const FRAME_STATE_SCRIPT: &str = r#"
() => {
  const normalize = (value) => String(value || "").replace(/\s+/g, " ").trim();
  const collectRoots = (root) => {
    const roots = [root];
    const visit = (currentRoot) => {
      for (const element of Array.from(currentRoot.querySelectorAll ? currentRoot.querySelectorAll("*") : [])) {
        if (element.shadowRoot) {
          roots.push(element.shadowRoot);
          visit(element.shadowRoot);
        }
        if (element.__touchBrowserClosedShadowRoot) {
          roots.push(element.__touchBrowserClosedShadowRoot);
          visit(element.__touchBrowserClosedShadowRoot);
        }
      }
    };
    visit(root);
    return roots;
  };
  const roots = collectRoots(document);
  const queryAll = (selector) => roots.flatMap((root) => Array.from(root.querySelectorAll ? root.querySelectorAll(selector) : []));
  const textFromRoot = (root) => {
    if (root.body && root.body.innerText) return root.body.innerText;
    if (root.host && root.host.innerText) return root.host.innerText;
    return root.textContent || "";
  };
  const baseHtml = document.documentElement ? document.documentElement.outerHTML : "";
  const syntheticHtml = roots
    .filter((root) => root !== document)
    .map((root, index) => {
      if (root.host) {
        const hostPath = root.host && root.host.tagName ? normalize(root.host.tagName.toLowerCase()) : "";
        return `<section data-touch-browser-shadow-root="${index}" data-touch-browser-shadow-host="${hostPath}"><main>${root.innerHTML || ""}</main></section>`;
      }
      return "";
    })
    .join("");
  const html = `${baseHtml}${syntheticHtml}`;
  return {
    finalUrl: location.href,
    title: document.title || "",
    visibleText: normalize(roots.map(textFromRoot).join(" ")),
    html,
    linkCount: queryAll("a").length,
    buttonCount: queryAll("button").length,
    inputCount: queryAll("input").length
  };
}
"#;

const ACTION_PROBE_SCRIPT: &str = r#"
(payload) => {
  const normalize = (value) => String(value || "").replace(/\s+/g, " ").trim();
  const collectRoots = (root) => {
    const roots = [root];
    const visit = (currentRoot) => {
      for (const element of Array.from(currentRoot.querySelectorAll ? currentRoot.querySelectorAll("*") : [])) {
        if (element.shadowRoot) {
          roots.push(element.shadowRoot);
          visit(element.shadowRoot);
        }
        if (element.__touchBrowserClosedShadowRoot) {
          roots.push(element.__touchBrowserClosedShadowRoot);
          visit(element.__touchBrowserClosedShadowRoot);
        }
      }
    };
    visit(root);
    return roots;
  };
  const roots = collectRoots(document);
  const queryAll = (selector) => roots.flatMap((root) => Array.from(root.querySelectorAll ? root.querySelectorAll(selector) : []));
  const isVisible = (element) => {
    const ownerWindow = element.ownerDocument && element.ownerDocument.defaultView ? element.ownerDocument.defaultView : window;
    const style = ownerWindow.getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return style.visibility !== "hidden" && style.display !== "none" && rect.width >= 0 && rect.height >= 0;
  };
  const elementPath = (element) => {
    const parts = [];
    let current = element;
    while (current && current.tagName) {
      parts.unshift(current.tagName.toLowerCase());
      current = current.parentElement || (current.getRootNode && current.getRootNode().host) || null;
    }
    return parts.join(" > ");
  };
  const describe = (element, index) => {
    if (!isVisible(element)) return null;
    const tagName = element.tagName.toLowerCase();
    const href = element.getAttribute("href") || undefined;
    const inputDescriptor = normalize([
      element.getAttribute("name"),
      element.getAttribute("type"),
      element.getAttribute("placeholder"),
      element.getAttribute("value"),
      element.getAttribute("aria-label")
    ].filter(Boolean).join(" "));
    const text = normalize(element.innerText || element.textContent || inputDescriptor);
    if (!text && !href) return null;
    return {
      index,
      text,
      href,
      tagName,
      fullPath: elementPath(element),
      parentPath: element.parentElement ? elementPath(element.parentElement) : ""
    };
  };
  const scoreText = (candidateText, targetText) => {
    if (!targetText) return 0;
    if (candidateText === targetText) return 5;
    if (candidateText.includes(targetText) || targetText.includes(candidateText)) return 3;
    return undefined;
  };
  const scoreHref = (candidateHref, targetHref) => {
    if (!targetHref) return 0;
    if (candidateHref === targetHref) return 4;
    return candidateHref ? undefined : 0;
  };
  const scoreTag = (candidateTagName, targetTagName) => {
    if (!targetTagName) return 0;
    return candidateTagName === String(targetTagName).toLowerCase() ? 2 : undefined;
  };
  const scoreContains = (candidateText, targetValue, score) => {
    if (!targetValue) return 0;
    return candidateText.includes(String(targetValue).toLowerCase()) ? score : undefined;
  };
  const scorePath = (candidate, domPathHint) => {
    if (!domPathHint) return 0;
    const hint = String(domPathHint).toLowerCase();
    if (candidate.parentPath === hint) return 6;
    if (candidate.fullPath === hint) return 5;
    return candidate.fullPath.startsWith(`${hint} >`) ? 2 : 0;
  };
  const findBest = (selector, target) => {
    const candidates = queryAll(selector)
      .map(describe)
      .filter(Boolean)
      .map((candidate) => {
        const candidateText = candidate.text.toLowerCase();
        const targetText = normalize(target.text).toLowerCase();
        const parts = [
          scoreText(candidateText, targetText),
          scoreHref(candidate.href, target.href),
          scoreTag(candidate.tagName, target.tagName),
          scoreContains(candidateText, target.name, 2),
          scoreContains(candidateText, target.inputType, 1),
          scorePath(candidate, target.domPathHint)
        ];
        if (parts.some((part) => part === undefined)) return null;
        return { candidate, score: parts.reduce((sum, part) => sum + part, 0) };
      })
      .filter(Boolean)
      .filter((entry) => entry.score > 0)
      .sort((left, right) => right.score - left.score || left.candidate.index - right.candidate.index);
    if (candidates.length === 0) return null;
    if (target.ordinalHint && target.ordinalHint > 1) {
      const topScore = candidates[0].score;
      const top = candidates.filter((entry) => entry.score === topScore);
      return top[target.ordinalHint - 1] || candidates[target.ordinalHint - 1] || candidates[0];
    }
    return candidates[0];
  };
  const target = payload.target || {};
  const action = payload.action || {};
  const selector = action.kind === "type"
    ? "input, textarea, [contenteditable='true']"
    : action.kind === "submit"
      ? "form, button[type='submit'], input[type='submit'], button, input[type='button']"
      : action.kind === "paginate"
        ? action.direction === "prev"
          ? "a[rel='prev'], button[rel='prev'], [data-touch-browser-direction='prev'], [data-direction='prev'], button, a"
          : "a[rel='next'], button[rel='next'], [data-touch-browser-direction='next'], [data-direction='next'], button, a"
        : action.kind === "follow"
          ? "a"
          : action.kind === "expand"
            ? "button, [role='button'], summary, a"
            : "button, [role='button'], a, input[type='submit'], input[type='button'], input[type='checkbox'], input[type='radio']";
  const best = findBest(selector, target);
  if (!best) {
    return { score: 0 };
  }
  return {
    score: best.score,
    targetText: best.candidate.text,
    targetHref: best.candidate.href
  };
}
"#;

const ACTION_PERFORM_SCRIPT: &str = r#"
(payload) => {
  const normalize = (value) => String(value || "").replace(/\s+/g, " ").trim();
  const collectRoots = (root) => {
    const roots = [root];
    const visit = (currentRoot) => {
      for (const element of Array.from(currentRoot.querySelectorAll ? currentRoot.querySelectorAll("*") : [])) {
        if (element.shadowRoot) {
          roots.push(element.shadowRoot);
          visit(element.shadowRoot);
        }
        if (element.__touchBrowserClosedShadowRoot) {
          roots.push(element.__touchBrowserClosedShadowRoot);
          visit(element.__touchBrowserClosedShadowRoot);
        }
      }
    };
    visit(root);
    return roots;
  };
  const roots = collectRoots(document);
  const queryAll = (selector) => roots.flatMap((root) => Array.from(root.querySelectorAll ? root.querySelectorAll(selector) : []));
  const isVisible = (element) => {
    const ownerWindow = element.ownerDocument && element.ownerDocument.defaultView ? element.ownerDocument.defaultView : window;
    const style = ownerWindow.getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return style.visibility !== "hidden" && style.display !== "none" && rect.width >= 0 && rect.height >= 0;
  };
  const elementPath = (element) => {
    const parts = [];
    let current = element;
    while (current && current.tagName) {
      parts.unshift(current.tagName.toLowerCase());
      current = current.parentElement || (current.getRootNode && current.getRootNode().host) || null;
    }
    return parts.join(" > ");
  };
  const describe = (element, index) => {
    if (!isVisible(element)) return null;
    const tagName = element.tagName.toLowerCase();
    const href = element.getAttribute("href") || undefined;
    const inputDescriptor = normalize([
      element.getAttribute("name"),
      element.getAttribute("type"),
      element.getAttribute("placeholder"),
      element.getAttribute("value"),
      element.getAttribute("aria-label")
    ].filter(Boolean).join(" "));
    const text = normalize(element.innerText || element.textContent || inputDescriptor);
    if (!text && !href) return null;
    return {
      element,
      index,
      text,
      href,
      tagName,
      fullPath: elementPath(element),
      parentPath: element.parentElement ? elementPath(element.parentElement) : ""
    };
  };
  const scoreText = (candidateText, targetText) => {
    if (!targetText) return 0;
    if (candidateText === targetText) return 5;
    if (candidateText.includes(targetText) || targetText.includes(candidateText)) return 3;
    return undefined;
  };
  const scoreHref = (candidateHref, targetHref) => {
    if (!targetHref) return 0;
    if (candidateHref === targetHref) return 4;
    return candidateHref ? undefined : 0;
  };
  const scoreTag = (candidateTagName, targetTagName) => {
    if (!targetTagName) return 0;
    return candidateTagName === String(targetTagName).toLowerCase() ? 2 : undefined;
  };
  const scoreContains = (candidateText, targetValue, score) => {
    if (!targetValue) return 0;
    return candidateText.includes(String(targetValue).toLowerCase()) ? score : undefined;
  };
  const scorePath = (candidate, domPathHint) => {
    if (!domPathHint) return 0;
    const hint = String(domPathHint).toLowerCase();
    if (candidate.parentPath === hint) return 6;
    if (candidate.fullPath === hint) return 5;
    return candidate.fullPath.startsWith(`${hint} >`) ? 2 : 0;
  };
  const findBest = (selector, target) => {
    const candidates = queryAll(selector)
      .map(describe)
      .filter(Boolean)
      .map((candidate) => {
        const candidateText = candidate.text.toLowerCase();
        const targetText = normalize(target.text).toLowerCase();
        const parts = [
          scoreText(candidateText, targetText),
          scoreHref(candidate.href, target.href),
          scoreTag(candidate.tagName, target.tagName),
          scoreContains(candidateText, target.name, 2),
          scoreContains(candidateText, target.inputType, 1),
          scorePath(candidate, target.domPathHint)
        ];
        if (parts.some((part) => part === undefined)) return null;
        return { candidate, score: parts.reduce((sum, part) => sum + part, 0) };
      })
      .filter(Boolean)
      .filter((entry) => entry.score > 0)
      .sort((left, right) => right.score - left.score || left.candidate.index - right.candidate.index);
    if (candidates.length === 0) return null;
    if (target.ordinalHint && target.ordinalHint > 1) {
      const topScore = candidates[0].score;
      const top = candidates.filter((entry) => entry.score === topScore);
      return (top[target.ordinalHint - 1] || candidates[target.ordinalHint - 1] || candidates[0]).candidate;
    }
    return candidates[0].candidate;
  };
  const fill = (candidate, value) => {
    const element = candidate.element;
    const tagName = element.tagName.toLowerCase();
    if (tagName === "input" || tagName === "textarea") {
      element.focus();
      element.value = value;
    } else if (element.hasAttribute("contenteditable")) {
      element.focus();
      element.textContent = value;
    } else {
      throw new Error("Target input does not support typing.");
    }
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
  };
  const click = (candidate) => {
    candidate.element.scrollIntoView({ block: "center", inline: "center" });
    candidate.element.click();
  };
  const target = payload.target || {};
  const action = payload.action || {};
  if (action.kind === "type") {
    const candidate = findBest("input, textarea, [contenteditable='true']", target);
    if (!candidate) throw new Error(`No input target was found for \`${target.text || ""}\`.`);
    fill(candidate, action.value || "");
    return { targetText: candidate.text, typedLength: String(action.value || "").length };
  }
  if (action.kind === "submit") {
    for (const entry of action.prefill || []) {
      const candidate = findBest("input, textarea, [contenteditable='true']", entry.target || {});
      if (candidate) fill(candidate, entry.value || "");
    }
    const candidate = findBest("form, button[type='submit'], input[type='submit'], button, input[type='button']", target);
    if (!candidate) throw new Error(`No submit target was found for \`${target.text || ""}\`.`);
    if (candidate.element.tagName.toLowerCase() === "form") {
      if (typeof candidate.element.requestSubmit === "function") candidate.element.requestSubmit();
      else candidate.element.submit();
    } else {
      click(candidate);
    }
    return { targetText: candidate.text, clickedText: candidate.text };
  }
  if (action.kind === "paginate") {
    const direction = action.direction === "prev" ? "prev" : "next";
    const selector = direction === "prev"
      ? "a[rel='prev'], button[rel='prev'], [data-touch-browser-direction='prev'], [data-direction='prev'], button, a"
      : "a[rel='next'], button[rel='next'], [data-touch-browser-direction='next'], [data-direction='next'], button, a";
    const labels = direction === "prev" ? ["previous", "back"] : ["next", "more", "continue"];
    const candidates = queryAll(selector).map(describe).filter(Boolean);
    const candidate = candidates.find((entry) => labels.some((label) => entry.text.toLowerCase().includes(label))) || candidates[0];
    if (!candidate) throw new Error(`No ${direction} pagination target was found.`);
    click(candidate);
    const page = direction === "prev" ? Math.max(1, Number(action.currentPage || 1) - 1) : Number(action.currentPage || 1) + 1;
    return { targetText: candidate.text, clickedText: candidate.text, page };
  }
  const selector = action.kind === "follow"
    ? "a"
    : action.kind === "expand"
      ? "button, [role='button'], summary, a"
      : "button, [role='button'], a, input[type='submit'], input[type='button'], input[type='checkbox'], input[type='radio']";
  const candidate = findBest(selector, target);
  if (candidate) {
    click(candidate);
    return { targetText: candidate.text, targetHref: candidate.href, clickedText: candidate.text };
  }
  if (action.kind === "follow" && target.href) {
    const resolved = new URL(target.href, location.href);
    location.href = resolved.toString();
    return { targetText: target.text || target.href, targetHref: target.href, clickedText: target.text || target.href };
  }
  throw new Error(`No ${action.kind || "browser"} target was found for \`${target.text || target.href || ""}\`.`);
}
"#;

#[cfg(test)]
mod tests {
    use super::{
        cdp_adapter_name_enabled, extract_chrome_version, extract_default_execution_context,
        is_chromium_executable_name, looks_like_search_challenge,
    };
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn recognizes_chromium_executable_names() {
        assert!(is_chromium_executable_name(Path::new("/tmp/chrome")));
        assert!(is_chromium_executable_name(Path::new("/tmp/Chromium")));
        assert!(!is_chromium_executable_name(Path::new("/tmp/chromedriver")));
    }

    #[test]
    fn cdp_adapter_name_accepts_only_explicit_cdp_values() {
        assert!(cdp_adapter_name_enabled(Some("cdp-rust")));
        assert!(cdp_adapter_name_enabled(Some("rust-cdp")));
        assert!(cdp_adapter_name_enabled(Some("cdp")));
        assert!(!cdp_adapter_name_enabled(None));
        assert!(!cdp_adapter_name_enabled(Some("playwright")));
    }

    #[test]
    fn detects_search_challenge_signals() {
        assert!(looks_like_search_challenge(
            "https://www.google.com/sorry/index",
            "Traffic verification",
            "Confirm you're not a robot"
        ));
        assert!(looks_like_search_challenge(
            "https://search.example.test/",
            "Security check",
            "Verify you are human before continuing"
        ));
        assert!(!looks_like_search_challenge(
            "https://example.test/search?q=rust",
            "Search results",
            "Result one Result two"
        ));
    }

    #[test]
    fn extracts_browser_version_from_common_outputs() {
        assert_eq!(
            extract_chrome_version("Google Chrome 147.0.7727.103"),
            Some("147.0.7727.103".to_string())
        );
        assert_eq!(
            extract_chrome_version("Chromium 146.0.0.0 built on Ubuntu"),
            Some("146.0.0.0".to_string())
        );
        assert_eq!(extract_chrome_version("not available"), None);
    }

    #[test]
    fn extracts_default_execution_context_for_frame_events() {
        let message = json!({
            "method": "Runtime.executionContextCreated",
            "params": {
                "context": {
                    "id": 17,
                    "auxData": {
                        "frameId": "frame-123",
                        "isDefault": true
                    }
                }
            }
        });
        assert_eq!(
            extract_default_execution_context(&message),
            Some(("frame-123".to_string(), 17))
        );
    }

    #[test]
    fn ignores_non_default_execution_context_events() {
        let message = json!({
            "method": "Runtime.executionContextCreated",
            "params": {
                "context": {
                    "id": 22,
                    "auxData": {
                        "frameId": "frame-456",
                        "isDefault": false
                    }
                }
            }
        });
        assert_eq!(extract_default_execution_context(&message), None);
    }
}

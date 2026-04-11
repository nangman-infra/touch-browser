use super::*;

#[test]
fn persists_browser_session_and_reads_current_snapshot() {
    let session_file = temp_session_path("session-open");
    let output = open_browser_session(
        &session_file,
        "fixture://research/navigation/browser-pagination",
    );

    assert_eq!(output["status"], "succeeded");
    assert!(session_file.exists());

    let snapshot = dispatch(CliCommand::SessionSnapshot(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session snapshot should succeed");

    assert_eq!(snapshot["action"]["status"], "succeeded");
    assert_eq!(
        snapshot["action"]["output"]["blocks"][1]["text"],
        "Browser Pagination"
    );

    fs::remove_file(session_file).ok();
}

#[test]
fn refreshes_browser_session_from_current_live_state() {
    let session_file = temp_session_path("session-refresh");
    open_browser_session(
        &session_file,
        "fixture://research/navigation/browser-follow",
    );

    let refreshed = dispatch(CliCommand::SessionRefresh(SessionRefreshOptions {
        session_file: session_file.clone(),
        headed: false,
    }))
    .expect("refresh should succeed");

    assert_eq!(refreshed["action"]["status"], "succeeded");
    assert_eq!(refreshed["action"]["action"], "read");
    assert_eq!(
        refreshed["action"]["diagnostics"]["surface"],
        "session-refresh"
    );
    assert_eq!(refreshed["action"]["diagnostics"]["captureMode"], "browser");

    fs::remove_file(session_file).ok();
}

#[test]
fn paginates_browser_session_and_updates_snapshot() {
    let session_file = temp_session_path("session-paginate");
    open_browser_session(
        &session_file,
        "fixture://research/navigation/browser-pagination",
    );

    let output = dispatch(CliCommand::Paginate(PaginateOptions {
        session_file: session_file.clone(),
        direction: PaginationDirection::Next,
        headed: false,
    }))
    .expect("paginate should succeed");

    assert_eq!(output["action"]["status"], "succeeded");
    assert_eq!(output["action"]["action"], "paginate");
    assert_eq!(output["action"]["output"]["adapter"]["page"], 2);
    assert!(output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Page 2"));

    fs::remove_file(session_file).ok();
}

#[test]
fn preserves_browser_dom_state_across_paginate_actions() {
    let session_file = temp_session_path("session-paginate-twice");
    open_browser_session(
        &session_file,
        "fixture://research/navigation/browser-pagination",
    );

    dispatch(CliCommand::Paginate(PaginateOptions {
        session_file: session_file.clone(),
        direction: PaginationDirection::Next,
        headed: false,
    }))
    .expect("first paginate should succeed");

    let second_paginate = dispatch(CliCommand::Paginate(PaginateOptions {
        session_file: session_file.clone(),
        direction: PaginationDirection::Next,
        headed: false,
    }))
    .expect_err("second paginate should fail after the next button disappears");

    assert!(
        second_paginate
            .to_string()
            .contains("No next pagination target was found."),
        "unexpected error: {second_paginate}"
    );

    fs::remove_file(session_file).ok();
}

#[test]
fn follows_browser_session_and_can_extract_from_persisted_state() {
    let session_file = temp_session_path("session-follow");
    let open_output = open_browser_session(
        &session_file,
        "fixture://research/navigation/browser-follow",
    );
    let follow_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "link")
        .and_then(|block| block["ref"].as_str())
        .expect("link ref should exist")
        .to_string();

    let follow_output = dispatch(CliCommand::Follow(FollowOptions {
        session_file: session_file.clone(),
        target_ref: follow_ref.clone(),
        headed: false,
    }))
    .expect("follow should succeed");

    assert_eq!(follow_output["action"]["status"], "succeeded");
    assert_eq!(follow_output["action"]["action"], "follow");
    assert_eq!(follow_output["result"]["status"], "succeeded");
    assert_eq!(follow_output["action"]["diagnostics"]["surface"], "follow");
    assert_eq!(
        follow_output["action"]["diagnostics"]["targetRef"],
        follow_ref
    );
    assert_eq!(
        follow_output["action"]["diagnostics"]["waitStrategy"],
        "action-settle"
    );
    assert!(follow_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Advanced guide opened for the next research step."));

    let extract_output = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
        session_file: Some(session_file.clone()),
        engine: None,
        claims: vec!["Advanced guide opened for the next research step.".to_string()],
        verifier_command: None,
    }))
    .expect("session extract should succeed");

    assert_eq!(extract_output["extract"]["status"], "succeeded");
    assert_eq!(extract_output["result"]["status"], "succeeded");
    assert_eq!(
        extract_output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
        "Advanced guide opened for the next research step."
    );

    let close_output = dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session close should succeed");
    assert_eq!(close_output["removed"], true);
}

#[test]
fn preserves_requested_budget_across_browser_follow_actions() {
    let session_file = temp_session_path("session-follow-budget");
    let open_output = open_browser_session_with_budget(
        &session_file,
        "fixture://research/navigation/browser-follow",
        64,
    );
    let follow_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "link")
        .and_then(|block| block["ref"].as_str())
        .expect("link ref should exist")
        .to_string();

    let follow_output = dispatch(CliCommand::Follow(FollowOptions {
        session_file: session_file.clone(),
        target_ref: follow_ref,
        headed: false,
    }))
    .expect("follow should succeed");

    assert_eq!(
        follow_output["action"]["output"]["snapshot"]["budget"]["requestedTokens"],
        64
    );

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn follows_duplicate_browser_link_using_stable_ref_ordinal_hint() {
    let session_file = temp_session_path("session-follow-duplicate");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-follow-duplicate".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let follow_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .filter(|block| block["kind"] == "link")
        .find(|block| {
            block["ref"]
                .as_str()
                .expect("ref should be present")
                .ends_with(":2")
        })
        .and_then(|block| block["ref"].as_str())
        .expect("second link ref should exist")
        .to_string();

    let follow_output = dispatch(CliCommand::Follow(FollowOptions {
        session_file: session_file.clone(),
        target_ref: follow_ref,
        headed: false,
    }))
    .expect("follow should succeed");

    assert_eq!(follow_output["action"]["status"], "succeeded");
    assert!(follow_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Current docs opened for the research step."));

    let close_output = dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session close should succeed");
    assert_eq!(close_output["removed"], true);
}

#[test]
fn expands_browser_session_and_can_extract_from_persisted_state() {
    let session_file = temp_session_path("session-expand");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-expand".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let expand_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "button")
        .and_then(|block| block["ref"].as_str())
        .expect("button ref should exist")
        .to_string();

    let expand_output = dispatch(CliCommand::Expand(ExpandOptions {
        session_file: session_file.clone(),
        target_ref: expand_ref,
        headed: false,
    }))
    .expect("expand should succeed");

    assert_eq!(expand_output["action"]["status"], "succeeded");
    assert!(expand_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Expanded details confirm"));

    let extract_output = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
        session_file: Some(session_file.clone()),
        engine: None,
        claims: vec![
            "Expanded details confirm that the runtime can reveal collapsed notes.".to_string(),
        ],
        verifier_command: None,
    }))
    .expect("session extract should succeed");

    assert_eq!(extract_output["extract"]["status"], "succeeded");
    assert_eq!(
        extract_output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
        "Expanded details confirm that the runtime can reveal collapsed notes."
    );

    let close_output = dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session close should succeed");
    assert_eq!(close_output["removed"], true);
}

#[test]
fn types_into_browser_session_and_marks_session_interactive() {
    let session_file = temp_session_path("session-type");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-login-form",
    );
    let email_ref = browser_session_block_ref(&open_output, "input", Some("agent@example.com"));

    let type_output = dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: email_ref.clone(),
        value: "agent@example.com".to_string(),
        headed: false,
        sensitive: false,
        ack_risks: Vec::new(),
    }))
    .expect("type should succeed");

    assert_eq!(type_output["action"]["status"], "succeeded");
    assert_eq!(type_output["action"]["action"], "type");
    assert_eq!(type_output["sessionState"]["mode"], "interactive");
    assert_eq!(
        type_output["sessionState"]["policyProfile"],
        "interactive-review"
    );
    assert_eq!(
        type_output["action"]["output"]["adapter"]["typedLength"],
        17
    );
    assert_eq!(type_output["action"]["diagnostics"]["surface"], "type");
    assert_eq!(type_output["action"]["diagnostics"]["targetRef"], email_ref);
    assert_eq!(type_output["action"]["diagnostics"]["sensitive"], false);
    assert!(type_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("agent@example.com"));

    close_browser_fixture_session(session_file);
}

#[test]
fn rejects_sensitive_type_without_explicit_opt_in() {
    let session_file = temp_session_path("session-type-sensitive");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-login-form",
    );
    let password_ref = browser_session_block_ref(&open_output, "input", Some("password"));

    let type_output = dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: password_ref,
        value: "hunter2".to_string(),
        headed: false,
        sensitive: false,
        ack_risks: Vec::new(),
    }))
    .expect("type command should return a rejection");

    assert_eq!(type_output["action"]["status"], "rejected");
    assert_eq!(type_output["action"]["failureKind"], "policy-blocked");

    close_browser_fixture_session(session_file);
}

#[test]
fn clicks_browser_session_button_after_interactive_typing() {
    let session_file = temp_session_path("session-click");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-login-form",
    );
    let email_ref = browser_session_block_ref(&open_output, "input", Some("agent@example.com"));
    let button_ref = browser_session_block_ref(&open_output, "button", None);

    dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: email_ref,
        value: "agent@example.com".to_string(),
        headed: false,
        sensitive: false,
        ack_risks: Vec::new(),
    }))
    .expect("type should succeed");

    let click_output = dispatch(CliCommand::Click(ClickOptions {
        session_file: session_file.clone(),
        target_ref: button_ref.clone(),
        headed: false,
        ack_risks: vec![AckRisk::Auth],
    }))
    .expect("click should succeed");

    assert_eq!(click_output["action"]["status"], "succeeded");
    assert_eq!(click_output["action"]["action"], "click");
    assert_eq!(click_output["action"]["diagnostics"]["surface"], "click");
    assert_eq!(
        click_output["action"]["diagnostics"]["targetRef"],
        button_ref
    );
    assert!(click_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Signed in draft session ready for review."));

    close_browser_fixture_session(session_file);
}

#[test]
fn submits_browser_session_form_after_interactive_typing() {
    let session_file = temp_session_path("session-submit");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-login-form",
    );
    let email_ref = browser_session_block_ref(&open_output, "input", Some("agent@example.com"));
    let form_ref = browser_session_block_ref(&open_output, "form", None);

    dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: email_ref,
        value: "agent@example.com".to_string(),
        headed: false,
        sensitive: false,
        ack_risks: Vec::new(),
    }))
    .expect("type should succeed");

    let submit_output = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref,
        headed: false,
        ack_risks: Vec::new(),
        extra_prefill: Vec::new(),
    }))
    .expect("submit should succeed");

    assert_eq!(submit_output["action"]["status"], "succeeded");
    assert_eq!(submit_output["action"]["action"], "submit");
    assert!(submit_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Signed in draft session ready for review."));

    close_browser_fixture_session(session_file);
}

#[test]
fn rejects_mfa_submit_without_ack_and_allows_it_with_ack() {
    let session_file = temp_session_path("session-mfa");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-mfa-challenge",
    );
    let otp_ref = browser_session_block_ref(&open_output, "input", None);
    let form_ref = browser_session_block_ref(&open_output, "form", None);

    let blocked = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref.clone(),
        headed: false,
        ack_risks: Vec::new(),
        extra_prefill: Vec::new(),
    }))
    .expect("submit should return a rejection");
    assert_eq!(blocked["action"]["status"], "rejected");

    dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: otp_ref,
        value: "123456".to_string(),
        headed: false,
        sensitive: true,
        ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
    }))
    .expect("sensitive MFA type should succeed");

    let approved = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref,
        headed: false,
        ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
        extra_prefill: Vec::new(),
    }))
    .expect("approved submit should succeed");
    assert_eq!(approved["action"]["status"], "succeeded");
    assert!(approved["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Verification code accepted for supervised review."));

    close_browser_fixture_session(session_file);
}

#[test]
fn checkpoint_and_approve_enable_supervised_session_without_repeating_ack_flags() {
    let session_file = temp_session_path("session-checkpoint-approve");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-mfa-challenge",
    );
    let otp_ref = browser_session_block_ref(&open_output, "input", None);
    let form_ref = browser_session_block_ref(&open_output, "form", None);

    let checkpoint = dispatch(CliCommand::SessionCheckpoint(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("checkpoint should succeed");
    assert!(checkpoint["checkpoint"]["requiredAckRisks"]
        .as_array()
        .expect("required risks should be an array")
        .iter()
        .any(|risk| risk == "mfa"));
    assert!(checkpoint["checkpoint"]["requiredAckRisks"]
        .as_array()
        .expect("required risks should be an array")
        .iter()
        .any(|risk| risk == "auth"));
    assert_eq!(
        checkpoint["checkpoint"]["recommendedPolicyProfile"],
        "interactive-supervised-auth"
    );
    assert_eq!(
        checkpoint["checkpoint"]["playbook"]["provider"],
        "generic-auth"
    );

    let approval = dispatch(CliCommand::Approve(ApproveOptions {
        session_file: session_file.clone(),
        ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
    }))
    .expect("approve should succeed");
    assert!(approval["approvedRisks"]
        .as_array()
        .expect("approved risks should be an array")
        .iter()
        .any(|risk| risk == "mfa"));
    assert_eq!(approval["policyProfile"], "interactive-supervised-auth");

    dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: otp_ref,
        value: "123456".to_string(),
        headed: false,
        sensitive: true,
        ack_risks: Vec::new(),
    }))
    .expect("approved MFA type should succeed without inline ack");

    let approved = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref,
        headed: false,
        ack_risks: Vec::new(),
        extra_prefill: Vec::new(),
    }))
    .expect("approved submit should succeed without inline ack");
    assert_eq!(approved["action"]["status"], "succeeded");

    close_browser_fixture_session(session_file);
}

#[test]
fn rejects_high_risk_submit_without_ack_and_allows_it_with_ack() {
    let session_file = temp_session_path("session-high-risk");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-high-risk-checkout",
    );
    let form_ref = browser_session_block_ref(&open_output, "form", None);

    let blocked = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref.clone(),
        headed: false,
        ack_risks: Vec::new(),
        extra_prefill: Vec::new(),
    }))
    .expect("submit should return a rejection");
    assert_eq!(blocked["action"]["status"], "rejected");

    let approved = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref,
        headed: false,
        ack_risks: vec![AckRisk::HighRiskWrite],
        extra_prefill: Vec::new(),
    }))
    .expect("approved submit should succeed");
    assert_eq!(approved["action"]["status"], "succeeded");
    assert!(approved["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Purchase confirmation staged for supervised review."));

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

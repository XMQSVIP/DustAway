//! Opt-in native observation. Never installs/removes a package or stops another process.
use super::*;
use std::{
    io::Write,
    os::windows::process::CommandExt,
    process::{Command, Stdio},
};

#[test]
#[ignore = "internal opt-in lifetime child; requires its own marked executable directory"]
fn process_lifetime_child() -> Result<()> {
    let root = std::env::current_exe()?
        .parent()
        .context("missing fixture root")?
        .to_path_buf();
    let token = std::env::var("DUSTAWAY_STORE_PROCESS_CHILD")?;
    uuid::Uuid::parse_str(&token)?;
    ensure!(
        matches!(
            root.file_name().and_then(|name| name.to_str()),
            Some(
                "store-process-lifetime-20261001"
                    | "desktop-process-cross-session-20261001"
                    | "desktop-process-cross-session-v3-20261001"
            )
        ),
        "wrong child scope"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-process-fixture"))? == token,
        "wrong child marker"
    );
    std::fs::write(root.join("ready"), &token)?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    // EOF permits a natural exit if the parent failed; never remain orphaned.
    ensure!(
        input.is_empty() || input.trim() == token,
        "invalid child stop token"
    );
    Ok(())
}

#[test]
#[ignore = "same-user SSH/interactive sessions on the isolated PC; dedicated process only"]
fn real_desktop_cross_session_guard() -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
        "wrong host"
    );
    ensure!(!security::elevated()?, "ordinary user required");
    let root = known_folder(&FOLDERID_LocalAppData)?.join("DustAwayAcceptance/20260930-software");
    ensure!(
        extended(
            std::env::current_exe()?
                .parent()
                .context("missing driver directory")?
        )? == extended(&root)?,
        "wrong driver scope"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
            == "DustAway.Acceptance.20260930.7c7e25e8",
        "wrong marker"
    );
    let output = root.join("desktop-process-cross-session-v3-native.json");
    ensure!(!output.try_exists()?, "evidence exists; no replay");
    let fixture = root.join("desktop-process-cross-session-v3-20261001");
    let token = std::fs::read_to_string(fixture.join(".dustaway-process-fixture"))?;
    uuid::Uuid::parse_str(&token)?;
    let context: serde_json::Value = serde_json::from_slice(&std::fs::read(
        root.join("desktop-process-cross-session-v3-context.json"),
    )?)?;
    let pid = u32::try_from(context["pid"].as_u64().context("missing child PID")?)?;
    ensure!(context["token"] == token, "wrong child context");
    // ProcessIdToSessionId itself can deny an ordinary caller access to the SSH
    // child. Validate the session in the WTS snapshot instead of elevating.
    let child_session = u32::try_from(
        context["session"]
            .as_u64()
            .context("missing parent-observed session")?,
    )?;
    let caller_session =
        security::process_session(std::process::id()).context("read observer session")?;
    let result = (|| -> Result<serde_json::Value> {
        ensure!(
            child_session != caller_session && caller_session != 0,
            "not a cross-session sample"
        );
        ensure!(
            std::fs::read_to_string(fixture.join("ready"))? == token,
            "child not ready"
        );
        // This is the unchanged boundary used by desktop residue scan/delete before
        // this fix. It must reproduce the other-session miss without deleting files.
        let name = "desktop-cross-session-child.exe";
        let legacy_missed = !security::process_running(name)?;
        let sid = security::current_sid()?;
        let owned = owned_processes(&sid, None).context("query current-user WTS processes")?;
        let same_user_observed = owned.iter().any(|p| {
            p.pid == pid
                && p.session == child_session
                && p.name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
        });
        let metadata_error = observe(pid, &sid).err().map(|e| e.to_string());
        let names = vec![name.to_ascii_uppercase()];
        let toolhelp_observed = matching_named_pids(&names, None)?.contains(&pid);
        let same_user_attested =
            context["parent_user_sid_sha256"] == format!("{:x}", Sha256::digest(sid.as_bytes()));
        let desktop_error = ensure_named_stopped(&names, None).err();
        let desktop_reason = desktop_error
            .as_ref()
            .and_then(|e| e.downcast_ref::<Refusal>())
            .map(|r| r.0);
        let package_error = ensure_stopped("DustAway.CrossSession_publisher", &fixture, &sid, None)
            .err()
            .map(|e| e.to_string());
        Ok(
            serde_json::json!({"ordinary_user":true,"same_user_observed_by_wts":same_user_observed,"same_user_attested_by_creator":same_user_attested,"toolhelp_observed_child":toolhelp_observed,"caller_session":caller_session,"child_session":child_session,"legacy_session_check_missed":legacy_missed,"new_desktop_check_rejected":desktop_error.is_some(),"desktop_reason":desktop_reason,"child_metadata_error":metadata_error,"package_guard_error":package_error,"real_package_process":false,"real_uninstall":false,"real_residual_file_deletion":false,"production_authority_granted":false}),
        )
    })();
    // Tell the dedicated SSH parent to release its own child's stdin. No termination API.
    let mut release = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(fixture.join("release"))?;
    release.write_all(token.as_bytes())?;
    release.sync_all()?;
    drop(release);
    // Ordinary observers may not obtain a handle to the higher-integrity SSH
    // child. Its owner/creator retains the real handle and records natural exit.
    // Do not elevate the observer just to turn an unknown result into success.
    let exit_path = root.join("desktop-process-cross-session-v3-parent-result.json");
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let parent = loop {
        if let Ok(bytes) = std::fs::read(&exit_path)
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(
                bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes),
            )
        {
            break value;
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "parent exit receipt unavailable; do not restart"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    ensure!(
        parent["ChildPid"] == pid
            && parent["ChildExitCode"] == 0
            && parent["Trigger"] == "controller_release",
        "child exit was not observed by its original parent"
    );
    ensure!(
        !owned_pids(&security::current_sid()?, None)?.contains(&pid),
        "child remains visible"
    );
    let mut report = result?;
    ensure_named_stopped(&["desktop-cross-session-child.exe".into()], None)?;
    report["child_exited_naturally"] = serde_json::json!(true);
    report["after_exit_clear"] = serde_json::json!(true);
    report["exit_observer"] = serde_json::json!("original SSH parent process handle");
    std::fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    ensure!(
        report["same_user_attested_by_creator"] == true
            && report["toolhelp_observed_child"] == true
            && report["new_desktop_check_rejected"] == true
            && matches!(
                report["desktop_reason"].as_str(),
                Some("application_running" | "process_unknown")
            ),
        "cross-session protection incomplete; diagnostic evidence retained"
    );
    Ok(())
}

#[test]
#[ignore = "ordinary-token process checks on the dedicated acceptance PC; no package removal"]
fn real_store_process_queries_and_lifetime() -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
        "wrong host"
    );
    ensure!(!security::elevated()?, "ordinary token required");
    let root = known_folder(&FOLDERID_LocalAppData)?.join("DustAwayAcceptance/20260930-software");
    ensure!(
        std::env::current_exe()?.parent() == Some(root.as_path()),
        "wrong driver scope"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
            == "DustAway.Acceptance.20260930.7c7e25e8",
        "wrong marker"
    );
    let output = root.join("store-process-native-20261001.json");
    ensure!(!output.try_exists()?, "evidence exists; no replay");
    let fixture = root.join("store-process-lifetime-20261001");
    ensure!(
        !fixture.try_exists()?,
        "fixture exists; inspect before replay"
    );
    std::fs::create_dir(&fixture)?;
    let token = uuid::Uuid::new_v4().to_string();
    std::fs::write(fixture.join(".dustaway-process-fixture"), &token)?;
    let exe = fixture.join("process-fixture.exe");
    std::fs::copy(std::env::current_exe()?, &exe)?;
    let sid = security::current_sid()?;
    let family = "DustAway.ProcessAcceptance_publisher";
    let pids = owned_pids(&sid, None)?;
    let mut packaged = vec![];
    let mut unknown = 0;
    for pid in &pids {
        match observe(*pid, &sid) {
            Ok(Some(process)) if process.family.is_some() => packaged.push(process),
            Ok(_) => {}
            Err(_) => unknown += 1,
        }
    }
    ensure!(
        !packaged.is_empty(),
        "no current-user packaged process observed"
    );
    let actual = packaged[0].family.as_ref().unwrap();
    let running = ensure_stopped(actual, &fixture, &sid, None).unwrap_err();
    ensure!(
        running
            .downcast_ref::<Refusal>()
            .is_some_and(|r| r.0 == ReasonCode::ApplicationRunning),
        "real package process was not rejected"
    );
    let before = ensure_stopped(family, &fixture, &sid, None);
    let before_clear = before.is_ok();
    let before_error = before.err().map(|error| error.to_string());
    // The child exits naturally on a pipe token/EOF and does not access app data.
    let mut child = Command::new(&exe)
        .args([
            "--ignored",
            "--exact",
            "software::store_processes::acceptance::process_lifetime_child",
            "--nocapture",
        ])
        .env("DUSTAWAY_STORE_PROCESS_CHILD", &token)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW.0)
        .spawn()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    let during = (|| -> Result<()> {
        while !fixture.join("ready").try_exists()? {
            ensure!(child.try_wait()?.is_none(), "child exited before ready");
            ensure!(
                std::time::Instant::now() < deadline,
                "child readiness timed out"
            );
            std::thread::sleep(Duration::from_millis(40));
        }
        ensure!(
            std::fs::read_to_string(fixture.join("ready"))? == token,
            "child marker differs"
        );
        let process = observe(child.id(), &sid)?.context("child process missing")?;
        ensure!(process.family.is_none(), "child must be unpackaged");
        let error = ensure_stopped(family, &fixture, &sid, None).unwrap_err();
        ensure!(
            error
                .downcast_ref::<Refusal>()
                .is_some_and(|r| r.0 == ReasonCode::ApplicationRunning),
            "unpackaged installation child was not rejected"
        );
        Ok(())
    })();
    if let Some(mut input) = child.stdin.take() {
        let _ = writeln!(input, "{token}");
    }
    // No process termination API is used, including on an assertion failure.
    let exit = child.wait()?;
    during?;
    ensure!(exit.success(), "child did not exit normally");
    ensure!(
        !owned_pids(&sid, None)?.contains(&child.id()),
        "exited child remains listed"
    );
    let after = ensure_stopped(family, &fixture, &sid, None);
    let after_clear = after.is_ok();
    let after_error = after.err().map(|error| error.to_string());
    let report = serde_json::json!({
        "ordinary_user":true, "user_process_count":pids.len(), "packaged_process_count":packaged.len(),
        "query_failures":unknown, "real_package_running_rejected":true,
        "unpackaged_child_running_rejected":true, "child_exited_naturally":true,
        "before_clear":before_clear, "before_error":before_error,
        "after_clear":after_clear, "after_error":after_error,
        "real_uninstall":false, "real_residual_file_deletion":false, "production_approval_added":false,
    });
    std::fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    ensure!(
        before_clear && after_clear,
        "another current-user process could not be inspected; results retained"
    );
    Ok(())
}

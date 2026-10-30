//! Prepared diagnostic-log policy. This module grants no release permission.
use crate::{
    applications,
    fs::*,
    historical_log_probe::{Probe, Processes, discover_todesk, observed},
};
use anyhow::{Context, Result, ensure};
use dustaway_core::{ReasonCode, Refusal, applications::application};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

pub(crate) const AGE_DAYS: u32 = 7;
#[cfg(test)]
pub(crate) const RULE_IDS: [&str; 2] = ["appcache.dingtalk_logs", "appcache.todesk_logs"];

#[derive(Clone, Copy)]
pub(crate) enum Policy {
    DingTalk,
    ToDesk,
}
impl Policy {
    pub(crate) fn for_rule(id: &str) -> Option<Self> {
        match id {
            "appcache.dingtalk_logs" => Some(Self::DingTalk),
            "appcache.todesk_logs" => Some(Self::ToDesk),
            _ => None,
        }
    }
    pub(crate) fn roots(self, local: &Path, roaming: &Path) -> Result<Vec<PathBuf>> {
        let paths = match self {
            Self::DingTalk => vec![
                roaming.join("DingTalk/log"),
                roaming.join("DingTalk/holmeslogs"),
            ],
            Self::ToDesk => vec![local.join("ToDesk/Logs")],
        };
        paths.iter().map(|p| extended(p)).collect()
    }
    pub(crate) fn accepts(self, root: &Path, path: &Path) -> bool {
        path.parent() == Some(root)
            && path
                .extension()
                .and_then(|v| v.to_str())
                .is_some_and(|v| v.eq_ignore_ascii_case("log") || v.eq_ignore_ascii_case("xlog"))
    }
    pub(crate) fn require_age(self, modified: i64, at: i64) -> Result<()> {
        ensure!(
            modified <= at - i64::from(AGE_DAYS) * 86400,
            Refusal(ReasonCode::Recent)
        );
        Ok(())
    }
    pub(crate) fn identify(
        self,
        probe: &dyn Probe,
        cancel: Option<&AtomicBool>,
    ) -> Result<LogIdentity> {
        let result = (|| {
            match self {
                Self::DingTalk => applications::identify_candidates(
                    application("dingtalk").unwrap(),
                    observed(cancel, || probe.dingtalk_candidates(cancel))?,
                    |pin, publisher| observed(cancel, || probe.signed_by(pin, publisher, cancel)),
                    |pin| observed(cancel, || probe.version(pin, cancel)),
                    cancel,
                )
                .map(LogIdentity::DingTalk),
                Self::ToDesk => {
                    let found = discover_todesk(probe, cancel)?;
                    ensure!(found.complete, Refusal(ReasonCode::ApplicationUnverified));
                    let mut verified = Vec::new();
                    for path in found.paths {
                        check_cancel(cancel)?;
                        ensure!(
                            path.is_absolute()
                                && path
                                    .file_name()
                                    .is_some_and(|n| n.eq_ignore_ascii_case("ToDesk.exe")),
                            Refusal(ReasonCode::ApplicationUnverified)
                        );
                        if !observed(cancel, || Ok(path.try_exists()?))? {
                            ensure!(
                                !found.registered_paths.contains(&path),
                                Refusal(ReasonCode::ApplicationUnverified)
                            );
                            continue;
                        }
                        let pin = observed(cancel, || {
                            PinnedPath::open_cancellable(&path, false, false, cancel)
                        })?;
                        if verified.iter().any(|(p, _): &(PinnedPath, String)| {
                            p.stamp.identity == pin.stamp.identity
                        }) {
                            continue;
                        }
                        ensure!(
                            observed(cancel, || probe.signed_by(
                                &pin,
                                "Hainan YouQu Technology Co., Ltd",
                                cancel
                            ))?,
                            Refusal(ReasonCode::ApplicationUnverified)
                        );
                        // Do not parse resources of a candidate that failed publisher verification.
                        ensure!(
                            observed(cancel, || probe.product(&pin, cancel))? == "ToDesk",
                            Refusal(ReasonCode::ApplicationUnverified)
                        );
                        let version = observed(cancel, || probe.version(&pin, cancel))?;
                        ensure!(
                            !version.trim().is_empty(),
                            Refusal(ReasonCode::ApplicationUnverified)
                        );
                        verified.push((pin, version));
                    }
                    check_cancel(cancel)?;
                    require_unique_installation(verified.len())?;
                    let (pin, version) = verified.remove(0);
                    Ok(LogIdentity::ToDesk { pin, version })
                }
            }
        })();
        result.map_err(|e: anyhow::Error| {
            if e.is::<ScanCancelled>() {
                e
            } else {
                Refusal(ReasonCode::ApplicationUnverified).into()
            }
        })
    }
}

fn require_unique_installation(count: usize) -> Result<()> {
    ensure!(count == 1, Refusal(ReasonCode::ApplicationUnverified));
    Ok(())
}

pub(crate) enum LogIdentity {
    DingTalk(applications::VerifiedApplication),
    ToDesk { pin: PinnedPath, version: String },
}
impl LogIdentity {
    pub(crate) fn fingerprint(&self) -> Result<String> {
        match self {
            Self::DingTalk(app) => Ok(serde_json::to_string(&(
                app.fingerprint()?,
                &app.pin.parent_ids,
            ))?),
            Self::ToDesk { pin, version } => Ok(serde_json::to_string(&(
                native(&pin.path),
                &pin.stamp,
                &pin.parent_ids,
                version,
            ))?),
        }
    }
    pub(crate) fn require_stopped(
        &self,
        probe: &dyn Probe,
        cancel: Option<&AtomicBool>,
    ) -> Result<()> {
        let (names, shared, install): (&[&str], &[&str], &Path) = match self {
            Self::DingTalk(app) => (
                application("dingtalk").unwrap().processes,
                &[],
                &app.install,
            ),
            Self::ToDesk { pin, .. } => (
                &[
                    "ToDesk.exe",
                    "ToDesk_Service.exe",
                    "ToDeskService.exe",
                    "todesk-mcp.exe",
                ],
                &[
                    "updater.exe",
                    "zagent.exe",
                    "CoreToolsMgrHelper.exe",
                    "CrashReport.exe",
                    "codec_checker.exe",
                ],
                pin.path.parent().context("invalid ToDesk executable")?,
            ),
        };
        let processes = observed(cancel, || probe.processes(cancel)).map_err(|e| {
            if e.is::<ScanCancelled>() {
                e
            } else {
                Refusal(ReasonCode::ProcessUnknown).into()
            }
        })?;
        process_decision(&processes, names, shared, install)
    }
}

fn process_decision(
    observed: &Processes,
    names: &[&str],
    shared: &[&str],
    install: &Path,
) -> Result<()> {
    let root = display_path(install).trim_end_matches('\\').to_lowercase() + "\\";
    let mut unknown = !observed.complete || observed.current_session.is_none();
    for process in &observed.entries {
        if names.iter().any(|n| process.name.eq_ignore_ascii_case(n)) {
            return Err(Refusal(ReasonCode::ApplicationRunning).into());
        }
        let relevant = shared.iter().any(|n| process.name.eq_ignore_ascii_case(n))
            || process.session.is_none()
            || observed
                .current_session
                .is_some_and(|session| process.session == Some(session));
        if !relevant {
            continue;
        }
        match &process.image {
            Some(path) if display_path(path).to_lowercase().starts_with(&root) => {
                return Err(Refusal(ReasonCode::ApplicationRunning).into());
            }
            Some(_) => {}
            None => unknown = true,
        }
    }
    ensure!(!unknown, Refusal(ReasonCode::ProcessUnknown));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn historical_log_policy_uses_seven_complete_days_and_direct_known_extensions() {
        for id in RULE_IDS {
            let p = Policy::for_rule(id).unwrap();
            let at = 2_000_000_000;
            let cutoff = at - 7 * 86400;
            assert!(p.require_age(cutoff, at).is_ok());
            for time in [cutoff + 1, at, at + 1] {
                assert!(p.require_age(time, at).is_err());
            }
            let root = Path::new("C:/fixture/Logs");
            for name in ["old.log", "old.xlog", "old.LOG"] {
                assert!(p.accepts(root, &root.join(name)));
            }
            for name in [
                "old.log.bak",
                "buffer.mmap",
                "cache/old.log",
                "../old.log",
                "chat.db",
            ] {
                assert!(!p.accepts(root, &root.join(name)));
            }
        }
        assert!(Policy::for_rule("appcache.todesk_web").is_none());
    }
}

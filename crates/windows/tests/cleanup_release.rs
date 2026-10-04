//! Integration tests link the ordinary library, without its cfg(test) fixture authorization.
use dustaway_core::{FileRecord, Platform, ReasonCode, Scope, cleanup_acceptance, rules};
use dustaway_windows::{WindowsPlatform, fs::error_reason};

#[test]
fn ordinary_build_cannot_enable_application_cleanup_with_forged_descriptors() {
    let platform = WindowsPlatform::new(Scope::User).unwrap();
    let rules = rules::for_scope(Scope::User).unwrap();
    assert_eq!(rules.len(), 112);
    assert_eq!(
        rules
            .iter()
            .filter(|r| platform.cleanup_validated(r))
            .count(),
        42
    );
    for mut rule in rules.into_iter().filter(|r| {
        r.app_id.is_some() && !matches!(r.id.as_str(), "dingtalk.http" | "dingtalk.code")
    }) {
        rule.cleanup_validated = true;
        assert!(cleanup_acceptance::versions(&rule).is_empty());
        assert!(!platform.cleanup_validated(&rule));
        let file = FileRecord {
            path_display: "synthetic-invalid-record".into(),
            bytes: 1,
            modified_unix: 0,
            opaque: "{}".into(),
        };
        assert_eq!(
            error_reason(&platform.delete(&rule, &file, &[]).unwrap_err()),
            ReasonCode::ValidationPending
        );
    }
}

#[test]
fn ordinary_dingtalk_permission_still_requires_a_real_frozen_file() {
    let platform = WindowsPlatform::new(Scope::User).unwrap();
    for rule in rules::for_scope(Scope::User)
        .unwrap()
        .into_iter()
        .filter(|r| matches!(r.id.as_str(), "dingtalk.http" | "dingtalk.code"))
    {
        assert!(platform.cleanup_validated(&rule));
        assert_eq!(cleanup_acceptance::versions(&rule), ["8.2.0.119"]);
        // This invalid opaque record cannot identify a file or reach filesystem deletion.
        let file = FileRecord {
            path_display: "synthetic-invalid-record".into(),
            bytes: 1,
            modified_unix: 0,
            opaque: "{}".into(),
        };
        assert!(platform.delete(&rule, &file, &[]).is_err());
        let mut forged = rule.clone();
        forged.relative = "Local Storage".into();
        assert!(!platform.cleanup_validated(&forged));
        assert!(platform.delete(&forged, &file, &[]).is_err());
    }
}

//! Closed application/rule registry. A directory name alone never authorizes deletion.
use crate::{CleanupPolicy, DataKind, RuleDescriptor};

pub struct Application {
    pub id: &'static str,
    pub title: &'static str,
    /// Observed fixed file version; ProductVersion strings may differ. This is not deletion acceptance.
    pub version: &'static str,
    pub executable: &'static str,
    pub processes: &'static [&'static str],
    pub publisher: &'static str,
    pub registry_keys: &'static [&'static str],
    pub executable_relative: &'static str,
}

pub const APPLICATIONS: &[Application] = &[
    Application {
        id: "weixin",
        title: "微信",
        version: "4.1.13.65",
        executable: "Weixin.exe",
        processes: &[
            "Weixin.exe",
            "WeChat.exe",
            "WeChatAppEx.exe",
            "WeChatUtility.exe",
            "WeChatCrash.exe",
        ],
        publisher: "Tencent Technology (Shenzhen) Company Limited",
        registry_keys: &["Software\\Tencent\\Weixin", "Software\\Tencent\\WeChat"],
        executable_relative: "Weixin.exe",
    },
    Application {
        id: "qq",
        title: "经典 QQ",
        version: "9.7.23.29394",
        executable: "QQ.exe",
        processes: &[
            "QQ.exe",
            "QQApp.exe",
            "QQExternal.exe",
            "QQProtect.exe",
            "TXPlatform.exe",
            "QQCrashReport.exe",
        ],
        publisher: "Tencent Technology (Shenzhen) Company Limited",
        registry_keys: &["Software\\Tencent\\QQ"],
        executable_relative: "Bin/QQ.exe",
    },
    Application {
        id: "wps",
        title: "WPS",
        version: "12.8.2.18205",
        executable: "wps.exe",
        processes: &[
            "wps.exe",
            "wpp.exe",
            "et.exe",
            "wpspdf.exe",
            "wpscloudsvr.exe",
            "wpscenter.exe",
            "wpsclouddrive.exe",
            "wpsrenderer.exe",
            "wpscrash.exe",
        ],
        publisher: "Zhuhai Kingsoft Office Software Co., Ltd.",
        registry_keys: &[],
        executable_relative: "office6/wps.exe",
    },
    Application {
        id: "feishu",
        title: "飞书",
        version: "7.64.0.0",
        executable: "Feishu.exe",
        processes: &["Feishu.exe"],
        publisher: "Beijing Feishu Technology Co., Ltd.",
        registry_keys: &[],
        executable_relative: "Feishu.exe",
    },
    Application {
        id: "dingtalk",
        title: "钉钉",
        version: "8.5.5.903",
        executable: "DingTalk.exe",
        processes: &["DingTalk.exe", "DingTalkUp.exe"],
        publisher: "DingTalk Technology Co., Ltd.",
        registry_keys: &[],
        executable_relative: "DingTalk.exe",
    },
    Application {
        id: "baidu_netdisk",
        title: "百度网盘",
        version: "8.8.3.101",
        executable: "BaiduNetdisk.exe",
        processes: &[
            "BaiduNetdisk.exe",
            "BaiduNetdiskRender.exe",
            "YunDetectService.exe",
            "YunUtilityService.exe",
        ],
        publisher: "Beijing Duyou Science and Technology Co.,Ltd.",
        registry_keys: &[],
        executable_relative: "BaiduNetdisk.exe",
    },
    Application {
        id: "quark_netdisk",
        title: "夸克网盘",
        version: "3.19.0.0",
        executable: "QuarkCloudDrive.exe",
        processes: &["QuarkCloudDrive.exe"],
        publisher: "ALIBABA (CHINA) NETWORK TECHNOLOGY CO.,LTD.",
        registry_keys: &[],
        executable_relative: "QuarkCloudDrive.exe",
    },
    Application {
        id: "thunder",
        title: "迅雷",
        version: "12.4.10.3940",
        executable: "Thunder.exe",
        processes: &[
            "Thunder.exe",
            "ThunderStart.exe",
            "ThunderEv.exe",
            "XLLite.exe",
            "XLServicePlatform.exe",
        ],
        publisher: "Shenzhen Xunlei Network Technology Co., Ltd.",
        registry_keys: &[],
        executable_relative: "Program/Thunder.exe",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationFolder {
    Roaming,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileLayout {
    Direct,
    /// One configuration root, requiring an ordinary Preferences marker.
    ChromiumRoot,
    QqNumeric,
    FeishuProfiles,
    DingTalkNumbered,
    WeixinWeb,
}

#[derive(Clone, Copy)]
pub struct ApplicationRule {
    pub id: &'static str,
    pub app: &'static str,
    pub folder: ApplicationFolder,
    pub profiles: ProfileLayout,
    /// Stable description of the observed layout, not deletion acceptance.
    pub layout_id: &'static str,
    /// Relative to the specified Known Folder; numbered layouts enumerate its direct children.
    pub base: &'static str,
    pub relative: &'static str,
    pub extension: Option<&'static str>,
    pub age: u32,
    pub kind: DataKind,
}

impl ApplicationRule {
    pub fn validated(&self) -> bool {
        !crate::cleanup_acceptance::versions_for_spec(
            self,
            1,
            crate::cleanup_acceptance::ACCEPTANCES,
        )
        .is_empty()
    }
}

pub const APPLICATION_RULES: &[ApplicationRule] = &[
    ApplicationRule {
        id: "weixin.http",
        app: "weixin",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::WeixinWeb,
        layout_id: "weixin-radium-web-v1",
        base: "Tencent/xwechat/radium/web/profiles",
        relative: "Cache/Cache_Data",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "weixin.code",
        app: "weixin",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::WeixinWeb,
        layout_id: "weixin-radium-web-v1",
        base: "Tencent/xwechat/radium/web/profiles",
        relative: "Code Cache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "weixin.gpu",
        app: "weixin",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::WeixinWeb,
        layout_id: "weixin-radium-web-v1",
        base: "Tencent/xwechat/radium/web/profiles",
        relative: "GPUCache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "weixin.logs",
        app: "weixin",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::Direct,
        layout_id: "weixin-diagnostics-v1",
        base: "Tencent/xwechat",
        relative: "log",
        extension: Some("xlog"),
        age: 7,
        kind: DataKind::Diagnostics,
    },
    ApplicationRule {
        id: "weixin.dumps",
        app: "weixin",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::Direct,
        layout_id: "weixin-diagnostics-v1",
        base: "Tencent/xwechat",
        relative: "crashinfo",
        extension: Some("dmp"),
        age: 30,
        kind: DataKind::Diagnostics,
    },
    ApplicationRule {
        id: "qq.http",
        app: "qq",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::QqNumeric,
        layout_id: "qq-classic-webkit-v1",
        base: "Tencent/QQ/webkitex_cache",
        relative: "Cache/Cache_Data",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "qq.code",
        app: "qq",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::QqNumeric,
        layout_id: "qq-classic-webkit-v1",
        base: "Tencent/QQ/webkitex_cache",
        relative: "Code Cache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "qq.gpu",
        app: "qq",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::QqNumeric,
        layout_id: "qq-classic-webkit-v1",
        base: "Tencent/QQ/webkitex_cache",
        relative: "GPUCache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "wps.logs",
        app: "wps",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::Direct,
        layout_id: "wps-office6-log-v1",
        base: "Kingsoft/office6",
        relative: "log",
        extension: Some("log"),
        age: 7,
        kind: DataKind::Diagnostics,
    },
    ApplicationRule {
        id: "feishu.http",
        app: "feishu",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::FeishuProfiles,
        layout_id: "feishu-web-profile-v1",
        base: "LarkShell/aha/users",
        relative: "Cache/Cache_Data",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "feishu.code",
        app: "feishu",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::FeishuProfiles,
        layout_id: "feishu-web-profile-v1",
        base: "LarkShell/aha/users",
        relative: "Code Cache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "feishu.gpu",
        app: "feishu",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::FeishuProfiles,
        layout_id: "feishu-web-profile-v1",
        base: "LarkShell/aha/users",
        relative: "GPUCache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "dingtalk.http",
        app: "dingtalk",
        folder: ApplicationFolder::Local,
        profiles: ProfileLayout::DingTalkNumbered,
        layout_id: "dingtalk-web-profile-v1",
        base: "",
        relative: "Cache/Cache_Data",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "dingtalk.code",
        app: "dingtalk",
        folder: ApplicationFolder::Local,
        profiles: ProfileLayout::DingTalkNumbered,
        layout_id: "dingtalk-web-profile-v1",
        base: "",
        relative: "Code Cache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "baidu_netdisk.http",
        app: "baidu_netdisk",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::ChromiumRoot,
        layout_id: "baidu-netdisk-web-profile-v1",
        base: "baidunetdisk",
        relative: "Cache/Cache_Data",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "baidu_netdisk.code",
        app: "baidu_netdisk",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::ChromiumRoot,
        layout_id: "baidu-netdisk-web-profile-v1",
        base: "baidunetdisk",
        relative: "Code Cache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "baidu_netdisk.gpu",
        app: "baidu_netdisk",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::ChromiumRoot,
        layout_id: "baidu-netdisk-web-profile-v1",
        base: "baidunetdisk",
        relative: "GPUCache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "quark_netdisk.code",
        app: "quark_netdisk",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::ChromiumRoot,
        layout_id: "quark-netdisk-web-profile-v1",
        base: "quark-cloud-drive",
        relative: "Code Cache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "quark_netdisk.gpu",
        app: "quark_netdisk",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::ChromiumRoot,
        layout_id: "quark-netdisk-web-profile-v1",
        base: "quark-cloud-drive",
        relative: "GPUCache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "thunder.code",
        app: "thunder",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::ChromiumRoot,
        layout_id: "thunder-web-profile-v1",
        base: "thunder",
        relative: "Code Cache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
    ApplicationRule {
        id: "thunder.gpu",
        app: "thunder",
        folder: ApplicationFolder::Roaming,
        profiles: ProfileLayout::ChromiumRoot,
        layout_id: "thunder-web-profile-v1",
        base: "thunder",
        relative: "GPUCache",
        extension: None,
        age: 0,
        kind: DataKind::Cache,
    },
];

pub fn application(id: &str) -> Option<&'static Application> {
    APPLICATIONS.iter().find(|a| a.id == id)
}

/// Canonical snapshot shared by the offline checker exporter and metadata collector.
pub fn evidence_catalog() -> serde_json::Value {
    let rows: Vec<_> = APPLICATION_RULES
        .iter()
        .map(|spec| {
            serde_json::json!({
                "rule_id": spec.id, "rule_version": 1, "app_id": spec.app,
                "supported_version": application(spec.app).unwrap().version,
                "min_age_days": spec.age, "data_kind": spec.kind,
                "folder": spec.folder, "profiles": spec.profiles, "base": spec.base,
                "relative": spec.relative, "extension": spec.extension,
                "layout_id": spec.layout_id, "cleanup_validated": spec.validated(),
            })
        })
        .collect();
    serde_json::json!(rows)
}

pub fn evidence_catalog_id() -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{:x}",
        Sha256::digest(evidence_catalog().to_string().as_bytes())
    )
}
pub fn rule(id: &str) -> Option<&'static ApplicationRule> {
    APPLICATION_RULES.iter().find(|r| r.id == id)
}
pub fn valid_descriptor(r: &RuleDescriptor) -> bool {
    rule(&r.id).is_some_and(|spec| {
        r.root == spec.app
            && r.app_id.as_deref() == Some(spec.app)
            && r.relative == spec.relative
            && r.min_age_days == spec.age
            && r.data_kind == spec.kind
            && r.process.is_none()
            && !r.requires_admin
            && !r.default_selected
            && r.cleanup_policy == CleanupPolicy::Review
            && r.version == 1
    })
}
pub fn accepts_file(id: &str, path: &std::path::Path) -> bool {
    rule(id).is_none_or(|r| {
        r.extension.is_none_or(|ext| {
            path.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn application_capability_legacy_fields_never_imply_a_known_scan_layout() {
        let legacy: crate::ApplicationRuleSupport = serde_json::from_value(serde_json::json!({
            "rule_id": "feishu.http", "cleanup_validated": false,
        }))
        .unwrap();
        assert_eq!(legacy.scan_basis, crate::ScanBasis::Unverified);
        assert!(legacy.layout_id.is_none());
        assert!(legacy.cleanup_versions.is_none());
        assert_eq!(legacy.cleanup_block, Some(crate::ReasonCode::Unverifiable));
        let user = crate::rules::for_scope(crate::Scope::User).unwrap();
        assert_eq!(user.len(), 115);
        assert_eq!(
            user.iter()
                .filter(|r| r.cleanup_validated && rule(&r.id).is_none_or(|s| s.validated()))
                .count(),
            42
        );
        for r in user
            .iter()
            .filter(|r| matches!(r.root.as_str(), "feishu" | "dingtalk"))
        {
            assert_eq!(
                r.cleanup_validated,
                matches!(r.id.as_str(), "dingtalk.http" | "dingtalk.code")
            );
            assert!(!r.default_selected);
            assert_eq!(r.cleanup_policy, CleanupPolicy::Review);
        }
    }
    #[test]
    fn narrow_application_rules_and_release_gate() {
        let rules = crate::rules::catalog().unwrap();
        for spec in APPLICATION_RULES {
            let r = rules.iter().find(|r| r.id == spec.id).unwrap();
            assert!(valid_descriptor(r));
            assert_eq!(
                spec.validated(),
                matches!(spec.id, "dingtalk.http" | "dingtalk.code")
            );
            for relative in ["", "..", "User Data", "Local Storage", "backup"] {
                let mut bad = r.clone();
                bad.relative = relative.into();
                assert!(crate::rules::validate(&bad).is_err());
            }
            let mut bad = r.clone();
            bad.default_selected = true;
            assert!(crate::rules::validate(&bad).is_err());
        }
        assert!(accepts_file(
            "weixin.logs",
            std::path::Path::new("old.XLOG")
        ));
        for p in [
            "config.ini",
            "current.mmap",
            "current.mmap3",
            "old.xlog.bak",
        ] {
            assert!(!accepts_file("weixin.logs", std::path::Path::new(p)));
        }
        assert!(!accepts_file(
            "weixin.dumps",
            std::path::Path::new("state.dat")
        ));
        assert!(!accepts_file(
            "wps.logs",
            std::path::Path::new("backup.docx")
        ));
    }
}

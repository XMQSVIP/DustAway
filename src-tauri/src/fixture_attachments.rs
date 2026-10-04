//! Debug-only, in-memory attachment UI fixture. No host media is enumerated or deleted.
use anyhow::{Result, ensure};
use dustaway_core::*;
use std::sync::atomic::{AtomicBool, Ordering};
pub struct FixtureAttachments;
const ROOT: &str = r"C:\DustAway-Synthetic\xwechat_files\wxid_fixture";
fn fixture_root(provider: AttachmentProvider) -> &'static str {
    match provider {
        AttachmentProvider::Weixin => ROOT,
        AttachmentProvider::QqClassic => r"C:\DustAway-Synthetic\Tencent Files\12345678",
        AttachmentProvider::Qqnt => r"C:\DustAway-Synthetic\TencentFiles\87654321",
    }
}
impl AnalysisPlatform for FixtureAttachments {
    fn locations(&self) -> Vec<(String, String)> {
        vec![("附件界面测试".into(), ROOT.into())]
    }
    fn validate_root(&self, _: &str) -> Result<AnalysisRoot> {
        anyhow::bail!("fixture only")
    }
    fn scan(
        &self,
        _: &AnalysisRoot,
        _: &AtomicBool,
        _: &mut dyn FnMut(AnalysisEvent) -> Result<bool>,
    ) -> Result<()> {
        anyhow::bail!("fixture only")
    }
    fn reveal(&self, _: &AnalysisRoot, _: &[Vec<u8>]) -> Result<()> {
        anyhow::bail!("fixture only")
    }
    fn delete_manual(&self, _: &ManualRecord, _: &[String]) -> Result<()> {
        anyhow::bail!("UI fixture never deletes files")
    }
}
impl AttachmentPlatform for FixtureAttachments {
    fn candidate_locations(&self, provider: AttachmentProvider) -> Result<Vec<String>> {
        Ok(vec![fixture_root(provider).into()])
    }
    fn account(&self, provider: AttachmentProvider, path: &str) -> Result<AttachmentRoot> {
        ensure!(path == fixture_root(provider), "fixture account required");
        Ok(AttachmentRoot {
            location: AttachmentLocation {
                provider,
                token: String::new(),
                path: path.into(),
                title: "附件界面测试".into(),
                version: match provider {
                    AttachmentProvider::Weixin => "4.fixture",
                    AttachmentProvider::QqClassic => "9.7.fixture",
                    AttachmentProvider::Qqnt => "9.9.fixture",
                }
                .into(),
                cleanup_block: None,
            },
            opaque: format!("fixture:{}", provider.source()),
        })
    }
    fn scan_attachments(
        &self,
        root: &AttachmentRoot,
        _: &[String],
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(Result<AttachmentRecord>) -> Result<()>,
    ) -> Result<()> {
        for i in 0..205u64 {
            ensure!(!cancel.load(Ordering::Acquire), "scan cancelled");
            let kind = if i == 0 {
                AttachmentKind::Unknown
            } else if i % 2 == 0 {
                AttachmentKind::Image
            } else {
                AttachmentKind::File
            };
            let name = format!(
                "测试附件-{i:03}.{}",
                if kind == AttachmentKind::Image {
                    "jpg"
                } else if kind == AttachmentKind::File {
                    "zip"
                } else {
                    "dat"
                }
            );
            let path = format!(r"{}\{name}", root.location.path);
            let bytes = (205 - i) * 1024;
            let modified = now() - 100 * 86400;
            emit(Ok(AttachmentRecord {
                entry: AttachmentEntry {
                    id: String::new(),
                    name,
                    path: path.clone(),
                    kind,
                    bytes: bytes.to_string(),
                    modified_unix: modified,
                    preview_available: kind == AttachmentKind::Image,
                    cleanup_block: (kind == AttachmentKind::Unknown)
                        .then(|| "未知格式，已保留".into()),
                    outcome: None,
                },
                record: ManualRecord {
                    file: FileRecord {
                        path_display: path,
                        bytes,
                        modified_unix: modified,
                        opaque: root.opaque.clone(),
                    },
                    directory: false,
                    identity: format!("fixture-{i}"),
                },
            }))?;
        }
        Ok(())
    }
    fn verify_attachment(
        &self,
        root: &AttachmentRoot,
        item: &ManualRecord,
        _: &[String],
    ) -> Result<()> {
        ensure!(
            root.opaque == item.file.opaque
                && root.opaque == format!("fixture:{}", root.location.provider.source()),
            "fixture only"
        );
        Ok(())
    }
    fn attachment_thumbnail(&self, item: &ManualRecord) -> Result<String> {
        ensure!(item.file.opaque.starts_with("fixture:"), "fixture only");
        if item.identity == "fixture-4" {
            anyhow::bail!("模拟缩略图解码失败");
        }
        Ok("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aD1sAAAAASUVORK5CYII=".into())
    }
    fn reveal_attachment(&self, item: &ManualRecord) -> Result<()> {
        ensure!(item.file.opaque.starts_with("fixture:"), "fixture only");
        Ok(())
    }
}

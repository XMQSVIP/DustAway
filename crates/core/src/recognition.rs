//! Offline, descriptive knowledge only. Never supplies cleanup rules or permissions.
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recognition {
    pub rule_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<Category>,
    pub title: String,
    pub description: String,
    pub guidance: String,
    pub evidence: String,
    pub source: String,
    pub reviewed: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    revision: String,
    rules: Vec<Rule>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    id: String,
    category: Category,
    matcher: Matcher,
    patterns: Vec<String>,
    /// Any of these complete directory sequences must be an ancestor of the match.
    #[serde(default)]
    within: Vec<String>,
    #[serde(default = "default_priority")]
    priority: u16,
    title: String,
    description: String,
    guidance: String,
    source: String,
    reviewed: String,
}
fn default_priority() -> u16 {
    100
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Windows,
    Development,
    Ai,
    Virtualization,
    Games,
    Applications,
    Media,
    Documents,
    Data,
    Archives,
}
impl Category {
    pub const ALL: [Self; 10] = [
        Self::Windows,
        Self::Development,
        Self::Ai,
        Self::Virtualization,
        Self::Games,
        Self::Applications,
        Self::Media,
        Self::Documents,
        Self::Data,
        Self::Archives,
    ];
    pub fn key(self) -> &'static str {
        [
            "windows",
            "development",
            "ai",
            "virtualization",
            "games",
            "applications",
            "media",
            "documents",
            "data",
            "archives",
        ][self as usize]
    }
    pub fn code(self) -> u8 {
        self as u8 + 1
    }
}
#[derive(Clone, Copy)]
pub struct Classification {
    pub rule: u32,
    pub category: Category,
}
#[derive(Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
enum Matcher {
    Extension,
    DirectorySequence,
    DriveRootFile,
    FileName,
}

#[derive(Clone, Copy)]
struct Hit {
    rule: usize,
    pattern: usize,
}
#[derive(Default)]
struct PathIndex {
    children: HashMap<String, PathIndex>,
    hits: Vec<Hit>,
}
impl PathIndex {
    fn insert(&mut self, pattern: &str, hit: Hit) {
        let mut node = self;
        for component in pattern.split('/') {
            node = node.children.entry(component.into()).or_default();
        }
        node.hits.push(hit);
    }
    fn visit(&self, parts: &[&str], mut emit: impl FnMut(Hit, usize)) {
        let mut pending = Vec::new();
        for start in 0..parts.len() {
            pending.push((self, start));
            while let Some((node, position)) = pending.pop() {
                let component = parts[position];
                let version_key = component.rsplit_once('_').and_then(|(prefix, version)| {
                    numeric_version(version).then(|| format!("{prefix}_{{version}}"))
                });
                for next in [
                    node.children
                        .get(component)
                        .filter(|_| !component.contains("{version}")),
                    version_key.as_ref().and_then(|key| node.children.get(key)),
                ]
                .into_iter()
                .flatten()
                {
                    for hit in &next.hits {
                        emit(*hit, start);
                    }
                    if position + 1 < parts.len() {
                        pending.push((next, position + 1));
                    }
                }
            }
        }
    }
}
struct Library {
    rules: Vec<Rule>,
    extensions: HashMap<String, Vec<Hit>>,
    filenames: HashMap<String, Vec<Hit>>,
    rootfiles: HashMap<String, Vec<Hit>>,
    paths: PathIndex,
}
fn sequence_in(pattern: &str, parts: &[&str]) -> bool {
    let sequence: Vec<_> = pattern.split('/').collect();
    parts.windows(sequence.len()).any(|window| {
        window.iter().zip(&sequence).all(|(part, expected)| {
            part == expected
                || expected
                    .strip_suffix("{version}")
                    .is_some_and(|prefix| part.strip_prefix(prefix).is_some_and(numeric_version))
        })
    })
}
fn numeric_version(value: &str) -> bool {
    value
        .split('.')
        .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}
fn valid_sequence(value: &str) -> bool {
    !value.is_empty()
        && value == value.to_lowercase()
        && !value.contains(['\\', ':', '*', '?'])
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && (!part.contains(['{', '}'])
                    || part
                        .strip_suffix("_{version}")
                        .is_some_and(|prefix| !prefix.is_empty() && !prefix.contains(['{', '}'])))
        })
}
impl Library {
    fn load(json: &str) -> Result<Self, String> {
        let catalog: Catalog = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if catalog.schema_version != 1 || catalog.revision.is_empty() || catalog.rules.is_empty() {
            return Err("unsupported or empty recognition catalog".into());
        }
        let mut ids = HashSet::new();
        let mut selectors = HashSet::new();
        for rule in &catalog.rules {
            if rule.id.is_empty()
                || !ids.insert(&rule.id)
                || rule.patterns.is_empty()
                || rule.title.is_empty()
                || rule.description.is_empty()
                || rule.guidance.is_empty()
                || !rule.source.starts_with("https://")
                || rule.reviewed.len() != 10
            {
                return Err(format!("invalid or duplicate rule: {}", rule.id));
            }
            let mut within = rule.within.clone();
            within.sort();
            within.dedup();
            if within.iter().any(|p| !valid_sequence(p)) || within.len() != rule.within.len() {
                return Err(format!("invalid context: {}", rule.id));
            }
            for pattern in &rule.patterns {
                if !valid_sequence(pattern)
                    || (rule.matcher != Matcher::DirectorySequence
                        && pattern.contains(['/', '{', '}']))
                    || (rule.matcher == Matcher::Extension
                        && (!pattern.starts_with('.') || pattern.len() < 2))
                    || !selectors.insert((&rule.matcher, pattern, within.clone()))
                {
                    return Err(format!(
                        "invalid or duplicate selector: {} / {pattern}",
                        rule.id
                    ));
                }
            }
        }
        let mut library = Self {
            rules: catalog.rules,
            extensions: HashMap::new(),
            filenames: HashMap::new(),
            rootfiles: HashMap::new(),
            paths: PathIndex::default(),
        };
        for (rule, item) in library.rules.iter().enumerate() {
            for (pattern, value) in item.patterns.iter().enumerate() {
                let hit = Hit { rule, pattern };
                match item.matcher {
                    Matcher::Extension => library
                        .extensions
                        .entry(value.clone())
                        .or_default()
                        .push(hit),
                    Matcher::FileName => library
                        .filenames
                        .entry(value.clone())
                        .or_default()
                        .push(hit),
                    Matcher::DriveRootFile => library
                        .rootfiles
                        .entry(value.clone())
                        .or_default()
                        .push(hit),
                    Matcher::DirectorySequence => library.paths.insert(value, hit),
                }
            }
        }
        Ok(library)
    }
    fn resolve(&self, path: &str, directory: bool) -> Option<(Hit, &str)> {
        let normalized = path
            .trim_start_matches(r"\\?\")
            .replace('/', "\\")
            .to_lowercase();
        let parts: Vec<_> = normalized.split('\\').filter(|p| !p.is_empty()).collect();
        if parts.iter().any(|p| *p == "." || *p == "..") {
            return None;
        }
        let name = *parts.last()?;
        let directories = if directory {
            &parts[..]
        } else {
            &parts[..parts.len() - 1]
        };
        let mut best: Option<(Hit, &str)> = None;
        let rank = |hit: Hit, context: &str| {
            let rule = &self.rules[hit.rule];
            let tier = match rule.matcher {
                Matcher::DriveRootFile => 0,
                _ if !context.is_empty() => 1,
                Matcher::DirectorySequence => 2,
                Matcher::FileName => 3,
                Matcher::Extension => 4,
            };
            (
                rule.priority,
                tier,
                rule.patterns[hit.pattern].matches("{version}").count()
                    + context.matches("{version}").count(),
                Reverse(rule.patterns[hit.pattern].len() + context.len()),
                &rule.id,
            )
        };
        let mut consider = |hit: Hit, start: usize| {
            let rule = &self.rules[hit.rule];
            let context = if rule.within.is_empty() {
                ""
            } else {
                let Some(context) = rule
                    .within
                    .iter()
                    .filter(|p| sequence_in(p, &directories[..start]))
                    .max_by_key(|p| (p.len(), p.as_str()))
                else {
                    return;
                };
                context.as_str()
            };
            if best.is_none_or(|(old, old_context)| rank(hit, context) < rank(old, old_context)) {
                best = Some((hit, context));
            }
        };
        self.paths.visit(directories, &mut consider);
        if !directory {
            if let Some(hits) = self.filenames.get(name) {
                for hit in hits {
                    consider(*hit, directories.len());
                }
            }
            if parts.len() == 2
                && normalized.as_bytes().get(1) == Some(&b':')
                && normalized.as_bytes()[0].is_ascii_alphabetic()
                && let Some(hits) = self.rootfiles.get(name)
            {
                for hit in hits {
                    consider(*hit, directories.len());
                }
            }
            // Probe each suffix, including compound extensions, without scanning the catalog.
            for (offset, _) in name.match_indices('.') {
                if let Some(hits) = self.extensions.get(&name[offset..]) {
                    for hit in hits {
                        consider(*hit, directories.len());
                    }
                }
            }
        }
        best
    }
    fn describe(&self, path: &str, directory: bool) -> Option<Recognition> {
        let (hit, context) = self.resolve(path, directory)?;
        let rule = &self.rules[hit.rule];
        let pattern = &rule.patterns[hit.pattern];
        let mut evidence = match rule.matcher {
            Matcher::Extension => format!("文件扩展名 {pattern}；未检查文件内容"),
            Matcher::DirectorySequence => format!("路径目录片段 {pattern}；未验证所属应用"),
            Matcher::DriveRootFile => format!("磁盘根目录文件名 {pattern}；未验证系统占用"),
            Matcher::FileName => format!("完整文件名 {pattern}；未检查文件内容"),
        };
        if !context.is_empty() {
            evidence.push_str(&format!("；同时匹配目录 {context}"));
        }
        Some(Recognition {
            rule_id: rule.id.clone(),
            category: Some(rule.category),
            title: rule.title.clone(),
            description: rule.description.clone(),
            guidance: rule.guidance.clone(),
            evidence,
            source: rule.source.clone(),
            reviewed: rule.reviewed.clone(),
        })
    }
}
static LIBRARY: LazyLock<Library> = LazyLock::new(|| {
    Library::load(include_str!("../../../rules/recognition.json"))
        .expect("invalid bundled recognition catalog")
});
pub fn recognize(path: &str, directory: bool) -> Option<Recognition> {
    LIBRARY.describe(path, directory)
}
/// Uses exactly the same winning match without allocating explanation strings.
pub fn classify(path: &str, directory: bool) -> Option<Classification> {
    let (hit, _) = LIBRARY.resolve(path, directory)?;
    Some(Classification {
        rule: hit.rule as u32 + 1,
        category: LIBRARY.rules[hit.rule].category,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_is_reviewed_and_covers_every_category() {
        let categories: HashSet<_> = LIBRARY.rules.iter().map(|r| &r.category).collect();
        assert_eq!(categories.len(), 10);
        assert!(LIBRARY.rules.len() >= 150);
    }
    #[test]
    fn every_pattern_and_combined_condition_is_reachable() {
        for rule in &LIBRARY.rules {
            for pattern in &rule.patterns {
                let contexts = if rule.within.is_empty() {
                    vec![""]
                } else {
                    rule.within.iter().map(String::as_str).collect()
                };
                for context in contexts {
                    let prefix = if context.is_empty() {
                        "D:/资料".into()
                    } else {
                        format!("D:/资料/{context}")
                    };
                    let pattern = pattern.replace("{version}", "134");
                    let prefix = prefix.replace("{version}", "134");
                    let (path, directory) = match rule.matcher {
                        Matcher::Extension => (format!("{prefix}/file{pattern}"), false),
                        Matcher::FileName => (format!("{prefix}/{pattern}"), false),
                        Matcher::DirectorySequence => (format!("{prefix}/{pattern}"), true),
                        Matcher::DriveRootFile => (format!("D:/{pattern}"), false),
                    };
                    assert_eq!(
                        recognize(&path.to_uppercase(), directory).unwrap().rule_id,
                        rule.id,
                        "{path}"
                    );
                    let compact = classify(&path.to_uppercase(), directory).unwrap();
                    assert_eq!(LIBRARY.rules[compact.rule as usize - 1].id, rule.id);
                    assert_eq!(compact.category, rule.category);
                    assert_eq!(
                        recognize(&path, directory).unwrap().category,
                        Some(rule.category)
                    );
                    if directory {
                        assert_eq!(
                            recognize(&format!("{path}/child/untyped"), false)
                                .unwrap()
                                .rule_id,
                            rule.id,
                            "{path}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn boundaries_types_and_ambiguous_names() {
        for (path, directory) in [
            (r"D:\Cache\WeChat", true),
            (r"D:\target", true),
            (r"D:\data", true),
            (r"D:\model.bin", false),
            (r"D:\file.pt", false),
            (r"D:\file.pth", false),
            (r"D:\media cache files-old\file.cfa", false),
            (r"D:\file.cfa", false),
            (r"D:\model.gguf", true),
            (r"D:\model.gguf.bak", false),
            (r"D:\my_node_modules\blob", false),
            (r"D:\copy\pagefile.sys", false),
            (r"D:\node_modules", false),
            (r"\\server\pagefile.sys", false),
            (r"D:\普通资料\data.mdb", false),
            (r"D:\fake-steamapps\common\blob", false),
            (r"D:\node_modules\..\private", false),
            (r"D:\AppData\Roaming\Mozilla\sccache\config\untyped", false),
            (r"D:\my.minecraft\data.dat", false),
            (r"D:\Factorio\saves-old\data.dat", false),
            (r"D:\Pub\Cache\data.bin", false),
            (r"D:\.nuxt-old\data.bin", false),
            (r"D:\scene.glb.bak", false),
            (r"D:\backup.tibx", true),
            (r"D:\photo.lrcat", true),
            (r"D:\AppData\Local\Cypress\Cache", false),
            (r"D:\build\report.dat", false),
            (r"D:\library\asset.bin", false),
            (r"D:\temp\record.dat", false),
            (r"D:\unpackage\dist-old\data.bin", false),
            (r"D:\temp\asset-db-old\assets\data.bin", false),
            (r"D:\my_oh_modules\data.bin", false),
            (r"D:\Foxmail\Storage-old\mail.dat", false),
            (r"D:\report.wps.bak", false),
            (r"D:\draft.spp", true),
            (r"D:\project\library\data.har", false),
            ("", false),
        ] {
            assert!(recognize(path, directory).is_none(), "{path}");
            assert!(classify(path, directory).is_none(), "{path}");
        }
        assert_eq!(
            recognize(r"\\?\D:\Models\MODEL.GGUF", false)
                .unwrap()
                .rule_id,
            "model.gguf"
        );
        assert_eq!(
            recognize("D:/project/node_modules/library/data.bin", false)
                .unwrap()
                .rule_id,
            "dev.node_modules"
        );
        assert_eq!(
            recognize(r"D:\pagefile.sys", false).unwrap().rule_id,
            "windows.pagefile"
        );
    }
    #[test]
    fn context_and_protection_win_over_generic_extensions() {
        for (path, expected) in [
            (r"C:\Windows\Installer\package.msi", "windows.installer"),
            (r"D:\repo\.git\objects\media.mp4", "dev.git"),
            (r"D:\Docker\wsl\data\ext4.vhdx", "vm.docker_wsl"),
            (r"D:\wsl\ext4.vhdx", "vm.wsl"),
            (
                r"D:\SteamLibrary\steamapps\common\Game\movie.mp4",
                "game.steam_installed",
            ),
            (
                r"D:\ComfyUI\models\checkpoints\model.safetensors",
                "model.comfy_checkpoints",
            ),
            (r"D:\repo\archive.tar.gz", "archive.tar"),
            (r"D:\training\model.weights.h5", "model.keras_weights"),
            (r"D:\experiment\results.h5", "data.hdf5"),
            (r"D:\repo\.git\lfs\objects\model.weights.h5", "dev.git"),
            (r"C:\Windows\Installer\bundle.msix", "windows.installer"),
            (
                r"D:\.minecraft\saves\world\region\data.dat",
                "game.minecraft_java",
            ),
            (r"D:\.minecraft\versions\game.jar", "game.minecraft_java"),
            (r"D:\Factorio\saves\autosave.zip", "game.factorio_saves"),
            (r"D:\.m2\repository\package.jar", "dev.maven"),
            (
                r"D:\.cache\torch\hub\checkpoints\network.pth",
                "model.torch_hub",
            ),
            (
                r"D:\.cache\huggingface\xet\chunk_cache\data.bin",
                "model.huggingface_xet",
            ),
            (
                r"C:\Users\test\AppData\Local\Cypress\Cache\15\Cypress.exe",
                "dev.cypress",
            ),
            (
                r"C:\Users\test\AppData\Roaming\JetBrains\Idea\plugins\plugin.jar",
                "dev.jetbrains_settings",
            ),
        ] {
            assert_eq!(recognize(path, false).unwrap().rule_id, expected, "{path}");
        }
        let info = recognize(r"C:\Users\test\AppData\Local\uv\cache\data.bin", false).unwrap();
        assert_eq!(info.rule_id, "dev.uv");
    }
    #[test]
    fn domestic_workflows_preserve_backups_sources_and_specific_formats() {
        for (path, expected) in [
            (r"D:\Kingsoft\office6\backup\恢复文档.wps", "app.wps_backup"),
            (
                r"D:\Documents\TencentMeeting\meeting.wemtv.idx",
                "app.tencent_meeting_raw",
            ),
            (
                r"D:\Documents\TencentMeeting\meeting.mp4",
                "app.tencent_meeting_recordings",
            ),
            (r"D:\Downloads\meeting.wemta.idx", "app.tencent_meeting_raw"),
            (
                r"D:\repo\unpackage\dist\build\app-harmony\entry.hap",
                "dev.uniapp_output",
            ),
            (
                r"D:\repo\unpackage\debug\android_debug.apk",
                "dev.uniapp_debug",
            ),
            (
                r"D:\repo\uni_modules\my-plugin\node_modules\lib\data.bin",
                "dev.uni_modules",
            ),
            (r"D:\repo\.git\objects\mockup.fig", "dev.git"),
            (r"D:\repo\oh_modules\package\entry.hap", "dev.oh_modules"),
            (
                r"D:\repo\temp\asset-db\assets\uuid\build\image.png",
                "dev.cocos_asset_cache",
            ),
            (
                r"D:\Foxmail7.1\Storage\mailbox\data.db",
                "app.foxmail_legacy",
            ),
            (r"D:\repo\.vs\project\browse.db", "dev.visual_studio"),
        ] {
            assert_eq!(recognize(path, false).unwrap().rule_id, expected, "{path}");
        }
    }
    #[test]
    fn language_sources_do_not_hide_environments_and_dependencies() {
        for (path, expected) in [
            (r"D:\project\main.py", "dev.source_python"),
            (r"D:\project\main.pyc", "dev.python_bytecode"),
            (
                r"D:\project\.venv\Lib\site-packages\pkg\main.py",
                "dev.python",
            ),
            (
                r"D:\Python\Lib\site-packages\pkg\main.py",
                "dev.python_packages",
            ),
            (r"D:\project\__pycache__\main.pyc", "dev.python_pycache"),
            (
                r"C:\Users\test\AppData\Local\uv\cache\archive-v0\pkg\main.py",
                "dev.uv",
            ),
            (
                r"C:\Users\test\AppData\Roaming\uv\data\python\cpython\Lib\site-packages\pkg\main.py",
                "dev.uv_python",
            ),
            (
                r"D:\.local\share\uv\tools\ruff\Lib\site-packages\pkg\main.py",
                "dev.uv_tools",
            ),
            (
                r"D:\pypoetry\cache\virtualenvs\env\Lib\site-packages\pkg\main.py",
                "dev.poetry_envs",
            ),
            (
                r"D:\pypoetry\cache\artifacts\wheel.whl",
                "dev.poetry_storage",
            ),
            (r"D:\repo\node_modules\library\index.ts", "dev.node_modules"),
            (r"D:\repo\types.d.ts", "dev.source_typescript"),
            (r"D:\recordings\video.ts", "dev.source_ts"),
            (r"D:\repo\.git\objects\source.rs", "dev.git"),
            (r"D:\repo\uv.lock", "dev.uv_project"),
        ] {
            assert_eq!(recognize(path, false).unwrap().rule_id, expected, "{path}");
        }
        for (path, directory) in [
            (r"D:\uv\data\python-old\data.bin", false),
            (r"D:\uv\data\credentials\data.bin", false),
            (r"D:\project\main.py.bak", false),
            (r"D:\project\uv.lock", true),
            (r"D:\project\main.py", true),
            (r"D:\project\__pycache__-old\data.bin", false),
        ] {
            assert!(recognize(path, directory).is_none(), "{path}");
        }
    }
    #[test]
    fn domestic_private_data_is_not_mislabeled_as_generic_cache() {
        for (path, expected) in [
            (
                r"E:\moved\xwechat_files\account\db_storage\message\message.db",
                "app.weixin_database",
            ),
            (
                r"E:\moved\xwechat_files\account\msg\attach\hash\image.dat",
                "app.weixin_attachments",
            ),
            (
                r"E:\moved\xwechat_files\account\msg\file\budget.xlsx",
                "app.weixin_files",
            ),
            (
                r"E:\moved\xwechat_files\account\msg\video\clip.mp4",
                "app.weixin_video",
            ),
            (
                r"E:\moved\xwechat_files\account\msg\migrate\untyped",
                "app.weixin_data",
            ),
            (
                r"C:\Users\test\AppData\Roaming\Tencent\xwechat\XPlugin\plugins\WeChatBackup\1\extracted\WeChatBackup.exe",
                "app.weixin_plugins",
            ),
            (
                r"C:\Users\test\AppData\Roaming\Tencent\xwechat\log\trace.xlog",
                "app.weixin_logs",
            ),
            (
                r"C:\Users\test\AppData\Roaming\Tencent\xwechat\crashinfo\reports\crash.dmp",
                "app.weixin_crash",
            ),
            (
                r"D:\Documents\Tencent Files\1234\Video\clip.mp4",
                "app.qq_data",
            ),
            (
                r"C:\Users\test\AppData\Roaming\Tencent\QQ\webkitex_cache\1234\Cache\Cache_Data\data_1",
                "app.qq_web_cache",
            ),
            (
                r"C:\Users\test\AppData\Roaming\Tencent\QQ\webkitex_cache\1234\Local Storage\db",
                "app.qq_web_profile",
            ),
            (
                r"C:\Users\test\AppData\Roaming\LarkShell\aha\users\account\profile_main\Code Cache\js\entry",
                "app.feishu_web_cache",
            ),
            (
                r"C:\Users\test\AppData\Roaming\LarkShell\aha\users\account\profile_explorer\IndexedDB\data.db",
                "app.feishu_profile",
            ),
            (
                r"C:\Users\test\AppData\Roaming\LarkShell\sdk_storage\log\xlog\trace",
                "app.feishu_logs",
            ),
            (
                r"C:\Users\test\AppData\Local\DingTalk_133\Default\Cache\Cache_Data\entry",
                "app.dingtalk_web_cache",
            ),
            (
                r"C:\Users\test\AppData\Local\DingTalk_133\Default\Local Storage\db",
                "app.dingtalk_web_profile",
            ),
            (
                r"C:\Users\test\AppData\Roaming\dinglive\logs\record.txt",
                "app.dingtalk_live_logs",
            ),
            (
                r"D:\Documents\WXWork\1234\Cache\File\2026-09\budget.xlsx",
                "app.wecom_received",
            ),
            (
                r"D:\Documents\WXWork\1234\Data\database.db",
                "app.wecom_data",
            ),
            (
                r"D:\JianyingPro\User Data\Projects\com.lveditor.draft\demo\assets\clip.mp4",
                "app.jianying_drafts",
            ),
            (
                r"D:\custom-drafts\demo\draft_content.json",
                "app.jianying_draft_file",
            ),
            (r"D:\repo\.git\objects\draft_content.json", "dev.git"),
            // A context must precede its match: an unrelated ancestor named Cache
            // must not turn the entire work profile into a received-file directory.
            (
                r"D:\Cache\Documents\WXWork\1234\Data\untyped",
                "app.wecom_data",
            ),
            (
                r"D:\db_storage\xwechat_files\account\untyped",
                "app.weixin_data",
            ),
            (
                r"D:\profile_main\Cache\AppData\Roaming\LarkShell\aha\users\account\untyped",
                "app.feishu_profile",
            ),
        ] {
            assert_eq!(recognize(path, false).unwrap().rule_id, expected, "{path}");
        }
        for (path, directory) in [
            (r"D:\xwechat_files-old\account\db_storage\blob", false),
            (r"D:\ordinary\msg\attach\blob.dat", false),
            (
                r"D:\AppData\Roaming\LarkShell-old\aha\users\a\profile_main\Cache\entry",
                false,
            ),
            (r"D:\ordinary\profile_main\Cache\entry", false),
            (
                r"D:\AppData\Local\DingTalk_backup\Default\Cache\entry",
                false,
            ),
            (r"D:\Documents\WXWork-old\Cache\entry", false),
            (
                r"D:\JianyingPro\User Data\Projects\com.lveditor.draft-old\entry",
                false,
            ),
            (r"D:\draft_content.json.bak", false),
            (r"D:\draft_content.json", true),
            (r"D:\xwechat_files", false),
        ] {
            assert!(recognize(path, directory).is_none(), "{path}");
        }
    }
    #[test]
    fn versioned_directories_follow_structure_without_a_version_allowlist() {
        for version in ["1", "99", "133", "134", "2026", "8.5.5"] {
            for (suffix, expected) in [
                (r"Default\Cache\entry", "app.dingtalk_web_cache"),
                (r"Default\Code Cache\js\entry", "app.dingtalk_web_cache"),
                (r"Default\IndexedDB\database.db", "app.dingtalk_web_profile"),
            ] {
                let path = format!(r"C:\Users\test\AppData\Local\DingTalk_{version}\{suffix}");
                assert_eq!(recognize(&path, false).unwrap().rule_id, expected);
            }
        }
        for version in [
            "",
            "backup",
            "133-old",
            "133evil",
            "133..1",
            ".133",
            "133.",
            "１２３",
            "{version}",
        ] {
            let path = format!(r"D:\AppData\Local\DingTalk_{version}\Default\Cache\entry");
            assert!(recognize(&path, false).is_none(), "{path}");
        }
        assert!(sequence_in(
            "appdata/local/dingtalk_{version}",
            &["appdata", "local", "dingtalk_134"]
        ));
        assert!(!sequence_in(
            "appdata/local/dingtalk_{version}",
            &["appdata", "local", "dingtalk_backup"]
        ));
        let mut data: serde_json::Value =
            serde_json::from_str(include_str!("../../../rules/recognition.json")).unwrap();
        let mut exact = data["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "app.dingtalk_web_cache")
            .unwrap()
            .clone();
        exact["id"] = "test.exact".into();
        exact["patterns"] = serde_json::json!(["appdata/local/dingtalk_134/default/cache"]);
        data["rules"].as_array_mut().unwrap().push(exact);
        let path = r"D:\AppData\Local\DingTalk_134\Default\Cache\entry";
        assert_eq!(
            Library::load(&data.to_string())
                .unwrap()
                .describe(path, false)
                .unwrap()
                .rule_id,
            "test.exact"
        );
        data["rules"].as_array_mut().unwrap().reverse();
        assert_eq!(
            Library::load(&data.to_string())
                .unwrap()
                .describe(path, false)
                .unwrap()
                .rule_id,
            "test.exact"
        );
        for invalid in [
            "dingtalk_{any}",
            "{version}",
            "dingtalk_{version}-old",
            "{x}_{version}",
        ] {
            let mut bad = data.clone();
            bad["rules"][0]["patterns"] = serde_json::json!([invalid]);
            assert!(Library::load(&bad.to_string()).is_err(), "{invalid}");
        }
    }

    #[test]
    fn netdisk_explanations_keep_cache_leaves_separate_from_persistent_data() {
        for (base, id) in [
            ("baidunetdisk", "baidu_netdisk"),
            ("quark-cloud-drive", "quark_netdisk"),
            ("thunder", "thunder"),
        ] {
            let root = format!(r"C:\Users\fixture\AppData\Roaming\{base}");
            for (suffix, kind) in [
                (r"Code Cache\js\blob", "web_cache"),
                (r"GPUCache\blob", "web_cache"),
                (r"Local Storage\state", "storage"),
                (r"Network\Cookies", "storage"),
                (r"Cache\untyped", "profile"),
            ] {
                let result = recognize(&format!(r"{root}\{suffix}"), false).unwrap();
                assert_eq!(result.rule_id, format!("app.{id}_{kind}"));
            }
            assert_eq!(
                recognize(&root, true).unwrap().rule_id,
                format!("app.{id}_profile")
            );
            assert!(recognize(&format!(r"{root}-backup\Code Cache\untyped"), false).is_none());
        }
        assert_eq!(
            recognize(r"D:\Downloads\movie.mp4.xltd", false)
                .unwrap()
                .rule_id,
            "app.thunder_partial"
        );
        for path in [
            r"D:\Downloads\untyped",
            r"D:\Cache\untyped",
            r"D:\file.dat",
            r"D:\file.cfg",
            r"D:\file.xltd.bak",
        ] {
            assert!(recognize(path, false).is_none(), "{path}");
        }
        assert!(recognize(r"D:\folder.xltd", true).is_none());
    }
    #[test]
    fn malformed_catalogs_are_rejected() {
        let original: serde_json::Value =
            serde_json::from_str(include_str!("../../../rules/recognition.json")).unwrap();
        let mut bad = original.clone();
        bad["schema_version"] = 2.into();
        assert!(Library::load(&bad.to_string()).is_err());
        let mut bad = original.clone();
        bad["rules"][0]["patterns"][0] = "".into();
        assert!(Library::load(&bad.to_string()).is_err());
        let mut bad = original.clone();
        bad["rules"][0]["within"] = serde_json::json!(["foo//bar"]);
        assert!(Library::load(&bad.to_string()).is_err());
        let mut bad = original.clone();
        bad["rules"][0]["surprise"] = true.into();
        assert!(Library::load(&bad.to_string()).is_err());
        let mut bad = original.clone();
        let duplicate = bad["rules"][0].clone();
        bad["rules"].as_array_mut().unwrap().push(duplicate);
        assert!(Library::load(&bad.to_string()).is_err());
        let mut bad = original.clone();
        let mut duplicate_selector = bad["rules"][0].clone();
        duplicate_selector["id"] = "different.id".into();
        bad["rules"]
            .as_array_mut()
            .unwrap()
            .push(duplicate_selector);
        assert!(Library::load(&bad.to_string()).is_err());
    }
    #[test]
    fn catalog_order_does_not_change_results() {
        let mut data: serde_json::Value =
            serde_json::from_str(include_str!("../../../rules/recognition.json")).unwrap();
        data["rules"].as_array_mut().unwrap().reverse();
        let reversed = Library::load(&data.to_string()).unwrap();
        for (path, dir) in [
            (r"D:\repo\.git\objects\file.zip", false),
            (r"C:\Windows\Installer\setup.msi", false),
            (r"D:\ComfyUI\models\checkpoints\model.safetensors", false),
            (r"D:\.gradle\caches", true),
            (r"D:\training\model.weights.h5", false),
            (r"D:\.minecraft\versions\game.jar", false),
        ] {
            assert_eq!(
                reversed.describe(path, dir).unwrap().rule_id,
                recognize(path, dir).unwrap().rule_id
            );
        }
    }
    #[test]
    fn unrelated_rules_do_not_grow_lookup_candidates() {
        // Structural scalability regression: unrelated keys never enter the candidate buckets.
        let mut data: serde_json::Value =
            serde_json::from_str(include_str!("../../../rules/recognition.json")).unwrap();
        let template = data["rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == "model.gguf")
            .unwrap()
            .clone();
        for n in 0..10_000 {
            let mut rule = template.clone();
            rule["id"] = format!("synthetic.{n}").into();
            rule["patterns"] = serde_json::json!([format!(".synthetic{n}")]);
            data["rules"].as_array_mut().unwrap().push(rule);
        }
        let large = Library::load(&data.to_string()).unwrap();
        assert_eq!(large.extensions.get(".gguf").unwrap().len(), 1);
        assert!(!large.extensions.contains_key(".unmatched"));
        assert_eq!(
            large.describe(r"D:\model.gguf", false).unwrap().rule_id,
            "model.gguf"
        );
    }
}

//! Trusted location explanations, built once from Windows Known Folders.
//! No disk reads or filename guesses are performed while describing a page.
use crate::{fs::display_path, platform::known_folder};
use dustaway_core::{AnalysisAdvice, AnalysisEntry, AnalysisPurpose, browsers::CHROMIUM_PRODUCTS};
use windows::Win32::UI::Shell::*;

#[derive(Default)]
pub(crate) struct PurposeMap {
    browsers: Vec<(String, String)>,
    roots: Vec<(String, AnalysisPurpose, &'static str)>,
    mixed: Vec<String>,
}

fn normalize(path: &str) -> String {
    path.trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}
fn contains(root: &str, path: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|tail| tail.starts_with('\\'))
}
impl PurposeMap {
    pub(crate) fn new() -> Self {
        let mut map = Self::default();
        if let Ok(profile) = known_folder(&FOLDERID_Profile) {
            map.mixed.push(normalize(&display_path(&profile)));
        }
        if let Ok(local) = known_folder(&FOLDERID_LocalAppData) {
            for product in CHROMIUM_PRODUCTS {
                map.browsers.push((
                    normalize(&display_path(&local.join(product.user_data))),
                    product.id.into(),
                ));
            }
            map.browsers.push((
                normalize(&display_path(&local.join("Mozilla/Firefox"))),
                "firefox".into(),
            ));
        }
        if let Ok(roaming) = known_folder(&FOLDERID_RoamingAppData) {
            map.browsers.push((
                normalize(&display_path(&roaming.join("Mozilla/Firefox"))),
                "firefox".into(),
            ));
        }
        // The order is deliberate. A personal folder redirected into application/system
        // data must not turn that data into a recommendation to organize personal files.
        for (id, kind, basis) in [
            (
                FOLDERID_LocalAppDataLow,
                AnalysisPurpose::AppData,
                "known_local_low_app_data",
            ),
            (
                FOLDERID_LocalAppData,
                AnalysisPurpose::AppData,
                "known_local_app_data",
            ),
            (
                FOLDERID_RoamingAppData,
                AnalysisPurpose::AppData,
                "known_roaming_app_data",
            ),
            (
                FOLDERID_Windows,
                AnalysisPurpose::SystemData,
                "known_windows",
            ),
            (
                FOLDERID_ProgramFiles,
                AnalysisPurpose::SystemData,
                "known_program_files",
            ),
            (
                FOLDERID_ProgramFilesX86,
                AnalysisPurpose::SystemData,
                "known_program_files",
            ),
            (
                FOLDERID_ProgramData,
                AnalysisPurpose::SystemData,
                "known_program_data",
            ),
            (
                FOLDERID_Downloads,
                AnalysisPurpose::Personal,
                "known_downloads",
            ),
            (
                FOLDERID_Documents,
                AnalysisPurpose::Personal,
                "known_documents",
            ),
            (FOLDERID_Desktop, AnalysisPurpose::Personal, "known_desktop"),
            (
                FOLDERID_Pictures,
                AnalysisPurpose::Personal,
                "known_pictures",
            ),
            (FOLDERID_Music, AnalysisPurpose::Personal, "known_music"),
            (FOLDERID_Videos, AnalysisPurpose::Personal, "known_videos"),
        ] {
            if let Ok(path) = known_folder(&id) {
                map.roots
                    .push((normalize(&display_path(&path)), kind, basis));
            }
        }
        map
    }
    pub(crate) fn describe(&self, entry: &AnalysisEntry) -> AnalysisAdvice {
        if !entry.rule_ids.is_empty() {
            return AnalysisAdvice {
                kind: AnalysisPurpose::Cache,
                app_id: None,
                basis: "verified_rule_location".into(),
                recognition: None,
                model_hint: None,
            };
        }
        let path = normalize(&entry.path);
        if self.mixed.contains(&path) {
            return AnalysisAdvice::default();
        }
        if let Some((_, app)) = self.browsers.iter().find(|(root, _)| contains(root, &path)) {
            return AnalysisAdvice {
                kind: AnalysisPurpose::BrowserData,
                app_id: Some(app.clone()),
                basis: "standard_browser_data".into(),
                recognition: None,
                model_hint: None,
            };
        }
        if let Some((_, kind, basis)) = self.roots.iter().find(|(root, _, _)| contains(root, &path))
        {
            return AnalysisAdvice {
                kind: kind.clone(),
                app_id: None,
                basis: (*basis).into(),
                recognition: None,
                model_hint: None,
            };
        }
        AnalysisAdvice::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(path: &str, rules: &[&str]) -> AnalysisEntry {
        serde_json::from_value(serde_json::json!({"id":"0","parent":null,"name":"name","path":path,"directory":true,"bytes":"0","files":0,"modified_unix":0,"shared":false,"incomplete":false,"issue":null,"rule_ids":rules,"excluded":false})).unwrap()
    }
    #[test]
    fn trusted_redirected_roots_and_component_boundaries_prevent_name_guesses() {
        let map = PurposeMap {
            mixed: vec![normalize(r"E:\用户根")],
            browsers: vec![(
                normalize(r"D:\资料\本地\BraveSoftware\Brave-Browser\User Data"),
                "brave".into(),
            )],
            roots: vec![
                (
                    normalize(r"E:\用户根"),
                    AnalysisPurpose::Personal,
                    "known_downloads",
                ),
                (
                    normalize(r"D:\资料\本地"),
                    AnalysisPurpose::AppData,
                    "known_local_app_data",
                ),
                (
                    normalize(r"E:\工作"),
                    AnalysisPurpose::Personal,
                    "known_documents",
                ),
            ],
        };
        for path in [
            r"E:\用户根",
            r"D:\资料",
            r"E:\工作-old",
            r"D:\AppData\Cache\WeChat",
            r"D:\资料\本地-old",
        ] {
            assert_eq!(
                map.describe(&entry(path, &[])).kind,
                AnalysisPurpose::Unknown
            );
        }
        assert_eq!(
            map.describe(&entry(r"e:\工作\文件.txt", &[])).kind,
            AnalysisPurpose::Personal
        );
        assert_eq!(
            map.describe(&entry(r"D:\资料\本地\WeChat", &[])).kind,
            AnalysisPurpose::AppData
        );
        let browser = r"D:\资料\本地\BraveSoftware\Brave-Browser\User Data";
        let result = map.describe(&entry(browser, &[]));
        assert_eq!(result.kind, AnalysisPurpose::BrowserData);
        assert_eq!(result.app_id.as_deref(), Some("brave"));
        assert_eq!(
            map.describe(&entry(
                &format!("{browser}\\Default\\Cache\\Cache_Data"),
                &["brave.http"]
            ))
            .kind,
            AnalysisPurpose::Cache
        );
        assert_eq!(
            map.describe(&entry(&format!("{browser}\\Default\\FakeCache"), &[]))
                .kind,
            AnalysisPurpose::BrowserData
        );
        assert_eq!(
            map.describe(&entry(&format!("{browser}-old"), &[])).kind,
            AnalysisPurpose::AppData
        );
    }
    #[test]
    fn real_known_folders_are_used_and_user_root_is_mixed() {
        let map = PurposeMap::new();
        for (id, kind) in [
            (FOLDERID_LocalAppData, AnalysisPurpose::AppData),
            (FOLDERID_Windows, AnalysisPurpose::SystemData),
            (FOLDERID_ProgramData, AnalysisPurpose::SystemData),
        ] {
            let path = known_folder(&id).unwrap();
            assert_eq!(map.describe(&entry(&display_path(&path), &[])).kind, kind);
        }
        let profile = known_folder(&FOLDERID_Profile).unwrap();
        assert_eq!(
            map.describe(&entry(&display_path(&profile), &[])).kind,
            AnalysisPurpose::Unknown
        );
        let local = known_folder(&FOLDERID_LocalAppData).unwrap();
        for product in CHROMIUM_PRODUCTS {
            let result = map.describe(&entry(&display_path(&local.join(product.user_data)), &[]));
            assert_eq!(result.kind, AnalysisPurpose::BrowserData);
            assert_eq!(result.app_id.as_deref(), Some(product.id));
        }
    }
}

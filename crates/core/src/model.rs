use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    User,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleDescriptor {
    pub id: String,
    pub version: u32,
    pub title: String,
    pub description: String,
    pub impact: String,
    pub root: String,
    pub relative: String,
    pub min_age_days: u32,
    pub process: Option<String>,
    pub requires_admin: bool,
    pub default_selected: bool,
    #[serde(default)]
    pub cleanup_policy: CleanupPolicy,
    /// Static release gate. False means scan-only, independently of local availability.
    #[serde(default = "default_cleanup_validated")]
    pub cleanup_validated: bool,
    #[serde(default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub data_kind: DataKind,
}

fn default_cleanup_validated() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupPolicy {
    Recommended,
    #[default]
    Review,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataKind {
    #[default]
    Cache,
    Diagnostics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockReason {
    pub code: ReasonCode,
    pub message: String,
}
impl BlockReason {
    pub fn new(code: ReasonCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationRuleSupport {
    pub rule_id: String,
    pub cleanup_validated: bool,
    #[serde(default)]
    pub scan_basis: ScanBasis,
    #[serde(default)]
    pub layout_id: Option<String>,
    /// Historical evidence samples, never a current-version allowlist.
    /// None means an older producer did not report per-rule cleanup capability.
    #[serde(default)]
    pub cleanup_versions: Option<Vec<String>>,
    #[serde(default = "unknown_cleanup_block")]
    pub cleanup_block: Option<ReasonCode>,
}

fn unknown_cleanup_block() -> Option<ReasonCode> {
    Some(ReasonCode::Unverifiable)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanBasis {
    VerifiedVersion,
    RecognizedLayout,
    #[default]
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationSupport {
    pub app_id: String,
    pub title: String,
    pub version: Option<String>,
    pub supported_version: String,
    pub identity_verified: bool,
    pub scan_supported: bool,
    pub cleanup_validated: bool,
    #[serde(default)]
    pub rules: Vec<ApplicationRuleSupport>,
    pub status: Option<ReasonCode>,
    pub message: String,
    pub running_components: Vec<String>,
    pub locations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub path_display: String,
    pub bytes: u64,
    pub modified_unix: i64,
    /// Platform-private snapshot; never exposed over the UI bridge.
    pub opaque: String,
}

pub enum ScanEvent {
    Checked,
    File(FileRecord),
    Skipped(String),
    Notice(String),
    Retained(ReasonCode),
    RetainedFile {
        reason: ReasonCode,
        bytes: u64,
    },
    /// Read-only detail accompanying a retained event. Never a deletion candidate.
    Observation(ObservedFile),
    ApplicationObserved(ApplicationObservation),
    NoticeCode(ReasonCode),
    /// A supported location could not be enumerated. Never estimates its file count.
    CoverageIssue(ReasonCode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    Recent,
    InUse,
    AccessDenied,
    Changed,
    LinkOrCloud,
    Protected,
    Excluded,
    BrowserRunning,
    ApplicationRunning,
    ApplicationUnverified,
    UnsupportedVersion,
    ValidationPending,
    ProcessUnknown,
    UnsupportedLayout,
    LocationMissing,
    DirectoryInaccessible,
    UnsupportedVolume,
    ScanFailed,
    DeleteFailed,
    Unverifiable,
    Unknown,
}
impl ReasonCode {
    pub fn message(self) -> &'static str {
        match self {
            Self::ApplicationRunning => "应用或后台组件仍在运行，请完全退出后重新检查",
            Self::ApplicationUnverified => "应用身份无法验证，仅提供处理指引",
            Self::UnsupportedVersion => "当前组件或数据格式尚未适配，仅提供处理指引",
            Self::ValidationPending => "真实应用验收尚未完成；允许只读扫描，暂不开放删除",
            Self::Recent => "最近修改的文件，已保留",
            Self::InUse => "文件正在使用，请关闭相关应用后重新扫描",
            Self::AccessDenied => "权限不足，已保留文件",
            Self::Changed => "文件或父目录已变化，请重新扫描",
            Self::LinkOrCloud => "链接、多硬链接或云文件，已保留",
            Self::Protected => "受保护文件，已保留",
            Self::Excluded => "已按排除设置保留",
            Self::BrowserRunning => "浏览器仍在运行；关闭窗口后可能还有后台进程，请完全退出再检查",
            Self::ProcessUnknown => "无法确认应用状态，请重新检查",
            Self::UnsupportedLayout => "未发现支持的标准配置；自定义与便携配置不在支持范围",
            Self::LocationMissing => "未发现该标准位置；可能尚未安装应用或尚未生成此类文件",
            Self::DirectoryInaccessible => "目录不可访问，请检查访问权限后重新扫描",
            Self::UnsupportedVolume => "仅支持本地固定 NTFS 卷",
            Self::ScanFailed => "扫描未完整完成，请重新扫描",
            Self::DeleteFailed => "删除失败，文件未确认删除",
            Self::Unverifiable => "无法完成安全校验，已保留文件",
            Self::Unknown => "状态无法确认，不能计为删除成功",
        }
    }
}
#[derive(Debug)]
pub struct Refusal(pub ReasonCode);
impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.message())
    }
}
impl std::error::Error for Refusal {}
pub fn reason_for(error: &anyhow::Error) -> ReasonCode {
    if let Some(reason) = error.downcast_ref::<Refusal>() {
        reason.0
    } else if error.downcast_ref::<DeleteFailure>().is_some() {
        ReasonCode::DeleteFailed
    } else {
        ReasonCode::Unknown
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasonCount {
    pub code: ReasonCode,
    pub count: u64,
}
pub fn count_reason(counts: &mut Vec<ReasonCount>, code: ReasonCode) {
    if let Some(item) = counts.iter_mut().find(|c| c.code == code) {
        item.count += 1;
    } else {
        counts.push(ReasonCount { code, count: 1 });
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}
impl ApiError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ApiError {}
pub fn classify_api_error(error: anyhow::Error) -> ApiError {
    if let Some(e) = error.downcast_ref::<ApiError>() {
        return e.clone();
    }
    if error
        .chain()
        .any(|e| e.is::<std::io::Error>() || e.is::<rusqlite::Error>())
    {
        return ApiError::new("storage_failed", format!("本地存储操作失败：{error:#}"));
    }
    ApiError::new("operation_failed", format!("{error:#}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSettingsResult {
    pub scan_invalidated: bool,
    pub settings: Settings,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsPatch {
    pub theme: Option<String>,
    pub exclusions: Option<Vec<String>>,
    pub excluded_rules: Option<Vec<String>>,
}
impl SettingsPatch {
    pub fn apply(self, mut settings: Settings) -> Settings {
        if let Some(value) = self.theme {
            settings.theme = value;
        }
        if let Some(value) = self.exclusions {
            settings.exclusions = value;
        }
        if let Some(value) = self.excluded_rules {
            settings.excluded_rules = value;
        }
        settings
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Preferences {
    pub last_scan_rule_ids: Option<Vec<String>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    #[serde(default)]
    pub uninstall: Option<crate::software::UninstallTask>,
    #[serde(default)]
    pub analysis: Option<crate::AnalysisSnapshot>,
    #[serde(default)]
    pub control: Option<ControlInfo>,
    pub status: JobStatus,
    pub summary: ScanSummary,
    pub report: Option<CleanupReport>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandRequest {
    pub runtime_id: String,
    pub request_id: String,
    pub expected_revision: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandReceipt {
    pub request_id: String,
    pub command: String,
    pub value: serde_json::Value,
    pub error: Option<serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlInfo {
    pub runtime_id: String,
    pub revision: String,
    pub last_request: Option<CommandReceipt>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionKind {
    File,
    Parent,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateExclusion {
    pub candidate_id: String,
    pub kind: ExclusionKind,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub system_cleanup: bool,
}

/// Validation refusals are skips; errors from an attempted filesystem mutation are failures.
#[derive(Debug)]
pub struct DeleteFailure(pub String);
impl std::fmt::Display for DeleteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DeleteFailure {}

pub trait Platform: Send + Sync + 'static {
    fn tool_discovery(&self, _rule: &str) -> Option<ToolDiscovery> {
        None
    }
    fn invalidate_tool_discovery(&self, _rule: Option<&str>) {}
    /// Backend-only release gate. Test platforms may model acceptance for isolated fixtures.
    /// The production Windows implementation never takes authorization from IPC or settings.
    fn cleanup_validated(&self, rule: &RuleDescriptor) -> bool {
        crate::cleanup_acceptance::release_enabled(rule)
    }
    fn validate_candidate(&self, _rule: &RuleDescriptor, _file: &FileRecord) -> Result<()> {
        Ok(())
    }
    fn file_volume(&self, _file: &FileRecord) -> Result<VolumeTarget> {
        Err(ApiError::new("volume_unavailable", "无法识别目标磁盘").into())
    }
    fn volume_available(&self, _volume: &VolumeTarget) -> Result<u64> {
        Err(ApiError::new("volume_unavailable", "无法读取目标磁盘可用空间").into())
    }
    fn candidate_root(&self, _file: &FileRecord) -> Option<String> {
        None
    }
    fn candidate_action(
        &self,
        _rule: &RuleDescriptor,
        _file: &FileRecord,
        _kind: ExclusionKind,
        _reveal: bool,
    ) -> Result<String> {
        Err(ApiError::new("capability_unavailable", "当前平台不支持候选操作").into())
    }
    fn scan(
        &self,
        rule: &RuleDescriptor,
        exclusions: &[String],
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(ScanEvent),
    ) -> Result<()>;
    /// Explicit volume restrictions must be applied before enumerating file entries.
    /// Platforms that cannot enforce the restriction fail closed.
    fn scan_on_volumes(
        &self,
        rule: &RuleDescriptor,
        exclusions: &[String],
        volumes: Option<&[VolumeTarget]>,
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(ScanEvent),
    ) -> Result<()> {
        anyhow::ensure!(volumes.is_none(), "当前平台不支持指定磁盘扫描");
        self.scan(rule, exclusions, cancel, emit)
    }
    fn blocked(&self, rule: &RuleDescriptor) -> Result<Option<BlockReason>>;
    fn delete(&self, rule: &RuleDescriptor, file: &FileRecord, exclusions: &[String])
    -> Result<()>;
    fn free_space(&self) -> Result<u64>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub exclusions: Vec<String>,
    pub excluded_rules: Vec<String>,
    #[serde(default = "default_theme")]
    pub theme: String,
}
fn default_theme() -> String {
    "system".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Idle,
    Scanning,
    Ready,
    Cleaning,
    Finished,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanProgress {
    pub checked: u64,
    pub elapsed_ms: u64,
    pub last_progress_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatus {
    #[serde(default)]
    pub rule_refresh: Option<RuleRefresh>,
    /// None means all supported rule locations; labels are display-only.
    #[serde(default)]
    pub scan_volumes: Option<Vec<String>>,
    #[serde(default)]
    pub scan_progress: ScanProgress,
    pub id: String,
    pub state: JobState,
    pub current_rule: String,
    pub visited: u64,
    pub completed: u64,
    pub total: u64,
    pub error: Option<String>,
}
impl Default for JobStatus {
    fn default() -> Self {
        Self {
            rule_refresh: None,
            scan_volumes: None,
            scan_progress: ScanProgress::default(),
            id: String::new(),
            state: JobState::Idle,
            current_rule: String::new(),
            visited: 0,
            completed: 0,
            total: 0,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSummary {
    #[serde(default)]
    pub observation: Option<CacheObservation>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub pending_tool_task: Option<String>,
    #[serde(default)]
    pub tool: Option<ToolDiscovery>,
    #[serde(default)]
    pub coverage: Option<ScanCoverage>,
    #[serde(default)]
    pub progress: ScanProgress,
    #[serde(default)]
    pub phase: ScanPhase,
    pub rule_id: String,
    pub files: u64,
    pub bytes: String,
    pub skipped: u64,
    pub blocked: bool,
    pub note: Option<String>,
    #[serde(default)]
    pub scan_failed: bool,
    #[serde(default)]
    pub scan_issue: Option<ReasonCode>,
    #[serde(default)]
    pub availability: Option<ReasonCode>,
    #[serde(default)]
    pub reasons: Vec<ReasonCount>,
}

/// Runtime scan metadata only. This never grants a deletion capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDiscovery {
    pub executable: Option<String>,
    pub entry: Option<String>,
    pub version: Option<String>,
    pub target: Option<String>,
    pub ready: bool,
    pub reason: Option<String>,
    pub message: String,
    /// Opaque identity of the executable, entry point, cache directory, and parent chains.
    pub identity: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleRefresh {
    pub id: String,
    pub rule_id: String,
    pub revision: u64,
    pub state: JobState,
    pub progress: ScanProgress,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhase {
    #[default]
    Pending,
    Scanning,
    Completed,
    Partial,
    Interrupted,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub session_id: String,
    pub state: JobState,
    pub rules: Vec<RuleSummary>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub root: Option<String>,
    pub id: String,
    pub rule_id: String,
    pub path: String,
    pub bytes: String,
    pub modified_unix: i64,
    pub status: String,
    pub reason: Option<String>,
    pub reason_code: Option<ReasonCode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CandidateQuery {
    pub search: String,
    pub sort: String,
    pub direction: String,
    pub status: Option<String>,
    pub view: String,
}
impl Default for CandidateQuery {
    fn default() -> Self {
        Self {
            search: String::new(),
            sort: "size".into(),
            direction: "desc".into(),
            status: None,
            view: "scan".into(),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidatePage {
    pub items: Vec<Candidate>,
    pub total: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Selection {
    pub rule_ids: Vec<String>,
    #[serde(default)]
    pub excluded_ids: Vec<String>,
    #[serde(default)]
    pub included_ids: Vec<String>,
    #[serde(default)]
    pub reviewed_rule_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupPlan {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<crate::ManualSummary>,
    pub id: String,
    pub session_id: String,
    pub files: u64,
    pub bytes: String,
    pub rule_ids: Vec<String>,
    pub impacts: Vec<String>,
    #[serde(default)]
    pub categories: Vec<PlanCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanCategory {
    #[serde(default)]
    pub coverage: Option<ScanCoverage>,
    pub rule_id: String,
    pub files: u64,
    pub bytes: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleOutcome {
    #[serde(default)]
    pub coverage: Option<ScanCoverage>,
    pub rule_id: String,
    pub total: u64,
    pub deleted: u64,
    pub skipped: u64,
    pub failed: u64,
    pub unknown: u64,
    pub deleted_bytes: String,
    pub reasons: Vec<ReasonCount>,
    pub retained: Vec<ReasonCount>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanCoverage {
    pub complete: bool,
    pub issues: Vec<ReasonCount>,
}
#[derive(Debug, Clone)]
pub struct VolumeTarget {
    pub key: String,
    pub label: String,
    pub opaque: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSpaceChange {
    pub volume: String,
    pub before_bytes: Option<String>,
    pub after_bytes: Option<String>,
    pub change_bytes: Option<String>,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<RuleOutcome>,
    #[serde(default)]
    pub directories: crate::DirectoryOutcome,
    #[serde(default)]
    pub volume_space_changes: Vec<VolumeSpaceChange>,
    pub id: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub status: String,
    pub total: u64,
    pub deleted: u64,
    pub skipped: u64,
    pub failed: u64,
    pub unknown: u64,
    pub candidate_bytes: String,
    pub deleted_bytes: String,
    pub free_space_change: Option<String>,
    pub scope: Scope,
    #[serde(default)]
    pub categories: Vec<RuleOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bootstrap {
    #[serde(default)]
    pub preferences: Preferences,
    pub rules: Vec<RuleDescriptor>,
    pub settings: Settings,
    pub version: String,
    pub capabilities: Capabilities,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Scan-only observations; never included in a deletion plan or historical report.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheObservation {
    pub retained_files: u64,
    pub retained_bytes: String,
    pub recent_files: u64,
    pub recent_bytes: String,
    pub reasons: Vec<ObservedReason>,
    pub application: Option<ApplicationObservation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplicationObservation {
    pub title: String,
    pub identity_status: String,
    pub locations: Vec<String>,
    pub inventory_complete: bool,
}

impl Default for CacheObservation {
    fn default() -> Self {
        Self {
            retained_files: 0,
            retained_bytes: "0".into(),
            recent_files: 0,
            recent_bytes: "0".into(),
            reasons: vec![],
            application: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedReason {
    pub code: ReasonCode,
    pub files: u64,
    pub bytes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedFile {
    pub path: String,
    pub bytes: Option<String>,
    pub modified_unix: Option<i64>,
    pub reason: ReasonCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationPage {
    pub items: Vec<ObservedFile>,
    pub total: u64,
    pub offset: u64,
}

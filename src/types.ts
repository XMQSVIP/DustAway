export type Scope = 'user' | 'system';
export type JobState =
  'idle' | 'scanning' | 'ready' | 'cleaning' | 'finished' | 'cancelled' | 'failed';
export interface Rule {
  app_id?: string | null;
  data_kind?: 'cache' | 'diagnostics';
  id: string;
  version: number;
  title: string;
  description: string;
  impact: string;
  root: string;
  relative: string;
  min_age_days: number;
  process?: string | null;
  requires_admin: boolean;
  default_selected: boolean;
  cleanup_policy?: 'recommended' | 'review';
  cleanup_validated?: boolean;
}
export interface Settings {
  exclusions: string[];
  excluded_rules: string[];
  theme: 'system' | 'light' | 'dark';
}
export interface Bootstrap {
  preferences: { last_scan_rule_ids: string[] | null };
  rules: Rule[];
  settings: Settings;
  version: string;
  capabilities: { system_cleanup: boolean };
}
export interface ApplicationSupport {
  app_id: string;
  title: string;
  version: string | null;
  supported_version: string;
  identity_verified: boolean;
  scan_supported: boolean;
  cleanup_validated: boolean;
  rules?: {
    rule_id: string;
    cleanup_validated: boolean;
    scan_basis?: 'verified_version' | 'recognized_layout' | 'unverified';
    layout_id?: string | null;
    cleanup_versions?: string[] | null;
    cleanup_block?: ReasonCode | null;
  }[];
  status: ReasonCode | null;
  message: string;
  running_components: string[];
  locations: string[];
}
export interface Session {
  scope: Scope;
  id: string;
}
export interface Job {
  rule_refresh?: RuleRefresh | null;
  scan_volumes?: string[] | null;
  scan_progress?: ScanProgress;
  id: string;
  state: JobState;
  current_rule: string;
  visited: number;
  completed: number;
  total: number;
  error: string | null;
}
export interface CacheObservation {
  reasons?: { code: ReasonCode; files: number; bytes: string }[];
  retained_files: number;
  retained_bytes: string;
  recent_files: number;
  recent_bytes: string;
  application: {
    title: string;
    identity_status: string;
    locations: string[];
    inventory_complete: boolean;
  } | null;
}
export interface ObservationPage {
  items: { path: string; bytes: string | null; modified_unix: number | null; reason: ReasonCode }[];
  total: number;
  offset: number;
}
export interface RuleSummary {
  observation?: CacheObservation | null;
  revision?: number;
  stale?: boolean;
  pending_tool_task?: string | null;
  tool?: ToolDiscovery | null;
  coverage?: ScanCoverage | null;
  progress?: ScanProgress;
  phase: 'pending' | 'scanning' | 'completed' | 'partial' | 'interrupted' | 'failed';
  rule_id: string;
  files: number;
  bytes: string;
  skipped: number;
  blocked: boolean;
  note: string | null;
  scan_failed: boolean;
  scan_issue: ReasonCode | null;
  availability: ReasonCode | null;
  reasons: ReasonCount[];
}
export interface ToolDiscovery {
  executable: string | null;
  entry?: string | null;
  version: string | null;
  target: string | null;
  ready: boolean;
  reason: string | null;
  message: string;
  identity: string | null;
}
export interface RuleRefresh {
  id: string;
  rule_id: string;
  revision: number;
  state: JobState;
  progress: ScanProgress;
  error: string | null;
}
export interface Summary {
  session_id: string;
  state: JobState;
  rules: RuleSummary[];
}
export interface Candidate {
  root?: string | null;
  id: string;
  rule_id: string;
  path: string;
  bytes: string;
  modified_unix: number;
  status: string;
  reason: string | null;
  reason_code: ReasonCode | null;
}
export type ReasonCode =
  | 'recent'
  | 'in_use'
  | 'access_denied'
  | 'changed'
  | 'link_or_cloud'
  | 'protected'
  | 'excluded'
  | 'browser_running'
  | 'application_running'
  | 'application_unverified'
  | 'unsupported_version'
  | 'validation_pending'
  | 'process_unknown'
  | 'unsupported_layout'
  | 'location_missing'
  | 'directory_inaccessible'
  | 'unsupported_volume'
  | 'scan_failed'
  | 'delete_failed'
  | 'unverifiable'
  | 'unknown';
export interface ReasonCount {
  code: ReasonCode;
  count: number;
}
export interface ScanCoverage {
  complete: boolean;
  issues: ReasonCount[];
}
export interface CandidateQuery {
  search: string;
  sort: 'size' | 'modified' | 'path';
  direction: 'asc' | 'desc';
  status: string | null;
  view: 'scan' | 'cleanup' | 'unplanned';
}
export interface ScanProgress {
  checked: number;
  elapsed_ms: number;
  last_progress_ms: number;
}
export interface ExcludedFile {
  file?: Candidate;
  scope: Scope;
  session: string;
  rule_id: string;
  bytes: string;
}
export interface RuntimeSnapshot {
  uninstall?: import('./softwareTypes').UninstallTask | null;
  analysis?: AnalysisSnapshot | null;
  control?: ControlInfo;
  status: Job;
  summary: Summary;
  report: Report | null;
}
export interface CommandRequest {
  runtime_id: string;
  request_id: string;
  expected_revision: string;
}
export interface ControlInfo {
  runtime_id: string;
  revision: string;
  last_request: {
    request_id: string;
    command: string;
    value: unknown;
    error: { code: string; message: string } | null;
  } | null;
}
export interface SaveResult {
  settings: Settings;
  scan_invalidated: boolean;
}
export interface ExclusionDraft {
  scope: Scope;
  session: string;
  file: Candidate;
  kind: 'file' | 'parent';
}
export interface PlanCategory {
  coverage?: ScanCoverage | null;
  rule_id: string;
  files: number;
  bytes: string;
}
export interface RuleOutcome {
  coverage?: ScanCoverage | null;
  rule_id: string;
  total: number;
  deleted: number;
  skipped: number;
  failed: number;
  unknown: number;
  deleted_bytes: string;
  reasons: ReasonCount[];
  retained: ReasonCount[];
}
export interface CandidatePage {
  items: Candidate[];
  total: number;
  offset: number;
}
export interface Selection {
  rule_ids: string[];
  excluded_ids: string[];
  included_ids?: string[];
  reviewed_rule_ids?: string[];
}
export interface Plan {
  manual?: {
    source?: string;
    session: string;
    files: number;
    directories: number;
    bytes: string;
    retained: number;
  } | null;
  id: string;
  session_id: string;
  files: number;
  bytes: string;
  rule_ids: string[];
  impacts: string[];
  categories: PlanCategory[];
}
export interface ScopedPlan {
  scope: Scope;
  plan: Plan;
}
export interface Report {
  manual?: RuleOutcome | null;
  directories?: {
    total: number;
    deleted: number;
    skipped: number;
    failed: number;
    unknown: number;
  };
  volume_space_changes?: VolumeSpaceChange[];
  id: string;
  started_at: number;
  finished_at: number | null;
  status: string;
  total: number;
  deleted: number;
  skipped: number;
  failed: number;
  unknown: number;
  candidate_bytes: string;
  deleted_bytes: string;
  free_space_change: string | null;
  scope: Scope;
  categories: RuleOutcome[];
}

export interface VolumeSpaceChange {
  volume: string;
  before_bytes: string | null;
  after_bytes: string | null;
  change_bytes: string | null;
  error: string | null;
}
export interface AnalysisLocation {
  token: string;
  title: string;
  path: string;
  kind?: 'directory' | 'volume';
  is_system?: boolean;
  supported?: boolean;
  unavailable_reason?: string | null;
}
export interface AnalysisSnapshot {
  query_api_version?: number;
  allocated_bytes?: string;
  allocation_unknown?: number;
  id: string;
  location: AnalysisLocation;
  state: 'scanning' | 'completed' | 'partial' | 'cancelled' | 'failed';
  started_at: number;
  elapsed_ms: number;
  checked: number;
  entries: number;
  files: number;
  logical_bytes: string;
  unique_bytes: string;
  issues: { code: string; count: number }[];
  stale: boolean;
  error: string | null;
  phase?: 'enumerating' | 'walking' | 'indexing' | 'finished';
  current_directory?: string | null;
}
export interface AnalysisEntry {
  allocated_bytes?: string | null;
  allocation_unknown?: number;
  verification_pending?: boolean;
  manual?: { allowed: boolean; reason: ReasonCode | null };
  ancestors?: string[];
  id: string;
  parent: string | null;
  name: string;
  path: string;
  directory: boolean;
  bytes: string;
  files: number;
  modified_unix: number;
  shared: boolean;
  incomplete: boolean;
  pending?: boolean;
  issue: string | null;
  rule_ids: string[];
  excluded: boolean;
  advice?: AnalysisAdvice | null;
}
export interface ManualSelection {
  session: string;
  selected_ids: string[];
  excluded_ids: string[];
}
export interface AnalysisAdvice {
  model_hint?: {
    purpose: 'cache' | 'logs' | 'temporary' | 'personal' | 'application_data';
    model_id: string;
    score_margin: number;
    evidence: string[];
  } | null;
  recognition?: {
    rule_id: string;
    category?: RecognitionCategory | null;
    title: string;
    description: string;
    guidance: string;
    evidence: string;
    source: string;
    reviewed: string;
  } | null;
  kind:
    'cache' | 'browser_data' | 'app_data' | 'system_data' | 'personal' | 'unknown' | 'incomplete';
  app_id: string | null;
  basis: string;
}
export interface AnalysisContext {
  location_token: string;
  volume: string | null;
  available_bytes: string | null;
  sampled_at: number;
  error: string | null;
  volume_key?: string | null;
}
export interface AnalysisFilesPage {
  generated_at?: number;
  items: AnalysisEntry[];
  total: number;
  next_cursor?: string | null;
  count_pending?: boolean;
  count_failed?: boolean;
  offset: number;
}
export type RecognitionCategory =
  | 'windows'
  | 'development'
  | 'ai'
  | 'virtualization'
  | 'games'
  | 'applications'
  | 'media'
  | 'documents'
  | 'data'
  | 'archives';
export interface AnalysisDiscoveryQuery {
  kind: 'directory' | 'file';
  category: RecognitionCategory | 'recognized' | 'unclassified' | 'all';
  min_bytes: string;
  search: string;
  include_nested: boolean;
  incomplete_only: boolean;
  offset: number;
}
export interface AnalysisPage {
  generated_at?: number;
  next_cursor?: string | null;
  count_pending?: boolean;
  count_failed?: boolean;
  coverage?: { completed: number; incomplete: number; pending: number };
  node: AnalysisEntry;
  breadcrumbs: AnalysisEntry[];
  items: AnalysisEntry[];
  total: number;
  offset: number;
}

export interface ManualPreview {
  request_id: string;
  session: string;
  files: number;
  directories: number;
  bytes: string;
  retained: number;
}

export interface AnalysisReadQuery {
  request_id: string;
  session: string;
  view: 'directories' | 'files' | 'discovery';
  node: string;
  search: string;
  metric: 'logical' | 'allocated';
  descending: boolean;
  discovery: AnalysisDiscoveryQuery | null;
  cursor: string | null;
  offset: number;
}
export interface AnalysisReadPage extends Omit<AnalysisPage, 'total' | 'node'> {
  request_id: string;
  total: number | null;
  node: AnalysisEntry | null;
  next_cursor: string | null;
}

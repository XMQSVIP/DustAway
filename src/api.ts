import { softwarePreview } from './softwareTypes';
import { ui } from './i18n';
import { CommandCoordinator, ipcError, withTimeout, type CoordinationState } from './coordinator';
import { invoke, isTauri } from '@tauri-apps/api/core';
import type {
  Bootstrap,
  Settings,
  Scope,
  Session,
  Job,
  Summary,
  CandidatePage,
  Selection,
  ScopedPlan,
  Report,
  CandidateQuery,
  SaveResult,
} from './types';
import previewRules from './previewRules';
export const desktop = isTauri();
const coordinator = new CommandCoordinator((command, args) => {
  if (!desktop) return Promise.reject(new Error(ui.desktopRequired));
  return invoke(command, args);
});
const call = <T>(command: string, args?: Record<string, unknown>) =>
  coordinator.read<T>(command, args);
const submit = <T>(command: string, args?: Record<string, unknown>) =>
  coordinator.submit<T>(command, args);
export const api = {
  windowsSnapshot: (scope: import('./types').Scope) =>
    call<import('./WindowsMaintenance').WindowsSnapshot | null>('get_windows_maintenance', {
      scope,
    }),
  windowsScan: (scope: import('./types').Scope) =>
    submit<string>('scan_windows_maintenance', { scope }),
  windowsPrepare: (scope: import('./types').Scope, session: string, ids: string[]) =>
    submit<void>('prepare_windows_maintenance', { scope, session, ids }),
  windowsCancel: () => submit<void>('cancel_windows_maintenance'),
  windowsHistory: (scope: import('./types').Scope) =>
    call<import('./WindowsMaintenance').WindowsReport[]>('get_windows_maintenance_history', {
      scope,
    }),
  openWindowsStorage: () => call<void>('open_storage_settings'),
  observations: (scope: import('./types').Scope, session: string, rule: string, offset = 0) =>
    call<import('./types').ObservationPage>('list_scan_observations', {
      scope,
      session,
      rule,
      offset,
      limit: 50,
    }),
  attachmentLocations: (provider: import('./attachmentTypes').AttachmentProvider) =>
    call<import('./attachmentTypes').AttachmentDiscovery>('get_attachment_locations', { provider }),
  pickAttachment: (provider: import('./attachmentTypes').AttachmentProvider) =>
    invoke<import('./attachmentTypes').AttachmentLocation | null>('pick_attachment_location', {
      provider,
    }),
  startAttachments: (token: string) => submit<string>('start_attachment_scan', { token }),
  attachmentSnapshot: () =>
    call<import('./attachmentTypes').AttachmentSnapshot | null>('get_attachment_snapshot'),
  attachments: (query: import('./attachmentTypes').AttachmentQuery) =>
    call<import('./attachmentTypes').AttachmentPage>('list_attachments', { query }),
  attachmentThumbnail: (session: string, id: string) =>
    call<string>('get_attachment_thumbnail', { session, id }),
  revealAttachment: (session: string, id: string) =>
    call<void>('reveal_attachment', { session, id }),
  cancelAttachments: () => call<void>('cancel_attachment_scan'),
  closeAttachments: () => call<void>('close_attachment_session'),
  cancelAttachmentPlan: () => call<void>('cancel_attachment_plan'),
  prepareAttachments: (
    attachments: import('./attachmentTypes').AttachmentSelection,
    preparationId: string,
  ) => submit<ScopedPlan[]>('prepare_cleanup', { requests: [], attachments, preparationId }),
  prepareTool: (session: string, ruleId: string, preparationId: string) =>
    submit<import('./ToolCache').ToolPlan>('prepare_tool_cache', {
      session,
      ruleId,
      preparationId,
    }),
  cancelToolPreparation: (preparationId: string) =>
    call<void>('cancel_tool_preparation', { preparationId }),
  executeTool: (planId: string) => submit<string>('execute_tool_cache', { planId }),
  cancelTool: () => submit<void>('cancel_tool_cache'),
  toolTask: () =>
    desktop
      ? call<import('./ToolCache').ToolReport | null>('get_tool_task')
      : Promise.resolve(null),
  toolHistory: () =>
    desktop ? call<import('./ToolCache').ToolReport[]>('get_tool_history') : Promise.resolve([]),
  prepareUv: (session: string, preparationId?: string) =>
    submit<import('./UvCache').UvPlan>('prepare_uv_cache', {
      session,
      ...(preparationId ? { preparationId } : {}),
    }),
  cancelUvPreparation: (preparationId: string) =>
    call<void>('cancel_uv_preparation', { preparationId }),
  executeUv: (planId: string) => submit<string>('execute_uv_cache', { planId }),
  cancelUv: () => submit<void>('cancel_uv_cache'),
  uvTask: () =>
    desktop ? call<import('./UvCache').UvReport | null>('get_uv_task') : Promise.resolve(null),
  uvHistory: () =>
    desktop ? call<import('./UvCache').UvReport[]>('get_uv_history') : Promise.resolve([]),
  softwareList: () =>
    desktop
      ? call<import('./softwareTypes').SoftwareList>('list_installed_software')
      : Promise.resolve(softwarePreview),
  prepareSoftwareUninstall: (softwareId: string) =>
    submit<import('./softwareTypes').UninstallPlan>('prepare_uninstall', { softwareId }),
  executeSoftwareUninstall: (planId: string) => submit<string>('execute_uninstall', { planId }),
  verifySoftware: (taskId: string) => submit<void>('verify_uninstall', { taskId }),
  scanSoftwareResiduals: (taskId: string) => submit<void>('scan_uninstall_residuals', { taskId }),
  softwareResiduals: (taskId: string, offset: number, search: string) =>
    call<import('./softwareTypes').ResidualScan>('list_uninstall_residuals', {
      taskId,
      offset,
      search,
    }),
  prepareSoftwareCleanup: (taskId: string, candidateIds: string[]) =>
    submit<import('./softwareTypes').ResidualCleanupPlan>('prepare_uninstall_residual_cleanup', {
      taskId,
      candidateIds,
    }),
  executeSoftwareCleanup: (planId: string) =>
    submit<void>('execute_uninstall_residual_cleanup', { planId }),
  stopSoftwareResiduals: (taskId: string) =>
    submit<void>('cancel_uninstall_residual_task', { taskId }),
  uninstallHistory: () =>
    desktop
      ? call<import('./softwareTypes').UninstallReport[]>('get_uninstall_history')
      : Promise.resolve([]),
  manualPreview: (selection: import('./types').ManualSelection, requestId: string) =>
    withTimeout(
      invoke<import('./types').ManualPreview>('get_manual_selection_preview', {
        selection,
        requestId,
      }),
      300_000,
    ),
  cancelManualPreview: (requestId: string) =>
    call<void>('cancel_manual_selection_preview', { requestId }),
  applicationSupport: () =>
    desktop
      ? call<import('./types').ApplicationSupport[]>('get_application_support')
      : Promise.resolve([]),
  analysisRead: (query: import('./types').AnalysisReadQuery) =>
    call<import('./types').AnalysisReadPage>('read_analysis_page', { query }),
  cancelAnalysisQuery: (requestId: string) => call<void>('cancel_analysis_query', { requestId }),
  analysisCount: (requestId: string) =>
    call<{ request_id: string; total: number | null; finished: boolean; error: string | null }>(
      'get_analysis_count',
      { requestId },
    ),
  analysisStorage: () =>
    desktop ? call<string | null>('get_analysis_storage') : Promise.resolve(null),
  pickAnalysisStorage: () => invoke<string | null>('pick_analysis_storage'),
  analysisFiles: (session: string, search: string, offset: number) =>
    call<import('./types').AnalysisFilesPage>('list_analysis_files', { session, search, offset }),
  analysisDiscovery: (session: string, query: import('./types').AnalysisDiscoveryQuery) =>
    call<import('./types').AnalysisFilesPage>('list_analysis_discovery', { session, query }),
  analysisContext: (locationToken: string) =>
    call<import('./types').AnalysisContext>('get_analysis_context', { locationToken }),
  refineAnalysis: (session: string, node: string) =>
    call<import('./types').AnalysisLocation>('refine_analysis_location', { session, node }),
  apps: () => call<void>('open_apps_settings'),
  analysisLocations: () =>
    desktop
      ? call<import('./types').AnalysisLocation[]>('get_analysis_locations')
      : Promise.resolve([]),
  // A native dialog has no deadline; normalize its structured errors like other IPC reads.
  pickAnalysis: () =>
    invoke<import('./types').AnalysisLocation | null>('pick_analysis_location').catch((error) => {
      throw ipcError(error);
    }),
  startAnalysis: (token: string) => submit<string>('start_analysis', { token }),
  cancelAnalysis: (session: string) => submit<void>('cancel_analysis', { session }),
  analysisChildren: (
    session: string,
    node: string,
    search: string,
    offset: number,
    descending: boolean,
  ) =>
    call<import('./types').AnalysisPage>('list_analysis_children', {
      session,
      node,
      search,
      offset,
      descending,
    }),
  revealAnalysis: (session: string, node: string) =>
    call<void>('reveal_analysis_entry', { session, node }),
  attach: () => coordinator.attach(),
  onCoordination: (listener: (checking: CoordinationState) => void) =>
    coordinator.subscribe(listener),
  bootstrap: () =>
    desktop
      ? call<Bootstrap>('bootstrap')
      : Promise.resolve<Bootstrap>({
          rules: previewRules.filter((r) => !r.requires_admin),
          version: '0.22.0',
          preferences: { last_scan_rule_ids: null },
          capabilities: { system_cleanup: false },
          settings: { exclusions: [], excluded_rules: [], theme: 'system' as const },
        }),
  scan: (ruleIds: string[], volumeTokens?: string[]) =>
    submit<{ sessions: Session[]; warning: string | null }>('start_scan', {
      ruleIds,
      ...(volumeTokens ? { volumeTokens } : {}),
    }),
  cancel: (targets: Session[], refreshTaskId?: string) =>
    submit<void>('cancel_job', { targets, ...(refreshTaskId ? { refreshTaskId } : {}) }),
  refreshRule: (session: string, ruleId: string) =>
    submit<string>('refresh_cleanup_rule', { session, ruleId }),
  snapshot: (scope: Scope) => coordinator.snapshot(scope),
  reveal: (scope: Scope, session: string, candidateId: string) =>
    call<void>('reveal_candidate', { scope, session, candidateId }),
  addExclusions: (
    scope: Scope,
    session: string,
    items: { candidate_id: string; kind: 'file' | 'parent' }[],
  ) => submit<SaveResult>('add_candidate_exclusions', { scope, session, items }),
  status: (scope: Scope) => call<Job>('get_job_status', { scope }),
  summary: (scope: Scope) => call<Summary>('get_scan_summary', { scope }),
  candidates: (
    scope: Scope,
    session: string,
    rule: string,
    offset: number,
    query?: CandidateQuery,
  ) => call<CandidatePage>('list_candidates', { scope, session, rule, offset, limit: 200, query }),
  refresh: (scope: Scope, session: string) =>
    submit<Summary>('refresh_rule_availability', { scope, session }),
  prepare: (
    requests: { scope: Scope; session: string; selection: Selection }[],
    manual?: import('./types').ManualSelection,
    preparationId = crypto.randomUUID(),
  ) => submit<ScopedPlan[]>('prepare_cleanup', { requests, manual, preparationId }),
  cancelPreparation: (preparationId: string) =>
    call<void>('cancel_cleanup_preparation', { preparationId }),
  excludeAnalysis: (session: string, node: string) =>
    submit<SaveResult>('exclude_analysis_entry', { session, node }),
  execute: (
    plans: ScopedPlan[],
    windowsPlans?: import('./WindowsMaintenance').ScopedWindowsPlan[],
  ) =>
    submit<{ scopes: Scope[]; warning: string | null }>('execute_cleanup', {
      plans,
      ...(windowsPlans?.length
        ? { windowsPlans: windowsPlans.map((p) => ({ scope: p.scope, plan_id: p.plan.id })) }
        : {}),
    }),
  report: (scope: Scope) => call<Report | null>('get_cleanup_report', { scope }),
  history: () => (desktop ? call<Report[]>('get_history') : Promise.resolve([])),
  save: (patch: Partial<Settings>) => submit<SaveResult>('save_settings', { patch }),
  pick: (kind: 'file' | 'folder') => invoke<string | null>('pick_exclusion', { kind }),
  export: () => invoke<boolean>('export_diagnostics'),
  clearHistory: () => submit<void>('clear_history'),
  storage: () => call<void>('open_storage_settings'),
};

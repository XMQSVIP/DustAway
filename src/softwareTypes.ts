export interface ResidualCapability {
  kind: 'cache' | 'settings' | 'local_data';
  inspection: 'fixed_locations' | 'unsupported';
  cleanup: 'unsupported' | 'not_validated' | 'requires_verification';
}
export const residualKindLabels: Record<ResidualCapability['kind'], string> = {
  cache: '缓存与日志',
  settings: '设置',
  local_data: '本地资料',
};
export const residualInspectionLabels: Record<ResidualCapability['inspection'], string> = {
  fixed_locations: '固定位置',
  unsupported: '暂不支持',
};
export const residualCleanupLabels: Record<ResidualCapability['cleanup'], string> = {
  unsupported: '暂不支持',
  not_validated: '尚未开放',
  requires_verification: '需逐项核验',
};
export const softwareMissingLabels = {
  publisher: '发布者',
  version: '版本',
  estimated_bytes: '登记大小',
  installed_on: '安装日期',
  install_location: '安装位置',
};
export interface InstalledSoftware {
  id: string;
  name: string;
  publisher: string;
  version: string;
  source: 'desktop' | 'store';
  scope: string;
  estimated_bytes: string | null;
  installed_on: string | null;
  install_location: string | null;
  icon_data_url?: string | null;
  can_uninstall: boolean;
  unavailable_reason: string | null;
  residual_supported: boolean;
  residual_note: string;
  residual_capabilities?: ResidualCapability[];
  missing_information?: (keyof typeof softwareMissingLabels)[];
}
export interface SoftwareList {
  items: InstalledSoftware[];
  warnings: string[];
}
export interface UninstallPlan {
  id: string;
  software: InstalledSoftware;
  impact: string;
}
export type UninstallPhase =
  | 'uninstalling'
  | 'verifying'
  | 'removed'
  | 'still_installed'
  | 'restart_required'
  | 'cancelled'
  | 'failed'
  | 'unknown'
  | 'scanning_residuals'
  | 'residuals_ready'
  | 'cleaning_residuals'
  | 'finished';
export type UninstallStatus = Exclude<
  UninstallPhase,
  'scanning_residuals' | 'residuals_ready' | 'cleaning_residuals' | 'finished'
>;
export interface UninstallTask {
  id: string;
  software: InstalledSoftware;
  phase: UninstallPhase;
  uninstall_status?: UninstallStatus | null;
  started_at: number;
  updated_at: number;
  progress: number | null;
  message: string;
  can_scan_residuals: boolean;
  residual_scan_id: string | null;
  residual: {
    deleted_files: number;
    deleted_directories: number;
    deleted_bytes: string;
    skipped: number;
    failed: number;
    unknown: number;
    unprocessed?: number;
  };
}
export type UninstallReport = UninstallTask;
export interface ResidualCandidate {
  id: string;
  path: string;
  category: 'cache' | 'data';
  bytes: string;
  directory: boolean;
  selectable: boolean;
  reason: string | null;
}
export interface ResidualScan {
  task_id: string;
  scan_id: string;
  items: ResidualCandidate[];
  total: number;
  offset: number;
  complete: boolean;
  supported: boolean;
  warnings: string[];
}
export interface ResidualCleanupPlan {
  id: string;
  task_id: string;
  files: number;
  directories: number;
  bytes: string;
  includes_data: boolean;
  preview: ResidualCandidate[];
}
export const uninstallActive = (phase?: UninstallPhase) =>
  !!phase &&
  ['uninstalling', 'verifying', 'scanning_residuals', 'cleaning_residuals'].includes(phase);
export const uninstallLabels: Record<UninstallPhase, string> = {
  uninstalling: '正在卸载',
  verifying: '正在核对',
  removed: '已卸载',
  still_installed: '仍有安装登记',
  restart_required: '需要重启后核对',
  cancelled: '已取消',
  failed: '卸载失败',
  unknown: '结果待核对',
  scanning_residuals: '正在检查残留',
  residuals_ready: '残留检查结束',
  cleaning_residuals: '正在清理残留',
  finished: '残留处理结束',
};
function standardUninstallStatus(task: UninstallTask): UninstallStatus {
  if (task.uninstall_status) return task.uninstall_status;
  switch (task.phase) {
    case 'scanning_residuals':
    case 'residuals_ready':
    case 'cleaning_residuals':
    case 'finished':
      return 'removed';
    default:
      return task.phase;
  }
}
export function standardUninstallLabel(task: UninstallTask): string {
  const status = standardUninstallStatus(task);
  return status === 'removed' ? '已核对移除' : uninstallLabels[status];
}
export function uninstallTaskLabel(task: UninstallTask): string {
  return task.phase === 'unknown' && standardUninstallStatus(task) === 'removed'
    ? '残留结果待核对'
    : uninstallLabels[task.phase];
}
export const softwarePreview: SoftwareList = {
  warnings: [],
  items: [
    {
      id: 'preview-browser',
      name: '示例浏览器',
      publisher: '示例发布者',
      version: '1.0',
      source: 'desktop',
      scope: 'machine',
      estimated_bytes: '524288000',
      installed_on: '2026-09-01',
      install_location: null,
      can_uninstall: false,
      unavailable_reason: '网页预览不能执行卸载',
      residual_supported: true,
      residual_note: '演示：检查缓存、设置与本地资料',
      residual_capabilities: [
        { kind: 'cache', inspection: 'fixed_locations', cleanup: 'not_validated' },
        { kind: 'settings', inspection: 'fixed_locations', cleanup: 'not_validated' },
        { kind: 'local_data', inspection: 'fixed_locations', cleanup: 'not_validated' },
      ],
      missing_information: ['install_location'],
    },
    {
      id: 'preview-store',
      name: '示例笔记',
      publisher: '示例发布者',
      version: '2.0',
      source: 'store',
      scope: 'current_user',
      estimated_bytes: null,
      installed_on: null,
      install_location: null,
      can_uninstall: false,
      unavailable_reason: '网页预览不能执行卸载',
      residual_supported: true,
      residual_note: '演示：检查当前用户的独占包数据目录',
      residual_capabilities: [
        { kind: 'cache', inspection: 'fixed_locations', cleanup: 'not_validated' },
        { kind: 'settings', inspection: 'fixed_locations', cleanup: 'not_validated' },
        { kind: 'local_data', inspection: 'fixed_locations', cleanup: 'not_validated' },
      ],
      missing_information: ['estimated_bytes', 'installed_on', 'install_location'],
    },
  ],
};

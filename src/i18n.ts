export const windowText = {
  titlebar: '拂尘窗口标题栏',
  controls: '窗口控制',
  drag: '拖动窗口，双击最大化或还原',
  minimize: '最小化',
  maximize: '最大化',
  restore: '还原窗口',
  close: '关闭窗口',
  closing: '正在退出…',
  failed: '窗口操作未完成，请重试：',
} as const;

export const text = {
  app: '拂尘',
  tagline: '给磁盘留一点余地',
  clean: '清理',
  history: '历史',
  settings: '设置',
  title: '清理，从看清楚开始',
  subtitle: '扫描临时文件与浏览器缓存，确认后再清理。',
  scan: '开始扫描',
  rescan: '重新全面扫描',
  cancel: '停止任务',
  prepare: '确认清理范围',
  scanning: '正在扫描',
  cleaning: '正在清理',
  ready: '扫描完成',
  cancelled: '任务已停止',
  finished: '清理完成',
  preview: '网页预览 · 实际扫描和清理需要启动桌面应用',
  confirmTitle: '确认永久删除这些文件？',
  confirmDescription: '所选内容将直接删除，不会进入回收站，无法通过拂尘撤销。',
  confirm: '永久删除',
  back: '返回检查',
  safeTitle: '只清理你确认的范围',
  safeBody: '建议项自动选择，自选资料由你决定。受保护、占用或无法验证的内容会保留。',
  emptyHistory: '还没有清理记录',
  emptyHistoryBody: '完成清理后，处理结果会保存在这里，默认保留 30 天。',
  exclusions: '排除项',
  exclusionsBody: '这些文件或目录不会被清理。修改排除项后需要重新扫描。',
  noExclusions: '尚未添加排除项',
  closing: '正在停止任务，请稍候… 已完成的删除不会撤销。',
} as const;
export const states: Record<string, string> = {
  idle: '尚未扫描',
  scanning: '正在扫描',
  ready: '扫描完成',
  cleaning: '正在清理',
  finished: '已完成',
  cancelled: '已停止',
  failed: '任务失败',
  interrupted: '异常中断',
  running: '进行中',
  candidate: '待清理',
  deleted: '已删除',
  skipped: '已跳过',
};
export function formatBytes(value: string | bigint): string {
  let bytes: bigint;
  try {
    bytes = BigInt(value);
  } catch {
    return '—';
  }
  const negative = bytes < 0n;
  if (negative) bytes = -bytes;
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  let unit = 0;
  let divisor = 1n;
  while (bytes >= divisor * 1024n && unit < units.length - 1) {
    divisor *= 1024n;
    unit++;
  }
  const tenths = (bytes * 10n + divisor / 2n) / divisor;
  return `${negative ? '−' : ''}${unit === 0 ? bytes.toString() : `${tenths / 10n}.${tenths % 10n}`} ${units[unit]}`;
}
export const formatTime = (seconds: number) =>
  new Date(seconds * 1000).toLocaleString('zh-CN', { hour12: false });

export const dynamic = {
  files: (count: number) => `${count.toLocaleString()} 个文件`,
  candidates: (count: number, excluded: number) =>
    `${count.toLocaleString()} 个文件${excluded ? `，另有 ${excluded} 项已取消勾选；以确认计划为准` : ''}`,
  discovered: (count: number) => `已发现 ${count.toLocaleString()} 个文件`,
  processed: (done: number, total: number) =>
    `已处理 ${done.toLocaleString()} / ${total.toLocaleString()}`,
  selectedCategories: (count: number) => `${count} 个类别已选择`,
  viewFiles: (title: string) => `查看${title}的文件`,
  removeExclusion: (path: string) => `移除排除项 ${path}`,
  selectFile: (path: string) => `选择 ${path}`,
  unknown: (count: number) => ` · 状态不确定 ${count.toLocaleString()} 项`,
  remaining: (count: number) => ` · 未处理 ${count.toLocaleString()} 项`,
};

// Supporting UI labels. All user-facing static copy belongs in this catalog.
export const ui = {
  temporaryFiles: '临时文件',
  renderCaches: '网页与渲染缓存',
  webCache: '网页缓存',
  settingsSaved: '设置已保存。下次清理前请重新扫描。',
  resultTitle: '每一项处理，都有记录',
  readyTitle: '扫描完成，选择要清理的内容',
  skipContent: '跳到主要内容',
  mainNavigation: '主导航',
  offline: '本地运行 · 无需账号',
  previewVersion: '预览版',
  dismissError: '关闭错误提示',
  dismissNotice: '关闭提示',
  knownCaches: '只清理已知缓存',
  scanSummary: '扫描摘要',
  selectedCandidates: '已选择的候选文件',
  spacePrompt: '准备好，腾出一点空间',
  scanFirst: '先扫描，再决定',
  scanExplanation: '仅检查你选择的类别，扫描过程不删除任何文件。',
  awaitingConfirmation: '等待你确认',
  resultsSaved: '处理结果已保存',
  scopeControl: '由你掌控清理范围',
  currentReport: '本次清理结果',
  scopeHeading: '清理范围',
  categoryUnit: '个类别',
  categories: '清理类别',
  selectedPrefix: '已选',
  authorization: '需授权',
  excludedRule: '已在设置中排除',
  skippedSafety: '项因保留时间或安全检查跳过',
  awaitingScan: '等待扫描',
  working: '操作进行中',
  cancelEffect: '停止任务不会撤销已完成的删除',
  confirmationExplanation: '清理前始终显示具体范围和影响',
  stop: '停止任务',
  historyHeading: '清理历史',
  historyRetention: '结果保存在本机，默认保留 30 天。',
  clearHistory: '清空记录',
  goToCleanup: '前往清理',
  settings: '设置',
  settingsDescription: '按自己的习惯，保留需要的内容。',
  appearance: '外观',
  appearanceDescription: '默认跟随 Windows 的颜色模式。',
  systemTheme: '跟随系统',
  lightTheme: '浅色',
  darkTheme: '深色',
  file: '文件',
  folder: '目录',
  excludeCategories: '按清理类别排除',
  diagnosticsHeading: '诊断与系统工具',
  diagnosticsPrivacy: '导出的摘要不包含文件路径、用户名或浏览记录。',
  diagnosticsSaved: '诊断摘要已导出。',
  exportDiagnostics: '导出诊断摘要',
  storageSettings: 'Windows 存储设置',
  about: 'MIT 开源 · 本地处理 · 手动更新',
  previewBuild: '免安装预览版 · 未签名 · 当前仅提供普通用户清理。',
  fileUnit: '个文件',
  finalValidation:
    '文件在扫描后发生变化或被占用时会跳过。候选文件大小不等于磁盘实际增加的可用空间。',
  clearHistoryTitle: '清空本机清理记录？',
  clearHistoryDescription: '这只移除清理与卸载的历史摘要，不会再次删除文件或卸载软件。',
  cancel: '取消',
  systemCleanup: '系统清理',
  userCleanup: '用户清理',
  candidateSize: '候选文件大小',
  deletedSize: '已删除文件大小',
  availableSpaceChange: '系统盘可用空间变化',
  deletedPrefix: '已删除',
  skippedInReport: '项 · 跳过',
  failedInReport: '项 · 失败',
  itemUnit: '项',
  spaceCaveat: '可用空间受系统和其他应用活动影响；已完成的删除无法撤销。',
  filesDescription: '逐项查看文件。取消勾选只影响本次清理。',
  closeFiles: '关闭文件明细',
  readingFiles: '正在读取文件…',
  noFiles: '该类别没有可清理的文件。',
  totalPrefix: '共',
  pageSize: '个文件 · 每页 200 项',
  previousPage: '上一页',
  nextPage: '下一页',
  desktopRequired: '请使用 pnpm desktop 启动桌面应用后操作。网页预览不会读取或清理文件。',
} as const;

export const reasonLabels: Record<import('./types').ReasonCode, string> = {
  recent: '近期修改，按保留期限留下',
  in_use: '文件被占用',
  access_denied: '权限不足',
  changed: '文件或父目录已经变化',
  link_or_cloud: '链接、多硬链接或云文件',
  protected: '受保护文件',
  excluded: '按排除设置保留',
  browser_running: '浏览器仍在运行',
  application_running: '应用或后台组件仍在运行',
  application_unverified: '应用身份无法验证',
  unsupported_version: '当前组件或数据格式尚未适配',
  validation_pending: '真实删除验收未完成，暂不开放删除',
  process_unknown: '无法确认应用状态',
  unsupported_layout: '未发现支持的标准配置',
  location_missing: '未发现标准位置（可能尚未安装或生成）',
  directory_inaccessible: '目录不可访问',
  unsupported_volume: '不支持的磁盘类型',
  scan_failed: '扫描失败',
  delete_failed: '删除失败',
  unverifiable: '无法完成安全校验，已保留',
  unknown: '状态不确定',
};

export const scanPhases = {
  pending: '等待扫描',
  scanning: '扫描中',
  completed: '扫描完成',
  partial: '部分位置未读取',
  interrupted: '扫描中断 · 结果不完整',
  failed: '扫描失败 · 结果不完整',
};
export const candidateActions = {
  menu: '文件操作',
  reveal: '在文件资源管理器中定位',
  file: '以后跳过此文件',
  parent: '以后跳过所在文件夹',
  review: '核对长期排除',
  save: '保存排除并重新扫描',
  saving: '正在保存与核对…',
  description:
    '保存后这些路径将不再参与清理，旧扫描和计划失效。文件夹包含其全部子目录；可在设置中移除。',
};

import type { AnalysisEntry } from './types';
import { applicationNames } from './applicationNames';

const descriptions = {
  cache: [
    '现有规则覆盖的位置',
    '使用对应清理类别选择，受保护、占用和排除内容继续保留；缓存可能重新生成。',
  ],
  browser_data: ['浏览器资料', '可能包含登录、密码、书签及站点资料，受保护的内容会保留。'],
  app_data: ['应用数据', '可能包含设置、登录状态和工作内容，受保护的内容会保留。'],
  system_data: ['系统、程序或共享数据', '可能影响系统或已安装程序，这些内容保持保护。'],
  personal: ['个人资料位置', '可能包含下载、文档或项目文件；是否保留由你决定。'],
  unknown: ['用途未确定', '尚未识别具体用途，文件大小本身不能说明是否需要保留。'],
  incomplete: ['读取不完整 · 用途未确定', '当前大小可能仅包含部分文件，请核对访问条件后补充分析。'],
} as const;
export const analysisBases: Record<string, string> = {
  verified_rule_location: '已验证的内置规则位置',
  standard_browser_data: '受支持浏览器的标准数据位置（不代表已安装）',
  known_local_app_data: 'Windows 登记的本地应用数据位置',
  known_local_low_app_data: 'Windows 登记的低权限应用数据位置',
  known_roaming_app_data: 'Windows 登记的漫游应用数据位置',
  known_windows: 'Windows 登记的系统位置',
  known_program_files: 'Windows 登记的程序安装位置',
  known_program_data: 'Windows 登记的共享程序数据位置',
  known_downloads: 'Windows 登记的下载位置',
  known_documents: 'Windows 登记的文档位置',
  known_desktop: 'Windows 登记的桌面位置',
  known_pictures: 'Windows 登记的图片位置',
  known_music: 'Windows 登记的音乐位置',
  known_videos: 'Windows 登记的视频位置',
  incomplete_read: '读取未完成',
};
const recognizedApps: Record<string, string> = {
  'app.baidu_netdisk_profile': 'baidu_netdisk',
  'app.baidu_netdisk_web_cache': 'baidu_netdisk',
  'app.baidu_netdisk_storage': 'baidu_netdisk',
  'app.quark_netdisk_profile': 'quark_netdisk',
  'app.quark_netdisk_web_cache': 'quark_netdisk',
  'app.quark_netdisk_storage': 'quark_netdisk',
  'app.thunder_profile': 'thunder',
  'app.thunder_web_cache': 'thunder',
  'app.thunder_storage': 'thunder',
  'app.thunder_partial': 'thunder',
  'app.chrome': 'chrome',
  'app.edge': 'edge',
  'app.firefox': 'firefox',
  'app.wps_backup': 'wps',
  'app.feishu_profile': 'feishu',
  'app.feishu_web_cache': 'feishu',
  'app.dingtalk_web_profile': 'dingtalk',
  'app.dingtalk_web_cache': 'dingtalk',
};

const modelPurposes = {
  cache: ['可能是缓存', '可先在所属应用中核对用途及清理方式。'],
  logs: ['可能是日志或诊断资料', '可能用于排查问题，请先确认是否需要保留。'],
  temporary: ['可能是临时文件', '可能仍被应用使用，请先核对来源与使用状态。'],
  personal: ['可能是个人资料', '请核对文件内容和备份，是否保留由你决定。'],
  application_data: ['可能是应用设置或数据', '可能包含设置与工作内容，建议先在所属应用中核对。'],
} as const;

/** Descriptive routing only; cleanup eligibility belongs to the cleanup model/backend. */
export function analysisDecision(entry: AnalysisEntry) {
  const kind = entry.advice?.kind ?? 'unknown';
  const recognition = entry.advice?.recognition;
  const candidateHint = entry.advice?.model_hint;
  const modelHint =
    candidateHint &&
    Object.hasOwn(modelPurposes, candidateHint.purpose) &&
    !recognition &&
    !entry.rule_ids.length &&
    !entry.pending &&
    !entry.incomplete &&
    !entry.issue &&
    !entry.excluded &&
    (kind === 'unknown' || kind === 'app_data')
      ? candidateHint
      : null;
  const modelPurpose = modelHint ? modelPurposes[modelHint.purpose] : null;
  const [label, explanation] = descriptions[kind] ?? descriptions.unknown;
  const declared = entry.advice?.app_id;
  const inferred = recognition && recognizedApps[recognition.rule_id];
  const conflict = !!(declared && inferred && declared !== inferred);
  const candidate = conflict ? null : declared || inferred;
  const appId = candidate && Object.hasOwn(applicationNames, candidate) ? candidate : null;
  const refine = entry.directory && (entry.pending || entry.incomplete || !!entry.issue);
  const primary = refine
    ? 'refine'
    : entry.rule_ids.length
      ? 'cleanup'
      : recognition || appId || kind === 'app_data' || kind === 'browser_data'
        ? 'guide'
        : kind === 'system_data'
          ? 'storage'
          : 'reveal';
  return {
    primary,
    appId,
    conflict,
    title: recognition
      ? `可能是${recognition.title}`
      : modelPurpose
        ? `${modelPurpose[0]} · 模型推测`
        : appId
          ? `${applicationNames[appId]} · ${label}`
          : label,
    description:
      recognition?.description ??
      (modelPurpose
        ? '根据目录和文件名称推测用途，未读取文件内容；这一判断不代表可以删除。'
        : explanation),
    guidance: refine
      ? entry.issue === 'directory_inaccessible' || entry.issue === 'access_denied'
        ? '此目录未能读取，请先核对访问权限与路径；条件恢复后再选择此目录补充分析。'
        : '此目录尚未完整统计，先选择此目录补充分析，再判断处理方式。'
      : conflict
        ? '应用身份线索不一致，暂时保留。'
        : (recognition?.guidance ?? modelPurpose?.[1] ?? explanation),
    explanation,
    recognition,
    modelHint,
    basis: analysisBases[entry.advice?.basis ?? ''] ?? '缺少可确认的位置信息',
    canReveal: !entry.issue,
    canManageWindows: !recognition && kind === 'system_data',
  } as const;
}

import appCaches from '../rules/application-cache.json';
import type { RuleDecision } from './cleanupDecision';
import type { Rule } from './types';

/** Stable display groups. Neither registration nor this grouping grants permission. */
export function applicationGroups(rules: Rule[]) {
  const names: Record<string, string> = {
    vscode: 'Visual Studio Code',
    vscode_insiders: 'Visual Studio Code Insiders',
    trae: 'Trae',
    trae_cn: 'Trae CN',
    kiro: 'Kiro',
    lingma: '通义灵码',
    workbuddy: 'WorkBuddy',
    jetbrains: 'JetBrains IDE',
    visual_studio: 'Visual Studio',
    hbuilder: 'HBuilderX',
    adrive: '阿里云盘',
    '123pan': '123 云盘',
    cherry: 'Cherry Studio',
    apipost: 'Apipost',
    apipost7: 'Apipost 7',
    redis_manager: 'Another Redis Desktop Manager',
    ziniao: '紫鸟',
    bcut: '必剪',
    opencode: 'OpenCode',
    cursor: 'Cursor',
    edge: 'Microsoft Edge',
    chrome: 'Google Chrome',
    brave: 'Brave',
    vivaldi: 'Vivaldi',
    firefox: 'Firefox',
    weixin: '微信',
    qq: 'QQ',
    wps: 'WPS',
    dingtalk: '钉钉',
    feishu: '飞书',
    baidu_netdisk: '百度网盘',
    quark_netdisk: '夸克网盘',
    thunder: '迅雷',
    shadowbot: '影刀',
    codex: 'Codex',
    claude: 'Claude',
    docker: 'Docker Desktop',
    reqable: 'Reqable',
    doubao: '豆包',
    gameviewer: 'UU 远程',
    ccswitch: 'CC Switch',
    ima: 'ima',
    easyspider: 'EasySpider',
    codebuddy_extension: 'CodeBuddy 扩展',
    workbuddy_extension: 'WorkBuddy 扩展',
    todesk: 'ToDesk',
    webtoapi: 'WebToApi',
    dbx: 'DBX',
  };
  const groups = new Map<string, { id: string; title: string; rules: Rule[] }>();
  for (const rule of rules) {
    const spec = appCaches.find((s) => s.id === rule.id);
    const family = rule.id.startsWith('dev.')
      ? rule.id.slice(4).split('_')[0]
      : rule.id.split('.')[0];
    const app = spec?.app ?? rule.app_id ?? family;
    const id = names[app] ? app : rule.id;
    const group = groups.get(id) ?? { id, title: names[app] ?? '', rules: [] };
    group.rules.push(rule);
    groups.set(id, group);
  }
  return [...groups.values()];
}

/** Display taxonomy only: rule eligibility always comes from cleanupDecision. */
const builtinPurposes = [
  { id: 'temporary', title: '临时文件', rules: ['windows.user_temp', 'windows.system_temp'] },
  {
    id: 'browser',
    title: '浏览器缓存',
    rules: ['edge', 'chrome', 'brave', 'vivaldi']
      .flatMap((app) => ['http', 'code', 'gpu'].map((kind) => `${app}.${kind}`))
      .concat('firefox.http'),
  },
  {
    id: 'application',
    title: '应用缓存',
    rules: ['weixin', 'qq', 'feishu', 'baidu_netdisk']
      .flatMap((app) => ['http', 'code', 'gpu'].map((kind) => `${app}.${kind}`))
      .concat(
        'dingtalk.http',
        'dingtalk.code',
        'quark_netdisk.code',
        'quark_netdisk.gpu',
        'thunder.code',
        'thunder.gpu',
      ),
  },
  { id: 'graphics', title: '系统图形缓存', rules: ['windows.directx_cache'] },
  {
    id: 'developer',
    title: '开发工具缓存',
    rules: [
      'dev.npm_cache',
      'dev.npm_logs',
      'dev.pip_http',
      'dev.pip_wheels',
      'dev.nuget_http',
      'dev.vscode_cache',
      'dev.vscode_logs',
      'dev.jetbrains_logs',
      'dev.uv',
      'dev.poetry',
      'dev.pdm',
      'dev.conda_archives',
      'dev.pnpm',
      'dev.yarn',
      'dev.yarn_berry',
      'dev.bun',
      'dev.deno',
      'dev.cargo_archives',
      'dev.rustup_downloads',
      'dev.sccache',
      'dev.go_build',
      'dev.go_modules',
      'dev.gradle',
      'dev.gradle_logs',
      'dev.maven',
      'dev.nuget_packages',
      'dev.composer_archives',
      'dev.gem_archives',
      'dev.bundler',
      'dev.pub',
      'dev.vcpkg_archives',
      'dev.ccache',
      'dev.conan',
      'dev.android_logs',
      'dev.visual_studio',
      'dev.cursor_cache',
      'dev.cursor_logs',
    ],
  },
  {
    id: 'diagnostics',
    title: '日志与错误报告',
    rules: [
      'weixin.logs',
      'weixin.dumps',
      'wps.logs',
      'windows.crash_dumps',
      'windows.wer_archive',
      'windows.iis_logs',
    ],
  },
] as const;

const developerApps = new Set([
  'vscode',
  'vscode_insiders',
  'trae',
  'trae_cn',
  'kiro',
  'lingma',
  'workbuddy',
  'jetbrains',
  'visual_studio',
  'pycharm',
  'hbuilder',
]);
const cachePurpose = (spec: (typeof appCaches)[number]) =>
  'data_kind' in spec && spec.data_kind === 'diagnostics'
    ? 'diagnostics'
    : spec.kind === 'updates'
      ? 'updates'
      : developerApps.has(spec.app)
        ? 'developer'
        : 'application';
export const cleanupPurposes = [
  ...builtinPurposes,
  { id: 'updates', title: '应用更新下载内容（仅查看）', rules: [] },
].map((purpose) => ({
  ...purpose,
  rules: [
    ...purpose.rules,
    ...appCaches.filter((spec) => cachePurpose(spec) === purpose.id).map((spec) => spec.id),
  ],
}));
export function cleanupPurpose(ruleId: string) {
  return cleanupPurposes.find((p) => p.rules.includes(ruleId));
}
export function purposeGroups(items: RuleDecision[]) {
  return [...cleanupPurposes, { id: 'other', title: '其他项目', rules: [] }]
    .map((purpose) => ({
      ...purpose,
      items: items.filter((item) => (cleanupPurpose(item.rule.id)?.id ?? 'other') === purpose.id),
    }))
    .filter((group) => group.items.length > 0);
}

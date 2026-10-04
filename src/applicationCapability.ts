import type { ApplicationSupport, ReasonCode, Rule } from './types';

export type ApplicationCapability = 'allowed' | 'readonly' | 'unknown';
export const capabilityLabels: Record<ApplicationCapability, string> = {
  allowed: '已开放 · 扫描后核验',
  readonly: '仅只读扫描',
  unknown: '能力未确认',
};

// This only narrows UI eligibility. The session and backend remain authoritative.
export function applicationCapability(rule: Rule, applications: ApplicationSupport[]) {
  if (rule.cleanup_validated === false) return 'readonly' as const;
  if (!rule.app_id) return 'allowed' as const;
  const matches = applications.filter((a) => a.app_id === rule.app_id);
  const entries =
    matches.length === 1 ? matches[0].rules?.filter((r) => r.rule_id === rule.id) : [];
  if (entries?.length !== 1 || typeof entries[0].cleanup_validated !== 'boolean')
    return 'unknown' as const;
  const entry = entries[0];
  if (!entry.cleanup_validated) return 'readonly' as const;
  if (
    !Array.isArray(entry.cleanup_versions) ||
    !entry.cleanup_versions.length ||
    entry.cleanup_versions.some((v) => !/^\d+\.\d+\.\d+\.\d+$/.test(v)) ||
    entry.cleanup_block === undefined ||
    !entry.layout_id ||
    !entry.scan_basis
  )
    return 'unknown' as const;
  return 'allowed' as const;
}

export function applicationCondition(
  support?: ApplicationSupport,
  scanCode?: ReasonCode | null,
  ruleId?: string,
) {
  const conditions: string[] = [];
  if (!support) conditions.push('本机应用信息尚未确认');
  else if (!support.identity_verified) conditions.push('尚未识别可信的应用安装');
  else if (!support.scan_supported && !support.status) conditions.push('尚未确认支持的目录结构');
  const entry = ruleId ? support?.rules?.find((r) => r.rule_id === ruleId) : undefined;
  const codes = [ruleId ? entry?.cleanup_block : support?.status, scanCode];
  if (codes.includes('unsupported_layout')) conditions.push('资料位置未获支持');
  if (codes.includes('location_missing')) conditions.push('未发现该标准位置');
  if (codes.includes('directory_inaccessible') || codes.includes('access_denied'))
    conditions.push('部分位置无法读取，以扫描覆盖情况为准');
  if (codes.includes('unsupported_version')) conditions.push('当前组件或数据格式尚未适配');
  if (codes.includes('application_unverified') && !conditions.includes('尚未识别可信的应用安装'))
    conditions.push('尚未识别可信的应用安装');
  if (support?.running_components.length || codes.includes('application_running')) {
    conditions.push(
      `应用或后台组件仍在运行${support?.running_components.length ? `：${support.running_components.join('、')}` : ''}`,
    );
  }
  if (codes.includes('process_unknown')) conditions.push('运行状态尚未确认');
  if (entry?.scan_basis === 'unverified' && !scanCode && support?.identity_verified)
    conditions.push('此类目录结构尚未确认');
  return conditions.length
    ? [...new Set(conditions)].join('；')
    : entry?.scan_basis === 'recognized_layout' ||
        support?.rules?.some((r) => r.scan_basis === 'recognized_layout')
      ? '已识别目录结构，版本仅作观察'
      : '本机身份已核对，目录以扫描结果为准';
}

export function applicationRuntimeBlock(
  rule: Rule,
  applications: ApplicationSupport[],
): ReasonCode | null {
  if (!rule.app_id) return null;
  const support = applications.find((a) => a.app_id === rule.app_id);
  if (!support?.identity_verified) return 'application_unverified';
  const entry = support.rules?.find((r) => r.rule_id === rule.id);
  if (!entry || !Array.isArray(entry.cleanup_versions) || entry.cleanup_block === undefined)
    return 'unverifiable';
  if (!entry.cleanup_validated || !entry.cleanup_versions.length) return 'validation_pending';
  // Evidence versions describe past validation, not a client-side allowlist.
  if (!support.version) return 'application_unverified';
  if (!entry.scan_basis || entry.scan_basis === 'unverified' || !entry.layout_id)
    return entry.cleanup_block ?? 'unsupported_layout';
  if (support.running_components.length) return 'application_running';
  // An aggregate status can belong to another rule; only this rule authorizes its next step.
  return entry.cleanup_block;
}

export function applicationScope(rule: Rule) {
  if (['baidu_netdisk', 'quark_netdisk', 'thunder'].includes(rule.app_id ?? ''))
    return '仅检查指定缓存子目录；不含下载文件、下载任务、同步与离线资料、登录和持久存储。';
  if (rule.id === 'weixin.logs') return '仅检查至少七天的 .xlog 日志，不包含聊天记录与附件。';
  if (rule.id === 'weixin.dumps') return '仅检查至少三十天的 .dmp 转储，不包含聊天记录与附件。';
  if (rule.app_id === 'qq')
    return '仅检查经典 QQ 指定网页缓存；不含 QQNT、聊天媒体、登录与持久站点数据。';
  if (rule.id === 'wps.logs') return '仅检查至少七天的 .log 日志，不包含文档、备份与离线云文档。';
  if (rule.app_id === 'feishu')
    return '仅检查已识别账号配置中的指定网页缓存；保留聊天附件、离线邮件、文档与登录数据。';
  if (rule.app_id === 'dingtalk')
    return '仅检查标准数字后缀配置中的网页与代码缓存；保留聊天附件、离线资料与登录数据。';
  return '';
}

import rules from '../previewRules';
import type { ApplicationSupport } from '../types';
export const applicationFixtures: ApplicationSupport[] = [
  ['weixin', '微信', '4.1.13.65'],
  ['qq', '经典 QQ', '9.7.23.29394'],
  ['wps', 'WPS', '12.8.2.18205'],
  ['feishu', '飞书', '7.64.0.0'],
  ['dingtalk', '钉钉', '8.5.5.903'],
  ['baidu_netdisk', '百度网盘', '8.8.3.101'],
  ['quark_netdisk', '夸克网盘', '3.19.0.0'],
  ['thunder', '迅雷', '12.4.10.3940'],
].map(([app_id, title, version]) => ({
  app_id,
  title,
  version,
  supported_version: version,
  identity_verified: true,
  scan_supported: true,
  cleanup_validated: false,
  rules: rules
    .filter((r) => r.app_id === app_id)
    .map((r) => ({
      rule_id: r.id,
      cleanup_validated: false,
      cleanup_versions: [],
      cleanup_block: 'validation_pending' as const,
      scan_basis: 'verified_version' as const,
      layout_id: 'synthetic-layout',
    })),
  status: 'validation_pending',
  message: '',
  running_components: [],
  locations: [],
}));

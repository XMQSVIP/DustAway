import { expect, it } from 'vitest';
import { analysisDecision } from '../analysisDecision';
import type { AnalysisEntry } from '../types';

const entry: AnalysisEntry = {
  id: '1',
  parent: '0',
  name: 'name',
  path: 'D:\\sample',
  directory: false,
  bytes: '9007199254740993',
  files: 1,
  modified_unix: 0,
  shared: false,
  incomplete: false,
  issue: null,
  rule_ids: [],
  excluded: true,
  advice: { kind: 'app_data', app_id: null, basis: 'known_local_app_data' },
};
const recognition = (rule_id: string) => ({
  rule_id,
  title: '专属用途',
  description: '专属描述',
  guidance: '使用所属工具核对并保留备份。',
  evidence: '路径推断',
  source: 'https://example.com/docs',
  reviewed: '2026-09-28',
});
it('explains access conditions before suggesting another partial directory analysis', () => {
  const result = analysisDecision({
    ...entry,
    directory: true,
    incomplete: true,
    issue: 'directory_inaccessible',
  });
  expect(result.guidance).toContain('先核对访问权限与路径');
  expect(result.guidance).toContain('条件恢复后');
  expect(result.canReveal).toBe(false);
});

it.each([
  'model.gguf',
  'vm.docker_wsl',
  'dev.node_modules',
  'backup.macrium',
  'database.sqlite',
  'windows.pagefile',
])('uses specific guidance for %s without inventing cleanup or a Windows shortcut', (id) => {
  const value = {
    ...entry,
    advice: { ...entry.advice!, kind: 'system_data' as const, recognition: recognition(id) },
  };
  const result = analysisDecision(value);
  expect(result.guidance).toBe('使用所属工具核对并保留备份。');
  expect(result.primary).toBe('guide');
  expect(result.canManageWindows).toBe(false);
  expect(value.rule_ids).toEqual([]);
  expect(value.excluded).toBe(true);
});
it.each(['chrome', 'edge', 'brave', 'vivaldi', 'firefox', 'weixin', 'qq', 'wps'])(
  'matches only the declared %s guide',
  (app_id) => {
    expect(analysisDecision({ ...entry, advice: { ...entry.advice!, app_id } }).appId).toBe(app_id);
  },
);
it.each([
  ['app.chrome', 'chrome'],
  ['app.edge', 'edge'],
  ['app.firefox', 'firefox'],
  ['app.wps_backup', 'wps'],
  ...['baidu_netdisk', 'quark_netdisk', 'thunder'].flatMap((app) =>
    ['profile', 'web_cache', 'storage'].map((kind) => [`app.${app}_${kind}`, app]),
  ),
  ['app.thunder_partial', 'thunder'],
])('maps explicit recognition %s', (id, app) => {
  expect(
    analysisDecision({ ...entry, advice: { ...entry.advice!, recognition: recognition(id) } })
      .appId,
  ).toBe(app);
});
it('does not guess unknown apps, Chromium brands or conflicting identities', () => {
  expect(
    analysisDecision({ ...entry, advice: { ...entry.advice!, app_id: 'unknown' } }).appId,
  ).toBeNull();
  expect(
    analysisDecision({
      ...entry,
      advice: { ...entry.advice!, recognition: recognition('app.chromium') },
    }).appId,
  ).toBeNull();
  const result = analysisDecision({
    ...entry,
    advice: { ...entry.advice!, app_id: 'edge', recognition: recognition('app.chrome') },
  });
  expect(result.appId).toBeNull();
  expect(result.conflict).toBe(true);
  expect(result.guidance).toContain('线索不一致');
});
it('prioritizes partial directories and preserves backend category navigation without authorizing cleanup', () => {
  expect(
    analysisDecision({ ...entry, directory: true, pending: true, rule_ids: ['weixin.logs'] })
      .primary,
  ).toBe('refine');
  expect(analysisDecision({ ...entry, rule_ids: ['weixin.logs'] }).primary).toBe('cleanup');
  expect(analysisDecision({ ...entry, advice: undefined }).primary).toBe('reveal');
});

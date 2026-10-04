import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import { applicationCapability, applicationRuntimeBlock } from '../applicationCapability';
import { ApplicationSupportNotice } from '../ApplicationSupport';
import { applicationFixtures } from './applicationFixtures';
import rules from '../previewRules';

afterEach(cleanup);
const ids = [
  'weixin.logs',
  'dingtalk.http',
  'dingtalk.code',
  'baidu_netdisk.http',
  'baidu_netdisk.code',
  'baidu_netdisk.gpu',
];
function accepted(id: string) {
  const rule = { ...rules.find((r) => r.id === id)!, cleanup_validated: true };
  const applications = structuredClone(applicationFixtures);
  const app = applications.find((a) => a.app_id === rule.app_id)!;
  const entry = app.rules!.find((r) => r.rule_id === id)!;
  Object.assign(entry, {
    cleanup_validated: true,
    cleanup_versions: [app.version],
    cleanup_block: null,
  });
  return { rule, applications, app, entry };
}

it.each(ids)('%s requires its own acceptance metadata without a version allowlist', (id) => {
  const { rule, applications, app, entry } = accepted(id);
  expect(applicationCapability(rule, applications)).toBe('allowed');
  // Aggregate layout/version status may belong to a sibling and must not block this rule.
  app.status = 'unsupported_layout';
  expect(applicationRuntimeBlock(rule, applications)).toBeNull();
  for (const sibling of rules.filter((r) => r.app_id === app.app_id && r.id !== id)) {
    expect(applicationCapability(sibling, applications)).toBe('readonly');
  }
  app.version = '99.0.0.1';
  expect(applicationRuntimeBlock(rule, applications)).toBeNull();
  // Updating sample metadata does not grant or withdraw runtime permission.
  app.supported_version = app.version;
  expect(applicationRuntimeBlock(rule, applications)).toBeNull();
  entry.cleanup_versions = undefined;
  expect(applicationCapability(rule, applications)).toBe('unknown');
});

it.each(ids)('%s preserves fail-closed runtime conditions and readonly state', (id) => {
  const { rule, applications, app, entry } = accepted(id);
  for (const reason of [
    'process_unknown',
    'directory_inaccessible',
    'unsupported_layout',
    'unsupported_version', // Keep honoring older backend refusals.
  ] as const) {
    entry.cleanup_block = reason;
    expect(applicationRuntimeBlock(rule, applications)).toBe(reason);
  }
  entry.cleanup_block = null;
  app.running_components = ['background.exe'];
  expect(applicationRuntimeBlock(rule, applications)).toBe('application_running');
  app.running_components = [];
  expect(applicationRuntimeBlock(rule, applications)).toBeNull();
  entry.cleanup_block = undefined;
  expect(applicationCapability(rule, applications)).toBe('unknown');
  entry.cleanup_validated = false;
  expect(applicationCapability(rule, applications)).toBe('readonly');
});

it('shows cleanup versions separately from read-only scan samples', () => {
  const { app } = accepted('weixin.logs');
  app.supported_version = '99.0.0.1';
  render(
    <ApplicationSupportNotice
      support={app}
      loading={false}
      error=""
      busy={false}
      onRefresh={() => {}}
    />,
  );
  expect(screen.getByText(/清理验收样本：4\.1\.13\.65（不限制版本）/)).toBeInTheDocument();
  expect(screen.getByText(/已核实样本的文件版本：99\.0\.0\.1/)).toBeInTheDocument();
  expect(screen.getByText(/微信历史崩溃转储.*尚未开放清理/)).toBeInTheDocument();
});

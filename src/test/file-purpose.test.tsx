import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { EntryAdvice } from '../AnalysisAdvice';
import { AnalysisReviewDialog } from '../AnalysisReviewDialog';
import userEvent from '@testing-library/user-event';
import { analysisDecision } from '../analysisDecision';
import type { AnalysisEntry } from '../types';

afterEach(cleanup);
vi.mock('../api', () => ({ api: {} }));
const entry: AnalysisEntry = {
  id: 'example',
  parent: 'root',
  name: 'cache',
  path: 'D:\\Unlisted\\cache',
  directory: true,
  bytes: '1000',
  files: 1,
  modified_unix: 0,
  shared: false,
  incomplete: false,
  issue: null,
  rule_ids: [],
  excluded: false,
  manual: { allowed: false, reason: 'unverifiable' },
  advice: {
    kind: 'unknown',
    app_id: null,
    basis: 'unrecognized_location',
    model_hint: {
      purpose: 'cache',
      model_id: 'seed-v1',
      score_margin: 4.2,
      evidence: ['dir:cache'],
    },
  },
};

it('labels a CPU prediction as a guess without granting cleanup or showing a probability', () => {
  render(<EntryAdvice entry={entry} onReview={vi.fn()} />);
  expect(screen.getByText('可能是缓存 · 模型推测')).toBeVisible();
  expect(screen.getByText(/已保留/)).toBeVisible();
  expect(screen.queryByText(/4.2|%/)).not.toBeInTheDocument();
  expect(analysisDecision(entry).primary).toBe('reveal');
  expect(analysisDecision(entry).description).toContain('不代表可以删除');
  expect(entry.manual?.allowed).toBe(false);
});

it('keeps protected locations, existing recognition and incomplete results ahead of model hints', () => {
  for (const kind of ['personal', 'system_data', 'browser_data', 'cache'] as const) {
    expect(analysisDecision({ ...entry, advice: { ...entry.advice!, kind } }).modelHint).toBeNull();
  }
  for (const change of [
    { pending: true },
    { incomplete: true },
    { issue: 'access_denied' },
    { excluded: true },
    { rule_ids: ['chrome.http'] },
  ]) {
    expect(analysisDecision({ ...entry, ...change }).modelHint).toBeNull();
  }
  const recognized = {
    ...entry,
    advice: {
      ...entry.advice!,
      recognition: {
        rule_id: 'known',
        title: '已有用途',
        description: '规则说明',
        guidance: '核对应用',
        evidence: '固定位置',
        source: 'https://example.test',
        reviewed: '2026-10-25',
      },
    },
  };
  expect(analysisDecision(recognized).title).toBe('可能是已有用途');
  expect(analysisDecision(recognized).modelHint).toBeNull();
});

it('keeps the old unknown state when a model abstains or an older backend omits hints', () => {
  expect(analysisDecision({ ...entry, advice: { ...entry.advice!, model_hint: null } }).title).toBe(
    '用途未确定',
  );
  expect(analysisDecision({ ...entry, advice: undefined }).title).toBe('用途未确定');
});

it('explains the model evidence in details while keeping selection disabled', async () => {
  render(
    <AnalysisReviewDialog
      target={{
        key: 1,
        session: 'example',
        entry,
        location: { token: 'root', title: '测试目录', path: 'D:\\Unlisted' },
        analyzedAt: 1,
      }}
      open
      busy={false}
      stale={false}
      choices={new Map()}
      onClose={vi.fn()}
      onRestoreFocus={vi.fn()}
      onCleanup={vi.fn()}
      onRefine={vi.fn()}
      onInvalid={vi.fn()}
    />,
  );
  await userEvent.setup().click(screen.getByText('依据与完整路径'));
  expect(screen.getByText(/本地模型推测，尚未经真实机器数据验证/)).toBeVisible();
  expect(screen.getByText(/依据：目录名含 cache/)).toBeVisible();
  expect(screen.queryByText(/4.2|42%/)).not.toBeInTheDocument();
  expect(screen.queryByRole('checkbox', { name: /选择/ })).not.toBeInTheDocument();
});

import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { EntryAdvice } from '../AnalysisAdvice';
import { analysisDecision } from '../analysisDecision';
import type { AnalysisEntry } from '../types';

vi.mock('../api', () => ({ api: {} }));
afterEach(cleanup);

const entry: AnalysisEntry = {
  id: 'model',
  parent: 'root',
  name: 'model.gguf',
  path: 'D:\\models\\model.gguf',
  directory: false,
  bytes: '8000000000',
  files: 1,
  modified_unix: 0,
  shared: false,
  incomplete: false,
  issue: null,
  rule_ids: [],
  excluded: false,
  advice: {
    kind: 'unknown',
    app_id: null,
    basis: 'unrecognized_location',
    recognition: {
      rule_id: 'model.gguf',
      title: 'GGUF 模型文件',
      description: '通常保存本地推理模型。',
      guidance: '在模型应用中核对。',
      evidence: '文件扩展名 .gguf；未检查文件内容',
      source: 'https://huggingface.co/docs/hub/gguf',
      reviewed: '2026-09-27',
    },
  },
};
function show(value: AnalysisEntry) {
  render(<EntryAdvice entry={value} onReview={vi.fn()} />);
}

it('shows a short inferred purpose without repeating evidence or granting cleanup actions', () => {
  show(entry);
  expect(screen.getByText('可能是GGUF 模型文件')).toBeVisible();
  expect(screen.queryByText('通常保存本地推理模型。')).not.toBeInTheDocument();
  expect(screen.queryByText('在模型应用中核对。')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '查看对应清理类别' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button')).not.toBeInTheDocument();
});

it('keeps partial scan status and existing verified cleanup action', () => {
  show({
    ...entry,
    incomplete: true,
    rule_ids: ['verified.cache'],
    advice: { ...entry.advice!, kind: 'cache', basis: 'verified_rule_location' },
  });
  expect(screen.getByText(/大小尚未统计完整/)).toBeVisible();
  expect(
    analysisDecision({ ...entry, incomplete: true, rule_ids: ['verified.cache'] }).primary,
  ).toBe('cleanup');
});

it('keeps unknown fallback for unmatched or older results', () => {
  show({ ...entry, advice: undefined });
  expect(screen.getByText('用途未确定')).toBeVisible();
});

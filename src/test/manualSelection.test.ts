import { describe, expect, it } from 'vitest';
import {
  manualRoots,
  toggleManualBatch,
  manualChecked,
  manualMixed,
  manualRequest,
  toggleManual,
  type ManualChoices,
} from '../manualSelection';
import type { AnalysisEntry } from '../types';
const entry = (id: string, ancestors: string[] = [], directory = false): AnalysisEntry => ({
  id,
  ancestors,
  directory,
  parent: ancestors[0] ?? null,
  name: id,
  path: `G:\\fixture\\${id}`,
  bytes: '9007199254740993',
  files: 1,
  modified_unix: 0,
  shared: false,
  incomplete: false,
  issue: null,
  rule_ids: [],
  excluded: false,
  manual: { allowed: true, reason: null },
});
describe('explicit manual selection', () => {
  it('inherits folder selection across pages and keeps a child override', () => {
    const folder = entry('folder', [], true),
      child = entry('child', ['folder']);
    let choices: ManualChoices = new Map();
    choices = toggleManual(choices, folder, true);
    expect(manualChecked(child, choices)).toBe(true);
    choices = toggleManual(choices, child, false);
    expect(manualChecked(child, choices)).toBe(false);
    expect(manualMixed(folder, choices)).toBe(true);
    expect(manualRequest('session', choices)).toEqual({
      session: 'session',
      selected_ids: ['folder'],
      excluded_ids: ['child'],
    });
  });
  it('reselects a descendant within an excluded subtree and resets child overrides when toggling parent', () => {
    const folder = entry('folder', [], true),
      sub = entry('sub', ['folder'], true),
      child = entry('child', ['sub', 'folder']);
    let choices = toggleManual(new Map(), folder, true);
    choices = toggleManual(choices, sub, false);
    choices = toggleManual(choices, child, true);
    expect(manualChecked(child, choices)).toBe(true);
    expect(manualMixed(sub, choices)).toBe(true);
    choices = toggleManual(choices, folder, false);
    expect(choices.size).toBe(1);
    expect(manualRequest('session', choices)).toBeUndefined();
  });
  it('keeps protected or capability-unknown results unselected and sends no paths', () => {
    const unknown = { ...entry('unknown'), manual: undefined };
    const blocked = {
      ...entry('blocked'),
      manual: { allowed: false, reason: 'protected' as const },
    };
    let choices = toggleManual(new Map(), unknown, true);
    choices = toggleManual(choices, blocked, true);
    expect(choices.size).toBe(0);
    choices = toggleManual(choices, entry('file'), true);
    expect(JSON.stringify(manualRequest('session', choices))).not.toContain('fixture');
    expect(manualRequest('', choices)).toBeUndefined();
    const selectedParent = toggleManual(new Map(), entry('parent', [], true), true);
    expect(manualChecked({ ...unknown, ancestors: ['parent'] }, selectedParent)).toBe(false);
    expect(manualChecked({ ...blocked, ancestors: ['parent'] }, selectedParent)).toBe(false);
  });
});

it('batches a page without disturbing unrelated choices and merges parent and descendant ranges', () => {
  const parent = entry('parent', [], true),
    sub = entry('sub', ['parent'], true),
    file = entry('file', ['sub', 'parent']);
  const other = entry('other');
  let choices = toggleManual(new Map(), other, true);
  choices = toggleManualBatch(choices, [file, parent, sub], true);
  expect([...choices.keys()]).toEqual(['other', 'parent']);
  choices = toggleManual(choices, sub, false);
  choices = toggleManual(choices, file, true);
  expect(manualRoots(choices).map((e) => e.id)).toEqual(['other', 'parent']);
  expect(manualMixed(parent, choices)).toBe(true);
  choices = toggleManualBatch(choices, [parent, sub], false);
  expect(manualRoots(choices).map((e) => e.id)).toEqual(['other']);
  expect(manualChecked(file, choices)).toBe(false);
  choices = toggleManual(choices, file, true);
  expect(manualRoots(choices).map((e) => e.id)).toEqual(['other', 'file']);
});

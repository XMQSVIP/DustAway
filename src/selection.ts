import type { ExcludedFile, RuleSummary, Session } from './types';

export function selectedTotals(
  rows: RuleSummary[],
  selected: Set<string>,
  excluded: Map<string, ExcludedFile>,
  sessions: Session[],
  included: Map<string, ExcludedFile> = new Map(),
) {
  const totals = new Map<string, { files: number; bytes: bigint }>();
  for (const row of rows) {
    if (
      selected.has(row.rule_id) &&
      !row.blocked &&
      (row.phase === 'completed' || row.phase === 'partial')
    )
      totals.set(row.rule_id, { files: row.files, bytes: BigInt(row.bytes) });
  }
  for (const [id, value] of included) {
    if (selected.has(value.rule_id) || excluded.has(id)) continue;
    if (!sessions.some((s) => s.scope === value.scope && s.id === value.session)) continue;
    const source = rows.find((r) => r.rule_id === value.rule_id);
    if (!source || source.blocked || !['completed', 'partial'].includes(source.phase)) continue;
    const row = totals.get(value.rule_id) ?? { files: 0, bytes: 0n };
    row.files++;
    row.bytes += BigInt(value.bytes);
    totals.set(value.rule_id, row);
  }
  for (const value of excluded.values()) {
    if (!selected.has(value.rule_id)) continue;
    if (!sessions.some((s) => s.scope === value.scope && s.id === value.session)) continue;
    const row = totals.get(value.rule_id);
    if (row) {
      row.files = Math.max(0, row.files - 1);
      row.bytes = row.bytes > BigInt(value.bytes) ? row.bytes - BigInt(value.bytes) : 0n;
    }
  }
  return {
    byRule: totals,
    files: [...totals.values()].reduce((n, r) => n + r.files, 0),
    bytes: [...totals.values()].reduce((n, r) => n + r.bytes, 0n),
  };
}

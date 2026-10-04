import type { AnalysisEntry, ManualSelection } from './types';

export type ManualChoices = Map<string, { entry: AnalysisEntry; selected: boolean }>;
export function manualChecked(entry: AnalysisEntry, choices: ManualChoices): boolean {
  if (!entry.manual?.allowed) return false;
  for (const id of [entry.id, ...(entry.ancestors ?? [])]) {
    const choice = choices.get(id);
    if (choice) return choice.selected;
  }
  return false;
}
export function manualMixed(entry: AnalysisEntry, choices: ManualChoices): boolean {
  const checked = manualChecked(entry, choices);
  return (
    entry.directory &&
    !!entry.manual?.allowed &&
    [...choices.values()].some(
      (choice) => choice.entry.ancestors?.includes(entry.id) && choice.selected !== checked,
    )
  );
}
export function toggleManual(
  choices: ManualChoices,
  entry: AnalysisEntry,
  selected: boolean,
): ManualChoices {
  return toggleManualBatch(choices, [entry], selected);
}
export function toggleManualBatch(
  choices: ManualChoices,
  entries: AnalysisEntry[],
  selected: boolean,
): ManualChoices {
  const eligible = entries.filter((entry) => entry.manual?.allowed);
  if (!eligible.length) return choices;
  const ids = new Set(eligible.map((entry) => entry.id));
  const next = new Map(
    [...choices].filter(([, choice]) => !choice.entry.ancestors?.some((id) => ids.has(id))),
  );
  // Choosing a parent replaces its subtree overrides; redundant children need no entry.
  for (const entry of eligible)
    if (!entry.ancestors?.some((id) => ids.has(id))) next.set(entry.id, { entry, selected });
  return next;
}
export function manualRoots(choices: ManualChoices): AnalysisEntry[] {
  return [...choices.values()]
    .filter(
      (choice) =>
        choice.selected && !choice.entry.ancestors?.some((id) => choices.get(id)?.selected),
    )
    .map((choice) => choice.entry);
}
export function manualRequest(
  session: string,
  choices: ManualChoices,
): ManualSelection | undefined {
  const selected_ids = [...choices].filter(([, c]) => c.selected).map(([id]) => id);
  if (!session || !selected_ids.length) return undefined;
  return {
    session,
    selected_ids,
    excluded_ids: [...choices].filter(([, c]) => !c.selected).map(([id]) => id),
  };
}

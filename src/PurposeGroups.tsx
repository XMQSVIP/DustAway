import { formatRecordedTotal } from './cleanupResults';
import { useEffect, useState } from 'react';
import { ChevronDown } from 'lucide-react';
import type { RuleDecision } from './cleanupDecision';
import type { AppController } from './useAppController';
import { applicationGroups, purposeGroups } from './cleanupPurposes';
import { CleanupRuleList } from './CleanupRuleList';
import { formatBytes, reasonLabels } from './i18n';
import { isSelectableCleanup } from './cleanupPresentation';

export function PurposeGroups({
  model,
  items,
  policy,
  result = false,
}: {
  model: AppController;
  items: RuleDecision[];
  policy?: 'recommended' | 'review';
  result?: boolean;
}) {
  const [open, setOpen] = useState(new Set<string>());
  useEffect(() => setOpen(new Set()), [model.sessionKey]);
  return (
    <div className="purpose-groups">
      {purposeGroups(items)
        .sort((a, b) => {
          if (!policy || result) return 0;
          const size = (group: typeof a) =>
            group.items
              .filter(isSelectableCleanup)
              .reduce((sum, item) => sum + item.foundBytes, 0n);
          const left = size(a),
            right = size(b);
          return left === right ? 0 : left > right ? -1 : 1;
        })
        .map((group) => {
          const eligible = group.items.filter(
            (item) =>
              isSelectableCleanup(item) &&
              (item.rule.cleanup_policy === 'recommended' ? 'recommended' : 'review') === policy,
          );
          const files = eligible.reduce((sum, item) => sum + item.foundFiles, 0);
          const selected = eligible.reduce((sum, item) => sum + item.files, 0);
          const bytes = eligible.reduce((sum, item) => sum + item.bytes, 0n);
          const availableBytes = eligible.reduce((sum, item) => sum + item.foundBytes, 0n);
          const partial = selected > 0 && selected < files;
          const expanded = open.has(group.id);
          return (
            <section className="purpose-group" key={group.id} data-purpose={group.id}>
              <div className="purpose-heading">
                {policy && (
                  <input
                    type="checkbox"
                    aria-label={`选择${group.title}${policy === 'recommended' ? '建议项' : '可自行选择项'}`}
                    checked={files > 0 && selected === files}
                    aria-checked={partial ? 'mixed' : files > 0 && selected === files}
                    ref={(element) => {
                      if (element) element.indeterminate = partial;
                    }}
                    disabled={!files || !model.ready || model.busy || model.saving}
                    onChange={(event) =>
                      model.selectRuleGroup(
                        eligible.map((item) => item.rule.id),
                        policy,
                        event.target.checked,
                      )
                    }
                  />
                )}
                <button
                  className="purpose-toggle"
                  aria-expanded={expanded}
                  aria-label={`查看${group.title}${result ? '清理结果' : policy === 'recommended' ? '建议项' : policy === 'review' ? '可自行选择项' : '未清理项'}`}
                  onClick={() =>
                    setOpen((previous) => {
                      const next = new Set(previous);
                      expanded ? next.delete(group.id) : next.add(group.id);
                      return next;
                    })
                  }
                >
                  <strong>{group.title}</strong>
                  <span>
                    {result
                      ? `已删除 ${formatRecordedTotal(
                          model.reports
                            .flatMap((report) => report.categories ?? [])
                            .filter((category) =>
                              group.items.some((item) => item.rule.id === category.rule_id),
                            )
                            .map((category) => category.deleted_bytes),
                        )}`
                      : policy
                        ? `${files} 个文件 · 可选 ${formatBytes(availableBytes)} · 已选 ${selected} 个 / ${formatBytes(bytes)}`
                        : `发现 ${group.items.reduce((sum, item) => sum + (item.recognizedFiles ?? item.foundFiles), 0)} 个文件 · ${formatBytes(group.items.reduce((sum, item) => sum + (item.recognizedBytes ?? item.foundBytes), 0n))}`}
                  </span>
                  {!result && (
                    <small className="group-retained">
                      {(() => {
                        const rows = model.summaryRows.filter((row) =>
                          group.items.some((item) => item.rule.id === row.rule_id),
                        );
                        const retained = rows.reduce(
                          (sum, row) => sum + BigInt(row.observation?.retained_bytes || '0'),
                          0n,
                        );
                        const reasons = [
                          ...new Set(
                            rows.flatMap((row) =>
                              row.reasons
                                .filter((r) => r.count > 0)
                                .map((r) => reasonLabels[r.code]),
                            ),
                          ),
                        ];
                        return retained > 0n
                          ? `已保留 ${formatBytes(retained)}${reasons.length ? ` · ${reasons.slice(0, 2).join('、')}` : ''}`
                          : '';
                      })()}
                    </small>
                  )}
                  <ChevronDown size={17} className={expanded ? 'is-open' : ''} />
                </button>
              </div>
              <div hidden={!expanded}>
                {applicationGroups(group.items.map((item) => item.rule)).map((app) => (
                  <section
                    key={app.id}
                    className="application-result-group"
                    data-application={app.id}
                  >
                    {app.title && <h4>{app.title}</h4>}
                    <CleanupRuleList model={model} rules={app.rules} />
                  </section>
                ))}
              </div>
            </section>
          );
        })}
    </div>
  );
}

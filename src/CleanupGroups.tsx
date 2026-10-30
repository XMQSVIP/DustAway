import { useEffect, useRef, useState } from 'react';
import { ChevronDown } from 'lucide-react';
import { cleanupGroups } from './cleanupCatalog';
import { CleanupRuleList } from './CleanupRuleList';
import { ApplicationSupportNotice } from './ApplicationSupport';
import { formatBytes } from './i18n';
import type { RuleDecision } from './cleanupDecision';
import type { AppController } from './useAppController';

function SuggestedSelection({
  model,
  items,
  title,
}: {
  model: AppController;
  items: RuleDecision[];
  title: string;
}) {
  const ref = useRef<HTMLInputElement>(null);
  const allowed = items.filter(
    (r) => r.rule.cleanup_policy === 'recommended' && ['ready', 'deselected'].includes(r.status),
  );
  const foundFiles = allowed.reduce((n, r) => n + r.foundFiles, 0);
  const allSelected = foundFiles > 0 && allowed.reduce((n, r) => n + r.files, 0) === foundFiles;
  const selectedFiles = allowed.reduce((n, r) => n + r.files, 0);
  const partial =
    selectedFiles > 0 && selectedFiles < allowed.reduce((n, r) => n + r.foundFiles, 0);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = partial;
  }, [partial]);
  if (!allowed.length || model.reports.length) return null;
  return (
    <label className="group-selection">
      <input
        ref={ref}
        type="checkbox"
        aria-label={`选择${title}建议项`}
        checked={allSelected}
        aria-checked={partial ? 'mixed' : allSelected}
        disabled={model.busy || model.saving}
        onChange={(e) =>
          model.selectRecommended(
            allowed.map((r) => r.rule.id),
            e.target.checked,
          )
        }
      />
      选择本组建议项
    </label>
  );
}

export function CleanupGroups({
  model,
  items,
  catalog = false,
  inspectedRules,
}: {
  model: AppController;
  items: RuleDecision[];
  catalog?: boolean;
  inspectedRules?: string[] | null;
}) {
  const [closed, setClosed] = useState<Set<string>>(new Set());
  useEffect(() => {
    setClosed(new Set());
  }, [model.sessionKey]);
  useEffect(() => {
    if (inspectedRules)
      setClosed((current) => {
        const next = new Set(current);
        for (const group of cleanupGroups)
          if (inspectedRules.some((rule) => rule.startsWith(group.id + '.'))) {
            next.delete(group.id);
            next.add('catalog-open-' + group.id);
          }
        return next;
      });
  }, [inspectedRules]);
  return (
    <div
      className={catalog ? 'scope-groups' : 'cleanup-groups'}
      aria-label={catalog ? '本版本检查范围' : '按应用核对'}
    >
      {cleanupGroups.map((group) => {
        const entries = items.filter((item) => item.rule.id.startsWith(group.id + '.'));
        if (!entries.length) return null;
        const open = !closed.has(group.id) && (!catalog || closed.has('catalog-open-' + group.id));
        const toggle = () =>
          setClosed((current) => {
            const next = new Set(current);
            if (catalog) next.add('catalog-open-' + group.id);
            open ? next.add(group.id) : next.delete(group.id);
            return next;
          });
        const running = entries.some((r) => ['running', 'process_unknown'].includes(r.status));
        return (
          <section className="cleanup-group" key={group.id} data-app-id={group.id}>
            <div className="cleanup-group-heading">
              <button
                className="group-heading"
                aria-expanded={open}
                aria-label={catalog ? group.title : `查看${group.title}类别`}
                onClick={toggle}
              >
                <group.icon size={20} />
                <strong>{group.title}</strong>
                {!catalog && (
                  <span>{formatBytes(entries.reduce((sum, r) => sum + r.foundBytes, 0n))}</span>
                )}
                <ChevronDown size={16} className={open ? 'is-open' : ''} />
              </button>
              {!catalog && <SuggestedSelection model={model} items={entries} title={group.title} />}
            </div>
            <div hidden={!open}>
              <CleanupRuleList
                model={model}
                rules={entries.map((item) => item.rule)}
                catalog={catalog}
              />
              {running && (
                <button
                  className="button secondary small recheck-application"
                  disabled={model.busy || model.saving}
                  onClick={model.refreshAvailability}
                >
                  重新检查{group.title}状态
                </button>
              )}
              {group.id !== 'windows' && (
                <div className="application-followup">
                  {group.book === 'applications' && model.applicationInfo && (
                    <details className="application-identity">
                      <summary>应用版本与检查位置</summary>
                      <ApplicationSupportNotice
                        rules={model.boot?.rules ?? []}
                        support={model.applicationInfo.applications.find(
                          (a) => a.app_id === group.id,
                        )}
                        loading={model.applicationInfo.loading}
                        error={model.applicationInfo.error}
                        busy={model.busy}
                        onRefresh={model.refreshAvailability}
                      />
                    </details>
                  )}
                </div>
              )}
            </div>
          </section>
        );
      })}
    </div>
  );
}

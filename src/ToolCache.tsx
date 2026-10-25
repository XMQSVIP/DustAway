import { useEffect, useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { api, desktop } from './api';
import { formatBytes } from './i18n';
import { CleanupEvidence, cleanupEvidence } from './CleanupEvidence';
import { UvReceipt, type UvPlan, type UvReport } from './UvCache';
import type { AppController } from './useAppController';
import type { RuleSummary } from './types';
import { RuleRefreshNotice, hasToolIssue } from './RuleRefresh';

export const toolActions = {
  'dev.go_build': '清空 Go 构建缓存',
  'dev.yarn': '清空 Yarn Classic 下载缓存',
  'dev.ccache': '整理 ccache 旧编译缓存',
} as const;
export { isOfficialRule } from './scanAssessment';
export interface ToolPlan extends UvPlan {
  rule_id: string;
  title: string;
}
export interface ToolReport extends UvReport {
  rule_id: string;
  title: string;
  session?: string;
  reason?: string | null;
}

export function useToolTask(enabled: boolean, session: string, rows?: RuleSummary[]) {
  const [task, setTask] = useState<ToolReport | null>(null);
  const [error, setError] = useState('');
  const [stale, setStale] = useState<Record<string, { session: string; revision: number }>>({});
  const [results, setResults] = useState<Record<string, ToolReport>>({});
  const observed = useRef(new Set<string>());
  const invalidate = (rule: string) =>
    setStale((previous) => ({
      ...previous,
      [rule]: { session, revision: rows?.find((r) => r.rule_id === rule)?.revision ?? 0 },
    }));
  useEffect(() => {
    if (!task) return;
    setResults((rows) => ({ ...rows, [task.rule_id]: task }));
    if (session && !observed.current.has(task.id)) {
      observed.current.add(task.id);
      if (task.session === session && !rows) invalidate(task.rule_id);
    }
  }, [task, session]);
  async function refresh() {
    try {
      setTask(await api.toolTask());
      setError('');
    } catch (e) {
      setError(String(e));
    }
  }
  useEffect(() => {
    if (!enabled || !desktop || !api.toolTask) return;
    let valid = true;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        const value = await api.toolTask();
        if (valid) {
          setTask(value);
          setError('');
        }
      } catch (e) {
        if (valid) setError(String(e));
      } finally {
        if (valid) timer = setTimeout(poll, 1200);
      }
    }
    void poll();
    return () => {
      valid = false;
      clearTimeout(timer);
    };
  }, [enabled]);
  return {
    task,
    error,
    results: Object.fromEntries(
      Object.entries(results).filter(([, report]) => report.session === session),
    ),
    refresh,
    invalidate,
    isStale: (rule: string) => {
      const row = rows?.find((r) => r.rule_id === rule);
      return (
        !!row?.stale ||
        (!!session &&
          stale[rule]?.session === session &&
          stale[rule]?.revision === (row?.revision ?? 0))
      );
    },
  };
}

export function ToolCacheCards({ model }: { model: AppController }) {
  return (
    <>
      {Object.entries(toolActions).map(([id, title]) => {
        const item = model.decision.items.find((r) => r.rule.id === id);
        const report = model.maintenance?.results[id];
        const row = model.summaryRows?.find((r) => r.rule_id === id);
        const refreshing =
          model.ruleRefresh?.rule_id === id && model.ruleRefresh.state === 'scanning';
        if (!item?.foundFiles && !report && !hasToolIssue(row) && !refreshing) return null;
        return <ToolCacheCard key={id} model={model} id={id} title={title} />;
      })}
    </>
  );
}
export function ToolCacheCard({
  model,
  id,
  title,
}: {
  model: AppController;
  id: string;
  title: string;
}) {
  const section = useRef<HTMLElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const pending = useRef(false);
  const [working, setWorking] = useState(false);
  const [plan, setPlan] = useState<ToolPlan | null>(null);
  const [error, setError] = useState('');
  const preparation = useRef<string | null>(null);
  const state = model.maintenance;
  const item = model.decision.items.find((r) => r.rule.id === id);
  const report = state?.results[id];
  const stale = state?.isStale(id);
  const row = model.summaryRows?.find((r) => r.rule_id === id);
  const impact = cleanupEvidence(id)?.conclusion;
  async function prepare() {
    const session = model.sessions.find((s) => s.scope === 'user');
    if (!session || pending.current) return;
    pending.current = true;
    setWorking(true);
    setError('');
    const preparationId = crypto.randomUUID();
    preparation.current = preparationId;
    try {
      const prepared = await api.prepareTool(session.id, id, preparationId);
      if (preparation.current === preparationId) setPlan(prepared);
    } catch (e) {
      setError(String(e));
    } finally {
      const cancelled = preparation.current !== preparationId;
      pending.current = false;
      setWorking(false);
      preparation.current = null;
      if (cancelled)
        requestAnimationFrame(() =>
          (trigger.current?.disabled ? section.current : trigger.current)?.focus({
            preventScroll: true,
          }),
        );
    }
  }
  async function execute() {
    if (!plan || pending.current) return;
    pending.current = true;
    setWorking(true);
    setError('');
    state.invalidate(id);
    try {
      await api.executeTool(plan.id);
      model.setHistoryEpoch((v) => v + 1);
    } catch (e) {
      setError(String(e));
    } finally {
      setPlan(null);
      await state.refresh();
      pending.current = false;
      setWorking(false);
    }
  }
  return (
    <section
      ref={section}
      className="result-section official-cache-tools"
      aria-label={title}
      tabIndex={-1}
    >
      <h3>{title}</h3>
      <p>{impact}</p>
      <p className="muted">官方工具操作，不计入逐文件已选大小。</p>
      {stale ? (
        <p>此项结果待更新；其他选择已保留。检查这一项后可再次操作。</p>
      ) : (
        !!item?.foundFiles && <p>扫描发现 {formatBytes(item.foundBytes)}，可释放大小尚未确定。</p>
      )}
      {!!item?.foundFiles && (
        <button
          ref={trigger}
          className="button secondary small"
          disabled={
            model.busy ||
            working ||
            model.saving ||
            stale ||
            !!state?.error ||
            item.incomplete ||
            !row?.tool?.ready
          }
          onClick={() => void prepare()}
        >
          {title}
        </button>
      )}
      <CleanupEvidence ruleId={id} />
      <RuleRefreshNotice model={model} rule={id} />
      {working && !plan && (
        <button
          className="result-link"
          onClick={() => {
            const token = preparation.current;
            preparation.current = null;
            if (token) void api.cancelToolPreparation(token).catch((e) => setError(String(e)));
          }}
        >
          取消检查
        </button>
      )}
      {(error || state?.error) && <p role="alert">{error || state.error}</p>}
      {state?.error && (
        <button className="result-link" onClick={() => void state.refresh()}>
          重新查询状态
        </button>
      )}
      {report && (
        <aside className="message info" role="status" aria-label={`${title}结果`}>
          <strong>
            {report.status === 'running'
              ? '正在处理缓存'
              : report.status === 'completed'
                ? '处理完成，请核对结果'
                : '处理已停止或需要核对'}
          </strong>
          <UvReceipt report={report} />
          {report.status === 'running' ? (
            <button
              className="button secondary small"
              onClick={() =>
                void api
                  .cancelTool()
                  .then(state.refresh)
                  .catch((e) => setError(String(e)))
              }
            >
              停止整理
            </button>
          ) : null}
        </aside>
      )}
      <Dialog.Root
        open={!!plan}
        onOpenChange={(open) => {
          if (!open && !working) setPlan(null);
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="overlay" />
          <Dialog.Content
            className="dialog uv-dialog"
            onEscapeKeyDown={(e) => {
              if (working) e.preventDefault();
            }}
            onCloseAutoFocus={(e) => {
              e.preventDefault();
              (trigger.current?.disabled ? section.current : trigger.current)?.focus();
            }}
          >
            <Dialog.Title>{title}？</Dialog.Title>
            <Dialog.Description>{impact} 扫描大小不等于可释放空间。</Dialog.Description>
            <div className="uv-dialog-body">
              <p className="break-path">缓存位置：{plan?.cache}</p>
              <p className="break-path">执行工具：{plan?.executable}</p>
              <p>停止时已处理内容不会恢复；超时、断线和重启不会自动重试。</p>
            </div>
            <div className="dialog-actions">
              <Dialog.Close className="button secondary" disabled={working}>
                取消
              </Dialog.Close>
              <button
                className="button primary"
                disabled={working || model.busy}
                onClick={() => void execute()}
              >
                {working ? '正在确认…' : '确认执行'}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </section>
  );
}

import { ToolCacheCards, isOfficialRule } from './ToolCache';
import { useEffect, useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { api, desktop } from './api';
import { formatBytes } from './i18n';
import type { AppController } from './useAppController';
import type { RuleSummary } from './types';
import { RuleRefreshNotice, confirmedEmptyResult, hasToolIssue } from './RuleRefresh';

export interface UvPlan {
  id: string;
  session: string;
  cache: string;
  executable: string;
  expires_at: number;
}
export interface UvReport {
  session?: string;
  id: string;
  started_at: number;
  finished_at: number | null;
  status: string;
  cache: string;
  command: string;
  message: string;
  output: string;
  before_bytes: string | null;
  after_bytes: string | null;
  free_before: string | null;
  free_after: string | null;
}
export function useUvTask(enabled: boolean, sessionKey = '', row?: RuleSummary) {
  const [task, setTask] = useState<UvReport | null>(null);
  const [error, setError] = useState('');
  const [invalidatedSession, invalidate] = useState<string | null>(null);
  const observed = useRef(new Set<string>());
  const invalidatedRevision = useRef(0);
  const invalidateCurrent = (session: string) => {
    invalidatedRevision.current = row?.revision ?? 0;
    invalidate(session);
  };
  useEffect(() => {
    if (task && sessionKey && !observed.current.has(task.id)) {
      observed.current.add(task.id);
      if (task.session === sessionKey && !row) invalidateCurrent(sessionKey);
    }
  }, [task?.id, sessionKey]);
  async function refresh() {
    try {
      const task = await api.uvTask();
      setTask(task);
      setError('');
    } catch (e) {
      setError(String(e));
    }
  }
  useEffect(() => {
    if (!enabled || !desktop || !api.uvTask) return;
    let valid = true;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        const value = await api.uvTask();
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
    refresh,
    invalidate: invalidateCurrent,
    stale:
      !!row?.stale ||
      (!!sessionKey &&
        invalidatedSession === sessionKey &&
        invalidatedRevision.current === (row?.revision ?? 0)),
  };
}

export function OfficialCacheTools({ model }: { model: AppController }) {
  const item = model.decision.items.find((r) => r.rule.id === 'dev.uv');
  const row = model.summaryRows?.find((r) => r.rule_id === 'dev.uv');
  const session = model.sessions.find((s) => s.scope === 'user')?.id;
  const emptyUv = confirmedEmptyResult(row) && !row?.observation?.retained_files;
  const showUv =
    (model.ruleRefresh?.rule_id === 'dev.uv' && model.ruleRefresh.state === 'scanning') ||
    (!emptyUv && !!item?.foundFiles) ||
    !!(
      model.uv?.task &&
      model.uv.task.session === session &&
      (!emptyUv || model.uv.task.status === 'running')
    ) ||
    hasToolIssue(row);
  const showOther =
    (!!model.ruleRefresh &&
      model.ruleRefresh.rule_id !== 'dev.uv' &&
      model.ruleRefresh.state === 'scanning' &&
      isOfficialRule(model.ruleRefresh.rule_id)) ||
    model.decision.items.some(
      (r) => r.rule.id !== 'dev.uv' && r.foundFiles && isOfficialRule(r.rule.id),
    ) ||
    model.summaryRows?.some(
      (r) => r.rule_id !== 'dev.uv' && isOfficialRule(r.rule_id) && hasToolIssue(r),
    ) ||
    Object.values(model.maintenance?.results ?? {}).length > 0;
  if (!showUv && !showOther) return null;
  return (
    <>
      <h3 id="official-tools-heading" tabIndex={-1}>
        官方工具整理
      </h3>
      {showUv && (
        <section
          className="result-section official-cache-tools"
          aria-label="官方工具整理"
          tabIndex={-1}
        >
          <h3>uv 闲置缓存</h3>
          <p>uv 闲置缓存 · 由官方工具判断可移除内容，不计入已选清理大小。</p>
          {model.uv?.stale ? (
            <p className="muted">uv 结果待更新。其他清理项的选择已保留；检查这一项后可再次整理。</p>
          ) : item && item.foundFiles > 0 ? (
            <p className="muted">扫描发现 {formatBytes(item.foundBytes)}，可释放大小尚未确定。</p>
          ) : null}
          {!!item?.foundFiles && <UvCacheAction model={model} />}
          <RuleRefreshNotice model={model} rule="dev.uv" />
          <UvTaskNotice model={model} />
          {model.uv?.error && !model.uv.task && (
            <>
              <p role="alert">整理状态读取失败：{model.uv.error}</p>
              <button className="result-link" onClick={() => void model.uv.refresh()}>
                重新查询状态
              </button>
            </>
          )}
        </section>
      )}
      <ToolCacheCards model={model} />
    </>
  );
}
export function UvReceipt({ report }: { report: UvReport }) {
  return (
    <div className="software-history-detail">
      <p>{report.message}</p>
      <p>
        缓存逻辑大小：{report.before_bytes === null ? '未取得' : formatBytes(report.before_bytes)} →{' '}
        {report.after_bytes === null ? '未取得' : formatBytes(report.after_bytes)}
        。包含共享文件，差值不代表实际释放磁盘空间。
      </p>
      <p>
        所在磁盘可用空间：{report.free_before === null ? '未取得' : formatBytes(report.free_before)}{' '}
        → {report.free_after === null ? '未取得' : formatBytes(report.free_after)}
        ，也会受其他程序写入影响。
      </p>
      <details>
        <summary>工具输出与处理位置</summary>
        <p className="break-path">{report.cache}</p>
        <pre className="uv-output">{report.output || '没有已确认的工具输出'}</pre>
      </details>
    </div>
  );
}
export function UvTaskNotice({ model }: { model: AppController }) {
  const [error, setError] = useState('');
  const task = model.uv?.task;
  if (!task || task.session !== model.sessions.find((s) => s.scope === 'user')?.id) return null;
  return (
    <aside className="message info uv-task-notice" aria-label="uv 整理状态" role="status">
      <strong>
        {task.status === 'running'
          ? '正在整理 uv 闲置缓存'
          : task.status === 'completed'
            ? 'uv 整理完成'
            : 'uv 整理需要核对'}
      </strong>
      <UvReceipt report={task} />
      {(error || model.uv.error) && <p role="alert">{error || model.uv.error}</p>}
      {model.uv.error && (
        <button className="result-link" onClick={() => void model.uv.refresh()}>
          重新查询状态
        </button>
      )}
      {task.status === 'running' ? (
        <button
          className="button secondary small"
          onClick={() => {
            void api
              .cancelUv()
              .then(model.uv.refresh)
              .catch((e) => setError(String(e)));
          }}
        >
          停止整理
        </button>
      ) : null}
    </aside>
  );
}
export function UvCacheAction({ model }: { model: AppController }) {
  const trigger = useRef<HTMLButtonElement>(null);
  const [plan, setPlan] = useState<UvPlan | null>(null);
  const [working, setWorking] = useState(false);
  const submitting = useRef(false);
  const preparation = useRef<string | null>(null);
  const [error, setError] = useState('');
  async function prepare() {
    const session = model.sessions.find((s) => s.scope === 'user');
    if (!session || preparation.current) return;
    const token = crypto.randomUUID();
    preparation.current = token;
    setWorking(true);
    setError('');
    try {
      const prepared = await api.prepareUv(session.id, token);
      if (preparation.current === token) setPlan(prepared);
    } catch (e) {
      setError(String(e));
    } finally {
      const cancelled = preparation.current !== token;
      preparation.current = null;
      setWorking(false);
      if (cancelled) requestAnimationFrame(() => trigger.current?.focus({ preventScroll: true }));
    }
  }
  async function execute() {
    if (!plan || submitting.current) return;
    submitting.current = true;
    setWorking(true);
    setError('');
    model.uv.invalidate?.(model.sessions.find((s) => s.scope === 'user')?.id ?? '');
    try {
      await api.executeUv(plan.id);
      setPlan(null);
      await model.uv.refresh();
      model.setHistoryEpoch((v) => v + 1);
    } catch (e) {
      setPlan(null);
      setError(String(e));
      await model.uv.refresh();
    } finally {
      submitting.current = false;
      setWorking(false);
    }
  }
  return (
    <div className="uv-action">
      <button
        ref={trigger}
        className="button secondary small"
        disabled={
          model.busy ||
          working ||
          model.saving ||
          model.uv?.stale ||
          !!model.uv?.error ||
          !model.summaryRows?.find((r) => r.rule_id === 'dev.uv')?.tool?.ready
        }
        onClick={() => void prepare()}
      >
        整理 uv 闲置缓存
      </button>
      {error && !plan && <p role="alert">{error}</p>}
      {working && !plan && preparation.current && (
        <button
          className="result-link"
          onClick={() => {
            const token = preparation.current;
            preparation.current = null;
            if (token) void api.cancelUvPreparation(token).catch((e) => setError(String(e)));
          }}
        >
          取消检查
        </button>
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
            onCloseAutoFocus={(event) => {
              event.preventDefault();
              if (trigger.current?.disabled) {
                trigger.current.closest<HTMLElement>('.official-cache-tools')?.focus();
              } else trigger.current?.focus();
            }}
            onEscapeKeyDown={(e) => {
              if (working) e.preventDefault();
            }}
          >
            <Dialog.Title>整理 uv 闲置缓存？</Dialog.Title>
            <Dialog.Description>
              由 uv 官方工具判断哪些缓存不再需要。扫描发现的总大小不等于本次能释放的大小。
            </Dialog.Description>
            <div className="uv-dialog-body">
              <p>
                会移除闲置缓存；缓存中的集中式项目环境也可能被移除，下次使用时重新创建。常规项目
                .venv、已安装工具和 Python 解释器保留。
              </p>
              <p>
                整理后可能需要重新下载或构建。停止时已经处理的内容不会恢复；重新连接不会自动续删。
              </p>
              <p className="break-path">缓存位置：{plan?.cache}</p>
              <details>
                <summary>执行工具</summary>
                <p className="break-path">{plan?.executable}</p>
                <code>uv cache prune</code>
              </details>
              {error && <p role="alert">{error}</p>}
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
                {working ? '正在确认…' : '确认整理闲置缓存'}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </div>
  );
}

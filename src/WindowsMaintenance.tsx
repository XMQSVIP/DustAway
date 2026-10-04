import { useEffect, useRef, useState } from 'react';
import { api, desktop } from './api';
import type { Scope } from './types';
import type { AppController } from './useAppController';
import { formatBytes } from './i18n';

export interface WindowsAction {
  id: string;
  title: string;
  scope: Scope;
  impact: string;
  estimate_bytes: string | null;
  volume: string | null;
  available: boolean;
  validated: boolean;
  message: string;
}
export interface WindowsPlan {
  id: string;
  session: string;
  actions: WindowsAction[];
  expires_at: number;
}
export interface ScopedWindowsPlan {
  scope: Scope;
  plan: WindowsPlan;
}
export interface WindowsReport {
  id: string;
  scope: Scope;
  started_at: number;
  finished_at: number | null;
  status: string;
  actions: {
    id: string;
    title: string;
    status: string;
    message: string;
    before_bytes: string | null;
    after_bytes: string | null;
    free_before: string | null;
    free_after: string | null;
  }[];
}
export interface WindowsSnapshot {
  session: string;
  status: string;
  actions: WindowsAction[];
  plan: WindowsPlan | null;
  report: WindowsReport | null;
  current: string | null;
  message: string;
  progress_bytes?: string | null;
}
const working = (status?: string) =>
  ['scanning', 'preparing', 'running', 'stopping'].includes(status ?? '');
const statusLabel: Record<string, string> = {
  idle: '等待检查',
  scanning: '正在估算',
  ready: '估算完成',
  preparing: '正在核验',
  prepared: '等待确认',
  running: '正在维护',
  stopping: '正在停止',
  completed: '维护完成',
  failed: '未全部完成',
  cancelled: '已停止',
  uncertain: '结果待核实',
};
export function useWindowsMaintenance(enabled: boolean, system: boolean) {
  const [snapshots, setSnapshots] = useState<Partial<Record<Scope, WindowsSnapshot>>>({});
  const [selected, setSelected] = useState(new Set<string>());
  const [plans, setPlans] = useState<ScopedWindowsPlan[]>([]);
  const [error, setError] = useState('');
  const cancelled = useRef(false);
  const scopes: Scope[] = system ? ['user', 'system'] : ['user'];
  async function read(scope: Scope) {
    let data: WindowsSnapshot | null;
    try {
      data = await api.windowsSnapshot(scope);
    } catch (error) {
      setPlans([]);
      setSnapshots((previous) => {
        const old = previous[scope];
        if (!old) return previous;
        return {
          ...previous,
          [scope]: {
            ...old,
            status: 'uncertain',
            plan: null,
            actions: old.actions.map((action) => ({ ...action, available: false })),
            message: '组件连接中断，维护结果待核实。请重新打开拂尘查看回执；不会自动重试删除。',
          },
        };
      });
      throw error;
    }
    if (data)
      setSnapshots((previous) =>
        JSON.stringify(previous[scope]) === JSON.stringify(data)
          ? previous
          : { ...previous, [scope]: data },
      );
    return data;
  }
  useEffect(() => {
    if (!enabled || !desktop || !api.windowsSnapshot) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function tick() {
      try {
        for (const scope of scopes) {
          if (!active) return;
          await read(scope);
        }
      } catch (e) {
        if (active) setError(String(e));
      }
      if (active) timer = setTimeout(() => void tick(), 1000);
    }
    void tick();
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [enabled, system]);
  const busy = Object.values(snapshots).some((s) => working(s.status));
  async function scan(scope: Scope) {
    setError('');
    setPlans([]);
    setSelected(
      (previous) =>
        new Set([...previous].filter((id) => !snapshots[scope]?.actions.some((a) => a.id === id))),
    );
    try {
      await api.windowsScan(scope);
      await read(scope);
    } catch (e) {
      setError(String(e));
    }
  }
  async function prepare() {
    cancelled.current = false;
    const prepared: ScopedWindowsPlan[] = [];
    for (const scope of scopes) {
      const snapshot = snapshots[scope];
      const ids =
        snapshot?.actions.filter((a) => a.available && selected.has(a.id)).map((a) => a.id) ?? [];
      if (!snapshot || !ids.length) continue;
      await api.windowsPrepare(scope, snapshot.session, ids);
      while (true) {
        if (cancelled.current) throw new Error('Windows 维护准备已取消');
        const current = await read(scope);
        if (current?.status === 'prepared' && current.plan) {
          prepared.push({ scope, plan: current.plan });
          break;
        }
        if (!current || !working(current.status))
          throw new Error(current?.message || 'Windows 维护计划未准备完成');
        await new Promise((resolve) => setTimeout(resolve, 250));
      }
    }
    return prepared;
  }
  async function cancel() {
    cancelled.current = true;
    setPlans([]);
    try {
      await api.windowsCancel();
    } catch (e) {
      setError(String(e));
    }
  }
  return { snapshots, selected, setSelected, plans, setPlans, error, busy, scan, prepare, cancel };
}
export function WindowsMaintenancePanel({ model }: { model: AppController }) {
  const maintenance = model.windowsMaintenance;
  const [history, setHistory] = useState<WindowsReport[]>([]);
  const [historyError, setHistoryError] = useState('');
  async function loadHistory() {
    const rows = await api.windowsHistory('user');
    if (model.boot?.capabilities.system_cleanup && maintenance.snapshots.system)
      rows.push(...(await api.windowsHistory('system')));
    setHistory(rows);
  }
  return (
    <details className="windows-maintenance">
      <summary>Windows 官方维护 · 单独估算与确认</summary>
      <p>由 Windows 按类别整理。下列为官方估算，不计入逐文件扫描或已选文件大小。</p>
      <div className="system-scan-entry">
        <button disabled={model.busy || model.saving} onClick={() => void maintenance.scan('user')}>
          估算缩略图与着色器缓存
        </button>
        {model.boot?.capabilities.system_cleanup ? (
          <button
            disabled={model.busy || model.saving}
            onClick={() => void maintenance.scan('system')}
          >
            授权估算系统维护
          </button>
        ) : (
          <button onClick={() => void api.openWindowsStorage()}>打开 Windows 存储设置</button>
        )}
        {maintenance.busy && (
          <button onClick={() => void maintenance.cancel()}>停止 Windows 维护</button>
        )}
      </div>
      {maintenance.error && <p role="alert">{maintenance.error}</p>}
      {Object.entries(maintenance.snapshots).map(
        ([scope, snapshot]) =>
          snapshot && (
            <div key={scope}>
              {snapshot.status !== 'idle' && (
                <p role="status">
                  {statusLabel[snapshot.status] ?? snapshot.status}
                  {snapshot.current ? ` · ${snapshot.current}` : ''} {snapshot.message}
                </p>
              )}
              {working(snapshot.status) && snapshot.progress_bytes != null && (
                <p>Windows 回调进度：{formatBytes(snapshot.progress_bytes)} · 不代表实际删除量</p>
              )}
              {snapshot.actions.map((action) => (
                <article className="windows-action" key={action.id}>
                  <label>
                    <input
                      type="checkbox"
                      checked={maintenance.selected.has(action.id)}
                      disabled={
                        !action.available ||
                        !['ready', 'prepared'].includes(snapshot.status) ||
                        model.busy ||
                        model.saving
                      }
                      onChange={(event) =>
                        maintenance.setSelected((previous) => {
                          const next = new Set(previous);
                          event.target.checked ? next.add(action.id) : next.delete(action.id);
                          return next;
                        })
                      }
                    />
                    <strong>{action.title}</strong>
                    <span>
                      {action.estimate_bytes === null
                        ? '估算未知'
                        : `约 ${formatBytes(action.estimate_bytes)}`}
                    </span>
                  </label>
                  <p>{action.message}</p>
                  <details>
                    <summary>处理范围与影响</summary>
                    <p>
                      {action.volume} · {action.impact}
                    </p>
                  </details>
                </article>
              ))}
              {snapshot.report && <WindowsReceipt report={snapshot.report} />}
            </div>
          ),
      )}
      {maintenance.selected.size > 0 && (
        <p>
          已选 Windows 维护 {maintenance.selected.size} 项 ·
          点击“清理已选”核验并确认；逐文件清理完成后再执行。
        </p>
      )}
      <details>
        <summary>当前保留的系统类别</summary>
        <p>
          Defender 扫描历史与定义备份、Windows Search 事务日志、Prefetch、未覆盖的 CBS/DISM
          日志当前不提供直接删除。这不表示这些位置没有占用。
        </p>
      </details>
      <details
        onToggle={(event) => {
          if (event.currentTarget.open) {
            setHistoryError('');
            void loadHistory().catch((error) => setHistoryError(String(error)));
          }
        }}
      >
        <summary>Windows 维护历史</summary>
        {historyError && <p role="alert">历史记录读取失败：{historyError}</p>}
        {history.map((report) => (
          <WindowsReceipt key={report.id} report={report} />
        ))}
      </details>
    </details>
  );
}
function WindowsReceipt({ report }: { report: WindowsReport }) {
  return (
    <div className="windows-receipt">
      <h4>
        {statusLabel[report.status] ?? report.status} ·{' '}
        {new Date(report.started_at * 1000).toLocaleString()}
      </h4>
      {report.actions.map((action) => (
        <p key={action.id}>
          {action.title}：{statusLabel[action.status] ?? action.status} · {action.message}
          <br />
          官方估算 {action.before_bytes === null ? '未知' : formatBytes(action.before_bytes)} →{' '}
          {action.after_bytes === null ? '未知' : formatBytes(action.after_bytes)}
          <br />
          磁盘可用空间 {action.free_before === null
            ? '未知'
            : formatBytes(action.free_before)} →{' '}
          {action.free_after === null ? '未知' : formatBytes(action.free_after)}
        </p>
      ))}
      <small>估算变化与磁盘变化均不等同于实际删除字节数。</small>
    </div>
  );
}

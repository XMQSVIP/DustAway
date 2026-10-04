import type { ToolReport } from './ToolCache';
import { useEffect, useState } from 'react';
import { ChevronDown, History, Trash2 } from 'lucide-react';
import { ReportRow } from './ReportReceipt';
import { LayerScene } from './LayerScene';
import { resultTitle, formatRecordedBytes } from './cleanupResults';
import { formatTime, ui } from './i18n';
import { api } from './api';
import { standardUninstallLabel, uninstallTaskLabel, type UninstallReport } from './softwareTypes';
import type { Report } from './types';
import type { AppController } from './useAppController';
import { UvReceipt, type UvReport } from './UvCache';

type Receipt =
  | { kind: 'cleanup'; report: Report }
  | { kind: 'uninstall'; report: UninstallReport }
  | { kind: 'uv'; report: UvReport | ToolReport };

function useToolHistory(
  load: (() => Promise<UvReport[]>) | undefined,
  epoch: number,
  status?: string,
) {
  const [rows, setRows] = useState<UvReport[]>([]);
  const [error, setError] = useState('');
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let valid = true;
    void (load?.() ?? Promise.resolve([])).then(
      (value) => {
        if (valid) {
          setRows(value);
          setError('');
        }
      },
      (reason) => {
        if (valid) setError(String(reason));
      },
    );
    return () => {
      valid = false;
    };
  }, [load, epoch, status, retry]);
  return { rows, error, retry: () => setRetry((value) => value + 1) };
}
export function HistoryPage({ model }: { model: AppController }) {
  const [filter, setFilter] = useState<'all' | 'cleanup' | 'uninstall'>('all');
  const [software, setSoftware] = useState<UninstallReport[]>([]);
  const uvHistory = useToolHistory(api.uvHistory, model.historyEpoch, model.uv?.task?.status);
  const toolHistory = useToolHistory(
    api.toolHistory,
    model.historyEpoch,
    model.maintenance?.task?.status,
  );
  const uv = [...uvHistory.rows, ...toolHistory.rows];
  const [softwareError, setSoftwareError] = useState('');
  const [loading, setLoading] = useState(false);
  // A terminal result can arrive in the same second as the previous phase.
  const uninstallResult = model.uninstall
    ? JSON.stringify([
        model.uninstall.id,
        model.uninstall.phase,
        model.uninstall.uninstall_status,
        model.uninstall.message,
        model.uninstall.residual,
      ])
    : '';
  useEffect(() => {
    let valid = true;
    setLoading(true);
    setSoftwareError('');
    (api.uninstallHistory?.() ?? Promise.resolve([]))
      .then((rows) => {
        if (valid) setSoftware(rows);
      })
      .catch((e) => {
        if (valid) setSoftwareError(String(e));
      })
      .finally(() => {
        if (valid) setLoading(false);
      });
    return () => {
      valid = false;
    };
  }, [model.historyEpoch, model.uninstall?.updated_at, uninstallResult]);
  const receipts: Receipt[] = [
    ...model.history.map((report) => ({ kind: 'cleanup' as const, report })),
    ...software.map((report) => ({ kind: 'uninstall' as const, report })),
    ...uv.map((report) => ({ kind: 'uv' as const, report })),
  ]
    .filter(
      (r) => filter === 'all' || r.kind === filter || (filter === 'cleanup' && r.kind === 'uv'),
    )
    .sort((a, b) => b.report.started_at - a.report.started_at);
  const days = new Map<string, Receipt[]>();
  for (const receipt of receipts) {
    const day = new Date(receipt.report.started_at * 1000).toLocaleDateString('zh-CN');
    days.set(day, [...(days.get(day) ?? []), receipt]);
  }
  const error = model.historyError || softwareError;
  return (
    <section className="page-stage history-page">
      <header className="page-heading">
        <div>
          <h1>操作历史</h1>
          <p>
            清理与卸载的实际处理结果。常规记录：{ui.historyRetention}{' '}
            官方工具整理记录保留至手动清空。
          </p>
        </div>
        <button
          className="button secondary small"
          disabled={(!model.history.length && !software.length && !uv.length) || model.busy}
          onClick={() => model.setClearDialog(true)}
        >
          <Trash2 size={16} />
          清空历史记录
        </button>
      </header>
      <div className="history-filter" role="group" aria-label="历史类型">
        {(
          [
            ['all', '全部'],
            ['cleanup', '清理'],
            ['uninstall', '卸载'],
          ] as const
        ).map(([key, label]) => (
          <button
            key={key}
            className={`button ${filter === key ? 'primary' : 'secondary'} small`}
            aria-label={`${label}记录`}
            aria-pressed={filter === key}
            onClick={() => setFilter(key)}
          >
            {label}
          </button>
        ))}
      </div>
      {error && (
        <div className="message error" role="alert">
          历史读取失败：{error}
          <button
            className="button secondary small"
            onClick={() => model.setHistoryEpoch((v) => v + 1)}
          >
            重试读取历史
          </button>
        </div>
      )}
      {(
        [
          ['uv', uvHistory],
          ['其他官方工具', toolHistory],
        ] as const
      ).map(
        ([label, source]) =>
          source.error && (
            <div key={label} className="message error" role="alert">
              {label}历史读取失败：{source.error}。已读取的记录仍保留。
              <button className="button secondary small" onClick={source.retry}>
                重新读取{label}历史
              </button>
            </div>
          ),
      )}
      {(model.historyLoading || loading) && <p role="status">正在读取历史记录…</p>}
      {receipts.length ? (
        <div className="receipt-archive" aria-label="操作回执">
          {[...days].map(([day, entries]) => (
            <section className="receipt-day" key={day}>
              <h2>
                <History size={16} />
                {day}
              </h2>
              <div>
                {entries.map((entry) => (
                  <details className="history-receipt" key={`${entry.kind}:${entry.report.id}`}>
                    <summary>
                      <time>{formatTime(entry.report.started_at)}</time>
                      <strong>
                        {entry.kind === 'cleanup'
                          ? resultTitle(entry.report)
                          : entry.kind === 'uv'
                            ? `${'title' in entry.report ? entry.report.title : 'uv 缓存整理'} · ${entry.report.status === 'completed' ? '已完成' : entry.report.status === 'running' ? '进行中' : '需核对'}`
                            : `${entry.report.software.name} · ${uninstallTaskLabel(entry.report)}`}
                      </strong>
                      <span>
                        {entry.kind === 'cleanup'
                          ? `已删除 ${formatRecordedBytes(entry.report.deleted_bytes)}`
                          : entry.kind === 'uv'
                            ? '工具管理的缓存'
                            : `残留删除 ${formatRecordedBytes(entry.report.residual.deleted_bytes)}`}
                      </span>
                      <ChevronDown size={16} />
                    </summary>
                    {entry.kind === 'cleanup' ? (
                      <ReportRow report={entry.report} rules={model.boot?.rules ?? []} />
                    ) : entry.kind === 'uv' ? (
                      <UvReceipt report={entry.report} />
                    ) : (
                      <div className="software-history-detail">
                        <p>{entry.report.message}</p>
                        <p>标准卸载：{standardUninstallLabel(entry.report)}。</p>
                        <p>
                          版本：{entry.report.software.version || '未知'} ·{' '}
                          {entry.report.software.source === 'store' ? '商店应用' : '桌面软件'}
                        </p>
                        <p>
                          残留文件 {entry.report.residual.deleted_files} 个，空文件夹{' '}
                          {entry.report.residual.deleted_directories} 个；保留{' '}
                          {entry.report.residual.skipped} 项，失败 {entry.report.residual.failed}{' '}
                          项，待核对 {entry.report.residual.unknown} 项，未处理{' '}
                          {entry.report.residual.unprocessed ?? 0} 项。
                        </p>
                      </div>
                    )}
                  </details>
                ))}
              </div>
            </section>
          ))}
        </div>
      ) : (
        !error &&
        !model.historyLoading &&
        !loading && (
          <div className="history-empty">
            <LayerScene stage="complete" />
            <h2>{filter === 'uninstall' ? '还没有卸载记录' : '还没有清理记录'}</h2>
            <p>完成一次处理后，这里会留下实际结果。</p>
            <button
              className="button secondary"
              onClick={() => model.setTab(filter === 'uninstall' ? 'software' : 'clean')}
            >
              {filter === 'uninstall' ? '去软件卸载' : '去清理垃圾'}
            </button>
          </div>
        )
      )}
    </section>
  );
}

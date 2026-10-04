import { useEffect, useMemo, useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import * as AlertDialog from '@radix-ui/react-alert-dialog';
import {
  AppWindow,
  ArrowLeft,
  ChevronRight,
  ExternalLink,
  LoaderCircle,
  RefreshCw,
  Search,
  Trash2,
  X,
} from 'lucide-react';
import { api, desktop } from './api';
import { formatBytes } from './i18n';
import type { AppController } from './useAppController';
import {
  type InstalledSoftware,
  type SoftwareList,
  type UninstallPlan,
  type ResidualScan,
  type ResidualCleanupPlan,
  uninstallActive,
  standardUninstallLabel,
  uninstallTaskLabel,
  residualKindLabels,
  residualInspectionLabels,
  residualCleanupLabels,
  softwareMissingLabels,
} from './softwareTypes';
import './software.css';

function SoftwareIcon({ software }: { software: InstalledSoftware }) {
  const [failedSource, setFailedSource] = useState<string | null>(null);
  const source = software.icon_data_url;
  return (
    <span className="software-icon" aria-hidden="true">
      {source?.startsWith('data:image/png;base64,') && source !== failedSource ? (
        <img
          src={source}
          alt=""
          width={32}
          height={32}
          loading="lazy"
          decoding="async"
          onError={() => setFailedSource(source)}
        />
      ) : (
        <AppWindow size={22} />
      )}
    </span>
  );
}

export function filterSoftware(
  items: InstalledSoftware[],
  search: string,
  source: string,
  sort: string,
) {
  const query = search.trim().toLocaleLowerCase();
  return items
    .filter(
      (s) =>
        (source === 'all' || s.source === source) &&
        `${s.name} ${s.publisher}`.toLocaleLowerCase().includes(query),
    )
    .sort((a, b) => {
      if (sort === 'size') {
        if (a.estimated_bytes === null)
          return b.estimated_bytes === null ? a.name.localeCompare(b.name, 'zh-CN') : 1;
        if (b.estimated_bytes === null) return -1;
        const diff = BigInt(b.estimated_bytes) - BigInt(a.estimated_bytes);
        if (diff) return diff > 0n ? 1 : -1;
      }
      if (sort === 'date') {
        const diff = (b.installed_on ?? '').localeCompare(a.installed_on ?? '');
        if (diff) return diff;
      }
      return a.name.localeCompare(b.name, 'zh-CN') || a.id.localeCompare(b.id);
    });
}

export function SoftwarePage({ model, active }: { model: AppController; active: boolean }) {
  const [list, setList] = useState<SoftwareList | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [search, setSearch] = useState('');
  const [source, setSource] = useState('all');
  const [sort, setSort] = useState('name');
  const [page, setPage] = useState(0);
  const [detail, setDetail] = useState<InstalledSoftware | null>(null);
  const [plan, setPlan] = useState<UninstallPlan | null>(null);
  const [cleanupConfirmation, setCleanupConfirmation] = useState<{
    scanId: string;
    plan: ResidualCleanupPlan;
  } | null>(null);
  const [working, setWorking] = useState(false);
  const [loadedResiduals, setResiduals] = useState<ResidualScan | null>(null);
  const [residualLoading, setResidualLoading] = useState(false);
  const [residualSearch, setResidualSearch] = useState('');
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [dismissedTask, setDismissedTask] = useState<string | null>(null);
  const [openedTask, setOpenedTask] = useState<string | null>(null);
  const task = model.uninstall;
  const showTask =
    !!task &&
    dismissedTask !== task.id &&
    (!['finished', 'removed'].includes(task.phase) ||
      task.can_scan_residuals ||
      openedTask === task.id);
  const residuals =
    loadedResiduals?.task_id === task?.id && loadedResiduals?.scan_id === task?.residual_scan_id
      ? loadedResiduals
      : null;
  const cleanPlan =
    task?.phase === 'residuals_ready' && cleanupConfirmation?.scanId === task.residual_scan_id
      ? (cleanupConfirmation?.plan ?? null)
      : null;
  function setCleanPlan(plan: ResidualCleanupPlan | null) {
    setCleanupConfirmation(
      plan && task?.residual_scan_id ? { scanId: task.residual_scan_id, plan } : null,
    );
  }
  const autoScanned = useRef(new Set<string>());
  const request = useRef(0);
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  const taskRef = useRef<HTMLElement | null>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);
  const previousShowTask = useRef(showTask);
  useEffect(() => {
    if (active && previousShowTask.current !== showTask) {
      (showTask ? taskRef.current : searchRef.current)?.focus();
    }
    previousShowTask.current = showTask;
  }, [active, showTask]);
  function returnToList() {
    if (!task) return;
    setDismissedTask(task.id);
    setSelected(new Set());
    setCleanPlan(null);
    setError('');
  }
  function restoreFocus() {
    const button = buttonRef.current;
    if (button?.isConnected && !button.disabled && !button.closest('[hidden]')) button.focus();
    else taskRef.current?.focus();
  }
  const disabled = !desktop || model.busy || working;
  const canChooseResiduals = selected.size > 0 || residuals?.items.some((r) => r.selectable);
  const items = useMemo(
    () => filterSoftware(list?.items ?? [], search, source, sort),
    [list, search, source, sort],
  );
  useEffect(() => {
    if (task && ['removed', 'residuals_ready', 'finished'].includes(task.phase))
      setList((previous) =>
        previous
          ? { ...previous, items: previous.items.filter((item) => item.id !== task.software.id) }
          : previous,
      );
    if (task?.phase !== 'residuals_ready') {
      setSelected(new Set());
      setCleanPlan(null);
    }
  }, [task?.id, task?.phase]);
  async function refresh() {
    const sequence = ++request.current;
    setLoading(true);
    setError('');
    setPlan(null);
    setDetail(null);
    try {
      const value = await api.softwareList();
      if (sequence === request.current) {
        setList(value);
        setPage(0);
      }
    } catch (e) {
      if (sequence === request.current) setError(String(e));
    } finally {
      if (sequence === request.current) setLoading(false);
    }
  }
  useEffect(() => {
    if (active && !showTask && !list && !loading && !error && !uninstallActive(task?.phase))
      void refresh();
  }, [active, showTask, list, loading, error, task?.phase]);
  useEffect(
    () => () => {
      request.current++;
    },
    [],
  );
  useEffect(() => {
    setPage(0);
  }, [search, source, sort]);
  useEffect(() => {
    setResiduals(null);
    setSelected(new Set());
    setCleanPlan(null);
    setOffset(0);
  }, [task?.id, task?.residual_scan_id]);
  useEffect(() => setResidualSearch(''), [task?.id]);
  useEffect(() => {
    if (!showTask || !task || task.phase !== 'residuals_ready' || !task.residual_scan_id) {
      setResiduals(null);
      setResidualLoading(false);
      return;
    }
    let valid = true;
    setResidualLoading(true);
    setResiduals(null);
    api
      .softwareResiduals(task.id, offset, residualSearch)
      .then((value) => {
        if (valid && value.task_id === task.id && value.scan_id === task.residual_scan_id)
          setResiduals(value);
      })
      .catch((e) => {
        if (valid) setError(String(e));
      })
      .finally(() => {
        if (valid) setResidualLoading(false);
      });
    return () => {
      valid = false;
    };
  }, [showTask, task?.id, task?.phase, task?.residual_scan_id, offset, residualSearch]);
  async function run(action: () => Promise<unknown>) {
    if (working) return;
    setWorking(true);
    setError('');
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      await model.reconcile();
      setWorking(false);
    }
  }
  useEffect(() => {
    if (
      task?.phase === 'removed' &&
      desktop &&
      task.can_scan_residuals &&
      !autoScanned.current.has(task.id) &&
      !working &&
      !model.busy
    ) {
      autoScanned.current.add(task.id);
      void run(() => api.scanSoftwareResiduals(task.id));
    }
  }, [task?.id, task?.phase, task?.can_scan_residuals, working, model.busy]);
  const sourceLabel = (s: InstalledSoftware) => (s.source === 'store' ? '商店应用' : '桌面软件');
  const hasResult =
    !!task &&
    (task.phase === 'finished' ||
      task.residual.deleted_files +
        task.residual.deleted_directories +
        task.residual.skipped +
        task.residual.failed +
        task.residual.unknown +
        (task.residual.unprocessed ?? 0) >
        0);
  const guidance =
    task &&
    (
      {
        uninstalling:
          task.software.source === 'store'
            ? 'Windows 正在卸载此应用。'
            : '请完成软件自带的卸载向导，拂尘会自动检查结果。',
        verifying: '正在确认软件是否已卸载。',
        scanning_residuals: '正在查找留下的文件，不会删除任何内容。',
        residuals_ready: !residuals
          ? '正在读取检查结果…'
          : !residuals.supported
            ? '该软件暂不支持残留检查。'
            : residuals.total === 0
              ? '残留检查已结束。'
              : residuals.total === residuals.items.length &&
                  residuals.items.every((r) => !r.selectable)
                ? '找到的残留暂不支持清理，已全部保留。'
                : '选择要清理的内容，不选的会保留。',
        cleaning_residuals: '正在清理你已确认的内容。',
        finished: '残留处理已结束。',
        unknown:
          task.uninstall_status === 'removed'
            ? '软件已卸载，残留处理结果还需核对。不会自动继续删除。'
            : '上次卸载的结果还不确定，请检查卸载结果。不会重新运行卸载。',
      } as Partial<Record<typeof task.phase, string>>
    )[task.phase];
  return (
    <section className="page-stage software-page" aria-label="软件卸载">
      <header className="page-heading">
        <div>
          <h1>软件卸载</h1>
          <p>卸载软件后，选择是否清理残留。</p>
        </div>
        <button
          className="button secondary small"
          hidden={showTask}
          disabled={loading || model.busy || working}
          onClick={() => void refresh()}
        >
          <RefreshCw size={16} aria-hidden="true" />
          刷新列表
        </button>
      </header>
      {!desktop && (
        <p className="message info">
          网页演示数据，不代表本机已安装软件。卸载与清理仅在桌面版可用。
        </p>
      )}
      {error && (
        <div className="message error" role="alert">
          <span>{error}</span>
          {!showTask && (
            <button
              className="button secondary small"
              disabled={working || model.busy}
              onClick={() => void refresh()}
            >
              重新读取列表
            </button>
          )}
        </div>
      )}
      {task && showTask && (
        <section
          className="software-task"
          aria-label="本次卸载任务"
          ref={taskRef}
          tabIndex={-1}
          key={task.id}
        >
          {!uninstallActive(task.phase) &&
            !['finished', 'residuals_ready'].includes(task.phase) && (
              <button
                className="button secondary small software-back"
                disabled={working}
                onClick={returnToList}
              >
                <ArrowLeft size={16} aria-hidden="true" />
                返回软件列表
              </button>
            )}
          <div className="software-task-heading">
            <div>
              <h2>{task.software.name}</h2>
            </div>
            <strong role="status">
              {uninstallActive(task.phase) && (
                <LoaderCircle size={16} className="spin" aria-hidden="true" />
              )}
              {uninstallTaskLabel(task)}
            </strong>
          </div>
          <p>{guidance ?? task.message}</p>
          {hasResult && (
            <p className="software-result">
              残留实际清理 <strong>{formatBytes(task.residual.deleted_bytes)}</strong>
              {task.residual.unknown > 0 && <span> · {task.residual.unknown} 项结果待核对</span>}
            </p>
          )}
          {task.progress !== null && (
            <progress max={100} value={task.progress} aria-label="系统卸载进度" />
          )}
          <div className="inline-actions">
            {['unknown', 'failed', 'still_installed', 'cancelled', 'restart_required'].includes(
              task.phase,
            ) && (
              <button
                className="button secondary"
                disabled={disabled}
                onClick={() => void run(() => api.verifySoftware(task.id))}
              >
                检查卸载结果
              </button>
            )}
            {task.can_scan_residuals &&
              task.phase === 'removed' &&
              autoScanned.current.has(task.id) &&
              !working && (
                <button
                  className="button secondary"
                  disabled={disabled}
                  onClick={() => {
                    setSelected(new Set());
                    setCleanPlan(null);
                    void run(() => api.scanSoftwareResiduals(task.id));
                  }}
                >
                  重新检查残留
                </button>
              )}
            {['scanning_residuals', 'cleaning_residuals'].includes(task.phase) && (
              <button
                className="button secondary"
                disabled={working || model.closing}
                onClick={() => void run(() => api.stopSoftwareResiduals(task.id))}
              >
                {task.phase === 'scanning_residuals' ? '停止检查' : '停止清理'}
              </button>
            )}
          </div>
          <details className="software-disclosure" open={task.phase === 'unknown'}>
            <summary>查看处理详情</summary>
            {guidance && guidance !== task.message && <p>{task.message}</p>}
            <p>
              标准卸载：{standardUninstallLabel(task)} · 残留实际删除：
              {formatBytes(task.residual.deleted_bytes)}
            </p>
            {hasResult && (
              <p>
                实际删除 {task.residual.deleted_files} 个文件、{task.residual.deleted_directories}{' '}
                个空文件夹，共 {formatBytes(task.residual.deleted_bytes)}；保留{' '}
                {task.residual.skipped} 项，失败 {task.residual.failed} 项，待核对{' '}
                {task.residual.unknown} 项，未处理 {task.residual.unprocessed ?? 0} 项。
              </p>
            )}
            {task.can_scan_residuals && ['residuals_ready', 'finished'].includes(task.phase) && (
              <button
                className="button secondary small"
                disabled={disabled}
                onClick={() => {
                  setSelected(new Set());
                  setCleanPlan(null);
                  void run(() => api.scanSoftwareResiduals(task.id));
                }}
              >
                重新检查残留
              </button>
            )}
          </details>
          {task.phase === 'finished' && (
            <button className="button primary" onClick={returnToList}>
              返回软件列表
            </button>
          )}
          {task.phase === 'residuals_ready' && (
            <section aria-label="卸载残留" className="software-residuals">
              <div className="software-tools">
                <label>
                  <span>搜索残留路径</span>
                  <input
                    value={residualSearch}
                    onChange={(e) => {
                      setResidualSearch(e.target.value);
                      setOffset(0);
                    }}
                  />
                </label>
              </div>
              {residualLoading && <p role="status">正在读取残留清单…</p>}
              {!!residuals?.warnings.length && (
                <details className="software-disclosure software-residual-warning">
                  <summary>部分位置未能检查，相关文件已保留</summary>
                  {residuals.warnings.map((w) => (
                    <p key={w}>{w}</p>
                  ))}
                </details>
              )}
              {residuals && !residuals.supported && <p>暂不支持该软件的残留检查。</p>}
              {residuals?.supported && residuals.total === 0 && (
                <p>
                  {residualSearch
                    ? '没有匹配的残留项目。'
                    : residuals.complete
                      ? '没有发现已支持范围内的残留。'
                      : '部分位置未能检查，不能确认没有残留。'}
                </p>
              )}
              {residuals &&
                ['cache', 'data', 'retained'].map((group) => {
                  const records = residuals.items.filter((r) =>
                    group === 'retained' ? !r.selectable : r.selectable && r.category === group,
                  );
                  if (!records.length) return null;
                  const rows = (
                    <>
                      {group === 'data' && (
                        <p>可能包含登录状态、浏览记录、离线内容或文档备份；删除后不会自动恢复。</p>
                      )}
                      {records.map((r) => (
                        <label className="residual-row" key={r.id}>
                          <input
                            type="checkbox"
                            checked={selected.has(r.id)}
                            disabled={disabled || !r.selectable}
                            onChange={(e) => {
                              const chosen = e.target.checked;
                              setSelected((prev) => {
                                const next = new Set(prev);
                                if (chosen) next.add(r.id);
                                else next.delete(r.id);
                                return next;
                              });
                              setCleanPlan(null);
                            }}
                            aria-label={`选择 ${r.path}`}
                          />
                          <span>
                            <strong>
                              {r.path.split(/[\\/]/).filter(Boolean).at(-1) || r.path}
                            </strong>
                            <small className="residual-path">{r.path}</small>
                            <small>
                              {r.reason ??
                                (r.directory
                                  ? '仅在清空后删除此文件夹'
                                  : r.category === 'data'
                                    ? '设置与本地资料'
                                    : '缓存与日志')}
                            </small>
                          </span>
                          <span>{r.directory ? '文件夹' : formatBytes(r.bytes)}</span>
                        </label>
                      ))}
                    </>
                  );
                  if (group === 'retained')
                    return (
                      <details key={group} className="residual-group software-disclosure">
                        <summary>
                          已保留 {records.length} 项{residuals.total > 200 ? '（本页）' : ''}
                        </summary>
                        {rows}
                      </details>
                    );
                  const allChosen = records.every((r) => selected.has(r.id));
                  return (
                    <section key={group} className="residual-group">
                      <div className="residual-group-heading">
                        <h3>{group === 'cache' ? '缓存与日志' : '设置与本地资料'}</h3>
                        {group === 'cache' && records.length > 1 && (
                          <button
                            className="button secondary small"
                            disabled={disabled || residualLoading}
                            onClick={() => {
                              setSelected((previous) => {
                                const next = new Set(previous);
                                for (const record of records) {
                                  if (allChosen) next.delete(record.id);
                                  else next.add(record.id);
                                }
                                return next;
                              });
                              setCleanPlan(null);
                            }}
                          >
                            {allChosen ? '取消本页缓存选择' : '选择本页缓存'}
                          </button>
                        )}
                      </div>
                      {rows}
                    </section>
                  );
                })}
              {residuals && residuals.total > 200 && (
                <div className="inline-actions">
                  <button
                    className="button secondary small"
                    disabled={!offset || residualLoading}
                    onClick={() => setOffset((v) => Math.max(0, v - 200))}
                  >
                    上一页
                  </button>
                  <span>
                    {offset + 1}–{Math.min(offset + 200, residuals.total)} / {residuals.total}
                  </span>
                  <button
                    className="button secondary small"
                    disabled={offset + 200 >= residuals.total || residualLoading}
                    onClick={() => setOffset((v) => v + 200)}
                  >
                    下一页
                  </button>
                </div>
              )}
              <div className="software-cleanup-actions">
                {canChooseResiduals ? (
                  <>
                    <span>已选 {selected.size} 项</span>
                    <div className="inline-actions">
                      <button
                        className="button secondary"
                        disabled={working}
                        onClick={returnToList}
                      >
                        暂不清理
                      </button>
                      <button
                        className="button primary"
                        disabled={disabled || !residuals || !selected.size || residualLoading}
                        onClick={(e) => {
                          buttonRef.current = e.currentTarget;
                          void run(async () =>
                            setCleanPlan(await api.prepareSoftwareCleanup(task.id, [...selected])),
                          );
                        }}
                      >
                        <Trash2 size={16} aria-hidden="true" />
                        清理已选残留
                      </button>
                    </div>
                  </>
                ) : (
                  <button
                    className="button primary"
                    disabled={working || residualLoading}
                    onClick={returnToList}
                  >
                    完成
                  </button>
                )}
              </div>
            </section>
          )}
        </section>
      )}
      <section aria-label="已安装软件" hidden={showTask}>
        {task && !showTask && (
          <div className="software-last-result">
            <span>
              {task.software.name} · {uninstallTaskLabel(task)}
            </span>
            <button
              className="button secondary small"
              onClick={() => {
                setOpenedTask(task.id);
                setDismissedTask(null);
              }}
            >
              查看处理结果
            </button>
          </div>
        )}
        {!!list?.warnings.length && (
          <details className="software-disclosure">
            <summary>部分软件信息未能读取</summary>
            {list.warnings.map((w) => (
              <p key={w}>{w}</p>
            ))}
          </details>
        )}
        <div className="software-tools">
          <label className="software-search">
            <span>搜索名称或发布者</span>
            <div>
              <Search size={17} aria-hidden="true" />
              <input
                ref={searchRef}
                type="search"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
          </label>
          <label>
            <span>软件类型</span>
            <select value={source} onChange={(e) => setSource(e.target.value)}>
              <option value="all">全部软件</option>
              <option value="desktop">桌面软件</option>
              <option value="store">商店应用</option>
            </select>
          </label>
          <label>
            <span>排序</span>
            <select value={sort} onChange={(e) => setSort(e.target.value)}>
              <option value="name">名称</option>
              <option value="size">大小优先</option>
              <option value="date">最近安装</option>
            </select>
          </label>
        </div>
        {loading && <p role="status">正在读取已安装软件…</p>}
        {list && <p className="software-count">{items.length} 个软件 · 大小为软件登记的估计值</p>}
        <div className="software-list">
          {items.slice(page * 50, page * 50 + 50).map((s) => (
            <button
              className="software-row"
              key={s.id}
              onClick={(e) => {
                buttonRef.current = e.currentTarget;
                setDetail(s);
              }}
            >
              <SoftwareIcon software={s} />
              <span className="software-name">
                <strong>{s.name}</strong>
                <small>
                  {s.publisher || '发布者未知'}
                  {s.version ? ` · ${s.version}` : ''}
                  <span className="software-source-inline"> · {sourceLabel(s)}</span>
                </small>
              </span>
              <span className="software-source">{sourceLabel(s)}</span>
              <span className="software-size">
                {s.estimated_bytes === null ? '未知' : formatBytes(s.estimated_bytes)}
              </span>
              <ChevronRight size={17} aria-hidden="true" />
            </button>
          ))}
        </div>
        {!loading && list && !items.length && (
          <div className="software-empty">
            <AppWindow size={36} aria-hidden="true" />
            <h2>
              {search || source !== 'all'
                ? '没有找到匹配的软件'
                : list.warnings.length
                  ? '软件列表未能完整读取'
                  : '没有发现可展示的软件'}
            </h2>
            <p>
              {search || source !== 'all'
                ? '试试其他名称，或调整软件类型。'
                : '可刷新列表，或在 Windows 应用设置中查看。'}
            </p>
          </div>
        )}
        {items.length > 50 && (
          <div className="inline-actions">
            <button
              className="button secondary small"
              disabled={!page}
              onClick={() => setPage((v) => v - 1)}
            >
              上一页
            </button>
            <span>
              第 {page + 1} / {Math.ceil(items.length / 50)} 页
            </span>
            <button
              className="button secondary small"
              disabled={(page + 1) * 50 >= items.length}
              onClick={() => setPage((v) => v + 1)}
            >
              下一页
            </button>
          </div>
        )}
        <button
          className="button secondary software-settings"
          disabled={!desktop}
          onClick={() => void api.apps().catch((e) => setError(String(e)))}
        >
          <ExternalLink size={16} aria-hidden="true" />
          打开 Windows 应用设置
        </button>
      </section>
      <Dialog.Root
        open={!!detail}
        onOpenChange={(open) => {
          if (!open) setDetail(null);
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="overlay" />
          <Dialog.Content
            className="dialog software-detail"
            onCloseAutoFocus={(e) => {
              e.preventDefault();
              restoreFocus();
            }}
          >
            <Dialog.Title className="software-detail-title">
              {detail && <SoftwareIcon key={detail.id} software={detail} />}
              {detail?.name}
            </Dialog.Title>
            <Dialog.Description>
              {detail?.publisher.trim() || '发布者未知'}
              {detail?.version.trim() ? ` · ${detail.version}` : ''}
            </Dialog.Description>
            <Dialog.Close className="icon-button software-close" aria-label="关闭软件详情">
              <X size={20} />
            </Dialog.Close>
            {detail && (
              <>
                <p className="software-detail-impact">
                  {detail.source === 'store'
                    ? '标准卸载可能同时删除应用数据，不能保证留到残留检查时再选择。'
                    : '将打开软件自带的卸载向导。'}
                </p>
                <p>
                  {detail.residual_supported
                    ? '卸载后自动检查残留，可清理的内容由你选择。'
                    : '暂不支持该软件的残留检查，不影响正常卸载。'}
                </p>
                {detail.unavailable_reason && (
                  <p className="message info">{detail.unavailable_reason}</p>
                )}
                <button
                  className="button primary software-uninstall-action"
                  disabled={disabled || !detail.can_uninstall}
                  onClick={() =>
                    void run(async () => {
                      const next = await api.prepareSoftwareUninstall(detail.id);
                      setDetail(null);
                      setPlan(next);
                    })
                  }
                >
                  卸载此软件
                </button>
                <details className="software-disclosure software-detail-more" key={detail.id}>
                  <summary>软件信息与清理范围</summary>
                  <dl>
                    <dt>发布者</dt>
                    <dd>{detail.publisher.trim() || '未知'}</dd>
                    <dt>版本</dt>
                    <dd>{detail.version.trim() || '未知'}</dd>
                    <dt>类型与范围</dt>
                    <dd>
                      {sourceLabel(detail)} · {detail.scope === 'machine' ? '此电脑' : '当前用户'}
                    </dd>
                    <dt>安装日期</dt>
                    <dd>{detail.installed_on?.trim() || '未知'}</dd>
                    <dt>登记大小</dt>
                    <dd>
                      {detail.estimated_bytes === null
                        ? '未知'
                        : `${formatBytes(detail.estimated_bytes)}（估计）`}
                    </dd>
                    <dt>安装位置</dt>
                    <dd>{detail.install_location?.trim() || '未提供'}</dd>
                  </dl>
                  {!!detail.missing_information?.length && (
                    <p className="software-missing-information">
                      未提供：
                      {detail.missing_information
                        .map((field) => softwareMissingLabels[field])
                        .join('、')}
                      。 卸载可用性以按钮和下方说明为准。
                    </p>
                  )}
                  <h3>残留检查</h3>
                  <p>{detail.residual_note}</p>
                  {!!detail.residual_capabilities?.length && (
                    <>
                      <table className="software-capabilities" aria-label="残留分类能力">
                        <thead>
                          <tr>
                            <th scope="col">类别</th>
                            <th scope="col">检查范围</th>
                            <th scope="col">清理能力</th>
                          </tr>
                        </thead>
                        <tbody>
                          {detail.residual_capabilities.map((capability) => (
                            <tr key={capability.kind}>
                              <th scope="row">{residualKindLabels[capability.kind]}</th>
                              <td>{residualInspectionLabels[capability.inspection]}</td>
                              <td>{residualCleanupLabels[capability.cleanup]}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                      <p className="software-capability-note">
                        可检查不代表可清理，检查后才确认哪些项目可选。设置与本地资料可能包含登录状态、浏览记录、离线内容或文档备份。
                      </p>
                    </>
                  )}
                </details>
              </>
            )}
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
      <AlertDialog.Root
        open={!!plan || !!cleanPlan}
        onOpenChange={(open) => {
          if (!open) {
            setPlan(null);
            setCleanPlan(null);
          }
        }}
      >
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="overlay" />
          <AlertDialog.Content
            className="dialog unfolded-sheet"
            onCloseAutoFocus={(e) => {
              e.preventDefault();
              restoreFocus();
            }}
          >
            <AlertDialog.Title>
              {plan ? `卸载 ${plan.software.name}？` : '永久删除已选残留？'}
            </AlertDialog.Title>
            <AlertDialog.Description>
              {plan?.impact ??
                `将永久删除 ${cleanPlan?.files ?? 0} 个文件、${cleanPlan?.directories ?? 0} 个空文件夹，共 ${formatBytes(cleanPlan?.bytes ?? '0')}。不会放入回收站。`}
            </AlertDialog.Description>
            {cleanPlan?.includes_data && (
              <p className="message info">
                包含设置或本地资料，可能失去登录状态、浏览记录、离线内容或文档备份。
              </p>
            )}
            {cleanPlan && (
              <div className="software-selection-preview">
                <strong>本次已选项目</strong>
                <ul>
                  {cleanPlan.preview.map((item) => (
                    <li key={item.id}>
                      {item.path}
                      <small>
                        {item.category === 'data' ? '设置与本地资料' : '缓存与日志'}
                        {' · '}
                        {item.directory ? '仅在为空时删除' : formatBytes(item.bytes)}
                      </small>
                    </li>
                  ))}
                </ul>
                {cleanPlan.files + cleanPlan.directories > cleanPlan.preview.length && (
                  <p>
                    另有 {cleanPlan.files + cleanPlan.directories - cleanPlan.preview.length}{' '}
                    项，可返回检查完整选择。
                  </p>
                )}
              </div>
            )}
            <div className="dialog-actions">
              <AlertDialog.Cancel className="button secondary" disabled={working}>
                返回检查
              </AlertDialog.Cancel>
              <button
                className="button primary"
                disabled={disabled}
                onClick={() => {
                  const uninstall = plan;
                  const clean = cleanPlan;
                  setPlan(null);
                  setCleanPlan(null);
                  void run(async () => {
                    if (uninstall) await api.executeSoftwareUninstall(uninstall.id);
                    else if (clean) {
                      await api.executeSoftwareCleanup(clean.id);
                      setSelected(new Set());
                    }
                  });
                }}
              >
                {plan ? '确认卸载' : '确认永久删除'}
              </button>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
    </section>
  );
}

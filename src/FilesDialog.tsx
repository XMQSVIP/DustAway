import {
  candidateActions,
  ui,
  dynamic,
  formatBytes,
  formatTime,
  states,
  reasonLabels,
} from './i18n';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { useVirtualizer } from '@tanstack/react-virtual';
import { ChevronLeft, ChevronRight, X, File, LoaderCircle } from 'lucide-react';
import { api } from './api';
import { ipcError } from './coordinator';
import { useDialogFocus } from './dialogFocus';
import { candidateNextStep } from './cleanupResults';
import type {
  Candidate,
  CandidatePage,
  CandidateQuery,
  ExcludedFile,
  ExclusionDraft,
  Rule,
  Scope,
} from './types';

export interface FileViewState {
  offset: number;
  search: string;
  query: CandidateQuery;
  scrollTop: number;
}
interface Props {
  revision?: number;
  viewCache?: Map<string, FileViewState>;
  rule: Rule;
  scope: Scope;
  session: string;
  excluded: Map<string, ExcludedFile>;
  included?: Map<string, ExcludedFile>;
  ruleSelected?: boolean;
  editable: boolean;
  cleanup: boolean;
  initialView?: CandidateQuery['view'];
  initialStatus?: string | null;
  onSelect: (files: Candidate[], chosen: boolean, scope: Scope, session: string) => void;
  actionsEnabled?: boolean;
  drafts?: ExclusionDraft[];
  onDraft?: (files: Candidate[], kind: 'file' | 'parent', scope: Scope, session: string) => void;
  onReviewDrafts?: () => void;
  onClose: () => void;
}
export function FilesDialog({
  revision,
  viewCache,
  rule,
  scope,
  session,
  excluded,
  included = new Map(),
  ruleSelected = true,
  editable,
  cleanup,
  initialView,
  initialStatus = null,
  onSelect,
  onClose,
  actionsEnabled = false,
  drafts = [],
  onDraft,
  onReviewDrafts,
}: Props) {
  const dialogFocus = useDialogFocus();
  const viewKey = `${scope}:${session}:${rule.id}:${initialView ?? (cleanup ? 'cleanup' : 'scan')}:${initialStatus}`;
  const initial = useRef(viewCache?.get(viewKey)).current;
  const restoreScroll = useRef(initial?.scrollTop ?? 0);
  const [offset, setOffset] = useState(initial?.offset ?? 0);
  const [search, setSearch] = useState(initial?.search ?? '');
  const [query, setQuery] = useState<CandidateQuery>(
    initial?.query ?? {
      search: '',
      sort: 'size',
      direction: 'desc',
      status: initialStatus,
      view: initialView ?? (cleanup ? 'cleanup' : 'scan'),
    },
  );
  const [page, setPage] = useState<CandidatePage | null>(null);
  const [error, setError] = useState('');
  const [unavailable, setUnavailable] = useState(false);
  const [actionError, setActionError] = useState('');
  const [loading, setLoading] = useState(true);
  const [reload, setReload] = useState(0);
  const [locating, setLocating] = useState(false);
  const cancelled = [...excluded.values()]
    .filter((e) => e.scope === scope && e.session === session && e.rule_id === rule.id)
    .flatMap((e) => (e.file ? [e.file] : []));
  const scroll = useRef<HTMLDivElement>(null);
  const serial = useRef(0);
  useEffect(() => {
    if (search === query.search) return;
    const timer = setTimeout(() => {
      restoreScroll.current = 0;
      setOffset(0);
      setQuery((q) => ({ ...q, search }));
    }, 250);
    return () => clearTimeout(timer);
  }, [search, query.search]);
  useEffect(() => {
    const token = ++serial.current;
    setLoading(true);
    setError('');
    setUnavailable(false);
    setActionError('');
    api
      .candidates(scope, session, rule.id, offset, query)
      .then((data) => {
        if (token === serial.current) setPage(data);
      })
      .catch((e) => {
        if (token === serial.current) {
          const failure = ipcError(e);
          setUnavailable(
            ['invalid_session', 'invalid_plan'].includes(failure.code ?? '') ||
              (failure.code === 'invalid_request' && failure.message.includes('未扫描')),
          );
          setError(failure.message);
          setPage(null);
        }
      })
      .finally(() => {
        if (token === serial.current) setLoading(false);
      });
    if (scroll.current) scroll.current.scrollTop = 0;
    return () => {
      serial.current++;
    };
  }, [scope, session, rule.id, offset, query, reload, revision]);
  const virtual = useVirtualizer({
    count: loading ? 0 : (page?.items.length ?? 0),
    getScrollElement: () => scroll.current,
    estimateSize: () => 96,
    overscan: 5,
  });
  useEffect(() => {
    viewCache?.set(viewKey, { offset, search, query, scrollTop: restoreScroll.current });
  }, [viewCache, viewKey, offset, search, query]);
  useLayoutEffect(() => {
    if (!loading && page && scroll.current) scroll.current.scrollTop = restoreScroll.current;
  }, [loading, page]);
  function update(patch: Partial<CandidateQuery>) {
    restoreScroll.current = 0;
    setOffset(0);
    setQuery((q) => ({ ...q, ...patch }));
  }
  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="overlay" />
        <Dialog.Content className="dialog files-dialog side-sheet" {...dialogFocus}>
          <div className="dialog-heading">
            <div>
              <Dialog.Title>{rule.title}</Dialog.Title>
              <Dialog.Description>
                {cleanup
                  ? query.view === 'unplanned'
                    ? '扫描时发现但未纳入本次计划的文件；不计入清理未完成。'
                    : '仅展示本次计划中的文件；未选择的文件不会计入未处理。'
                  : ui.filesDescription}
              </Dialog.Description>
            </div>
            <Dialog.Close className="icon-button" aria-label={ui.closeFiles}>
              <X size={20} />
            </Dialog.Close>
          </div>
          <p className="impact-note">{rule.impact}</p>
          <div className="file-tools">
            <label>
              搜索文件或路径
              <input
                type="search"
                maxLength={256}
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="搜索整个类别，按字面匹配"
              />
            </label>
            <label>
              排序
              <select
                value={query.sort}
                onChange={(e) => update({ sort: e.target.value as CandidateQuery['sort'] })}
              >
                <option value="size">文件大小</option>
                <option value="modified">修改时间</option>
                <option value="path">路径</option>
              </select>
            </label>
            <label>
              顺序
              <select
                value={query.direction}
                onChange={(e) =>
                  update({ direction: e.target.value as CandidateQuery['direction'] })
                }
              >
                <option value="desc">降序</option>
                <option value="asc">升序</option>
              </select>
            </label>
            {cleanup && (
              <label>
                明细范围
                <select
                  value={query.view}
                  onChange={(e) =>
                    update({ view: e.target.value as CandidateQuery['view'], status: null })
                  }
                >
                  <option value="cleanup">本次清理计划</option>
                  <option value="unplanned">未纳入本次计划</option>
                </select>
              </label>
            )}
            {cleanup && query.view === 'cleanup' && (
              <label>
                处理结果
                <select
                  value={query.status ?? ''}
                  onChange={(e) => update({ status: e.target.value || null })}
                >
                  <option value="">全部</option>
                  <option value="deleted">已删除</option>
                  <option value="skipped">已跳过</option>
                  <option value="failed">失败</option>
                  <option value="candidate">未处理</option>
                  <option value="unknown">状态不确定</option>
                </select>
              </label>
            )}
          </div>
          {editable && (
            <div className="inline-actions page-selection">
              <button
                className="button secondary small"
                disabled={loading || !page?.items.length}
                onClick={() => onSelect(page!.items, false, scope, session)}
              >
                取消本页选择
              </button>
              <button
                className="button secondary small"
                disabled={loading || !page?.items.length}
                onClick={() => onSelect(page!.items, true, scope, session)}
              >
                选择本页
              </button>
              <small>只影响当前页，翻页与搜索保留选择</small>
            </div>
          )}
          {actionsEnabled && (
            <details className="retention-settings">
              <summary>长期保留设置</summary>
              <div className="inline-actions page-selection">
                <button
                  className="button secondary small"
                  disabled={!cancelled.length}
                  onClick={() => onDraft?.(cancelled, 'file', scope, session)}
                >
                  将本类别已取消的 {cancelled.length} 项加入长期排除
                </button>
              </div>
            </details>
          )}
          {drafts.length > 0 && (
            <div className="inline-actions exclusion-draft-notice" role="status">
              <span>{drafts.length} 项长期保留草稿尚未保存</span>
              <button className="result-link" disabled={!actionsEnabled} onClick={onReviewDrafts}>
                核对并保存草稿
              </button>
            </div>
          )}
          {error && (
            <p role="alert" className="error-text">
              {error}{' '}
              {!unavailable && (
                <button className="button secondary small" onClick={() => setReload((v) => v + 1)}>
                  重试读取明细
                </button>
              )}
            </p>
          )}
          {actionError && (
            <p role="alert" className="error-text">
              {actionError}。文件明细与选择已保留，可再次定位或核对完整路径。
            </p>
          )}
          <div
            className="file-list"
            ref={scroll}
            onScroll={(event) => {
              if (loading) return;
              restoreScroll.current = event.currentTarget.scrollTop;
              viewCache?.set(viewKey, { offset, search, query, scrollTop: restoreScroll.current });
            }}
            aria-busy={loading}
            tabIndex={0}
            aria-label="文件列表，可使用方向键滚动"
          >
            {loading ? (
              <div className="empty compact">
                <LoaderCircle className="spin" />
                {ui.readingFiles}
              </div>
            ) : error ? (
              <div className="empty compact">
                {unavailable
                  ? '当前会话明细已不可用。关闭明细后可核对报告中保留的统计；不会读取其他会话。'
                  : '暂时无法读取文件明细，请重试。'}
              </div>
            ) : !page?.items.length ? (
              <div className="empty compact">
                {cleanup
                  ? '当前范围和筛选条件下没有文件记录。'
                  : query.search || query.status
                    ? '没有符合筛选条件的文件'
                    : ui.noFiles}
              </div>
            ) : (
              <div style={{ height: virtual.getTotalSize(), position: 'relative' }}>
                {virtual.getVirtualItems().map((item) => {
                  const file = page.items[item.index];
                  return (
                    <div
                      className="file-row"
                      key={file.id}
                      data-index={item.index}
                      ref={virtual.measureElement}
                      style={{
                        position: 'absolute',
                        top: 0,
                        left: 0,
                        width: '100%',
                        transform: `translateY(${item.start}px)`,
                      }}
                    >
                      {editable ? (
                        <input
                          type="checkbox"
                          aria-label={dynamic.selectFile(file.path)}
                          checked={ruleSelected ? !excluded.has(file.id) : included.has(file.id)}
                          onChange={(event) =>
                            onSelect([file], event.target.checked, scope, session)
                          }
                        />
                      ) : (
                        <File size={18} aria-hidden="true" />
                      )}
                      <div className="file-name">
                        <span>{file.path.split(/[\\/]/).pop()}</span>
                        {editable && (
                          <small className="file-selection-state">
                            {(ruleSelected ? !excluded.has(file.id) : included.has(file.id))
                              ? '已选，将参与本次清理'
                              : '未选择，本次不清理'}
                          </small>
                        )}
                        <small>
                          {formatTime(file.modified_unix)} ·{' '}
                          {file.reason_code
                            ? reasonLabels[file.reason_code]
                            : (file.reason ??
                              (cleanup && file.status === 'candidate'
                                ? '未处理'
                                : (states[file.status] ?? file.status)))}
                        </small>
                        {cleanup && query.view === 'cleanup' && candidateNextStep(file) && (
                          <p className="file-next-step">{candidateNextStep(file)}</p>
                        )}
                        {cleanup && file.reason && (
                          <details onToggle={() => virtual.measure()}>
                            <summary>原始原因</summary>
                            <p className="full-path">{file.reason}</p>
                          </details>
                        )}
                        <details onToggle={() => virtual.measure()}>
                          <summary>完整路径</summary>
                          <div className="full-path">{file.path}</div>
                        </details>
                        {actionsEnabled && file.status !== 'deleted' && (
                          <details className="file-actions" onToggle={() => virtual.measure()}>
                            <summary>{candidateActions.menu}</summary>
                            <div className="inline-actions">
                              <button
                                className="button secondary small"
                                disabled={locating}
                                onClick={async () => {
                                  const token = serial.current;
                                  setLocating(true);
                                  setActionError('');
                                  try {
                                    await api.reveal(scope, session, file.id);
                                  } catch (e) {
                                    if (token === serial.current)
                                      setActionError(`无法定位：${ipcError(e).message}`);
                                  } finally {
                                    setLocating(false);
                                  }
                                }}
                              >
                                {candidateActions.reveal}
                              </button>
                              <button
                                className="button secondary small"
                                onClick={() => onDraft?.([file], 'file', scope, session)}
                              >
                                {candidateActions.file}
                              </button>
                              <button
                                className="button secondary small"
                                onClick={() => onDraft?.([file], 'parent', scope, session)}
                              >
                                {candidateActions.parent}
                              </button>
                            </div>
                          </details>
                        )}
                      </div>
                      <strong>{formatBytes(file.bytes)}</strong>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
          <div className="pagination">
            <span aria-live="polite">
              共 {page?.total.toLocaleString() ?? '—'} 项 · 第 {Math.floor(offset / 200) + 1} 页
            </span>
            <div>
              <button
                className="button secondary small"
                disabled={loading || offset === 0}
                onClick={() => {
                  restoreScroll.current = 0;
                  setOffset((v) => Math.max(0, v - 200));
                }}
              >
                <ChevronLeft size={16} />
                {ui.previousPage}
              </button>
              <button
                className="button secondary small"
                disabled={loading || !page || offset + 200 >= page.total}
                onClick={() => {
                  restoreScroll.current = 0;
                  setOffset((v) => v + 200);
                }}
              >
                {ui.nextPage}
                <ChevronRight size={16} />
              </button>
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

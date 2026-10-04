import { useEffect, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { ChevronRight, FolderOpen, File, Search } from 'lucide-react';
import { api, desktop } from './api';
import { ipcError } from './coordinator';
import { formatBytes, formatTime, reasonLabels } from './i18n';
import type {
  AnalysisPage,
  AnalysisFilesPage,
  AnalysisSnapshot,
  AnalysisEntry,
  ReasonCode,
  AnalysisDiscoveryQuery,
} from './types';
import { AnalysisDiscoveryFilters, defaultDiscovery } from './AnalysisDiscoveryFilters';
import { EntryAdvice } from './AnalysisAdvice';
import { relativeSize } from './rankingSize';
import { useSpaceJourney, type SpaceJourney } from './useSpaceJourney';
import { SpaceStatus } from './SpaceStatus';
import { AnalysisReviewDialog, type AnalysisReviewTarget } from './AnalysisReviewDialog';
import { manualChecked, manualMixed, type ManualChoices } from './manualSelection';

interface ReviewOrigin {
  session: string;
  view: 'directories' | 'files' | 'discovery';
  discovery: AnalysisDiscoveryQuery;
  discoverySearch: string;
  trigger: 'review' | 'entry';
  node: string;
  search: string;
  query: string;
  offset: number;
  descending: boolean;
  fileSearch: string;
  fileQuery: string;
  fileOffset: number;
  readKey: string;
  scrollTop: number;
  pageScroll: number;
  index: number;
  entryId: string;
}

export const analysisStates = {
  scanning: '正在分析',
  completed: '分析完成',
  partial: '统计不完整',
  cancelled: '已停止 · 统计不完整',
  failed: '读取失败 · 统计不完整',
};
const issueLabel = (code: string) =>
  ({
    resource_limit: '已达到本次分析的资源限额，保留部分结果；请选择更小的目录',
    entry_limit: '达到本次分析的条目上限，请选择更小的目录',
    depth_limit: '达到 128 层目录深度上限',
    directory_inaccessible: '部分目录或文件无法访问',
  })[code] ??
  reasonLabels[code as ReasonCode] ??
  '无法完成统计';

export function AnalysisPanel({
  active,
  snapshot,
  excludedRules = [],
  busy,
  stopping,
  onStart,
  onStop,
  onCleanup,
  journey,
  taskNotice,
  onCheckTask,
  manualChoices = new Map(),
  onSelectManual,
  onSelectManualBatch,
  selectionTarget,
  onExcludeManual,
}: {
  journey?: SpaceJourney;
  active: boolean;
  snapshot: AnalysisSnapshot | null;
  excludedRules?: string[];
  busy: boolean;
  stopping: boolean;
  onStart: (token: string) => Promise<void>;
  onStop: () => void;
  onCleanup: (rules: string[]) => void;
  taskNotice?: string;
  onCheckTask?: () => void;
  manualChoices?: ManualChoices;
  onSelectManual?: (entry: AnalysisEntry, selected: boolean) => void;
  onSelectManualBatch?: (entries: AnalysisEntry[], selected: boolean) => void;
  selectionTarget?: { session: string; entry: AnalysisEntry; key: number } | null;
  onExcludeManual?: (entry: AnalysisEntry) => void;
}) {
  const localJourney = useSpaceJourney(active && !journey, snapshot);
  const space = journey ?? localJourney;
  const {
    locations,
    setLocations,
    selected,
    setSelected,
    locationError,
    setLocationError,
    reloadLocations,
  } = space;
  const differentLocation = !!snapshot && !!selected && selected !== snapshot.location.token;
  const selectedLocation = locations.find((l) => l.token === selected);
  const [picking, setPicking] = useState(false);
  const [node, setNode] = useState('0');
  const [search, setSearch] = useState('');
  const [query, setQuery] = useState('');
  const [offset, setOffset] = useState(0);
  const [descending, setDescending] = useState(true);
  const [page, setPage] = useState<AnalysisPage | null>(null);
  const [view, setView] = useState<'directories' | 'files' | 'discovery'>(
    snapshot?.state === 'scanning' ? 'directories' : 'discovery',
  );
  const [discovery, setDiscovery] = useState<AnalysisDiscoveryQuery>({ ...defaultDiscovery });
  const [discoverySearch, setDiscoverySearch] = useState('');
  const [discoveryPage, setDiscoveryPage] = useState<AnalysisFilesPage | null>(null);
  const discoveryKey = JSON.stringify(discovery);
  const [discoveryOrigin, setDiscoveryOrigin] = useState<ReviewOrigin | null>(null);
  const [fileSearch, setFileSearch] = useState('');
  const [fileQuery, setFileQuery] = useState('');
  const [fileOffset, setFileOffset] = useState(0);
  const [sizeMetric, setSizeMetric] = useState<'allocated' | 'logical'>('allocated');
  const cursors = useRef(new Map<string, Map<number, string | null>>());
  const useAllocated = snapshot?.query_api_version === 2 && sizeMetric === 'allocated';
  const entryBytes = (entry: AnalysisEntry) =>
    useAllocated ? (entry.allocated_bytes ?? null) : entry.bytes;
  const [filePage, setFilePage] = useState<AnalysisFilesPage | null>(null);
  const [readError, setReadError] = useState('');
  const [actionError, setActionError] = useState('');
  const [loading, setLoading] = useState(false);
  const [epoch, setEpoch] = useState(0);
  const scroll = useRef<HTMLDivElement>(null);
  const serial = useRef(0);
  const lastReadKey = useRef('');
  const completedRead = useRef('');
  const [successfulKey, setSuccessfulKey] = useState('');
  const currentReadKey = JSON.stringify([
    snapshot?.id,
    node,
    query,
    offset,
    descending,
    view,
    fileQuery,
    fileOffset,
    discoveryKey,
    sizeMetric,
  ]);
  const hasEntries = !!snapshot?.entries;
  const displayed =
    successfulKey === currentReadKey
      ? view === 'discovery'
        ? discoveryPage
        : view === 'files'
          ? filePage
          : page
      : null;
  const largestDisplayed =
    displayed?.items.reduce(
      (max, item) => (BigInt(entryBytes(item) ?? 0) > max ? BigInt(entryBytes(item) ?? 0) : max),
      0n,
    ) ?? 0n;
  const visibleOffset =
    view === 'discovery' ? discovery.offset : view === 'files' ? fileOffset : offset;
  const visibleQuery =
    view === 'discovery' ? discovery.search : view === 'files' ? fileQuery : query;
  const changeDiscoveryOffset = (change: (value: number) => number) =>
    setDiscovery((d) => ({ ...d, offset: change(d.offset) }));
  const changeOffset =
    view === 'discovery' ? changeDiscoveryOffset : view === 'files' ? setFileOffset : setOffset;
  const [review, setReview] = useState<(AnalysisReviewTarget & { origin: ReviewOrigin }) | null>(
    null,
  );
  const [reviewOpen, setReviewOpen] = useState(false);
  const [reviewNotice, setReviewNotice] = useState('');
  const reviewSerial = useRef(0);
  const latestReview = useRef(review);
  latestReview.current = review;
  const startAnalysisButton = useRef<HTMLButtonElement>(null);
  const locationPickerButton = useRef<HTMLButtonElement>(null);
  const restorePending = useRef<ReviewOrigin | null>(null);
  const [focusEpoch, setFocusEpoch] = useState(0);
  useEffect(() => {
    setReview(null);
    setReviewOpen(false);
    setReviewNotice('');
    restorePending.current = null;
    setNode('0');
    setOffset(0);
    setSearch('');
    setQuery('');
    setPage(null);
    setDiscovery({ ...defaultDiscovery });
    setDiscoverySearch('');
    setDiscoveryPage(null);
    setDiscoveryOrigin(null);
    setView(snapshot?.state === 'scanning' ? 'directories' : 'discovery');
    setFileOffset(0);
    setFileSearch('');
    setFileQuery('');
    setFilePage(null);
    setActionError('');
  }, [snapshot?.id]);
  const previousAnalysisState = useRef(snapshot?.state);
  useEffect(() => {
    if (previousAnalysisState.current === 'scanning' && snapshot && snapshot.state !== 'scanning') {
      setView('discovery');
      setDiscovery({ ...defaultDiscovery });
      setDiscoverySearch('');
    }
    previousAnalysisState.current = snapshot?.state;
  }, [snapshot?.state]);
  useEffect(() => {
    if (search === query) return;
    const timer = setTimeout(() => {
      setQuery(search);
      setOffset(0);
    }, 250);
    return () => clearTimeout(timer);
  }, [search]);
  useEffect(() => {
    if (fileSearch === fileQuery) return;
    const timer = setTimeout(() => {
      setFileQuery(fileSearch);
      setFileOffset(0);
    }, 250);
    return () => clearTimeout(timer);
  }, [fileSearch]);
  useEffect(() => {
    if (discoverySearch === discovery.search) return;
    const timer = setTimeout(
      () => setDiscovery((d) => ({ ...d, search: discoverySearch, offset: 0 })),
      250,
    );
    return () => clearTimeout(timer);
  }, [discoverySearch, discovery.search]);
  useEffect(() => {
    if (
      !active ||
      !snapshot ||
      !hasEntries ||
      differentLocation ||
      (view === 'discovery' && snapshot.state === 'scanning')
    )
      return;
    const readKey = currentReadKey;
    const resultKey = `${readKey}:${snapshot.state}:${epoch}`;
    // Returning from cleanup must preserve the exact rows, scroll and trigger element.
    if (completedRead.current === resultKey) {
      setLoading(false);
      return;
    }
    const version = ++serial.current;
    setLoading(true);
    setReadError('');
    const requestId = crypto.randomUUID();
    let countTimer: ReturnType<typeof setTimeout> | undefined;
    const baseKey = JSON.stringify([
      snapshot.id,
      snapshot.state,
      view,
      node,
      query,
      fileQuery,
      descending,
      sizeMetric,
      { ...discovery, offset: 0 },
    ]);
    if (!cursors.current.has(baseKey)) {
      if (cursors.current.size >= 16) cursors.current.delete(cursors.current.keys().next().value!);
      cursors.current.set(baseKey, new Map([[0, null]]));
    }
    const readV2 = snapshot.query_api_version === 2;
    const request = readV2
      ? api
          .analysisRead({
            request_id: requestId,
            session: snapshot.id,
            view,
            node,
            search: view === 'files' ? fileQuery : query,
            metric: sizeMetric,
            descending: view === 'directories' ? descending : true,
            discovery: view === 'discovery' ? discovery : null,
            cursor: cursors.current.get(baseKey)?.get(visibleOffset) ?? null,
            offset: visibleOffset,
          })
          .then((data) => {
            if (version === serial.current && data.next_cursor)
              cursors.current.get(baseKey)?.set(visibleOffset + 200, data.next_cursor);
            return {
              ...data,
              total: data.total ?? visibleOffset + data.items.length + (data.next_cursor ? 1 : 0),
              count_pending: data.total === null,
            } as AnalysisPage;
          })
      : view === 'discovery'
        ? api.analysisDiscovery(snapshot.id, discovery)
        : view === 'files'
          ? api.analysisFiles(snapshot.id, fileQuery, fileOffset)
          : api.analysisChildren(snapshot.id, node, query, offset, descending);
    request
      .then((data) => {
        if (version === serial.current) {
          completedRead.current = resultKey;
          setSuccessfulKey(readKey);
          if (view === 'discovery') setDiscoveryPage(data);
          else if (view === 'files') setFilePage(data);
          else setPage(data as AnalysisPage);
          if (readV2 && data.count_pending) {
            const failCount = (message: string) => {
              const update = <T extends AnalysisFilesPage>(page: T | null): T | null =>
                page ? { ...page, count_pending: false, count_failed: true } : page;
              if (view === 'discovery') setDiscoveryPage(update);
              else if (view === 'files') setFilePage(update);
              else setPage(update);
              setReadError(message);
            };
            const pollCount = async () => {
              try {
                const count = await api.analysisCount(requestId);
                if (version !== serial.current) return;
                if (count.total !== null) {
                  const update = <T extends AnalysisFilesPage>(page: T | null): T | null =>
                    page ? { ...page, total: count.total!, count_pending: false } : page;
                  if (view === 'discovery') setDiscoveryPage(update);
                  else if (view === 'files') setFilePage(update);
                  else setPage(update);
                } else if (!count.finished) countTimer = setTimeout(pollCount, 200);
                else failCount(count.error ?? '总数未完成，可刷新重试。');
              } catch {
                if (version === serial.current) failCount('总数未完成，可刷新重试。');
              }
            };
            countTimer = setTimeout(pollCount, 200);
          }
        }
      })
      .catch((e) => {
        if (version === serial.current) {
          completedRead.current = '';
          if (ipcError(e).code === 'stale_cursor') {
            changeOffset(() => 0);
            cursors.current.delete(baseKey);
          }
          if (ipcError(e).code === 'invalid_session') {
            setReview(null);
            setReviewOpen(false);
            setDiscoveryOrigin(null);
            setReviewNotice('分析会话已失效，请重新分析。');
          }
          setReadError(`暂时无法读取目录，请重试。${ipcError(e).message}`);
        }
      })
      .finally(() => {
        if (version === serial.current) setLoading(false);
      });
    if (scroll.current && readKey !== lastReadKey.current) scroll.current.scrollTop = 0;
    lastReadKey.current = readKey;
    return () => {
      serial.current++;
      if (countTimer) clearTimeout(countTimer);
      if (readV2) void api.cancelAnalysisQuery(requestId).catch(() => {});
    };
  }, [
    active,
    snapshot?.id,
    snapshot?.state,
    hasEntries,
    node,
    query,
    offset,
    descending,
    epoch,
    differentLocation,
    view,
    fileQuery,
    fileOffset,
    discoveryKey,
    sizeMetric,
  ]);
  const virtual = useVirtualizer({
    count: displayed?.items.length ?? 0,
    getScrollElement: () => scroll.current,
    estimateSize: () => 104,
    overscan: 4,
  });
  function captureOrigin(
    entry: AnalysisEntry,
    index: number,
    trigger: ReviewOrigin['trigger'] = 'review',
  ): ReviewOrigin {
    return {
      session: snapshot!.id,
      view,
      node,
      search: query,
      query,
      offset,
      descending,
      fileSearch: fileQuery,
      fileQuery,
      fileOffset,
      discovery: { ...discovery },
      discoverySearch: discovery.search,
      trigger,
      readKey: currentReadKey,
      scrollTop: scroll.current?.scrollTop ?? 0,
      pageScroll: document.getElementById('main')?.scrollTop ?? 0,
      index,
      entryId: entry.id,
    };
  }
  function changeDiscovery(changes: Partial<AnalysisDiscoveryQuery>) {
    if (changes.search !== undefined) setDiscoverySearch(changes.search);
    setDiscovery((d) => ({ ...d, ...changes, offset: 0 }));
  }
  function showIncomplete() {
    setDiscovery({
      ...defaultDiscovery,
      kind: 'directory',
      category: 'all',
      include_nested: true,
      incomplete_only: true,
    });
    setDiscoverySearch('');
    setView('discovery');
    setDiscoveryOrigin(null);
  }
  function openReview(entry: AnalysisEntry, index: number) {
    if (!snapshot) return;
    restorePending.current = null;
    setReviewNotice('');
    setReview({
      key: ++reviewSerial.current,
      session: snapshot.id,
      entry,
      location: snapshot.location,
      analyzedAt: snapshot.started_at,
      origin: captureOrigin(entry, index),
    });
    setReviewOpen(true);
  }
  function restoreOrigin(origin: ReviewOrigin) {
    if (origin.session !== snapshot?.id) {
      setDiscoveryOrigin(null);
      return;
    }
    restorePending.current = origin;
    if (snapshot) setSelected(snapshot.location.token);
    setView(origin.view);
    setNode(origin.node);
    setSearch(origin.search);
    setQuery(origin.query);
    setOffset(origin.offset);
    setDescending(origin.descending);
    setFileSearch(origin.fileSearch);
    setFileQuery(origin.fileQuery);
    setFileOffset(origin.fileOffset);
    setDiscovery({ ...origin.discovery });
    setDiscoverySearch(origin.discoverySearch);
    setFocusEpoch((value) => value + 1);
  }
  function closeReview() {
    if (!review || review.session !== snapshot?.id) return;
    const origin = review.origin;
    restoreOrigin(origin);
    setReviewOpen(false);
    setReview(null);
  }
  useEffect(() => {
    const origin = restorePending.current;
    if (!active) return;
    if (!origin || reviewOpen) return;
    if (origin.session !== snapshot?.id) {
      restorePending.current = null;
      return;
    }
    if (currentReadKey !== origin.readKey || !displayed) return;
    let cancelled = false;
    const frame = requestAnimationFrame(() => {
      if (scroll.current) scroll.current.scrollTop = origin.scrollTop;
      const main = document.getElementById('main');
      if (main) main.scrollTop = origin.pageScroll;
      const findTrigger = () =>
        Array.from(
          scroll.current?.querySelectorAll<HTMLButtonElement>(
            origin.trigger === 'entry' ? '[data-analysis-entry]' : '[data-analysis-review]',
          ) ?? [],
        ).find(
          (button) =>
            (origin.trigger === 'entry'
              ? button.dataset.analysisEntry
              : button.dataset.analysisReview) === origin.entryId,
        );
      if (!findTrigger()) virtual.scrollToIndex(origin.index, { align: 'start' });
      requestAnimationFrame(() => {
        if (cancelled || restorePending.current !== origin) return;
        const trigger = findTrigger();
        (trigger ?? scroll.current)?.focus({ preventScroll: true });
        if (trigger) trigger.scrollIntoView({ block: 'nearest' });
        restorePending.current = null;
      });
    });
    return () => {
      cancelled = true;
      cancelAnimationFrame(frame);
    };
  }, [focusEpoch, active, reviewOpen, currentReadKey, successfulKey, snapshot?.id, displayed]);
  const handledTarget = useRef<number | null>(null);
  useEffect(() => {
    if (
      !active ||
      !selectionTarget ||
      selectionTarget.session !== snapshot?.id ||
      handledTarget.current === selectionTarget.key
    )
      return;
    handledTarget.current = selectionTarget.key;
    setReview(null);
    setReviewOpen(false);
    restorePending.current = null;
    const entry = selectionTarget.entry;
    setView('directories');
    setNode(entry.directory ? entry.id : (entry.parent ?? '0'));
    setOffset(0);
    setSearch('');
    setQuery('');
    setActionError('');
  }, [active, selectionTarget, snapshot?.id]);
  function navigate(id: string, entry?: AnalysisEntry, index?: number) {
    if (view === 'discovery' && entry)
      setDiscoveryOrigin(captureOrigin(entry, index ?? 0, 'entry'));
    setView('directories');
    setNode(id);
    setOffset(0);
    setSearch('');
    setQuery('');
    setActionError('');
  }
  async function pick() {
    setPicking(true);
    setLocationError('');
    try {
      const location = await api.pickAnalysis();
      if (location) {
        setLocations((list) =>
          list.some((l) => l.token === location.token) ? list : [...list, location],
        );
        setSelected(location.token);
      }
    } catch (e) {
      setLocationError(
        `无法选择此目录：${ipcError(e).message}。可重新选择可访问的本地固定 NTFS 目录。`,
      );
    } finally {
      setPicking(false);
    }
  }
  return (
    <section className="analysis-panel" aria-label="空间分析">
      <header className="page-heading">
        <div>
          <h1>空间分析</h1>
          <p>找到大占用，勾选不再需要的内容后清理。</p>
        </div>
      </header>
      {reviewNotice && (
        <p role="status" className="message info">
          {reviewNotice}
        </p>
      )}
      <details
        className="analysis-location-fold"
        open={!snapshot || differentLocation || snapshot.state === 'scanning'}
      >
        <summary>
          位置与重新分析 · {selectedLocation?.title ?? snapshot?.location.title ?? '选择位置'}
        </summary>
        <div className="analysis-location surface">
          <label>
            准备分析的位置
            <select
              value={selected}
              disabled={busy || picking}
              onChange={(e) => {
                setSelected(e.target.value);
                setLocationError('');
              }}
            >
              {!locations.length && (
                <option value="">{desktop ? '正在读取常用位置…' : '桌面应用中选择目录'}</option>
              )}
              {locations.map((l) => (
                <option key={l.token} value={l.token}>
                  {l.kind === 'volume' ? l.title : `${l.title} · ${l.path}`}
                  {l.supported === false ? '（不可用）' : ''}
                </option>
              ))}
              {snapshot && !locations.some((l) => l.token === snapshot.location.token) && (
                <option value={snapshot.location.token}>
                  {snapshot.location.title} · {snapshot.location.path}
                </option>
              )}
            </select>
          </label>
          <div className="inline-actions">
            <button
              className="button secondary"
              disabled={busy || picking || !desktop}
              onClick={pick}
              ref={locationPickerButton}
            >
              <FolderOpen size={17} />
              选择目录
            </button>
            {snapshot?.state === 'scanning' ? (
              <button className="button secondary" disabled={stopping} onClick={onStop}>
                {stopping ? '停止中…' : '停止分析'}
              </button>
            ) : (
              <button
                ref={startAnalysisButton}
                className="button primary"
                disabled={
                  busy || picking || !selected || selectedLocation?.supported === false || !desktop
                }
                onClick={() => void onStart(selected)}
              >
                <Search size={17} />
                {snapshot && !differentLocation ? '重新分析' : '开始只读分析'}
              </button>
            )}
          </div>
          {selectedLocation?.unavailable_reason && (
            <p role="status">{selectedLocation.unavailable_reason}</p>
          )}
          {locationError && (
            <p role="alert">
              {locationError}{' '}
              <button className="result-link" onClick={reloadLocations}>
                重新读取位置
              </button>
            </p>
          )}
        </div>
      </details>
      {(!snapshot || differentLocation) && <SpaceStatus journey={space} compact />}

      {!snapshot && (
        <div className="analysis-intro empty-folio">
          <div className="empty-pages" aria-hidden="true">
            <i />
            <i />
            <i />
          </div>
          <h2>空间大头不一定是垃圾</h2>
          <p>可展开目录查找具体文件；可删除的普通文件由你勾选，受保护的内容会保留。</p>
        </div>
      )}
      {snapshot && differentLocation && (
        <section className="previous-analysis surface" aria-label="上次分析">
          <span>
            上次分析：{snapshot.location.path} · {formatTime(snapshot.started_at)}
          </span>
          <button className="result-link" onClick={() => setSelected(snapshot.location.token)}>
            查看上次结果
          </button>
        </section>
      )}
      {snapshot && (
        <div hidden={differentLocation} className="analysis-results">
          <p className="analysis-source" title={snapshot.location.path}>
            本次结果来源：{snapshot.location.path} · {formatTime(snapshot.started_at)}
          </p>
          <section className="analysis-summary" aria-label="分析摘要">
            <div>
              <small>{useAllocated ? '已统计占用空间（去重）' : '已统计文件大小（去重）'}</small>
              <strong>
                {formatBytes(
                  useAllocated ? (snapshot.allocated_bytes ?? '0') : snapshot.unique_bytes,
                )}
              </strong>
              {useAllocated && !!snapshot.allocation_unknown && (
                <small>另有 {snapshot.allocation_unknown.toLocaleString()} 个文件占用未知</small>
              )}
              <span>
                {snapshot.files.toLocaleString()} 个文件条目 · {analysisStates[snapshot.state]}
              </span>
            </div>
            <div>
              <span>
                已检查 {snapshot.checked.toLocaleString()} 项 · 已用{' '}
                {Math.floor(snapshot.elapsed_ms / 1000)} 秒
              </span>

              {snapshot.state === 'scanning' && (
                <small className="analysis-path">
                  {snapshot.phase === 'indexing'
                    ? '正在建立排行索引'
                    : snapshot.phase === 'enumerating'
                      ? '正在列出根层条目'
                      : '正在读取目录'}
                  {snapshot.current_directory ? `：${snapshot.current_directory}` : ''}
                </small>
              )}
            </div>
            <SpaceStatus journey={space} compact />
          </section>
          <details className="analysis-size-help">
            <summary>大小与统计范围说明</summary>
            <p className="full-path">{snapshot.location.path}</p>
            <p className="muted small-text">
              仅分析所选位置可读取的文件信息，不删除或打开正文；清理排除项仍参与统计。
              文件大小是逻辑长度；占用空间按文件分配空间统计，各目录内去重。跨目录硬链接可能共享数据，兄弟目录不能直接相加。未知大小和未读取区域不计入已知占用；文件系统元数据、快照等也不在其中。这些数值不是承诺可释放的空间。
              分析路径和元数据会临时存到本机，退出后清除；异常退出的残留下次启动清理。程序自己的临时索引目录不参与统计。
            </p>
          </details>
          {snapshot.stale && (
            <p role="status" className="message info">
              这是清理前的统计，请重新分析查看当前占用。
            </p>
          )}
          {snapshot.error && <p role="alert">{snapshot.error}</p>}
          {snapshot.issues.length > 0 && (
            <details className="reason-list">
              <summary>
                未完整统计的原因 · {snapshot.issues.reduce((n, i) => n + i.count, 0)} 项
              </summary>
              <ul>
                {snapshot.issues.map((i) => (
                  <li key={i.code}>
                    {issueLabel(i.code)}
                    <span>{i.count} 项</span>
                  </li>
                ))}
              </ul>
            </details>
          )}
          {hasEntries && (
            <>
              {discoveryOrigin && view !== 'discovery' && (
                <button
                  className="button secondary"
                  onClick={() => {
                    restoreOrigin(discoveryOrigin);
                    setDiscoveryOrigin(null);
                  }}
                >
                  返回空间排行
                </button>
              )}
              <div className="analysis-toolbar">
                {snapshot.query_api_version === 2 && (
                  <label>
                    统计口径{' '}
                    <select
                      aria-label="统计口径"
                      value={sizeMetric}
                      onChange={(e) => {
                        setSizeMetric(e.target.value as 'allocated' | 'logical');
                        setOffset(0);
                        setFileOffset(0);
                        setDiscovery((d) => ({ ...d, offset: 0 }));
                      }}
                    >
                      <option value="allocated">占用空间</option>
                      <option value="logical">文件大小</option>
                    </select>
                  </label>
                )}
                <div className="analysis-view-switch" role="group" aria-label="分析视图">
                  {(['file', 'directory'] as const).map((kind) => (
                    <button
                      key={kind}
                      className="button secondary"
                      aria-pressed={view === 'discovery' && discovery.kind === kind}
                      disabled={snapshot.state === 'scanning'}
                      onClick={() => {
                        setView('discovery');
                        changeDiscovery({ kind, incomplete_only: false });
                        setDiscoveryOrigin(null);
                      }}
                    >
                      {kind === 'file' ? '大文件' : '大目录'}
                    </button>
                  ))}
                  <button
                    className="result-link"
                    aria-pressed={view === 'directories'}
                    onClick={() => setView('directories')}
                  >
                    目录浏览
                  </button>
                </div>
                {view !== 'discovery' && (
                  <div className="file-tools">
                    <label>
                      {view === 'files' ? '搜索本次范围的文件名' : '搜索当前目录名称'}
                      <input
                        type="search"
                        maxLength={256}
                        placeholder={view === 'files' ? '搜索所有已读取文件' : '仅搜索当前层级'}
                        value={view === 'files' ? fileSearch : search}
                        onChange={(e) =>
                          (view === 'files' ? setFileSearch : setSearch)(e.target.value)
                        }
                      />
                    </label>
                    {view === 'directories' && (
                      <label>
                        {useAllocated ? '占用空间排序' : '文件大小排序'}
                        <select
                          value={descending ? 'desc' : 'asc'}
                          onChange={(e) => {
                            setDescending(e.target.value === 'desc');
                            setOffset(0);
                          }}
                        >
                          <option value="desc">从大到小</option>
                          <option value="asc">从小到大</option>
                        </select>
                      </label>
                    )}
                    <button className="button secondary" onClick={() => setEpoch((n) => n + 1)}>
                      重新读取当前结果
                    </button>
                  </div>
                )}
              </div>
              {snapshot.state === 'scanning' && (
                <p className="muted small-text">
                  结束或停止后可查看全范围排行，扫描中可浏览已检查的目录。
                </p>
              )}
              {view === 'discovery' && (
                <>
                  <AnalysisDiscoveryFilters
                    query={discovery}
                    search={discoverySearch}
                    onSearch={setDiscoverySearch}
                    onChange={changeDiscovery}
                  />
                </>
              )}
              {snapshot.state !== 'completed' && snapshot.state !== 'scanning' && (
                <button className="result-link" onClick={showIncomplete}>
                  查看待补充目录
                </button>
              )}
              <p className="analysis-coverage" role="status">
                <span>读取当前结果不会重新扫描磁盘。 </span>
                {snapshot.state === 'completed'
                  ? '本次范围排行'
                  : '已读取范围内排行 · 未读取位置不在其中'}
                {view === 'directories' && page?.coverage && successfulKey === currentReadKey && (
                  <span>
                    {' '}
                    · 本层已发现目录：完成 {page.coverage.completed}，未完成{' '}
                    {page.coverage.incomplete}，待统计 {page.coverage.pending}
                  </span>
                )}
                {displayed?.generated_at && (
                  <span> · {formatTime(displayed.generated_at)} 更新</span>
                )}
                {loading && displayed && <span> · 正在刷新，保留上次列表</span>}
                {view === 'discovery' && (
                  <button className="result-link" onClick={() => setEpoch((n) => n + 1)}>
                    重新读取当前结果
                  </button>
                )}
              </p>
              {view === 'directories' && (
                <nav className="analysis-breadcrumbs" aria-label="分析目录层级">
                  {(successfulKey === currentReadKey ? (page?.breadcrumbs ?? []) : []).map(
                    (b, i) => (
                      <span key={b.id}>
                        {i > 0 && <ChevronRight size={14} />}
                        <button
                          className="result-link"
                          aria-current={b.id === node ? 'location' : undefined}
                          onClick={() => navigate(b.id)}
                        >
                          {b.name || '所选目录'}
                        </button>
                      </span>
                    ),
                  )}
                </nav>
              )}
              {readError && (
                <p role="alert" className="message error">
                  {displayed ? `本次更新失败，保留上次已读列表。${readError}` : readError}
                  <button className="button secondary small" onClick={() => setEpoch((n) => n + 1)}>
                    重试读取目录
                  </button>
                </p>
              )}
              {actionError && <p role="alert">{actionError}</p>}
              {!!displayed?.items.length && (
                <p className="ranking-scale">
                  按已读取大小排序 · 条带长度相对本页最大项；目录包含关系与共享文件的大小不能相加。
                </p>
              )}
              <div
                ref={scroll}
                className="analysis-list"
                tabIndex={0}
                aria-label={
                  view === 'discovery'
                    ? '本次范围空间排行'
                    : view === 'files'
                      ? '本次范围最大文件排行'
                      : '目录与文件排行'
                }
                aria-busy={loading}
              >
                {loading && !displayed ? (
                  <p className="empty compact">正在读取目录…</p>
                ) : readError && !displayed ? (
                  <p className="empty compact">读取失败，尚不能判断目录内容。</p>
                ) : !displayed?.items.length && view === 'discovery' ? (
                  <div className="empty compact" role="status">
                    <p>
                      {discovery.incomplete_only
                        ? '没有可列出的待补充目录。尚未发现的位置无法估算，请选择较小范围。'
                        : discovery.category === 'unclassified'
                          ? '本次已读取范围内，没有符合条件的未归类项目。'
                          : '本次已读取范围内，没有符合这些筛选条件的项目。'}
                      {snapshot.state !== 'completed'
                        ? '统计仍不完整，不能据此判断整个范围没有占用。'
                        : ''}
                    </p>
                    <div className="inline-actions">
                      <button
                        className="result-link"
                        onClick={() =>
                          changeDiscovery({
                            search: '',
                            min_bytes: '0',
                            category: 'all',
                            incomplete_only: false,
                          })
                        }
                      >
                        清除筛选限制
                      </button>
                      {discovery.kind === 'directory' && !discovery.include_nested && (
                        <button
                          className="result-link"
                          onClick={() => changeDiscovery({ include_nested: true })}
                        >
                          包含内部目录
                        </button>
                      )}
                      {discovery.incomplete_only && (
                        <button
                          className="result-link"
                          onClick={() => {
                            const fold =
                              document.querySelector<HTMLDetailsElement>('.analysis-location-fold');
                            if (fold) fold.open = true;
                            locationPickerButton.current?.focus();
                          }}
                        >
                          选择较小范围
                        </button>
                      )}
                    </div>
                  </div>
                ) : !displayed?.items.length ? (
                  <p className="empty compact">
                    {visibleQuery
                      ? view === 'files'
                        ? '已读取范围内没有符合名称的文件。'
                        : snapshot.state !== 'completed' || page?.node.incomplete
                          ? '当前目录已读取范围内没有符合名称的条目。'
                          : '完整读取的当前目录没有符合名称的条目。'
                      : snapshot.state !== 'completed' || page?.node.incomplete
                        ? '此目录尚未发现可显示的条目，统计并不完整。'
                        : view === 'files'
                          ? '本次范围没有已读取的文件。'
                          : '此目录没有文件或子目录。'}
                    {visibleQuery && snapshot.state !== 'completed' ? '仍有位置未完整读取。' : ''}
                  </p>
                ) : (
                  <div style={{ height: virtual.getTotalSize(), position: 'relative' }}>
                    {virtual.getVirtualItems().map((v) => {
                      const entry = displayed.items[v.index];
                      return (
                        <div
                          key={entry.id}
                          data-index={v.index}
                          ref={virtual.measureElement}
                          className="analysis-entry"
                          style={{
                            position: 'absolute',
                            top: 0,
                            left: 0,
                            width: '100%',
                            transform: `translateY(${v.start}px)`,
                          }}
                        >
                          <span
                            className="entry-size-band"
                            aria-hidden="true"
                            style={{
                              width: `${relativeSize(entryBytes(entry) ?? '0', largestDisplayed)}%`,
                            }}
                          />
                          <div className="analysis-entry-heading">
                            {onSelectManual && (
                              <input
                                type="checkbox"
                                aria-label={`选择${entry.name}`}
                                checked={manualChecked(entry, manualChoices)}
                                ref={(el) => {
                                  if (el) el.indeterminate = manualMixed(entry, manualChoices);
                                }}
                                disabled={busy || snapshot.stale || !entry.manual?.allowed}
                                title={
                                  entry.manual?.reason
                                    ? reasonLabels[entry.manual.reason]
                                    : '仅删除你明确选择的内容'
                                }
                                onChange={(e) => onSelectManual(entry, e.target.checked)}
                              />
                            )}
                            {entry.directory ? <FolderOpen size={18} /> : <File size={18} />}
                            <div className="analysis-entry-name">
                              {entry.directory ? (
                                <button
                                  className="result-link"
                                  data-analysis-entry={entry.id}
                                  onClick={() => navigate(entry.id, entry, v.index)}
                                >
                                  {entry.name}
                                </button>
                              ) : (
                                <button
                                  className="result-link"
                                  data-analysis-review={entry.id}
                                  aria-label={`查看详情：${entry.name}`}
                                  onClick={() => openReview(entry, v.index)}
                                >
                                  {entry.name}
                                </button>
                              )}
                              <small>
                                {entry.directory
                                  ? `${entry.files.toLocaleString()} 个文件`
                                  : '文件'}
                                {entry.shared ? ' · 含共享硬链接' : ''}
                                {entry.verification_pending ? ' · 清理前核验' : ''}
                                {entry.excluded ? ' · 已在清理设置中保留' : ''}
                                {entry.rule_ids.length > 0 &&
                                entry.rule_ids.every((id) => excludedRules.includes(id))
                                  ? ' · 对应清理类别已停用'
                                  : ''}
                              </small>
                            </div>
                            <strong>
                              {entry.pending
                                ? '待统计'
                                : entryBytes(entry) === null
                                  ? '占用未知'
                                  : `${entry.incomplete || (useAllocated && entry.allocation_unknown) ? '已知至少 ' : ''}${formatBytes(entryBytes(entry)!)}`}
                            </strong>
                          </div>
                          {(view === 'discovery' || view === 'files') && (
                            <p className="discovery-location" title={entry.path}>
                              {entry.path.slice(
                                0,
                                Math.max(entry.path.lastIndexOf('\\'), entry.path.lastIndexOf('/')),
                              ) || entry.path}
                            </p>
                          )}
                          {entry.issue && (
                            <p className="muted small-text">{issueLabel(entry.issue)}</p>
                          )}
                          <EntryAdvice entry={entry} onReview={() => openReview(entry, v.index)} />
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
              <div className="pagination">
                {onSelectManualBatch && (
                  <button
                    className="result-link"
                    disabled={busy || snapshot.stale || !displayed?.items.length}
                    onClick={() => onSelectManualBatch?.(displayed?.items ?? [], true)}
                  >
                    选择本页可选项
                  </button>
                )}
                {onSelectManualBatch && (
                  <button
                    className="result-link"
                    disabled={busy || snapshot.stale || !displayed?.items.length}
                    onClick={() => onSelectManualBatch(displayed?.items ?? [], false)}
                  >
                    取消本页选择
                  </button>
                )}
                <span>
                  {displayed?.count_pending
                    ? '总数统计中'
                    : displayed?.count_failed
                      ? '总数未完成'
                      : `共 ${displayed?.total.toLocaleString() ?? '—'} 项`}{' '}
                  · 第 {Math.floor(visibleOffset / 200) + 1} 页
                </span>
                <div>
                  <button
                    className="button secondary small"
                    disabled={loading || visibleOffset === 0}
                    onClick={() => changeOffset((n) => n - 200)}
                  >
                    上一页
                  </button>
                  <button
                    className="button secondary small"
                    disabled={
                      loading ||
                      !displayed ||
                      (snapshot.query_api_version === 2
                        ? !displayed.next_cursor
                        : visibleOffset + 200 >= displayed.total)
                    }
                    onClick={() => changeOffset((n) => n + 200)}
                  >
                    下一页
                  </button>
                </div>
              </div>
            </>
          )}
        </div>
      )}
      {review && review.session === snapshot?.id && (
        <AnalysisReviewDialog
          key={review.key}
          target={review}
          open={reviewOpen && active}
          busy={busy || !desktop}
          stale={!!snapshot?.stale}
          taskNotice={taskNotice}
          onCheckTask={onCheckTask}
          onClose={closeReview}
          onRestoreFocus={() => setFocusEpoch((value) => value + 1)}
          choices={manualChoices}
          onSelect={onSelectManual}
          onExclude={onExcludeManual ? () => onExcludeManual(review.entry) : undefined}
          onParent={
            review.entry.parent
              ? () => {
                  setReviewOpen(false);
                  setReview(null);
                  restorePending.current = null;
                  navigate(review.entry.parent!, review.entry, review.origin.index);
                  requestAnimationFrame(() => scroll.current?.focus({ preventScroll: true }));
                }
              : undefined
          }
          onInvalid={() => {
            if (latestReview.current?.key !== review.key) return;
            setReview(null);
            setReviewOpen(false);
            setDiscoveryOrigin(null);
            setReviewNotice('分析会话已失效，请重新分析。');
          }}
          onCleanup={() => {
            closeReview();
            space.returnFocus.current = null;
            onCleanup(review.entry.rule_ids);
          }}
          onRefine={async () => {
            const location = await api.refineAnalysis(review.session, review.entry.id);
            if (latestReview.current?.key !== review.key) return;
            setLocations((list) => [
              ...list.filter((item) => item.token !== location.token),
              location,
            ]);
            setSelected(location.token);
            setLocationError('');
            restorePending.current = null;
            setReviewOpen(false);
            setReview(null);
            setReviewNotice('已选择此目录。请点击“开始只读分析”补充统计，新分析会替换当前结果。');
            requestAnimationFrame(() => {
              if (!latestReview.current) startAnalysisButton.current?.focus();
            });
          }}
        />
      )}
    </section>
  );
}

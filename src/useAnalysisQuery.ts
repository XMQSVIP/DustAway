import { useEffect, useRef, useState } from 'react';
import { defaultDiscovery } from './analysisQuery';
import { api } from './api';
import { ipcError } from './coordinator';
import type {
  AnalysisDiscoveryQuery,
  AnalysisEntry,
  AnalysisFilesPage,
  AnalysisPage,
  AnalysisSnapshot,
} from './types';

/** Query state, cursor lifetime and cancellation share one owner across all three views. */
export function useAnalysisQuery({
  active,
  snapshot,
  differentLocation,
  onInvalidSession,
}: {
  active: boolean;
  snapshot: AnalysisSnapshot | null;
  differentLocation: boolean;
  onInvalidSession: () => void;
}) {
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
  const visibleOffset =
    view === 'discovery' ? discovery.offset : view === 'files' ? fileOffset : offset;
  const visibleQuery =
    view === 'discovery' ? discovery.search : view === 'files' ? fileQuery : query;
  const changeDiscoveryOffset = (change: (value: number) => number) =>
    setDiscovery((d) => ({ ...d, offset: change(d.offset) }));
  const changeOffset =
    view === 'discovery' ? changeDiscoveryOffset : view === 'files' ? setFileOffset : setOffset;
  useEffect(() => {
    setNode('0');
    setOffset(0);
    setSearch('');
    setQuery('');
    setPage(null);
    setDiscovery({ ...defaultDiscovery });
    setDiscoverySearch('');
    setDiscoveryPage(null);
    setView(snapshot?.state === 'scanning' ? 'directories' : 'discovery');
    setFileOffset(0);
    setFileSearch('');
    setFileQuery('');
    setFilePage(null);
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
            onInvalidSession();
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
  return {
    node,
    setNode,
    search,
    setSearch,
    query,
    setQuery,
    offset,
    setOffset,
    descending,
    setDescending,
    page,
    view,
    setView,
    discovery,
    setDiscovery,
    discoverySearch,
    setDiscoverySearch,
    fileSearch,
    setFileSearch,
    fileQuery,
    setFileQuery,
    fileOffset,
    setFileOffset,
    sizeMetric,
    setSizeMetric,
    useAllocated,
    entryBytes,
    readError,
    loading,
    setEpoch,
    scroll,
    successfulKey,
    currentReadKey,
    hasEntries,
    displayed,
    visibleOffset,
    visibleQuery,
    changeOffset,
  };
}

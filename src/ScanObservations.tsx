import { useEffect, useRef, useState } from 'react';
import { api } from './api';
import { formatBytes, reasonLabels } from './i18n';
import type { ObservationPage, Scope } from './types';

interface ObservationProps {
  scope: Scope;
  session: string;
  rule: string;
}
export function ScanObservations(props: ObservationProps) {
  return <ObservationView key={`${props.scope}:${props.session}:${props.rule}`} {...props} />;
}
function ObservationView({ scope, session, rule }: ObservationProps) {
  const [page, setPage] = useState<ObservationPage | null>(null);
  const [offset, setOffset] = useState(0);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const [retry, setRetry] = useState(0);
  const region = useRef<HTMLDivElement>(null);
  const restoreFocus = useRef(false);
  useEffect(() => {
    let current = true;
    setLoading(true);
    setError('');
    setPage(null);
    api
      .observations(scope, session, rule, offset)
      .then((value) => {
        if (current) setPage(value);
      })
      .catch((e) => {
        if (current) setError(String(e));
      })
      .finally(() => {
        if (current) setLoading(false);
      });
    return () => {
      current = false;
    };
  }, [scope, session, rule, offset, retry]);
  useEffect(() => {
    if (!loading && restoreFocus.current) {
      if (document.activeElement === document.body) region.current?.focus();
      restoreFocus.current = false;
    }
  }, [loading]);
  return (
    <div
      ref={region}
      tabIndex={-1}
      className="scan-observations"
      aria-label="已保留文件明细"
      aria-busy={loading}
    >
      <p>只读明细 · 按文件大小从大到小排列 · 不加入清理清单</p>
      {loading && <p role="status">正在读取保留明细…</p>}
      {error && (
        <div>
          <p role="alert">保留明细暂不可用：{error}</p>
          <button
            disabled={loading}
            onClick={() => {
              restoreFocus.current = true;
              setRetry((value) => value + 1);
            }}
          >
            重试本页
          </button>
        </div>
      )}
      {page?.items.map((file) => (
        <article key={file.path}>
          <span className="observation-path">{file.path}</span>
          <span>
            {file.bytes === null ? '大小未知' : formatBytes(file.bytes)} ·{' '}
            {file.modified_unix === null
              ? '修改时间未知'
              : new Date(file.modified_unix * 1000).toLocaleString()}{' '}
            · {reasonLabels[file.reason]}
          </span>
        </article>
      ))}
      {page && (
        <div className="observation-pages">
          <button
            disabled={loading || offset === 0}
            onClick={() => setOffset(Math.max(0, offset - 50))}
          >
            上一页
          </button>
          <span>
            {page.total
              ? `${offset + 1}–${offset + page.items.length} / ${page.total}`
              : '本次没有可展示的保留明细'}
          </span>
          <button
            disabled={loading || offset + 50 >= page.total}
            onClick={() => setOffset(offset + 50)}
          >
            下一页
          </button>
        </div>
      )}
    </div>
  );
}

import { HardDrive, RefreshCw } from 'lucide-react';
import { desktop } from './api';
import { formatBytes, formatTime } from './i18n';
import type { useHomeSpace, DiskReading } from './useHomeSpace';
import type { AnalysisLocation } from './types';

function Reading({ volume, reading }: { volume: AnalysisLocation; reading?: DiskReading }) {
  const value = reading?.context;
  const stale = !!reading?.loading || !!reading?.error;
  return (
    <div
      className="home-disk-reading"
      data-status={reading?.loading || !reading ? 'loading' : reading.error ? 'error' : 'ready'}
      title={value ? `${formatTime(value.sampled_at)} 查询` : undefined}
      aria-label={`${volume.path}可用空间`}
      aria-busy={reading?.loading ?? true}
    >
      <div className="storage-caption">
        <span>{stale && value ? '上次查询可用' : '当前可用'}</span>
        <span>
          <HardDrive size={13} aria-hidden="true" />
          {volume.path}
        </span>
      </div>
      <strong className="storage-value">
        {value?.available_bytes != null ? formatBytes(value.available_bytes) : '—'}
      </strong>
      <small>
        {reading?.loading || !reading
          ? value
            ? '正在查询 · 上次读数'
            : '正在查询…'
          : reading.error
            ? value
              ? '读取失败 · 上次读数'
              : '读取失败，可重试'
            : value
              ? `${formatTime(value.sampled_at)} 查询`
              : '暂未取得磁盘读数'}
      </small>
    </div>
  );
}
export function HomeStorage({
  space,
  pending = false,
  unavailable = false,
}: {
  space: ReturnType<typeof useHomeSpace>;
  pending?: boolean;
  unavailable?: boolean;
}) {
  const { volumes, readings, refresh } = space;
  return (
    <section className="home-storage" aria-label="磁盘可用空间" data-multiple={volumes.length > 1}>
      <div className="home-disk-list">
        {volumes.length > 1 && (
          <strong className="home-disks-title">已选 {volumes.length} 个磁盘</strong>
        )}
        {!volumes.length && (
          <p className="muted">
            {pending
              ? '正在读取磁盘…'
              : unavailable
                ? '暂未取得磁盘读数'
                : '选择磁盘后显示可用空间'}
          </p>
        )}
        {volumes.slice(0, 3).map((volume) => (
          <Reading key={volume.token} volume={volume} reading={readings.get(volume.token)} />
        ))}
        {volumes.length > 3 && (
          <details className="home-more-disks">
            <summary>其余 {volumes.length - 3} 个磁盘</summary>
            {volumes.slice(3).map((volume) => (
              <Reading key={volume.token} volume={volume} reading={readings.get(volume.token)} />
            ))}
          </details>
        )}
      </div>
      <div className="storage-meta">
        <small>{volumes.length > 1 ? '逐盘可用空间' : '所选磁盘可用空间'}</small>
        <button
          className="icon-button"
          title="刷新磁盘空间"
          aria-label="刷新磁盘空间"
          disabled={!desktop || !volumes.length || [...readings.values()].some((r) => r.loading)}
          onClick={refresh}
        >
          <RefreshCw size={14} />
        </button>
      </div>
    </section>
  );
}

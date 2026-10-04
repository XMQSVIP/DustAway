import { HardDrive } from 'lucide-react';
import type { AnalysisLocation } from './types';

export function ScanVolumes({
  locations,
  selection,
  disabled,
  error,
  loading = false,
  loaded = true,
  onChange,
  onReload,
}: {
  locations: AnalysisLocation[];
  selection: string[] | null;
  disabled: boolean;
  error: string;
  loading?: boolean;
  loaded?: boolean;
  onChange: (tokens: string[] | null) => void;
  onReload: () => void;
}) {
  const volumes = locations.filter((l) => l.kind === 'volume');
  const available = volumes.filter((l) => l.supported !== false);
  const selected = selection === null ? available.map((l) => l.token) : selection;
  const count = available.filter((l) => selected.includes(l.token)).length;
  const all = selection === null || (available.length > 0 && count === available.length);
  return (
    <fieldset className="scan-volumes" disabled={disabled || loading || !loaded}>
      <legend>扫描磁盘</legend>
      <div className="scan-volume-options">
        <label className="scan-volume-option" data-selected={all}>
          <input
            type="checkbox"
            checked={all}
            ref={(el) => {
              if (el) el.indeterminate = !all && count > 0;
            }}
            onChange={(e) => onChange(e.target.checked ? null : [])}
          />
          全部磁盘
        </label>
        {volumes.map((volume) => (
          <label
            key={volume.token}
            className="scan-volume-option"
            data-selected={volume.supported !== false && selected.includes(volume.token)}
            title={volume.unavailable_reason || volume.title}
          >
            <input
              type="checkbox"
              aria-label={`扫描 ${volume.path}`}
              disabled={volume.supported === false}
              checked={volume.supported !== false && selected.includes(volume.token)}
              onChange={(e) =>
                onChange(
                  e.target.checked
                    ? [...new Set([...selected, volume.token])]
                    : selected.filter((id) => id !== volume.token),
                )
              }
            />
            <HardDrive size={15} aria-hidden="true" />
            {volume.path.replace(/\\$/, '')}
            {volume.supported === false && <small>暂不支持</small>}
          </label>
        ))}
      </div>
      {loading || !loaded ? (
        <p role="status">正在读取磁盘列表…</p>
      ) : error ? (
        <p role="status">磁盘列表读取失败</p>
      ) : !available.length ? (
        <p role="status">没有可用的磁盘。</p>
      ) : selection !== null && count === 0 ? (
        <p role="status">请至少选择一个可用磁盘。</p>
      ) : (
        <p>扫描{all ? '全部支持的磁盘' : `${count} 个所选磁盘`}中的常见垃圾位置</p>
      )}
      {loaded && !loading && (error || !available.length) && (
        <button type="button" className="result-link" onClick={onReload}>
          重新读取磁盘
        </button>
      )}
    </fieldset>
  );
}

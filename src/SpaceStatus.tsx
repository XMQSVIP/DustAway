import { desktop } from './api';
import { formatBytes, formatTime } from './i18n';
import type { SpaceJourney } from './useSpaceJourney';

export function SpaceStatus({
  journey,
  onAnalysis,
  compact = false,
  review = false,
}: {
  journey: Pick<
    SpaceJourney,
    | 'context'
    | 'baseline'
    | 'contextLoading'
    | 'contextError'
    | 'selected'
    | 'refresh'
    | 'locationError'
    | 'reloadLocations'
  >;
  onAnalysis?: () => void;
  compact?: boolean;
  review?: boolean;
}) {
  const { context, baseline, contextLoading, contextError, selected, refresh } = journey;
  const change =
    context &&
    baseline &&
    !context.error &&
    !contextError &&
    !contextLoading &&
    context.volume_key &&
    context.volume_key === baseline.volume_key &&
    context.available_bytes !== null &&
    baseline.available_bytes !== null
      ? BigInt(context.available_bytes) - BigInt(baseline.available_bytes)
      : null;
  if (!selected)
    return (
      <section
        className="space-status compact-space-status action-folio"
        aria-label="所选位置的磁盘信息"
      >
        <div>
          <strong>磁盘可用空间未知</strong>
          <p>{journey.locationError || '尚未取得分析位置。'}</p>
        </div>
        <div className="inline-actions">
          <button className="result-link" disabled={!desktop} onClick={journey.reloadLocations}>
            重试读取位置
          </button>
          {onAnalysis && (
            <button className="result-link" onClick={onAnalysis}>
              选择分析位置
            </button>
          )}
        </div>
      </section>
    );
  return (
    <section
      className={`space-status action-folio${compact ? ' compact-space-status' : ''}`}
      aria-label="所选位置的磁盘信息"
      aria-busy={contextLoading}
    >
      <div className="space-status-reading">
        <span className="muted">{review ? '本项目所在磁盘' : '正在关注的磁盘'}</span>
        <strong>
          {context?.volume && context.available_bytes !== null
            ? `${context.volume} · ${contextError || contextLoading ? '上次成功查询可用空间' : '当前用户可用空间'} ${formatBytes(context.available_bytes)}`
            : contextLoading
              ? '正在查询磁盘空间…'
              : '磁盘可用空间未知'}
        </strong>
        <small>
          {context ? `${formatTime(context.sampled_at)} 查询。` : ''}
          {review ? '磁盘读数不会更新文件大小。' : '可用空间不等于本次可清理容量。'}
        </small>
        {change !== null && context && baseline && context !== baseline && (
          <details className="space-observation" open={compact ? undefined : true}>
            <summary>
              同卷观察变化：{change > 0n ? '+' : ''}
              {formatBytes(change)} · 非本工具删除量
            </summary>
            <p className="space-observation">
              从 {formatTime(baseline.sampled_at)} 到本次查询：{change > 0n ? '+' : ''}
              {formatBytes(change)}。
              这是同一磁盘的观察变化，包含其他程序和外部操作的影响，不计为拂尘删除成绩。
            </p>
          </details>
        )}
        {(context?.error || contextError) && (
          <p role="alert">本次刷新失败：{contextError || context?.error}。可确认位置后重试。</p>
        )}
      </div>
      <div className="inline-actions">
        <button
          className="button secondary small"
          disabled={!desktop || contextLoading}
          onClick={refresh}
        >
          {context?.error || contextError ? '重试磁盘查询' : '刷新磁盘空间'}
        </button>
        {onAnalysis && (
          <button className="result-link" onClick={onAnalysis}>
            {compact ? '更换关注位置' : '返回空间分析'}
          </button>
        )}
      </div>
      {compact && (
        <small className="space-scope-note">
          {review
            ? '同卷变化不能归因于本项目。'
            : '空间读数对应当前分析位置，与首页扫描磁盘分别设置。'}
        </small>
      )}
    </section>
  );
}

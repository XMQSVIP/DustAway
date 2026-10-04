import type { AnalysisEntry } from './types';
import { analysisDecision } from './analysisDecision';
import { reasonLabels } from './i18n';

export function EntryAdvice({
  entry,
  onReview,
}: {
  entry: AnalysisEntry;
  onReview: (trigger: HTMLElement) => void;
}) {
  const decision = analysisDecision(entry);
  return (
    <div className="entry-advice">
      <p className="entry-purpose">
        {decision.title}
        {entry.pending ? ' · 待统计' : entry.incomplete || entry.issue ? ' · 大小尚未统计完整' : ''}
      </p>
      {!entry.manual?.allowed && (
        <p className="entry-retained">
          已保留 ·{' '}
          {entry.manual?.reason
            ? (reasonLabels[entry.manual.reason] ?? '当前不可删除')
            : entry.rule_ids.length
              ? '请在对应清理类别中选择'
              : '尚未确认可以删除'}
        </p>
      )}
      {entry.directory && (
        <div className="inline-actions">
          <button
            className="result-link"
            data-analysis-review={entry.id}
            aria-label={`查看详情：${entry.name}`}
            onClick={(event) => onReview(event.currentTarget)}
          >
            详情
          </button>
        </div>
      )}
    </div>
  );
}

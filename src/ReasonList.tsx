import { formatResultCount, reasonNextStep } from './cleanupResults';
import { reasonLabels } from './i18n';
import type { ReasonCount } from './types';
export function ReasonList({
  title,
  reasons,
  showNextSteps = false,
}: {
  title: string;
  reasons: ReasonCount[];
  showNextSteps?: boolean;
}) {
  if (!reasons.length) return null;
  return (
    <details className="reason-list">
      <summary>
        {title} ·{' '}
        {formatResultCount(
          reasons.every((r) => Number.isSafeInteger(r.count) && r.count >= 0)
            ? reasons.reduce((n, r) => n + r.count, 0)
            : NaN,
        )}{' '}
        项
      </summary>
      <ul>
        {reasons.map((r) => (
          <li key={r.code}>
            <div>
              {reasonLabels[r.code] ?? r.code}
              {showNextSteps && <p className="muted">后续处理：{reasonNextStep(r.code)}</p>}
            </div>
            <span>{formatResultCount(r.count)} 项</span>
          </li>
        ))}
      </ul>
    </details>
  );
}

import type { ScanCoverage } from './types';
import { reasonLabels } from './i18n';

export function CoverageNotice({ coverage }: { coverage?: ScanCoverage | null }) {
  if (!coverage) return <p className="muted small-text">旧记录未提供扫描完整程度。</p>;
  if (coverage.complete) return null;
  return (
    <div className="coverage-notice">
      <p>扫描结果不完整；清理范围仅包含已通过安全校验并确认的候选。</p>
      {coverage.issues.length > 0 && (
        <details className="reason-list">
          <summary>
            读取问题 ·{' '}
            {coverage.issues.reduce((sum, issue) => sum + issue.count, 0).toLocaleString()} 次
          </summary>
          <p className="muted small-text">次数不代表未读取的文件数量，未读取容量不作估算。</p>
          <ul>
            {coverage.issues.map((issue) => (
              <li key={issue.code}>
                {reasonLabels[issue.code]}
                <span>{issue.count.toLocaleString()} 次</span>
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}

import { forwardRef } from 'react';
import { resultLabels, type ResultStatus, type ResultTarget } from './cleanupResults';

export const ResultReviewPanel = forwardRef<
  HTMLDivElement,
  {
    status: ResultStatus;
    targets: ResultTarget[];
    onOpen: (target: ResultTarget) => void;
    onReport: () => void;
  }
>(({ status, targets, onOpen, onReport }, ref) => (
  <div
    className="result-review surface"
    ref={ref}
    tabIndex={-1}
    role="region"
    aria-label={`${resultLabels[status]}类别`}
  >
    <h2>{resultLabels[status]} · 按类别查看</h2>
    <p>仅列出本次记录中对应状态的类别；查看不会提交清理。</p>
    <ul>
      {targets.map((target) => (
        <li key={target.key}>
          <span>
            <strong>{target.title}</strong> · {target.count.toLocaleString()} 个文件
          </span>
          {target.session ? (
            <button className="result-link" onClick={() => onOpen(target)}>
              查看{target.title}的{resultLabels[status]}文件
            </button>
          ) : (
            <small>当前会话明细不可用，仅保留统计；不会读取其他会话的文件。</small>
          )}
        </li>
      ))}
    </ul>
    <button className="result-link" onClick={onReport}>
      查看完整报告
    </button>
  </div>
));

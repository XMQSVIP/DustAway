import * as Dialog from '@radix-ui/react-dialog';
import { X } from 'lucide-react';
import { useState } from 'react';
import type { AnalysisEntry, AnalysisLocation } from './types';
import { analysisDecision } from './analysisDecision';
import { api } from './api';
import { ipcError } from './coordinator';
import { formatBytes, formatTime, reasonLabels } from './i18n';
import { manualChecked, manualMixed, type ManualChoices } from './manualSelection';

export interface AnalysisReviewTarget {
  key: number;
  session: string;
  entry: AnalysisEntry;
  location: AnalysisLocation;
  analyzedAt: number;
}

export function AnalysisReviewDialog({
  target,
  open,
  busy,
  stale,
  onClose,
  onRestoreFocus,
  choices,
  onSelect,
  onExclude,
  onParent,
  onCleanup,
  onRefine,
  onInvalid,
  taskNotice,
  onCheckTask,
}: {
  target: AnalysisReviewTarget;
  open: boolean;
  busy: boolean;
  stale: boolean;
  onClose: () => void;
  onRestoreFocus: () => void;
  choices: ManualChoices;
  onSelect?: (entry: AnalysisEntry, selected: boolean) => void;
  onExclude?: () => void;
  onParent?: () => void;
  onCleanup: () => void;
  onRefine: () => Promise<void>;
  onInvalid: () => void;
  taskNotice?: string;
  onCheckTask?: () => void;
}) {
  const { entry } = target;
  const decision = analysisDecision(entry);
  const [error, setError] = useState('');
  const [working, setWorking] = useState(false);
  async function perform(action: () => Promise<void>) {
    setError('');
    setWorking(true);
    try {
      await action();
    } catch (error) {
      const failure = ipcError(error);
      if (failure.code === 'invalid_session') onInvalid();
      else setError(failure.message);
    } finally {
      setWorking(false);
    }
  }
  const disabled = busy || working;
  return (
    <Dialog.Root
      open={open}
      onOpenChange={(value) => {
        if (!value) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="overlay" />
        <Dialog.Content
          className="dialog analysis-review-dialog side-sheet"
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            onRestoreFocus();
          }}
        >
          <div className="dialog-heading">
            <div>
              <Dialog.Title>文件详情</Dialog.Title>
              <Dialog.Description>查看文件用途和保留原因；勾选即加入本次清理。</Dialog.Description>
            </div>
            <Dialog.Close className="icon-button" aria-label="关闭文件详情">
              <X size={20} />
            </Dialog.Close>
          </div>
          <div className="analysis-review-body">
            {taskNotice && (
              <p role="status" className="message info">
                {taskNotice}
                {onCheckTask && (
                  <button className="result-link" onClick={onCheckTask}>
                    核对任务状态
                  </button>
                )}
              </p>
            )}
            <section className="review-subject">
              <h3>{entry.name || target.location.title}</h3>
              <p className="full-path">{entry.path}</p>
              <p>
                {entry.pending
                  ? '待统计'
                  : `${entry.incomplete || entry.issue ? '已读取至少 ' : ''}${formatBytes(entry.bytes)}`}{' '}
                · 文件大小，非可释放空间
              </p>
              {entry.allocated_bytes !== undefined && (
                <p>
                  {entry.allocated_bytes === null
                    ? '占用空间未知'
                    : `${entry.allocation_unknown || entry.incomplete ? '已知占用至少 ' : '占用空间 '}${formatBytes(entry.allocated_bytes)}`}
                  {entry.shared ? ' · 含共享数据' : ''}
                </p>
              )}
              {entry.verification_pending && <p>清理前会重新核验文件身份、变更和硬链接数量。</p>}
              <small>
                {formatTime(target.analyzedAt)} 开始的分析结果
                {stale ? ' · 清理前统计，需要重新分析' : ''}
              </small>
            </section>
            <div className="detail-selection">
              {entry.manual?.allowed && onSelect ? (
                <label>
                  <input
                    type="checkbox"
                    aria-label={`选择${entry.name}`}
                    checked={manualChecked(entry, choices)}
                    ref={(el) => {
                      if (el) el.indeterminate = manualMixed(entry, choices);
                    }}
                    disabled={disabled || stale}
                    onChange={(e) => onSelect(entry, e.target.checked)}
                  />
                  {manualMixed(entry, choices)
                    ? '部分内容已保留'
                    : manualChecked(entry, choices)
                      ? '已选，确认后清理'
                      : '选择此项清理'}
                </label>
              ) : (
                <p>
                  已保留 ·{' '}
                  {entry.manual?.reason
                    ? (reasonLabels[entry.manual.reason] ?? '当前不可删除')
                    : '尚未确认可以删除'}
                </p>
              )}
              {entry.directory && entry.manual?.allowed && (
                <small>只包含扫描时发现的已选内容，受保护项目继续保留。</small>
              )}
            </div>
            <section aria-label="处理建议">
              <h3>{decision.title}</h3>
              <p>{decision.description}</p>
              {decision.primary === 'refine' && (
                <p className="review-guidance">{decision.guidance}</p>
              )}
              {decision.primary === 'refine' && (
                <button
                  className="button secondary"
                  disabled={disabled}
                  onClick={() => void perform(onRefine)}
                >
                  选择此目录补充分析
                </button>
              )}
              {entry.rule_ids.length > 0 && (
                <button className="button secondary" disabled={disabled} onClick={onCleanup}>
                  查看对应清理类别
                </button>
              )}
            </section>
            <details className="review-more">
              <summary>更多操作</summary>
              <div className="inline-actions">
                {onParent && (
                  <button className="result-link" disabled={disabled} onClick={onParent}>
                    打开所在目录
                  </button>
                )}
                {onExclude && entry.manual?.allowed && (
                  <button className="result-link" disabled={disabled || stale} onClick={onExclude}>
                    长期保留
                  </button>
                )}
              </div>
              {decision.canManageWindows && (
                <div className="inline-actions">
                  <button
                    className="button secondary"
                    disabled={disabled}
                    onClick={() => void perform(api.storage)}
                  >
                    Windows 存储设置
                  </button>
                  <button
                    className="result-link"
                    disabled={disabled}
                    onClick={() => void perform(api.apps)}
                  >
                    Windows 应用管理
                  </button>
                </div>
              )}
              {decision.canReveal && (
                <button
                  className="result-link"
                  disabled={disabled}
                  onClick={() => void perform(() => api.revealAnalysis(target.session, entry.id))}
                >
                  在资源管理器中定位
                </button>
              )}
            </details>
            <details className="review-evidence">
              <summary>依据与完整路径</summary>
              {decision.modelHint && (
                <p>
                  本地模型推测，尚未经真实机器数据验证。依据：
                  {decision.modelHint.evidence
                    .map((feature) =>
                      feature
                        .replace(/^dir:/, '目录名含 ')
                        .replace(/^name:/, '名称含 ')
                        .replace(/^ext:/, '扩展名 .'),
                    )
                    .join('、')}
                  。 仅作用途参考，不改变清理资格。
                </p>
              )}
              {decision.recognition && (
                <>
                  <p>
                    本地识别库 · 可能用途：{decision.recognition.evidence}。用途推断不代表可以删除。
                  </p>
                  <p className="full-path">
                    参考来源：{decision.recognition.source}（核对日期：
                    {decision.recognition.reviewed}）
                  </p>
                </>
              )}
              <p>{decision.explanation}</p>
              <p>位置依据：{decision.basis}。位置说明不构成删除授权。</p>
              <p className="full-path">{entry.path}</p>
              {entry.excluded && <p>已在清理设置中保留；查看建议不会取消排除。</p>}
            </details>
            {error && <p role="alert">{error}。可核对后重试；未确认操作已完成。</p>}
          </div>
          <div className="analysis-review-actions">
            <small>查看详情不会改变清理选择。</small>
            <button className="button primary" onClick={onClose}>
              返回列表
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

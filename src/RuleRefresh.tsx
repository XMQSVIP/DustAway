import { useLayoutEffect, useRef } from 'react';
import type { AppController } from './useAppController';
import type { RuleSummary } from './types';

export const hasToolIssue = (row?: RuleSummary) =>
  !!(
    row?.stale ||
    row?.scan_failed ||
    row?.coverage?.complete === false ||
    ['partial', 'failed', 'interrupted'].includes(row?.phase ?? '') ||
    row?.coverage?.issues.length ||
    (row?.tool &&
      !row.tool.ready &&
      !['tool_missing', 'cache_missing'].includes(row.tool.reason ?? '') &&
      (row.tool.executable || row.tool.reason))
  );

export const confirmedEmptyResult = (row?: RuleSummary) =>
  !!row &&
  row.files === 0 &&
  row.phase === 'completed' &&
  !row.stale &&
  !row.scan_failed &&
  row.coverage?.complete !== false &&
  !row.coverage?.issues.length &&
  (!row.tool || row.tool.ready || row.tool.reason === 'cache_missing');

/** Shared inline feedback; never calls a cleanup command. */
export function RuleRefreshNotice({ model, rule }: { model: AppController; rule: string }) {
  const row = model.summaryRows?.find((r) => r.rule_id === rule);
  const refresh = model.ruleRefresh?.rule_id === rule ? model.ruleRefresh : null;
  const running = refresh?.state === 'scanning';
  const section = useRef<HTMLDivElement>(null);
  const wasRunning = useRef(false);
  const focusInSection = useRef(false);
  useLayoutEffect(() => {
    if (
      wasRunning.current &&
      !running &&
      focusInSection.current &&
      (document.activeElement === document.body ||
        section.current?.contains(document.activeElement))
    )
      section.current?.focus({ preventScroll: true });
    wasRunning.current = running;
  }, [running]);
  useLayoutEffect(
    () => () => {
      if (section.current?.contains(document.activeElement)) {
        queueMicrotask(() =>
          (
            document.getElementById('official-tools-heading') ??
            document.querySelector<HTMLElement>('.desk-results') ??
            document.getElementById('main')
          )?.focus({ preventScroll: true }),
        );
      }
    },
    [],
  );
  const error = model.ruleRefreshError?.[rule] || refresh?.error;
  return (
    <div
      ref={section}
      tabIndex={-1}
      className="rule-refresh-notice"
      aria-label="此项检查状态"
      onFocusCapture={() => {
        focusInSection.current = true;
      }}
      onBlurCapture={(event) => {
        if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget))
          focusInSection.current = false;
      }}
    >
      {row?.tool && !row.tool.ready && <p>{row.tool.message}</p>}
      {(row?.scan_failed ||
        row?.coverage?.complete === false ||
        ['partial', 'failed', 'interrupted'].includes(row?.phase ?? '')) && (
        <p>此项检查不完整，暂不能确认可整理内容；请重新检查。</p>
      )}
      {running ? (
        <>
          <p role="status">
            正在检查这一项，已检查 {refresh.progress.checked.toLocaleString()}{' '}
            个目录项；其他选择已保留。
          </p>
          <button className="result-link" onClick={() => void model.cancelRuleRefresh()}>
            取消此项检查
          </button>
        </>
      ) : (
        <>
          {error && <p role="alert">{error}</p>}
          {refresh?.state === 'finished' && !row?.stale && (
            <p role="status">
              此项检查已更新，其他选择已保留。
              {confirmedEmptyResult(row)
                ? row?.tool?.reason === 'cache_missing'
                  ? '当前位置没有可整理的缓存。'
                  : '没有符合当前规则的文件。'
                : hasToolIssue(row) || row?.coverage?.complete === false
                  ? '检查未通过，暂不能确认可整理内容；请查看上方原因。'
                  : ''}
            </p>
          )}
          {model.uncertain ? (
            <button className="result-link" onClick={model.retryStatus}>
              重新查询状态
            </button>
          ) : (
            <button
              className="result-link"
              disabled={model.busy || model.saving}
              onClick={() => void model.refreshRule(rule)}
            >
              重新检查这一项
            </button>
          )}
        </>
      )}
    </div>
  );
}

import { useEffect, useState } from 'react';
import type { Job, Report, Rule, Scope, Session, Summary } from './types';

/** A detail view is valid only for the exact scan session and optional report it opened. */
export function useCandidateDetails({
  sessions,
  summaries,
  jobs,
  reports,
  setNotice,
}: {
  sessions: Session[];
  summaries: Partial<Record<Scope, Summary>>;
  jobs: Partial<Record<Scope, Job>>;
  reports: Report[];
  setNotice: (message: string) => void;
}) {
  const [detail, setDetail] = useState<Rule | null>(null);
  const [detailQuery, setDetailQuery] = useState<{
    view: 'scan' | 'cleanup' | 'unplanned';
    status: string | null;
    session: string;
    reportId: string | null;
  }>({ view: 'scan', status: null, session: '', reportId: null });
  function openDetail(
    rule: Rule,
    view: 'scan' | 'cleanup' | 'unplanned' = reports.length ? 'cleanup' : 'scan',
    status: string | null = null,
    reportId?: string,
  ) {
    const scope = rule.requires_admin ? 'system' : 'user';
    const session = sessions.find((s) => s.scope === scope)?.id;
    const report = reports.find((r) => r.scope === scope);
    if (
      !session ||
      summaries[scope]?.session_id !== session ||
      jobs[scope]?.id !== session ||
      !summaries[scope]?.rules.some((r) => r.rule_id === rule.id) ||
      (view !== 'scan' && (!report || (reportId !== undefined && reportId !== report.id)))
    ) {
      setNotice('本次文件明细已不可用，保留的统计仍可在报告中核对；不会借用其他扫描会话。');
      return;
    }
    setDetailQuery({ view, status, session, reportId: view === 'scan' ? null : report!.id });
    setDetail(rule);
  }
  const detailScope = detail?.requires_admin ? 'system' : 'user';
  const detailCurrent =
    !!detail &&
    sessions.some((s) => s.scope === detailScope && s.id === detailQuery.session) &&
    jobs[detailScope]?.id === detailQuery.session &&
    summaries[detailScope]?.session_id === detailQuery.session &&
    !!summaries[detailScope]?.rules.some((r) => r.rule_id === detail.id) &&
    (detailQuery.reportId === null ||
      reports.some((r) => r.scope === detailScope && r.id === detailQuery.reportId));
  useEffect(() => {
    if (detail && !detailCurrent) setDetail(null);
  }, [detail, detailCurrent]);
  return { detail, setDetail, detailQuery, setDetailQuery, detailCurrent, openDetail };
}

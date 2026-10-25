import evidence from '../rules/cleanup-evidence.json';
export function cleanupEvidence(ruleId: string) {
  return evidence.find((row) => row.rule_id === ruleId);
}
export function CleanupEvidence({ ruleId }: { ruleId: string }) {
  const row = cleanupEvidence(ruleId);
  if (!row) return null;
  return (
    <details className="rule-evidence">
      <summary>{row.method === 'readonly' ? '为什么保留' : '为什么可以处理'}</summary>
      <p>{row.conclusion}</p>
      <p className="muted">
        适用范围：{row.applicability} · 资料核查：{row.reviewed}
      </p>
      <p>处理依据：{row.method === 'readonly' ? '当前仍保留限制' : '官方资料与固定范围保护规则'}</p>
      <p>
        对应清理操作验收：
        {row.real_acceptance === 'passed' ? '已通过对应操作验证' : '未验证'}
      </p>
      {row.sources.length ? (
        row.sources.map((source) => (
          <a
            className="result-link"
            key={source.url}
            href={source.url}
            target="_blank"
            rel="noreferrer"
          >
            {'title' in source && source.title ? source.title : '查看依据资料'}
          </a>
        ))
      ) : (
        <p>尚无足够的清理依据，保持只读。</p>
      )}
    </details>
  );
}

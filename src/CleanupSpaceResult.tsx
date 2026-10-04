import { formatRecordedBytes } from './cleanupResults';
import { HomeStorage } from './HomeStorage';
import type { AppController } from './useAppController';

/** Use native report samples; never infer physical savings from deleted file lengths. */
export function CleanupSpaceResult({ model }: { model: AppController }) {
  const samples = model.reports.flatMap((report) =>
    (report.volume_space_changes ?? []).map((sample) => ({ ...sample, report: report.id })),
  );
  return (
    <section className="result-section cleanup-space-result" aria-label="清理后的磁盘空间">
      <h3>清理后还剩多少空间</h3>
      {samples.length ? (
        samples.map((sample, index) => (
          <p key={`${sample.report}:${sample.volume}:${index}`}>
            {sample.volume} · 清理前{' '}
            {sample.before_bytes === null ? '未取得' : formatRecordedBytes(sample.before_bytes)}
            {' → '}清理后{' '}
            {sample.after_bytes === null ? '未取得' : formatRecordedBytes(sample.after_bytes)}
            {sample.error && '（采样不完整）'}
          </p>
        ))
      ) : (
        <p>本次未取得逐盘清理前后读数。</p>
      )}
      <p className="muted">
        实际删除量是文件逻辑大小。磁盘可用空间还受共享文件和其他程序写入影响。
      </p>
      <HomeStorage space={model.homeSpace} />
    </section>
  );
}

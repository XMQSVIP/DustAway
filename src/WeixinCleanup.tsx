import { useState } from 'react';
import { ChevronDown, MessageCircle } from 'lucide-react';
import { CleanupRuleList } from './CleanupRuleList';
import { scanAccounting } from './ScanTotals';
import { formatBytes, reasonLabels } from './i18n';
import { desktop } from './api';
import type { AppController } from './useAppController';
import './weixin.css';

export function WeixinCleanup({
  model,
  onAttachments,
}: {
  model: AppController;
  onAttachments: () => void;
}) {
  const [open, setOpen] = useState(false);
  const items = model.decision.items.filter((item) => item.rule.id.startsWith('weixin.'));
  const rows = model.summaryRows.filter((row) => row.rule_id.startsWith('weixin.'));
  const totals = scanAccounting({ decision: { ...model.decision, items }, summaryRows: rows });
  const scanned = rows.length > 0;
  const unread =
    scanned &&
    items.some(
      (item) =>
        !rows.some(
          (row) =>
            row.rule_id === item.rule.id &&
            ['completed', 'partial', 'failed', 'interrupted'].includes(row.phase),
        ),
    );
  const retainedReasons = [
    ...new Set(
      rows.flatMap((row) =>
        (row.observation?.reasons ?? []).map((reason) => reasonLabels[reason.code]),
      ),
    ),
  ];
  const pending = model.busy || model.saving;
  const running = items.some(
    (item) => item.capability === 'allowed' && ['running', 'process_unknown'].includes(item.status),
  );
  const amount = (bytes: bigint) =>
    !scanned
      ? '待扫描'
      : totals.unknown || unread
        ? `${formatBytes(bytes)} + 未知大小`
        : formatBytes(bytes);
  return (
    <section className="weixin-cleanup" aria-label="微信专项清理">
      <div className="weixin-heading">
        <button
          className="group-heading"
          aria-expanded={open}
          aria-controls="weixin-cleanup-details"
          onClick={() => setOpen(!open)}
        >
          <MessageCircle size={21} aria-hidden="true" />
          <strong>微信专项清理</strong>
          <ChevronDown size={17} className={open ? 'is-open' : ''} aria-hidden="true" />
        </button>
        <button
          className="button secondary small"
          disabled={pending || !desktop}
          onClick={onAttachments}
        >
          整理微信聊天附件
        </button>
      </div>
      <dl className="weixin-totals" aria-label="微信缓存与日志容量">
        <div>
          <dt>缓存与日志已扫描</dt>
          <dd>{amount(totals.observed)}</dd>
        </div>
        <div>
          <dt>可直接清理</dt>
          <dd>{scanned ? formatBytes(totals.available) : '待扫描'}</dd>
        </div>
        <div>
          <dt>已保留</dt>
          <dd>{amount(totals.retained)}</dd>
        </div>
      </dl>
      <p className="muted">
        缓存与日志已纳入常规扫描。聊天图片、视频和文件需选择账号后单独整理，不计入以上容量。
      </p>
      {!!retainedReasons.length && (
        <p className="muted">保留原因：{retainedReasons.join('、')}。</p>
      )}
      <div id="weixin-cleanup-details" hidden={!open}>
        <p>
          网页缓存用于加快内容加载。历史日志保留最近 7 天，崩溃转储保留最近 30
          天；各项是否开放清理见下方状态。
        </p>
        {!scanned && <p role="status">尚未扫描微信缓存与日志，不能据此判断微信没有占用空间。</p>}
        {totals.incomplete && <p role="status">部分微信位置尚未读完，容量仅统计已读取范围。</p>}
        <CleanupRuleList model={model} rules={items.map((item) => item.rule)} />
        <div className="inline-actions">
          <button
            className="button secondary small"
            disabled={pending || !desktop || !model.canScanVolumes}
            onClick={() => void model.scan()}
          >
            {scanned ? '重新常规扫描' : '开始常规扫描'}
          </button>
          {running && (
            <button
              className="result-link"
              disabled={pending || !desktop}
              onClick={() => void model.refreshAvailability()}
            >
              退出微信后重新检查
            </button>
          )}
        </div>
        <p className="muted">缓存按需勾选后，通过“清理已选”统一确认。尚未开放的项目只供查看。</p>
        <details className="weixin-boundaries">
          <summary>扫描范围与保留内容</summary>
          <p>
            当前识别微信 4.x
            的标准日志目录、内置网页组件，以及所选账号的聊天附件。自定义账号目录可在附件整理中选择；旧版
            WeChat Files 和商店版布局暂未覆盖。
          </p>
          <p>
            保留聊天数据库、登录信息、收藏、备份、小程序数据及未知格式。不能预览的加密图片会显示占用与保留原因；聊天附件不默认勾选。
          </p>
        </details>
      </div>
    </section>
  );
}

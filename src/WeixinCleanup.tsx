import { useState } from 'react';
import { ChevronDown, MessageCircle } from 'lucide-react';
import { CleanupRuleList } from './CleanupRuleList';
import { scanAmount } from './scanAssessment';
import { weixinStatus } from './weixinStatus';
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
  const status = weixinStatus(model);
  const { items, totals, scanned } = status;
  const pending = model.busy || model.saving;
  const historical = !!model.reports?.length || model.currentJob?.state === 'cleaning';
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
      <dl className="weixin-status" aria-label="微信检查状态">
        <div>
          <dt>扫描</dt>
          <dd>{status.progress}</dd>
        </div>
        <div>
          <dt>清理能力</dt>
          <dd>{status.permission}</dd>
        </div>
        <div>
          <dt>运行状态</dt>
          <dd>{status.runtime}</dd>
        </div>
      </dl>
      {status.incomplete && <p role="status">部分微信位置尚未读完，容量仅统计已读取范围。</p>}
      <div id="weixin-cleanup-details" hidden={!open}>
        <dl className="weixin-totals" aria-label="微信缓存与日志容量">
          <div>
            <dt>{historical ? '扫描时缓存与日志占用' : '缓存与日志已扫描'}</dt>
            <dd>{scanAmount(totals, 'observed')}</dd>
          </div>
          {!historical && (
            <>
              <div>
                <dt>可直接清理</dt>
                <dd>{scanAmount(totals, 'available')}</dd>
              </div>
              <div>
                <dt>已保留</dt>
                <dd>{scanAmount(totals, 'retained')}</dd>
              </div>
            </>
          )}
        </dl>
        {historical && <p className="muted">以上为扫描时占用，本次删除结果见清理回执。</p>}
        <p className="muted">
          缓存与日志已纳入常规扫描。聊天图片、视频和文件需选择账号后单独整理，不计入以上容量。
        </p>
        {!historical && !!status.reasons.length && (
          <p className="muted">
            主要保留原因：{status.reasons.slice(0, 3).join('、')}
            {status.reasons.length > 3 ? '等，展开查看全部原因' : ''}。
          </p>
        )}
        <p>
          网页缓存用于加快内容加载。历史日志保留最近 7 天，崩溃转储保留最近 30
          天；各项是否开放清理见下方状态。
        </p>
        {!scanned && <p role="status">尚未扫描微信缓存与日志，不能据此判断微信没有占用空间。</p>}
        <CleanupRuleList model={model} rules={items.map((item) => item.rule)} />
        <div className="inline-actions">
          <button
            className="button secondary small"
            disabled={pending || !desktop || !model.canScanVolumes}
            onClick={() => void model.scan()}
          >
            {scanned ? '重新常规扫描' : '开始常规扫描'}
          </button>
          <button
            className="result-link"
            disabled={pending || !desktop}
            onClick={() => void model.refreshAvailability()}
          >
            重新检查微信运行状态
          </button>
        </div>
        <p className="muted">
          运行状态复查保留原有候选和选择，不查找新增文件；查找新增文件请重新常规扫描。退出微信不会开放只读项目。
        </p>
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

import { useEffect, useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { File, FolderOpen, Image as ImageIcon, X } from 'lucide-react';
import { api, desktop } from './api';
import { formatBytes, formatTime } from './i18n';
import { useDialogFocus } from './dialogFocus';
import { ReportRow } from './ReportReceipt';
import {
  attachmentKindLabels,
  attachmentProviderLabels,
  type AttachmentProvider,
  type AttachmentDiscovery,
  type AttachmentEntry,
  type AttachmentKind,
  type AttachmentLocation,
  type AttachmentPage,
  type AttachmentSnapshot,
} from './attachmentTypes';
import type { Report, ScopedPlan } from './types';
import './attachments.css';

function Thumbnail({
  item,
  session,
  onPreview,
}: {
  item: AttachmentEntry;
  session: string;
  onPreview: (src: string) => void;
}) {
  const element = useRef<HTMLButtonElement>(null);
  const [visible, setVisible] = useState(false);
  const [src, setSrc] = useState('');
  const [error, setError] = useState('');
  useEffect(() => {
    if (!element.current || !item.preview_available) return;
    if (typeof IntersectionObserver === 'undefined') {
      setVisible(true);
      return;
    }
    const observer = new IntersectionObserver(([entry]) => setVisible(entry.isIntersecting));
    observer.observe(element.current);
    return () => observer.disconnect();
  }, [item.id, item.preview_available]);
  useEffect(() => {
    let valid = true;
    setSrc('');
    setError('');
    if (visible && item.preview_available)
      void api.attachmentThumbnail(session, item.id).then(
        (value) => {
          if (valid) setSrc(value);
        },
        (reason) => {
          if (valid) setError(String(reason));
        },
      );
    return () => {
      valid = false;
    };
  }, [visible, session, item.id, item.preview_available]);
  return (
    <div className="attachment-preview">
      <button
        type="button"
        ref={element}
        onClick={() => src && onPreview(src)}
        disabled={!src}
        aria-label={`预览${item.name}`}
      >
        {src ? <img src={src} alt={item.name} /> : <ImageIcon aria-hidden="true" size={30} />}
        <span>
          {attachmentKindLabels[item.kind]} · {formatBytes(item.bytes)}
        </span>
      </button>
      {error && <small>{`预览不可用：${error}`}</small>}
    </div>
  );
}

export function AttachmentPanel({ onClose }: { onClose: () => void }) {
  const focus = useDialogFocus();
  const [provider, setProvider] = useState<AttachmentProvider>('weixin');
  const [discovery, setDiscovery] = useState<AttachmentDiscovery | null>(null);
  const context = useRef(0);
  const [locations, setLocations] = useState<AttachmentLocation[]>([]);
  const [token, setToken] = useState('');
  const [snapshot, setSnapshot] = useState<AttachmentSnapshot | null>(null);
  const [page, setPage] = useState<AttachmentPage | null>(null);
  const [selected, setSelected] = useState(new Map<string, AttachmentEntry>());
  const [kind, setKind] = useState<AttachmentKind | ''>('');
  const [days, setDays] = useState(30);
  const [search, setSearch] = useState('');
  const [offset, setOffset] = useState(0);
  const [onlySelected, setOnlySelected] = useState(false);
  const [pending, setPending] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [preview, setPreview] = useState('');
  const [plans, setPlans] = useState<ScopedPlan[] | null>(null);
  const [running, setRunning] = useState(false);
  const [report, setReport] = useState<Report | null>(null);
  const [revision, setRevision] = useState(0);
  const preparing = useRef<string | null>(null);
  const submitting = useRef(false);
  const job = useRef('');
  const busy = pending || running || snapshot?.state === 'scanning';
  const selectedBytes = [...selected.values()].reduce((sum, item) => sum + BigInt(item.bytes), 0n);
  const selectionKey = onlySelected ? [...selected.keys()].join('|') : '';
  const session = snapshot?.session;
  const providerLabel = attachmentProviderLabels[provider];
  async function switchContext(nextProvider: AttachmentProvider, nextToken = '') {
    // Invalidate callbacks immediately, before waiting for the backend to retire the plan.
    context.current += 1;
    await api.closeAttachments();
    setSnapshot(null);
    setPage(null);
    setSelected(new Map());
    setPreview('');
    setPlans(null);
    setReport(null);
    setLoading(false);
    setOffset(0);
    setOnlySelected(false);
    setKind('');
    setDays(30);
    setSearch('');
    if (nextProvider !== provider) {
      setLocations([]);
      setDiscovery(null);
    }
    setProvider(nextProvider);
    setToken(nextToken);
  }
  async function action(fn: () => Promise<void>) {
    if (submitting.current) return;
    submitting.current = true;
    setPending(true);
    setError('');
    try {
      await fn();
    } catch (reason) {
      setError(String(reason));
    } finally {
      submitting.current = false;
      setPending(false);
    }
  }
  useEffect(() => {
    let valid = true;
    const generation = context.current;
    if (desktop)
      void api.attachmentLocations(provider).then(
        (value) => {
          if (valid && generation === context.current && value.provider === provider) {
            setDiscovery(value);
            setLocations(value.locations);
          }
        },
        (e) => {
          if (valid && generation === context.current) setError(String(e));
        },
      );
    return () => {
      valid = false;
    };
  }, [provider]);
  useEffect(() => {
    if (!session || (snapshot?.state !== 'scanning' && !running)) return;
    let valid = true;
    const generation = context.current;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        const next = await api.attachmentSnapshot();
        if (!valid || generation !== context.current) return;
        if (next?.session === session) setSnapshot(next);
        if (running) {
          const result = await api.report('user');
          if (!valid || generation !== context.current) return;
          if (result?.id === job.current && result.finished_at !== null) {
            // The last deletion may finish between the first snapshot and report.
            const settled = await api.attachmentSnapshot();
            if (!valid || generation !== context.current) return;
            if (settled?.session === session) setSnapshot(settled);
            setReport(result);
            setRunning(false);
            setPlans(null);
            const deleted = new Set(
              settled && settled.session === session ? settled.deleted_ids : [],
            );
            setSelected((previous) => new Map([...previous].filter(([id]) => !deleted.has(id))));
            setOffset(0);
            setRevision((v) => v + 1);
            return;
          }
        }
        if (next?.state === 'scanning' || running) timer = setTimeout(() => void poll(), 600);
      } catch (reason) {
        if (valid && generation === context.current) {
          setError(`状态读取失败，正在重新核对：${String(reason)}`);
          timer = setTimeout(() => void poll(), 1500);
        }
      }
    }
    void poll();
    return () => {
      valid = false;
      clearTimeout(timer);
    };
  }, [session, snapshot?.state, running]);
  useEffect(() => {
    if (!session || snapshot?.state === 'scanning') return;
    let valid = true;
    const generation = context.current;
    setLoading(true);
    const timer = setTimeout(() => {
      void api
        .attachments({
          session,
          kind: onlySelected ? null : kind || null,
          min_age_days: onlySelected ? 0 : days,
          search: onlySelected ? '' : search,
          offset,
          selected_ids: onlySelected ? selectionKey.split('|').filter(Boolean) : null,
        })
        .then(
          (value) => {
            if (valid && generation === context.current && value.snapshot.session === session) {
              setPage(value);
              setLoading(false);
            }
          },
          (reason) => {
            if (valid && generation === context.current) {
              setError(String(reason));
              setPage(null);
              setLoading(false);
            }
          },
        );
    }, 200);
    return () => {
      valid = false;
      clearTimeout(timer);
    };
  }, [session, snapshot?.state, kind, days, search, offset, onlySelected, selectionKey, revision]);
  function select(items: AttachmentEntry[], checked: boolean) {
    if (busy || plans) return;
    setSelected((previous) => {
      const next = new Map(previous);
      for (const item of items) {
        if (!checked) next.delete(item.id);
        else if (!item.cleanup_block && !item.outcome) next.set(item.id, item);
      }
      return next;
    });
  }
  const close = () => {
    if (busy) return;
    void action(async () => {
      await api.closeAttachments();
      onClose();
    });
  };
  async function prepare() {
    if (!session) return;
    await action(async () => {
      const id = crypto.randomUUID();
      preparing.current = id;
      try {
        setPlans(await api.prepareAttachments({ session, selected_ids: [...selected.keys()] }, id));
      } finally {
        preparing.current = null;
      }
    });
  }
  const available = page?.entries.filter((item) => !item.cleanup_block && !item.outcome) ?? [];
  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open) close();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="attachment-dialog"
          {...focus}
          onInteractOutside={(e) => e.preventDefault()}
          onEscapeKeyDown={(e) => {
            if (busy || plans || preview) {
              e.preventDefault();
              if (preview) setPreview('');
              else if (!busy)
                void action(async () => {
                  await api.cancelAttachmentPlan();
                  setPlans(null);
                });
            }
          }}
        >
          <header className="attachment-header">
            <div>
              <Dialog.Title>聊天附件整理 · {providerLabel}</Dialog.Title>
              <Dialog.Description>
                先辨认，再选择。附件不会自动勾选；文件时间是最后修改时间。
              </Dialog.Description>
            </div>
            <button
              className="icon-button"
              onClick={close}
              disabled={!!busy}
              aria-label="关闭附件整理"
            >
              <X />
            </button>
          </header>
          {!desktop && <p role="status">请在桌面程序中扫描本地聊天资料。</p>}
          {error && (
            <p role="alert" className="attachment-error">
              {error}
            </p>
          )}
          <div className="attachment-location">
            <label>
              应用
              <select
                value={provider}
                disabled={!!busy}
                onChange={(e) =>
                  void action(() => switchContext(e.target.value as AttachmentProvider))
                }
              >
                {Object.entries(attachmentProviderLabels).map(([key, label]) => (
                  <option key={key} value={key}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
            <label>
              账号资料位置
              <select
                value={token}
                disabled={!!busy}
                onChange={(e) => void action(() => switchContext(provider, e.target.value))}
              >
                <option value="">请选择资料位置</option>
                {locations.map((l) => (
                  <option key={l.token} value={l.token}>
                    {l.title} · {l.path}
                  </option>
                ))}
              </select>
            </label>
            <button
              className="button secondary small"
              disabled={!!busy || !desktop}
              onClick={() =>
                void action(async () => {
                  const location = await api.pickAttachment(provider);
                  if (location) {
                    if (location.provider !== provider) throw new Error('资料位置与应用不匹配');
                    await switchContext(provider, location.token);
                    setLocations((prev) => [
                      ...prev.filter((l) => l.token !== location.token),
                      location,
                    ]);
                  }
                })
              }
            >
              <FolderOpen size={16} />
              选择其他位置
            </button>
            <button
              className="button primary small"
              disabled={!token || !!busy || !!plans || !desktop}
              onClick={() =>
                void action(async () => {
                  const id = await api.startAttachments(token);
                  const next = await api.attachmentSnapshot();
                  if (next?.session !== id) throw new Error('扫描状态未确认，请重新核对');
                  setSnapshot(next);
                  setPage(null);
                  setSelected(new Map());
                  setReport(null);
                  setOffset(0);
                  setOnlySelected(false);
                })
              }
            >
              {snapshot ? '重新扫描附件' : '开始扫描'}
            </button>
          </div>
          <p className="muted">切换位置或重新扫描会清除当前选择；每次仅整理一个应用的一个账号。</p>
          {provider === 'qqnt' && (
            <p className="muted attachment-location-note">
              这里只扫描账号资料目录。QQNT
              保存在系统“下载”等其他位置的文件暂不包含，可在空间分析中查看。
            </p>
          )}
          {desktop && !discovery && <p role="status">正在识别{providerLabel}资料位置…</p>}
          {discovery && discovery.state !== 'ready' && (
            <p role="status">
              {discovery.state === 'not_installed'
                ? `未检测到${providerLabel}安装。`
                : discovery.state === 'no_accounts'
                  ? `未发现${providerLabel}账号资料，请先登录，或选择经过核验的自定义位置。`
                  : discovery.state === 'unsupported_location'
                    ? '发现的资料位置不受支持，未开放扫描。'
                    : '应用安装身份或布局未通过核验，未开放扫描。'}
            </p>
          )}
          {!!discovery?.issues.length && (
            <details>
              <summary>识别详情（{discovery.issues.length}）</summary>
              {discovery.issues.map((issue, i) => (
                <p key={i}>{issue}</p>
              ))}
            </details>
          )}
          {snapshot && (
            <p className="attachment-scope">
              {snapshot.location.path} · {attachmentProviderLabels[snapshot.location.provider]}{' '}
              {snapshot.location.version}
              <br />
              扫描时发现 {snapshot.files} 个文件 · {formatBytes(snapshot.bytes)}
              {snapshot.state === 'scanning'
                ? ' · 正在扫描'
                : snapshot.state === 'complete'
                  ? ' · 扫描完成'
                  : ' · 扫描不完整，已保留'}
              {snapshot.location.cleanup_block && <span> · {snapshot.location.cleanup_block}</span>}
              {snapshot.issues > 0 && <span> · {snapshot.issues} 项未能读取</span>}
              {snapshot.error && <span> · {snapshot.error}</span>}
            </p>
          )}
          {snapshot?.state === 'scanning' && (
            <button
              className="button secondary small"
              onClick={() => void api.cancelAttachments().catch((e) => setError(String(e)))}
            >
              停止扫描
            </button>
          )}
          {snapshot && snapshot.state !== 'scanning' && !plans && (
            <>
              {page?.categories && (
                <>
                  <p className="muted">
                    分类为扫描时占用；本次删除结果单独列出，当前剩余占用需重新扫描。
                  </p>
                  <div className="attachment-categories" aria-label="附件分类占用">
                    {page.categories.map((category) => (
                      <button
                        key={category.kind}
                        className="attachment-category"
                        disabled={!!busy || onlySelected}
                        aria-pressed={kind === category.kind}
                        onClick={() => {
                          setKind(kind === category.kind ? '' : category.kind);
                          setOffset(0);
                        }}
                      >
                        <strong>{attachmentKindLabels[category.kind]}</strong>
                        <span>
                          {formatBytes(category.bytes)} · {category.files} 个
                        </span>
                        <small>
                          {category.deleted_files === undefined
                            ? '扫描记录不可选'
                            : '已保留或待重新核验'}{' '}
                          {category.retained_files} 个 · {formatBytes(category.retained_bytes)}
                        </small>
                        {category.deleted_files !== undefined && category.deleted_files > 0 && (
                          <small>
                            本次已删除 {category.deleted_files} 个 ·{' '}
                            {category.deleted_bytes === undefined
                              ? '容量未知'
                              : formatBytes(category.deleted_bytes)}
                          </small>
                        )}
                      </button>
                    ))}
                  </div>
                </>
              )}
              {provider === 'weixin' && (
                <p className="muted">
                  仅扫描所选账号的聊天附件，保留聊天数据库、收藏与备份。加密或未知格式只显示占用；图片预览失败不代表文件没有价值。
                </p>
              )}
              <div className="attachment-filters">
                <label>
                  类型
                  <select
                    value={kind}
                    disabled={!!busy || onlySelected}
                    onChange={(e) => {
                      setKind(e.target.value as AttachmentKind | '');
                      setOffset(0);
                    }}
                  >
                    <option value="">全部类型</option>
                    {Object.entries(attachmentKindLabels).map(([key, label]) => (
                      <option key={key} value={key}>
                        {label}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  最后修改时间
                  <select
                    value={days}
                    disabled={!!busy || onlySelected}
                    onChange={(e) => {
                      setDays(Number(e.target.value));
                      setOffset(0);
                    }}
                  >
                    {[30, 90, 180, 0].map((d) => (
                      <option key={d} value={d}>
                        {d ? `超过 ${d} 天` : '全部时间'}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  文件名
                  <input
                    value={search}
                    disabled={!!busy || onlySelected}
                    onChange={(e) => {
                      setSearch(e.target.value);
                      setOffset(0);
                    }}
                    placeholder="搜索文件名"
                  />
                </label>
                <button
                  className="result-link"
                  disabled={!!busy}
                  onClick={() => {
                    setOnlySelected((v) => !v);
                    setOffset(0);
                  }}
                >
                  {onlySelected ? '返回全部附件' : `查看全部已选（${selected.size}）`}
                </button>
              </div>
              {page && !loading && (
                <p role="status" className="attachment-filter-summary">
                  当前筛选扫描记录 {page.total} 个 ·{' '}
                  {page.filtered_bytes === undefined
                    ? '容量未知'
                    : formatBytes(page.filtered_bytes)}
                  {' · '}可选 {page.selectable_files ?? '未知'} 个 ·{' '}
                  {page.selectable_bytes === undefined
                    ? '容量未知'
                    : formatBytes(page.selectable_bytes)}
                  {days > 0 && !onlySelected && (
                    <>
                      。近期文件已计入分类占用；
                      <button
                        className="result-link"
                        disabled={!!busy}
                        onClick={() => {
                          setDays(0);
                          setOffset(0);
                        }}
                      >
                        查看全部时间
                      </button>
                    </>
                  )}
                </p>
              )}
              <div className="inline-actions">
                <button
                  className="result-link"
                  disabled={!!busy || loading || !available.length}
                  onClick={() => select(available, true)}
                >
                  选择本页可选附件
                </button>
                <button
                  className="result-link"
                  disabled={!!busy || !selected.size}
                  onClick={() => setSelected(new Map())}
                >
                  取消全部选择
                </button>
                <span>按大小从大到小 · {page?.total ?? 0} 项</span>
              </div>
              <div className="attachment-content" aria-busy={loading}>
                {loading ? (
                  <p role="status">正在读取附件…</p>
                ) : !page?.entries.length ? (
                  <p>当前筛选下没有附件。可以调整时间或类型。</p>
                ) : (
                  <div className="attachment-grid">
                    {page.entries.map((item) => (
                      <article className={`attachment-card kind-${item.kind}`} key={item.id}>
                        {item.preview_available ? (
                          <Thumbnail
                            item={item}
                            session={snapshot.session}
                            onPreview={setPreview}
                          />
                        ) : (
                          <div className="attachment-file-icon">
                            <File size={28} />
                            <span>
                              {attachmentKindLabels[item.kind]} · {formatBytes(item.bytes)}
                            </span>
                          </div>
                        )}
                        <label className="attachment-choice">
                          <input
                            type="checkbox"
                            aria-label={`选择${item.name}`}
                            checked={selected.has(item.id)}
                            disabled={!!busy || !!item.cleanup_block || !!item.outcome}
                            onChange={(e) => select([item], e.target.checked)}
                          />
                          <strong>{item.name}</strong>
                        </label>
                        <small>
                          最后修改：
                          {item.modified_unix > 0 ? formatTime(item.modified_unix) : '未知'}
                        </small>
                        {item.outcome && (
                          <small role="status">
                            {item.outcome === 'deleted' ? '已删除' : item.outcome}
                          </small>
                        )}
                        {item.cleanup_block && !item.outcome && <small>{item.cleanup_block}</small>}
                        <details>
                          <summary>位置与详情</summary>
                          <p>{item.path}</p>
                          <button
                            className="result-link"
                            disabled={item.outcome === 'deleted'}
                            onClick={() =>
                              void api
                                .revealAttachment(snapshot.session, item.id)
                                .catch((e) => setError(String(e)))
                            }
                          >
                            定位文件
                          </button>
                        </details>
                      </article>
                    ))}
                  </div>
                )}
              </div>
              <div className="attachment-pagination">
                <button
                  className="button secondary small"
                  disabled={!!busy || loading || offset === 0}
                  onClick={() => setOffset((v) => Math.max(0, v - 100))}
                >
                  上一页
                </button>
                <span>第 {Math.floor(offset / 100) + 1} 页</span>
                <button
                  className="button secondary small"
                  disabled={!!busy || loading || offset + 100 >= (page?.total ?? 0)}
                  onClick={() => setOffset((v) => v + 100)}
                >
                  下一页
                </button>
              </div>
            </>
          )}
          {plans && (
            <section className="attachment-confirm" aria-label="确认永久删除附件">
              <h3>永久删除所选附件</h3>
              <p>
                {plans.reduce((n, p) => n + p.plan.files, 0)} 个文件 ·{' '}
                {formatBytes(plans.reduce((n, p) => n + BigInt(p.plan.bytes), 0n))}
              </p>
              <p>
                {providerLabel} · {snapshot?.location.path}
              </p>
              <p>本机对应聊天附件可能无法再次打开，不能保证重新下载。文件不会进入回收站。</p>
              <button
                className="button secondary"
                disabled={!!busy}
                onClick={() =>
                  void action(async () => {
                    await api.cancelAttachmentPlan();
                    setPlans(null);
                  })
                }
              >
                返回选择
              </button>
              <button
                className="button primary"
                disabled={!!busy}
                onClick={() =>
                  void action(async () => {
                    job.current = plans[0].plan.id;
                    await api.execute(plans);
                    setRunning(true);
                    setReport(null);
                  })
                }
              >
                确认永久删除
              </button>
            </section>
          )}
          {report && (
            <div className="attachment-result">
              <p>本次清理结果，以上分类仍为扫描时记录。重新扫描后更新当前占用。</p>
              <ReportRow report={report} rules={[]} />
            </div>
          )}
          <footer className="attachment-footer">
            <strong>
              已选 {selected.size} 个文件 · {formatBytes(selectedBytes)}
            </strong>
            <span>发现量和已选量不等于实际释放空间</span>
            {running ? (
              <button
                className="button secondary"
                onClick={() =>
                  void api
                    .cancel([{ scope: 'user', id: plans?.[0].plan.session_id ?? '' }])
                    .catch((e) => setError(String(e)))
                }
              >
                停止后续删除
              </button>
            ) : pending && preparing.current ? (
              <button
                className="button secondary"
                onClick={() =>
                  void api.cancelPreparation(preparing.current!).catch((e) => setError(String(e)))
                }
              >
                取消核验
              </button>
            ) : (
              !plans && (
                <button
                  className="button primary"
                  disabled={!selected.size || !!busy || snapshot?.state !== 'complete'}
                  onClick={() => void prepare()}
                >
                  清理所选附件
                </button>
              )
            )}
          </footer>
          {preview && (
            <Dialog.Root
              open
              onOpenChange={(open) => {
                if (!open) setPreview('');
              }}
            >
              <Dialog.Portal>
                <Dialog.Overlay className="dialog-overlay" />
                <Dialog.Content className="attachment-image-dialog">
                  <Dialog.Title>图片或视频封面预览</Dialog.Title>
                  <Dialog.Description>预览不会改变选择</Dialog.Description>
                  <img src={preview} alt="附件预览" />
                  <Dialog.Close className="button secondary">关闭预览</Dialog.Close>
                </Dialog.Content>
              </Dialog.Portal>
            </Dialog.Root>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

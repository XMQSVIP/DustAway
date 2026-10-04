import { useMemo, useState } from 'react';
import { manualMixed, manualRoots } from './manualSelection';
import type { AppController } from './useAppController';

export function ManualCart({
  model,
  readonly = false,
}: {
  model: AppController;
  readonly?: boolean;
}) {
  const [search, setSearch] = useState('');
  const [offset, setOffset] = useState(0);
  const roots = useMemo(() => manualRoots(model.manualChoices), [model.manualChoices]);
  const items = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    return roots
      .filter((entry) => `${entry.name}\n${entry.path}`.toLocaleLowerCase().includes(query))
      .sort((a, b) => a.path.localeCompare(b.path, 'zh-CN', { numeric: true }));
  }, [roots, search]);
  const start = Math.min(offset, Math.max(0, Math.floor((items.length - 1) / 200) * 200));
  return (
    <section
      className="manual-cart"
      data-readonly={readonly}
      aria-label={readonly ? '确认自选范围' : '本次自选清单'}
    >
      {!readonly && (
        <div className="manual-cart-heading">
          <strong>你自选的内容 · {roots.length} 个范围</strong>
          {!readonly && (
            <button
              className="result-link"
              disabled={model.busy || model.saving}
              onClick={model.clearManual}
            >
              取消全部自选
            </button>
          )}
        </div>
      )}
      {(!readonly || roots.length > 5) && (
        <label className="manual-cart-search">
          搜索已选名称或路径
          <input
            type="search"
            value={search}
            onChange={(e) => {
              setSearch(e.target.value);
              setOffset(0);
            }}
          />
        </label>
      )}
      <ul className="manual-cart-items">
        {items.slice(start, start + 200).map((entry) => (
          <li key={entry.id}>
            <div>
              <strong>{entry.name}</strong>
              <small>
                {entry.directory ? '文件夹内已选内容' : '文件'}
                {manualMixed(entry, model.manualChoices) ? ' · 部分内容已保留' : ''}
              </small>
              <span className="full-path">{entry.path}</span>
            </div>
            {!readonly && (
              <div className="inline-actions">
                <button
                  className="result-link"
                  disabled={model.busy || model.saving}
                  onClick={() => model.inspectManual(entry)}
                >
                  {entry.directory ? '查看目录内容' : '查看所在目录'}
                </button>
                <button
                  className="result-link"
                  aria-label={`移除${entry.name}`}
                  disabled={model.busy || model.saving}
                  onClick={() => model.selectManual(entry, false)}
                >
                  移除
                </button>
              </div>
            )}
          </li>
        ))}
      </ul>
      {!items.length && (
        <p className="muted">{search ? '没有匹配的已选内容。' : '尚未选择自选内容。'}</p>
      )}
      {items.length > 200 && (
        <div className="pagination">
          <span>
            共 {items.length} 项 · 第 {Math.floor(start / 200) + 1} 页
          </span>
          <button
            className="result-link"
            disabled={start === 0}
            onClick={() => setOffset(start - 200)}
          >
            上一页
          </button>
          <button
            className="result-link"
            disabled={start + 200 >= items.length}
            onClick={() => setOffset(start + 200)}
          >
            下一页
          </button>
        </div>
      )}
      {!readonly && <small>只清理所选内容，文件夹可能保留。</small>}
    </section>
  );
}

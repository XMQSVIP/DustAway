import type { AnalysisDiscoveryQuery, RecognitionCategory } from './types';

export const recognitionCategories: Record<RecognitionCategory, string> = {
  windows: 'Windows 与安装维护',
  development: '开发工具与构建',
  ai: 'AI 模型与工作流',
  virtualization: '虚拟机与子系统',
  games: '游戏与启动器',
  applications: '应用资料与邮件',
  media: '音视频、设计与工程素材',
  documents: '办公文档',
  data: '数据库与数据集',
  archives: '归档与备份',
};
export const defaultDiscovery: AnalysisDiscoveryQuery = {
  kind: 'file',
  category: 'all',
  min_bytes: '0',
  search: '',
  include_nested: false,
  incomplete_only: false,
  offset: 0,
};
export function AnalysisDiscoveryFilters({
  query,
  search,
  onSearch,
  onChange,
}: {
  query: AnalysisDiscoveryQuery;
  search: string;
  onSearch: (search: string) => void;
  onChange: (changes: Partial<AnalysisDiscoveryQuery>) => void;
}) {
  const count =
    Number(query.category !== 'all') +
    Number(query.min_bytes !== '0') +
    Number(query.include_nested) +
    Number(query.incomplete_only);
  return (
    <div className="discovery-filters">
      <div className="file-tools">
        <label>
          搜索本次范围的名称
          <input
            type="search"
            maxLength={256}
            value={search}
            placeholder="搜索已读取的目录或文件名"
            onChange={(e) => onSearch(e.target.value)}
          />
        </label>
      </div>
      <details>
        <summary>筛选{count > 0 ? ` · ${count} 项已应用` : ''}</summary>
        <div className="file-tools">
          <label>
            用途筛选
            <select
              value={query.category}
              onChange={(e) =>
                onChange({ category: e.target.value as AnalysisDiscoveryQuery['category'] })
              }
            >
              <option value="all">不限用途</option>
              <option value="recognized">全部已识别用途</option>
              {Object.entries(recognitionCategories).map(([key, label]) => (
                <option key={key} value={key}>
                  {label}
                </option>
              ))}
              <option value="unclassified">未归类项目</option>
            </select>
          </label>
          <label>
            最小大小
            <select
              value={query.min_bytes}
              onChange={(e) => onChange({ min_bytes: e.target.value })}
            >
              <option value="0">不限大小</option>
              <option value="104857600">至少 100 MiB</option>
              <option value="1073741824">至少 1 GiB</option>
            </select>
          </label>
          {query.kind === 'directory' && (
            <label className="check-label">
              <input
                type="checkbox"
                checked={query.include_nested}
                onChange={(e) => onChange({ include_nested: e.target.checked })}
              />
              包含内部目录
            </label>
          )}
        </div>
        <p className="muted small-text">
          目录默认先显示同一用途的外层项目，名称搜索也遵循“包含内部目录”设置。
          各行可能包含父子目录或共享硬链接，大小不能直接相加。
        </p>
      </details>
      {count > 0 && (
        <button
          className="result-link"
          onClick={() =>
            onChange({
              category: 'all',
              min_bytes: '0',
              include_nested: false,
              incomplete_only: false,
            })
          }
        >
          清除筛选
        </button>
      )}
      {query.incomplete_only && (
        <p className="message info">
          仅查看已发现但未完成的目录。
          <button className="result-link" onClick={() => onChange({ incomplete_only: false })}>
            查看全部统计状态
          </button>
        </p>
      )}
    </div>
  );
}

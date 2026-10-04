// Development-only report of a read-only audit; never part of the cleaning program.
import { readFile, writeFile } from 'node:fs/promises';
const root = 'artifacts/application-coverage-20261002';
const source = process.argv[2] ?? 'host-audit-delivery.json';
const audit = JSON.parse((await readFile(`${root}/${source}`, 'utf8')).replace(/^\uFEFF/, ''));
const catalog = JSON.parse(await readFile('rules/application-cache.json', 'utf8'));
const cell = (v) =>
  String(v ?? '未取得')
    .replaceAll('|', '&#124;')
    .replaceAll('\n', ' ');
const mib = (b) => `${(Number(b) / 1048576).toFixed(2)} MiB`;
const counts = {};
for (const entry of audit.software) counts[entry.conclusion] = (counts[entry.conclusion] ?? 0) + 1;
const lines = [
  '# 本机应用覆盖对照表',
  '',
  '核查日期：2026-10-02。本机只读：扫描未准备清理计划、未提交删除，也未执行任何整理命令。原始输入保留在 `artifacts/application-coverage-20261002/host-audit-delivery.json`。',
  '',
  `现有软件清单返回 **${audit.software.length} 个登记项**（包括桌面软件、商店包和组件，不等于独立应用数量），读取警告 ${audit.inventory_warnings.length} 项。另盘点 Local/Roaming 下 **${audit.data_directories.length} 个一级数据目录**；不搜索任意 Cache 目录来授予许可。`,
  '',
  '安装在其他盘不影响 C 盘数据目录识别。表中“已有规则”只表示同一软件家族有部分固定位置适配；“用途待确认”是明确缺口，不能解读为没有缓存。“新增适配”也可能只有只读能力。未读到容量不估算，不把这些逻辑大小当成可释放空间。',
  '',
  ...Object.entries(counts).map(([key, n]) => `- ${key}：${n} 项。`),
  '',
  '## 本轮新增规则在本机的实际结果',
  '',
  '| 应用 / 规则 | 已读文件（含量化保留） | 已读逻辑大小 | 其中保留 | 许可与本机状态 |',
  '|---|---:|---:|---:|---|',
];
for (const spec of catalog) {
  const row = audit.summary.rules.find((r) => r.rule_id === spec.id);
  if (!row) throw Error(`Missing audit row ${spec.id}`);
  const retained = row.observation?.retained_files ?? 0;
  const total = BigInt(row.bytes) + BigInt(row.observation?.retained_bytes || '0');
  const permit = ['vsix', 'vscode_old_code'].includes(spec.kind)
    ? '按条件主动选择'
    : '仅识别 / 只读';
  lines.push(
    `| ${cell(spec.title)} / \`${spec.id}\` | ${row.files + retained} | ${mib(total)} | ${retained} 文件 / ${mib(row.observation?.retained_bytes || '0')} | ${permit}；${cell(row.availability ?? row.scan_issue ?? row.phase)} |`,
  );
}
lines.push(
  '',
  '更新下载内容与 IDE 系统占用仅供查看。新应用未取得对应版本依据的保持只读。VS Code 候选量也需要用户选择及执行前复核；本机没有进行删除验收。',
  '',
  '## 全部软件登记项',
  '',
  '| 登记名称 | 版本 | 来源 | 覆盖结论 | 新增规则 |',
  '|---|---|---|---|---|',
);
for (const e of audit.software)
  lines.push(
    `| ${cell(e.name)} | ${cell(e.version)} | ${cell(e.source)} | ${cell(e.conclusion)} | ${cell(e.new_rules.join(', ') || '—')} |`,
  );
lines.push(
  '',
  '## 全部一级应用数据目录',
  '',
  '| 位置 | 目录 | 结论 | 新增规则 |',
  '|---|---|---|---|',
);
for (const e of audit.data_directories)
  lines.push(
    `| ${e.folder === 'local' ? '%LOCALAPPDATA%' : '%APPDATA%'} | ${cell(e.name)} | ${cell(e.conclusion)} | ${cell(e.new_rules.join(', ') || '—')} |`,
  );
await writeFile('docs/application-coverage-audit.md', lines.join('\n') + '\n');
console.log(
  JSON.stringify({
    registrations: audit.software.length,
    data_directories: audit.data_directories.length,
    conclusions: counts,
  }),
);

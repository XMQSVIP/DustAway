# 未清理说明与 uv 空缓存修正

日期：2026-10-03。版本：0.18.0，测试目录后缀 `result-clarity-test`。

## 修改范围

- 原“已保留 · N 个清理项”改为“本次未清理 · N 类”，分类汇总说明数量口径。显示运行中、需重新检查、未开放等原因分布，各规则行说明下一步；展开不改变选择。
- 未勾选的可选文件显示“未选择”；执行回执分别统计文件与文件夹，跳过明确表示未删除。失败与不确定项不合并成“保留”，历史原因及当前文件明细显示后续处理建议。
- uv 只读发现时先核验目录并判断是否为空。空目录返回 `cache_missing`，界面隐藏该项，单项刷新后也不留下空项目标题或提示。
- 有内容但缺少或含无效 `CACHEDIR.TAG` 返回明确的布局限制，不补写标记、不跳过目录核验；读取失败不能冒充空目录。
- 单项检查失败不再同时显示“没有符合当前规则的文件”；部分读取仍可查看原因和重新检查。空卡片消失时键盘焦点回到剩余工具标题或清理结果区域。
- 通用路径保护、扫描选择、删除授权、一次性计划和历史格式均未改变；不执行用户缓存删除。

## 本机复现与验证

开发电脑 uv 0.7.5 的默认缓存目录存在但为空，`CACHEDIR.TAG` 不存在。修复前的实际只读扫描稳定返回“文件或父目录已变化，请重新扫描”，且候选数为零；修复后返回 `cache_missing`，界面按空结果隐藏。此判断不依赖版本白名单。

- 前后只读证据：`artifacts/uv-inspection-before.json`、`artifacts/uv-inspection-after.json`；均记录 `deletion_attempted: false`。
- Rust uv 测试：7 项通过，1 项需要显式执行真实工具整理的测试保持忽略。覆盖空目录、未标记内容、无效/有效标记、取消及旧计划保护；`artifacts/uv-cache-fix-tests.log`。
- 前端相关测试：150 项通过；覆盖类别与文件数区别、未清理原因、保留选择、空结果隐藏、部分读取、失败不误报为空及焦点回退；`artifacts/uncleared-uv-frontend-final.log`。
- 原生界面检查通过：`scripts/uncleared-desktop-check.mjs` 使用独立、带标记的 debug 夹具，验证 1100 与 760 像素窗口下的键盘展开、原因提示和勾选保持，只扫描、不准备或执行删除；结果保存在 `artifacts/uncleared-desktop-check/`。
- 构建及静态检查日志：`artifacts/uncleared-uv-build.log`、`artifacts/uncleared-uv-clippy.log`、`artifacts/uncleared-uv-release-build.log`。

不重放此前微信、QQ 或 uv 的真实删除验收，不修改用户设置或操作其他应用。交付为独立免安装目录，不生成压缩包。

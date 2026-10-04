# 0.18 整理台界面截图

这些图片来自本轮实际桌面程序，不是设计稿。发布版仅作只读界面检查；扫描、清理和大量文件截图来自独立隔离夹具，不使用真实个人文件作为删除样本。完整验证范围与限制见[验收记录](verification-0.18.md)。

## 发布版四页

首页保留唯一扫描入口，磁盘空间和范围说明退到辅助位置。

![清理首页，浅色](../artifacts/desk-release-0.18.0/clean-light-1100.png)

空间分析先选择范围，再主动开始只读分析。

![空间分析，深色](../artifacts/desk-release-0.18.0/analysis-dark-1100.png)

历史保留回执；没有记录时明确呈现空状态。

![清理历史](../artifacts/desk-release-0.18.0/history-light-1100.png)

设置使用侧边索引和同一整理台的外观缩略预览。

![设置](../artifacts/desk-release-0.18.0/settings-dark-1100.png)

发布版截图通过视觉覆盖分别检查深浅主题，未保存用户设置；设置中的主题选择仍保留原值。另有[最小窗口首页](../artifacts/desk-release-0.18.0/clean-light-760.png)及[最小窗口设置](../artifacts/desk-release-0.18.0/settings-dark-760.png)。

## 隔离任务状态

扫描结果只展示实际发现，文件明细向侧边展开，清理只需一张永久删除确认单。

![扫描结果](../artifacts/discovery-0.18.0/review-1100.png)

![文件明细](../artifacts/desktop-details-light-0.18.0.png)

![唯一删除确认单](../artifacts/discovery-0.18.0/confirmation.png)

分析默认查询全范围大文件，也可切换大目录并按用途筛选。以下排行截图处于“大目录／开发工具与缓存”筛选；历史展开实际删除回执，部分结果保留核对入口。

![全范围目录排行与用途筛选](../artifacts/discovery-0.18.0/discovery-light-1100.png)

![大文件的用途与路径侧页](../artifacts/largest-files-0.18.0.png)

![展开清理回执](../artifacts/desktop-e2e-0.18.0.png)

![部分处理结果，最小窗口](../artifacts/discovery-0.18.0/partial-dark-reduced-760.png)

图片位于本地 `artifacts/` 中，源码仓库不会携带这些生成产物；其他环境可按[测试指南](testing.md)重新生成。

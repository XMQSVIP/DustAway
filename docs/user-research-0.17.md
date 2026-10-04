# 0.17：帮用户找全，再让用户决定删除

研究日期：2026-09-28。依据为项目源码、公开产品资料与任务场景分析；本轮没有真实参与者访谈，不将推断称为用户研究实测结论。

## 用户视角

小白用户不熟悉缓存名称，扫描前选类别会漏掉垃圾；原先同一组选中状态同时控制扫描和清理，取消删除会影响下次发现。现在默认检查全部未排除的普通用户规则，扫描不删除。用户只需对结果作决定。

用户需要能直接回答：扫过哪些位置、哪些可以选、删除有什么影响、实际腾出多少空间。空间册保留，增加跨册的建议清理、需要你确认、暂不可清理入口；容量区分扫描发现、当前已选、实际删除与磁盘空间变化。未发现标准位置不推断应用一定没安装；未读取不算零垃圾。

日志和崩溃转储可能是排障资料，图形缓存可能影响下次运行速度。因此三条 Windows 残留规则默认不选，且在真实删除验收完成前只读。查看明细、切换分册、刷新应用状态都不授权清理。

## 产品经理视角

成功标准是完成一次可理解的清理，不追求规则数量或更大的垃圾数字。主流程为全面扫描、核对结果、一次确认、清理回执。保留扫描与删除解耦、长期排除、单文件选择、统一容量计算和后端不可绕过的删除边界。

本轮明确采用用户选择：保留空间册；扫描支持的常见位置；风险项目手动选择；统一确认一次；优先扩展 Windows 常见残留。全盘占用继续交给空间分析，个人资料和 Windows 系统维护内容不作为自动垃圾。

## 竞品参考与使用边界

- [Windows 清理建议](https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/free-up-drive-space-in-windows)：分类查看临时文件、大文件、云文件与应用，用户选择后清理。借鉴分类决策，不将所有占用合为垃圾。
- [CCleaner 分析与结果](https://support.ccleaner.com/articles/en_US/Master_Article/custom-clean-s-analysis-and-results-function)：按应用、文件数和大小展示，并提供文件明细。借鉴结果透明和选择控制。
- [微软电脑管家](https://pcmanager.microsoft.com/en-us)：清理与大文件管理并存。借鉴清理和空间管理的不同入口。
- [用户态转储](https://learn.microsoft.com/en-us/windows/win32/wer/collecting-user-mode-dumps)及 [DirectX 缓存](https://microsoft.github.io/DirectX-Specs/d3d/ShaderCache.html)：用于确定文件用途；不等于对所有本机布局做过删除验收。

交互采用 UI/UX Pro Max 中的破坏性操作统一确认、结果反馈和渐进展开建议，不重新设计主题。验证见 [验收记录](verification-0.17.md)，参与者任务见 [可用性验证](usability-0.17.md)。

# 0.5 需求研究与产品决策

研究日期：2026-09-26。方法为公开用户讨论、开源软件问题记录、官方技术文档与本项目实现对照。没有进行一手用户访谈，也没有足够样本计算需求占比；帖子中的容量与因果解释属于发帖者自述。

| 观察 | 来源与时间 | 局限 | 本轮决策 |
| --- | --- | --- | --- |
| 用户希望看到“临时文件”分类下具体是什么占用了空间 | [WindowsHelp 用户讨论](https://www.reddit.com/r/WindowsHelp/comments/1cfqlff/)，2024 | 单个求助，不能代表所有 Windows 用户 | 单独提供只读目录排行、逐层查看与定位 |
| 微信升级后有人反馈 C 盘告急；同帖对于迁移、已有设置和硬链接有不同观察 | [V2EX：微信 4.0 升级讨论](https://global.v2ex.com/t/1159678)，2025-09-16 | 未独立复现各版本行为，不把目录相加当作物理占用证据 | 支持用户目录与自选目录；识别硬链接共享，聊天内容不加入删除规则 |
| 用户希望保留特定聊天图片，另有人希望按联系人设置白名单 | [V2EX：CleanMyWechat 讨论](https://v2ex.com/t/715316)，2020-10-15 | 较早的讨论，只用于识别“个人资料不能统一判为垃圾”的需求 | 分析文件时解释用途由用户核对，推荐使用应用内存储管理 |
| 删除量与磁盘可用空间变化不一致，用户难以判断清理是否有效 | [BleachBit issue 1026](https://github.com/bleachbit/bleachbit/issues/1026)，2020-10-02 | 旧版本的用户报告，未据此推断当前软件仍有同样缺陷 | 删除逻辑大小与各受影响卷的空间采样分开呈现，保留负数和采样失败 |
| 用户反馈临时文件删除后再次出现，并提到视频剪辑和 3D 工作流程 | [WindowsHelp 求助](https://www.reddit.com/r/WindowsHelp/comments/1rvq2dg/my_temporary_files_take_up_so_much_space_and/)，2026 | 不能从该帖确认实际来源，不采纳评论中的手工删除系统目录建议 | 清理后提示缓存可能重新生成，通过目录分析查找来源，不承诺持续加速 |

官方依据：[Chrome 文档](https://support.google.com/chrome/answer/2392709?co=GENIE.Platform%3DDesktop&hl=en)区分缓存、历史及网站数据；[Windows 存储建议](https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/free-up-drive-space-in-windows)提供系统管理入口；[GetDiskFreeSpaceExW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdiskfreespaceexw)区分磁盘总空闲与当前用户可用空间。拂尘使用当前用户可用空间，配额和其他应用活动都会影响其变化。

## 与现有实现的对照

0.4 已有 8 条规则、占用检测、永久删除确认、排除与故障恢复。缺口主要是范围之外的空间定位、清理结果的下一步和多卷统计。本轮沿用其删除边界，不把“可见”“大”“很久未修改”视为删除许可。

## 本轮验收对应

1. 目录树中找到指定的大目录并定位文件，不修改内容。
2. 占用文件可以统计，无法访问的区域明确显示为统计不完整。
3. 整体逻辑大小按卷和文件身份去重；目录行仍是文件条目合计，标记硬链接共享。
4. 分析节点无法用作清理候选或计划；切换到缓存清理仍需独立扫描和确认。
5. 报告按实际涉及卷显示采样，旧报告保留旧口径。

Windows 11、屏幕阅读器、真实云文件及真实多卷情况的完成状态以验收记录为准。自动测试和公开帖子不能替代目标用户试用。

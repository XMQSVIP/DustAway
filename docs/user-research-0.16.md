# 0.16 双视角研究：先找到值得核对的占用

资料核对日期：2026-09-28。依据为代码、专用夹具、既有验收与公开资料；没有真实参与者记录，不能将以下判断称为用户试用结论。

## 用户视角

0.15 已提供具体建议和单项目复核，但找到项目之前仍需逐层翻目录或猜文件名。单个文件排行不能代替目录分析：项目依赖可能由大量小文件组成，npm 官方也明确其按项目目录组织。[npm 目录说明](https://docs.npmjs.com/cli/v11/configuring-npm/folders/)

本轮让用户按模型、开发工具、办公资料等可能用途跨目录查找，分别查看目录和文件。默认展示外层用途目录，内部仍可展开；未归类项目有独立入口，不被解释为没有占用。

用户看到排行后仍可能担心资料损失，清理后也可能继续遇到更新受阻。[判断困难案例](https://www.reddit.com/r/WindowsHelp/comments/1h9jl9j/can_anyone_spot_the_obvious_eliminations_to_free/) · [清理后仍不足案例](https://www.reddit.com/r/WindowsHelp/comments/1m6sf3o/very_low_disk_space_after_cleaning/)。这些仅说明存在相应情境，不推断需求比例，不采用评论中的删除指令。

目录上下级可能包含同一内容，硬链接可指向同一文件；不能将各条大小简单相加为可释放量。[Microsoft 硬链接说明](https://learn.microsoft.com/en-us/windows/win32/fileio/hard-links-and-junctions)

## 产品经理视角

用户已选择：优先更快找到值得查看的占用，目录与大文件一起覆盖，用途筛选在结束或主动停止后开放。成功标准是找到相关项目、理解保留要求并进入复核，不以用途规则数量或更大的容量数字衡量。

三项取舍：筛选在后端覆盖全部已读索引，不能只过滤当前200项；不估算未读位置，不将部分结果说成完整；保持既有清理授权边界，查看、筛选与用途识别均不授权删除。处理后回到保存、安装等原任务验证，符合逐步整理并核对空间的路径。[Windows 官方说明](https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/free-up-drive-space-in-windows)

界面沿用空间册。遵循 UI/UX Pro Max 中已核对的空结果恢复、筛选集合可访问与返回状态保留建议；无需重新设计整套主题。工程结果见[验收记录](verification-0.16.md)，真实试用见[任务与记录表](usability-0.16.md)。

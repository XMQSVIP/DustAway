# 0.15 双视角研究与取舍

资料核对：2026-09-28。依据是源码、既有验收截图、公开求助和官方说明，不是真实参与者试用结论。

## 用户视角

用户真正需要的是解决保存、安装、更新受阻，并保留仍有用途的资料。公开求助者已经使用空间排行，仍担心处理 Chrome 等目录导致数据丢失；另有用户清过缓存仍无法更新系统。[排行后的判断困难](https://www.reddit.com/r/WindowsHelp/comments/1h9jl9j/can_anyone_spot_the_obvious_eliminations_to_free/) · [清理后仍不足](https://www.reddit.com/r/WindowsHelp/comments/1m6sf3o/very_low_disk_space_after_cleaning/)。这些只作为需求案例，不采用评论中的删除建议，不推断需求比例。

源码中存在可复现的断点：具体用途的 guidance 藏在折叠区，主操作主要按所在目录决定；浏览器指引未复用清理页的独立内容。外部处理后，重新读取索引也容易被理解为重新分析。0.15 将下一步直接显示，把完整建议、应用指引和复核集中到列表外面板，保留当前项目及返回位置。

浏览器缓存、Cookie、登录和其他数据有不同影响，指引必须按类型核对，不能建议全选。[Chrome 官方说明](https://support.google.com/chrome/answer/2392709?co=GENIE.Platform%3DDesktop&hl=zh-Hans)。模型缓存可能跨库共用；虚拟磁盘与容器卷也可能保存工作数据，不能由名称或体积直接获得删除资格。[Hugging Face 缓存](https://huggingface.co/docs/huggingface_hub/guides/manage-cache) · [Docker 备份说明](https://docs.docker.com/desktop/settings-and-maintenance/backup-and-restore/)。本轮只复用已有专属建议，不执行这些工具的清理命令。

## 产品经理视角

用户选择“大占用处理衔接”和“逐项处理并返回复核”。本轮衡量能否找到适合的下一步、理解保留要求、回到原任务验证，不用规则数量或展示容量代替完成情况。[Windows 官方建议](https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/free-up-drive-space-in-windows)区分临时文件、应用和个人文件，并建议每一步后检查空间是否足够。

只保留一个内存中的核对项目，不建立跨次待办。打开指引不是执行成功，磁盘变化不是项目释放量，读取索引不是重新分析。结束本项不写入清理报告。现有20条清理规则和六条只读标记不变；190条用途识别仅说明可能用途。

## 验证边界

实现前相关22项前端测试通过，只覆盖既有分析与识别行为。完整本轮结果见 [验收记录](verification-0.15.md)。五名非开发者试用尚无参与记录，继续标记人工可用性待验证。

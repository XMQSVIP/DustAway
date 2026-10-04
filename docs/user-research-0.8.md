# 拂尘 0.8：用户需求与产品判断

整理与访问日期：2026-09-26（America/New_York）。研究对象是公开用户个案和官方文档；没有开展真人访谈，也不将自动化桌面走查当作真人可用性研究。以下转述不代表发生比例，评论中的删除建议不直接进入规则。

| 来源、日期与版本 | 真实用户陈述（转述） | 工程观察 | 产品经理推断与落地 | 局限 |
| --- | --- | --- | --- | --- |
| [Windows 空间求助](https://www.reddit.com/r/WindowsHelp/comments/1ndm89f/)，2025-09-10；用户报 Windows 11 Home 24H2、26100.4946 | 系统显示大量应用占用，用户难以找到对应文件；清理收益很小，很快又满，担心第三方工具损害文件 | 系统分类与所选目录遍历口径不同；目录逐层展开增加寻找深层文件的步骤 | 在现有只读索引增加全范围最大文件排行，并显示所选范围，避免承诺解释整盘占用 | 单个自行描述的环境，无磁盘镜像，不能确认成因或衡量拂尘性能 |
| [AppData 误删求助](https://www.reddit.com/r/WindowsHelp/comments/1iuvv6y/)，2025-02-21；Windows 10，应用版本不详 | 用户删除 AppData 后应用资料异常，恢复文件也未能顺利恢复使用 | 目录名不足以区分可重建缓存与数据库、设置、用户成果 | 后端基于 Known Folders 与受支持浏览器标准位置说明用途；配置根与缓存叶目录分开，不提供大文件删除 | 无法核实完整删除过程，不推断所有 AppData 内容相同，也不引用评论作为安全依据 |
| [微信目录讨论](https://fast.v2ex.com/t/1151987)，2025-08-13；讨论混有 macOS、Windows 与不同微信版本 | 新旧目录同时存在，用户不确定是否可以删除其中一份 | 同名或并存不能证明重复、迁移完成或有完整备份 | 不识别微信专属目录、不自动去重；在分析页主动提供通用原应用管理指引，菜单未经逐版本核对 | 跨系统个案不能验证 Windows 清理位置，不能将讨论中的路径直接设为规则 |
| [微软释放空间指引](https://support.microsoft.com/en-us/windows/experience/storage-filemanagement/free-up-drive-space-in-windows)，访问日版本；Windows 10/11 支持文章 | 官方建议逐项处理后检查空间是否足够 | 用户最终目标通常是完成更新、安装或工作，而不是让“垃圾”数字归零 | 展示当前用户可用磁盘空间与查询时间，提供存储设置及应用管理入口；不相减生成“其他垃圾” | 属官方产品指导而非用户样本；不同 Windows 版本的页面可能不同 |

## 从需求到实现边界

用户需要先找到足够大的占用，再理解是否值得处理。0.8 保留清理首页，在顶部提供诊断入口；最大文件视图跨层查询，每页 200 项，允许文件名搜索、定位和返回所在目录。排行只来自本次已读取文件，部分、取消或失败不会被称为完整扫描。共享硬链接仍保留提示，排行大小不是预计释放量。

应用专属说明仅识别现有四款 Chromium 浏览器和 Firefox 标准数据位置；其他 AppData 只显示通用应用数据。个人资料使用系统登记的下载、文档、桌面、图片、音乐、视频位置，支持登记路径重定向；混合的用户根目录不整体归类。系统目录使用 Windows、Program Files 和 ProgramData。未知位置明确显示未知，不按 WeChat、AppData、Cache 等名称猜测。

已有规则覆盖的位置可以继续清理扫描，但用途说明不代替候选校验。仍需主动扫描、核对计划和确认；保留 14 条规则，不增加聊天附件、大文件、备份、迁移或自动清理。

## 技术依据与解释限度

- [Microsoft Known Folder ID](https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid)：使用系统登记的位置标识，避免硬编码个人目录路径。登记信息用于说明用途，不验证某个应用已经安装。重定向映射测试不等于实际更改用户的 Known Folders。
- [GetDiskFreeSpaceExW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdiskfreespaceexw)：取可供调用用户使用的字节，可能受配额影响。与目录逻辑大小分别显示；失败为未知而不是零。
- [Windows Settings URI](https://learn.microsoft.com/en-us/windows/apps/develop/launch/launch-settings)：应用管理固定使用 `ms-settings:appsfeatures`。系统版本及策略可能影响是否能打开，失败时提示从开始菜单进入设置。
- [Brave 敏感数据说明](https://support.brave.app/hc/en-us/articles/29808985123085-Sensitive-data-storage)：浏览器数据根含敏感配置，不能将整个 User Data 视作缓存。具体清理仍限既有规则叶目录。

## 验证方法

自动化任务走查模拟“首页进入诊断 → 最大文件搜索 → 看用途 → 返回目录并定位 → 完成既有缓存清理”。这是流程与保护数据验证，不是对用户理解程度的实测。实际测试数值、版本和未执行环境见[验收记录](verification.md)；位置覆盖见[用途识别矩阵](support-matrix-0.8.md)。后续真人研究应观察是否能正确解释“未知”“不完整”“共享数据”和“可进一步扫描”，不以自动化脚本通过代替这些结论。

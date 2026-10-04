# 0.6 需求研究与产品决策

研究日期：2026-09-26。方法：公开求助、工具作者说明、官方文档、0.5 源码及验收证据对照。没有开展用户访谈，也没有可用于推断比例的代表性样本。容量和因果描述保留为来源自述，不将评论里的手工删除建议直接写入规则。

| 观察 | 来源、时间与性质 | 局限 | 决策 |
|---|---|---|---|
| 用户看到微信两个目录，询问能否删除一份 | [V2EX 1151987](https://fast.v2ex.com/t/1151987)，2025-08-13，用户求助 | 未复现版本迁移与物理占用；不能由目录大小相加得出释放量 | 保留 0.5 硬链接提示和分析能力，不自动删旧目录 |
| “缓存”清理脚本实际删除 QQ 图片和短视频 | [工具作者仓库](https://github.com/Tsuki0125/Clean-cache-of-newQQnt)，说明适配至 2024-10-01 的 QQ 9.9.15 | 是工具作者对自身删除范围的说明，不是 QQ 安全删除契约 | 聊天附件不因被称为缓存而加入规则 |
| WPS 用户发现 data 与插件 pool 占用较大 | [WPS 社区用户帖](https://forum.wps.cn/topic/90864)，2026-06-22 | 官方社区中的用户文章不等于官方技术保证；其建议包含备份和插件 | 只给应用内管理指引，保护恢复文件、离线文档与插件 |
| 清理后临时文件重新增长，用户难以持久释放空间 | [WindowsHelp](https://www.reddit.com/r/WindowsHelp/comments/1l61bjd/)，2025-06-08，用户求助 | 无法仅凭帖子确定写入进程或因果 | 继续说明缓存可重建；增长诊断列入后续，不承诺长期加速 |
| 用户不敢删除占用中的临时文件 | [WindowsHelp](https://www.reddit.com/r/WindowsHelp/comments/1jvq1bs/)，2025-04-10 | 与系统更新的关系是用户推测 | 保留占用与进程检查，不强制结束应用 |
| 用户误以为清除网页缓存会清除全部 Service Worker 存储 | [Brave issue 47275](https://github.com/brave/brave-browser/issues/47275)，2025-06-30，项目问题记录 | 原报告涉及 Brave 1.80.113；不能推断所有缓存种类都可安全删除 | 只支持指定网页、代码、图形缓存；Service Worker 留在保护范围 |

## 官方边界与应用选择

- [Brave 数据说明](https://support.brave.com/hc/en-us/articles/4413256282765-How-do-I-delete-my-data-in-Brave)区分各类浏览数据，并明确整份用户目录包含钱包及 Rewards。本文不采用其整目录重置操作。
- [Vivaldi 数据分类](https://help.vivaldi.com/desktop/tools/delete-browsing-data/)分别列出缓存、Cookies、Storage、密码等；[标准配置位置](https://help.vivaldi.com/desktop/privacy/preventing-vivaldi-profiles-from-being-uploaded-to-git-repositories/)提供 Windows 路径依据。
- [Chromium 用户目录文档](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/user_data_dir.md)解释配置与缓存位置，以及自定义目录覆盖。具体三个缓存叶目录还须以真实产品生成和重建验证，不仅依据同源内核推断。
- [Microsoft Teams 说明](https://support.microsoft.com/en-us/teams/troubleshooting/i-can-t-reconnect-to-microsoft-teams)指出其整目录清理会移除本地消息历史等，因此本轮不将此操作包装为普通缓存规则。

用户选择优先扩展清理覆盖、关注微信/QQ/WPS，随后选择在缺乏安全边界证据时改选能直接清理的应用。因此本轮限定为 Brave 与 Vivaldi 六条默认关闭的规则，并保留国内三款的研究与通用指引。选择不表示其用户量或清理收益高于其他应用。

## 本地证据与待办

0.5 的 91 个源文件哈希在实施前全部匹配归档；Rust 64 项和前端 33 项完成复核。前端先出现 worker 启动超时，重任务结束后的默认模式全量通过。0.5 实际临时目录扫描约 30.7 万项耗时超过 266 秒；本轮不以扩大规则掩盖这一已知问题，继续保留性能优化待办。

程序只使用内置规则，不联网抓取这些帖子，也不读取聊天正文。真实浏览器验收使用官方程序与隔离资料目录，准确版本、通过状态和未执行项目见本轮验收记录与支持矩阵。

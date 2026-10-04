# 0.13：结果核对与结束判断

日期：2026-09-27。依据源码、专用夹具和公开官方说明；没有新增真实用户访谈或五人试用记录。以下是产品判断与待检验假设，不代表需求比例。

## 用户角度

清理之后看见一个异常数字，用户需要找到对应文件，而不是先在完整报告里重新寻找类别。单类别直达，跨类别先列出问题最多的类别，减少重复判断；关闭后留在原入口，便于逐项核对。

“跳过”有两种不同的含义。按规则留下近期文件、保护对象或排除对象时，用户可以结束处理；占用、文件变化、无法验证或原因缺失仍需要解释。扫描期间被保留的文件没有进入计划，不能拿来解释执行中跳过的项目。每个文件提供下一步，同时保留原始原因，便于理解建议的依据。

用户还需要知道外部应用能做什么：浏览器指引聚焦缓存复核；微信、QQ、WPS 保持已知版本边界和资料保留说明。相应应用只显示自己的步骤，未知应用不猜菜单。

## 产品角度

优先消除歧义，不增加删除能力。以同一纯前端模型计算优先问题、正常保留和可访问的类别目标；总览、底栏、报告共用。正常保留采取保守判定：正常结束、计数一致、无失败/不确定/未处理，且全部执行跳过原因完整属于已知四类；不满足即继续核对。

入口绑定报告、scope、扫描会话、类别和状态，复用 `list_candidates` 的查询、搜索与分页。失效只保留统计；新目标重置搜索和分页，旧请求返回不覆盖新内容。不会跨类别预读文件，也不会重放删除计划。

本轮没有目标容量、自动清理、强制退出、后台任务、遥测、权限扩展或数据库迁移。维持 20 条规则，微信、QQ、WPS 六条只读。成功标准是理解问题、知道能否结束，并回到原工作验证；清理字节数不能单独衡量成功。

## 离线指引来源

以下官方桌面说明在 2026-09-27 核对。界面使用简要离线步骤，不依赖联网；说明缓存与其他浏览数据是独立选项，不建议全选。菜单随版本、语言变化，实际以应用当前界面为准。

| 应用 | 官方来源 | 采用的边界 |
| --- | --- | --- |
| Chrome | [删除浏览数据](https://support.google.com/chrome/answer/2392709?co=GENIE.Platform%3DDesktop&hl=zh-Hans) | 更多 → 删除浏览数据；只核对缓存图片与文件，保留登录、历史和其他资料 |
| Edge | [查看和删除浏览器历史记录](https://support.microsoft.com/en-US/edge/view-and-delete-browser-history-in-microsoft-edge) | 设置中选择要清除的内容；区分缓存、密码、Cookie 与同步范围 |
| Firefox | [清除 Firefox 缓存](https://support.mozilla.org/en-US/kb/how-clear-firefox-cache) | 仅临时缓存文件与页面；不同时清除 Cookie、站点数据和历史 |
| Brave | [清除 Cookie 与站点数据的选项说明](https://support.brave.app/hc/en-us/articles/360048833872-How-Do-I-Clear-Cookies-And-Site-Data-In-Brave)、[官方快捷键](https://support.brave.com/hc/en-us/articles/360032272171-What-keyboard-shortcuts-can-I-use-in-Brave)、[数据删除说明](https://support.brave.com/hc/en-us/articles/4413256282765-How-do-I-delete-my-data-in-Brave) | Ctrl+Shift+Delete 进入后仅核对缓存；保留钱包、Rewards、登录及扩展数据 |
| Vivaldi | [删除浏览数据](https://help.vivaldi.com/desktop/tools/delete-browsing-data/) | 工具 → 删除浏览数据；Cache 与 Storage / Application Cache 分开，不扩展到邮件、日历或订阅 |

微信、经典 QQ、WPS 延续 [0.9 研究](user-research-0.9.md) 与 [支持矩阵](support-matrix-0.9.md) 的来源和版本限制。本轮没有声称重新实测这些应用菜单。微信 4.x 指引不套用于 3.x；QQNT 不套经典 QQ 规则；WPS 2026 年 7 月入口不推断为旧版 12.8.2.18205 的菜单。日志不能承诺删除后重建历史记录。

## 验证方式

行为与工程证据见 [验收记录](verification-0.13.md)。人工新增“正常保留后结束”“跨类别异常核对”任务，沿用五名非开发者方案，记录无提示完成、求助、误解和无效重扫。目标至少四人无提示完成全流程且所有参与者分清只读与可删除；尚无实际参与记录，**人工可用性待验证**。

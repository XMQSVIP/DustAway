# 拂尘 0.6.0 验收记录

验证日期：2026-09-26（America/New_York）；主机使用 UTC+08:00。Windows 10 专业版 22H2（19045）x64，普通权限进程；Rust 1.97.1 MSVC、Node.js 24.12.0、pnpm 10.30.3，WebView2 / EdgeDriver 153.0.4234.48。

本轮新增 Brave / Vivaldi 六条默认关闭的缓存规则，普通用户规则共 14 条；增加微信、QQ、WPS 可展开指引，修复空间分析目录选择的结构化错误显示。删除边界、显式确认、任务协调和多卷报告沿用 0.5。真实删除只在新建、带标记的隔离夹具和真实浏览器测试配置执行；发行程序只做启动、只读扫描和无效请求拒绝检查。

## 固定基线与自动检查

实施前 0.5 的 91 个源文件与已有逐文件 SHA-256 清单全部匹配；归档为 [baseline-0.5-fixed.zip](../artifacts/baseline-0.5-fixed.zip)，哈希 `B5847EE76738CE2062AC5B36CB0C8621EEB0EE007AED87EC723BC4939FCE159A`。随后 Rust 64 项、前端 33 项全量通过，见 [基线复核](../artifacts/baseline-0.5-validation.json)。[0.5 原验收](verification-0.5.md) 保留原文。

与归档对照，Cargo.lock 的 508 条外部包记录（名称、版本、来源与校验值）及 pnpm-lock.yaml 内容均未变化；Cargo 本地包版本随本轮更新为 0.6.0。

| 检查 | 0.6 最终结果 |
|---|---|
| cargo test --workspace --locked | 66 项通过；另 1 项真实浏览器桥接测试默认忽略，由专项脚本实际执行 |
| cargo fmt --all --check | 通过 |
| cargo clippy --workspace --all-targets -- -D warnings | 通过 |
| cargo check -p dustaway --features system-cleanup --locked | 通过；仅类型检查，不代表 UAC 验收 |
| pnpm test --maxWorkers=1 | 最后固定源码后 6 个文件、38 项通过，65.12 秒 |
| pnpm build | TypeScript 与生产资源构建通过 |
| Prettier | src、scripts 和配置通过 |
| pnpm notices | 465 项依赖、0 项待审查，1 项已有记录的仅开发依赖许可文本例外 |

证据：[Rust](../artifacts/tests-rust-0.6.log)、[前端](../artifacts/tests-frontend-0.6.log)、[Clippy](../artifacts/clippy-0.6.log)、[格式](../artifacts/format-0.6.log)、[构建](../artifacts/build-frontend-0.6.log)、[系统功能类型检查](../artifacts/check-system-0.6.log)。

新增与扩展检查覆盖四种 Chromium 产品的 Default / Profile N 和配置标记、拒绝非标准布局、六条新规则默认关闭、混合规则/目录/进程描述拒绝、逐缓存叶目录扫描与排除、硬链接和扫描后文件变化保留、所有保护文件的内容与修改时间不变。现有公共文件系统测试继续覆盖中文长路径、占用、Junction、目录替换和身份变化；分析测试包含真实拒绝访问 ACL、离线属性、取消与上限；公共引擎测试包含后台进程、进程检测失败、伪造计划、取消和历史兼容。这些公共检查不等于在每个新浏览器版本上分别重复全部故障组合。

前端新增检查涵盖首次启动 8 条默认选择、升级保留旧类别/排除且新规则关闭、Brave/Vivaldi 分析跳转不自动扫描、目录选择失败显示后端说明和下一步、取消选择保留原位置、按应用合并后台进程提示及三款国内应用指引。已有取消、迟到响应、页面重载和明细错误恢复继续通过。

发布前复核还移除分类行里重复的浏览器运行说明，统一由处理建议按应用显示；每类仍保留“暂不可清理”，扫描失败时保留具体错误。该改动后固定源码重新完成上述 38 项全量复跑。

期间 worker 曾在其他重任务运行时启动超时，这些批次不计为通过。完整复跑还发现原有“重新检查应用状态”测试的模拟后端在随后轮询中返回旧阻塞状态；修正测试夹具使其与真实后端一致，最终全量 38 项通过，没有放宽生产状态检查。

## 真实浏览器专项验收

使用官方下载的稳定版程序，提取到本项目验收目录，无系统安装或默认浏览器变更。测试脚本新建临时目录，使用本地测试网站、独立配置及 headless 模式；不读取或删除日常浏览器配置。实际 WindowsPlatform 和 Engine 完成扫描、计划和删除，并拒绝伪造计划。桥接模块仅在 Rust test 构建存在，不进入交付程序。

| 项目 | Brave | Vivaldi |
|---|---|---|
| 产品版本 | 1.96.59，Chromium 154.0.8037.58 | 8.2.4133.76 |
| Windows EXE ProductVersion | 154.1.96.59 | 8.2.4133.76 |
| Authenticode | Valid / Brave Software, Inc. | Valid / Vivaldi Technologies AS |
| 三类缓存删除文件 | 18 | 25 |
| 删除逻辑字节 | 3,150,980 | 9,414,039 |
| 重启后重建缓存文件 | 16 | 17 |
| 非目标文件内容与修改时间保持不变 | 232 个 | 472 个 |
| 测试 Cookie、Local Storage、书签保留 | 通过 | 通过 |
| 浏览器运行中拒绝计划 | 通过 | 通过 |
| 单次三规则扫描耗时 | 127 ms | 213 ms |

两款应用的 Cache/Cache_Data、Code Cache、GPUCache 都由真实程序生成，清理后为空，再启动后三者均重建。上述字节数为文件逻辑大小，不是空间释放保证；报告仍单独记录目标卷前后可用空间。运行于小型配置的一次耗时不是性能上限或日常资料规模的承诺。

证据：[执行脚本](../scripts/browser-validation.mjs)、[原始结果（passed=true）](../artifacts/browser-validation-0.6/evidence.json)、[版本、签名与哈希](../artifacts/browser-validation-0.6/binaries.json)。Brave ZIP SHA-256 已与官方发布附件核对，值为 `F2D439484D89E634792D8CE93A6A8E920AB01E70889F455D5511B78AF831ED09`。

来源：[Brave v1.96.59 官方发布](https://github.com/brave/brave-browser/releases/tag/v1.96.59)、[Vivaldi 官方稳定版下载](https://downloads.vivaldi.com/stable/Vivaldi.8.2.4133.76.x64.exe)。CDP 对 Brave 返回内核版本，不能将其当作产品版本；表格同时记录官方产品版本与 EXE 元数据。

测试 Cookie 模拟登录，不涉及真实账号。书签在浏览器关闭时写入标准书签结构，并在重启后核对；保护文件快照在删除前后、浏览器关闭状态比较。钱包、Rewards、邮件和笔记的保护边界另由合成文件夹具核对，未声称验证真实资产、邮件账户或所有交互功能。

## 真实桌面与用户任务

最终完整运行通过：[desktop-e2e-0.6.json](../artifacts/desktop-e2e-0.6.json)。调试版 EXE SHA-256 为 `0AC544CF1CC9BA55427ED1647B64F1A81FD9C5F02C30051B5B340C48EF09B33B`。删除夹具要求 debug、e2e-fixture 和临时根标记，正式程序不能加载该后端。

- 新增六条规则未勾选，Brave 网页缓存可键盘选择/取消；处理指引可展开。
- 最小窗口 760×580 CSS 像素无横向溢出，底部操作区可见；明暗主题、分析和清理明细键盘流程通过。
- 在 100,002 个真实文件中找到最大目录、逐层查看，原生 Explorer 实际选中中文文件；200 项分页、搜索、排序和硬链接去重通过。
- 分析/清理互斥，伪造节点拒绝定位，分析会话拒绝生成删除计划；重载接回、取消保留部分结果、分析期间关闭窗口通过。
- 现有缓存扫描、明细、取消确认、显式永久删除、长期排除、历史与重载闭环通过；保护文件和近期文件保留。
- 扫描与清理响应丢失后各命令只调用一次；未受理的迟到请求被拒绝为 request_expired。
- 清理后原分析标记为清理前统计；真实目标 C: 空间采样成功，本次 20 字节夹具清理前后可用空间差为 −114,688 字节，原样保留负数，未用逻辑删除量替代磁盘差值。其他应用活动也会改变可用空间，该差值不能单独归因于拂尘。

已查看截图：[新增类别浅色](../artifacts/apps-light-0.6.png)、[指引最小窗口](../artifacts/guides-minimum-0.6.png)、[指引深色](../artifacts/guides-dark-0.6.png)、[清理结果](../artifacts/desktop-results-0.6.png)。分析与清理明细截图及原始 HTML 一并保留于 artifacts。

桌面复跑中，WebView 缩放把 580 CSS 像素底边返回为 580.0000076293945；在记录实测值后将布局断言容差设为 0.01 像素，保留横向溢出检查。新增指引改变内容高度后，EdgeDriver 默认点击选择了被滚动容器裁剪的按钮中心；脚本改为先滚动到可见区域，再使用原生点击，不使用 JavaScript 强制触发按钮。键盘可达性仍独立验证。这些失败轮次不计为完整通过。

## 性能记录与结论边界

| 指标 | 0.5 最终记录 | 0.6 最终完整通过运行 |
|---|---:|---:|
| 文件条目 | 100,002 | 100,002 |
| 分析引擎耗时 | 31,909 ms | 28,631 ms |
| 包含界面调度的扫描等待 | 32,059 ms | 28,808 ms |
| 根目录查询往返 | 19.21 ms | 22.52 ms |
| 请求停止到后台终态 | 223 ms | 57 ms |
| 主进程工作集采样 | 290.22 MiB | 289.42 MiB |
| 含 WebView2 工作集合计 | 686.34 MiB | 692.78 MiB |
| 含 WebView2 私有字节合计 | 470.59 MiB | 489.79 MiB |

[0.6 原始性能证据](../artifacts/analysis-performance-0.6.json)为调试版、同类 NTFS 夹具和结束后单次资源快照，不是峰值；工作集中的共享页可能重复计算。此前未完成整个桌面流程的一轮分析为 55,929 ms，见 [首次采样](../artifacts/analysis-performance-0.6-first.json)；提示去重前完整运行的分析为 45,717 ms，见 [中间采样](../artifacts/analysis-performance-0.6-before-prompt-dedup.json)。三次读取均为 100,002 个文件，最后一轮采用最终前端资源。本机也有其他项目任务，不能把这些采样解释为固定负载实验或发行版性能。

最终单次扫描耗时及资源占用与 0.5 记录处于同一量级，没有在该轮观察到明显增加；但三次耗时波动较大，**不能据此证明提速或归因于扩展规则**，固定负载重复 A/B 仍有必要。新增缓存自身的真实小型配置扫描分别为 127/213 ms。既有大量近期小文件的清理扫描超过四分钟问题继续列为后续工作，未通过放宽删除前验证来绕开。

## 发行文件夹

`pnpm portable` 与 `node scripts/smoke-portable.mjs` 最终均通过。交付目录为 [拂尘_0.6.0_windows-x64](../artifacts/portable/拂尘_0.6.0_windows-x64/)，包含拂尘.exe、LICENSE、THIRD-PARTY-NOTICES.txt、inventory.json、portable-readme.txt、SHA256SUMS.txt，共 6 个文件。未生成安装器或分发 ZIP，不包含 helper；旧版本产物保留。

EXE 为 12,550,656 字节，ProductVersion 0.6.0，Authenticode 为 NotSigned。SHA-256：

```text
6037C7A9B2D6516E1E63EE59A18426C0B6591C0CF7A937B0ABE97A6AA533FDE6
```

发行启动检查覆盖中文和空格路径、拒绝程序目录写入、单实例、移动后设置沿用、顶栏最大化/双击/最小化/关闭。程序目录未产生设置或 WebView2 文件，客户区顶部间距为 0 px。Bootstrap 返回标准后端、0.6.0、14 条普通用户规则和 6 个常用分析位置，没有自动分析或清理。

本轮发行只读扫描专门选择六条新增规则；本机日常用户目录未发现支持的标准 Brave/Vivaldi 配置，六条均完成并返回对应说明，没有将零候选解释为“未安装”或“没有占用”。真实缓存删除与重建由前述隔离浏览器专项验证覆盖；发行检查不删除日常缓存。已有临时文件扫描由基线与桌面闭环覆盖，其大量小文件耗时问题继续保留。

7 个系统/混合能力请求全部返回 capability_unavailable，伪造计划、分析位置令牌、分析节点会话分别返回 invalid_plan、invalid_location、invalid_session；任务状态保持不变。未触发 UAC 或有效删除。脚本恢复测试目录原 DACL，并仅在本次写入值未被他人改变时恢复原扫描偏好。

最终 EXE 实际加载的资源为 `index-BvMv0OoP.js`、`index-CDL0QMTU.css`，与当前 dist 构建一致。构建 EXE、交付 EXE、SHA256SUMS.txt 和启动检查记录的哈希一致；最终调试 EXE 与桌面证据哈希一致。

证据：[发行启动结果](../artifacts/portable-smoke-0.6.json)、[运行截图](../artifacts/portable-readonly-0.6.png)、[构建日志](../artifacts/build-portable-0.6.log)、[交付清单](../artifacts/release-manifest-0.6.json)、[最终源码与资源哈希](../artifacts/source-manifest-0.6.json)。最后启动检查于主机当地 2026-09-27 02:54 完成。

## 未执行与适用边界

- Windows 11、无 WebView2 的真实系统、中文 Windows 账户、完整 100%–200% DPI、高对比度及屏幕阅读器矩阵。
- 每个新浏览器单独组合的真实 ACL 故障、进程检测失败和启停竞态压力；公共平台与引擎测试覆盖相关分支，真实浏览器覆盖运行中阻止和正常退出后清理。
- 完整交互式 Brave/Vivaldi、真实密码登录、同步、钱包资产、Rewards、笔记及邮件账号；当前为隔离配置和本地网站的缓存与保护数据验证。
- 真实云提供方下载行为、真实同时多卷清理、配额/卷拔除/磁盘满和中断 VM；多卷计算与失败处理由模拟测试覆盖，本机真实清理采样仅 C:。
- 50 万条完整资源压力、更多硬件；原生目录选择器和重定向 Known Folder 的完整人工兼容矩阵。
- 微信、QQ、WPS 各版本菜单与真实清理操作；本轮只有注明局限的通用指引，没有相应删除规则。
- 真实用户试用、远程 CI、独立安全审查、系统提权、安装升级和发行签名。

研究个案不用于推断需求比例；自动化任务验收不等于用户访谈或兼容保证。当前保持未签名免安装预览版、本地运行、无遥测、无后台扫描；不增加聊天附件、大文件、全目录删除或自动清理。

相关文档：[研究记录](user-research-0.6.md)、[支持矩阵](support-matrix-0.6.md)、[用户手册](user-guide.md)；历史验收：[0.5](verification-0.5.md)、[0.4](verification-0.4.md)、[0.3](verification-0.3.md)。

# 拂尘 0.8.0 验收记录

验证日期：2026-09-26–27（原始证据保留 UTC，用户时区 America/New_York；主机显示 UTC+08:00）。Windows 10 专业版 22H2 / 19045，Intel i5-10400、约 16 GB RAM、固定 NTFS 卷；Rust 1.97.1 MSVC、Node 24.12.0、pnpm 10.30.3。默认普通权限、未签名免安装预览版，仍为 14 条清理规则。

本轮完成全范围最大文件排行、可信位置用途说明、磁盘可用空间与下一步入口。用途不构成删除授权；所有真实删除验收只使用隔离夹具及独立浏览器配置。

## 基线与自动检查

0.7 的 219 个文件与[清单](../artifacts/source-manifest-0.7.json)一致，复制后保留[源码归档](../artifacts/baseline-0.7-fixed.zip)和[历史验收](verification-0.7.md)。实施前 Rust 74 项、前端 44 项全量复跑通过：[Rust 日志](../artifacts/baseline-rust-for-0.8.log)、[前端日志](../artifacts/baseline-frontend-for-0.8.log)。

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| Rust 全量 | 80 项通过，3 项 opt-in 专项默认忽略，分别运行浏览器、配对性能、查询性能 | [日志](../artifacts/rust-tests-0.8.log) |
| 前端全量 | 6 文件、50 项通过 | [日志](../artifacts/frontend-tests-0.8.log) |
| Rust 格式、Clippy | 全 workspace / all-targets，-D warnings 通过 | [格式](../artifacts/format-0.8.log)、[Clippy](../artifacts/clippy-0.8.log) |
| 实验 system-cleanup | 类型检查通过，不等于提权验收 | [日志](../artifacts/check-system-0.8.log) |
| TypeScript、生产前端、Prettier | 通过 | [构建](../artifacts/build-frontend-0.8.log)、[格式](../artifacts/prettier-check-0.8.log) |
| 依赖锁 | 外部 Cargo 和 pnpm 依赖不变 | [核对](../artifacts/dependency-lock-check-0.8.json) |

新增测试覆盖跨层最大文件、同名稳定排序、首尾分页、中文搜索与长路径、零字节、超过 JS 安全整数、共享硬链接、排除标注、不读节点、令牌失效、取消及部分范围。用途测试覆盖本机 Known Folders、合成重定向映射、混合用户根、标准浏览器资料与缓存叶位置、相似前缀及无关 WeChat/AppData/Cache 名称。前端覆盖独立视图搜索、返回父目录、迟到结果丢弃、新会话重置、采样失败未知、手动重试、进度不触发采样、位置变化与固定设置失败提示。既有 Junction、权限不足、占用、稀疏文件、云属性、目录替换、文件变化、伪造计划、互斥、过期请求、旧历史与脱敏回归继续执行。

Clippy 初次指出可选用途结构增大事件枚举，改为查询时分配的 Box 可选字段后通过，没有屏蔽警告或增加扫描阶段分配。[初次诊断](../artifacts/clippy-0.8-first-attempt.log)。前端类型检查曾发现测试中多余的 exact 属性，删除后全量构建通过。

## Release 配对性能

总结果：**通过全部约定门槛**。两版使用相同工具链、Release / thin LTO / 单 codegen unit 与依赖锁。[原始 92 条数据](../artifacts/performance-0.8/measurements.jsonl)、[汇总](../artifacts/performance-0.8/summary.json)、[环境与二进制哈希](../artifacts/performance-0.8/environment.json)、[硬件](../artifacts/performance-0.8/host.json)、[日志](../artifacts/performance-test-0.8.log)。

| 负载 | 0.7 中位数 / 范围（秒） | 0.8 中位数 / 范围（秒） | 耗时变化 | 最大采样内存峰值变化 | 门槛 |
| --- | ---: | ---: | ---: | ---: | --- |
| 十万文件，90% 近期 | 6.27 / 6.19–6.52 | 6.07 / 6.04–6.39 | -3.13% | -1.20% | 通过 |
| 十万文件，50% 近期 | 32.17 / 31.58–35.10 | 32.19 / 31.31–32.43 | +0.04% | -0.19% | 通过 |
| 十万文件，全部旧文件 | 64.71 / 64.21–72.12 | 63.49 / 63.23–66.52 | -1.89% | -0.09% | 通过 |
| 一万文件，24 层嵌套 | 23.23 / 23.16–23.96 | 23.05 / 23.02–23.82 | -0.75% | -0.35% | 通过 |
| 十万文件，500 条排除路径 | 32.66 / 32.30–32.95 | 32.79 / 31.54–34.86 | +0.39% | +0.12% | 通过 |
| 十万文件，只读分析 | 12.65 / 12.62–12.93 | 11.98 / 11.96–12.41 | -5.26% | +4.92% | 通过 |

每版每负载预热一次，随后交替五组配对：12 次预热、60 次正式测量、20 次清理扫描取消。沿用原 G: 固定 NTFS 夹具的内容与时间，没有重新播种。基线直接复制已核对哈希的 0.7 current.exe；两版 test-only 扫描桥源码字节一致，删除方法固定拒绝。测量与构建、其他验收串行；预热保留但不进入成绩，正式样本没有挑选删除。

首轮完整测量的只读分析耗时回退 32.58%，未达到门槛；[首轮全部原始数据](../artifacts/performance-0.8-initial/measurements.jsonl)、[首轮汇总](../artifacts/performance-0.8-initial/summary.json)与二进制保留。随后在分析入库中复用编译后的 SQLite 插入语句，并在收尾时一次建立排行索引，减少逐文件维护排序树的开销。每条记录仍单独绑定，不缓存文件句柄、身份结果或删除授权；扫描中查询仍只使用已读记录，索引未完成前任务保持繁忙。修正后重新执行完整配对与查询专项，本表只使用最终实现的数据。首轮失败不计为通过，也未混入最终成绩。

所有清理候选集合和字节哈希一致，分析统计保持 100,000 文件、100,101 索引项、2,800,000 逻辑/去重字节与完整状态。内存门槛取五次正式样本中 working set 与 private commit 最大采样峰值的较大比值，句柄及各次原始值也保留。约 5 ms 采样不保证捕获瞬时峰值；测试 EXE 资源与含 WebView2 的产品资源分开。门槛是耗时和内存均不回退超过 10%，不沿用 0.7 的 50% 提速目标。

清理扫描 20 次取消 P95：**6.05 ms**，通过 1 秒目标。既有受控阻塞测试仍确认系统调用返回前保持繁忙，不提前允许新任务；没有人为制造真实内核 I/O 永久挂起。

## 最大文件查询与分析取消

使用同一十万旧文件夹具，完整分析后仅查询内存索引。每类预热一次，再测量 50 次。[250 次原始查询及 20 次取消](../artifacts/performance-0.8/analysis-query-0.8.json)、[运行日志](../artifacts/query-performance-0.8.log)。

| 查询 | 次数 | P95 | 200 ms 目标 |
| --- | ---: | ---: | --- |
| first | 50 | 18.77 ms | 通过 |
| last | 50 | 36.10 ms | 通过 |
| search-first | 50 | 22.99 ms | 通过 |
| search-last | 50 | 22.93 ms | 通过 |
| search-empty | 50 | 38.35 ms | 通过 |

分析取消 P95：**2.39 ms**，通过 1 秒目标；计时从请求到真实后端终态，约 1 ms 轮询。nearest-rank P95 对 50 次取第 48 个，对 20 次取第 19 个。

另在真实 Debug Tauri / WebView2 下串行测量首尾页、搜索首尾页各 50 次，包含 WebDriver IPC 往返，P95 分别为 all-first 60.91 ms、all-last 73.99 ms、search-first 102.31 ms、search-last 91.78 ms。[200 次原始记录](../artifacts/analysis-ipc-queries-0.8.json)。此组是调试构建的交互补充，单独列出，不混入 Release 成绩。

## 真实浏览器与桌面流程

沿用已核对来源、稳定版版本、签名和哈希的[官方程序清单](../artifacts/browser-validation-0.6/binaries.json)，重新建立独立配置和本地测试网站，未使用用户日常浏览器资料或真实账号。[本轮完整证据](../artifacts/browser-validation-0.8/evidence.json)、[日志](../artifacts/browser-tests-0.8.log)。

| 程序 | 实际删除文件 | 删除逻辑字节 | 重建缓存文件 | 非目标文件内容与时间保留 |
| --- | ---: | ---: | ---: | ---: |
| Brave 1.96.59 / Chromium 154.0.8037.58 | 17 | 1650422 | 16 | 231 |
| Vivaldi 8.2.4133.76 | 25 | 9414901 | 17 | 471 |

HTTP、代码、GPU 缓存都实际生成、清理并重建；进程运行及伪造计划拒绝；重启后测试 Cookie 登录状态、Local Storage、书签保留。不把测试状态外推到真实钱包、邮件、扩展或所有浏览器版本。

[真实桌面证据](../artifacts/desktop-e2e-0.8.json)、[日志](../artifacts/e2e-0.8.log)通过：首页诊断入口 → 最大文件搜索深层文件 → 展开用途 → 返回所在目录 → Explorer 实际选中 → 既有隔离缓存扫描、确认、清理；保护文件内容和时间保持。100,002 文件分析后端耗时 48.83 秒，桌面取消约 235 ms；Debug 数值不作为 Release 回退判断。包含 WebView2 的进程资源快照在该 JSON 中，不是峰值或两版资源对比。

覆盖 760×580 最小窗口、浅色/深色、键盘焦点、默认关闭新规则、部分扫描确认与历史、迟到响应、重载接回、任务互斥与关闭等待。部分扫描由明确 debug 标记注入，真实 ACL 场景另由 Windows 测试覆盖。[首页](../artifacts/home-diagnostic-0.8.png)、[最大文件](../artifacts/largest-files-0.8.png)、[最小窗口](../artifacts/analysis-minimum-0.8.png)、[深色](../artifacts/analysis-dark-0.8.png)。自动化走查不称为真人访谈或真人理解度验收。

首轮桌面脚本误选隐藏分析页中的同名应用指引，修正为只点击实际显示的匹配控件后复跑通过，[失败日志](../artifacts/e2e-0.8-first-attempt.log)保留。没有通过跳过清理或保护验证规避失败。

最终复跑曾在“重新分析”处暴露脚本同步问题：直接提交伪造计划会更新后端请求回执版本，而空闲界面尚未同步，下一次原生点击按预期被拒绝为过期请求。脚本改为在伪造请求拒绝后通过真实页面重载接回同一分析会话，再继续完整交互流程；没有放宽后端版本校验。[失败日志](../artifacts/e2e-0.8-control-sync-failure.log)、[失败截图](../artifacts/e2e-0.8-control-sync-failure.png)保留。

## 交付与未执行环境

[0.8 程序文件夹](../artifacts/portable/拂尘_0.8.0_windows-x64)、[构建日志](../artifacts/portable-build-0.8.log)、[发行冒烟](../artifacts/portable-smoke-0.8.json)、[冒烟日志](../artifacts/portable-smoke-test-0.8.log)。发行 EXE 验证中文/空格路径、拒绝写入目录、重复实例、移动后设置沿用及只读扫描，不对日常资料删除。目录只有 EXE、说明、许可与校验清单，不含 helper、安装器或调试夹具。

EXE ProductVersion **0.8.0**，Authenticode **NotSigned**，12776960 字节；SHA-256：`BF9365B875535DC14D9172094DE0F1052BC9C24047A639F2882236F51B3714F3`。前端资源 ["index-D7s0sFRS.js","index-XUB29qEX.css"]。[SHA-256 清单](../artifacts/portable/拂尘_0.8.0_windows-x64/SHA256SUMS.txt)覆盖五个内容文件；[交付清单](../artifacts/release-manifest-0.8.json)记录六文件哈希，[源码清单](../artifacts/source-manifest-0.8.json)固定最终源码。

研究与用户说明：[双视角研究](user-research-0.8.md)、[用途矩阵](support-matrix-0.8.md)、[用户手册](user-guide.md)。

源码清单覆盖本轮程序、配置、测试和文档；工作区独立的 prototypes 设计原型保持原样，不是本次构建输入，也不纳入发行功能或该清单。

未执行：Windows 11、其他硬件、真实配额/多卷异常/拔除、真实 Known Folder 重定向操作、无 WebView2 系统、完整 100%–200% DPI、高对比度和屏幕阅读器、真实云提供方下载监测、50 万条上限资源压力、完整交互式浏览器钱包/邮件/扩展/启停竞态、真人可用性研究和独立安全审计。系统设置失败已模拟验证，未逐系统版本走查实际设置页面。合成映射、属性夹具与单机结果不替代这些环境。

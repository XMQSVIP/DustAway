# 拂尘 0.10 验收记录

日期：2026-09-27。版本：0.10.0 未签名 Windows x64 免安装预览版。沿用空间册，完成“选择磁盘 → 找主要占用 → 理解影响 → 按合适入口处理 → 核对空间变化”。规则、计划授权、删除协议、数据库格式及普通用户权限边界不变。

## 改造前备份与恢复

独立归档：`G:\my_ai\DustAway-backups\ui-spatial-0.9.0-20260927T093925Z`。经典版归档 `ui-classic-0.9.0-20260927T075051Z` 保留且未改动。仓库没有 Git 提交，未以新分支代替备份。

- 源码包覆盖 252 个可重建文件，包括未跟踪源码、配置、锁文件、规则、资源、文档和测试，排除依赖/编译缓存和个人运行数据。
- 源码 ZIP SHA-256：`f920e54466d51aa2a9a85d63b9c07421909db7c1d5b1ca434f95b2a349cfa136`。
- `restore-check` 独立解压，锁定离线安装、前端构建、59 项前端测试及 Tauri Release 构建通过，之后才改造。恢复构建共用编译缓存，源码确实来自独立解压目录。
- 对应旧版程序 `verified-portable/dustaway-0.9.0.exe` SHA-256：`4805d374f22a20483c8e8abfee1619e5c60f7eade610c52671b7671ceebf777c`。旧程序、许可和使用说明一起保留；现场已有程序另存 `existing-portable`，不与恢复构建混同。
- 原空间册各页面和状态证据存于 `existing-evidence`；是原有证据，不声称本轮重新执行了旧版全部桌面测试。
- `manifest.json` 为逐源码哈希，`archive-files.json` 为源码 ZIP、程序和已有证据共 69 项的哈希；恢复验证后再次核对 252 个源码文件仍完全一致。`RESTORE.md` 说明先独立恢复再替换，不覆盖用户设置和历史数据库。

## 环境与自动检查

实际机器：Windows 10 Pro 22H2，10.0.19045；Intel Core i5-10400，6 核 12 线程，约 16 GiB 内存。Node 24.12.0、pnpm 10.30.3、Rust/Cargo 1.97.1。原生 WebView2 实测 `devicePixelRatio=1.25`，默认窗口 1100×780 CSS 像素。

| 检查 | 结果与边界 | 证据（仓库相对路径） |
| --- | --- | --- |
| 前端完整行为测试 | 9 文件、64 项通过；最后两处展示调整后，相关 42 项再通过 | artifacts/space-0.10/frontend-tests-final.log、frontend-journey-final.log |
| TypeScript / Vite | 通过，打包前端与 Release 内嵌资源一致 | artifacts/space-0.10/frontend-build-final.log、portable-build-final.log |
| Rust 工作区 | 99 项通过，3 项专用环境测试保持 ignored；不是全部真实应用验收通过 | artifacts/space-0.10/rust-tests-startup.log |
| 格式与 Clippy | cargo fmt --check、workspace all-targets -D warnings 通过 | artifacts/space-0.10/clippy-startup.log |
| 实验 feature 编译 | system-cleanup cargo check 通过；不包含真实 UAC 验收，默认包仍不带 helper | artifacts/space-0.10/system-feature-check.log |
| 原生桌面 E2E | 真实 Tauri/WebView2、专用标记夹具，完整闭环通过 | artifacts/space-0.10/desktop-e2e-final.log、artifacts/desktop-e2e-0.10.json |
| 原生失败与部分成功 | 定位失败保留文件行和勾选；1 项删除、1 项跳过；保护文件不变 | artifacts/space-0.10/native-states-final.log、state-evidence.json |
| 数据库初始化恢复 | 真实无法打开的夹具数据库路径触发原生提示；保留原始错误与存储设置入口，不重建、不删除 | artifacts/space-0.10/startup-failure-verified.log、startup-database-failure.json |
| 发行版冒烟 | 0.10 版本、当前内嵌前端、不可写程序目录、中文/空格路径、重复启动、移动后设置、窗口控制通过 | artifacts/space-0.10/portable-smoke-final.log、artifacts/portable-smoke-0.10.json |

3 项 ignored 分别为隔离真实浏览器验收、清理扫描性能夹具、旧版专用分析查询性能入口。本轮另有新版 Release 分析基准；没有把这些 ignored 测试写成通过。

## 关键场景

- 核心 Fake 平台验证卷身份变化、卷消失、不可用卷、采样失败与真实零空间的区别；未验证身份不发放可用磁盘入口。旧令牌不能转而分析新映射磁盘。
- Windows NTFS 夹具验证根层先列举、目录细分的身份与父链复核、文件/目录替换拒绝、128 层分支超限后仍读取另一分支。原有硬链接、链接/云属性、权限拒绝、稀疏/长路径测试通过。
- 发行版识别 C、D、F、G 四个本地 NTFS 卷并成功采样；默认系统卷来自后端。明确启动 C 与 D 根目录只读分析后停止，均保留部分结果，真实权限、链接或深度问题不当作空目录。本次根探测没有完整扫描整盘，不声称全盘遍历完成。
- 桌面 100,002 个合成文件验证最大文件、跨层查询、分页、硬链接去重、定位、取消等待、页面重载恢复和任务互斥；清理后标记分析过期。专用夹具通过扫描、文件选择、长期排除、再次扫描、计划、取消确认、永久删除、结果与历史。
- 迟到未受理请求失效；丢失扫描和删除响应后只读核对，不重复执行。页面重载后重新选择，未选文件不冒充未处理。
- 六条应用规则的运行/退出/重查场景由夹具模拟，始终无法进入删除计划；没有借此声称三款真实应用清理已通过验收。未知进程、零候选、部分读取与结果不确定由前端/核心测试覆盖。
- 前端验证“查看”不更改范围，“加入”合并原范围并要求重扫；返回同一条目、滚动位置、查询保留。精度测试跨 JavaScript 安全整数范围，同卷负增长仍精确至 1 字节，身份不同不相减，采样失败不显示为零。
- 真正 SQLite `max_page_count` 限额和 `query_only` 分别产生 DiskFull / ReadOnly，Engine 不能初始化，平台扫描/删除函数不得被调用。原生数据库错误夹具又验证实际 GUI 路径。未填满任何日常磁盘。

## 视觉、键盘与证据

原生桌面验证 760×580、1100×780、明暗主题，最小窗口无横向溢出、底部操作区可见；候选与分析流程使用真实键盘导航。网页预览验证四页 × 三种尺寸（760×580、1100×780、1600×1000）× 四档像素密度（1/1.25/1.5/2），共 48 组；减少动画过渡为 0s，强制颜色采用平面边界与文字。

像素密度模拟不等于真实 Windows DPI。早期使用 `html.style.zoom` 的额外探针会放大固定 vh 容器、使底栏超出视口，因此该组不计作 DPI 通过；没有通过隐去这组记录来宣称 200% 原生缩放已验证。原始数据保留 `preview-accessibility.json`，正确的像素密度布局模拟另存 `layouts-dpr.json`。

本轮原生截图包括：

- `artifacts/space-0.10/release-volume-entry.png`：发行版磁盘入口与真实采样。
- `artifacts/space-0.10/confirmation-light.png`、`failure-file-changed.png`、`partial-success.png`：确认、真实定位失败、部分处理。
- `artifacts/space-0.10/history-partial.png`、`analysis-strata.png`、`settings-dark.png`、`result-dark.png`：四页及结果。
- `artifacts/space-0.10/startup-database-failure.png`：原生数据库恢复提示。
- `artifacts/analysis-minimum-0.10.png`、`analysis-dark-0.10.png`、`applications-gated-0.10.png`：最小窗口与只读限制；`preview-forced.png`、`preview-reduced.png` 为明确的浏览器模拟截图。

## 本轮发现并修复

1. 定位失败原来复用列表错误状态，隐藏已经读到的文件；现在操作错误独立，原列表与选择保留。
2. 分析跳转类别原来替换清理选择；现在查看和加入范围分离，新增范围使旧计划不可执行。
3. 返回页面时重新置 loading 会丢失原行与焦点；现在命中同一结果时保留 DOM、查询和滚动，迟到响应仍丢弃。
4. 真机故障注入发现 Tauri 在 setup 钩子返回错误时会直接 panic，外层 run 错误分支接不到。现在先完成 build 的单实例检查，再初始化服务，最后创建界面；错误在窗口创建前原生显示。重复启动不会打开/恢复已有实例日志。

早期测试失败记录保留。最后原生故障脚本还修正了 PowerShell 5.1 的 UTF-8 BOM 识别，避免中文断言误判；不是通过移除断言绕过失败。

## 尚未验证

Windows 11、真实 Windows 100%/150%/200% DPI、屏幕阅读器、非 NTFS 实盘、卷热拔插/盘符重新映射、真实跨卷清理与配额异常、物理磁盘耗尽时 WebView2 的行为仍待专用环境验证。相应逻辑已有单元或部分本机边界检查，不能等同真实环境完成。

微信/QQ/WPS 六条规则的隔离真实应用验收继续独立推进，后端开关保持只读。五名非开发者试用尚未开展，状态为“人工可用性待验证”，人员由项目所有者安排；[试用脚本](usability-0.10.md) 已准备。公开资料研究不是用户访谈或需求比例统计。

## 程序交付

目录：`artifacts/portable/拂尘_0.10.0_windows-x64/`。仅 EXE、说明与许可/依赖/校验文件，不含 helper、安装器或 ZIP。程序未签名；最终文件版本、SHA-256 和逐文件核对见 `artifacts/space-0.10/delivery-verification.json`。源码仍为原有未提交工作区，没有擅自创建提交。


## Release 性能对比

同机相同 Rust 工具链与优化选项，harness 分别链接已恢复的 0.9 源码和 0.10 源码。每个负载每版完整扫描三次，顺序为旧/新、新/旧、旧/新；每版每个负载另测三次取消。没有与本任务编译或其他测试并行，未清空系统缓存。每组 1000 个 7 字节文件，目录与文件都计入索引；50 万文件负载在 50 万条索引处终止为 partial，不能称为所有文件已统计。

| 文件负载 | 版本 | 完成耗时中位数（范围），秒 | 查询中位数：全部 / 名称搜索，毫秒 | 私有提交峰值，MiB | 取消到终态中位数（范围），毫秒 |
| --- | --- | --- | --- | --- | --- |
| 10 万 | 0.9 恢复源码 | 13.80（13.45–19.06） | 18.63 / 41.18 | 269.7 | 5（5–6） |
| 10 万 | 0.10 | 12.83（12.47–12.91） | 19.76 / 42.35 | 269.3 | 5（5–6） |
| 50 万 | 0.9 恢复源码 | 79.87（61.59–164.66） | 85.34 / 109.14 | 1342.4 | 5（5–6） |
| 50 万 | 0.10 | 61.67（59.23–166.19） | 89.94 / 114.48 | 1339.9 | 5（5–5） |

峰值来自 GetProcessMemoryInfo 的 PeakPagefileUsage，包含查询阶段，只代表分析 harness 进程，不是桌面应用总内存。查询每次返回最多 200 项，每轮交替进行五次全部文件和五次名称包含“099”的查询。扫描未知总量，没有用这些耗时生成 UI 百分比或预计剩余时间。

十万负载完成，五十万条上限与取消均保留部分索引。五十万负载约 1.31 GiB 的分析内存峰值是实际成本，低内存环境应缩小目录范围。耗时存在缓存和系统波动；三轮合成样本不能外推为整盘速度、冷启动表现或其他硬件的性能承诺。

全部 24 条原始样本与环境、两版 EXE 哈希保存在 artifacts/space-0.10/release-benchmark.json；汇总为 release-benchmark-summary.json。可复用代码与方法见 scripts/analysis-benchmark/，本机种植及执行脚本另保留在 artifacts/space-0.10/。


最终交付核对：EXE 版本 0.10.0，12971520 字节，签名状态 NotSigned。SHA-256：`8ae33416f242f65047508d59321fc6913e7d914d16a33f942723bfcdefdbc090`。包内 5 项校验全部匹配，EXE 与最后一次发行冒烟实际运行文件相同。


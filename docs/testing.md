# 测试指南

本文描述可重复执行的测试流程。具体机器、版本和已通过结果放在 [验收记录](verification.md)，不要用测试计划替代测试结果。

## 环境准备

在普通用户 PowerShell 中，从仓库根目录执行：

```powershell
pnpm install --frozen-lockfile
pnpm notices
pnpm build
pnpm test:evidence
pnpm test:acceptance
pnpm check:applications
pnpm check:recognition
```

工具链与启动模式见 [开发指南](development.md)。主程序拒绝管理员身份运行；默认版本应拒绝全部系统 scope 和混合请求；真实提权是独立实验路线。

## 自动检查

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
pnpm test
pnpm build
```

`pnpm check` 可以执行主要构建和测试组合，但不包含格式检查、Clippy、桌面 E2E 或真实 Windows 发布验收。

| 层级         | 重点                                                                        | 数据范围                        |
| ------------ | --------------------------------------------------------------------------- | ------------------------------- |
| Rust 核心    | 会话、一次性计划、取消、排除、浏览器状态变化、恢复、10 万索引分页、属性测试 | Fake 平台与测试数据库           |
| Windows 适配 | 标准配置识别、句柄删除、链接、占用、文件和目录替换、长路径                  | tempfile 专用目录               |
| 提权协议边界 | 不存在任意路径/Shell 请求，系统 Engine 拒绝用户规则与无效计划               | 测试 Engine，不需要系统目录删除 |
| 前端         | 选择准确、主题保留结果、状态不确定、占用重查、搜索防抖与整数精度            | Mock API                        |
| 桌面         | 真正的 WebView2、Tauri IPC 与文件明细/确认流程                              | 显式调试夹具后端                |

能力拒绝测试不等于完成实验版本的真实 UAC 测试；内存索引的 10 万候选测试不等于完成真实大目录性能测试。

## 桌面夹具 E2E

先安装 `tauri-driver` 并将匹配 WebView2 的 `msedgedriver.exe` 放到 `artifacts/webdriver/`：

所有桌面入口使用 `scripts/desktop-driver.mjs`：创建会话时校验 EdgeDriver 与实际 WebView2 的前三段版本，不匹配则关闭会话并失败。测试请求的 `window/rect` 调整由 `window-state.ps1` 转为外层 Tauri 原生窗口调整，单位为逻辑客户区尺寸；不要绕过共享入口直接把调整请求发给 EdgeDriver。原始接口在本机可移动 Chromium 子窗口，造成内容整体向右下偏移，且网页内部截图和布局断言仍可通过。

调整后同时验证客户区大小、窗口位置不变、WebView 子窗口原点及边界、DOM 视口尺寸与 DPI。独立只读回归使用 `pnpm test:window-desktop`（预先构建普通桌面程序，或通过 `DUSTAWAY_UI_APP` 指定程序）；它不启动扫描或清理，证据位于 `artifacts/window-offset-regression`。纯断言回归为 `pnpm test:desktop-driver`。详细定位见 [窗口偏移修复记录](verification-window-offset.md)。

```powershell
cargo install tauri-driver --locked --version 2.0.6
pnpm test:e2e
```

脚本自动创建临时夹具：可清理的旧文件、应保留的新文件、缓存根之外的保护文件，以及专用标记。它依次验证扫描、文件明细、返回检查、再次确认、删除结果和历史。

保护条件包括：

- Rust 后端同时要求调试构建和 `e2e-fixture` feature。
- 测试目录必须是系统临时目录的直接子目录，名称具有专用前缀，标记内容匹配。
- 脚本在扫描前确认界面接收到后端的夹具构建标记；普通构建应被拒绝。
- 真正的 release 构建不会编入夹具后端，不能通过环境变量把它切换成测试模式。

不要删除这些保护条件，也不要把个人目录传给夹具变量。脚本会保留自己的临时目录用于检查；清理测试残留时，只处理本次测试输出的明确目录，不能使用宽泛通配符清理整个系统 Temp。

当前脚本将过程截图与汇总写入 `artifacts/cleanup-preparation-e2e/`，可用 `DUSTAWAY_E2E_OUTPUT` 指定独立输出目录；两份桌面脚本使用同一变量。旧网盘阶段证据保留。资源快照包含子进程，但只是一个时刻的统计；工作集相加可能重复计算共享页。应用夹具覆盖 18 条只读规则，验证逐规则空清理版本及阻止原因、分组、运行提示、保护文件，以及带手工复核字段的网盘与迅雷请求仍被后端拒绝。桌面实际删除仅作用于专用夹具里的既有清理规则；六条新规则的成功路径由上述 Rust 专用夹具验证。

## 自选删除夹具

`cargo test -p dustaway-windows --locked --test manual_cleanup` 链接普通生产库，测试只在仓库 `artifacts/manual-fixture-*` 下创建资料；不启用应用规则验收许可。它验证整个子树与排除、身份变化、文件替换、空目录、占用、取消和保护边界。核心测试另外覆盖选择继承、大整数、部分子树、准备取消及意图恢复。

`pnpm test:manual-desktop` 构建不含 `e2e-fixture` 的普通程序，运行 `scripts/manual-standard-check.mjs`。需要上述 WebDriver。脚本在桌面创建自身带标记的临时目录，仅读取父目录以取得后端位置令牌，所有删除只选此目录内的合成资料；不扫描真实垃圾或清理应用。成功后仅移除自身夹具和按唯一 ID 识别的合成历史，保留其余用户数据。输出在 `artifacts/manual-standard-desktop/`，失败时保留夹具和截图。

普通构建即使设置测试环境变量也必须报告 `standard`。完整桌面 E2E 另覆盖规则与自选混合清单。测试记录必须标明 `synthetic=true`、`realApplicationAcceptance=not_run`；本轮证据见[验收记录](verification-manual-cleanup.md)。

## 应用目录与证据回归

本轮新增 `cargo test -p dustaway-windows --locked --lib application_acceptance_tests`：在带标记的专用临时目录内，验证六条目标规则的实际句柄删除、单次计划、保护样本、版本/身份/文件变化、后台状态、取消恢复、硬链接/云属性和微信 7 天边界。`cargo test -p dustaway-windows --locked --test cleanup_release` 链接普通库，确认伪造描述符仍不能开启应用清理。

`pnpm test:acceptance` 检查独立开发验收记录的结构、整数精度、保护摘要、真实/合成区分、哈希与防覆盖；完整记录生成和后续人工步骤见[操作单](support-matrix-cleanup-preparation.md)。这些测试不会启动微信、钉钉或百度网盘，也不会接触其日常资料。真实应用验收保持未执行。

`cargo test -p dustaway-windows --locked` 覆盖应用身份、飞书同一安装的双程序版本、经典 QQ 与 QQNT 的区分，以及多账号、多数字目录、缺失标记、拒绝访问、链接、取消和长期排除。版本变化只影响观察信息；删除仍须通过独立静态验收门槛。

修改封闭注册表后运行 `pnpm applications:catalog`，同时生成 v3 工具目录、catalog_id 与前端规则预览；`pnpm check:applications` 检查生成结果是否过期。v1/v2 旧包分别使用冻结的 6/11 条目录校验；v3 使用当前编译目录标识，未知标识拒绝。

本机只读采集命令为 `node scripts/application-evidence.mjs collect artifacts/<新的输出目录>`，校验命令为 `node scripts/application-evidence.mjs check artifacts/<输出目录>`。不使用已有证据目录覆盖历史结果。v3 覆盖全部 18 条应用规则，所有只读采集都必须输出“删除未执行”，不得生成删除解锁凭据。真实应用验收流程见 [逐规则操作单](support-matrix-netdisk-preparation.md)，本轮未执行。

## 只读实际目录检查

```powershell
cargo run -p dustaway-windows --example scan_smoke
```

该示例扫描本机标准普通用户目录，输出分类数量、大小和阻止状态，不执行清理。若希望所有测试都与个人文件隔离，使用前述夹具即可，不必运行此项。

发行程序检查使用下面的脚本，仅扫描，不启动清理：

```powershell
pnpm portable
node scripts/smoke-portable.mjs
```

它复制发行 EXE 到临时目录中的中文和空格路径，仅对此专用目录设置拒绝写入权限，在 finally 中恢复原 DACL；验证启动、重复实例、只读扫描、移动目录与设置沿用。另发送系统 scope、混合请求及伪造计划，断言它们在启动任务前被拒绝。脚本不提交有效清理计划，不删除实际用户缓存。当前输出 `artifacts/portable-smoke-0.12.json` 与 `artifacts/portable-readonly-0.12.png`。测试需要匹配的 WebDriver，不验证缺少 WebView2 的环境。

窗口专项检查包含最大化/还原按钮、顶栏双击、最小化及重复启动恢复、关闭按钮。`scripts/window-state.ps1` 只读探测测试进程的窗口：展开 Windows 短路径，固定初次读取的 HWND，并验证它仍属于同一可执行文件的进程，避免最小化后自动选到其他辅助窗口。测量客户区顶部与窗口顶部的实际间距，不能仅凭 WS_CAPTION 样式位推断是否显示原生标题栏。

## 人工或 VM 场景（系统提权、安装器属于独立实验，不是当前文件夹版的交付门槛）

在具有快照的虚拟机中预建有限测试数据，再测试系统项。不要以开发主机系统目录承担提权删除和故障注入验证。

| 类别       | 操作重点                                                            | 应检查的结果                                             |
| ---------- | ------------------------------------------------------------------- | -------------------------------------------------------- |
| 权限       | 接受/拒绝 UAC，其他管理员凭据，辅助进程退出                         | 普通用户任务仍可解释；无跨域任意删除                     |
| 文件系统   | 拒绝访问、锁定、链接循环、扫描后替换、云占位、非 NTFS               | 拒绝或跳过，不改变保护文件                               |
| 进程变化   | 确认后启动真实浏览器、任务中关闭主界面                              | 重新检查、取消并保留部分结果                             |
| 故障恢复   | 数据库无法写入、磁盘不足、删除意图后中断                            | 不自动续删，区分已完成和不确定项                         |
| 兼容与体验 | Windows 10/11、中文账户、长路径、100%–200% DPI、键盘、高对比度      | 无遮挡或越界操作，焦点和状态清晰                         |
| 规模       | 10 万真实测试文件、不同磁盘、扫描期间操作窗口                       | 分页可用、取消可响应，记录实际资源与耗时                 |
| 分发与更新 | 文件夹运行、缺少 WebView2、移动、只读目录、设置沿用；安装器另行验证 | 主程序普通权限运行，设置符合保留策略，卸载不影响清理目标 |

## 测试结果记录模板

```text
测试编号：
程序版本 / 源码版本或源码归档标识：
交付包 SHA-256：
Windows 版本 / 架构 / 缩放：
WebView2 / EdgeDriver 版本：
账户与提权方式：
VM 快照或专用夹具位置：
测试前数据与保护文件校验值：
操作步骤：
预期结果：
实际结果与保护文件校验值：
报告 / 截图 / 日志：
结论：通过 / 失败 / 未执行
剩余问题：
```

发现失败时保留可复现步骤和最小夹具。涉及误删、权限或日志不确定性的问题应阻止正式发行；普通视觉问题则记录影响范围，避免把单次截图当成全平台验收。

## 0.3 增量回归

核心测试覆盖分类中断、一致快照、偏好持久化、字段补丁、排除批次原子性/上限/失效会话与存储失败；Windows 夹具覆盖候选操作的文件变化、父目录替换、缺失、越界和硬链接。原有句柄删除回归继续执行。

前端覆盖启动记忆过滤、响应丢失后接回扫描/清理且不重放、重载后重新选择、跨明细草稿和保存失败。`scripts/e2e.mjs` 在 debug 专用夹具中验证真实 Explorer 选中文件、排除保存后重扫、保护排除文件及近期文件、再次清理与重载。`scripts/check-reveal.ps1` 只读取资源管理器的选择，用于定位验收。

0.3 阶段截图和结果使用 0.3 后缀；历史验收保留在 verification-0.2-titlebar.md。发行程序 smoke 只读扫描及无效命令拒绝测试，不删除真实缓存。

## 0.4 增量回归

请求协调测试使用 fake timers 和真正永不结束的 Promise，覆盖 15 秒后恢复受理回执、封存未受理命令、10 秒核对失败后拒绝队列中的后续写入，以及读取重试。Rust 控制层使用 Barrier 让命令延后进入控制锁，验证封存后不执行；核心清理使用 Barrier 阻止首项处理，确认停止后零删除。

扫描测试覆盖仅近期文件时的中途进度、取消后不完整状态、异常后待扫描类别的中断标记；明细验证未纳入计划查询与执行结果互斥。前端覆盖启动失败重载、等待反馈、部分成功的结果标题、未纳入计划入口和重扫失败保留结果。

桌面脚本 0.4 在已有专用夹具保护下拦截真实 IPC：扫描/清理已受理后不向界面返回结果，确认自动接回且各只调用一次；延迟一个尚未进入后端的扫描命令，核对后再送达，必须得到 request_expired。另核对结果明细与最小窗口布局。此拦截仅存在测试脚本，不写入发行程序。

输出使用 `desktop-e2e-0.8.json`、`desktop-results-0.8.png`、`portable-smoke-0.8.json`、`portable-readonly-0.8.png`，历史证据保留。测试数量与具体通过结果以 [验收记录](verification.md) 为准。

调试夹具使用独立实例标识 io.dustaway.desktop.fixture；仅在 debug + e2e-fixture 同时满足时生效，发行版仍为 io.dustaway.desktop。

## 0.5 增量测试

`cargo test --workspace --locked` 增加独立分析与 Windows 元数据夹具、多卷报告测试。`pnpm test` 增加空间分析主动启动、读取失败重试、逐层查看、会话定位与不完整状态测试。

`pnpm test:e2e` 继续只在带标记的 debug 专用夹具执行删除，新增真实 100,002 文件分析（十万个小文件与两条共享大文件路径），验证去重、目录定位、任务互斥、重载、取消与文件内容/修改时间保持。`DUSTAWAY_ANALYSIS_SMALL=1` 仅用于开发时缩小夹具，不能用其结果声称完成十万文件验收。

证据输出 `analysis-performance-0.8.json`（扫描、查询、取消耗时与包含 WebView2 的进程资源）、`analysis-light-0.8.png`、`analysis-dark-0.8.png`、`analysis-minimum-0.8.png`，以及完整 `desktop-e2e-0.8.json`。历史证据不覆盖。记录实际环境，不将单一主机结果外推为跨平台性能承诺。

## 0.6 新增验收

先用 source-manifest-0.5.json 核对并归档基线，再全量复跑。前端优先与重构建串行；本机 worker 启动超时时可用 `pnpm test --maxWorkers=1` 全量复核，记录准确命令和结果，超时批次不能计作通过。性能采样期间不运行本项目的其他重任务。

真实浏览器验收：从 Brave 官方发布下载 Windows x64 ZIP 并匹配官方 SHA-256，从 Vivaldi 官方下载稳定版安装包并验证 Authenticode 后仅解包到 artifacts/browser-validation-0.6。运行 `node scripts/browser-validation.mjs`。脚本自行创建带标记的临时根目录和本地 HTTP 测试网站，只允许 ignored 的 Rust 测试桥访问该目录；测试桥不编译进产品。浏览器无头运行、不导入资料、不登录真实账号、不修改默认浏览器。

脚本生成真实 HTTP/代码/GPU 缓存，验证进程运行时不能生成计划、关闭后通过 Engine 生成计划及清理、非缓存文件内容与时间不变、重启后缓存重建及 Cookie/Local Storage/书签保留。输出 evidence.json 的 passed 必须为 true；二进制来源、版本、哈希及签名另存 binaries.json。

自动夹具覆盖标准多配置、缺少标记、非标准配置、保留数据、排除、多硬链接及扫描后变化；通用 Windows 测试继续覆盖 Junction、占用、父链替换、长路径及访问失败。新增前端测试覆盖默认关闭、旧偏好及排除、结构化选择器错误、取消选择、分析跳转和应用建议。桌面脚本验证新增六项默认关闭、键盘切换勾选、最小窗口/明暗主题及原有清理闭环。

真实浏览器验证不等于真实钱包资产、邮件账号或所有版本的产品兼容测试。具体范围及未执行矩阵以本轮验收记录为准。

## 0.7 性能和部分扫描

基准使用 test-only scan_benchmark 模块，绝不随应用发布，删除方法固定拒绝。首先保存 0.6 源码与清单，将相同桥接模块编入两版 Release 测试 EXE，分别保存为 artifacts/performance-0.7/baseline.exe、current.exe；构建使用相同工具链、LTO 和依赖锁。node scripts/scan-performance.mjs --seed 只创建全新的带标记目录，--measure 串行预热并执行五组配对及 20 次取消，--summarize 重新生成汇总。现有夹具和测量不会被脚本静默覆盖。原始数据包含样本顺序、可执行文件哈希、候选集合哈希、内存和句柄采样。

0.6 原始副本保存在 artifacts/baseline-0.6-source，保持清单一致；独立的 artifacts/baseline-0.6-benchmark 仅添加 cfg(test) 模块声明及同一测试桥。两版分别执行 `cargo test -p dustaway-windows --release --lib --no-run --locked --message-format=json`，从 compiler-artifact 的 executable 字段取得测试 EXE。大量排除项场景使用现有设置上限 500 条，并显式设置合法主题；夹具创建与采样分开执行，不允许指向日常缓存。

性能采样期间不运行构建或其他验收。前端模拟测试、真实 Windows ACL 夹具、旧历史反序列化和 debug 桌面 E2E 分别覆盖部分成功、全部失败、取消、确认与历史提示；E2E 使用显式 .simulate-unreadable 标记，真实 ACL 测试不依赖该模拟。真实浏览器验证沿用独立配置与本地网站，不使用日常资料。


## 0.8 排行、用途与性能回归

本轮先核对 0.7 的 219 文件清单，保留完整源码与 zip；实施前全量 Rust 74 / 前端 44 通过，日志分别是 baseline-rust-for-0.8.log、baseline-frontend-for-0.8.log。历史 0.7 性能命令及数据保持存档，不重用其成绩作为 0.8 成绩。

scripts/scan-performance.mjs 的本轮输出改为 artifacts/performance-0.8，夹具继续使用 artifacts/dustaway-perf-0.7 的固定内容与时间；不要再运行 --seed。基线 EXE 从 performance-0.7/current.exe 复制，核对哈希和相同工具链，test-only scan_benchmark.rs 与 0.7 归档逐字节核对。新版本通过 `cargo test -p dustaway-windows --release --no-run --locked --message-format=json` 构建。从 compiler-artifact 取 lib test 可执行文件为 current.exe，串行运行 --measure。所有六类负载耗时及采样内存峰值回退门槛均为不超过 10%，不沿用 0.7 的近期文件提速 50% 门槛。

额外集成测试 `largest_file_query_performance` 默认忽略，只在设置 DUSTAWAY_PERF_ROOT 为已标记夹具时执行 Release 测量。它先完成旧文件夹具的十万文件分析，对无搜索首尾页、有搜索首尾页、搜索无结果各预热一次后采样 50 次；再执行 20 次分析取消。结果在标记目录内生成 analysis-query-0.8.json，不读取日常缓存。P95 使用 nearest rank：50 次取第 48 个，20 次取第 19 个。查询 P95 ≤ 200 ms，取消 P95 ≤ 1 s；查询采样与配对采样、构建、浏览器及桌面验收串行。

桌面脚本从首页诊断入口进入，最大文件搜索深层文件、查看未知用途、返回目录并原生定位，同时验证磁盘令牌与伪造会话拒绝。已存在的清理闭环继续执行，独立验证保护文件、重载接回、迟到响应、互斥、停止、主题和最小窗口。共享指引在两个页面保留挂载，测试原生点击只选择实际显示的同名控件。

单元测试使用合成的已登记路径映射覆盖重定向语义，不修改用户实际 Known Folders。模拟系统设置启动失败验证错误恢复；Windows 设置 URI 成功启动与不同系统版本页面内容另行记录，不将模拟通过当作真实系统页面验收。

## 0.9 应用规则检查

`cargo run -p dustaway-windows --example application_support --locked` 只读识别固定应用；输出含本机显示路径，应仅保留在本地验收目录。`application_scan` 示例只输出六条规则的计数、逻辑字节、保留原因与阻止状态，不输出候选路径、不调用删除。

核心测试覆盖结构化阻止原因、关闭重查、程序身份变化和准备后重启；Windows 测试覆盖文件格式/年龄、QQ 数字配置与标记、标记丢失、未通过验收的删除拒绝和原有目录/硬链接/权限边界。测试身份覆盖仅在 cfg(test) 编译，不能进入发行版。桌面夹具模拟六条规则的运行占用与关闭重查，后端验收门始终关闭，并核对夹具文件内容及修改时间不变。

实际 QQ 缓存重建与登录持久数据、微信/WPS 新日志生成和资料保护仍须在独立真实应用环境验收；当前未执行。已测结果与待测环境在 [0.9 验收记录](verification-0.9.md) 分开记录。

## 0.10 新增检查

`pnpm test:e2e` 构建完成后可执行 `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/startup-failure.ps1`：只创建带专用标记的临时夹具，使历史数据库路径成为不可打开的目录，验证原生错误提示、关闭后退出、文件不变与数据库不被重建。不要将普通 Release 程序传给这个脚本；Release 不接受夹具环境变量。SQLite Full / ReadOnly 初始化失败另由核心真实 SQLite 限额与只读测试覆盖；这些测试不等于把物理磁盘填满。

0.10 的发行冒烟还核对系统卷默认值和各个可用卷采样，并对系统卷、一个非系统卷明确启动短暂只读分析后请求停止。它只读元数据，不创建清理计划或执行删除；零候选与六条只读规则的完整场景继续由专用桌面夹具执行。

Release 对比工具与重现说明在 `scripts/analysis-benchmark/`。使用完全相同的标记夹具、工具链和 harness 分别链接旧版恢复源码与新版源码，交替测量 10 万、50 万文件的完成时间、查询延迟、取消响应与进程私有提交高水位。原始样本保留，不把单机测量外推为整盘速度。

视觉验证区分 CSS 窗口尺寸、浏览器像素密度模拟和真实 Windows DPI。不要用 `html.style.zoom` 放大固定 vh 容器的结果冒充 Windows 缩放；只有真实执行的环境才能写成已验证。人工试用见 [五人观察脚本](usability-0.10.md)。


## 0.11 回归重点

前端测试覆盖待选/结果分离、恢复原位置、同卷基线、失败保留读数、同键刷新保留 DOM/滚动与新查询隐藏旧行、三/八条摘要、可信用途与部分大小。Rust 测试覆盖真实 SQLite 小额页面限制的原子计数、宽目录部分查询、SQL 取消、索引阶段可观察与取消、原生 UTF-16 记录及父目录替换拒绝。

桌面大夹具在项目 artifacts 内创建独立 dustaway-e2e-* 目录，并仅对该测试子进程设置 TEMP/TMP；保留后端“临时目录直接子目录 + 标记 + debug feature”三重检查。发行版不接受夹具提供者。不会全局改临时目录或用个人文件测试删除。

Release 性能方法见 scripts/analysis-benchmark/README.md。本版性能目标、全部失败修正及未验证环境见 verification-0.11.md；人工理解效果须按 usability-0.11.md 另行观察，不能用 E2E 代替。

## 0.12 决策与完成体验

前端以纯决策模型覆盖同一应用的混合状态、部分扫描与失效会话区分、跨会话排除、BigInt 容量、零候选与全部取消选择、新增范围需要扫描、请求不确定及取消/中断报告。集成测试核对导航不改变选择、确认单仅含合法候选、未访问分析页即可查询空间。磁盘测试覆盖启动一次、终态一次、手动刷新、失败保留旧读数、切换卷和迟到响应。

`pnpm test:e2e` 延续专用夹具和 100,002 文件只读分析，新增扫描后改动一项、删除另一项的部分成功流程；同时核对六条只读规则仍不能进入计划。完成后先展开报告或类别再用原生键盘/点击访问，不能通过隐藏控件绕过折叠交互。`artifacts/decision-0.12/` 保存首屏布局、明暗主题和减少动效截图；完整结果分别为 `desktop-e2e-0.12.json`、`analysis-performance-0.12.json`。单机 E2E 的耗时仅作本次观测，没有重复执行 0.11 的五轮配对性能研究。

真实五人试用按 [0.12 观察脚本](usability-0.12.md) 执行；自动化通过不计入参与人数或理解成功率。

## 0.16 用途查找回归

执行 `cargo test --workspace --locked`、`pnpm test`、`pnpm test:evidence`、`pnpm check:recognition`、格式检查、Clippy、构建及专用桌面夹具。新增 `analysis_discovery` 集成测试验证全范围分页、目录聚合、嵌套、十种类别、u64边界、部分/停止/资源限额、旧会话与不可清理性；前端验证筛选、空结果恢复、往返焦点及请求顺序。

性能基线必须使用同一规则库：将归档旧实现恢复到独立目录后，仅为对比复制当前规则文件与匹配器，不修改原归档；记录两份规则和匹配器哈希一致，从而控制同期规则扩充的影响。沿用analysis-benchmark夹具，新harness启用 `--features discovery` 记录用途查询，基线关闭该feature记录共同负载。各五轮交替，不并行运行其他检查；首次查询、p95、实际条数、峰值及取消分别报告。原始数据和实际通过范围见[0.16验收](verification-0.16.md)。

## 0.18 整理台界面

先构建debug夹具：`pnpm build`、`cargo build --locked -p dustaway --features e2e-fixture,custom-protocol`，再运行 `node scripts/e2e.mjs`。测试目录必须有受保护的fixture标记，根目录绑定当前运行时；正式版不允许加载fixture。该流程创建100002个隔离文件，验证全范围排行、分页、目录返回、键盘、明细、单次确认、真实夹具删除、长期保留、请求丢失、只读门槛和部分结果。

发布后运行 `node scripts/desk-release.mjs`，只读检查包内程序的版本、能力、四页及深浅主题和窗口尺寸；不会启动扫描、删除或保存设置。额外Chromium设备模拟覆盖100/125/150%，不改变Windows系统DPI。脚本记录模拟是否可用，不能把不可用当作通过。发布包与夹具任务均需结束后再启动下一项WebDriver任务，避免共用4444端口。

`node scripts/home-smoke.mjs`提供更短的首页/明细/取消确认夹具检查。真实非开发者可用性任务另见[0.18任务](usability-0.18.md)，自动化通过不代替人工可用性。

## 新增应用分组的实际截图

pnpm test:e2e 在完整桌面流程后运行 scripts/application-visual-checks.mjs。后者核对上次 E2E 通过状态、夹具标记/边界与调试程序 SHA-256，只复用专用夹具扫描，并在两种窗口尺寸定位可见应用分组。输出 application-group-layouts.json 和 *-group-*.png；不能用回到页首的 overview 截图代替软件类别截图。可在同一调试构建、完整 E2E 成功后单独执行 node scripts/application-visual-checks.mjs。

# 软件卸载验收记录

## 2026-10-01 钉钉交互卸载：已移除，残留未清理

用户明确选择“卸载钉钉，再处理残留”，此前已确认从托盘退出。目标只限闲置电脑 DESKTOP-24NJTT6，原主程序固定版本 8.2.0.119、SHA-256 `5705DE940C8BF048792AF1E99B55C19F914FC1CA89B6DDB7AB3C4AC0595F69F4`。没有卸载开发机软件或改变安全设置。

- 原普通构建因登记的 `C:\Program Files (x86)\DingDing\uninst.exe` 未加引号而拒绝。首次驱动断言在准备/执行之前失败，`submitted=false`，原证据没有改写。现在只对完整存在、可固定文件身份、无参数且没有可执行竞争前缀的路径作兼容；包装器、参数猜测、缺失、链接及歧义继续拒绝。
- 新普通构建 `AD1A4592C1DCD6D85A045E88A778AB3C97362822F57ED534E181B6B1FF69E888` 于 16:57 通过真实 Tauri 页面“卸载此软件 → 确认卸载”提交一次；用户完成官方交互向导，没有设置静默参数。任务 `b8081baa-afde-48d8-9bd9-f47f938ef54e`，标准状态 `removed`。后续只读检查四注册表视图中原产品键不存在、主程序不存在、相关进程为 0。
- 残留扫描 ID `87491417-1457-4d35-b7f0-5515f1002602`，`complete=false`、`total=0`，警告“无法核对软件签名或安装归属，残留位置已保留”。它不是无残留结果。此前已核验的固定 Default、Cache、Code Cache、Local Storage 目录仍存在。残留实际删除 **0 B**，没有执行清理命令，没有保存候选路径清单或读取资料正文。
- 原因定位：安装目录缺失时，已签名钉钉主程序位于 `main/current`，根目录的 `uninst.exe` 未被原有归属比较覆盖。源码已补仅钉钉、确切主程序名/两层目录/卸载器名的关联；仍先要求应用签名、文件身份与版本识别成功。新增反例覆盖其他产品、祖先、相邻目录、不同磁盘、错误主程序与未知布局。该修复尚无新的卸载前实机验证，不能补发本次未捕获的残留上下文。
- 两项路径回归通过，软件模块定向 **10 通过 / 1 忽略**；路径修复完整 workspace **254 通过 / 24 忽略**（`dingtalk-path-workspace.log`）。后加的目录关联回归另有 **1 项通过**（`dingtalk-owner-regression.log`），不冒称包含在前次全量结果。前端和 IPC 未变，沿用前阶段构建/测试，不重复前端验证。新普通构建恢复摘要见接续记录。
- 原失败与真实卸载共 **9 份原件**已 SCP 下载并逐一核对长度和 SHA-256，位于 `artifacts/home-cleanup-real-20261001/dingtalk-removal-evidence/`。17:03 已关闭该普通测试窗口与其 WebDriver，会话、应用与厂商相关进程均为 0；测试任务 Ready。保留历史和原始证据，自动跟进继续暂停。

本次结果只证明路径兼容修复后的真实标准卸载。钉钉生产残留删除未开放，首页两项缓存权限未转为残留许可。用户要求的“卸载后清残留”尚未完整完成，不用零清单、合成测试或事后证据恢复授权代替实际结果。

日期：2026-09-30。环境：本机 Windows，普通权限 DustAway，原生窗口使用 Edge WebDriver。测试没有卸载本机实际软件。

## 已执行的检查

- 前端生产构建通过；完整前端回归 21 个文件、258 项通过，最后的确认页补充由软件页面的 7 项测试复核通过。
- 软件页面测试覆盖首次进入加载、搜索与排序、读取失败、明确确认且仅提交后端计划、资料默认不选、只读项禁选、未知卸载状态阻止残留检查，以及具体资料选择与独立删除确认。
- 核心软件任务测试覆盖单次计划、列表刷新使计划失效、取消/失败/待重启/无法跟踪/仍有登记、来源读取失败、明确选择与去重、重新安装/排除阻止删除、恢复不回放命令及隐私字段不持久化。
- Rust 工作区测试 188 项通过，5 项原有隔离或性能验收按默认配置跳过。随后补充退出时在途删除的恢复测试，软件模块 8 项复测通过。`cargo clippy --workspace --all-targets --locked -- -D warnings` 通过。
- Windows 文件夹具在临时目录中核对变化文件、新增文件、目录替换、硬链接和长期排除；冻结文件可以通过独立底层执行器删除。另检查注册表视图的安装身份区分、同名不同安装、缺失信息及禁止移除。夹具不授权真实软件规则。
- 现有清理证据脚本 12 项、验收脚本 6 项、桌面驱动脚本 2 项通过；识别目录 256 条规则/510 个模式，以及应用目录 18 条规则投影核对通过。
- 原生只读发现烟测读取 104 个桌面登记、90 个当前用户主应用包，无来源读取告警；没有准备或执行任何真实卸载。
- 最后重新构建了不含 e2e-fixture 的普通调试程序：`target/debug/dustaway.exe`。构建通过；Vite 提示主 JS 包超过 500 kB，不影响本轮功能检查，尚未生成发布安装包。

## 原生界面模拟

`scripts/software-desktop-check.mjs` 使用仅调试可用的 FixtureSoftware，启动后必须检查 `data-runtime=fixture`。模拟卸载只修改内存中的安装状态，扫描返回模拟结果，不能调用系统卸载程序或删除软件资料。

验证 1100×780 和 760×580 的明暗主题，检查真实窗口尺寸及 DOM 边界。验证详情、取消确认后的焦点恢复、键盘进入详情、重新确认、后端任务、自动只读检查、卸载后列表移除和历史筛选。运行输出在 `artifacts/software-verification/verification.json`，截图在同一目录。

## 闲置 Windows 11 设备真实验收：第一轮

2026-09-30，通过已核验主机公钥的 SSH/Tailscale 连接闲置电脑 DESKTOP-24NJTT6。系统为 Windows 11 25H2（26200），WebView2 154.0.4258.37。未修改防火墙、网络类别或 SSH 服务配置。专用样本位于当前用户 LocalAppData 下的 `DustAwayAcceptance/20260930-software`，可执行程序检查电脑名、绝对目录和标记文件后才允许运行。这是独立物理设备上的限定样本验收，未声称拥有虚拟机回滚快照。

SSH 负责调度；实际 EXE/MSI/MSIX 验收进程通过临时交互计划任务在 MSN 的桌面会话 2 中运行，权限记录 `Elevated=false`。调用的是生产 `WindowsSoftware` 和 `SoftwareEngine`，不是 FixtureSoftware；未选择或卸载该电脑已有的软件。

| 项目 | 结果与范围 |
| --- | --- |
| EXE 真实后端流程 | 45 项通过；真实注册/运行独占测试 EXE，覆盖同名不同安装、缺失字段、隐藏/更新过滤、禁止移除、命令包装器/歧义命令拒绝、一次性计划、任务互斥、正常卸载、返回零但仍有登记、启动器提前退出后复核、重装后拒绝残留检查、真实进程退出后恢复且不重放卸载。 |
| EXE 测试数据保护 | 卸载器只移除其测试安装文件，保留测试资料；估计安装大小不计入删除量。不支持的残留检查明确返回 unsupported。样本登记已清理，剩余 0 项。 |
| MSIX 真实后端流程 | 通过：专用签名包当前用户安装 → `PackageManager` 标准卸载 → 包身份消失核验 → 只读残留检查。其他安装身份集合未变化；专用包的 LocalState 测试数据随标准卸载被删除。扫描无残留、无告警、删除统计为 0。系统进度被采样，但这个小包只采到 0，未据此声称中间百分比已充分验证。 |
| MSIX 临时证书 | 开发机只生成短期测试签名证书，未导出私钥，签名后删除本地证书及私钥。闲置电脑临时信任的公钥证书已移除；测试 MSIX 已不再安装。 |
| MSI 真实后端流程 | 已核验真实移除及中断后的恢复核对。登记原位于 HKLM 32 位视图，通过系统 `msiexec /x ... /norestart` 启动。测试驱动在 90 秒后停止跟踪；21:26 +08:00 Windows Installer 日志记录移除成功（1034/11724，状态 0），21:29 核对原卸载登记已消失、产品状态为 INSTALLSTATE_UNKNOWN、测试组件已移除。没有结束或重新启动原卸载器。恢复任务由 Unknown 核对为 Removed，并拒绝恢复已丢失的残留删除授权。全程交互/无中断跟踪尚未通过。 |
| UAC 取消 | 通过：用户在系统提示中点击“否”，普通权限、会话 2 的实际驱动收到 Cancelled；卸载器未启动，安装登记保留至样本清理，残留检查被拒绝。经用户授权临时开启管理员提示，测试结束后已恢复原值；两个临时 UAC 任务和样本登记已清理。 |
| MSI 人工交互 | 用户确认没有窗口；复核表明原 MSI 卸载已结束。此前远程桌面窗口两次截图失败（FrameArrived/window capture timeout），不能据此推断安装器当时停在确认窗口。 |

证据位于 `artifacts/software-remote-acceptance/evidence/`：`exe-acceptance.json`、`execution-context.json`、`msix-acceptance.json`、`acceptance-status.json` 和 MSI 诊断日志。初始 EXE 报告的 scope 文本写有 SSH session；实际执行会话与普通权限以单独的 execution-context.json 为准。初始 package-acceptance-summary.json 保留首次 MSI 样本构造失败，不代表后来安装结果；最终状态以 acceptance-status.json 和对应阶段记录为准。

后续 MSI 核对证据为 `msi-recovery-verification.json`、`msi-system-events.json`、`acceptance-status-rechecked.json`，后者取代先前的 pending 状态。只读恢复核对通过 SSH 执行，不重放卸载；原卸载仍由普通用户会话启动。专用 MSI 进程数为 0，EXE/MSIX 测试登记、临时签名信任和两个临时计划任务均已清理。新建的 `software_verify_recovery` 工具不包含启动卸载的路径。

21:35 +08:00 的 `uac-policy-observed.json` 记录 `EnableLUA=1`、`ConsentPromptBehaviorAdmin=0`、`PromptOnSecureDesktop=0`。根据[微软 UAC 设置说明](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/user-account-control/settings-and-configuration)，管理员策略 0 表示不询问直接提权，因此当时无法验证用户取消。首次启动检查在创建任务、样本登记和卸载日志前停止，没有运行卸载器或改变 UAC 设置。

用户明确授权“允许，测试后恢复原设置”后，21:40:34 +08:00 独立控制任务仅将 `ConsentPromptBehaviorAdmin` 从 0 改为 5；实际卸载驱动仍由另一个普通权限任务运行。用户点击“否”后，驱动核验取消、卸载器未启动、原登记仍在及残留检查被拒绝；控制任务于 21:40:46 恢复该值为 0。21:42 最终复核三个策略值均与基线一致，临时 UAC 任务已删除、样本登记剩余 0 项，样本安装文件仍保留。未修改其他安全设置或防火墙。证据为 `uac-acceptance.json`、`uac-execution-context.json`、`uac-controller-result.json`、`uac-temporary-prompt-state.json`、`uac-policy-during-test.json` 和 `uac-final-status.json`。UAC 人工响应等待上限单独延长至 600 秒，超时不结束卸载器；对应驱动构建、Clippy 和格式检查通过。

### 真实环境发现并修复

1. 某个商店包的 DisplayName 返回 `0x80073B1F`（未找到 ResourceMap），导致整个商店列表读取失败。改为显示包身份名称，发布者资源缺失时保留空值；身份枚举与卸载核验仍严格执行。修复后读取 121 个当前用户主应用包，无来源告警。
2. 系统 msiexec.exe 的 Windows 维护硬链接被通用清理文件保护拒绝。新增仅限 `GetSystemDirectoryW()/msiexec.exe` 的只读启动核验，保持父目录和文件身份锁定，不请求写入或 DELETE 权限。普通 EXE、普通清理和残留删除的硬链接拒绝边界不变。

本轮 Rust 完整工作区 190 项通过、5 项原有用例忽略；随后 MSI 专用核验修复的软件模块 7 项复测通过（含普通硬链接仍被拒绝）。最终工作区全部目标 Clippy `-D warnings` 和不带 e2e-fixture 的普通桌面构建通过。前端没有本轮改动，沿用前述已通过的构建和界面回归，不将未运行的远程 UI 项目记为通过。

## 闲置设备真实验收：第二轮

仍使用普通用户会话 2，未调整 UAC、防火墙或网络设置。仅新增两个产品代码不同的专用 MSI，不重放第一轮任务。

- 商店策略：只读比对 121 个当前用户主应用包，45 个系统组件及 2 个非系统签名但 `NonRemovable=true` 的包均不可卸载，准备接口也拒绝受保护项目；没有调用这些应用的卸载接口。证据为 `store-policy-acceptance.json` 和 `store-policy-context.json`。
- 由实机检查发现仅检查系统签名不足，已补充固定的系统 PowerShell/Appx 只读策略查询，按完整包身份匹配。缺失、重复、无效或无法读取的策略不授予卸载权限；准备和实际执行前重新核验。查询只读取当前用户主包，限时 30 秒、输出最多 8 MiB，不接受前端命令或路径。
- MSI Completion（产品代码末尾 0931）：真实安装、人工确认、持续跟踪至安装登记消失、只读检查通过。Windows Installer 的确认框只显示通用文字；先核对唯一桌面安装器进程的产品代码及父进程，再由用户确认。该专用样本无残留规则，结果正确显示“不支持检查”，实际删除量为 0，其他软件安装身份集合未变化。证据为 `msi-completion-acceptance.json`、`msi-completion-terminal-task.json` 和 `msi-second-round-context.json`。
- MSI Restart（产品代码末尾 0932）：通过。人工确认后，样本在移除时执行 ScheduleReboot，生产 `/norestart` 与包内 ReallySuppress 保证未实际重启。后端进入 RestartRequired；立即复核、残留扫描，以及重新打开任务数据库后的复核与扫描均被拒绝。样本安装登记和独占组件已移除，其他软件身份未变化，估计大小未计入释放量。证据为 `msi-restart-acceptance.json` 和 `msi-restart-terminal-task.json`。这验证的是抑制重启的待重启路径，不代表实际重启后的结果核对已实测。

22:07 +08:00 两个 MSI 用例均通过，系统启动时间未变化；新增样本产品与卸载进程均剩余 0，两个临时任务已删除。UAC 三项策略仍为原始 1/0/0，生产残留删除开关仍关闭。完整收尾证据为 `msi-second-round-result.json`、`msi-second-round-events.json`、`second-round-final-status.json`。

本轮完整 Rust 工作区回归 193 项通过、5 项原有用例忽略；全部目标 Clippy `-D warnings`、格式检查和不带 e2e-fixture 的普通桌面构建通过。新增回归覆盖非系统包禁止移除、未知策略、包身份变化，以及缺失、错误类型和冲突策略的拒绝。前端没有本轮改动。

## 闲置设备真实验收：第三轮

- 商店 API 错误：普通用户调用生产共用的私有 PackageManager 移除函数，目标为已核对不存在的固定 Missing 包身份，得到真实 `0x80073CF1`，没有误报成功。前后安装身份集合一致。生产入口仍先检查存在性和策略；这个用例验证 API 错误边界，不表示生产入口可跳过身份核验。证据为 `store-api-failure-acceptance.json`、`store-api-failure-context.json`。
- 原生页面与 IPC：在闲置电脑普通用户会话运行明确标记为 e2e-fixture 的程序，模拟软件卸载，未调用真实卸载器。760×580、1100×780 的明暗主题布局、详情、取消确认后的焦点、键盘操作、完成后的列表与历史均通过；已查看小尺寸浅色及大尺寸深色截图。
- 回执丢失与重连：测试只丢弃一次原生卸载受理后的前端响应，经历真实 15 秒超时后由受理回执恢复。重复请求返回同一任务，前端只发起一次卸载，历史只有一条。进行中刷新被拒绝，切换页面仍可见任务；重新加载 WebView 后保持同一完成结果。证据和截图在 `artifacts/software-remote-acceptance/evidence/desktop-third-round/`。
- 原生退出提示：启动了最长 180 秒的模拟任务，准备让用户先取消退出、再确认退出。等待人工确认 120 秒超时，测试程序已结束，该项**未通过验收**。只结束了专用模拟程序，没有真实卸载器。失败记录为 `software-exit-result.json`、`software-exit-failure.txt`，不得用前述 WebView 重载成功替代正常退出恢复验收。

22:28 +08:00 收尾确认测试进程为 0、4444/4445 测试端口均关闭、三个临时任务已删除；UAC 值仍为原始 1/0/0，残留删除开关保持关闭。汇总为 `third-round-final-status.json`。

本轮软件页面与协调器前端回归 11 项通过，Windows 软件模块 9 项通过，专用 Missing 包用例另在闲置电脑通过。完整 Rust 回归首次触发原有空间分析测试的时序问题：固定等待 30 毫秒后取消，繁忙机器可能尚未产出文件。测试夹具已改为产出文件后等待取消，主测试观察真实进度再取消并核对 cancelled 状态；未改变生产分析逻辑。该测试集 4 项复测通过，随后完整工作区 193 项通过、6 项默认忽略（其中 1 项为已在闲置电脑单独执行的 Missing 包用例）。默认配置及 e2e-fixture 的全部目标 Clippy、格式检查、普通桌面构建均通过。普通调试程序已恢复，不将模拟构建作为交付程序。

### 原生退出提示复测

22:38 +08:00 在新的 `desktop-exit-retest` 目录启动普通权限模拟程序。用户依次点击“否”和“是”：取消退出后，脚本核对相同任务仍处于 Uninstalling；确认退出后进程结束，重新启动读取相同任务 ID、Unknown 状态，等待后状态未自动推进，历史只有一条。

首次复测的最后一个页面断言超时，原始退出码 1 和错误日志保留。原因是测试把正常退出保存的“拂尘已停止跟踪”误写为异常中断恢复时的“上次跟踪已中断”；上述任务断言已经通过，不能把这个脚本的整体退出码改记为成功。已将页面检查改为显示后端保存的实际消息，并校验停止跟踪语义、“重新核对安装状态”可见及“检查残留”不可用；同时移除与任务状态无关的 WebView 焦点等待、延长第二次人工响应等待，每次新用例清空旧的人工确认标记。

22:41 +08:00 使用 `--recover-existing` 只重新打开同一标记目录和同一待核对任务，没有再次发起卸载或人工提示。恢复、唯一历史记录、页面文案和禁止残留检查均通过，退出码 0；已查看恢复截图。该恢复报告明确标记 `recoveryOnly=true`，不单独冒充两次人工点击的端到端报告。两阶段证据共同补齐本轮模拟软件的正常退出恢复验收；没有调用真实卸载器。

证据在 `artifacts/software-remote-acceptance/evidence/exit-retest/`：`software-exit-retest-result.json`、`software-exit-retest-error.log`、`software-exit-recovery-verification.json`、`software-exit-recovery-result.json`、`software-exit-recovered.png`。22:43 +08:00 的 `exit-retest-final-status.json` 确认测试进程为 0、4444/4445 端口关闭、两个临时任务删除，UAC 保持 1/0/0，生产残留删除仍关闭。本次仅修改测试脚本及文档，Node 语法和脚本格式检查通过；没有重新运行或冒充重跑前述完整前后端回归。

### 实际重启后的结果复核

22:49 +08:00 在闲置电脑普通权限、会话 2 执行 Restart 样本预检。核对产品代码末尾 0932 的 MSI 产品与原卸载登记均不存在，但原任务仍为 RestartRequired；系统尚未重启时，重新核对和残留扫描仍被拒绝。原任务 ID 与第二轮终态证据一致，没有重新启动卸载。预检退出码 0，临时任务已删除，证据为 `msi-restart-preflight-context.json`、`msi-restart-preflight-result.json`、`msi-restart-preflight-verification.json`。

新增复核工具模式与普通权限运行脚本已准备。核心软件测试 9 项通过，其中新增重启恢复回归分别核对重启前、启动状态不可读、重启后已移除、重装及安装来源读取失败，并保证不重复卸载、不重建残留授权。专用工具构建与 Clippy `-D warnings` 通过。此时尚未执行实际系统重启，后续结果须以单独证据为准。

用户选择自行重启并重新登录。实际启动时间从 9 月 26 日 12:56:20 更新到 9 月 30 日 22:52:56（+08:00）。22:56 在普通用户、会话 2 运行已有任务的复核，工具哈希与预检一致。任务 `98947ad0-2580-4924-8318-813b0c7144f2` 从 RestartRequired 变为 Removed；原卸载登记不存在，Windows Installer 产品状态为 INSTALLSTATE_UNKNOWN。历史仍只有原记录，没有重新调用卸载，登记估计大小未计入释放量，实际删除为 0。尝试残留扫描被拒绝，恢复不能重建已丢失的内存授权。真实重启后的结果复核通过，退出码 0。

证据为 `msi-after-restart-context.json`、`msi-after-restart-result.json`、`msi-after-restart-verification.json` 及运行日志。收尾脚本首次因 Windows PowerShell 5.1 默认编码误读 Rust 写出的 UTF-8 JSON 而停止，未重新运行结果核对；为相关读取指定 UTF-8 后完成收尾。22:58 的 `after-restart-final-status.json` 确认测试进程为 0、测试端口关闭、临时任务删除，UAC 保持原始 1/0/0，生产残留删除开关仍关闭。未修改产品代码，本轮不重复前述完整回归。

### Firefox 实际软件验收

确认闲置电脑没有 Firefox 登记，Local/Roaming Mozilla 目录均不存在后，使用本机代理从 Mozilla 官方下载 Firefox ESR 140.17.0，验证有效的 Mozilla Corporation 签名及 SHA-256 `DF6F7046D53FF17403E49092C0FE830E24CABD8DACBFAD3225870955908FCCF9`。以普通权限安装到专用验收目录，建立 `DustAwayAcceptance20260930` 新配置，只访问本机回环测试页面，未登录账号。

首次生成资料的脚本未等待 Windows 启动器背后的浏览器，提前关闭测试服务器而失败；仅停止这一轮失败的专用无界面浏览器，不涉及卸载器。补充 Mozilla `-wait-for-browser` 参数后生成成功，原失败日志保留。真实缓存、测试 Cookie 和本地存储均已生成并读取。

首轮标准卸载启动器提前返回，测试误以为此时应已删除安装登记而失败。发现实际 NSIS `Un_A.exe` 仍运行后，没有再次启动卸载，由新的核对进程准备并跟踪原安装身份，用户完成原向导。随后整个 Roaming Firefox 目录的哈希比较失败，测试在任何残留删除之前停止；因当时未保存文件级差异，不能推断具体变化内容。使用同一官方版本重新安装后，原 Cookie 与本地存储均可读取，这一项通过。

第二轮增加官方卸载前后文件级差异，并等待实际 NSIS 进程退出。23:28 +08:00 用户完成向导后，原登记和程序文件均消失，其他软件身份集合未变化；卸载前捕获的范围成功用于生产只读扫描，候选均不可选择，生产删除调用被拒绝，长期排除、变化文件和占用文件的删除保护通过。随后测试错误地要求独占内容句柄必须使属性扫描告警，导致退出码 101，在任何真实缓存删除之前停止。原报告与错误日志原样保留。生产扫描只读取属性，本次实测可以读取被占用文件的属性，而删除正确失败；这不属于删除保护失效。

本轮官方卸载器的文件级差异只有 `Background Tasks Profiles` 中默认代理配置的 `prefs.js`，见 `firefox-official-uninstall-diff.json`；不能用这个结果倒推首轮未记录的差异。缓存保护比较以官方卸载结束后的完整 Roaming Firefox 清单为基线。

修正测试后没有再次启动交互卸载。23:36 +08:00 在普通用户、会话 2 运行独立的 `real_firefox_residual_supplement`：核对原失败类型、卸载后的安装清单、固定新配置标记与文件哈希，再以**新建的测试专用上下文**重新冻结候选，未恢复原生产任务授权。这个补测使用从未登记的合成扫描身份，没有可执行命令，也不调用卸载。扫描得到 182 条记录，生产候选及删除入口仍受关闭规则保护；只通过内部测试入口删除一个真实缓存条目，实际 16,563 字节。资料哈希和共享父目录见证文件未变；长期排除、变化、占用、硬链接、新增文件、非空目录保护，以及硬链接触发的部分扫描告警、停止扫描均通过。补测退出码 0，不把原完整用例的 101 改记为成功。

证据为 `firefox-residual-second-result.json`、`firefox-residual-second-error.log`、`firefox-official-uninstall-diff.json`、`firefox-residual-supplement-context.json`、`firefox-residual-supplement-result.json` 和 `firefox-residual-verification.json`。报告明确区分阶段及上下文来源，生产规则仍未获开放依据。测试入口仅存在于测试程序中。Windows 软件模块 9 项回归、Clippy `-D warnings`、格式及编译检查通过，3 项实机测试默认忽略，其中补测另在闲置电脑执行通过。本轮未改变产品逻辑，没有重复前述完整前后端回归。

23:37 +08:00 使用同一已核验的 Firefox ESR 140.17.0 安装包重新安装。安装前、安装后打开浏览器前，逐一核对标记用户配置的文件哈希；再打开本机回环页面，原 Cookie 和本地存储均可读取，截图与事件记录保存。随后通过官方卸载器的 `/S` 完成测试安装收尾，程序文件消失，退出码 0；静默收尾不作为交互卸载验收。证据为 `firefox-reinstalled-verification.json`、`firefox-reinstall-verification.json`、`firefox-reinstall-result.json` 和 `firefox-reinstalled.png`。这证明本轮单个缓存文件处理后的测试资料可用性，不代表真实账号登录状态、多版本迁移或完整生产页面闭环已验收。

23:39 +08:00 收尾确认 Firefox 安装登记与测试进程均为 0，19837 回环端口已关闭，8 个临时计划任务已移除。新配置和测试见证文件保留用于证据核对；UAC 三项策略仍为原始 1/0/0，没有修改防火墙或网络配置。证据包为 `artifacts/software-remote-acceptance/evidence/firefox-acceptance-evidence.zip`，解包记录在同级 `firefox-acceptance/`；`firefox-final-status.json` 明确保留原完整用例未通过，仅补测与重装的有限范围通过。

### Firefox 普通桌面版页面流程

23:48 +08:00 准备 Firefox 普通桌面版页面复测。前端构建与不带 e2e-fixture 的普通桌面构建通过；程序 SHA-256 为 `6A033A9785E3D403F3B5FCADA42B9F68ED853EB6A927F340BEBCB6DC299B034B`。在闲置电脑会话 2、普通权限安装同一测试版本后，真实页面展示唯一目标及卸载确认框；身份、版本、固定安装路径和 Mozilla 签名已核对。此时测试停在 `ready_for_confirmation`，尚未提交卸载。构建保留既有“大于 500 kB 的前端分块”提示，未出现构建错误。

23:51 +08:00 拂尘确认框在驱动开始信号之前已被确认，原向导启动。后台发现准确指向专用安装目录的 `Un_A.exe` 后，没有写开始信号，也没有再次点击卸载。原普通版任务 `fad47b25-0400-44da-9b3f-557099b9e25f` 为 StillInstalled，页面说明启动器已退出但仍有安装登记。用户随后完成官方向导；普通用户预检确认进程、程序文件、四个注册表视图中的 Firefox 登记均消失。

原驱动仍等待开始信号，因此没有将其记为通过。另一个只核对现有任务的页面脚本连接**同一个 WebDriver 会话和仍在运行的普通版拂尘**，没有重启应用、重建残留权限或调用卸载。先核对后端原始 `execute_uninstall` 成功回执及任务 ID，再通过真实页面点击“重新核对安装状态”，自动只读检查进入 ResidualsReady。处理前后 runtime ID 均为 `32b7bdee-3f17-49b6-89df-e37dcbc06d81`，卸载前捕获的真实上下文仍保留在原进程内。

23:56 +08:00 接续检查退出码 0：193 条残留记录全部只读且未选，清理按钮不可用；硬链接见证触发部分检查告警，页面明确显示“残留检查未完成，部分位置已保留”。路径筛选与历史页通过；原任务只有一条历史，历史安装路径脱敏为 null，实际残留删除 0 字节。检查前后配置和缓存文件哈希、共享父目录见证文件均未变，接续期间安装身份集合未变。已查看残留与历史截图。没有测量完整卸载期间的所有请求次数，也没有保存原驱动内存中的卸载前完整身份清单；本轮不另声称这两项通过。

原脚本尝试替换 Tauri `invoke` 以计数，但实机属性为不可写、不可配置且无 setter，所以该计数无效，不能用 0 推断没有请求。后续脚本改为与既有可靠性测试相同的 IPC fetch 观察方式，并在等待人工期间识别已出现的原任务，避免再次点击确认。改动通过 Node 语法及格式检查，未为重跑计数测试再次卸载。此次实际页面流程由初始页面、原后端回执、人工向导、同一运行实例的接续核对和历史证据共同组成，不冒充单个自动化脚本从头到尾成功。

23:59 +08:00 已关闭无活动任务的测试应用，结束仍在等待开始信号的旧驱动（原退出码 -1 保留），删除 2 个临时计划任务。测试进程为 0，4444/4445/19837 端口关闭，UAC 仍为原始 1/0/0，防火墙未改动。测试配置和原历史保留；所有生产残留删除权限仍关闭。截图及接续记录在 `artifacts/software-remote-acceptance/evidence/firefox-desktop-real/`，收尾为 `firefox-desktop-final-status.json`。

本轮归档为 `artifacts/software-remote-acceptance/evidence/firefox-desktop-real-evidence.zip`，SHA-256 `B30B8EB493964C614F857EF612D57F3FA5AE7BD0A094833FC30278B590B46B57`，包含原准备状态、人工确认观察、接续前后快照、文件哈希基线、页面截图、原退出码与最终收尾。仅修改验收脚本和文档，未改产品逻辑；Node 语法、脚本格式与 PowerShell 语法检查通过。

## 2026-10-01：恢复状态与残留清单一致性

本轮修正产品逻辑。恢复后的任务明确标识内存范围已失效，核对为已移除后不再诱导自动残留扫描；EXE 启动器提前退出留下的 StillInstalled 任务可跨进程继续核对，不重复执行。每次扫描生成独立清单 ID，前端同步废弃旧分页、选择及确认，拒绝迟到的旧扫描和计划响应。全只读残留显示保留说明；扫描、清理开始时在任务锁内重置取消标识，避免刚受理的停止被覆盖。

新增字段仅描述当前进程状态，历史固定保存 false/null，旧 JSON 缺字段仍可读取。未放开任何真实软件删除规则，也未增加持久化目录或命令。

针对性验证：核心软件任务 12 项通过；软件页面 11 项通过，包含同一秒内完成重扫、旧响应晚于新响应、迟到清理确认、恢复后无残留授权。首次前端测试在 forks 工作进程启动时超时，没有执行用例；改用单工作线程后完整执行这些用例并通过。前端生产构建通过，仍有既有主 JS 分块大于 500 kB 的提示。IPC、使用说明和用户手册已同步更新。

此处是新源码的本地回归记录。上文 Firefox、MSI、MSIX、UAC 等实机证据对应此前构建，不宣称新构建重新完成了真实卸载验收。

全量前端使用 `pnpm exec vitest run --pool=threads --maxWorkers=1`：21 个文件、263 项通过。现有清理证据、验收及桌面驱动脚本合计 20 项通过；识别目录 256 条规则、510 个模式核对通过。Rust 格式与修改的前端文件格式检查通过。

`cargo test --workspace --locked`：197 项通过、0 失败，8 项既有实机或性能验收按默认配置忽略，文档测试通过。编译后的 18 条应用规则投影核对通过；普通清理和自选删除的既有回归均包含在本轮工作区测试中。

`cargo clippy --workspace --all-targets --locked -- -D warnings` 通过。

00:35 +08:00 原生模拟复测 `node scripts/software-desktop-check.mjs --reliability` 通过：1100×780、760×580 的明暗主题无横向溢出；取消确认后的焦点恢复、键盘操作、跨页任务状态、丢失回执后核对、重复请求返回同一任务、WebView 重连保持原任务均通过。新增检查确认同一卸载任务重扫生成不同清单 ID，分页响应与任务清单一致；唯一历史记录的扫描能力为 false、清单 ID 为 null。已查看最小窗口的浅色列表和深色结果截图。

证据为 `artifacts/software-verification/verification.json` 和同目录截图。提供程序为 synthetic fixture，`nativeUninstallExecuted=false`，未卸载本机真实软件。生产残留删除规则继续关闭。

模拟结束后，`cargo build -p dustaway --locked --features custom-protocol` 通过，`target/debug/dustaway.exe` 已恢复为不含 e2e-fixture 的普通调试程序。未生成发布安装包。

## 2026-10-01 夜间：确认失效与开始前写盘失败

新增六项核心回归先复现失败，再修复：重新准备卸载失败、残留选择无效、软件列表刷新失败均使旧确认不可执行；新卸载历史写入失败保留原任务并废弃本次确认；扫描和清理开始前写入失败不进入没有工作线程的永久进行中状态。使用测试数据库触发器拒绝写入，断言未启动新卸载/扫描/删除；修复存储后必须重新准备或重试相应步骤。

`cargo test -p dustaway-core --locked`：115 项通过、0 失败，2 项性能验收默认忽略，其中软件任务 18 项通过。`cargo clippy -p dustaway-core --all-targets --locked -- -D warnings` 与 Rust 格式检查通过。本小轮仅更改核心逻辑及回归，没有重跑前端或原生流程，也没有更新普通桌面二进制；上文完整工作区和原生记录仍对应此前版本。后续阶段必须补齐整体验证。

01:04 +08:00 通过既有严格主机指纹校验的专用 SSH，只读确认闲置电脑的 SSH/RDP 服务运行、RDP 已开启、标记测试范围匹配，当前无测试应用和测试端口监听。证据 `artifacts/software-remote-acceptance/evidence/night-20261001-health.json`。未修改防火墙、安全策略、应用或资料。完整需求、当前缺口及下一步见 [持续开发记录](software-uninstall-next.md)。

## 2026-10-01 夜间：停止跟踪与残留中断统计

修复后台操作不绑定任务的问题。先用受控阻塞的平台复现四种旧错误：停止跟踪后卸载结果改写为 Removed、扫描发布新清单、清理改写为 Finished，以及平台 panic 后一直显示进行中。修复后按任务 ID 与操作标识回写；退出清除标识、计划及残留范围并拒绝新操作，迟到结果不能覆盖中断记录。线程启动失败与原生调用 panic 转入 Unknown，不回放操作。

新增删除前的持久化数量意图，只有保存成功才调用文件操作。每次确认结果后再保存累计统计，保存失败停止后续处理。恢复只把在途的一项计为待核对，其余未开始的选择计为 `unprocessed`；保存内容不含路径、候选 ID 或可回放清单。主动停止、意图写入失败、结果写入失败、删除前/间隙/完成后崩溃边界均有回归；旧记录继续按默认值读取。页面与历史显示未处理数量，待核对任务也保留实际统计；同一秒内结束不再漏刷历史。

截至 01:29 +08:00：软件任务测试 25 项、软件与历史页面专项 14 项通过；`cargo test --workspace --locked` 共 210 项通过、0 失败，8 项专用实机/性能验收默认忽略。普通清理、手动删除与 Windows 文件保护回归包含在内。核心全目标 Clippy `-D warnings`、Rust 格式检查通过。

前端构建通过，仍有既有主 JS 分块大于 500 kB 的提示；完整前端 21 个文件、274 项通过。首次完整测试在数分钟未输出完成结果后被主动终止，不能算通过或据此断言产品失败；随后增加逐项进度日志重跑，239.86 秒完成全部测试，未修改其他模块来绕过检查。证据/验收/桌面驱动脚本 20 项、256 条识别规则和 510 个模式、18 条应用规则投影核对通过。

本轮没有执行真实卸载或残留删除，没有生成发布包。原生桌面驱动当时被其他任务的布局测试使用，未终止或接管其进程，未将旧原生截图计为本轮新验证。所有生产残留删除规则仍关闭；还需单独构建并验证本轮对应的普通桌面程序。持续开发账本保留这些后续项。

## 2026-10-01 夜间：独立残留规则与上下文第二版

新增 93 条独立残留声明，将缓存、设置和本地资料分别绑定规则 ID、规则版本、范围摘要、核验软件版本与证据摘要。卸载前冻结每个明确范围，扫描与删除再次核对；未知后代的 Cache/logs 名称不再决定类别或权限。同系列另一安装登记会阻止残留处理。钉钉观察范围修正为 LocalAppData，浏览器配置识别提取为共享只读函数，普通清理权限未扩大。微信/WPS 设置与资料及真实规则验收仍缺失，生产许可表为空。

最终 `cargo test --workspace --locked` 共 219 项通过、0 失败，8 项专用实机/性能测试默认忽略，包含新增 9 项独立规则、配置位置与同系列重装回归。完整日志为 `artifacts/software-verification/residual-v2-workspace.log`。工作区全目标 Clippy `-D warnings`、Rust 格式检查与前端构建通过；前端未改源码，本阶段未重跑上一阶段的 274 项前端测试。构建仍有既有 JS 分块提示。

01:58 +08:00 独立复制的模拟程序通过原生可靠性复测：两种窗口尺寸、明暗主题、键盘与焦点恢复、重复请求、超时核对、跨页状态、清单替换、历史和重连。证据为 `artifacts/software-verification/residual-v2-native-20261001/verification.json`。提供程序为 synthetic fixture，`nativeUninstallExecuted=false`，模拟残留扫描为空；该结果不验证真实残留选择、删除或生产规则授权。最小窗口的列表和结果截图已查看。模拟程序 SHA-256 为 `2AD5447A4480BCC6746F2FB9992F3554F559173BAF9F84564CDE13BF90A003BE`。

普通调试程序随后已重新构建，未包含 e2e-fixture；`target/debug/dustaway.exe` 的 SHA-256 为 `9F84F6F68BD55E403D4A8D7F4FBC834B00FABB14A4C8F26B800AAB437D2A3EAD`。未生成发布安装包。01:54/01:57 的远端只读检查确认专用 Firefox 已卸载且测试配置、缓存和安装程序仍存在；证据 `residual-v2-readiness.json`、`residual-v2-fixture-state.json` 不构成新规则删除验收。具体规则声明与缺口见 [独立残留规则](software-residual-rules.md)。

## 2026-10-01 夜间：刷新隔离、独立结果与残留确认界面

连续刷新时，较早读取的成功结果可能晚于新结果返回，并覆盖新列表或在新读取失败后恢复旧软件 ID。两项受控并发测试在旧实现上失败；加入后台请求代次后，旧返回不能修改列表或最新确认。此变更不新增前端可提交参数。

新增兼容的 `uninstall_status`，将标准卸载结果与当前阶段分开持久化。残留扫描/删除中断、退出、写盘失败及恢复保留已核验的移除结果；标准卸载本身中断仍为未知。旧清理阶段可依据原状态推断已移除，旧未知记录继续未知。软件页与历史分别显示标准卸载结果、残留结果及实际删除统计，不因已移除而恢复残留权限。取消残留确认后，键盘焦点返回触发按钮；触发按钮已不可用时转向任务区域。

完整回归：前端 21 个文件、276 项通过；Rust 工作区 222 项通过、0 失败、8 项专用实机/性能验收默认忽略。日志分别为 `artifacts/software-verification/status-refresh-frontend.log` 与 `status-refresh-workspace.log`。工作区全目标 Clippy、启用 e2e-fixture 的桌面全目标 Clippy（均 `-D warnings`）、Rust/前端格式检查及前端构建通过。仍有既有 500 kB JS 分块提示。

原生模拟首次覆盖非空残留：缓存文件、父目录、设置文件和只读共享项全部默认未选；重扫清除旧选择；明确选择两份文件及一个父目录后，确认页说明资料影响和永久删除，取消不执行且恢复焦点。再次确认后计为 2 份文件、1 个空目录、1,280 字节，不包含软件登记的 10,240 字节。重扫只剩未选的只读共享项。两种尺寸与明暗主题下，列表和残留布局均无横向溢出；确认框、小窗口截图已检查。重复请求、超时核对、跨页状态、WebView 重连和历史也通过。

证据：`artifacts/software-verification/status-refresh-native-20261001/verification.json`。提供程序为 synthetic fixture，所有残留操作仅更新内存；`nativeUninstallExecuted=false`、`realFileDeletionExecuted=false`。本轮不是实际文件删除、产品目录归属或整机重启验收，不填充生产残留许可表。

02:20:55 +08:00 普通调试程序已恢复（仅 custom-protocol，无 e2e-fixture），SHA-256 为 `E26F8720D27D321B269C346FAB979029E76FE8A1C129B6A72DD6F86FA11647E6`。模拟副本 SHA-256 为 `D005201BBC3C89DC3E921A5A42EC25391B6449C5DC957AF91422173061F76842`。源码摘要、构建和验证汇总见 `artifacts/software-verification/status-refresh-checks.json`。未生成发布安装包。

02:14 +08:00 远端只读检查确认 RDP/SSH 服务运行、RDP 已启用、桌面会话 2；没有新建 RDP 会话或修改安全配置。记录为 `artifacts/software-remote-acceptance/evidence/rdp-readiness-20261001-021417.json`。

## 2026-10-01 夜间：Firefox 第二版范围实机预检查

在原带标记的专用安装/配置位置重新安装已核验的 Firefox，驱动运行于普通用户会话 2。新的 `software::firefox_scope_acceptance::real_firefox_v2_scope_preflight` 只调用发现、准备和只读核验，未调用卸载或删除。实际签名程序的 PE 版本为 `140.17.0.1158`；cache2、prefs.js、user.js、cookies.sqlite、places.sqlite、storage、bookmarkbackups 共 7 个范围与各自规则 ID、摘要、目标类型一一对应并再次核验身份。sessionstore-backups 不存在，未捕获且未计入通过。共享父目录和任意后代 Cache 名称不获得分类，仍安装的软件不能进入残留扫描。

真实 Firefox 通过仅回环可访问的测试网页核对原测试 Cookie 和本地存储。最终脚本的浏览器与范围测试退出码均为 0，新实机测试显式运行 1 项通过。该证据仅覆盖卸载前范围识别，不覆盖卸载后的范围保持、真实残留选择/删除或资料保护；生产许可表保持为空。报告不保存 PreparedSoftware、执行命令或文件身份授权，后续卸载必须重新准备上下文。

失败记录完整保留。首先，PowerShell 重定向子进程在 `WaitForExit()` 后返回 null 退出码，外围脚本将它误判为浏览器失败；固定的退出零诊断在同一设备重现该现象，`Start-Process -Wait -PassThru` 返回 0，脚本已改为后者。其次，实机测试最初把 PE 版本末段写为 0，遇到实际 1158 后正确拒绝；只修正测试样本的预期版本，没有改变生产版本匹配逻辑。两次接续都复用既有安装，没有重复安装或卸载。

证据位于 `artifacts/software-remote-acceptance/evidence/`：`firefox-v2-scope-preflight.json`、`firefox-v2-scope-version-corrected-check-result.json`、`firefox-v2-scope-version-corrected-retained-verification.json`、`firefox-v2-child-exitcode.json` 和各首次失败日志。报告摘要 SHA-256：`288D0A016DF988EACE7A2A9E29B59652016581F387DE14369D498757B823522E`；测试程序 SHA-256：`54DF307A54853B4701F554FB8A72197784D096C20298A863552C3A69A9F4CB2B`。验证汇总及源码摘要为 `artifacts/software-verification/firefox-v2-preflight-checks.json`。

本地 Windows 测试 87 项通过、0 失败、7 项专用验收默认忽略；新预检查包括在默认忽略项中，另在指定设备显式执行。Windows 全目标 Clippy `-D warnings`、Rust 格式、PowerShell/Node 语法检查通过。本轮仅新增测试及验收脚本，没有改动生产行为或前端；未重复整套前端回归，也未重新构建普通桌面程序，上一阶段普通程序仍为 `E26F8720D27D321B269C346FAB979029E76FE8A1C129B6A72DD6F86FA11647E6`。

已有目标 IP 的 mstsc 窗口，但两次画面捕获报 `FrameArrived timed out` 与 `window capture timed out`；本轮未能观察远程桌面，未发送卸载界面输入。02:41 +08:00 已清理本轮三个临时计划任务，相关测试进程为 0；专用 Firefox 安装保留，等待后续新上下文与交互卸载。未改变防火墙、UAC 或账户设置。

## 2026-10-01 夜间：分类能力与缺失信息

软件清单增加兼容的 `residual_capabilities` 与 `missing_information`，详情将缓存日志、设置、本地资料分别显示。微信和 WPS 的设置/资料没有独立规则，明确显示暂不支持；其他已声明范围也不再泛化为可删除。清理能力只描述尚未开放或仍需逐项核验，不读取登记显示版本来授予权限，不使用其他商店包的证据。标准卸载按钮仍独立依据后台的可卸载状态。

缺失字段按后台信息生成；空白按缺失，零大小保留。旧历史缺少这两个字段时按空数组兼容，页面保留原说明，不由总开关推断类别权限。字段不含路径、命令、逐文件清单或可回放授权。新增测试覆盖缓存不能推导设置资料、精确包族/范围证据、旧数据、缺失信息与卸载禁止独立、商店数据影响和关闭详情后的焦点恢复。

完整回归通过：前端 21 文件/279 项、Rust 工作区 225 项（0 失败，9 项专用实机/性能测试默认忽略）、脚本 20 项；256 条识别规则/510 个模式和 18 条应用投影一致。工作区及 fixture 全目标 Clippy `-D warnings`、Rust/前端格式、Node 语法和前端构建通过，仍有既有主 JS 分块大于 500 kB 的提示。日志为 `artifacts/software-verification/capabilities-frontend.log`、`capabilities-workspace.log` 及同前缀各检查日志。

原生模拟复测通过，列表、新能力表和残留页在 760×580、1100×780 的明暗主题下均无横向溢出；最小窗口两种主题的能力表截图已查看。分类文本、缺失信息、资料影响、关闭焦点、重复请求、超时核对、残留选择与独立确认、统计、历史和重连均通过。证据 `artifacts/software-verification/capabilities-native-20261001/verification.json`，提供程序仍为 synthetic fixture；没有真实卸载或文件删除，不代表任何生产规则获准。

03:03:39 +08:00 普通调试程序已恢复（custom-protocol，无 e2e-fixture），SHA-256：`E1126FE6A02B6755223E39542D6ADF8FB366760D4CF18ED38586C2BBCA24D99A`。独立模拟副本 SHA-256：`313A75A1FE2EA91C56361A6E1997CCF7002239CE619718F5A458E1C7E9634AC0`。本轮源码/构建摘要汇总为 `artifacts/software-verification/capabilities-checks.json`，测试进程及驱动监听已退出。未改动远端设备、生产许可表或生成发布包。

## 2026-10-01 夜间：WPS 独立备份与加载项设置

在原 93 条声明上增加 WPS 默认文档备份、publish.xml 和 jsplugins.xml 三条独立规则，分类为本地资料、设置、设置。位置由 WPS 官方文档明确描述，见[范围依据](software-residual-rules.md#wps-独立范围依据)。新位置识别器不依赖日志目录存在，不扩大到 Kingsoft 父目录、加载项代码或自定义备份路径；新规则的 Roaming 基准路径也纳入范围摘要。导出 `artifacts/software-verification/residual-rules-wps-v2.json`，共 96 条，逐一比较确认原 93 条范围摘要不变，生产验收表仍为空。

两项新增 Windows 临时目录测试验证独立识别、类别、精确文件名、父目录/相邻目录/嵌套同名文件/其他基准目录保留，以及替换、硬链接、新出现的位置和非 WPS 身份拒绝。它们只证明结构与身份边界，不属于真实 WPS 删除验收。界面补充文档备份影响，默认不选，清理前仍需单独确认；软件详情明确只包含默认备份与两份加载项设置。

新增 `software_wps_scope_probe` 只读示例，仅准备并输出规则 ID、范围摘要及是否捕获，不输出路径、命令、文件身份或可恢复的上下文，也不读取备份/配置正文。本机运行被现有标准卸载解析器拒绝（“仅支持直接运行本地卸载程序”），没有建立范围结论。首个失败日志和补充原因报告分别保存为 `wps-readonly-scope-probe.log`、`wps-readonly-scope-status.json`（位于 artifacts/software-verification）；没有改动安装登记、放宽解析或卸载开发机软件。这些记录不能填入生产许可。

本轮远端只读核对确认 SSH/RDP 服务运行、会话 2 和零测试进程。Computer Use 能读取本地 mstsc 的文字控件，但还原操作报 `coordinate input geometry is unavailable`，画面捕获分别报 `FrameArrived timed out`、`window capture timed out`；随后只读控件未能确认视图变化。未发送卸载器输入，没有远端安装/卸载/删除。记录 `artifacts/software-verification/rdp-observation-20261001.json`，等待可可靠观察的交互条件后再继续 Firefox 实机流程。

本阶段验证：Rust 工作区 227 项通过、0 失败、9 项默认忽略；软件页面 19 项、脚本 20 项通过。工作区及 fixture 全目标 Clippy `-D warnings`、格式与前端构建通过，仍有既有分块大小提示。前端只改资料影响文案，因此运行专项和原生检查，本阶段没有重跑前一阶段的全部 279 项前端测试。日志均以 `wps-` 为前缀保存在 artifacts/software-verification。

原生模拟的四尺寸/主题组合、分类能力、残留选择与资料确认、键盘焦点、重复请求、超时核对、历史及重连均通过；浅色详情和深色确认的最小窗口截图已查看。证据 `wps-native-20261001/verification.json`，其中数据仅在内存模拟，不是 WPS 产品或真实备份删除验收。03:27:30 +08:00 普通调试程序已恢复，SHA-256 `F180D6CF6465485F4F684DB67E7688559891888FEBA15D940802DD1961DC8DC7`；独立模拟副本 SHA-256 `8B1BA3834FAC4E87F693D787A22681C1D71F83F428CAC653C21008BEA2DCE33B`。测试进程和驱动监听为 0，未生成发布包。阶段摘要见 `wps-scope-checks.json`。

## 商店残留共享关系保护（2026-10-01）

新增当前用户商店包的独占性检查：卸载前匹配原完整包身份、唯一包族登记及主包类型，读取所有当前用户共享容器成员，再双向核对可选包和宿主等关联。卸载后重新查成员定义和 Main/Optional 包关系；无法完整读取时保留，不推断独占，也不影响独立的标准卸载策略。旧上下文缺失归属证明时不能遍历冻结目录。五条商店规则升为版本 2 并纳入契约摘要，导出 `residual-rules-store-v2.json`；比较确认另外 91 条摘要保持不变，生产审批仍为空。

普通用户会话 2 的只读查询读取 121 个当前用户主包，对其中 74 个可卸载包核对共享/关联关系，均成功；当前没有共享成员，前后安装身份集合一致。这证明真实 API 可用，不证明这些应用的数据删除已验收。该轮没有触发“未知关系”的实机准备回退分支，报告 `fallback_verified=false` 如实保留，不能算作通过该分支。

随后只在闲置电脑建立固定名称的空共享容器，成员限定为两个**未安装**的 DustAway 测试包族，无需安装 MSIX 或恢复证书信任。普通用户执行生产检查，两个成员均因共享关系被拒绝；移除本次精确 ID 的容器定义后，再次读取共享成员及卸载后的 Main/Optional 关联查询，结果通过。没有调用强制关闭或重置容器，没有卸载应用或删除真实残留文件。03:50 +08:00 已确认临时容器不存在、其他容器身份未变化、三个临时任务已移除、测试进程为零；专用 Firefox 保持安装，测试证书仍未信任。

证据位于 `artifacts/software-remote-acceptance/evidence/`：`store-isolation-query-20261001.json`、`store-shared-shared-20261001.json`、`store-shared-restored-20261001.json`、`store-shared-fixture-restored.json` 和 `store-isolation-finished.json`，并保留各运行日志与普通权限记录。测试驱动及脚本摘要分别在 `store-isolation-query-manifest.json` 与 `store-shared-guard-manifest.json`；它们在 `artifacts/software-verification/` 中，不能重建生产残留授权。

新增三项本地回归覆盖共享成员、关联包、同包族重新安装、用户/包/契约绑定和旧上下文拒绝遍历；两项实机用例默认忽略，本轮分别显式运行只读查询一次、共享用例两阶段。最终 Rust 工作区 230 项通过、0 失败、11 项默认忽略；普通/fixture 全目标 Clippy、格式及 20 项脚本测试通过。前端构建通过，仍有既有 500 kB 分块提示；完整前端首轮长期停在启动阶段后主动中断，原日志保留，重跑结果与最终二进制摘要见 `store-isolation-checks.json`。

本轮没有重新进行真实 EXE/MSI/MSIX 卸载和原生页面布局测试，前端页面逻辑未修改。共享容器保护的实机通过不能代替真实包数据残留/删除、可选关联包正向样本、包进程退出核对或通用系统契约验收；全部生产残留删除仍关闭。

最终完整前端重跑 21 文件/279 项通过。03:54:49 +08:00 普通调试程序 `target/debug/dustaway.exe` 构建完成，SHA-256 `AC755AEB8BC0AADAFA291B97A3EB0E54849F82A2D3C4ACDF9108FFE41C09FDD8`，不含 e2e-fixture；未生成发布安装包。

## 商店相关进程检查（2026-10-01）

新增独立的当前用户进程核对模块。Windows WTS 元数据决定归属，不以会话号或进程名猜测；打开有限查询/退出观察句柄后再次核对用户，使用内核包族身份及卸载前捕获的本地安装目录判断相关进程。无包身份的安装目录子程序同样阻止残留处理；相邻前缀目录不匹配，Windows Unicode 大小写比较覆盖路径边界。运行与无法核验分别保留，不结束进程、不读进程内存、不提权。

私有商店契约更新为 2，五条商店规则更新为 3；新增安装位置只供进程检查，不能用于删除。旧上下文缺字段即失效。共享/关联/进程遍历在条目之间检查同一个停止标识，停止告警与未知归属、仍运行告警分开。普通清理及自选删除流程未修改。

普通权限 WTS 只读预探测读取 240 条元数据，其中 102 条报告当前用户 SID，其余 138 条的 SID 被系统隐藏；该观察用于确认 API 行为，没有打开或控制其他用户程序。最初通过 SSH 编码发送长脚本超出命令长度，随后直接执行文件受到 PowerShell 执行策略限制；两次均未启动测试。改为传输专用脚本、沿用既有进程级执行参数后运行，未改变持久执行策略或其他安全设置。

04:12 +08:00 普通用户会话 2 的实机测试检查 104 个当前用户进程，其中 21 个有真实包身份，无查询失败。选取已运行的包进程作只读核对，生产检查返回仍运行；随后在专用标记子目录启动无包身份的测试副本，路径检查同样拒绝。通过测试管道令牌正常退出后，前后检查均可确认不存在该测试包/安装目录的相关进程。没有操作实际包进程，没有调用任何进程终止接口。该子程序不是 MSIX，不能声称验证了真实包卸载期间的进程生命周期。

新证据：`artifacts/software-remote-acceptance/evidence/store-process-native-20261001.json`、`store-process-check-context.json`、`store-process-check-result.json`、`store-process-finished.json`。驱动摘要为 `artifacts/software-verification/store-process-check-manifest.json`。04:15 +08:00 两个临时任务已移除，专用进程数为 0；Firefox 测试安装保持、临时证书信任仍未恢复。标记样本与日志保留，不重放卸载或删除。

本地新增三项回归覆盖身份/路径匹配、相邻路径与 Unicode 比较、停止、缺少进程范围的旧证明、仍运行/未知/停止告警。Rust 工作区 233 项通过、0 失败、13 项默认忽略；其中两个新增忽略项为内部自然退出子程序和上述实机用例。软件页面 19 项专项通过；前端源码、IPC 形状未改变，没有重跑上一阶段的完整 279 项和原生布局截图。新 PowerShell 脚本语法检查通过。最终构建与静态检查摘要见 `store-process-checks.json`。

本阶段只补进程保护，不开放生产删除。仍需受控的同用户跨会话、可选/宿主关联包样本和新构建真实 MSIX 卸载/数据残留闭环，以及通用商店规则许可和全部真实产品残留验收。

最终普通/fixture 全目标 Clippy `-D warnings`、格式检查通过，最终商店相关专项 8 项通过/4 项默认忽略。04:20:38 +08:00 普通调试程序构建完成，SHA-256 `CDF9741FE7E9C4B10FFAB7DB3B2266CFC925F65B8BB850F130BE5F4D94C37711`，无 e2e-fixture。没有重建前端资源，沿用上一阶段与当前前端源码一致的构建。导出规则 `residual-rules-store-v3.json` 并确认仅五条商店摘要更新，另外 91 条不变。

## Firefox 六条生产残留规则（2026-10-01）

在专用标记配置中重新捕获卸载前上下文，使用官方静默卸载准备资料测试。cache2、prefs.js、user.js、cookies.sqlite、places.sqlite、storage 六条分别完成全部保护断言及真实文件删除：长期排除、占用、内容变化、硬链接和路径替换被拒绝；新增文件与非空目录保留，每个阶段都比对未选择文件哈希和共享见证。证据采集使用只编译进测试程序的判定，与生产共用范围、安装状态、进程和文件句柄检查。首轮随后因 bookmarkbackups 为空而失败，六条完成记录及失败均保留；不将空目录或整轮失败算作第七条通过。

六条记录分别绑定签名程序版本 `140.17.0.1158`、规则 ID、范围和证据 SHA-256。证据归档为 `docs/evidence/software/firefox-rules-20261001.json`；普通清理、自选删除、其他版本、其他产品、书签备份和会话没有因此得到权限。生产授权入口没有运行时、环境变量或 IPC 的替代判定。

生产入口复测的首次重新安装使用混合斜杠路径，安装器截断到 DustAwayAcceptance 测试父目录，预期安装检查失败，尚未执行该轮残留删除。按精确登记、Mozilla 签名与 PE 数字版本核验后，以官方卸载器收尾，原证据目录 1092 份文件哈希不变。收尾预检曾误用三段显示版本，未启动操作；改用四段数值字段后完成。此问题局限于测试安装脚本，生产标准卸载没有增加或更改参数。

修正为[官方 INI 配置](https://firefox-source-docs.mozilla.org/browser/installer/windows/installer/FullConfig.html)及规范化 Windows 路径，并增加混合斜杠回归。新的普通用户实例重新取得上下文后，六条正式 `WindowsSoftware.delete` 入口测试于 04:51:34 +08:00 通过：删除 22 份测试文件、6,114,449 字节及 11 个空目录。真实重新安装后，规则范围本身仍有效，但生产删除明确因安装状态返回 Protected；再次官方卸载、重新扫描后才继续执行。每条再次完成排除、占用、变化、硬链接、替换和未选择哈希检查，新范围与整个配置根被拒绝。

报告为 `artifacts/software-remote-acceptance/evidence/firefox-production-ini-20261001-report.json`，连同 `firefox-rules-20261001-partial.json`、原失败日志、安装位置诊断、收尾结果和普通权限记录保留。这次静默操作只证明真实残留规则与生产文件操作，不证明交互向导或真实 Tauri 页面选择/确认/结果流程；也不代表云同步提供程序、书签/会话、其他版本或产品全部通过。

04:53:49 +08:00 四个临时任务已移除，测试进程及 Firefox 登记为 0，两处测试安装程序均不存在，标记资料与证据保留；没有强制结束进程、重启、安全设置或证书信任变更。当前不能重放旧脚本，下一轮需新建验收上下文。

本轮 Rust 工作区 235 项通过、0 失败、15 项默认忽略；软件页面 19 项通过，前端构建通过并保留既有分块大小提示。普通/fixture 全目标 Clippy、格式和新 PowerShell 语法通过。没有前端逻辑或 IPC 形状修改，没有重复前阶段全部 279 项或原生四布局截图。96 条规则范围摘要逐一未变，新增六条 Firefox 许可。普通调试程序 04:54:18 +08:00 完成，SHA-256 `2CB798728C1E8173883D47545F92D8283B7BB0A53727CD14FB2D513CCDA754A1`，不含 e2e-fixture，未发布安装包。汇总 `artifacts/software-verification/firefox-approved-checks.json`。

## Firefox 正式页面的真实残留删除（2026-10-01）

普通用户会话 2 使用无 fixture 的正式 `WindowsSoftware` 和 Tauri 命令完成专用 Firefox 样本的卸载、身份核对、残留选择、确认、实际删除、历史与 WebView 重连。样本为官方签名 `140.17.0.1158`；仅对本次专用安装的精确 HKCU 登记加入 Mozilla 官方 `/S` 参数，方便无人值守。生产程序没有注入卸载命令或权限开关。这是拂尘的真实页面流程，不能作为本轮操作供应商交互向导的证据。

05:23:55 +08:00 完成：默认所有资料未选；通过路径搜索选择缓存文件、其父目录、user.js 和 cookies.sqlite。两次准备得到不同确认 ID；第一次取消后所有文件哈希不变、没有删除请求、焦点回到清理按钮。第二次确认后只删除三份文件，共 **524,814 字节**，非空父目录保留。确认后新建的见证文件、所有未选资料和 Mozilla 共享见证均保留。卸载和删除各只有一次业务请求；实际释放量没有采用软件登记的 267.4 MB 估计值。历史增加一条记录，旧记录不变，保存结果不携带安装位置、候选清单或残留授权；刷新 WebView 后仍是原运行实例和原任务，重新扫描撤销旧选择并保留累计删除量。

两次脚本问题如实保留。首次路径字符串没有归一化分隔符，在残留选择前失败，没有删除；测试计划任务退出后原应用也关闭，因此后续使用新安装和新上下文。第二次批量 WebDriver 输入长路径丢字，保留原应用、原任务和会话，通过单独的逐字输入脚本继续成功，未再次卸载。原脚本的退出码仍为 1，续测脚本退出码为 0，不能表述为一次不中断的脚本运行通过。

原第二次脚本在输入前检查了 1100×780、760×580 明暗四种原生窗口的行边界并保存截图，本地已在续测前取回。续测的同名布局截图处于先前无匹配搜索状态，其 `rowsFit` 不能单独证明非空清单。资料确认截图显示长路径可滚动查看、影响文字和两个操作按钮在小窗口内可用；实际结果与历史截图已查看。重连截图保留了“列表读取已被新的刷新取代”：验收脚本的独立清单核对与页面刷新冲突，属于测试查询时序；后续脚本已增加等待页面清单完成，这一时序修改没有再次实机运行。

归档摘要 `docs/evidence/software/firefox-desktop-delete-20261001.json`，原始记录位于 `artifacts/software-remote-acceptance/evidence/firefox-desktop-delete-20261001-v2/`，并保留第一次失败目录。测试程序 SHA-256 为 `2CB798728C1E8173883D47545F92D8283B7BB0A53727CD14FB2D513CCDA754A1`。05:25:51 +08:00 三个临时任务已移除，测试进程、监听和 Firefox 登记为零，恢复原主题，证据和未选资料保留。没有重启、强制结束卸载器、安全设置或证书信任变更。

随后修复实机路径形式暴露的搜索遗漏：固定位置组合可能包含两种分隔符，原来以标准 `C:\...` 搜索混合 `/` 的候选会漏掉结果。新增核心集成回归先在旧实现失败，再将查询和候选的**搜索比较**统一分隔符后通过；返回路径、候选 ID、扫描 ID、分页和只读权限保持不变，测试不执行删除。软件任务专项 30 项、Rust 工作区 236 项通过，0 失败、15 项专用测试默认忽略；普通工作区全目标 Clippy 和格式通过。前端源码、删除模块和六条许可未变，没有重复前阶段前端构建/279 项测试；上述真实页面证据对应搜索修复前的已注明程序哈希，新程序不能声称重新通过全部实机流程。

05:33:55 +08:00 新普通调试程序构建完成，SHA-256 `DE8EB9A81D1BA0717DA557E48DD4FADA7D61EFA2E24738F3BF6AD6B9C26B976D`，未生成发布包。两份新 JS 和 13 份 PowerShell 语法检查通过；没有重复 fixture Clippy。最终摘要 `artifacts/software-verification/firefox-ui-checks.json`。

### 仍需完成

- Firefox 的真实标准卸载、核对、残留选择、删除、结果和历史页面流程已取得上述限定范围证据；其他真实产品的页面闭环仍待补充。
- 各真实产品、版本、分类的独立目录归属和资料保护；自定义资料、共享父目录、云文件、占用和部分读取失败的产品证据。
- Firefox 六条规则已完成逐条真实删除、生产入口及重新安装保护；本轮页面覆盖缓存、设置和 Cookie。书签、会话、其他版本、真实登录状态及其他产品仍待独立验收。

当前只有上述六条 Firefox 规则及精确版本可在全部运行时检查通过后选择，其他残留规则继续保留。普通清理与自选删除的既有权限不变。这不是完整真实软件验收通过声明，也没有生成安装包或发布版本。

## Firefox 书签与会话备份独立验收（2026-10-01）

Mozilla 官方 geckodriver 0.37.1 下载包摘要 `DFED9315ABE8D2FBC1B6161A2EE8002452E79CF05EE92FDC653A4E26BC35EDD8` 与官方发布元数据相符。仅使用闲置电脑普通用户、标记配置和回环测试页面，通过 Firefox 的 Places / SessionStore 实际生成压缩备份，并解码核对测试书签、两个标签页及下一次启动的上一会话；不手工伪造备份正文。API 调用依据 [Mozilla 会话刷新源码](https://raw.githubusercontent.com/mozilla/gecko-dev/esr140/browser/components/sessionstore/TabStateFlusher.sys.mjs)与[会话文件源码](https://raw.githubusercontent.com/mozilla/gecko-dev/esr140/browser/components/sessionstore/SessionFile.sys.mjs)。

准备过程有两项失败，未隐藏：首轮 HTTP favicon 请求使 Node 异常退出，专用 Firefox 仍存活；连接原浏览器后又发现会话落盘早于标签状态刷新。先核对原进程、父进程和 Marionette 监听，再刷新原会话并正常请求浏览器退出，没有因超时重启。连接既有浏览器时 WebDriver DELETE 只脱离，随后明确核对浏览器自然退出。最终新种子于 05:55:34 完成两次正常浏览器退出。脚本只恢复本轮起始时的测试 user.js，不宣称恢复首次准备以前的状态。

05:58:06 普通用户原生证据测试通过：`uninstall.firefox.bookmarks` 删除 1 份 990 字节，`uninstall.firefox.sessions` 删除 5 份 5,255 字节。每条均验证排除、占用、内容变化、硬链接和路径替换拒绝，部分扫描告警、冻结后新增文件、非空目录、未选择文件哈希及共享见证保留。原安装消失且其他软件身份清单不变；整个配置及新建未识别范围被拒绝。采用同一份内存中的卸载前上下文，未保存可恢复授权。此阶段使用仅编译在测试中的证据判定，不是生产入口或界面测试。

据此新增两条精确版本 `140.17.0.1158` 的独立许可，证据 `docs/evidence/software/firefox-history-rules-20261001.json` SHA-256 为 `8419DFA2FF643DE5A073783F63853D25539E664351C574CAB6CFCF3271A5A048`。原六条证据 SHA 保持 `7054E8E1188039EBEAFAD2B8D5C126A449C340A35F1E3F42C95459CA7FAE1F71`。回归逐条核对两个证据文件、规则摘要、独立保护结果和精确版本；其他版本、产品及未声明范围不被开放。

06:06:49 新安装与新的真实备份生成完成。06:08:00，测试程序 `444FEC4B4B37A6ECA50DF9F019E02B5868889819EA111468CD63CACB45AC952C` 经正式 `WindowsSoftware.delete` 入口通过两条规则复测，包括真实重新安装时拒绝旧记录，随后官方卸载、重新扫描再删除。此次书签范围 2 份 1,036 字节，会话范围 3 份 3,174 字节，合计 5 份 4,210 字节；数量包含前轮保留的测试见证，不能全部计作浏览器正文。两个非空目录和本轮新增见证保留，未选文件哈希不变。原始报告 SHA-256 `50BE2F947ED610F0AC0F23FDEA97BD664EE40AA98CCA99155556586C9B0B0A7C`；官方静默卸载用于资料场景，不代替供应商向导或这两条规则的页面交互验证。

06:10:29 六个临时任务移除，专用进程、端口、Firefox 四个注册表视图中的登记和安装均不存在；资料、失败记录及证据保留，测试证书仍不受信任，没有改变安全设置或强制结束进程。当前支持八条规则，其他产品/版本、云同步提供程序、微信独立资料与商店通用许可等仍未完成；完整目标继续。

阶段回归：核心规则 7 项、Rust 工作区 236 项通过，0 失败、17 项默认忽略；新增两项已单独实机执行通过。普通工作区全目标 Clippy、格式、6 份 PowerShell 和一份 JS 语法通过。前端及 fixture 未变，未重复它们的既有测试或四布局。目录声明仍为 96 条，许可为 8 条；Windows 生产删除模块及旧证据摘要未变。06:14:23 +08:00 普通 custom-protocol 程序 SHA-256 `68A9463FC8CA72EE6FA747459056B1EAE19810761883248A20447BC27572E22E`，不含 e2e-fixture、未发布安装包，也没有把原生测试驱动的结果等同于新普通程序完整 UI 实机验收。汇总 `artifacts/software-verification/firefox-history-checks.json`；[生产入口证据归档](evidence/software/firefox-history-production-20261001.json)保留具体范围与限制。

## 大目录残留扫描停止与遍历上限（2026-10-01）

原实现只在取出已枚举路径时检查取消和候选数，单个目录的 `read_dir` 循环可能一次加入大量路径；读取失败的条目不计候选数，还会不断追加警告。新增 `residual_walk` 将检查放到每次枚举读取前后，并让所有根目录共享十万条遍历预算，读取失败也计入预算。停止或耗尽后结束扫描，以既有警告说明结果不完整，不把尚未处理的路径列为候选。当前单次系统调用仍须等其返回；未扩大任何删除权限。

三项回归均先在原枚举行为下失败，修改后通过：目录中途取消后不再读取下一项；跨目录预算包含不可读条目且耗尽后不推进迭代器；完整空目录与部分读取失败分别保留正确结果。原失败日志 `residual-walk-before.log` 未覆盖，修复日志 `residual-walk-after.log`、全工作区日志和 Clippy 日志位于 `artifacts/software-verification`。

完整 Rust 工作区 **239 项通过、0 失败、18 项默认忽略**；普通全目标 Clippy `-D warnings` 和格式检查通过。新增第三方软件用例仅为 Brave 只读预检，尚未实机执行，不能计入通过项。前端、fixture、IPC 形状与删除许可未变，未重复之前的前端 279 项、页面 19 项、构建和原生四布局。Firefox 精确版本的八条许可及原证据保持不变，不能把此轮扫描修复视为新普通程序的全部 UI 闭环验收。

06:47:54 +08:00 普通 custom-protocol 程序构建完成，SHA-256 `9BEEEA58B88E1C72406317635D13888452ED2A798E454AFC50CD68D3B928EC6D`，不含 e2e-fixture、未发布安装包。此前五个已记录源文件/证据摘要逐一一致。汇总 `artifacts/software-verification/residual-walk-checks.json`。

### Brave 准备现场

闲置电脑原无 Brave 安装/资料，06:27:42 +08:00 以普通用户启动已核验的官方 v1.96.60 离线包。外层包仍在跟踪中，未生成准备成功结果，未生成测试资料、卸载或执行残留删除。外层包不接受本次传入的 Chromium 内层参数；已核对转发的实际命令行，但错误窗口内容尚未取得，不能把“可能等待提示”写为已确认原因。RDP 捕获两次超时、恢复窗口操作返回 `coordinate input geometry is unavailable`；未重启安装或盲点。

仅从原进程的专用临时目录保存了签名有效、版本与清单匹配的内部包，没有运行它。包摘要、单次运行边界及未部署的种子/预检说明见 `scripts/software-acceptance/README.md`。原进程结束前不执行后续安装；此项待核对不妨碍独立代码开发。本轮没有新增 Brave 或其他产品的生产许可，没有测试用户个人资料。

## 商店 LocalCache 资料分类修正（2026-10-01）

根据微软的 [LocalCacheFolder 定义](https://learn.microsoft.com/en-us/uwp/api/windows.storage.applicationdata.localcachefolder?view=winrt-26100)和 [ApplicationData 持久性说明](https://learn.microsoft.com/en-us/uwp/api/Windows.Storage.ApplicationData)，该目录并不保证只含可重新生成的缓存。原先按 `cache` 分类会让今后获得权限的候选落入错误分组，清理计划也可能缺少资料影响提示。现保持具体目录边界，将 `uninstall.store.local_cache` 改为 `local_data`、规则版本 4；返回 `category=data`，选择后 `includes_data=true`。TempState 和其他四条商店范围不变，商店生产权限仍为空。

两个回归在旧分类下分别失败，修复后通过：规则语义及旧缓存证据拒绝；真实规则分类经过模拟候选进入核心分页和清理计划时必须触发资料提示，未执行任何文件删除。旧失败分别保存在 `store-local-data-before.log` 和 `store-local-data-engine-before.log`，成功日志为 `store-local-data-after.log`。这些是本地规则与任务回归，不能当成真实 MSIX 应用数据或删除证据。

编译规则导出 `residual-rules-store-local-data.json` 与前一商店版本导出逐条比较，96 条中的 95 条范围摘要一致。只有 LocalCache 从 `cache` / 3 变为 `local_data` / 4，新摘要 `019b5dc5093cbd6ecc190456993345cc20a35f91306697bc314e6dfee4ef794f`；八条 Firefox 许可未变。差异报告 `store-local-data-catalog-diff.json`，均位于 `artifacts/software-verification`。

RDP 已重新核对唯一现有窗口及输入焦点，提交微软文档定义的 Ctrl+Alt+Pause 窗口切换快捷键；返回的界面结构未证实切换，随后捕获仍为 `FrameArrived timed out`，未继续发送远端窗口操作。06:59:39 +08:00 原安装器 14876 / 子进程 5872 仍在，未出现更新日志或网络连接。状态等待与代码修复分别记录；没有把快捷键返回成功当成安装完成。

回归中的失败保留：第一次全工作区运行发现 Windows 目录夹具仍把 LocalCache 断言为缓存，已按系统含义改为资料，并加入 TempState 仍为缓存的对照；文件身份替换与相邻包保护断言继续保留。核心计划用例使用明确的模拟商店身份，不安装包或操作个人资料。前端首次工作进程启动超时，零用例执行；确认该进程已结束后独立重跑原配置，于 07:02:28 +08:00 **19 项通过**，不把首次启动失败改写为成功。

最终 Rust 工作区 **241 项通过、0 失败、18 项默认忽略**；普通全目标 Clippy `-D warnings` 和格式通过。前端源码未修改，没有重复完整前端 279 项、构建或原生四布局；没有在闲置电脑运行新的 MSIX 生命周期或残留删除。07:05:14 +08:00 普通 custom-protocol 程序 SHA-256 `851B4A1ECC53268CCA1E1EA50D753FE9A99655B3D8B0E19DD579499112EC4457`，无 e2e-fixture、未发布安装包。07:06:20 原 Brave 安装器仍在，未重放。汇总 `artifacts/software-verification/store-local-data-checks.json`。该阶段是资料语义修复与回归进展，原完整目标仍未完成。

## MSIX ApplicationData 独立样本准备（2026-10-01）

新增 `store_data_fixture` Rust example 和 `Build-MsixData.ps1`，固定身份为 `DustAway.Acceptance.Data.20261001` / `1.0.0.0`。样本使用真正的 `ApplicationData` / `FileIO` 接口生成并读回四类文件和 LocalSettings 键值；限定指定电脑、普通用户、验收标记、精确包身份及包内 EXE。只接受 `seed` / `verify`，每阶段以系统 FailIfExists 语义阻止重放；结果留在自己的 LocalState，未来控制端须在卸载前归档。不关闭虚拟化，不操作共享或其他包，不包含卸载/删除/许可接口。TempState 若在两次激活之间被系统回收，复核只报告缺失。

首次编译因 windows-rs 的 Storage 搜索、流和属性 feature 未开启而失败，保留 `store-data-fixture-build.log`；补齐仅用于开发目标的 feature 并移除未用导入后，独立重编通过。两项身份与参数保护测试通过；07:19:25 在开发机以 `seed` 启动，退出码 2、错误 `wrong fixture host`，在 WinRT 激活和数据写入前拒绝。这只是保护检查，不能写为包内数据生成通过。example Clippy、工作区全目标 Clippy `-D warnings`、格式、PowerShell 语法均通过。

07:19:39 已构建签名包 `artifacts/store-data-msix-20261001/DustAwayAcceptanceData.msix`，SHA-256 `B23BB9234C598DA79E08D7728E9B0057079EBC0DB41087BA4F96AE896C2840DE`；程序摘要 `5F76B0B872402994F5736E1E9DE68DB68EE5B25657CDBFB4873267BB7EF62B10`。包内 EXE 和清单摘要与输入一致，签名项存在，MakeAppx/SignTool 返回成功。该结果不等于目标设备部署成功或信任链验证通过。临时签名证书 `86E1409FDD915DBD0A2875F53BF24429BABDC758` 及不可导出私钥已移除，四个用户/系统 Root、TrustedPeople 存储均无该证书。没有添加信任、部署包或更改安全设置。

本阶段没有修改生产逻辑、规则、授权或前端，没有重跑上一阶段 241 项 Rust / 19 项页面测试、完整前端构建或真实 UI 闭环。普通程序摘要仍为 `851B4A1ECC53268CCA1E1EA50D753FE9A99655B3D8B0E19DD579499112EC4457`，两份 Firefox 许可证据摘要逐一未变。摘要 `artifacts/software-verification/store-data-fixture-checks.json` 将样本准备与待执行的包内激活、跨进程持久性、标准卸载、残留检查/删除分开。商店删除许可仍为空，不把新样本包当作商店通用能力已完成。

07:19:27 远端只读观察仍为原 Brave 安装器 14876、子进程 5872 和运行中的原计划任务，只有原启动记录。未重启安装、运行内部包、强制结束或发送新的 RDP 窗口操作；待原提示内容或自然退出后再核对。

### MSIX 单次激活与归档控制端

新增 `store_data_control` 和 `Run-MsixData.ps1`。控制端固定本轮包及样本 EXE 摘要，启动前同步落盘单次意图；使用系统 ActivateApplication 和同会话事件握手，拿到经用户/会话/包身份/程序位置核对的原进程句柄后，保存 PID 与创建时间，再允许专用样本开始资料阶段。只请求查询和同步权限，不包含进程终止代码。90 秒观察超时只返回不确定结果，后续 observe 不重新激活、不发继续事件，原进程消失且退出记录缺失时不能宣称成功。报告身份、四类范围和文字摘要必须逐项完整，退出码必须为零才归档完成；阶段报告只读取一次，摘要对应同一份字节。

六项本地测试通过（控制端 4、样本 2），工作区全目标 Clippy `-D warnings`、格式和 PowerShell 语法通过。开发机控制端退出 1，明确拒绝错误主机，未进行包激活。此处并未测试真正的包内事件握手或跨进程持久化。新包于 07:31:50 构建，目录 `artifacts/store-data-msix-20261001-handshake`、包摘要 `E2248F341B0EA92D5E8AACEBF923026F29DC45C73A2C92B2D446F6187E575A2E`，内置样本摘要 `3DFBB5BA09DCF99F50FC0CB87FE29823C248505BFFDF4BA14D792091D44F5666`；控制端摘要 `5598F519976F571591350CA4F08AF7DE0F90748A903F6084281E1C6C0E9D99DD`。初版包保留但不能与新控制端混用。新证书 `EB799D221A9CCECDFC7676C4A262D80EE406325C` 和私钥已移除，未建立信任或部署包；用户的本轮临时证书信任授权仍待答复。

两次真实普通用户 Preflight 均抵达“专用包未安装”的保护，没有生成启动意图或调用激活。第一次 07:33:29 暴露封装程序的问题：异步 Start-Process 返回对象的 ExitCode 为 null，退出时变为零。没有把计划任务零状态计为成功。改用同步 -Wait 并明确拒绝空退出码后，第二次 07:35:20 原生退出 1、封装记录 1、任务返回 1，错误与预期缺失包相符。日志前缀分别为 `msix-data-run-preflight-69ffa07abb8c4dbd9e8a61df7206a698` 和 `msix-data-run-preflight-40d7cff5c15148bd879e41b7cd56da42`，原失败源码与证据均保留。

07:36:14 移除两个已结束的专用预检任务，控制端进程为零，MSIX 仍未安装，无激活意图文件；Brave 原安装器 14876 / 5872 仍存活，未结束或重启。普通 DustAway 程序及两份 Firefox 许可证据摘要未变，生产能力、前端和已有 241 项 Rust / 19 项页面基线不变。没有新增残留删除许可或把测试工具构建算作普通程序完整验收。汇总 `artifacts/software-verification/store-data-control-checks.json`，测试设备收尾证据 `msix-data-preflight-finished.json`。

## 桌面残留跨会话进程保护（2026-10-01）

桌面残留扫描和删除改用全会话 Toolhelp32 名称/PID 快照，对相关进程仅以查询和同步权限读取真实用户、程序路径及退出状态。确认为其他用户或已退出时才跳过，相关进程无法查询返回 `process_unknown`；不结束进程、不改变普通清理规则或残留授权。两个本地回归覆盖跨会话用户筛选、真实自身进程的大小写名称匹配、用户/程序名复核及取消。

初版尝试复用商店 WTS 清单，实机证据否定了该方案。V1 观察器访问 SSH 子进程被拒，子进程在原父程序超时后自然退出，观察器失败没有计为通过。V2 首次观察器同样失败；续测连接仍存活的同一个子进程，证明 WTS 漏列该同用户、不同会话且不可访问的进程，初版新检查错误放行。续测退出 101、原始报告与自然退出记录均保留。旧 `security::process_running` 在会话查询失败时已经保守阻止，因此不能声称这个不可访问样本重现了旧代码误删；最终改动还消除可读取同用户进程的会话过滤，后者尚无独立实机正向样本。

修正为 Toolhelp32 后，08:06:46 +08:00 V3 原生用例通过：普通用户会话 2 看见会话 0 专用子进程，创建者用户摘要匹配；元数据访问被拒时返回 `process_unknown` 并阻止处理。仅发送该子进程自己的退出令牌，由原 SSH 父进程句柄确认退出 0，随后检查恢复通过。没有真实软件卸载、包进程或残留文件删除，不能作为新规则许可。该测试也确认商店现有 WTS 路径仍有相同漏列边界，商店删除许可为零，不能把本次桌面修复称为商店保护完成。

完整 Rust 工作区 243 项通过、0 失败、19 项默认忽略；新增忽略项已显式在闲置电脑通过。普通全目标 Clippy `-D warnings`、格式与 custom-protocol 构建通过。08:13:07 +08:00 普通程序 SHA-256 `83DF0CAD2473770C28DD42738BF57619268A0B40B5E907E58832DA8E1AB32B4E`，无 e2e-fixture，未发布或重复完整 UI 闭环。前端未修改，沿用上一阶段 19 项页面、此前 279 项完整前端及构建的限定证据；本轮未重跑。96 条声明、八条 Firefox 精确版本许可与两份许可证据摘要不变。

08:13:13 四个已结束的专用计划任务按精确动作核验后移除，子进程与驱动均不存在；所有失败记录和测试资料保留，没有强制结束进程或改变安全设置。汇总 `artifacts/software-verification/desktop-process-checks.json`，原始记录位于 `artifacts/software-remote-acceptance/evidence/desktop-process-cross-session*`。取证时一次错误文件名导致 scp 拒绝，改为实际 continuation 文件名后归档完成，未重跑测试。

08:12:19 原 Brave 安装器结束，原生错误 `-2147218431`（`0x80040C01`），封装/任务退出 99；08:13:27 确认原进程及子进程均已消失，没有 ready 结果。用户回复没有可见提示。未将错误码推断为已经证实的故障原因，原失败文件已归档，新的安装准备仍须核对当前登记和标记目录。

## Brave 真实资料与只读范围预检（2026-10-01）

原失败结束后，08:16:47 核对四个注册表视图无 Brave、专用产品根仅有本轮标记、内层包签名与 SHA 一致。新的 `Run-BraveInnerPrepare.ps1` 限普通用户并记录原句柄、PID 和单次启动标记，参数交给支持 installerdata 的实际程序；保留原封装失败。08:19:42 原生/封装均退出 0，已安装签名有效、PE 版本 `154.1.96.60`，浏览器 SHA-256 `B7995F5B0926F45804DC12D5EA8057E7DB760DCB6AF2F18565B21A43EE3AE560`。没有启动浏览器或更改 Windows 安全设置。

安装器在 User Data 内初始化 Crashpad，首轮传输预检拒绝。只读查看元数据后确认没有 Default 或其他配置，只含安装时间窗口内的 Crashpad，未读取其内容；种子封装仅接受这一明确状态。第一次真实种子因 `brave://version/` 被规范化到 `chrome://version/` 而等待超时，独立连接原实例证实页面完成、配置正确，以 Browser.close 正常关闭，原句柄确认退出 0；原驱动退出 1 保留。V2 规范化修复后到达书签阶段，实际版本缺少新 Chromium 私有模块；没有通过手写文件绕过。只读核对实际页面的 `chrome.bookmarks` 服务可用，再正常关闭原实例，保留第二次失败 1。

V3 于 08:31:53 完成：浏览器服务生成书签，回环页面生成测试 Cookie、Local Storage、IndexedDB；两次启动均核对专用配置，第二次能读回同一资料与书签，两次正常退出 0。11 个独立范围都存在非空真实文件，包含缓存、两份设置、书签、历史、Cookie、两种存储和会话文件。此阶段未验证会话自动恢复、云同步或个人资料，不把存在文件等同于对应功能全部通过。原始种子归档 [brave-seed-20261001.json](evidence/software/brave-seed-20261001.json)，SHA-256 `285A78ED096182C7463EE66ABA09524233E9AB711EC8EC10DB67F3CFC5D9D05C`。

更新默认忽略的原生用例绑定 V3 报告名，重新编译后的驱动 SHA-256 `923A3A72FE217E79262BEE804BC0E74A55E307284748D6B52DE9384D3E8CF5D8`。08:34:07 普通用户显式预检通过：11 个范围独立捕获并分类，精确安装器和版本匹配，共享父目录、整个配置及未知嵌套 Cache 均排除，软件仍安装时残留扫描被拒绝。没有执行卸载或文件删除，不开放任何许可。原生归档 [brave-preflight-20261001.json](evidence/software/brave-preflight-20261001.json)，SHA-256 `7375889A8A682806F62CEB02261F76250778E84014514069C267ED90E3743B4A`。

08:35:26 六个已结束任务按精确动作核验后移除，浏览器、种子驱动和测试监听为零；保留安装与标记资料供下一阶段使用。首次外层安装失败、两次种子失败和对应正常退出均归档；读取仍被重定向占用的日志时观察器曾失败，修正为逐文件报告不可读，没有重启原任务。没有强制结束进程、删除资料或改变安全设置。

本阶段只改变测试脚本及默认忽略用例所读的报告名；编译、显式实机预检、格式和脚本语法通过，生产源码及普通程序摘要仍为 `83DF0CAD2473770C28DD42738BF57619268A0B40B5E907E58832DA8E1AB32B4E`。完整 Rust 243 项/19 项忽略、普通 Clippy、前端 19 项及此前 279 项/构建是之前阶段的结果，本阶段没有重复这些完整检查。96 条声明、八条 Firefox 许可和两份许可证据 SHA 不变。汇总 `artifacts/software-verification/brave-preflight-checks.json`。真实 Brave 卸载、逐规则删除保护、重装拒绝、生产入口及 UI 仍待完成，完整目标尚未完成。

### 夜间定时跟进收尾

2026-10-01 的定时唤醒确认用户已经返回，按原自动跟进中的停止条件将 `dustaway` 设为 `PAUSED`，工具回执确认状态。没有启动新测试、卸载、删除、证书导入或改变安全设置，不新增验收通过项。代码、普通程序和上述证据保留；商店测试证书的临时信任仍待明确授权，完整功能继续按待办推进，不记为完成。

## Brave 交互卸载与逐规则证据任务（2026-10-01）

用户要求开始下一步后，新增默认忽略的 `real_brave_interactive_rules`。用例限定已标记闲置电脑、普通用户和 PE 版本 `154.1.96.60`，逐项核对 V3 种子报告摘要及真实文件，再捕获 11 条独立范围。调用实际 Windows 软件卸载入口，保持原登记参数 `--uninstall --verbose-logging` 和官方向导；不写入静默参数、不修改卸载登记，不从报告重建上下文。第一次编译的引用类型错误、修正编译和最终编译日志分别保留，未将失败记作通过。

08:50:49 +08:00 单次启动 `DustAway-Brave-Rules-20261001`，原生驱动 SHA-256 `B9A6D68364F10C5E0A8F3B4593B4C659C56677D313DF5721A9E707E193AA979C`。08:55:23 只读观察确认原驱动 5172、setup 9324/6724 及其向导进程仍在普通用户会话 2，阶段 `waiting_for_brave_uninstall_wizard`。已经请求用户保持删除浏览资料选项未勾选后完成官方向导；此时没有完成结果、没有残留删除，不将进程存在视为卸载成功。只能继续观察原任务，不重放、不强制结束、不重新安装。

用例在确认原安装身份、程序和进程均消失后，才以同一内存上下文检查停止、整配置/未知范围拒绝，并逐条检查排除、占用、修改、硬链接、替换、部分扫描、新增文件及未选择文件摘要。删除阶段使用仅测试构建可用的证据判定，共用生产范围、身份及文件句柄保护；每条先确认正式入口仍拒绝未验收规则。它不会授予生产许可，也不覆盖实际重新安装拒绝、正式页面或云提供程序。上述是用例设计，须等待原任务报告才能记录实机通过。

本轮完整 Rust 工作区 243 项通过、0 失败、20 项默认忽略，Clippy `-D warnings`、格式和 PowerShell 解析通过。日志前缀 `artifacts/software-verification/brave-rules-`。只有测试代码改变，普通程序 SHA-256 仍为 `83DF0CAD2473770C28DD42738BF57619268A0B40B5E907E58832DA8E1AB32B4E`，未发布、未重新构建普通程序或重跑前端。96 条声明和八条 Firefox 许可不变；Brave 和商店删除仍未开放，商店证书信任仍待授权。

### 首轮失败与新的 V2 实例

用户完成向导后，原生用例通过原登记、程序和进程消失、其他软件身份不变的核对。08:59:14 封装退出 101：用例误把预先停止的扫描当作带警告的成功返回，而进程保护按既定契约返回“检查已停止，结果不完整”。原生日志停在 `uninstall_verified_checking_residual_protection`，没有执行任何残留删除。09:04:29 按卸载后摘要核对 211 个文件保持不变，原任务移除，原上下文失效。官方卸载器改写了 Preferences，变化路径单独保留，不将其算作拂尘删除或推断一定替换了文件身份。

修正仅涉及测试。预先停止须返回确切停止错误，已有停止检查回归通过；另对卸载后无法核验的独立范围记录保留，不能重新认领或计为删除通过。原失败源码摘要 `83BE3A267A8F8183E138F35CFEC84C953A5E541EAE4A088EE3CAA667A8E235D4` 与旧驱动保留。收尾工具第一次因 Windows PowerShell 无 BOM 中文解码错误在变更前拒绝，新的独立脚本改核对原错误文件 SHA 后成功；原脚本未覆盖。取证曾请求不存在的 processes 文件并失败，不构成种子执行失败或重跑依据。

09:06:06 新的普通用户测试安装返回 0，原标记资料保留；09:07:07 V4 种子通过 11 项、两次自然退出、测试 Cookie/Local Storage/IndexedDB 和既有书签读回。该轮书签已存在，没有重复创建。新[种子归档](evidence/software/brave-rules-seed-20261001.json) SHA-256 `F4CB0F83712B0B5D507336EC314EAD306A43018554C06533BEF5EDF6C86BA853`，不沿用上一轮文件哈希。V2 驱动摘要 `DEAF6F7821806FBA0C634EFBFDD65C0FFA25E7A0186C2042A19304C8C936FF3E`，09:09:09 单次运行，用户再次保留浏览资料并完成官方向导。

09:10:32 读取到 V2 原生报告通过：11 条独立规则均完成测试专用删除及保护验证，39 个冻结文件、3,900,481 字节、6 个空目录；6 处树范围的新增文件及非空目录保留，所有未选择文件和共享见证摘要不变。五个单文件规则的 `new_files_preserved=false` 表示没有树内新增文件场景，不能误读为删除了新增文件。变化/占用/排除/硬链接/替换和部分扫描逐条验证；整配置及未知范围拒绝。这一轮没有触发“供应商改变范围身份”的保留分支，不能将该新分支称为实机覆盖。原始[逐规则报告](evidence/software/brave-rules-20261001.json) SHA-256 `8BE3789CD51DB153D7604CED627364599CCC2CDDB678245EFBFCD5B046399753`。

外围 `Start-Process -Wait` 还等待由 setup 17644 启动的 Chrome 10452 及其子进程，因此原生驱动已经消失时封装尚未保存退出码。没有强制结束浏览器或把报告代替原进程退出记录；已请用户关闭刚才自动打开的 Chrome 窗口。原生报告通过与外围任务收尾分别记录，需拿到原封装结果后才移除任务。以后新的封装应单独持有并等待原生进程句柄，不能修改已经执行的脚本或通过重跑补造原退出码。

第二次完整 Rust 工作区仍为 243 项通过、0 失败、20 项默认忽略；原生交互项已取得上述单独报告，但不计入默认运行的通过总数。工作区 Clippy、格式及新脚本语法通过，日志前缀 `brave-rules-v2-`。生产源码、普通程序、96 条声明、八条 Firefox 许可及旧两份许可证据摘要不变；前端未修改或重跑。本轮使用测试证据判定，尚非 `WindowsSoftware.delete` 的正式允许路径，真实重装拒绝、正式页面、其他版本和云提供程序依然没有覆盖，Brave 不增加许可。

### 原进程退出与任务收尾

用户正常关闭卸载器额外打开的 Chrome 后，原封装于 09:25:05 +08:00 保存 ExitCode 0，09:25:25 原任务为 Ready / 返回 0。归档 stdout 显示原生显式用例 1 项通过、0 失败，实际耗时 56.04 秒；外围等待浏览器的时间不计为残留处理耗时。没有重跑、关闭其他应用或补造退出码。

09:26:15 再次核对 173 个未选择文件的卸载后哈希保持不变，6 个后来新增文件及共享见证完整。四个注册表视图无 Brave，测试浏览器程序不存在，相关进程及 19839 测试监听为零；核验精确动作和终态后移除本轮三个临时任务。标记资料、所有成功/失败记录和执行过的脚本保留，没有强制结束或改变安全设置。详见[收尾归档](evidence/software/brave-rules-finished-20261001.json)，阶段汇总 `artifacts/software-verification/brave-rules-checks.json`。本阶段完成，不表示 Brave 正式残留删除或全部原方案验收完成。

## 简化卸载操作流程（2026-10-01）

按用户“执行你的建议”调整界面，正常流程为“选软件 → 卸载 → 自动检查残留 → 选择清理或暂不清理”。详情首屏直接提供卸载按钮和影响说明；安装信息、三类能力、缺失信息、保留原因及完整统计按需展开。切换页面继续自动衔接只读检查；其他操作忙碌时等待。已结束且没有原内存范围的旧任务收起为结果入口，不自动卸载或恢复删除。返回列表及重新查看仅改变呈现，并清除未确认选择。

新增六项页面回归覆盖跨页只扫描一次、忙碌等待、暂不清理/重开不执行、焦点恢复、缓存批选排除资料及只读项目、旧完成任务收起和仅保留结果完成入口。完整前端 21 文件/285 项通过，脚本 20 项通过；Rust 工作区 243 项通过、0 失败、20 项专用验收默认忽略。前端构建通过，保留既有大于 500 kB 分块提示。日志前缀 `artifacts/software-verification/simple-uninstall-`。

早期页面运行的旧 UI 断言及恢复提示重复失败分别保留；修正后的单文件 fork worker 启动超时未执行测试，随后完整运行以单 worker threads 完成，实际耗时 304.10 秒。首次原生模拟可靠性与四布局检查通过，证据 `simple-uninstall-native/verification.json`；查看截图发现小窗口操作栏遮挡内容，后续取消浮动栏、压缩残留搜索与重复返回入口，并新增操作栏不得覆盖可见残留行的几何断言。调整后页面 25 项和前端构建再次通过；最终原生复测和普通程序恢复另记如下。

本轮未修改 Rust、IPC、规则声明或生产许可，没有远程操作、真实卸载或文件删除。模拟界面证据仅验证交互，不作为真实残留规则开放依据；Firefox 八条精确版本许可保持，Brave 和商店删除许可未增加。

最终原生复测通过并检查截图，证据 `artifacts/software-verification/simple-uninstall-native-final/verification.json`，程序 SHA-256 `975270C7C1EAF150528B9944E91E95ACD966CC3BEBA8558DEAEF763229C5C7AF`。两种尺寸/明暗主题均无横向溢出，操作栏不覆盖残留行；首屏卸载、折叠详情、键盘焦点、返回与重开不回放、重复请求/超时重连、取消确认、清单替换、资料默认未选、父子计数和实际统计均通过。V2 首次几何断言误将折叠详情里的隐藏行计入遮挡，失败日志与截图保留；修正可见行过滤后用同一二进制复测，没有通过修改产品权限消除测试失败。

普通程序已用 `cargo build -p dustaway --locked --features custom-protocol` 恢复，SHA-256 `93D5C9DFD6823825B7F57BA9D6262AA52B87F44E9FCD52A13FA240F3CE474FAE`。本轮未生成发布包或继续远程验收。当前交付为简化界面，剩余真实产品规则开放不在本次完成声明中。汇总、源文件/程序摘要和测试清理状态见 `artifacts/software-verification/simple-uninstall-checks.json`。

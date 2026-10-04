# 专用软件卸载验收样本

这些工具不会随 DustAway 发布。原生驱动限定到已授权的闲置电脑及当前用户独占目录，只允许固定的测试注册表身份或 MSIX 包身份。不能用软件名称模糊匹配来扩大目标，也不能把生产残留删除开关打开来替代真实规则证据。

- `Fixture.cs`：.NET Framework EXE 安装/卸载样本，测试正常、延迟、提前退出、返回零但保留登记、重装和恢复。只操作固定前缀的 HKCU 登记和标记目录中的测试文件。
- `Build-Msi.ps1 -OutputDirectory <新目录>`：使用 Windows Installer 数据库 API 构造独占注册表组件的 MSI。仅生成包，不安装。`-Scenario Completion` 和 `Restart` 使用不同产品代码、组件及登记值；后者仅在移除时执行 ScheduleReboot，包内 `REBOOT=ReallySuppress` 与生产 `/norestart` 双重阻止实际重启。
- `Build-Msix.ps1 -OutputDirectory <新目录>`：使用 Windows SDK 22621 的 MakeAppx/SignTool 构造专用 MSIX。短期签名证书的私钥不导出，签名后移除本地签名证书和私钥。安装设备的临时 TrustedPeople 公钥证书需要在验收结束后按确切指纹移除。
- `Build-MsixData.ps1 -OutputDirectory <工作区 artifacts 下新的 store-data-msix-* 目录>`：打包新的 ApplicationData 样本；先执行 `cargo build -p dustaway-windows --example store_data_fixture --locked`。固定包名 `DustAway.Acceptance.Data.20261001`、版本 `1.0.0.0`，与旧 MSIX 独立。脚本不安装或添加信任，临时签名私钥不导出且签名后移除；每次构建使用新目录，保留失败日志。包内保持 Windows 默认文件/注册表虚拟化，不增加禁用虚拟化的声明。
- `Run-Packages.ps1`：在样本目录、指定电脑的普通用户交互会话中运行。安装准备可静默；待验证的 MSI 卸载保留系统交互，不加入静默参数。失败后不得直接重复整个脚本。
- `Run-MsiSecondRound.ps1`：只安装和卸载新建的 Completion/Restart 两个固定样本，检查包哈希、单次启动标记与独立任务数据库，禁止重放旧任务。MSI 跟踪上限为 600 秒；待重启用例验证残留检查、立即复核及恢复后的复核均被阻止，核对启动时间没有变化。
- `RequireAdministrator.manifest`：只为专门的 UAC 取消样本生成带提权清单的 EXE；须确认人工在场后再启动取消测试。

Rust 驱动为 `crates/windows/examples/software_acceptance.rs`、`software_package_acceptance.rs`；`software_inventory.rs` 和 `software_store_probe.rs` 是只读诊断。先在开发机编译，再复制到指定闲置电脑。不要在开发机安装样本，也不要修改保护条件来测试现有软件。

`software_store_policy_check.rs` 只读对照系统的当前用户主包策略快照，核对生产发现结果和受保护项目的准备拒绝；不调用卸载。实际组件用于验证保护，不作为移除样本，报告仅输出数量。

`software::tests::missing_fixture_store_package_returns_real_system_error` 默认忽略，只可把本地编译的 Windows 库测试程序复制到已授权测试根目录，以普通用户运行指定测试。它先核对固定 Missing 包族不存在，再调用与生产相同的私有 PackageManager 移除函数，要求返回 0x80073CF1，核对全部安装身份未变化。这只证明真实 API 错误传递，不代表生产准备接口会允许卸载不存在的包。

`Bundle-Desktop.ps1` 打包已编译的 e2e-fixture 程序和匹配的 EdgeDriver/tauri-driver；不得打包为交付版本。`software-desktop-check.mjs --reliability` 丢弃一次已实际受理的 IPC 回执，等待真实 15 秒超时后核对受理回执，验证相同请求返回同一任务、切页显示状态及 WebView 重载不重复卸载。`software-exit-check.mjs` 用至多 180 秒的模拟任务验证原生退出提示，人工先点“否”、再点“是”；首次取消由调度端写入专用确认标记后继续。两者均检查 fixture 标记，不调用真实卸载器。

退出脚本的 `--recover-existing artifacts/dustaway-e2e-exit-<标识>` 模式只打开已有模拟任务，不启动卸载。目录必须直接位于当前工作区的 artifacts 下且带有 fixture 标记，任务须为 Unknown。报告单独写入 `software-exit-recovery-verification`，标记 `recoveryOnly=true`，不能替代人工取消和确认退出的证据。正常退出与异常中断的提示文案不同，页面检查应显示实际保存的状态；任务断言另外写入 `runtime-verification.json`，避免后续截图或页面断言失败掩盖已完成阶段。

`software_verify_recovery.rs` 专用于核对已经启动过的固定 MSI 样本任务。它先确认原登记和 MSI 产品身份消失，再恢复原任务并核对结果；没有启动卸载的代码。已记录任务不能通过原始安装/卸载脚本重放。

该工具的 `restart-preflight` 模式限定第二轮 Restart 样本，验证原任务身份与重启前的复核、残留检查拦截；`after-restart` 模式须有匹配的预检记录且系统确已重启，才能重新核对安装状态。`Run-MsiRecovery.ps1 -Stage BeforeRestart|AfterRestart` 在普通用户会话运行工具，记录启动时间、文件哈希和结果；后一个阶段还与预检记录比较 Windows 启动时间。工具及脚本本身均不重启系统、不调用卸载，也不恢复内存中已经丢失的残留删除授权。实际系统重启须在用户保存工作并确认后单独进行。

原生驱动日志中的退出或超时均不代表卸载成功。驱动超时不会终止卸载器；必须查看正在运行的系统向导，再复核原安装身份。持久化任务存在时包驱动拒绝重复启动，只有“准备失败且没有任何任务记录”的空数据库允许重试准备。

`software_acceptance --uac-cancel` 单独等待人工响应至多 600 秒，不自动操作 UAC 提示。管理员策略若为不询问直接提权，须先停止该用例；本轮经用户明确授权临时将 `ConsentPromptBehaviorAdmin` 从 0 改为 5，由独立控制任务在结束或超时后恢复。人工点击“否”的用例已通过，原策略和临时任务清理结果保存在 `uac-final-status.json`；此授权不适用于后续测试中再次更改策略。

这些样本只能证明通用执行与状态保护行为，不能证明 Chrome、微信等真实产品目录可安全删除。已完成项、失败尝试和待处理事项见 `docs/verification-software-uninstall.md`。

## 新 MSIX 的真实数据样本（仅准备完成）

`crates/windows/examples/store_data_fixture.rs` 只接受单个 `seed` 或 `verify` 参数，在指定闲置电脑的普通用户下核对原验收根标记、当前精确包身份和包内程序位置后运行。通过 Windows `ApplicationData` 和 `FileIO` 写入/读回专用包自己的 LocalCache、TempState、LocalState、RoamingState 文件，以及 LocalSettings 键值。LocalCache 标为持久资料，TempState 在复核时若被系统回收则报告缺失，不重新生成来冒充正向证据。它不接收路径或其他包名，没有卸载、删除及生产授权入口。

每个阶段先以 FailIfExists 写入单次标记，之后只处理固定的测试文件；失败保留，不删除标记重放。结果和失败报告都保存在该包 LocalState，控制端必须在卸载前归档，不能通过关闭虚拟化来绕开包数据语义。先以包身份激活 `seed`，自然退出后再独立激活 `verify`；直接双击 EXE 的拒绝不代表包内实际运行通过。实际包内激活、真实数据生成、标准卸载和残留保护仍待验收。

2026-10-01 07:19:39 +08:00 已生成本地 `artifacts/store-data-msix-20261001/DustAwayAcceptanceData.msix`，包 SHA-256 `B23BB9234C598DA79E08D7728E9B0057079EBC0DB41087BA4F96AE896C2840DE`。已核对包内程序/清单与构建输入摘要一致、签名项存在；这不等同于目标机部署成功或可信签名验证。短期签名证书指纹 `86E1409FDD915DBD0A2875F53BF24429BABDC758`，开发机签名证书及私钥已移除，没有添加任何信任，也没有在闲置电脑安装新包。此包不会随 DustAway 发布，商店残留删除许可仍为空。API 语义依据 [微软 ApplicationData 文档](https://learn.microsoft.com/en-us/uwp/api/windows.storage.applicationdata?view=winrt-26100)，签名流程依据 [MSIX 签名文档](https://learn.microsoft.com/en-us/windows/msix/package/signing-package-overview)。

上述初版包仅作为原始构建记录保留。07:31:50 新增启动握手后的匹配包位于 `artifacts/store-data-msix-20261001-handshake/DustAwayAcceptanceData.msix`，SHA-256 `E2248F341B0EA92D5E8AACEBF923026F29DC45C73A2C92B2D446F6187E575A2E`；样本 EXE 摘要 `3DFBB5BA09DCF99F50FC0CB87FE29823C248505BFFDF4BA14D792091D44F5666`。当前 `store_data_control.exe` 摘要 `5598F519976F571591350CA4F08AF7DE0F90748A903F6084281E1C6C0E9D99DD`，只认可这份样本 EXE，不能配合初版包。新临时签名证书指纹 `EB799D221A9CCECDFC7676C4A262D80EE406325C`，签名证书/私钥也已从开发机移除，未添加信任。已向用户请求本轮测试设备临时 TrustedPeople 信任许可；在获得明确答复前不导入证书或部署包。

控制端只支持 `preflight`、`seed`、`verify`、`observe seed`、`observe verify`。它先保存不可覆盖的启动意图，再通过 [ActivateApplication](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationactivationmanager-activateapplication) 激活固定 AUMID；样本等待本会话固定名称事件，直到控制端核对并保存 PID、创建时间、所属用户、会话、包全名、程序位置。控制端只申请查询和等待进程的权限，随后发出继续事件，等样本自然退出。超时不结束进程；observe 分支只核对原进程和归档，不激活、发事件或根据旧记录再次启动。丢失句柄又没有已保存退出结果时不能推断成功。

退出码为零、报告身份及全部四类范围证据一致后才生成 `msix-data-<阶段>-completed.json`，包含包内报告和其原始字节摘要；保留缺失 TempState 的真实结果。`Run-MsixData.ps1 -Stage Preflight|Seed|Verify|ObserveSeed|ObserveVerify` 是普通用户交互会话的封装，固定控制端摘要，每次输出独立日志；它不安装包、导入证书或调用卸载。必须在卸载前归档完成记录。六项本地测试只验证身份、报告完整性、临时数据缺失含义及单次记录，不能当作 WinRT 激活握手实机通过。

07:33:29 首次远端 Preflight 因专用包未安装而按预期拒绝，但封装的异步 Process 对象丢失 ExitCode，原 JSON 为 null、任务错误地返回零；此原始失败保留。改用 Start-Process -Wait 并拒绝空退出码，07:35:20 再次只读预检正确记录原生退出 1 及“专用包未安装”，无激活意图或包数据生成。07:36:14 两个预检任务移除，控制端进程为零；已传送的控制端和脚本、所有证据保留，原 Brave 安装器未触碰。不要将这项预期拒绝标成已完成包内正向验收。

## Firefox 新配置的真实软件检查

`Run-FirefoxPrepare.ps1` 仅在指定闲置电脑的普通用户会话运行，要求不存在 Firefox 登记及 Local/Roaming Mozilla 资料。安装包必须来自 Mozilla、签名有效且哈希与本地清单一致；安装到本次验收目录，关闭快捷方式、维护服务和默认浏览器检查计划任务的安装选项。安装选项依据 [Mozilla 完整安装器文档](https://firefox-source-docs.mozilla.org/browser/installer/windows/installer/FullConfig.html)。

`firefox-profile-check.mjs` 使用官方浏览器命令行，在 `127.0.0.1:19837` 的本地页面产生测试 Cookie、本地存储和缓存。仅使用命名的新配置，不登录账号。Windows 启动器使用 `-wait-for-browser` 等待实际浏览器退出，参见 [Mozilla 启动器说明](https://wiki.mozilla.org/Platform/Integration/InjectEject/Launcher_Process/)。资料生成、重装前的保留检查及缓存处理后的重装检查分别记录，失败日志保留。

`software::firefox_acceptance::real_firefox_uninstall_residual_protection` 是默认忽略且只存在于测试程序中的验收项。调用生产发现、准备、标准交互卸载及只读扫描；确认签名、唯一安装、固定安装目录、新配置标记及两个标准残留根。卸载启动器可能提前退出，须保持跟踪原身份及实际 NSIS 卸载进程，不能再次调用卸载代替等待。

官方卸载器可能改变其自己的资料登记文件，因此分别保存卸载前文件哈希及官方卸载后的差异。只有卸载确认完成后，才通过测试专用入口调用底层文件执行器删除新配置中**一个已冻结的真实 cache2/entries 文件**；全部 Roaming Firefox 文件以官方卸载结束后的状态为基线，核对缓存删除未改变资料。另验证共享父目录、排除、变化、占用、硬链接、新增文件、非空目录、停止及部分扫描告警。生产候选必须全部只读，生产删除调用必须被拒绝。这不是整目录、配置资料、云文件或多版本规则的开放凭据。

独占内容句柄不一定阻止 `FILE_READ_ATTRIBUTES` 读取属性，因此占用用例只要求删除被拒绝；部分扫描用例使用会被文件身份检查拒绝的硬链接，并检查告警及候选缺失。不能为了让测试通过而放宽生产删除检查。

`Run-FirefoxResidualSupplement.ps1` 仅补测此次固定配置的残留保护，不启动卸载器。对应的默认忽略用例要求原第二轮记录恰为已知断言失败、真实 Firefox 登记与进程已消失、资料哈希仍与官方卸载后清单一致、各测试见证文件未进入后续删除阶段。它建立**新的测试专用扫描上下文**，使用从未写入注册表的合成身份，重新冻结文件；不恢复生产任务的旧授权。报告明确记录 `supplement_only`、原完整用例失败及新上下文来源。单次标记阻止重复执行，实际删除限一个真实缓存文件；不能把补测退出码替换原始失败记录。

`Run-FirefoxReinstall.ps1` 使用同一安装包恢复测试安装，在打开浏览器前核对标记配置内的文件哈希，再验证原 Cookie 与本地存储；完成后用官方 `/S` 卸载进行测试安装收尾，保留验收资料。这个静默收尾不冒充前面的交互卸载验收；参数依据 [Mozilla 企业卸载文档](https://support.mozilla.org/en-US/kb/silently-install-uninstall-firefox-enterprise)。

`Run-FirefoxDesktop.ps1` 与 `firefox-desktop-check.mjs` 补齐普通版拂尘的真实页面流程，要求上一阶段收尾完成，测试安装路径及配置固定。程序使用标准 WindowsSoftware，断言版本不带 fixture 标记，保留现有设置和历史；任何既有真实卸载任务都会阻止启动。页面准备确认后等待单独的人工在场信号，只提交一次卸载。用户完成官方向导且后台确认程序、进程、安装登记消失后，再发完成信号，通过页面重新核对原任务，检查残留告警、只读勾选、实际删除量为零及唯一历史记录。只读查询可直接核对 IPC，所有业务写操作由真实页面触发；不模拟响应、不开放生产删除权限、不重放旧任务。新目录及单次标记防止整段重试；卸载提交后的异常保留程序供核对，不能强制结束卸载器。

用户可能先于开始信号点击已显示的拂尘确认框。驱动此时应观察并使用已存在的同一任务，跳过卸载按钮；IPC 调用计数使用 fetch 传输层，不能替换本机不可写的 `__TAURI_INTERNALS__.invoke`。这些修正不能追认旧计数有效。此次已有流程通过 `Run-FirefoxDesktopContinue.ps1` 和 `firefox-desktop-continue.mjs` 接续：限定原任务与仍存活的 WebDriver 会话，验证原成功卸载回执，从页面继续核对和只读扫描，不包含准备或执行卸载的接口调用。原驱动未完整运行，接续报告单独标注，未测量的卸载调用次数及原卸载前全量身份比较不得声称通过。详见验收记录中的分阶段证据。

第二版残留范围的卸载前预检查：`Run-FirefoxV2Prepare.ps1` 仅在原标记测试范围重新安装已核验的 Firefox，再读取测试 Cookie/本地存储并运行 `software::firefox_scope_acceptance::real_firefox_v2_scope_preflight`。它不调用卸载或残留删除，也不保存可恢复授权。测试绑定签名程序 PE 版本 140.17.0.1158；实际存在的范围逐条核验，缺失范围不计为通过。`Run-FirefoxV2ScopeCheck.ps1` 只接续现有测试安装，不能重放安装。运行记录与一次性标记防止重复启动；先检查确切任务和进程状态，不因观察超时就重试。浏览器脚本新增可选输出前缀，接续结果与原失败证据分开保存。

2026-10-01 的预检查曾通过 7 个现存范围，会话备份目录没有生成。该预检查及合成界面结果不开放生产残留删除，也不能用预检查 JSON 重建删除权限。后续逐条真实删除见下节；目前专用 Firefox 已卸载，不能重放此前脚本。

## Firefox 逐条残留删除

`Run-FirefoxRules.ps1` 是已执行的一次性证据采集：同一进程捕获安装身份和独立范围，官方静默卸载只为准备真实残留，逐规则核对排除、变化、占用、硬链接、路径替换和未选择文件的哈希。测试专用判定与生产共用范围、安装、进程及文件句柄检查，不开放运行时或 IPC 授权。六条完成后因书签备份为空而失败，原失败保留；空目录存在不等于真实文件删除通过。已经执行的标记不能移除来重放。

`Run-FirefoxProductionRules.ps1` 的首轮在测试重新安装时遇到混合斜杠被安装器截断的问题，安装落到专用 DustAwayAcceptance 父目录。没有进入这一轮删除阶段。`Repair-FirefoxTestLocation.ps1` 核验精确登记、Mozilla 签名及 PE 数字版本后用官方卸载器收尾，比对原证据目录内 1092 份文件不变。最初版本预检误用了三段显示字符串，修正为四段数值字段；预检失败时没有启动卸载。保留两类失败及修复结果。

后续 `Run-FirefoxProductionIni.ps1` 使用[官方 INI 配置](https://firefox-source-docs.mozilla.org/browser/installer/windows/installer/FullConfig.html)和规范化的 Windows 路径，建立新实例、新上下文和独立报告。生产入口六条规则及重新安装保护均通过；所有安装/卸载仅涉及同一已核验 Firefox 安装包和专用测试范围，不是交互 UI 验收。`firefox-production-ini-20261001-report.json` 和逐条原始证据共同限定许可。书签备份、会话及其他版本未获准。

2026-10-01 04:53 +08:00 四个临时任务已移除，测试进程、Firefox 登记及两处测试安装均不存在，标记资料和日志保留。后续需要新的真实页面闭环时重新准备专用样本，不能从这些 JSON 恢复删除权限或重放已完成脚本。

`Run-StoreIsolationQuery.ps1` 在普通用户会话运行默认忽略的只读系统查询。报告只保存包身份摘要和归属检查结果，不保存包路径、用户 SID 或可恢复上下文。它不调用安装、卸载、证书或残留删除操作；运行过的单次标记和报告不得覆盖。

`Run-StoreSharedGuard.ps1` 检查两个指定且未安装的 DustAway 测试包族：临时空共享容器存在时，生产残留归属检查必须拒绝；移除该容器定义后，重新执行卸载后的关联查询。容器准备/收尾只在专用电脑进行，使用固定名称与创建 ID，确认两个包均未安装，不用强制关闭或重置容器。两阶段使用不同报告与单次标记。它证明共享成员不依赖安装登记，不证明真实 MSIX 卸载、文件删除或通用规则验收，生产许可仍关闭。

`Run-StoreProcessCheck.ps1` 使用普通用户令牌显式运行 `software::store_processes::acceptance::real_store_process_queries_and_lifetime`。先只读识别实际运行的包进程，再在有独立 UUID 标记的专用子目录启动测试程序副本：没有包身份的子程序仍应阻止该安装目录的残留处理；发送只属于本次子程序的管道令牌后自然退出，重新检查通过。父程序异常关闭管道时子程序也自然退出，不调用任何进程终止接口。报告只保存数量与结果，无应用包身份、SID、安装路径或可恢复权限。专用测试程序不能被当作真实 MSIX 卸载或残留删除证据。

### Firefox 正式页面删除及脚本失败续测

`Run-FirefoxDesktopDelete.ps1` / `Run-FirefoxDesktopDeleteV2.ps1` 和 `firefox-desktop-delete.mjs` 只接受专用电脑、标记目录、原 Mozilla 安装包和正式应用哈希。使用规范化 Windows 路径的 INI 安装，回环页面生成 Cookie/本地存储/缓存；只对本次专用 HKCU 安装登记添加官方 `/S`。所有卸载和残留操作由拂尘按钮发起。它不验证供应商向导人工操作，也不增加运行时删除权限。

第一次脚本在选择前因路径分隔符字符串比较失败；失败的计划任务结束后应用也结束，没有残留删除。第二次修正后完成真实卸载与四布局检查，但批量 WebDriver 长路径输入丢字。此时脚本保持原进程和会话，`Run-FirefoxDesktopDeleteContinue.ps1` / `firefox-desktop-delete-continue.mjs` 仅接受记录中的同一任务、会话、扫描 ID 和“一个卸载、零次删除”的状态，逐字输入完成选择、两次确认准备、取消、实际删除、结果、历史与重连。续测成功后才发出专用完成标记，让原驱动正常关闭应用。原失败不覆盖为成功。

完成于 2026-10-01 05:23:55 +08:00；三份文件 524,814 字节，非空父目录和新增文件保留，未选/共享文件哈希不变。原脚本退出码 1，续测退出码 0。详见 `docs/evidence/software/firefox-desktop-delete-20261001.json` 及验证文档；续测同名布局截图处于无匹配搜索、重连截图中独立清单核对产生代次冲突均有明确限制，不能隐藏。原非空清单截图已在续测前本地归档。主脚本目前已改用逐字输入，并等待重连后的页面清单再做独立清单审计，后者尚未再次实机执行。

05:25:51 +08:00 三个专用任务已移除，应用/驱动/Firefox 进程、测试端口和 Firefox 登记均为零，主题恢复，标记数据和日志保留。安装、卸载和清理都是已完成的一次性操作，不移除单次标记重跑；后续场景须按当前状态另行准备，不能用证据 JSON 恢复内存删除权限。

### Firefox 书签与会话备份

`firefox-history-seed.mjs` 使用 [Mozilla geckodriver](https://github.com/mozilla/geckodriver/releases/tag/v0.37.1) 连接专用标记配置。正式下载包摘要与官方发布元数据一致；解压程序摘要在每轮清单内再次核对。仅在该浏览器实例传入 [allow-system-access](https://firefox-source-docs.mozilla.org/testing/geckodriver/Flags.html)，不修改 Windows 安全策略。书签通过 Firefox Places 接口创建并备份；会话在 `TabStateFlusher.flushWindow` 后写入，解码核对两个回环测试页面和下次启动读取的上一会话。两次正常退出后才运行残留场景。

首轮 `Run-FirefoxHistoryPrepare.ps1` 因回环 favicon 请求的未处理异常退出。`Run-FirefoxHistorySeedContinue.ps1` 连接原浏览器后发现会话内容尚未刷新，未进入残留删除。保留原驱动及浏览器检查句柄，通过正常退出关闭；`--connect-existing` 的 WebDriver DELETE 只脱离会话，不能当作浏览器退出证据。修正服务器与会话刷新后，`Run-FirefoxHistoryFinalSeed.ps1` 完成新的两次正常启动/退出，原失败记录保留。

`Run-FirefoxHistoryRules.ps1` 显式运行默认忽略的 `real_firefox_history_rules`，仅采集两条规则的独立证据；生产权限仍关闭。05:58:06 +08:00 两条均通过。证据绑定后，`Run-FirefoxHistoryProductionPrepare.ps1` 使用新的 INI 安装并重新生成真实备份，`Run-FirefoxHistoryProductionRules.ps1` 通过正式删除入口与真实重新安装保护，06:08:00 完成。前者的旧阶段/文件名不可重用；后者不再使用证据判定绕过许可表。

选择范围同时限定到确切标记配置和规则，不按规则 ID 扫选其他配置。每轮独立 `CreateNew` 标记防重放；JSON 只保存诊断与结果，不能恢复删除上下文。真实数据、变化/占用/排除/硬链接/替换、新增文件、未选哈希和非空目录保护分别断言；报告的文件数量可能包括前轮留下、此次用户选择范围内的测试见证，不全部称为浏览器生成文件。此次没有真实页面交互、云同步或其他版本证据。

06:10:29 +08:00 六个临时任务已移除；测试进程、监听、安装登记和专用 Firefox 均不存在。标记资料与原始失败记录保留，没有强制结束进程或改变系统设置。后续验收必须按这一状态准备，不能重放已完成的样本。

### Brave 准备历史与只读范围证据

已只读确认闲置电脑没有 Brave 登记、数据根或进程，随后取得 [官方 v1.96.60 离线安装包](https://github.com/brave/brave-browser/releases/tag/v1.96.60)。外层包 SHA-256 为 `1F420CC550CF70CCFE555802E1BA6AE2D4D901288C119D678790BC7AE5D8EAE8`，发布摘要匹配且 Brave 签名有效。首次代理下载中断记录保留；确认下载进程已终止后才续传、完成核验，未执行不完整包。

`Run-BravePrepare.ps1` 已于 06:27:42 +08:00 单次启动，**不要重放或覆盖这个已运行的脚本**。原外层进程 14876、子进程 5872 和计划任务 `DustAway-Brave-Prepare-20261001` 仍待核对，尚无准备成功结果。当前脚本误把 Chromium 内层参数传给外层 Omaha 包，命令行转发已由子进程观测和 [Omaha 元安装器源码](https://raw.githubusercontent.com/google/omaha/main/omaha/mi_exe_stub/mi.cc)核对；是否显示错误提示仍未获画面验证。RDP 两次捕获超时，后续窗口操作返回坐标几何不可用，未盲点、强制结束或另起安装。先核对同一进程的结束结果，必要时由用户提供窗口提示；超时不是允许重装的依据。

原进程已解出的内部安装器经过有效签名、PE 版本 `154.1.96.60` 与 `OfflineManifest.gup` 摘要核验，于 06:45:29 只复制保存到专用测试根 `brave-inner-installer-154.1.96.60.exe`，SHA-256 `6BF976C55EE239065ACC5702035D590EAF6A85B6E92EF7724B79A872E098A66E`。**未执行**。外层、内部包、准备脚本和结果分别保留，不用内部包启动来掩盖原安装器的未结束状态。

`brave-seed.mjs`、`Run-BraveSeed.ps1` 和默认忽略的 `real_brave_scope_preflight` 仅完成语法/编译准备，未部署或运行。种子限定标记标准配置、普通用户及回环页面，拟通过真实浏览器服务生成 11 个独立范围的数据；调试地址取自本次子进程输出，不能复用旧端口文件，每次正常退出须核对退出码。预检不授予删除权限，后续还需逐条真实删除、资料保护、重装拒绝与生产入口及页面验证。不能把已编译、只读预检或其他浏览器证据算作 Brave 已验收。

08:12:19 +08:00 原 Brave 安装器返回 `-2147218431`，封装与任务退出 99；08:13:27 原进程均已消失。失败资料归档，原脚本及标记不能重放，也不能覆盖为成功。准备新流程前须重新核对无登记和专用标记目录。

08:16:47 再核对四个注册表视图均无 Brave，产品根仅有本轮标记，内层包签名和摘要一致。`Run-BraveInnerPrepare.ps1` 使用已保存的官方 mini installer，并按 [Chromium 安装程序说明](https://chromium.googlesource.com/chromium/src/+/main/chrome/installer/setup/README.md)将 installerdata 交给实际支持它的程序；[初始偏好](https://www.chromium.org/administrators/configuring-other-preferences/)限制本次普通用户安装不启动浏览器、不设默认浏览器、不创建快捷方式或导入其他浏览器资料。原封装失败不能由本轮成功覆盖。08:19:42 原生及封装退出 0、有效 Brave 签名和 PE 版本 `154.1.96.60` 核对通过，浏览器摘要 `B7995F5B0926F45804DC12D5EA8057E7DB760DCB6AF2F18565B21A43EE3AE560`，未进行卸载或删除。

安装器已在新的 User Data 中创建 Crashpad，最初种子传输预检因此拒绝，没有运行。随后只读元数据证实该目录只含安装时间窗口内的 Crashpad、无 Default 配置，未读取其内容。种子封装仅在这些条件和原专用根标记全部成立时添加新的验收标记；发现任何其他既有内容均拒绝。`Run-BraveSeed.ps1` 绑定新的 inner-prepare 成功结果和 ready 来源，并明确拒绝空原生退出码。08:22:54 已单次启动；在实际报告与退出结果核对前，不能称为真实资料生成完成。

原种子和 V2 均已失败结束，分别为内部页面地址规范化和书签私有模块不存在，原记录不可覆盖；对应 inspect/close 工具只操作固定的原 PID、父进程、回环端点和已核验配置，以 Browser.close 正常退出并由原句柄确认退出码。它们是已经完成的一次性现场恢复工具，不能再运行。V3 使用实际页面的 `chrome.bookmarks` 服务；08:31:53 完成两次正常退出、资料/书签重开读回和 11 个非空独立范围。没有手工生成书签或数据库文件，未验证会话自动恢复和云同步。

`Run-BraveScopePreflight.ps1` 显式运行默认忽略的原生只读用例，绑定 V3 报告。08:34:07 已通过 11 项独立归属、共享/整配置排除和安装中禁止扫描；没有卸载、删除或增加许可。08:35:26 六个 Brave 临时任务已移除，测试进程与监听为零，测试安装和资料保留。后续应捕获新的内存上下文再进行真实卸载/保护验收，不重放已完成准备和种子，不从 JSON 恢复权限。证据与失败边界见 `docs/verification-software-uninstall.md`、`artifacts/software-verification/brave-preflight-checks.json`。

### Brave 交互卸载与逐条残留证据

`Run-BraveRules.ps1` 只运行新的默认忽略用例 `real_brave_interactive_rules`，限定已标记闲置电脑、普通用户、固定版本和 V3 种子摘要。原生程序在同一内存上下文中调用实际标准卸载接口，原登记参数必须为 `--uninstall --verbose-logging`；需要用户完成官方向导并保留浏览资料，不添加静默参数。封装和原生各自使用 CreateNew 标记，重连只观察原进程，不能重放或从 JSON 重建权限。

安装登记、程序及相关进程全部消失后，才进行逐条保护检查和冻结文件删除；共享父目录、整个配置、未知范围、未选文件和后来新增文件保留。该用例使用仅测试构建可用的证据判定，所有生产范围/归属/文件句柄检查继续生效，正式删除入口仍须拒绝没有许可的 Brave 规则。结果不覆盖重新安装保护、正式页面或云提供程序，不自动生成生产许可。

08:50:49 +08:00 已启动唯一任务 `DustAway-Brave-Rules-20261001`，08:57:28 仍为原驱动 5172 和官方 setup 9324/6724，等待用户完成向导，没有残留删除结果。后续只用 `artifacts/ssh-remote/read-brave-rules.ps1` 观察。必须保留已执行脚本、驱动摘要和单次标记；任务退出后归档原报告及错误，再决定下一阶段，不删除标记以重跑。

首轮后续已结束：标准卸载核对通过，用例因预先停止返回错误而退出 101，未删除残留；09:04:29 验证 211 个卸载后文件哈希不变并移除旧任务。原源码归档 `artifacts/software-remote-acceptance/brave-rules-first-source.rs`，原失败不可覆盖。`Run-BraveReprepareV2.ps1` 与 `Run-BraveSeedV4.ps1` 为新的单次实例，保留专用资料，重新验证实际浏览器数据；新 V4 种子只接受自己的准备报告。

`Run-BraveRulesV2.ps1` 使用修正后的同一原生测试名及新驱动、阶段名、见证名。09:10:32 取得 11 项通过报告、39 文件 / 3,900,481 字节实际删除；原生产许可表仍为八条 Firefox，测试不开放 Brave。原生程序退出后，外围 `Start-Process -Wait` 仍在等卸载器另行打开的 Chrome；已请用户关闭该窗口，退出码与临时任务收尾仍须从原实例取得。不能结束用户浏览器、重跑或改写执行过的封装；新的封装应缓存原生句柄、只等待该进程，并单独跟踪可能长期存在的后代。

09:25:05 原封装在用户关闭 Chrome 后退出 0；原 stdout 为 1 项通过、56.04 秒。09:26:15 三个本轮临时任务已移除，测试进程、监听与 Brave 登记均不存在；再次核对 173 个未选文件哈希、6 个新增文件和共享见证完整。不要重放 V2 或调用依赖已删除任务的旧观察器。下一阶段重新准备实际资料和内存上下文，不能从报告恢复授权；正式入口、真实重装拦截和页面仍待验证。

### 桌面跨会话进程保护

`Hold-DesktopCrossSession*.ps1` 经 SSH 启动专用测试子进程，`Run-DesktopCrossSession*.ps1` 在普通用户交互会话观察。父程序只持有自己的子进程句柄，收到确切控制标记或超时后发送本次令牌，使子进程自然退出；不强制结束任何进程。版本目录、脚本、摘要和 CreateNew 标记独立，已执行版本不能重放。

V1/V2 元数据访问失败，以及 V2 续测对 WTS 漏列的失败证据保留；续测使用原子进程，没有重新启动。最终 V3 检查 Toolhelp32 能看见子进程，无法读取用户时生产桌面检查返回 `process_unknown`，原父句柄确认退出 0 后恢复通过。报告明确限定为不可访问的同用户不同会话测试，没有包进程、卸载、残留删除或授权。商店现有 WTS 检查仍需修复和独立验收；不能把桌面通过用于商店许可。08:13:13 四个已结束任务移除，专用进程为零，资料保留。

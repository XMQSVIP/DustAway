# 首页垃圾清理：接续记录

更新：2026-10-01 17:00 +08。用户已确认退出钉钉，并明确改为“卸载钉钉，再处理残留”。普通首页闭环暂停，不再要求退出、登录或重装。禁止把登录成功等同于整个账号均为测试资料。

## 当前操作切换：钉钉卸载

- 已经通过普通程序发起一次官方交互卸载，用户确认完成，后台安装身份核对为 `removed`。任务 `b8081baa-afde-48d8-9bd9-f47f938ef54e`。四注册表视图中原 DingTalk 键与主程序均不存在。
- 第一轮未提交卸载：未加引号的空格路径 `C:\Program Files (x86)\DingDing\uninst.exe` 被严格解析拒绝。已修复唯一实际文件的无参数路径，仍拒绝竞争前缀、包装器、缺失/链接文件及歧义命令；新增两项回归通过。修复构建 `AD1A4592C1DCD6D85A045E88A778AB3C97362822F57ED534E181B6B1FF69E888` 才发起真正卸载，没有重复执行。
- 普通残留扫描不完整、可选项 0、实际删除 **0 B**，原因是钉钉卸载器在产品根目录，已核验主程序在 `main/current`，原归属比较未匹配。固定位置只读复查确认 `DingTalk_133/Default`、Cache、Code Cache、Local Storage 仍存在，不能把空清单说成没有残留。没有读资料正文或删除文件。
- 代码已增加仅针对签名识别成功的钉钉 `main/current/DingTalk.exe` 与同产品根 `uninst.exe` 的精确关联，其他祖先/产品/文件名仍拒绝，新增回归；不改变普通缓存与卸载残留的独立许可。此次未取得有效卸载前目录上下文，不能靠更新程序或持久化 JSON 补发删除授权。钉钉生产残留删除仍未开放；普通首页闭环也仍未完成。
- 远端证据目录 `20261001-home-cleanup/dingtalk-removal-20261001` 为原失败；`dingtalk-removal-path-fix` 为实际卸载与不完整扫描。脚本不得重放。自动跟进仍暂停。当前测试结果与收尾以 `docs/verification-software-uninstall.md` 最新段落为准。
- 本次修复后的普通调试程序已恢复，SHA-256 `BBA294194894D98D86B4A0294CA89308D268E28A6FB5B628F86B737367C3DA41`。路径修复完整工作区 254 通过 / 24 忽略，后加目录归属回归独立 1 项通过，格式、脚本语法及普通构建通过；前端未改。远端窗口和驱动已关闭、9 份原件哈希核验通过；尚未完成残留删除或普通首页真实闭环。

## 当前状态

- 目标：钉钉网页/代码缓存、百度网盘网页/代码/图形缓存，共五条。用户最新要求不等待微信七天，本轮跳过微信日志验收，不缩短年龄或改时间。
- 源码已增加两项独立普通许可：`dingtalk.http`、`dingtalk.code` / 固定文件版本 `8.2.0.119`，均默认不选。两项真实周期均通过，普通首页闭环仍待验收。用户明确未勾选自动登录，退出后登录页是正常行为，**不是失败**；后续确认登录后页面和测试资料正常，代码缓存已再生成 15 / 68,844 B。最新支持数 35 / 16 / 19。百度三条未完成清理。软件卸载残留权限与本轮无关。
- 用户已确认微信、百度网盘、钉钉全部登录。用户指定保护样本：`D:\BaiduNetdiskDownload\amazon_twister_parser.py` 已完成（19,316 B）；`py_project260907.zip` 已暂停，对应 `.baiduyun.p.downloading` 文件 40,266,470 B。只核对这两个明确指定下载，不涉及其他云文件。
- 自动跟进 `dustaway` 已从旧卸载任务改为本任务；本轮独立工作已收尾，剩余步骤需人工，现已 **PAUSED**（原每小时一次、最多八次设置保留）。等待用户回复退出钉钉后人工接续，不自动删除或重放。

## 远程与安装证据

复用专用 SSH：`artifacts/ssh-remote/Invoke-Remote.ps1`。目标 DESKTOP-24NJTT6 / MSN，Tailscale 100.69.113.49，固定已核验主机密钥；不读取密码，不改网络、防火墙、UAC。交互会话 2，安装器由 Limited / Interactive 计划任务启动。

专用根：`C:\Users\MSN\AppData\Local\DustAwayAcceptance\20261001-home-cleanup`，标记 `.dustaway-test-only` 内容 `DustAway.HomeCleanup.20261001`。本地证据 `artifacts/home-cleanup-real-20261001/`，脚本 `scripts/home-cleanup-acceptance/`。源码改动前部分文件快照 `before.zip`。

首次检查三款软件无安装登记、目标资料或进程；每个安装入口额外核对默认安装和资料路径。官方直连下载只对 curl 使用 `--noproxy '*'`，未改系统代理。安装包均核验签名 Valid 和 SHA-256，清单保存在 downloads 和远端根：

| 产品 | 来源 | 安装包版本 | SHA-256 |
| --- | --- | --- | --- |
| 钉钉 | dingtalk.com/download 官方链接 dtapp-cast.dingtalk.com | 8.2.0.260119001 | C4664B74F7B688FCBE30B951632EB6B0ABB820703908835695B5F82F8431CF56 |
| 微信 | pc.weixin.qq.com 官方链接 dldir1v6.qq.com | 4.1.15.1000 | D5F3BB4232C0F172BCE53141DB90BF60F5F0A53EB137663AB8D32E74E04B9F4F |
| 百度网盘 | pan.baidu.com/disk/cmsdata?do=client&adCode=1 官方 x64 链接 issuepcdn.baidupcs.com | 8.8.8.101 | 1A388C5D0A8B90D04BD2FB83652F8DECC19310E020A74A3587F955A90259922B |

安装包版本不代替安装后真实程序版本/哈希/签名。百度签名是项目已识别的 Beijing Duyou Science and Technology Co.,Ltd.，不是名称中带 Baidu 的发布者。

### 失败与重试（不要重复执行）

- 微信第一次调度早于 SCP 完成，缺少 manifest，任务结果 1，未启动安装器且无 started 标记。传输完成后 V2 调度成功，用户已登录。原失败日志保留。
- 钉钉首次启动后没有安装登记或进程，只留下创建于本轮首次启动时的 Roaming/DingTalk 目录。用户要求重装后，专用 `Retry-DingTalk.ps1` 核对原缺失证据、当前无登记/进程/Local 配置、固定安装包哈希，再启动一次交互安装器。没有删除或覆盖资料脚本。
- `install-inspect-4.json` 核对三个安装启动回执和钉钉重试标记。计划任务 Ready / 0 仅代表启动，不代表安装完成。
- `Prepare-Remote.ps1`、`Start-Installers.ps1`、`Download-BaiduRemote.ps1` 和各次启动脚本均已执行，不要重放。钉钉重试任务 `DustAway-HomeCleanup-DingTalk-UserRetry-20261001`，其余任务以前缀 `DustAway-HomeCleanup-` 命名。
- 钉钉两次显示正在运行，但两次后台检查未发现残留进程，也没有执行 kill。安装包错误地命名为与主程序同名的 `DingTalk.exe`，疑似触发安装器自身检测。10:36 恢复官方原名 `DingTalk_v8.2.0.260119001.exe`，哈希保持原值，用户手动安装后确认登录。证据 `dingtalk-rename-receipt.json`、`installed-metadata-3.json`。

## 已实现与正在验证

- 首页已改：实际用途与规则名、可清理但未选提示、手动折叠可选容量、分组可选/已选量、支持且运行中的项目退出后复查及焦点恢复。新增 5 项行为测试；全量 21 文件 290 项通过，前端构建通过。
- 仅 `cfg(test)` 的 `real_application_acceptance` 已加入真实扫描、运行拦截和逐规则单次清理入口。固定实际电脑名、MSN 标准资料目录、专用可执行位置和标记、普通权限；规则/目录/真实程序指纹独立限定，生产版本不包含入口。清理前后哈希与完整 Stamp 对比保护范围，执行前写入 create_new + fsync 标记，复用 Engine 冻结清单及底层文件检查。仅检查原有规则固定缓存目录，未扩大范围。
- 本次官网钉钉签名 `DingTalk Technology Co.,Ltd.` 与原登记 `DingTalk Technology Co., Ltd.` 仅空格不同。已添加仅钉钉使用的精确可信别名并真实复测识别成功，不做全局签名归一化。安装显示 8.2.0.260119001，Windows 固定文件版本 **8.2.0.119**；版本许可必须使用后者。
- computer-use 已发现既有 RDP 窗口，但捕获分别失败 `FrameArrived timed out: timed out waiting on channel`、`window capture timed out: timed out waiting on channel`。按工具恢复规则重选激活一次后仍失败，停止该 UI 自动化；没有使用 PowerShell UI Automation 替代。SSH 和普通权限任务仍正常。后续界面功能闭环未验收。
- `real-probes-1.json` 仅为第一次读取视图，Windows PowerShell 默认编码及 Get-Content 附加属性导致该视图乱码冗余；远端原始 JSON 未损坏。读取器已改 .NET UTF-8 ReadAllText，须取原始证据，不据乱码视图判定。
- 当前远端测试程序是 v4，SHA-256 `828AB34AADDADCEB35433727DDB64DC4A9C289D0F5B159B1F244CB0E73CE8ED3`。本地 `dustaway-real-app-tests.exe` 保留该二进制，不拿后续 workspace 编译产物覆盖它。原始证据已 SCP 下载至 `remote-originals/`，13 个 JSON 的长度与 SHA-256 均与远端清单一致。
- 五条规则均完成真实非零扫描及运行中阻止：钉钉 HTTP 8 / 6,168,773 B、代码 14 / 42,208 B；百度 HTTP 47 / 16,227,194 B、代码 45 / 15,039 B、GPU 5 / 557,424 B。均未因“退出后复查”自动开放。
- 用户确认从托盘退出两款后，百度仍有 `YunDetectService.exe` PID 17412，路径属于已核验安装目录。HTTP 清理在准备前拒绝，没有 started 标记或删除；不强杀、不放宽进程检查。其他两条未尝试。
- 钉钉 HTTP 单次清理成功：`real-cleanup-b873af6c-6233-427b-954d-d0364c691632.json`，实际删除 **8 个 / 6,168,773 B**，failed/skipped/unknown 均 0，**7274 个保护文件**的 SHA 与完整文件信息一致。`real-clean-dingtalk.http.started` 已存在，**绝不重放、删除标记或接续同一清单**。
- 已通过普通用户任务重新打开钉钉（`DustAway-HomeCleanup-Reopen-DingTalk-Http`，Ready / 0）。11:08 只读复查 `real-scan-175e8ce9-16dc-42a9-a8ea-f832285845c0.json` 发现 HTTP **5 个 / 5,050,736 B** 再生成。原清理回执保留当时 `not_run` 字段，后续证据单独记录，不回写原件。
- 用户先确认 HTTP 后功能正常，随后确认托盘退出；后台无钉钉进程后，于 15:35 启动代码缓存单次清理。删除 16 / 95,608 B、7365 个保护文件一致；15:38 已执行 `Run-ReopenDingTalkCode.ps1` / `Start-ReopenDingTalkCode.ps1`，**不要重放**。15:40 只读扫描仅 4 / 144 B，不能代替实际缓存内容再生成。早先将登录页误判为失败的记录保留；16:02 用户说明未勾选自动登录，保存 `dingtalk-login-expectation-correction.json` 纠正解释，不能继续引用旧失败结论。
- 当前 Rust workspace **252 通过 / 24 忽略**，`rust-workspace-home-complete.log`。新增边界逐条覆盖五条缓存的长期排除、冻结后新增、占用、根替换、目录链接、部分读取失败与全根读取失败，10 项合成边界测试通过；合成不替代真实验收。前端当前 **290 测试通过**（`frontend-home-final.log`），构建通过。证据工具 12、验收工具 6、桌面驱动工具 2 测试通过；18 条应用投影校验通过。
- 原有桌面回归两次失败已保留：折叠筛选器内选项不可见（修正测试点击顺序）；测试后端只暴露目录，首页无可用磁盘（添加 debug 专用、标记验证的目录模拟磁盘，仍不枚举真实磁盘，补身份与范围回归）。后续完整回归推进至只读应用状态刷新；旧验收文案断言已按当前折叠技术说明修正。当时的 `desktop-home-final.log` 流水线已结束，后续完整结果见最终交付。

## 待完成

1. 钉钉两项证据已分别审阅并按精确版本加入普通许可，下一步先完成 HTTP 普通首页闭环，不重复原测试清单。未开启自动登录，正常重启的登录页不是异常，不再要求登录保持对照或修改登录设置。百度需要正常退出残留后台组件，不默认获得强杀权限。
2. 逐规则边界回归已完成；继续将合成边界测试与真实效果证据分开，未清理过的百度三条不能据此开放。
3. 用普通首页完成至少一款真实闭环，再计新增清理能力。钉钉两项准确版本有源码许可，普通首页闭环未完成，百度三条只读。
4. 本地验证与同数据性能对照已完成，见下方最终交付。207 ms 与 206 ms 两次真实扫描数据不同，不作为性能对照。
5. 剩余普通首页真实操作及百度后台退出需要人工，停用自动跟进，等待用户回复。未发布安装包。

没有新增公共 IPC 或数据库迁移；普通许可只增加上述两项准确版本；卸载残留许可不变。详见 [本轮验收记录](verification-home-cleanup.md)。

## 15 时历史进展（当前状态见最终交付）

- 人工功能确认与 13 份原件重新校验后，生成 dingtalk-http-evidence-review.json，代码证据标识 home-cleanup-20261001-dingtalk-http；支持表为 35/15/20。仅源码加入许可，普通首页真实验收尚未执行，不能据此计为本轮能力交付。远端 v4 仍是旧的专用验收二进制，不能用它的 production_permissions 字段验证新普通许可。
- 新增精确版本、相邻版本/安装器显示版本拒绝和普通库伪造候选拒绝测试；核心测试与普通库 2 项集成测试通过。最新 workspace 251 / 前端 290 已通过，构建与工具 20 项通过，旧 249/289 是前一阶段结果。
- 桌面回归发现窄窗口隐藏了唯一重扫入口。已将入口放到扫描范围旁，删除结果页底部重复按钮，新增组件回归；完整原生 E2E 正在基于新前端重跑。失败记录保存在 desktop-e2e-home-flow.log。
- 新 computer-use 插件重新读取 RDP 窗口：最初最小化，按恢复步骤激活重取后依旧 FrameArrived timed out，停止 UI 自动化；没有点击或关闭远端应用。钉钉当前已由用户重新登录，百度 YunDetectService 仍在运行。
- 代码清理计划 `64383e0e-2329-488b-9b4b-4c9f25a88ea9`，原件 `real-cleanup-7b5ac2ac-a483-4407-a76d-e2e39b088e89.json`；单次标记 `real-clean-dingtalk.code.started` 已生效。`remote-code-originals/` / `remote-code-regeneration-originals/` 已核验 15 / 16 份原件。保护哈希一致不能代替登录功能通过，重登后的写入不能用于回写原周期。
- 精确许可的原生回归还发现新版本限制可能被旧扫描的“运行中”状态覆盖，导致错误显示退出提示。前端已优先采用最新版本/身份限制，新增反例；本次定向 25 项与决策 35 项、构建通过。当时原生流水线为 `desktop-home-version-check.log`，已结束；后续验证与恢复见最终交付。

## 16 时纠正与接续（下方最终交付已完成后续验证）

- 用户明确未勾选自动登录，并回答清理代码缓存后正常登录的功能及原有测试资料“正常，没有发现异常”。`dingtalk-login-expectation-correction.json` 修正旧失败解释，`dingtalk-code-human-functional-confirmation.json` 保存功能确认；不能继续把登录页算作失败。
- 16:06 只读任务 `DustAway-HomeCleanup-RealProbe-Scan-160607` 已 Ready / 0，代码再生成 15 / 68,844 B，完整读取；原件 `real-scan-1d921ab0-1afe-4055-a9dc-46ea8d32cec2.json`。`remote-post-login-originals/` 共 17 份原件哈希核验通过；`dingtalk-code-evidence-review.json` 独立审阅后才新增代码许可。没有重复删除。
- 前端当前 21 文件 / 291 通过；增加代码许可后的生成投影及构建已完成，workspace 回归进行中。`desktop-home-final-run.log` 完整清理/空间分析/手动清理流程通过，附加视觉脚本仍按旧“全部静态只读”断言导致失败，已改成核对真实“只读或当前版本受限”、锁图标和无选择框。最终新两项许可构建还须原生回归。
- 普通首页保护核对入口 `ordinary_home_protection` 只读、仅测试构建可用，保留独立 Before/After 指纹，普通程序不含入口。`Run-OrdinaryHome.ps1` 需先有保护基线才能启动，绝不自动扫描或删除。当前远端尚未传新普通程序或此检查工具，原 v4 保持不变。

## 最终本地交付与下一次人工步骤

- 最新 Rust workspace **252 通过 / 24 忽略**，`rust-workspace-home-complete.log`；包含新增只读保护检查点的身份/文件集合/顺序校验，以及普通库对两项许可的伪造文件拒绝。全量前端 291 通过；代码许可及投影变化后相关 60 项再通过。构建、20 项工具测试、18 条目录投影验证通过。
- `desktop-home-final-run/desktop-e2e-0.18.0.json` 为完整原生流程通过证据，包含两尺寸两主题、键盘/焦点、一次确认、部分结果、请求丢失恢复、手动清理及 100,002 文件空间分析。它对应增加代码许可前的 fixture 二进制；增加第二项许可后，用同一标记夹具完成独立只读能力与布局复核，见 `application-visual-two-rules-complete/application-group-layouts.json`，记录两个二进制哈希，不冒称重新跑了整套流程。
- 附加脚本失败记录保留：旧静态只读标签断言不适用于准确版本限制、前端 bootstrap 尚未完成时误读默认 runtime、同一用途分组较长时需要滚动到具体应用行。修正的是测试的就绪与导航步骤，未放宽产品能力；新脚本先用后台 bootstrap 确认 fixture，拒绝普通程序后才扫描。
- 隔离性能对照 `scan-performance-isolated/summary.json`：同一 10,000 临时文件，预热各一次、交替各五次；中位数 13,521.7688 → 13,653.0214 ms（+0.97%），候选指纹完全一致，10% 阈值通过。范围仅后端临时文件扫描，不代表真实应用或界面渲染性能。没有并行构建/测试。
- 普通 debug/custom-protocol 已恢复，SHA-256 **E971634972A2E331A4A61EDC9EA0C65339B3F4C0A3E2C42E92BC6F0FEB0C1245**；普通程序不含 fixture 或测试删除许可。本地交付目录 `artifacts/home-cleanup-real-20261001/ordinary-home-ready/`。独立核对测试程序 SHA-256 **01053012DAE3352E307EB2FEE83D857D34F4D84B002A86E55446F7927146DB70**，运行脚本固定选择只读入口；该测试二进制还包含其他测试，不是普通应用。
- 两程序、两个清单与启动/核对脚本已传远端本轮根目录，四个构建文件哈希全部匹配，`ordinary-home-transfer-verification.json`。**尚未启动普通程序、尚未取 Before/After 基线、未执行新的清理**。原 v4 和原清理标记保持不变。当前本地无本任务的 WebDriver 或测试程序运行。
- 已向用户发出一次问题：普通首页复测前是否已从托盘退出钉钉；仍待回复，不重复提问。确认退出后，先在本地执行 `Start-OrdinaryProtection.ps1 -Phase Before`，检查任务完成及原始基线；然后通过 SSH wrapper 执行 `Start-OrdinaryHome.ps1`。普通界面由用户扫描并**只选择钉钉网页缓存，取消其他项目**，一次确认。结果返回后，钉钉仍关闭时执行 `-Phase After`，核对通过再正常打开钉钉并验证功能/再生成。用户未勾自动登录，登录页正常，不要求修改设置。
- 本轮还未完成普通首页真实闭环，不能声称五条全部开放或本轮能力交付已完成。百度三条仍被后台组件或未完成验收限制，不能强杀后台进程或借用卸载残留权限。

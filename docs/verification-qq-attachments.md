# QQ 附件整理：实现与待验收范围

当前兼容性策略已调整：软件版本仅作记录，不作为清理白名单；已开放规则按实际身份、布局、组件和格式核验。扫描后更新仍会使旧清单失效。详见 [应用清理兼容性策略](cleanup-compatibility.md)。以下实机数据与旧包校验值保留为历史证据，不代表所有版本均已实测。

本轮扩展现有聊天附件整理，分为经典 QQ 与 QQNT 两个独立来源。**两系列已分别完成指定接收文件的实机删除及用户重新登录确认：经典 QQ 9.7.25.29417 仅开放 `FileRecv/MobileFile`，QQNT 9.9.33.52230 仅开放 `nt_qq/nt_data/File` 直属 JPG、MP4、ZIP 独立文件。** 其他目录及格式只读。微信仍仅开放先前验证过的布局和格式，见 [微信验收](verification-attachments.md)。

## 已实现

- 默认选择微信；新增经典 QQ／QQNT。用户选择账号位置后主动扫描，普通垃圾扫描不遍历聊天媒体。
- 修改时间 30／90／180 天及全部、类型和文件名筛选、大小降序、100 项分页、本页选择、跨筛选保留已选、图片与视频本地缩略图、文件定位。
- 切换应用或账号关闭旧会话、作废清理计划、清除选择与预览；扫描、准备和执行期间禁止切换。页面和位置发现的迟到响应丢弃，后台缩略图通过会话代数阻止旧结果重新进入缓存。
- 明确显示未安装、安装未核验、未发现账号、资料布局不支持和扫描不完整；未知格式保留并说明原因。
- 确认页展示应用来源、账号位置、准确数量与容量；一次确认永久删除。取消、结果与磁盘空间变化复用既有流程。

## 来源与接口

`AttachmentProvider` 为 `weixin`、`qq_classic`、`qqnt`。`AttachmentLocation` 增加后端核验的 `provider`。`get_attachment_locations(provider)` 返回位置、状态和读取问题；`pick_attachment_location(provider)` 使用系统目录选择器并执行对应验证。

`AttachmentSelection` 仍只接受会话 ID 与候选 ID。真实路径和身份只在后端保存，来源选择不授予任意目录删除许可。经典 QQ 与 QQNT 不能交换账号位置、候选 ID 或计划。历史保留 `weixin.attachments`，新增 `qq.attachments`、`qqnt.attachments`，兼容旧记录。

## 当前识别边界

| 来源 | 识别约束 | 附件范围 | 删除许可 |
| --- | --- | --- | --- |
| 经典 QQ | 腾讯签名、完整文件版本与组件一致、`Bin/QQ.exe`、同版本签名 `TXPlatform.exe` 与 `KernelUtil.dll`、当前用户所属数字账号、`Tencent Files`／`TencentFiles` 父目录，`Registry2.0.db`、`FileInfo.db` 文件身份 | `Image`、`Video`、`Audio`、`FileRecv` | `qq-classic-account-v1` 布局、`FileRecv/MobileFile` 直属 JPG／MP4／ZIP 文件 |
| QQNT | 腾讯签名启动器、匹配版本的 `versions/config.json` 与 `resources/app/package.json`、腾讯签名 `QQNT.dll`；核对 `nt_qq/nt_db` 目录以及 `nt_msg.db`、`profile_info.db` 文件身份 | `nt_qq/nt_data/Pic`、`Video`、`Audio`、`File` | `qqnt-account-v1` 布局、`nt_qq/nt_data/File` 直属 JPG／MP4／ZIP 文件 |

上述 QQNT 安装组件来自签名有效的 9.9.33.52230 官方安装包观察，账号标记与指定接收文件已在真实登录账号验收；匹配失败保持不可扫描。自定义位置必须满足相同结构和归属，完全重命名父目录不开放。不会读取聊天数据库正文，也不解密附件。备份、迁移、收藏、QQGuild、数据库、索引和配置目录不作为附件删除范围。来源发现只将具有对应系列标记的数字目录作为候选；另一系列的资料不会误报成损坏账号，部分匹配布局仍提示核验失败。

扫描、准备与执行使用固定文件身份和父目录链，应用重新安装、文件变化、排除项、链接、云文件、占用或状态不明均保留。扫描读取不完整的会话不能准备清理。接收文件形式的图片和视频验收不扩展到 `Image`、`Video`、`Audio`，也不扩展其他 `FileRecv` 子目录或嵌套目录。

## 自动验证

- 全量前端 347 项通过；追加确认页切换场景后，附件专项 10 项通过。
- 后端全量 337 项通过、31 项显式忽略；追加 QQ 边界及旧缩略图会话测试后，附件专项 11 项通过（5 项核心会话、6 项 Windows 附件边界），2 项旧微信实机测试显式跳过。
- QQNT 启动器与组件版本绑定测试通过；另对官方安装包解出的启动器、配置、应用标记与 DLL 运行一次只读 Rust 签名及固定身份验证，通过，未启动 QQNT。
- QQNT 账号标记及同名后台组件适配后，Windows 库全量 143 项通过、34 项实机／交互测试保持显式忽略；附件前端专项 10 项通过，前端构建通过。忽略项不计为实机验收通过。
- 实机完成后，精确开放 QQNT 版本与目录的专项检查 8 项通过、5 项实机测试显式跳过；其中新增检查覆盖其他完整版本、其他系列、Thumb／嵌套目录、语音、普通文本、数据库及未知格式不继承许可。
- Clippy `--workspace --all-targets --locked -- -D warnings` 通过。
- 原生界面使用 205 项纯内存夹具：100 项分页、预览不选择、跨页选择、隐藏选择、取消确认、三个来源切换、浅／深色、1100×780 和 760×580。夹具禁止删除任何真实文件。
- 加入 QQNT 扫描范围提示后，上述原生窗口检查再次通过；窄窗口的已选汇总和执行入口仍可见。最新截图与 `result.json` 位于 `artifacts/qq-attachments-ui/`。
- 最终生产规则在测试机以普通用户再次只读扫描：QQNT 账号识别就绪、扫描无读取错误；保留的 WAV／TXT、Thumb 图片与未知文件共 7 项全部保持只读，缩略图可生成，未再次准备或执行删除。证据为 `nt-released-production-probes.log`。
- 规则目录投影（18 条应用规则）、用途识别矩阵（256 条规则／510 个模式）及 20 项工具测试通过。
- 日志在 `artifacts/qq-acceptance/`，截图在 `artifacts/qq-attachments-ui/`。安装器、真实账号路径和合成样本不会随 DustAway 测试包分发。

## 测试机与真实验收

测试机 DESKTOP-24NJTT6／MSN，沿用严格 SSH 指纹校验。没有改变防火墙、网络、UAC 或 ACL。

已从 [QQ 官网](https://im.qq.com/pcqq/index.shtml) 配置取得经典 QQ 9.7.25.29417 官方安装包，签名有效。SHA-256：`804A72F08E692A6317ABB31445990E4DDC66DBC163967B5D0C4DB360B250AE05`。用户已完成安装与登录，普通交互用户探针识别到账号与预期数据库标记；扫描无读取错误。客户端接收的标记样本位于 `FileRecv/MobileFile`。

该安装的 `TXPlatform.exe`、`KernelUtil.dll` 腾讯签名及完整版本与启动器一致；附件识别、准备、执行均绑定两者的文件身份。此核验只用于附件能力，不扩展普通 QQ 缓存规则。发现频道组件 `QQGuild.exe` 位于 QQ 安装目录之外，附件退出检查已补充该组件。

2026-10-03 普通交互用户实机检查通过：接收的 JPG 与 MP4 可生成本地缩略图；QQ、TXPlatform、QQGuild 运行期间，附件准备返回 `application_running`，没有删除。实机测试入口仅存在于测试构建，绑定专用机器、账号、固定样本路径与 SHA-256；每次删除有独立、不可重放的标记。Windows 库测试 138 项通过、31 项显式忽略；追加的删除句柄兼容、磁盘统计与许可边界专项通过。证据在 `artifacts/qq-acceptance/classic-*.log`。

正常退出后，第一轮仅删除用户通过 QQ 接收的 JPG、MP4、ZIP 三个合成样本，共 4,375 字节，失败／跳过／未知均为 0；腾讯资料目录内其余 366 个文件 SHA-256 前后一致，重放同一计划被拒绝。用户已重新登录确认聊天记录、保留 WAV 和说明文件正常。

第一轮发现 QQ 附件快照将公共文件字段放在了内层，导致磁盘统计无法识别目标卷，原始回执如实记录 `volume_unavailable`，没有伪造净释放量。已改为与微信一致的公共快照字段结构，并为经典 QQ、QQNT 各自覆盖卷识别与可用空间采样。修复后通过 QQ 新接收一份独立 ZIP，再执行一次单独验收：删除 1 个文件、233 字节，C 盘执行前后均为 25,293,561,856 字节，净变化为 0；其余 366 个文件内容一致。两个批次共删除 4 个明确标记样本，不重置旧标记、不重放旧计划。证据为 `classic-acceptance.json`、`classic-space-remote.log` 和 `classic-human-confirmation.json`。

QQNT 官网当前 9.9.36 下载返回 403，另取得 [winget 9.9.33.52230 清单](https://github.com/microsoft/winget-pkgs/blob/master/manifests/t/Tencent/QQ/NT/9.9.33.52230/Tencent.QQ.NT.installer.yaml) 中的腾讯官方安装包，签名有效，SHA-256 与清单一致：`B25C0D3CE9DF764074A9118D0DED927E1B2D7EBF60E306112E8DF18A040EC492`。已安装到独立目录并由用户登录另一个测试账号。安装器仍移除了经典 QQ 程序；旧经典账号 156 个文件的内容核对无变化。QQNT 第一批文件默认下载至通用 Downloads，保持在附件删除范围之外。用户随后通过客户端重新接收 5 个标记样本至账号 `nt_qq/nt_data/File`，各文件哈希与合成样本一致。QQNT 页面已明确提示账号外的 Downloads 等位置不在本轮扫描范围。

QQNT 图片与视频本地预览通过，运行时准备操作返回 `process_unknown`，未删除文件。用户正常退出 QQNT 后，准备最初仍被同一原因拦截。普通用户探针与 SSH 只读诊断对照定位到网易 GameViewer 的 3 个 `crashpad_handler.exe`：普通用户无法读取它们的会话、镜像或令牌，另有可读取的 GameViewer 和微信同名进程。NT 安装中确实包含此共享组件名称，不能简单从退出检查中删除。

已将 QQNT 的 `crashpad_handler.exe` 与 `crashpad_handler_extension.exe` 按镜像路径归属核验：可读取且明确属于其他安装目录时不误报为 QQNT；属于 QQNT 安装目录时拦截；路径不可读取时仍以 `process_unknown` 保留，并展示名称及 PID。没有基于管理员诊断结果为普通用户添加进程白名单，没有提升附件清理权限或修改安全设置。用户退出 GameViewer 后服务已停止，但仍有两组 SYSTEM 健康守护及其子进程；用户单独授权处理这 4 个残留。处理期间原生终止调用曾返回失败，随后独立核对确认它们全部退出，未盲目重试。普通用户再次检查得到空 QQNT 进程清单、无未知状态；仍运行的微信同名组件被正确区分。证据：`process-probe-after-fix.log`、`gameviewer-residual-after-attempt.log`、`process-probe-before-delete.log`；跨安装同名、相近路径边界和未知归属保持拦截的自动检查通过。

2026-10-03 08:55（北京时间）普通交互用户单次删除验收通过：仅删除账号 `nt_qq/nt_data/File` 直属 JPG、MP4、ZIP 三个指定样本，共 4,375 字节，失败／跳过／未知均为 0，清理来源为 `qqnt.attachments`。腾讯资料目录其余 3,046 个文件 SHA-256 前后一致，Downloads 原件内容一致，目录未删除，同一计划重放被拒绝。C 盘可用空间从 23,887,724,544 变为 23,887,732,736 字节，实测净变化 +8,192 字节，与逻辑删除字节分别记录。`nt-delete.started` 已生成，禁止重置或再次执行该测试。用户已在 QQNT 重新打开后确认登录、聊天记录及保留 WAV／TXT 使用正常。新进程探针确认修复后的退出检查识别到真实 QQ.exe 和 QQNT 自己的崩溃收集组件，未误计同时运行的微信组件。原始证据为 `nt-acceptance.json`、`nt-delete-report.json`、两份保护文件清单、`nt-human-confirmation.json` 和 `process-probe-after-qqnt-reopen.log`。

已生成明确标记的 JPG、MP4、WAV、ZIP 测试样本。QQ 语音消息需通过真实客户端另行验证，WAV 文件不能替代语音消息验收。

QQNT 已独立完成专用账号登录与目录观察、客户端接收、预览和运行中拦截、退出核验、冻结指定样本并单次删除、完整性核对及重新登录人工确认。此前微信删除授权不用于 QQ 既有资料；不复用、删除或重置微信或 QQ 已完成批次的单次删除标记。

未完成验证的版本、目录和格式不宣称支持删除，包括 QQNT 的 Pic、Video、Audio、File/Thumb 等目录及 QQ 语音消息。两系列分别交付和验收，不能共用验收结论。QQ 缓存、重复文件删除及系统空间管理均不扩展。

## 先前只读测试包（历史记录）

版本仍为 0.18.0。交付 `qq-classic-readonly-test` 与 `qqnt-readonly-test` 两个独立目录及 ZIP；共用同一构建，分别用于两个系列的浏览验收，均未开放 QQ 删除。默认应用仍是微信，由用户主动切换系列。

Release EXE 大小 19,502,592 字节，SHA-256：`4208CBB76CBA3A5529AF6ED5C76E49A79FBB2158D787D6E67320A82AFC1E7A77`。正式程序启动检查通过：默认微信、账号未选、未自动扫描、QQ 两来源的全部发现位置均只读、旧清理回执保持不变。证据在 `artifacts/qq-attachments-release-ui/`。

包内 `SHA256SUMS.txt` 校验每个载荷文件；本次测试包不含安装器、测试账号资料、聊天附件或测试探针。

## 经典 QQ 第一批验收测试包

批次 `qq-classic-test`，版本 0.18.0。Release EXE 19,505,664 字节，SHA-256：`1B465AED49D219E920F14015E8564BF8396B722542690A2C9358DF2840CD87AC`。默认仍为微信，不自动选择账号或开始扫描；正式程序启动、账号未选、QQNT 全部位置只读、原有清理回执不变的检查通过，截图在 `artifacts/qq-classic-release-ui/`。

经典 QQ 删除支持范围仅为上表列明的版本、目录和格式。本批包不包含后来验收的 QQNT 删除许可；QQNT 使用后续独立测试包，不以本批经典 QQ 结论开放。

## QQNT 第二批验收测试包

批次 `qqnt-test`，版本 0.18.0。Release EXE 19,550,720 字节，SHA-256：`C881B57AF33361BF023D0946879A0B3F3E91F7E44BA0090F5603C8E4C3F2BF68`。本批包含上述精确 QQNT 删除许可及同名后台组件归属修复，保留原经典 QQ 和微信验收范围。

生产启动检查通过：默认微信、未选择账号、不自动扫描或勾选，既有清理回执保持不变；截图和结果在 `artifacts/qqnt-release-ui/`。最终 Clippy 全工作区检查通过。包内含支持范围、验收说明与逐文件 `SHA256SUMS.txt`，不含安装器、账号资料、完整附件清单或测试探针。ZIP 校验信息另存 `artifacts/qq-acceptance/nt-package.json`。

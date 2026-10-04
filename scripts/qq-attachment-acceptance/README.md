# QQ 附件实机验收接续

经典 QQ 9.7.25.29417 的 `FileRecv/MobileFile`、QQNT 9.9.33.52230 的 `nt_qq/nt_data/File` 直属 JPG、MP4、ZIP 已分别完成实机删除与用户重启确认；其他格式、目录及版本继续只读。不要修改微信或已完成 QQ 批次的验收标记，不要复跑旧删除测试。

1. DESKTOP-24NJTT6 上分别安装经典 QQ 与 QQNT，使用专用测试账号，不导入原有资料。用户负责登录。安装包与合成样本已放在 `%LOCALAPPDATA%\DustAwayAcceptance\20261002-qq`，安装包来源与哈希见 `docs/verification-qq-attachments.md`。
2. 构建 `cargo build -p dustaway-windows --example attachments_probe --locked`，经严格 SSH 校验传到上述目录；同时部署 `Probe.ps1` 与记录该 EXE SHA-256 的 `probe-binary.json`。
3. 用 MSN 普通交互用户运行 `Probe.ps1 -Provider qq_classic` 或 `qqnt`。探针只发现、扫描与预览，不准备或执行删除。记录布局与无法识别的原因，按实际观察修正对应适配器。账号路径与原始输出仅留开发验收目录，不打入软件包。
4. 客户端实际接收明确标记的测试附件后，分别准备运行拦截和单次删除验收。WAV 接收文件不能替代语音消息；加密语音保持只读。只有确认独立文件、版本、目录和格式后，才添加受专用机器、账号、样本哈希和一次性标记限制的 `cfg(test)` 删除入口。
5. 核对保护文件与磁盘采样；用户重新登录确认聊天记录及未删附件正常后，补充该系列的证据与精确生产许可。未经实际验收的格式继续只读，不把安装包组件验证写成聊天附件删除验收。

经典 QQ 已完成 `classic_delete_once` 与 `classic_space_delete_once`，分别保留 `classic-delete.started` 和 `classic-space-delete.started`；禁止复跑或重置。第一轮的磁盘统计失败如实留存，第二轮以新接收的独立 ZIP 核验修复：删除 233 字节，C 盘净变化 0，366 个保留文件内容一致。用户已确认第一轮后的登录、聊天与保留附件正常。原始证据在 `artifacts/qq-acceptance/`。

`Classic.ps1` 只允许明确测试名，核验测试二进制哈希并分别收集标准输出和标准错误，避免 PowerShell 5 将原生错误输出提前转换成终止异常。Rust 的 UTF-8 JSON 在 PowerShell 中必须以 `Get-Content -Encoding UTF8` 读取。

需要人工操作或新版布局时，保留明确的待验收状态，禁止扩大通用手动删除对聊天目录的权限。

QQNT 已完成账号内 3 个标记样本的单次删除：4,375 字节，3,046 个保护文件内容一致，C 盘净变化 +8,192 字节，Downloads 原件完整。`nt-delete.started` 已生成，禁止重置或再次执行 `Nt.ps1 -Phase DeleteOnce`。此前 GameViewer 不可读取的同名进程曾导致准备失败；用户单独授权处理残留后，普通用户退出检查通过才执行了删除。用户已在 QQNT 重新打开后确认登录、聊天记录及保留 WAV／TXT 的实际使用正常，生产许可按上述精确范围开放。

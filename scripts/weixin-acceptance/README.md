# 微信专项 SSH 验收（0.21.1）

本轮只做真实数据的只读检查和隔离夹具删除。不启动微信、浏览器或远程桌面，不修改网络、防火墙、UAC 或执行策略。真实网页恢复始终为 `not_verified`，本工具不能写入发行许可。

## 构建与运行

1. `cargo test -p dustaway-core -p dustaway-windows --lib --no-run --locked`，从编译输出取得两个测试二进制；`cargo build -p dustaway-windows --example weixin_special_probe --locked`。
2. `node scripts/weixin-acceptance/run.mjs run <唯一运行ID> <Windows测试exe> <Core测试exe> <probe.exe>`。
3. 断连后只用 `resume <运行ID>`；它查询已有任务并收集证据，绝不重新发起。`status <运行ID>` 只查状态，`cancel <运行ID>` 写入取消请求。
4. `node scripts/weixin-acceptance/bundle.mjs artifacts/weixin-0211/<运行ID>` 离线复核证据包。

运行目录为测试机 MSN LocalAppData 下 `DustAwayAcceptance/weixin-0211/<运行ID>`，本机证据保存在 `artifacts/weixin-0211/<运行ID>`。运行 ID 不可复用；二进制和 worker 哈希写入 request，重复或已开始的运行只能查询。现有 0.20/0.21 目录保留。

SSH 沿用已核验的专用密钥、Tailscale 地址及 ED25519 主机指纹。传输脚本上传到独立临时文件，核对摘要后作为命令执行，再移除；不更改 PowerShell 执行策略。MSN 没有可用普通用户会话时记录 blocked，不用管理员身份代替。受限任务不进行界面操作。

## 阶段与结果

- preflight：生产 WindowsPlatform + Engine 扫描微信规则，生产附件适配器汇总所有识别账号；输出候选、保留、读取失败、未知容量和分类，不输出聊天文件名、账号名或内容。
- code / gpu：验证正式 Engine 拒绝未获许可的删除请求。两项未开放时必须是 blocked，不能把拒绝当成真实删除成功。
- fixtures / boundary-*：独立夹具分别验证单文件、部分清理、取消、安装身份、文件变化、链接、锁定、排除、拒绝访问和旧计划。保护摘要包括未选缓存。
- attachments：万条附件、分页、选择与删除结果核对。

每一阶段最多 120 秒；到期请求取消，再等 30 秒。未能确认停止记录 uncertain，终止本次创建的测试进程，绝不按进程名终止微信等应用。阶段失败后不启动后续阶段。临时任务只有结束且 XML 摘要与创建记录一致才移除，证据保留。

manifest schema 2 绑定 run ID、输入二进制、worker、request、状态、意图与阶段日志。校验文件实际字节数、SHA256、目录边界、阶段顺序、退出码、非零测试数、保护摘要与回执。`noninteractivePassed=true` 仅表示非交互测试通过，不能推出网页功能恢复。旧 schema 1 的 `validate.mjs` 仅检查历史元数据，返回 `complete=false`，不会升级旧证据。

下载先写入独立 `.part-*` 文件，通过哈希校验后才成为证据附件。断连留下的部分文件保留，恢复时只补下载；已有正式附件不一致则停止，不覆盖。启动失败记录 `process_start_failed`，测试进程非零退出记录 `test_process_nonzero_exit`，两者都不能作为通过。

`Test-Remote.ps1` 使用全新标记目录验证重复运行、二进制替换和外来任务保护。测试产生的临时任务在归属确认后移除。它不执行伪造的二进制。

## 交付

0.21.1 安装版使用原受限提权链路，免安装版不包含辅助组件。升级验收针对已核验的 0.21.0 安装，原始包与证据保留。网页首次/再次加载、真实缓存功能恢复、760×580、浅深色和原生键盘测试未执行，微信代码/图形缓存继续只读。

旧 Prepare/Start-Checks/Finish-Checks/Copy-Remote 脚本仅保留用于查看 0.21 历史工作流；本轮统一使用 run.mjs。

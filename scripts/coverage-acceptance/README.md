# 应用覆盖 SSH 验收（0.23.1）

仅使用生产扫描器读取八项固定范围；不启动应用、不操作桌面、不清理真实缓存。安装登记仅为线索。日志与缓存分别统计，所有新增占用保留，可清理增量必须为零。

1. 编译 `cargo test -p dustaway-core -p dustaway-windows --lib --no-run --locked` 和 `cargo build -p dustaway-windows --example coverage_probe --locked`。
2. 使用编译输出的路径运行 `node scripts/coverage-acceptance/run.mjs run <唯一ID> <windows-tests.exe> <core-tests.exe> <coverage_probe.exe>`。
3. 用 `status <ID>` 查询、`resume <ID>` 接续查询与下载、`cancel <ID>` 请求取消；不重放已派发任务。
4. `node scripts/coverage-acceptance/bundle.mjs artifacts/coverage-024/<ID>` 校验 schema 3 的实际证据文件；不接受旧微信包作为本轮覆盖通过依据。

测试机固定为已授权的 DESKTOP-24NJTT6，沿用严格 SSH 主机指纹。MSN 普通用户会话缺失时阻塞，不改用管理员测试。五阶段依次为只读预检、二十六规则（含十八项历史回归）边界、空/缺失/拒绝/取消/排除状态、硬链接/目录联接、Core 回归。每阶段 120 秒，取消等待 30 秒；不确定结果不自动重试。仅在任务结束且 XML 摘要匹配后移除任务，证据保留。

真实输出不包含私人文件名、路径或内容；夹具日志只能包含本次自建目录。下载复用 0.21.1 已验证的临时文件与哈希检查。schema 3 保留原始请求和每阶段意图，绑定二进制 SHA256；成功不代表真实应用恢复，也不改变清理许可。

开发机只读对照：设置 `DUSTAWAY_COVERAGE_RUN` 为本次唯一 ID 后执行同一 `coverage_probe`。本机结果与测试机结果分别记录，不能互相替代。

证据读取器保留 0.22.0 八项历史批次；当前派发仅支持 0.24.0 固定八项。版本、规则集合或候选批次不符即拒绝，不能用旧证据作为新批次通过。

0.23.1 沿用十项固定范围，只输出本次占用与零识别增量，不把旧范围宣称为新增识别。0.22.0、0.23.0 历史证据均保持可校验。

0.24.0 固定批次仅统计本轮新增八项，不能混入以前十八项；新增识别与已保留量相同，可清理增量为零。0.23.1 继续使用原来的十项及零识别增量口径；0.22.0、0.23.0、0.23.1 均保留独立校验。

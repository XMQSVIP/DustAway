# 钉钉与 ToDesk 历史日志清理准备（2026-10-25）

基线提交：`15b93d73e24a1c57f55281daff9039b51f4d65e3`。版本保持 **0.24.0**；本轮交付源码、隔离测试和本文，不制作、安装或部署发行包。

## 能力状态

本轮实现待验收的日志适配器，通过本地自建夹具验证生产 Engine 的选择、计划、逐文件删除与回执流程。正式构建没有启用入口，两条规则仍为 `readonly_observed`，默认不选，准备和底层删除均拒绝。用户规则仍为 **141 条：42 条逐文件清理、4 条官方工具、95 条只读**。

没有修改规则 JSON、编译期清理证据、前端目录投影、公开 IPC 或历史数据库。生产扫描仍按 0.24.0 的完整固定目录统计占用，包括下面不属于准备删除范围的文件；不缩减只读统计，不增加可清理容量。

| 规则 | 准备清理的范围 | 未开放事项 |
|---|---|---|
| `appcache.dingtalk_logs` | 当前用户 RoamingAppData 下 `DingTalk/log`、`DingTalk/holmeslogs` 直属 `.log` / `.xlog` | 文件布局为待实机确认的设计；`recordLog`、`updaterlogs` 与所有子目录继续保留 |
| `appcache.todesk_logs` | 当前用户 LocalAppData 下 `ToDesk/Logs` 直属 `.log` / `.xlog` | `cache` 等子目录、其他格式、安装目录及系统服务日志继续保留 |

扩展名按 Windows 常用方式忽略大小写。不读取真实日志正文、不以文件名日期推断年龄；按最后修改时间比较 `modified <= now - 7 * 86400`，恰满连续 7 天可进入隔离测试候选。近期和未来时间文件保留。年龄和固定范围在扫描、计划核验、最终删除阶段再次检查。

日志是诊断资料，删除会失去过去的排障记录。[ToDesk 官方日志说明](https://docs.todesk.com/faq/73.html)说明其日志用于故障排查，没有为本项目的删除行为提供验收许可。本机元数据观察也不等于产品布局、删除及恢复功能已验证。

## 实现与边界

- 日志策略与 Chromium 代码缓存适配独立，复用既有文件身份快照、固定 NTFS 检查、父链核验、排除项和句柄删除机制；不递归删除文件夹。
- 钉钉复用已有可信安装识别。ToDesk 从安装登记与固定安装位置寻找 `ToDesk.exe`，要求有效的离线发布者签名 `Hainan YouQu Technology Co., Ltd`、一致的 `ProductName=ToDesk` 及可读取的版本资源；零个或多个不同的有效程序身份拒绝。版本仅参与安装指纹，不作为版本白名单。
- 已知主程序跨会话检查；同安装目录后台组件和 ToDesk 的共享辅助程序按归属核验。运行、身份无法确认、状态查询失败均拒绝。程序不结束应用或服务，也不申请管理员权限。
- 测试许可仅编译于 `cfg(test)`，绑定一个规则、测试进程自行创建的临时目录身份、固定子目录与标记文件快照。目录、标记、权限域或描述符变化即失效；其他测试身份、前端输入、环境变量和发行 feature 不能开启该许可。
- 测试中用合成安装指纹和进程状态替代真实应用；Windows 签名、产品资源与真实后台组件的产品兼容性仍须后续实机验收。夹具成功不写入任何正式许可或 `real_acceptance=passed`。

## 本地验证

环境：Windows x64、中等完整性令牌（`S-1-16-8192`），Rust/Cargo 1.97.1、Node.js 24.12.0、pnpm 10.30.3；仅操作自动生成的本地 NTFS 夹具。原始输出放在 `artifacts/historical-logs-20261025/`，不覆盖之前的验收资料。

下列命令为本轮检查入口；工作区因历史环境与过期断言问题分阶段完成，原始记录与补测结果见下文：

```powershell
cargo test -p dustaway-windows --lib historical_log --locked -- --nocapture
$testTarget = (Get-Item target).Target # 本机 target 是目录联接，使用真实路径运行测试程序
cargo test --workspace --locked --target-dir $testTarget
cargo test -p dustaway-windows --locked --target-dir $testTarget --test cleanup_release --test filesystem --test manual_cleanup
cargo test --workspace --doc --locked --target-dir $testTarget
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked --target-dir $testTarget -- -D warnings
pnpm build
pnpm test
pnpm check:applications
pnpm check:recognition
git diff --check
```

最终结果（重复补测不重复计数）：

| 检查 | 结果 | 证据 |
|---|---|---|
| 新增日志专项 | 12 项通过，无忽略；两款应用各实际删除 1 个夹具文件，27 / 17 个保留文件摘要不变 | `log-tests-complete.log` |
| Rust 工作区分批回归 | 384 项通过；42 项原有专用环境/性能测试显式忽略；包含 3 项最终发行边界测试 | `workspace-tests-canonical.log`、`remaining-integration-tests.log`、`doc-tests.log` |
| 前端回归 | 32 个文件、384 项通过 | `frontend-tests-single-worker.log` |
| TypeScript / Vite | 通过；既有大块提示保留 | `frontend-build.log` |
| Clippy 全工作区、所有目标 | `-D warnings` 通过 | `clippy-final.log` |
| 编译规则投影 | 21 条应用接口规则及前端目录、141 条能力摘要一致 | `application-projection-check.log` |
| 用途库 | 259 条规则、514 个模式一致 | `recognition-check.log` |
| Rust 格式、差异空白检查 | 通过 | 本轮命令退出码 0 |

42 项忽略由 Core 的 2 项性能验收、Windows 库原有 39 项专门环境验收和独立查询性能测试 1 项组成，本轮没有新增忽略项。普通构建拒绝测试链接不包含合成许可的库，未扫描真实应用数据。最终修改的 Rust 源文件摘要与环境信息分别保存在 `source-sha256.json`、`environment.json`。

夹具覆盖：明确选择与单文件删除、相邻资料 SHA-256 保持一致、冻结清单不加入新增文件、7 天边界、未来时间、未知格式与子目录、安装变化及运行状态变化、文件更新与同路径替换、长期排除、占用、硬链接、云文件标志、父目录替换与目录联接、取消后的剩余文件、失败回执、重启恢复和旧计划不可重放。未启用许可的隔离生产扫描仍统计原有只读目录，计划及直接删除都拒绝。

开发中曾修复测试错误：故障注入最初使用了错误的 `DeleteFailure` 参数类型；第一轮可运行测试中，排除项夹具遗漏有效主题设置，导致保存设置被拒绝。这些失败不作为通过证据，最终结果以修复后的日志为准。

前端首次使用两个工作线程运行时，29 个测试文件通过，1 个分页测试达到 15 秒超时，另外 2 个文件的工作线程启动超时。恢复仓库原有的单工作线程配置后，32 个文件、384 项测试全部通过；没有修改测试超时或前端源码。TypeScript/Vite 构建通过，保留既有的单块超过 500 kB 提示。两次前端原始日志分别保留。

首次 Rust 工作区回归中，Windows 库为 173 项通过、2 项失败、39 项忽略。两个失败分别是已有 Maintenance 与 uv 测试打开 `current_exe()` 时，因本机 `target` 目录联接触发“链接、多硬链接或云文件”拒绝；不涉及新增日志断言。从缓存真实路径运行同样的两个测试均通过。保留原始失败日志，完整回归改用 `--target-dir` 指定真实构建目录；没有修改链接保护，也没有修改这两个旧测试。

真实路径下的工作区回归中，Windows 库 175 项全部通过、39 项按原设置忽略；随后发行集成测试暴露其遗留的 `112` 条规则断言。已按当前编译目录改为 `141`，保留 `42` 条逐文件许可断言，并增加链接普通库（不含 `cfg(test)` 许可）的两条日志拒绝测试。该修正不改变任何发行规则。Clippy 首次还指出测试代码中一次可避免的字符串复制，已改用借用切片；未添加 lint 豁免。

## 后续真实验收

本轮没有清理真实钉钉或 ToDesk 数据，没有连接 SSH、启动目标应用或验证应用重启。钉钉的具体文件布局、ToDesk 的真实安装/组件兼容性、实际非零清理、保护资料核对与正常使用均未验收。

后续需要在明确授权的测试环境中分别取得上述证据，再审阅固定规则范围并以独立源码变更开放；不能只修改 `cleanable`、复用隔离许可或依据本记录宣称正式版可清理。

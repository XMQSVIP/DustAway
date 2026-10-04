# 验收与测试记录

基准环境：2026-09-26，Windows 10 Pro 22H2（19045），x64、非提权进程，Rust 1.97.1 MSVC，Node.js 24，WebView2 153.0.4234.48。以下区分自动测试、真实桌面检查和仍待执行的发布验收。

本机已通过 `cargo test --workspace --locked`（29 项）、`cargo clippy --workspace --all-targets -- -D warnings`、前端构建和 6 项 Vitest 测试。真实桌面夹具流程已跑通。自动测试结果不能替代下表的真实系统发布验收。

NSIS x64 安装包已构建，体积 4,114,862 字节，Authenticode 状态为 NotSigned。最终 release 可执行文件另经真实 WebDriver 启动，确认普通后端、系统项默认不选，并完成本机只读扫描；没有触发清理。校验值在 `target/release/bundle/nsis/SHA256SUMS.txt`，构建元数据在 `artifacts/release-manifest.json`，发行版截图在 `artifacts/release-readonly.png`。

## 自动测试

| 层    | 命令                             | 已覆盖                                                                                                                                                           |
| ----- | -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 核心  | `cargo test -p dustaway-core`    | 规则分域、只读扫描、单次计划、过期 ID、排除、设置失效、浏览器重启、新文件不扩入计划、取消、持久摘要、异常意图恢复、错误分类、10 万候选分页、随机无效会话属性测试 |
| Win32 | `cargo test -p dustaway-windows` | 真实临时夹具句柄删除、修改/替换文件与父目录、原有/新增硬链接、占用、排除、年龄、越界/UNC/ADS、Junction、长中文路径；Chromium/Firefox 多配置与异常布局            |
| React | `pnpm test`                      | 不自动扫描、默认不选系统、独立永久删除确认、UAC 拒绝解释、系统断连后用户扫描可用、历史入口、大整数大小                                                           |
| 桌面  | `pnpm test:e2e`                  | 真实 Tauri/WebView2/IPC：扫描、文件明细、准备计划、返回检查、再次确认、夹具删除、保护文件保留和历史                                                              |

Rust 文件系统测试只在 tempfile 专用目录运行。端到端测试使用 `debug_assertions && e2e-fixture` 双重门控后端，要求临时目录前缀 `dustaway-e2e-` 和匹配的专用标记文件；正式 release 无法加载该后端。脚本在扫描前检查后端返回的构建标识，普通构建会被拒绝。测试保留自己的夹具目录供复查。**不要把用户目录替换成测试根。**

E2E 准备：

```powershell
cargo install tauri-driver --locked --version 2.0.6
# 从 Microsoft 下载与 WebView2 匹配的 EdgeDriver，放在以下位置：
# artifacts/webdriver/msedgedriver.exe
pnpm test:e2e
```

[Tauri WebDriver 文档](https://v2.tauri.app/develop/tests/webdriver/) · [Microsoft Edge WebDriver](https://developer.microsoft.com/microsoft-edge/tools/webdriver/)

E2E 输出 `artifacts/desktop-e2e.png`，以及包含主进程和 WebView2 子进程的 `artifacts/resource-snapshot.json`。后者是调试构建的一次工作集/私有内存/累计 CPU 快照，不能当成发行版的稳态占用或性能保证。

本机一次夹具清理后的快照：主程序 + 6 个 WebView2 子进程，工作集简单合计 395.3 MiB（共享页可能重复计算），私有内存合计 207.9 MiB。仅处理一个测试文件，该值不代表大目录扫描的内存上限。

本机另执行过 `cargo run -p dustaway-windows --example scan_smoke`，只读检查实际标准用户临时目录与浏览器配置；没有执行真实缓存删除。该示例仅输出分类计数/大小/阻止状态，不打印候选路径。

## 界面检查

已检查 1100×780 的浅色/深色前端预览及真实桌面历史页；浏览器模拟 760×580、deviceScaleFactor=2 时没有横向溢出，操作栏位于可见区域（截图 `artifacts/preview-760-dpr2.png`）。这不能代替 Windows 真实 DPI 验收。界面采用系统字体，有可见焦点、原生 checkbox、Radix 模态对话框、系统减少动画与强制颜色样式。不能仅凭这些实现声明完整通过无障碍验收。

## 仍须在可还原虚拟机中完成

| 场景                                                          | 状态与验收方式                                                 |
| ------------------------------------------------------------- | -------------------------------------------------------------- |
| Windows 11，中文 Windows 用户名，100% / 150% / 200% 缩放      | 待验证真实环境；长中文路径夹具不能代替中文账户                 |
| 提权成功、取消 UAC、改用其他管理员凭据                        | 待验证；单元/前端模拟不能代替安全桌面上的 UAC                  |
| 系统临时文件实际删除与断连取消                                | 待验证，仅在 VM 预建旧文件夹具；确认新文件、占用文件和目录不动 |
| 管道伪造 PID/nonce、远程客户端、低完整性进程、日志目录预植入  | 需独立安全验证；源码中的拒绝检查并非测试结论                   |
| 浏览器在确认后真实重新启动、配置目录正在迁移                  | 待验证；现有测试覆盖核心阻止状态及文件竞态                     |
| NTFS 权限拒绝、磁盘满、删除后/提交前强制终止                  | 部分逻辑夹具覆盖，仍需真实故障注入与恢复检查                   |
| 10 万真实磁盘文件、交互不卡顿、键盘/屏幕阅读器/高对比度       | 内存索引分页已测，端到端大目录与辅助技术待测                   |
| 无 WebView2、离线首次安装、全新安装、升级、卸载、非默认安装盘 | 待验证；确认设置保留，卸载不影响任何清理目标                   |
| Authenticode/SmartScreen                                      | 无发行证书，当前包未签名；已提供签名构建入口                   |

对每项记录 VM 快照、系统版本、程序版本、操作步骤、预期/实际结果。以上未完成项是从预览版进入正式发行的门槛。

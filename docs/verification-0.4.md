# 拂尘 0.4.0 验收记录

验证日期：2026-09-26（America/New_York）；测试主机使用 UTC+08:00，最后一轮桌面测试结束于当地 2026-09-27 00:05。环境为 Windows 10 专业版 22H2（19045）x64，普通权限进程；Rust 1.97.1 MSVC、Node.js 24.12.0、pnpm 10.30.3，WebView2 / EdgeDriver 153.0.4234.48。

本轮实现请求核对、真实扫描反馈与清理结果分离。继续提供自定义标题栏、8 条用户规则和免安装文件夹。实际删除仅在专用调试夹具执行；发行程序只对实际目录进行只读扫描和无效请求拒绝测试。功能实现、自动测试与真实环境兼容性分别记录。

## 自动检查

| 检查 | 本轮结果 |
| --- | --- |
| `cargo fmt --all --check` | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| `cargo test --workspace --locked` | 54 项通过：桌面能力与协调 5、规则 1、核心引擎 31、Windows 单元 4、文件系统 13 |
| `cargo check -p dustaway --features system-cleanup` | 类型检查通过；不是系统提权或 UAC 实测 |
| `pnpm test` | 29 项通过：页面 21、明细 3、选择计算 1、命令协调 4 |
| `pnpm build` | TypeScript 与生产资源构建通过 |
| Prettier（src、scripts、配置） | 通过 |
| `pnpm notices` | 465 项依赖；0 项待审查，1 项已有记录的仅开发依赖许可文本例外 |
| Tauri 显式权限核对 | 20 个业务命令全部列入清单，包括新增核对命令 |

新增 Rust 回归使用同步屏障控制命令进入控制锁和清理首项处理的时序，覆盖受理回执、失败/异常后的版本消耗、迟到请求封存、重载、线程开始处理前停止且零删除。扫描覆盖只有近期文件时的实时检查数量和保留原因、取消后的不完整状态、异常时尚未扫描类别的中断标记，以及未纳入计划的独立分页查询。

前端使用可控时钟和真正不结束的 Promise，验证 10 秒读取与 15 秒受理等待、响应丢失后接回、不自动重发写入、核对失败后拦住队列中的后续写入及手动重查。页面覆盖初始化失败后重新加载全部启动信息、扫描反馈、部分结果标题、结果筛选入口、重扫失败保留报告，以及历史和候选读取失败不能误报空结果。

既有句柄删除、Junction、硬链接、父目录替换、长中文路径、占用、日志故障、状态不确定恢复、排除原子性、旧历史兼容、十万候选索引分页及大整数字节统计继续通过。索引规模测试不等于十万真实磁盘文件的性能验收。

开发过程中一次并行前端检查遇到测试 worker 启动超时，未计为通过；最终完整前端检查为 29 项通过。早期桌面测试发现旧版单实例冲突及测试注入方式不适用于不可改写的 invoke 属性；后续使用独立的调试夹具实例标识及真实 IPC fetch 传输层故障注入，并完整重跑通过。失败截图保留作调试材料，不作最终通过证据。

## 真实桌面夹具

使用 `cargo build -p dustaway --features e2e-fixture,custom-protocol` 和 `node scripts/e2e.mjs`。后端同时要求 debug、e2e-fixture 和带标记的专用临时根目录。仅此构建采用 io.dustaway.desktop.fixture；正式发行仍为 io.dustaway.desktop。

最终调试 EXE SHA-256：`A688F58EA2660419B4CD2F73F4F5EEACD0785CE89C909567C0BA55C9B67266D7`。最终调试 EXE 与发行 EXE 嵌入相同界面资源 `index-CUH1mK1J.js`，但构建能力及二进制不同，不能互换哈希。

通过项目：

- 扫描、候选预览、搜索、分页取消、主题切换、确认后返回、真实句柄删除、历史和页面重载。
- 中文带空格的夹具文件在 Explorer 中实际被选中；长期排除保存后重新扫描，排除文件、近期文件、根外保护文档保持存在。
- 在后端受理后丢弃扫描及清理响应，等待界面核对恢复；两种命令的实际调用计数均为 1，未自动重发删除。
- 阻止一个扫描请求进入后端，核对封存后再送达原请求，得到 request_expired；原报告继续保留。
- 本次计划只有第三个旧夹具文件时，仅删除该项；取消勾选的另一个文件保留，出现在“未纳入计划”视图，未计入未完成项。
- 键盘访问已删除结果数量并打开对应明细，完成退出；既有文件菜单、定位、排除确认流程可用，目标焦点在可见区域。
- 最小 760×580 CSS 像素窗口下，明细底边 560 px、分页底边约 543.2 px，列表高度约 220.6 px，无横向溢出。结果筛选和分页可操作。

最终夹具：`C:\Users\ADMINI~1\AppData\Local\Temp\dustaway-e2e-DlO1la`。证据为 [desktop-e2e-0.4.json](../artifacts/desktop-e2e-0.4.json)、[结果截图](../artifacts/desktop-results-0.4.png)、[最小窗口截图](../artifacts/desktop-minimum-0.4.png) 和 `desktop-details-light-0.4.png`、`desktop-details-0.4.png`。已查看最终结果与最小窗口截图。

[资源快照](../artifacts/resource-snapshot-0.4.json) 包含调试主进程及 WebView2 子进程的工作集、私有字节和累计 CPU 时间。这是夹具流程中的单次采样，工作集求和可能重复计算共享页，不作为发行版峰值或性能承诺。

## 发行文件夹

`pnpm portable` 通过。目录为 [拂尘_0.4.0_windows-x64](../artifacts/portable/拂尘_0.4.0_windows-x64/)，恰好包含：

- 拂尘.exe
- LICENSE
- THIRD-PARTY-NOTICES.txt
- inventory.json
- portable-readme.txt
- SHA256SUMS.txt

本轮没有生成安装器或 ZIP，不包含提权 helper。旧版产物和历史记录保留。

最终 EXE：12,082,688 字节，ProductVersion 0.4.0，Authenticode：NotSigned。

```text
SHA-256: FEDB23B11977D97C9C28FA47AE8F39266936D5DB1CF9718EFBB91F99F4EC7FB8
```

`node scripts/smoke-portable.mjs` 对上述最终发行 EXE 实测通过：中文/空格路径、无 helper、禁止程序目录写入后正常启动及只读扫描、重复启动恢复现有窗口、移动 EXE 后设置沿用、自定义最大化/还原/双击/最小化/关闭。客户区顶部与窗口顶部间距为 0 px；程序目录没有产生设置或 WebView2 数据。

Bootstrap 返回标准后端、0.4.0、8 条用户规则。系统状态/快照/定位/排除、混合扫描/准备/执行请求共 7 项返回 capability_unavailable；伪造用户计划返回 invalid_plan。拒绝前后会话与状态一致，没有启动 UAC 或有效清理。

发行测试恢复专用目录的原 DACL；只在扫描偏好仍等于本次测试写入值时恢复原偏好，避免覆盖并发变更。实际缓存未执行删除。证据为 [portable-smoke-0.4.json](../artifacts/portable-smoke-0.4.json)、[只读运行截图](../artifacts/portable-readonly-0.4.png) 和 [release-manifest-0.4.json](../artifacts/release-manifest-0.4.json)。最终构建 EXE、交付 EXE、启动测试记录和 SHA256SUMS.txt 的 SHA-256 一致。

## 尚未实际验证的范围

Windows 11、没有 WebView2 的真实系统、中文 Windows 账户、完整 100%–200% DPI/高对比度/屏幕阅读器、十万真实磁盘文件的资源与取消延迟、多卷空间统计、云占位文件、磁盘满、复杂 ACL 和强制中断 VM 场景仍待扩展验证。模拟测试不替代这些环境。

Native Shell 长路径定位的完整兼容矩阵尚未实测；已有长中文路径句柄检查通过。真实浏览器启动/退出竞态仍需专门环境测试，现有引擎夹具覆盖对应防护分支。远程 CI 未执行。

当前没有发行证书，为未签名预览版。系统提权、命名管道对抗及安装升级仍属独立实验，不属于本轮文件夹版交付。

## 历史记录

- [0.3 原始验收](verification-0.3.md)：原文、日期、通过结论与 EXE 哈希保留。
- [0.2 自定义顶栏与文件夹版](verification-0.2-titlebar.md)。
- [0.2 初次验收](verification-0.2-initial.md)。
- [0.1 验收](verification-0.1.md)。

# 拂尘 0.13 验收记录

版本：0.13.0，Windows x64 未签名免安装预览版。日期：2026-09-27。

## 本轮范围

结果入口携带状态；单类别直达，多个类别按数量降序显示。主操作按受理不明、不确定、失败、未处理、需要关注的保留项排序。正常保留须执行原因完整且分类计数一致，才允许刷新空间结束处理；取消、中断、缺失原因与不一致报告继续核对。

明细绑定原报告、范围、会话和类别，新目标重置搜索及页码，旧响应不能覆盖新内容。文件旁提供下一步与原始原因，继续使用现有定位。八种应用分别显示离线管理指引，未知应用不误配；顶部标题紧凑，阶段结论留在总览，长指引移出列表。

20 条规则、六条只读不变；`artifacts/completion-0.13/backend-boundary.json` 记录 38 个 Rust 与规则文件逐一匹配 0.12 归档，没有实现变化。版本号、前端与文档有更新；IPC、删除协议、历史结构没有变化。

## 可恢复基线

独立目录：`G:\my_ai\DustAway-backups\completion-0.12.0-20260927T152218Z`。

- 实施前归档 275 个源码文件，`source.zip` 与逐文件 `manifest.json` 已保存。
- 独立解压到 `restore-check` 并核对全部哈希；没有另行重构建旧版，不以解压校验冒充构建验证。
- `existing-portable` 保存原有 0.12 程序，恢复步骤在 `RESTORE.md`；没有归档或覆盖用户数据库。
- ZIP SHA-256：`6F8E07FDC1FA685B43420B9189231E7B9703DFF4D3E4A7077DEDBF10AFFF0C3C`。

## 自动化与交付结果

| 检查 | 本轮结果 | 证据 |
| --- | --- | --- |
| 前端 | 13 文件、132 项通过 | `artifacts/completion-0.13/frontend-verified.log` |
| Rust 工作区 | 104 项通过、3 项既有忽略 | `artifacts/completion-0.13/rust-test.log` |
| TypeScript / Vite | 通过 | `artifacts/completion-0.13/desktop.log` |
| 前端格式 / Rust 格式 | 通过 | `artifacts/completion-0.13/format.log`、`rust-fmt.log` |
| Clippy | 通过，warnings 作为错误 | `artifacts/completion-0.13/clippy.log` |
| 桌面专用夹具 | 通过 | `artifacts/desktop-e2e-0.13.json`、`artifacts/completion-0.13/desktop.log` |
| 发行构建 / 依赖声明 | 通过 | `artifacts/completion-0.13/portable-build.log` |
| 发行只读冒烟 | 通过，未启动清理 | `artifacts/portable-smoke-0.13.json`、`artifacts/completion-0.13/portable-smoke-verified.log` |
| 交付哈希 | 五项全部相符，EXE 与实测版本相同 | `artifacts/completion-0.13/delivery.json` |

新增前端测试覆盖单类别直达、跨类别按数量排列、对应状态查询、关闭焦点与列表上下文、重扫清除目标、新目标重置查询、迟到响应隔离、正常保留、混合问题、缺失原因、统计不一致、取消与中断、失效会话。八种应用逐项匹配、未知应用（含原型同名标识）不误配，打开指引不改选择或清理范围。

首次全量测试的旧指引文字断言已更新；新增测试曾因恢复会话后未重新选择、待用模拟响应未重置，以及同文案同时存在于报告和列表而失败。修正测试夹具与查询范围后全量通过，失败日志保留为 `frontend-first.log`、`frontend.log`、`frontend-final.log`，不计为通过结果。

## 桌面与视觉

本机 Windows 10 专业版 10.0.19045 x64；Rust 1.97.1、Node 24.12.0、pnpm 10.30.3、EdgeDriver 153.0.4234.48。WebView2 实际 devicePixelRatio 1.25，1100×780 和 760×580 是客户区 CSS 尺寸，没有用 CSS zoom 冒充其他系统 DPI。

专用夹具 `artifacts/dustaway-e2e-WXEI75` 通过真实 Tauri IPC 执行扫描、确认返回、删除、排除、重扫、历史与重载。扫描和删除响应丢失后分别只受理一次，晚到未受理请求被 `request_expired` 拒绝。六条只读规则在退出应用并重查后仍阻止计划；个人保护夹具保持内容与时间。所有真实删除只发生在受调试 feature 和专用标记约束的夹具内。

部分成功为删除 1、跳过 1、失败 0、不确定 0：扫描后修改的文件仍存在。主操作“核对保留项”直接打开该类别，执行状态为 skipped，显示“文件或所在目录已变化”及重新扫描建议；没有选择框或直接重删。关闭明细后焦点回到主按钮。原生证据在 `filtered-skipped-760.png` 及 `desktop.log`；跨类别核对和正常保留结束由前端 Mock 行为测试验证，没有将它们冒充为原生夹具覆盖。

`artifacts/completion-0.13/layouts.json` 记录六种状态 × 两个尺寸，共 12 次布局断言。首屏结论、问题计数与主操作可见，无横向溢出；明暗主题、减少动效、键盘候选和分析流程通过。已人工查看最小窗口明细、微信与 Brave 指引、结果与完整尺寸截图：长说明在页面或明细内滚动，底栏保持可见。类别列表并不承诺全部在首屏。截图里的负空间变化明确标为同卷观察值，不能当作本工具删除量。

100,002 文件只读分析回归完成，硬链接去重、分页、最大文件、定位、重载、取消与任务互斥通过。单次墙钟 15,619 ms、查询 19.74 ms、取消 91 ms，见 `artifacts/analysis-performance-0.13.json`；仅记录本次运行，不据此宣称性能提升，也不替代既有多轮研究。

## 发行交付

目录：`artifacts/portable/拂尘_0.13.0_windows-x64/`，包含 EXE、LICENSE、第三方声明、依赖清单、说明与 SHA256SUMS 六个文件；不包含安装器或 helper。

- `拂尘.exe`：12,976,128 字节，文件版本与产品版本均为 0.13.0，Authenticode 状态 `NotSigned`。
- EXE SHA-256：`B4743FB44D475BACEC5DDD5BFE03E0FC320065F8562FB830E23888BD7EB6A83F`。
- SHA256SUMS 中五个交付文件逐一通过；实测的发行 EXE 哈希与交付文件一致。
- 依赖声明生成 465 个包，0 项需要许可文本复查，1 项既有开发依赖例外已记录。

发行冒烟目录 `C:\Users\Administrator\AppData\Local\Temp\fuchen-portable-smoke-w3oz0q`（日志使用对应短路径）验证中文与空格路径、拒绝写入的程序目录、无 helper、单实例、窗口操作、移动目录后设置沿用。发行程序仅只读扫描和无效请求拒绝测试，`cleanupStarted` 为 false，没有执行有效删除计划。首页进入分析前读取和刷新空间，受支持卷采样与 C/D 根目录只读分析可停止，实际加载资源与最终 `dist/assets` 一致。

浅色主按钮 hover 对比度 6.08:1、深色 7.01:1，两种尺寸均通过 4.5:1 和无横向溢出检查。色彩测量临时切换 DOM 主题以隔离持久设置，不冒充真实系统主题切换；原生截图为 `artifacts/completion-0.13/release-{light,dark}-{1100,760}-hover.png`。

首次发行冒烟因已有 0.12 程序占用单实例而无法创建驱动会话，未进入扫描，日志 `portable-smoke.log` 保留。旧实例退出后重新运行通过，没有更改产品代码或放宽断言。源代码和程序的 0.12 归档继续保留；本轮用户手册、研究、路线、试用和发布文档已同步。

## 未完成事项与结论边界

- **人工可用性待验证**：没有五名非开发者参与记录；已加入正常保留后结束和跨类别异常核对任务，见 [试用方案](usability-0.13.md)。
- 六条微信、QQ、WPS 规则仍未完成隔离真实应用删除、重建、资料保护验收，保持只读。
- Windows 11、缺少 WebView2、完整真实 100%–200% DPI、屏幕阅读器及物理磁盘写满环境，本轮不冒充已经执行。
- 官方资料核对不等于逐版本实测所有浏览器菜单；指引为离线核对步骤，不授权扩大删除范围。

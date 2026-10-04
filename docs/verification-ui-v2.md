# 空间册 UI 重构验收

日期：2026-09-27。业务版本保持 0.9.0；界面修订标识 ui-v2。四个主要页面已经替换，不在程序中并存经典 UI。规则、IPC、应用标识和数据库格式未变化。

## 改造前归档与恢复

独立备份：G:\my_ai\DustAway-backups\ui-classic-0.9.0-20260927T075051Z。

- source.zip：240 个完整可重建源码文件，包含未跟踪源码、配置、锁文件、规则、资源、文档与测试；排除依赖/编译缓存和用户数据库。
- manifest.json：逐文件 SHA-256、工具链、归档时间与源码包校验值。源码包 SHA-256 为 d6178b24285ab96cfa981c33a4756b6616ebe20a472b4c7e54f88b70a62ebcdc。
- restore-check：从源码包独立解压，原目录、源码包和恢复文件的哈希核对通过；离线锁定安装、54 项旧版前端测试、前端与 Windows 免安装构建通过，随后才修改 UI。
- verified-portable：恢复源码构建的经典 0.9.0 程序。existing-portable 保留现场找到的 0.8.0 和 0.9.0 目录；已有程序不被假定为归档源码的对应构建。
- classic-evidence：保留既有清理、分析、明细、报告、历史、设置、最小窗口等截图与原验收文件，并补拍旧版网页首页、设置。经典确认窗口在改造后从恢复目录补拍（confirmation-web-fixture.png），使用明确标注的模拟数据、不连接扫描或删除接口；源码归档未修改。旧版运行时完整 E2E 没有重跑。
- RESTORE.md 与 validation.json 记录恢复方法和真实验证范围。不覆盖 %LOCALAPPDATA%\DustAway 的设置与历史，不做数据库回迁。

仓库原本没有 Git 提交；没有用新分支冒充备份。

## 本次检查结果

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| 前端行为 | 9 个测试文件、59 项通过 | artifacts/ui-v2/frontend-tests-final.log |
| 线程池交叉复核 | 改造阶段 58 项通过；随后新增定位回归并通过最终默认命令 | artifacts/ui-v2/frontend-tests-threads.log |
| TypeScript / Vite | 通过 | artifacts/ui-v2/build.log |
| 前端与脚本格式 | 本次修改文件 Prettier 通过 | artifacts/ui-v2/format-check.log |
| Rust 工作区基线 | 91 项通过、3 项忽略；后端内容未变 | artifacts/ui-v2-rust-baseline.log |
| Rust 格式 / Clippy | 基线 cargo fmt 与 Clippy -D warnings 通过 | artifacts/ui-v2-clippy.log |
| 业务边界哈希审计 | 135 个后端、规则、IPC、协调器和依赖文件与归档一致 | artifacts/ui-v2/business-boundary-audit.json |
| 桌面 E2E | 真实 WebView2/Tauri IPC 完整闭环通过 | artifacts/desktop-e2e-0.9.json、artifacts/ui-v2/desktop-e2e.log |
| 大量条目 | 100,002 个 NTFS 专用夹具文件，分页、查询、共享硬链接、取消和恢复通过 | artifacts/analysis-performance-0.9.json |
| 四页布局 | 760×580、1100×780、1600×1000 浏览器预览无横向溢出；托盘与导航可见 | artifacts/ui-v2/preview-layouts.json |
| 桌面最小窗口 | 文件明细、分页、分析列表、应用限制与操作栏边界通过 | desktop-e2e-0.9.json 的布局字段及桌面截图 |
| 深色 / 减少动画 / 强制颜色 | Chromium 媒体模拟检查；强制颜色显示平面应用入口，减少动画计算值为 0 s | artifacts/ui-v2/preview-accessibility.json、preview-forced-final.png |
| 100%–200% 等效视口 | 1100×780 基准缩小 CSS 视口检查；不等同于真实 Windows DPI 矩阵 | artifacts/ui-v2/preview-css-viewport-scaling.json |
| 最终免安装构建 | 通过；EXE 12,898,816 字节 | artifacts/ui-v2/portable-build.log |
| 补充故障 / 部分成功 | 真实夹具文件在扫描后消失：定位被拒绝；执行删除 1、跳过 1，保护文件保留 | artifacts/ui-v2/native-states.log |
| 发行程序运行检查 | 通过；中文/空格路径、目录拒绝写入、重复实例、移动后设置保留、只读扫描及系统权限拒绝 | artifacts/ui-v2/portable-smoke.log |

桌面 E2E 包含：扫描、跨视图选择保持、部分读取失败、文件取消/选择、排除草稿与保存、取消确认、真实夹具删除、保护文件、历史、响应丢失核对、未受理迟到请求封存、恢复后重新选择、任务互斥、取消等待，以及六条应用规则的运行中/退出重查/仍受验收限制。键盘操作和定位走真实桌面流程。

第一轮改造检查发现导航测试定位需更新，以及对话框动画覆盖居中 transform 导致最小窗口越界；均已修复并重跑。一次全套 Vitest 遇到 forks 工作进程启动超时，日志保留；最终原 pnpm test 命令已完整通过。上述不是旧版基线失败。

## 环境与限制

本机 Windows 10 专业版 22H2（10.0.19045）x64，Rust 1.97.1、Node.js 24.12.0、pnpm 10.30.3。预览检查和真实 WebView2 检查分别记载，不混称为同一种环境。

未实际验证：Windows 11、缺少 WebView2、真实系统强制颜色主题、真实 100%/150%/200% DPI、多显示器切换，以及微信/QQ/WPS 的真实应用删除验收。新增六条规则保持只读。原有版本不支持、身份未确认、进程未知和设置失败场景继续由前端/核心测试覆盖，并不声称每个场景都有桌面截图。

初步 CSS zoom 压力实验没有模拟真实浏览器缩放：其 100dvh 会一起放大，不能作为 DPI 通过证据；后续采用等效 CSS 视口并保留原始记录。数据表的通过范围以对应证据为限。

恢复旧 UI 时先在独立目录解压、校验与构建，验证成功后再替换工作目录或程序；保留当前用户设置与历史。

## 交付文件

新版：artifacts/portable/拂尘_0.9.0_windows-x64/拂尘.exe。EXE SHA-256：A0ECEF25077B98EC1A5CCE27CDC9D0F16E1828A9AD10A11245635E9ABC679030。其余文件校验值在同目录 SHA256SUMS.txt。

免安装目录已包含使用说明、许可证和第三方清单；465 项清单、无缺少正文的许可、1 项已记录的开发工具例外。不得把 Debug 夹具程序当作发行版。

截图集中在 artifacts/ui-v2：confirmation-light.png、failure-file-changed.png、partial-success.png、history-partial.png、analysis-strata.png、settings-dark.png，以及旧路径中本轮 E2E 生成的最小窗口、文件明细、规则限制等截图。state-evidence.json 标明故障成因和真实处理结果。历史回执只有摘要，未新增虚假路径入口。

四页预览为浏览器演示；删除验证仅使用专用临时目录夹具。发行程序 smoke 只进行真实目录的只读扫描和必须被拒绝的无效请求，没有提交有效删除计划。

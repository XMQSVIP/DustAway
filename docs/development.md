# 开发指南（0.25.0）

面向 Windows x64；使用普通用户 PowerShell、MSVC/Windows SDK、rust-toolchain.toml 指定的 Rust、Node.js 24、pnpm 10.30.3 和 WebView2。

```powershell
pnpm install --frozen-lockfile
pnpm notices
pnpm applications:catalog
pnpm build
pnpm check:quality
```

`check:quality` 是本地与 CI 共用入口，包含格式、Clippy、完整工程检查、模型与证据验证；若生成器修改受检查的源文件则失败。先明确运行生成器并审阅差异，再运行检查。需要真实应用、普通用户权限、桌面驱动或大规模夹具的测试单列，不将忽略项当作通过。

## 运行模式

| 入口 | 用途 |
|---|---|
| `pnpm dev` | 网页预览，无真实扫描或删除 |
| `pnpm desktop` | 普通桌面开发，操作真实目录 |
| `pnpm test:e2e` | 带标记的隔离桌面夹具 |
| `pnpm portable` | 无系统辅助组件的免安装包 |
| `pnpm bundle:installer` | 启用受限 system-cleanup 的安装包 |

只有调试构建和专用测试入口可使用夹具许可；环境变量不能解锁发行删除权限。真实应用验收与夹具测试分别记录，见[本轮验收](verification-025.md)。

原生 E2E 默认写入 `artifacts/desktop-e2e-<版本>`，已存在的目录会被拒绝。重复运行前设置 `DUSTAWAY_E2E_OUTPUT` 为新的目录；分析、清理与应用布局证据均写入该运行目录，不复用历史输出位置。

## 规则与证据

Rust 注册表是能力来源。`pnpm applications:catalog` 更新前端预览、能力摘要、应用目录与证据台账；`pnpm check:applications` 只校验。`pnpm check:recognition` 检查用途库，`pnpm model:validate` 校验模型可复现性与边界。

台账中的隔离测试路径只表示验证入口。`passed` 操作记录不自动代表本次真实应用恢复；`not_indexed` 表示历史证据尚未结构化。实际结果写入独立运行目录，不覆盖旧记录。

## 构建缓存和测试驱动

`pnpm cache:report` 仅报告。清理缓存前先停止本任务编译进程并使用 `pnpm cache:clean -WhatIf`；工具拒绝目录链接，不扩大范围到外部缓存。当前 target 如为目录联接，文件身份测试使用其已核验真实路径，不放宽产品路径保护。

原生测试使用 scripts/desktop-driver.mjs 和匹配 WebView2 的 EdgeDriver；窗口调整使用共享驱动，不直接移动 WebView 子窗口。完整命令和历史环境限制见[测试指南](testing.md)。

[发行流程](releasing.md) · [此前开发指南](history/pre-025/docs/development.md)

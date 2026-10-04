# 开发指南

所有命令在仓库根目录的普通用户 PowerShell 中执行。当前工程面向 Windows x64，不提供 Linux/macOS 桌面构建支持。

## 环境

| 工具       | 当前约定                                     | 依据                               |
| ---------- | -------------------------------------------- | ---------------------------------- |
| Rust       | 1.97.1，MSVC x64，rustfmt、Clippy            | `rust-toolchain.toml`              |
| C++ 工具链 | Visual Studio C++ Build Tools 与 Windows SDK | Win32 / Tauri 编译需要             |
| Node.js    | 24 系列                                      | 当前开发与 CI 配置                 |
| pnpm       | 10.30.3                                      | `package.json` 的 `packageManager` |
| WebView2   | 已安装的兼容运行时                           | Windows 桌面界面需要               |
| 测试驱动   | tauri-driver 2.0.6，匹配运行时的 EdgeDriver  | 仅桌面端到端测试需要               |

初次构建通常需要下载依赖；默认免安装构建无需 NSIS 工具。应用自身的离线运行不表示开发环境可以在未缓存依赖时离线构建。

产品中文名为“拂尘”，Cargo/npm、应用 ID 和数据目录继续使用 DustAway。

## 首次运行

```powershell
pnpm install --frozen-lockfile
pnpm notices
pnpm desktop
```

`pnpm desktop` 直接启动默认用户版开发服务和桌面主程序，不准备辅助进程。它会使用真实的用户目录；调试界面的永久删除仍是真实操作。

需要验证删除时使用专用夹具流程，不以个人缓存目录代替测试数据。默认版本在前后端均关闭系统清理。实验构建通过 `pnpm bundle:installer` 启用；其辅助进程仍要求 Program Files，不属于当前交付。

## 三种运行模式

| 模式         | 命令            | 行为                                          |
| ------------ | --------------- | --------------------------------------------- |
| 网页预览     | `pnpm dev`      | 本地页面，只展示规则和界面，禁止实际扫描/删除 |
| 桌面开发     | `pnpm desktop`  | 真实 Tauri 命令与普通用户目录                 |
| 桌面夹具测试 | `pnpm test:e2e` | 调试构建启用专用后端，仅操作带标记的测试目录  |

网页预览地址为 `http://127.0.0.1:1420`。`pnpm desktop` 会自行启动 Vite；同端口已经有独立服务时，先停止自己之前启动的服务，避免端口冲突。

## 常用命令

| 命令                              | 用途                                                     |
| --------------------------------- | -------------------------------------------------------- |
| `pnpm format`                     | 格式化前端、脚本和指定配置文件，不包含全部 Markdown 文档 |
| `cargo fmt --all`                 | 格式化 Rust                                              |
| `pnpm build`                      | TypeScript 检查和前端生产构建，不生成桌面安装包          |
| `pnpm test`                       | Vitest 与 React Testing Library                          |
| `cargo test -p dustaway-core`     | 核心状态、计划、报告与属性测试                           |
| `cargo test -p dustaway-windows`  | Windows 路径、文件夹具和协议边界测试                     |
| `cargo test --workspace --locked` | 工作区 Rust 测试                                         |
| `pnpm test:evidence`              | 只读证据白名单、完整性与附件哈希测试                    |
| `pnpm check`                      | 许可清单、前端构建/测试、证据检查器及 Rust 工作区测试    |
| `pnpm notices`                    | 从锁定依赖与本地源文件生成第三方声明                     |
| `pnpm portable`                   | 生成声明、默认用户版 EXE 和免安装文件夹                  |

`pnpm check` 不包含下面两项，需要单独执行：

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

首次单独运行桌面 Cargo 命令前，先完成依赖安装、`pnpm notices` 和 `pnpm build`，确保资源已准备好。

## 构建缓存与磁盘空间

开发及测试构建默认使用 `debug = 1`、`incremental = false`：保留基本调试信息，减少完整类型信息和增量对象缓存。代价是修改代码后的再次编译可能变慢，调试器默认不包含完整局部变量信息。需要完整调试时，在当前 PowerShell 会话临时设置 `$env:CARGO_PROFILE_DEV_DEBUG='2'`；测试对应 `CARGO_PROFILE_TEST_DEBUG`。增量编译可临时设置 `$env:CARGO_INCREMENTAL='1'`，用完移除环境变量。Release 配置不变。设置语义见 [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)。

`pnpm cache:report` 只读显示仓库 `target` 各子目录的逻辑大小。`pnpm cache:clean -WhatIf` 预览调试输出清理，`pnpm cache:clean` 才实际清理 `target/debug`。也可直接运行：

```powershell
& ./scripts/build-cache.ps1 -Mode Report
& ./scripts/build-cache.ps1 -Mode Clean -Scope incremental -WhatIf
& ./scripts/build-cache.ps1 -Mode Clean -Scope debug
```

`-Scope all` 会删除整个仓库 `target`，包括 Release 与基准输出，使用前将需要保留的程序打包到 `artifacts/portable`。脚本不扫描或删除源码、备份、测试证据、用户缓存或其他项目；遇到链接或运行中的编译进程会拒绝。它只处理仓库本地 `target`，不跟随 `CARGO_TARGET_DIR` 或其他外部构建目录。构建期间不要清理，也不要在脚本检查与删除期间启动新构建。默认不自动删除缓存，没有硬性磁盘配额；长期切换功能组合仍可能积累旧输出。

重复实验尽量复用同一构建目录，保留小型日志及必要基线 EXE 即可，不必为每轮实验复制一套依赖。独立 benchmark 继续按其说明构建，历史记录中的目录是当时测量配置，不要求全部重建。

停止编译与基准进程后可运行 `pnpm test:build-cache`，验证报告、预览不删除及目录链接拒绝。测试仅在临时目录建立小型标记夹具，不执行实际删除，最后输出夹具位置供核对。

## 代码入口

| 位置                                 | 职责                                    |
| ------------------------------------ | --------------------------------------- |
| `crates/core/src/model.rs`           | 规则、候选、计划、报告、状态与平台接口  |
| `crates/core/src/engine.rs`          | 会话、SQLite 索引、计划生成、执行与恢复 |
| `crates/core/src/rules.rs`           | 内置 TOML 解析与规则白名单              |
| `crates/windows/src/platform.rs`     | 标准目录识别、扫描与平台执行入口        |
| `crates/windows/src/fs.rs`           | 句柄、目录链、属性、身份与删除系统调用  |
| `crates/windows/src/security.rs`     | 令牌、进程、会话和安全描述符            |
| `crates/windows/src/ipc.rs`          | 提权启动、管道认证与辅助进程服务        |
| `crates/helper/src/main.rs`          | 辅助进程入口                            |
| `src-tauri/src/commands.rs`          | 业务命令及用户/系统任务协调             |
| `src-tauri/src/lib.rs`               | 应用初始化、服务状态、单实例与退出处理  |
| `src/App.tsx`、`src/FilesDialog.tsx` | 主界面、状态展示、明细与确认            |
| `src/api.ts`、`src/types.ts`         | 前端命令封装与数据类型                  |
| `src/i18n.ts`、`src/styles.css`      | 中文界面文案与样式                      |

核心不得依赖 Tauri 或 React。文件删除和根目录判断留在 Rust；前端只管理选择、展示和用户确认。

## 修改功能的顺序

1. 确定是否改变产品范围、权限域、规则或数据结构。
2. 在所属模块实现逻辑，不通过扩大通用 IPC 或开放 Shell 省略业务边界。
3. 对文件操作、会话失效和异常处理补充能揭示风险的夹具测试。
4. 同步前后端类型、文案、规则预览与对应文档。
5. 执行受影响测试；涉及桌面命令或确认流程时执行夹具 E2E。
6. 需要交付时再构建程序文件夹，并记录实际通过的验收范围。

## 常见开发问题

- **MSVC 或 Windows SDK 缺失**：先补齐 C++ 工具链；不要通过删除 Win32 检查绕过编译问题。
- **实验构建缺少辅助进程**：执行 `pnpm prepare:helper`；输出名称带 `x86_64-pc-windows-msvc`，由打包流程使用。
- **找不到前端资源**：先执行 `pnpm build`，并检查 `frontendDist` 指向根目录 `dist/`。
- **Cargo 正在等待构建目录锁**：等待正在进行的本仓库 Cargo 命令结束，不需要删除锁文件或依赖缓存。
- **WebDriver 版本不兼容**：核对 WebView2 与 EdgeDriver 版本、驱动路径和 4444 端口占用。
- **E2E 拒绝普通构建**：使用完整 `pnpm test:e2e` 重新构建，不移除后端模式检查。
- **NSIS 下载失败**：检查访问官方发布地址的网络配置，保留工具校验，不替换为未验证的二进制。

本仓库 CI 配置在 `.github/workflows/windows.yml`。它定义自动构建和测试流程，但配置文件存在不等于已在远程服务执行成功；本机结果与真实 Windows 验收仍需分别记录。

## 应用只读验收准备（0.14）

使用 `node scripts/application-evidence.mjs collect <新目录>` 生成本地脱敏证据包，`check <目录>` 校验哈希和逐规则结果，`node --test scripts/application-evidence.test.mjs` 验证检查器。完整命令与隔离实测操作单见 [支持矩阵](support-matrix-0.14.md)。收集不使用个人资料做删除实验，不会改变静态验收标记。

## 自选统计性能与普通桌面检查

`cargo test -p dustaway-core --locked manual_selection_performance -- --ignored --nocapture` 对 50 万条合成记录分别测索引、完整摘要、稀疏摘要和准备，三次输出；增加 `--release` 可单独测优化构建。`manual_preview_half_million_records` 检查分页与取消延迟。均不删除真实用户文件。

先执行 `pnpm build` 和 `cargo build -p dustaway --locked --features custom-protocol`，再运行 `node scripts/stability-desktop-checks.mjs`。它串行执行磁盘选择、浅色/深色 × 760×580/1100×780 四次完整自选隔离流程及原生窗口边界检查；不启用 `e2e-fixture`。桌面删除范围仅为工具生成并验证的专用夹具。记录位于 `artifacts/stability-desktop`、`artifacts/manual-standard-desktop/<theme>-<width>`。结果与限制见 [验收记录](verification-stability-speed.md)。

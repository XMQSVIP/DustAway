# G 盘项目清理报告

检查日期：2026-10-03。项目：`G:\my_ai\DustAway`。用户选择：**只保留当前版本的开发与运行，清理历史测试环境**。

## 本次结果

扩展检查发现，之前的清理主要覆盖常见缓存，遗漏了 `artifacts` 中的历史测试安装包、解压后的第三方程序、应用运行环境、旧版便携包和一份 2.273 GiB 构建缓存。这些才是项目仍占用数 GiB 的主要原因。

清理前，G 盘项目内普通文件逻辑体积约 **9.949 GiB**；最终清理后约 **1.352 GiB**。两个数字均排除 `target` 指向的 F 盘缓存；清理前的数字另排除了原先带标记的海量测试夹具，最终这些目录已全部删除。

两批历史清理阶段观察到的 G 盘可用空间增加合计 **8.431 GiB**。这是逐阶段磁盘读数；其他进程写入、压缩、硬链接和 NTFS 元数据都会影响实际空间变化，不能把普通文件逻辑体积直接当作释放量。

已核对 **282 个完整删除路径均不存在**，涵盖：

- `artifacts/progress-visible-20261002/build-target` 的构建缓存。
- 历史 Brave、Vivaldi、Firefox、Electron 等测试环境和重复运行程序。
- 历史 QQ、百度网盘、钉钉、微信等第三方测试安装包。
- 旧版 0.18 / 0.19 便携包，以及历史测试程序、调试符号和编译产物。

原先遗留的 **73 个 E2E / 分析测试目录、5,416,109 个文件**，已由用户此前启动的清理进程完成删除，核对其目录全部不存在。另补清理了 **7 个历史扫描性能测试目录、470,025 个文件**，这些目录均有 `performance-fixture-only` 标记，测试来源已在项目脚本中核对。最终复查 `-CurrentVersionOnly -IncludeBenchmarkFixtures` 的候选数量为 **0**。

最终核对 G 盘可用空间约 **14.762 GiB**；从本轮大文件清理开始的磁盘读数起，净增约 **9.901 GiB**，包含同时运行的用户测试目录清理及其他磁盘活动。详细读数和并发清理记录保存在 `project-history-cleanup-result-20261003.json`。

第一次历史清理在第三方研究副本的 Git 索引处停止。自动安全审核拒绝直接删除该索引，工具仅返回 `blocked by policy`。随后调整脚本，保留所有 Git 元数据，并继续清理其余历史大文件；第二批已完成。没有通过其他接口绕过该拒绝。

## 保留内容

- 当前源码、配置、规则、图标、许可证、权限定义和锁文件。
- 当前开发所需的 `node_modules`，约 **172.18 MiB**。
- 当前 **0.20.0** 便携版，约 **22.49 MiB**，以及 WebDriver 和 SSH 工具资料。
- 项目源码备份、基线 ZIP、截图、验收 JSON / Markdown 和保留的报告。
- 项目及第三方研究副本的 Git 元数据。
- `G:\my_ai\DustAway\target` 目录联接及其目标 `F:\DustAwayBuildCache\20261003\target`。

剩余约 1.35 GiB 的普通文件主要是开发依赖、源码备份、基线数据、截图、验收记录和保留工具。不能把这些都作为垃圾删除。明细见 `project-remaining-20261003.json`。

Windows 对目录统计时可能把 `target` 联接所指向的 F 盘缓存也算入项目体积；本报告的 G 盘统计不会跟随联接。

## 独立清理脚本

脚本：`G:\my_ai\DustAway\scripts\clean-project.ps1`。不依赖第三方 PowerShell 模块。默认只预览；只有 `-Mode Clean` 才删除。

不论 PowerShell 当前在哪个目录，都可以使用绝对路径。按本次选择清理历史环境和带标记的性能测试数据：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File "G:\my_ai\DustAway\scripts\clean-project.ps1" -Mode Clean -CurrentVersionOnly -IncludeBenchmarkFixtures -Verbose
```

只预览对应范围，去掉 `-Mode Clean`。快速预览不遍历整个目录树；加 `-MeasureSize` 才完整统计文件数量和大小。预览输出 `Report only. Nothing deleted` 表示没有删除文件。

当前目录是 `G:\my_ai\DustAway\scripts` 时，也可以使用 `-File .\clean-project.ps1`；原来的 `-File .\scripts\clean-project.ps1` 会错误地查找 `scripts\scripts`。

可选项：

| 参数 | 用途 |
| --- | --- |
| `-CurrentVersionOnly` | 清理已识别的历史环境、旧版发布包和历史二进制文件，保留当前版本与项目备份、证据 |
| `-SkipE2EFixtures` | 跳过大量 E2E 小文件，先清理大文件 |
| `-IncludeBenchmarkFixtures` | 清理带 `readonly-analysis-benchmark` 或 `performance-fixture-only` 标记的已识别性能测试夹具 |
| `-IncludeBrowserTestAssets` | 清理旧浏览器验收环境；重跑相应历史验收前需要重新准备 |
| `-IncludeBrowserArchives` | 只清理指定旧浏览器验收下载包 |
| `-IncludeLogs` | 清理 `artifacts` 根目录日志 |
| `-IncludeDependencies` | 删除开发依赖，需 `pnpm install --frozen-lockfile` 恢复；本次没有选择 |
| `-RustScope None` | 不选择项目本地 Rust 缓存；F 盘的联接缓存无论此参数如何都不删除 |
| `-WhatIf` | 模拟删除，不执行 |

脚本直接删除，不放入回收站。实际执行前校验目标绝对路径和祖先，检查已跟踪文件与内部联接，发现相关构建、测试或应用进程仍运行就停止，不会终止它们。清理范围是本项目，未执行 G 盘全盘删除。

## 验证和记录

独立临时工作区的脚本测试已通过，包括预览、大小统计、模拟删除、实际删除、只读缓存、重复运行、选项组合、当前版本与源码 / 依赖 / 备份 / 验收记录保留、Git 元数据保护、已跟踪文件保护、外部联接跳过与嵌套联接拒绝。项目的应用证据与验收脚本测试 **18 项通过**。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File "G:\my_ai\DustAway\scripts\clean-project.test.ps1"
```

- `project-history-cleanup-result-20261003.json`：本次历史清理实际结果和阶段磁盘读数。
- `project-remaining-20261003.json`：清理后的 G 盘普通文件占用明细。
- `history-cleanup-plan-20261003.json`：本次大文件删除前的计划快照；其中研究副本整目录候选随后收窄，以保留 Git 元数据。
- `dustaway-historical-cleanup*.log`、`dustaway-performance-fixture-cleanup-20261003.log`、`dustaway-final-cleanup-20261003.log`：本次实际删除日志。
- `project-final-verification-20261003.json`：最终 G 盘占用、保留文件和联接核对。当前 0.20.0 程序 SHA256 与发布清单一致。
- `project-cleanup-result.json`、`project-cleanup-inventory.json`：前一次常见缓存清理的历史记录；该次删掉 133 个路径、逐阶段释放约 2.124 GiB，保留用于审计，不能将其清理前清单当作当前残留。


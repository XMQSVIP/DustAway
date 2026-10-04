# 开发工具缓存首批补齐（0.18.0 独立测试包）

2026-10-01。此前开发工具只有空间分析用途识别，没有普通垃圾扫描规则；本轮新增 8 条固定目录规则，归入“开发工具缓存”。扫描只展示实际候选，未安装但仍有缓存的工具也可以出现；无文件的条目不展示。继续隐藏“本版本检查范围”。

## 范围

所有新规则默认不选，需要查看影响后自行选择。只检查当前用户标准位置；自定义缓存位置、便携 IDE 配置、项目内目录不自动推断。`L` 表示 LocalAppData，`R` 表示 RoamingAppData，均由 Windows 已知目录取得。

| 规则 | 固定位置与格式 | 最小文件年龄 |
| --- | --- | --- |
| npm 下载缓存 | `L/npm-cache/_cacache`；content-v2/sha512 和 index-v5 已知哈希布局 | 30 天 |
| npm 历史日志 | `L/npm-cache/_logs` 下带年份的 `.log` | 7 天 |
| pip 下载缓存 | `L/pip/Cache/http`、`http-v2` 的哈希文件及 `.body` | 30 天 |
| pip 构建轮子缓存 | `L/pip/Cache/wheels` 的哈希层级 `.whl`；保留 origin.json | 30 天 |
| NuGet 下载缓存 | `L/NuGet/v3-cache` 已知源哈希目录内 `.dat` | 7 天 |
| VS Code 运行缓存 | `R/Code`、`R/Code - Insiders` 的 Cache/Cache_Data、Code Cache、GPUCache；仅已知 Chromium 缓存文件格式 | 7 天 |
| VS Code 历史日志 | 同上产品的 logs/时间戳目录内 `.log` 及数字轮转日志 | 7 天 |
| JetBrains 历史日志 | `L/JetBrains/已知产品年份版本/log` 下 `.log` 及数字轮转日志 | 7 天 |

删除下载缓存可能使下次安装重新联网；不保证离线安装仍可使用缓存。删除日志会失去对应历史排错记录。源码、node_modules、虚拟环境、SDK、NuGet 全局包目录、IDE 设置/插件/工作区/未保存备份、JetBrains LocalHistory 不在这些规则中。pnpm store、Cargo registry/target、uv 缓存、Docker/WSL、项目构建产物尚未纳入本批。

Node、Python/pip/uv、dotnet/MSBuild/Visual Studio、VS Code、JetBrains/Java 等相关进程存在时阻止对应清理。检查失败沿用“状态未知”与复查流程；不强杀进程。执行前再次核对进程、固定根目录、文件身份、年龄和父目录；链接、云文件、长期排除与已有中断不续删机制保留。无新 IPC、任意路径入口、数据库迁移或历史报告格式变化。既有应用的版本许可不变。

## 依据

- [npm cache](https://docs.npmjs.com/cli/v11/commands/npm-cache/) 与 [日志目录](https://docs.npmjs.com/cli/v11/using-npm/logging/)。
- [pip 缓存说明](https://pip.pypa.io/en/stable/topics/caching/)；其内部布局不是稳定 API，因此仅接纳明确的现有格式，未来布局不会自动扩大匹配。
- [NuGet 缓存与全局包目录区分](https://learn.microsoft.com/en-us/nuget/consume-packages/managing-the-global-packages-and-cache-folders)。
- [VS Code 主进程缓存实现](https://github.com/microsoft/vscode/blob/main/src/vs/code/electron-main/app.ts)。
- [JetBrains 设置、缓存、插件和日志目录说明](https://www.jetbrains.com/help/idea/directories-used-by-the-ide-to-store-settings-caches-plugins-and-logs.html)。本批只处理日志，不删除整个 system 目录。

## 验证记录

原始日志保留于 `artifacts/developer-cleanup-20261001/`。`before.zip` 是本批改动前源码快照。首次核心测试的旧规则数量断言、首次 Windows 测试的扩展路径比较、junction 测试命令斜杠以及首次 UI 测试未展开折叠区的失败日志均保留，不作为通过证据。

实机只读预检确认：DESKTOP-24NJTT6 的 MSN 用户已有 npm/pip 缓存；NuGet、VS Code、JetBrains 标准目录不存在。新建这些目录仅用于专用夹具，不能称为核验安装。未安装工具的实际使用与再生成、所有工具版本兼容仍未验证。

本次不扩展百度等未开放规则，也不改变钉钉许可；钉钉首页真实闭环仍未验证。此前浏览器真实清理证据绑定此前程序哈希，不能替代本次程序的验收。

### 自动检查与性能

- 前端构建通过；21 个测试文件、298 项通过。
- Rust workspace：264 项通过，24 项默认忽略，忽略项不计为通过。新规则测试覆盖固定布局、近期文件、身份变化、链接目录、排除、取消、混合根目录、进程查询失败、明确选择、单文件选择与计划防重放。
- workspace/all-targets Clippy（`-D warnings`）、Rust 格式、本次前端文件格式检查通过。全仓库 Prettier 仍报告 11 个本批未修改文件的既有格式问题，详情保留于 `frontend-format.log`，没有将该次检查计为通过。
- 应用目录投影与识别库一致性检查通过；证据工具 12 项、验收工具 6 项、桌面驱动 2 项通过。许可证清单生成通过。
- 与上一轮留存的 debug 扫描程序比较：同一万文件夹具，预热后交替五轮，候选清单哈希全部一致。基线中位数 15,931.897 ms，本次 15,074.330 ms，比值 0.94617，满足不退化超过 10% 的门槛。仅代表合成临时文件扫描，不外推至真实开发缓存吞吐；本轮未据此声称普遍提速。原始数据见 `scan-performance/`。
- 另行执行原显式忽略的 50 万条记录响应测试并通过：摘要 9,227 ms；摘要期间分页 466 ms、取消 0 ms；准备期间分页 505 ms、取消 0 ms，分页与取消均小于一秒。

### Windows 11 与交付

通过严格主机校验连接 DESKTOP-24NJTT6，使用 MSN 交互桌面的 Limited 令牌。未改变防火墙、网络、UAC 或持久执行策略，未安装软件、未强杀用户应用。测试结束后程序及驱动均已退出。

普通 Windows 后端夹具覆盖全部 8 条新规则：只按精确候选 ID 选择预先记录的 8 个新文件，实际删除 1,098 B，failed/skipped/unknown 均为 0；8 个保护样本逐项 SHA-256 一致，计划防重放验证通过。扫描同时发现原有 npm/pip 缓存与日志共 770 个候选文件，未选入实际删除清单。测试期间卷可用空间变化 -131,072 B，独立记录，不等同于逻辑删除量。

实际交付的普通 release 程序完成 760×580、1100×780 × 深浅主题四组原生流程：扫描、开发缓存默认不选、Node 运行时阻止 npm、复查保留选择、取消确认保留选择并恢复焦点、核对一次性清单、删除、完成及历史核对。每组只删除 1 个专用 VS Code 日志样本，共 4 个、148 B；保护样本保持不变。已查看浅色选择页、深色窄窗确认及完成截图。

首次桌面脚本在后台 ready 后、前端轮询仍显示扫描中时过早检查主操作，失败发生在任何界面清理计划与执行之前。保留 `evidence/` 原始失败截图和日志。改为等待前端 review 状态后，在独立 `evidence-desktop-v2/` 完成四组验收；八条后端夹具清理没有重跑，原始 GUI 样本仅重新扫描，没有重放删除。保护样本留作证据。

远端原始证据包 SHA-256：`69AE3916D5F2DAFF735AFAEC21B78B040437C129A6A91FDBA379804864C2FC62`。回传后 56 项清单记录逐项长度、SHA-256 一致，成功与失败记录均保留。清单和结果见 `artifacts/developer-cleanup-20261001/delivery-verification.json`。

交付：[Windows x64 免安装测试包](../artifacts/portable/拂尘_0.18.0_developer-cache-test-20261001_windows-x64.zip)。版本保持 0.18.0，包含程序、许可证、第三方许可清单、使用说明和 SHA256SUMS；没有夹具后端或系统提权 helper。

- 程序 SHA-256：`E8E4A20F0D5176B270F9F2DBD08257DA38566BE1D37E330157BD19E8A0FB241B`，与四组原生流程逐项绑定。
- ZIP SHA-256：`4761977FEB6B42D977DAF1D1CEB54B0A246BA0FDC513B8A7934294E2A96F4559`。

新增规则的真实用户缓存删除与工具重新使用/缓存再生成未测试。npm/pip 完成真实缓存识别；NuGet、VS Code、JetBrains 仅完成布局与专用夹具验证，不能称为已安装软件的完整兼容验证。

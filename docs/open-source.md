# 开源参考与许可

拂尘（DustAway）自有代码和规则采用根目录 MIT 许可。以下项目作为设计研究资料，没有复制其实现或清理规则：

| 项目                                                                                                         | 吸收的思路                               | 本项目范围                                 |
| ------------------------------------------------------------------------------------------------------------ | ---------------------------------------- | ------------------------------------------ |
| [BleachBit](https://github.com/bleachbit/bleachbit) / [CleanerML](https://docs.bleachbit.org/cml/cleanerml/) | 声明式分类、预览与执行分离、应用运行条件 | 自行编写窄 TOML；不引入 GPL 规则或脚本动作 |
| [Czkawka](https://github.com/qarmin/czkawka)                                                                 | Rust 核心独立于 UI，取消和进度属于引擎   | 不实现重复文件、相似媒体分析               |
| [BCUninstaller](https://github.com/BCUninstaller/Bulk-Crap-Uninstaller)                                      | 给出清理依据和影响                       | 不用猜测评分驱动残留删除                   |
| [disktree](https://github.com/tobi/disktree) | 批量元数据读取、分配空间与逻辑长度区分、协作取消 | 自行实现 Windows 读取与临时索引，保留 Tauri 界面和删除前核验；见[设计](analysis-million-design.md) |

路径依据：[Chromium User Data Directory](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/user_data_dir.md)、[Firefox Profiles](https://support.mozilla.org/en-US/kb/profiles-where-firefox-stores-user-data)。系统组件维护交给 [Windows 官方工具](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/clean-up-the-winsxs-folder?view=windows-11)。

主要直接依赖包括 Tauri（MIT/Apache-2.0）、React（MIT）、TypeScript（Apache-2.0）、Vite（MIT）、Tailwind（MIT）、Radix（MIT）、Lucide（ISC）、TanStack Virtual（MIT）、windows-rs（MIT/Apache-2.0）、rusqlite（MIT）和 SQLite（公有领域）。准确版本、许可表达式及传递依赖以自动生成的 `licenses/inventory.json` 与 `licenses/THIRD-PARTY-NOTICES.txt` 为准，不以此简表代替许可声明。

运行 `pnpm notices` 从已安装的锁定依赖生成清单，保留包自带的 LICENSE / NOTICE 等原文。清单包含开发、测试和跨平台构建依赖，是保守超集，不表示全部被编入 Windows 可执行文件。没有顶层许可文件的包会标记待复核；发行前对这些包核查来源并补齐。

WebView2 是另行安装的 Microsoft 运行时；NSIS 是安装器构建工具，包含自己的许可说明。DustAway 的 MIT 许可不会改变第三方组件各自的许可。

当前清单中 `stackback 0.0.2` 仅由 Vitest 经 why-is-node-running 引入；其[上游包声明 MIT](https://github.com/defunctzombie/node-stackback/blob/master/package.json)，但 npm 包与上游仓库未附许可全文。`licenses/review.json` 记录这一开发依赖例外，清单仍如实保留 `licenseTextIncluded: false`，没有自行编造许可正文。它不被前端导入，也不进入发行程序；若未来用于运行时须重新复核。部分其他包缺失的许可已从记录了提交/内容哈希的上游来源补入 `licenses/overrides/`；正常离线构建只读取这些文件。

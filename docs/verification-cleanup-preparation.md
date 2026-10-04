# 微信、钉钉、百度网盘：清理能力准备验收

日期：2026-09-30。基于 0.18.0 源码开发，未发布新包。本轮完成六条规则的开发准备，真实应用删除、复启、功能核对、缓存再生成及新日志生成均未执行；不据合成结果开放规则。正式能力仍为 **35 条用户规则、14 条条件清理、21 条只读**，18 条应用规则全部只读。

## 实现与范围

首批为 `weixin.logs`、`dingtalk.http`、`dingtalk.code`、`baidu_netdisk.http`、`baidu_netdisk.code`、`baidu_netdisk.gpu`。固定路径、期限和默认手动选择策略保持原约束。需求依据、保护范围与后续步骤见[支持矩阵及操作单](support-matrix-cleanup-preparation.md)。未声称完成真实用户访谈。

新增编译期逐规则验收表，绑定规则版本、目录布局、准确应用版本与证据标识；本轮表为空。扫描观察版本不再决定删除版本资格。计划准备、执行与 Windows 删除边界复核同一资格；同一应用的其他规则不会连带开放。界面使用逐规则 `cleanup_versions` 和 `cleanup_block`，缺失信息时拒绝推断清理权限。旧字段和历史数据库保持兼容。

Rust 的合成授权模块只在 `cfg(test)` 构建存在；仅通过模块内工厂创建专用临时目录并验证目录身份、标记和路径映射，不能由环境变量、IPC、验收 JSON 或普通发行构建启用。测试复用真实候选校验与 Windows 句柄删除。

## 工程验证

环境为 Windows 10 Pro 10.0.19045，本次没有另行验证 Windows 11 或全部 DPI 配置。

| 检查 | 结果与记录 |
| --- | --- |
| 前端全量 | 18 个文件、235 项通过；`artifacts/cleanup-preparation-frontend-final.log` |
| Rust workspace | 153 项通过、3 项按原配置忽略；`artifacts/cleanup-preparation-rust-final.log` |
| 独立验收及历史证据工具 | 18 项通过；`artifacts/cleanup-preparation-tools-final-2.log` |
| 前端生产构建 | TypeScript 与 Vite 构建通过 |
| 格式 | Rust fmt 与 Prettier 通过；`artifacts/cleanup-preparation-format-final.log` |
| 用途库及许可 | 256 条规则、510 个主匹配项一致；许可清单生成通过，保留既有开发依赖例外 |
| Clippy | workspace / all-targets / `-D warnings` 通过；`artifacts/cleanup-preparation-clippy-final.log` |
| 应用目录投影 | 18 条应用规则投影一致；`artifacts/cleanup-preparation-catalog.log` |
| 桌面构建与完整流程 | 调试构建及完整流程通过；`artifacts/cleanup-preparation-desktop-build.log`、`artifacts/cleanup-preparation-e2e.log` |

忽略项是隔离真实浏览器验收、标记目录扫描性能基准、最大文件查询性能基准；不计为通过。全量前端首次运行遇到一个 worker 启动超时，完整重跑后通过。Rust 首轮暴露旧内存 Fake 直接启用 QQ 规则与新的生产资格校验冲突，已将模拟授权明确限制在 Fake 实现中；普通库的拒绝测试随后通过。失败轮次日志保留，不作为通过证据。

## 六条规则的合成验证

七个 Windows 测试覆盖六条规则的真实扫描、候选选择、手工复核、一次性计划、句柄删除、报告、取消、重开历史，以及执行两次删除之间的文件变化和后台进程重启。另覆盖微信七天临界值、近期和未来文件、多配置、配置标记缺失、长期排除、硬链接与离线属性、同版本程序身份变化、状态查询失败和伪造计划。既有布局测试继续覆盖相邻目录与重解析路径。

保护样本通过 SHA-256 与完整 FILETIME 字符串核对，避免 JavaScript 大整数舍入。包含聊天附件、已完成/未完成下载、任务数据、持久站点存储及离线文档。所有实际删除仅涉及工具创建的合成目录。

独立工具的 `init / synthetic / pack / check` 命令分别创建待验收模板、运行合成闭环、封装人工记录和校验记录。输出固定为 `may_enable_cleanup=false`，真实步骤未执行时保持 `not_run`；即使人工填为通过也不能自动生成解锁凭据。

已生成[六条合成验收记录](../artifacts/cleanup-preparation-synthetic/acceptance.json)及同目录校验清单，工具再次校验通过：`synthetic_passed=6`、`real_recorded_passed=0`、`real_acceptance_review=not_performed`。对应日志为 `artifacts/cleanup-preparation-synthetic-final.log`。当前源码的[待验收模板](../artifacts/cleanup-preparation-template-final/acceptance.json)单独保存；初始模板没有覆盖。

| 规则 | 合成候选 | 删除 | 保留未选 | 删除逻辑字节 | 保护样本未变 | 真实验收 |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| weixin.logs | 3 | 2 | 1 | 86 | 15 | 未执行 |
| dingtalk.http | 6 | 5 | 1 | 215 | 20 | 未执行 |
| dingtalk.code | 6 | 5 | 1 | 215 | 20 | 未执行 |
| baidu_netdisk.http | 3 | 2 | 1 | 86 | 10 | 未执行 |
| baidu_netdisk.code | 3 | 2 | 1 | 86 | 10 | 未执行 |
| baidu_netdisk.gpu | 3 | 2 | 1 | 86 | 10 | 未执行 |

合成字节仅说明夹具流程，不代表用户收益或卷可用空间变化。首次记录生成与其他 Cargo 检查同时运行，等待构建锁超时且没有输出证据包；工具时限已调整至 15 分钟，并提供不含私人路径的超时提示。顺序重跑后成功生成与校验，原失败日志保留。

## 桌面回归

[桌面结果](../artifacts/cleanup-preparation-e2e/desktop-e2e-0.18.0.json)确认 `passed=true`、`perRuleCleanupCapabilityVerified=true`。涵盖 35 条扫描规则、18 条应用只读规则、确认取消、专用夹具删除、保护文件、长期排除、丢失请求核对、恢复、历史及 100,002 个文件只读分析。逐规则能力字段均按当前只读状态返回，应用候选不能进入预计清理量。真实应用权限未改变。

微信、钉钉、百度网盘及既有夸克、迅雷分组在 1100×780、760×580 两种窗口下的 10 个组合全部通过：无横向溢出、分组标题和只读行可见、没有勾选控件。布局断言见 [application-group-layouts.json](../artifacts/cleanup-preparation-e2e/application-group-layouts.json)，日志为 `artifacts/cleanup-preparation-visual-final.log`。已查看[微信窄窗口](../artifacts/cleanup-preparation-e2e/weixin-group-760.png)、[钉钉标准窗口](../artifacts/cleanup-preparation-e2e/dingtalk-group-1100.png)和[百度窄窗口](../artifacts/cleanup-preparation-e2e/baidu_netdisk-group-760.png)截图，已选与预计清理量保持 0 B。

首次扩充扫描范围后，补测脚本仍等待旧的 7 条数量导致超时，已改为从实际请求列表计算预期数量；重跑通过，失败日志保留。窗口尺寸检查不等于全部真实 DPI 验收。桌面程序为带既有夹具功能的调试构建；普通库集成测试另外确认所有应用规则的伪造清理开关均在文件解析前被拒绝，不能获得 Rust 测试模块的合成授权。

## 兼容与交付边界

v1–v3 只读证据保持原字段投影与目录校验规则；新增 IPC 字段没有写进旧 v3 文件格式。既有 v2 最终包 `artifacts/office-extension-local-evidence-final` 和 v3 包 `artifacts/netdisk-local-evidence` 按原工具通过校验。旧 v2 中间草稿 `office-extension-local-evidence` 的版本不符仍被拒绝，没有改写草稿以制造通过。

修改前已保存 360 个源码文件：[基线快照](../artifacts/cleanup-preparation-baseline.zip)，SHA-256 为 `55ECD38BCB16EA93F54BE06A24028480ED641763C5A4713F27E6DBF97527B8F8`，元数据见同名 JSON。源码保持原有 Git 未跟踪状态；相对快照的变更记录见 `artifacts/cleanup-preparation-source-changes.json`。历史应用证据与发布包保留。

本轮未启动三款真实应用来制造缓存，也没有删除真实应用资料。后续须逐规则取得真实非零候选，完成保护资料核对、复启与功能检查、缓存再生成或正常新日志生成，再独立审阅证据并变更编译记录及规则描述符。未完成的规则持续只读。

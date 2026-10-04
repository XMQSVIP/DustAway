# 微信、钉钉、百度网盘：清理能力开发准备

2026-10-01 后续进展：钉钉网页、代码缓存分别完成真实删除、保护核对、缓存再生成，用户确认正常登录后功能和测试资料正常。源码为 `dingtalk.http`、`dingtalk.code` / `8.2.0.119` 分别加入独立许可，默认不选，普通首页闭环尚未完成。当前 35 条用户规则、16 条条件清理、19 条只读；百度三条仍只读。用户未开启自动登录，退出后登录页是正常行为，不作为失败。微信日志按用户要求本轮跳过。以下为 09-30 开发准备快照（当时许可表为空），当前证据见 [首页清理验收](verification-home-cleanup.md)。

日期：2026-09-30。基于 0.18.0 源码，未发布新包。35 条用户规则、14 条条件清理、21 条只读；本轮六条目标仍只读。真实应用删除与用户试用均未执行。

## 需求与取舍

用户需要在保留工作资料的前提下释放空间，能理解清理范围、退出应用是否有用、软件升级后的可用性与实际空间变化。产品优先级依据已有非零样本与边界可验证程度：微信日志约 289 MiB、钉钉两类缓存约 192 MiB、百度三类缓存约 17 MiB，仅为[上一轮单机记录](verification-netdisk-preparation.md)，不代表普遍收益。

公开[误删下载案例](https://www.reddit.com/r/WindowsHelp/comments/1069gxk/)和[查看临时文件明细的需求](https://www.reddit.com/r/WindowsHelp/comments/1cfqlff/)支持明确范围和保留明细；[Electron 文档](https://www.electronjs.org/docs/latest/api/session#sesclearcache)区分 HTTP 缓存与持久站点存储。以上是公开个案、技术资料与场景分析，不是本项目访谈结果，也不能代替私有应用真实验收。

## 首批范围

| 规则 | 固定范围 | 影响与保护 |
| --- | --- | --- |
| weixin.logs | Roaming / Tencent/xwechat/log，满 7 天 `.xlog` | 旧排障信息无法恢复；保留近期日志、映射缓冲、聊天记录与附件 |
| dingtalk.http | Local / DingTalk_数字/Default/Cache/Cache_Data | 网页资源可能重新下载；保留登录、持久站点数据、文档与离线资料 |
| dingtalk.code | 同配置 / Code Cache | 首次脚本加载可能变慢；同上，多个数字配置分别校验 |
| baidu_netdisk.http | Roaming / baidunetdisk/Cache/Cache_Data | 资源重新下载；保留完整下载、未完成任务、同步和离线资料 |
| baidu_netdisk.code | 同配置 / Code Cache | 脚本缓存可能重建；同上 |
| baidu_netdisk.gpu | 同配置 / GPUCache | 图形缓存可能重建；同上 |

缓存配置要求普通文件 Preferences 标记，仅读取其元数据。所有规则继续手动确认、默认不选；规则路径、文件期限、范围与原版一致。

## 资格与接口

`crates/core/src/cleanup_acceptance.rs` 的编译期 `ACCEPTANCES` 为空。记录结构绑定规则 ID、规则版本、布局 ID、准确应用文件版本、证据标识；观察样本版本不会增加清理版本。规则描述符开关和编译记录都通过后才具备清理资格，运行状态、身份与文件校验仍须单独通过。

`ApplicationRuleSupport.cleanup_versions` 是逐规则清理验收版本数组（当前全空），`cleanup_block` 是对应阻止原因。旧接口缺少信息时前端按能力未确认处理；旧 `cleanup_validated=false` 仍明确只读。应用汇总状态仅用于说明，不能替代某条规则的授权或阻塞原因。

准备计划与每次删除均检查后端资格。Windows 执行继续核对程序指纹、固定资料布局、文件身份/时间/大小、父链、排除、硬链接和云属性。普通构建没有合成授权字段或入口。

## 合成测试与记录工具

Rust 测试模块只在 `cfg(test)` 编入，内部创建带专用标记的 Temp 直接子目录；仅可授权本页六个固定规则，并检查目录身份及 local/roaming/system 映射。复用真实 Windows 扫描、候选验证、句柄删除和核心计划/报告流程。合成版本固定为 `0.0.0.1`，不得视为软件版本。

```powershell
pnpm test:acceptance
pnpm acceptance init artifacts/cleanup-preparation-template
pnpm acceptance synthetic artifacts/cleanup-preparation-synthetic
pnpm acceptance check artifacts/cleanup-preparation-synthetic
```

输出文件夹必须不存在。`synthetic` 仅运行指定 Rust 测试，记录六条候选/已选/删除/保留数量、逻辑字节、保护样本 SHA-256 与完整 FILETIME 字符串、单次计划及结果；同时记录源码哈希和规则目录标识。测试前后源码发生变化则拒绝生成。输出没有文件路径、账号、正文或真实登录信息。

每条记录另有 `real` 区域：默认 `not_run`，其他值为空。未来人工验收可复制模板为 `manual_record`，逐条填写操作系统版本、真实程序文件版本/哈希/签名结果、候选与删除计数、保护样本前后摘要、复启、功能和再生成结果，然后封装到另一个新目录：

```powershell
pnpm acceptance pack artifacts/review-record artifacts/manual-record.json
```

这里的再生成，对缓存指恢复正常使用后形成相应缓存，对微信日志指后续正常产生新日志，不要求恢复已删除的旧日志。保护摘要在应用退出后、删除前后采集，复启后的正常写入另作功能核对。样本摘要仅在专用测试资料上计算。

`check` 检查结构、范围、计数与一致性；即使人工填写为通过，也返回 `may_enable_cleanup=false`、`real_acceptance_review=not_performed`。哈希不证明来源真实性，工具不会改规则、写授权或执行真实应用清理。既有 v1/v2/v3 只读证据格式与目录不改写，继续使用原工具校验。

## 后续真实开放条件（本轮未执行）

在独立 Windows 环境使用测试账号，逐条取得真实非零候选；微信日志须自然达到 7 天，不通过修改真实文件时间制造合格样本。验证清理前后的保护资料、退出后的删除、复启、正常功能和后续生成情况。任何零样本、失败、未执行或资料变化均不作通过。

证据经独立审阅后，后续以源码变更加入指定规则版本与应用版本记录、开放对应描述符，并更新证据目录版本和发布矩阵。本轮只交付开发准备，不把工具输出、手工勾选或应用退出当成开放依据。

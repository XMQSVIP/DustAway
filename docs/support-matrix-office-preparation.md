# 办公通信软件扩展：当前源码支持矩阵

历史阶段记录：以下数量与结果对应上一轮办公通信扩展；当前源码见[网盘与下载工具矩阵](support-matrix-netdisk-preparation.md)。

本轮为 0.18.0 基础上的开发准备，未发布新免安装包。当前源码有 28 条普通用户扫描规则：14 条既有条件清理规则、14 条只读规则。已有发布目录仍以其随包说明为准。

| 范围 | 数量 | 扫描与默认选择 | 删除能力 |
| --- | --- | --- | --- |
| 用户临时文件及五种浏览器缓存 | 14 | 主动全面扫描，遵守长期排除；通过核验的建议项可默认选择 | 沿用现有条件清理 |
| 微信、经典 QQ、WPS | 6 | 身份和明确布局匹配后只读发现；版本作为观察信息 | 未验收，只读 |
| 飞书、钉钉 | 5 | 新增只读发现，不默认选择 | 未验收，只读 |
| CrashDumps、WER 归档、DirectX 缓存 | 3 | 沿用原有目录、期限和只读策略 | 未验收，只读 |

## 应用规则

| 规则 | Known Folder 与固定范围 | 条件与影响 |
| --- | --- | --- |
| weixin.logs | Roaming / Tencent/xwechat/log，`.xlog` | 满 7 天；历史排障信息不可重建 |
| weixin.dumps | Roaming / Tencent/xwechat/crashinfo，`.dmp` | 满 30 天；保留近期转储 |
| qq.http / qq.code / qq.gpu | Roaming / Tencent/QQ/webkitex_cache/数字配置，分别 Cache/Cache_Data、Code Cache、GPUCache | 需要普通文件 LocalPrefs.json；仅经典 QQ，QQNT 不适用 |
| wps.logs | Roaming / Kingsoft/office6/log，`.log` | 满 7 天；保留文档、备份和恢复文件 |
| feishu.http / feishu.code / feishu.gpu | Roaming / LarkShell/aha/users/直接账号目录/profile_main 或 profile_explorer，分别 Cache/Cache_Data、Code Cache、GPUCache | 需要普通文件 Preferences；多账号汇总；没有额外年龄限制 |
| dingtalk.http / dingtalk.code | Local / DingTalk_数字/Default，分别 Cache/Cache_Data、Code Cache | 需要普通文件 Preferences；数字后缀可变；没有额外年龄限制 |

缓存目录旁的 Local Storage、IndexedDB、Network、Session Storage、聊天附件、离线邮件、文档、备份均不纳入。只读取配置标记元数据，不解析账号资料或 Preferences 正文。缺少位置、布局不支持和访问失败分别记录；运行中允许只读检查，关闭应用不会解除只读能力。

身份识别使用注册表寻找候选、离线 Authenticode 发布者验证与固定安装结构。经典 QQ 要求 Bin/QQ.exe，不能把 QQNT 根目录的 QQ.exe 套入。飞书启动器与 app/Feishu.exe 分别验证并归并至同一安装，版本可以不同；两者都进入扫描身份快照。多个不同有效安装保持拒绝判定。

版本匹配且目录存在标记为 `verified_version`，其他版本匹配目录为 `recognized_layout`；这两者均只说明扫描依据。删除仍需静态逐规则验收、已支持执行版本、应用退出、后端计划与文件重验。本轮不开放任何应用规则的删除。

## 从编译规则生成证据目录

```powershell
pnpm applications:catalog
pnpm check:applications
node scripts/application-evidence.mjs catalog
node scripts/application-evidence.mjs collect artifacts/my-new-office-evidence
node scripts/application-evidence.mjs check artifacts/my-new-office-evidence
```

采集目录必须不存在；不会覆盖旧证据。当前注册表导出 5 个应用、11 条应用规则，生成 `scripts/application-catalog-v2.json` 与网页预览规则。旧 `application-catalog.json` 是冻结的 v1 校验目录，不再作为当前能力来源。

v2 证据包括版本、逐规则扫描依据、布局 ID、候选数量/逻辑字节、覆盖和阻止原因；不输出文件名、账号、个人路径、日志正文或原始错误。v1 包仍按冻结目录校验。两者的 `acceptance_status` 必须为 `not_run`，`deletion_performed` 必须为 false，校验器始终返回 `may_enable_cleanup=false`。清单哈希仅校验证据完整性，不是授权。

## 后续真实应用验收操作单（未执行）

在独立 Windows 测试环境使用测试账号，不复制日常资料。逐规则记录源码标识、系统版本、真实程序来源与哈希、签名、版本、目录结构、文件年龄、操作和结果。

以下每行均须单独记录结果，当前全部为“未执行”。先确认至少一个真实候选，不能把其他规则成功当成本行通过。

| 规则 | 生成与复测任务 | 必须保留和核对 |
| --- | --- | --- |
| weixin.logs | 正常收发后观察新 `.xlog`；仅对满 7 天样本安排验收，复启后产生新日志 | 聊天记录、附件、近期日志；不要求旧日志恢复 |
| weixin.dumps | 使用已有真实诊断样本核实满 30 天 `.dmp`，无样本则保持待执行；复启微信正常使用 | 近期转储、日志、聊天资料；不为验收故意使应用崩溃 |
| qq.http | 用经典 QQ 的网页功能形成资源缓存，处理后重新访问并检查缓存再生成 | 聊天媒体、账号、Cookie、Local Storage、IndexedDB |
| qq.code | 使用经典 QQ 网页功能形成脚本缓存，处理后首次加载和后续加载均正常 | 同上，且 `LocalPrefs.json` 不变 |
| qq.gpu | 使用经典 QQ 网页渲染功能形成图形缓存，处理后显示正常、缓存再生成 | 同上，检查聊天窗口与网页显示 |
| wps.logs | 打开并保存测试文档，核实满 7 天 `.log`；处理后应用可启动并产生新日志 | 原文档、备份、恢复文件、离线云文档、近期日志 |
| feishu.http | 两个测试账号分别使用主界面和网页窗口，按配置核对资源缓存，再访问形成新缓存 | 聊天附件、离线邮件、文档、登录和持久站点数据 |
| feishu.code | 两个账号及两种配置分别形成脚本缓存，复测首次加载、功能和再生成 | 同上，保留每个配置的 `Preferences` |
| feishu.gpu | 两个账号分别使用需渲染的页面，复测画面和图形缓存再生成 | 同上，观察显示异常，不把普通空目录算通过 |
| dingtalk.http | 标准数字后缀配置中访问网页功能，核对资源缓存及复启后再生成 | 聊天附件、离线资料、文档、登录和持久站点数据 |
| dingtalk.code | 已安装版本及升级后的数字配置分别形成脚本缓存，复测加载和再生成 | 同上，保留 `Preferences`，不碰旧配置中的保护资料 |

1. 每条缓存规则用真实应用生成至少一个非零候选；日志/转储需实际诊断样本。人为回填年龄须注明，不能当作自然积累样本。
2. 保存合成聊天附件、登录/站点数据、离线邮件、文档与备份的内容哈希和修改时间。运行中、重新启动、身份变化和未知进程状态必须拒绝清理。
3. 退出应用后，在隔离验收分支核对计划并执行真实清理；重新运行应用，验证缓存再生成或新日志产生。历史日志和转储不要求也不承诺重建。
4. 核对保护资料、近期文件、排除项、链接、云占位、父目录替换、文件变化、权限不足及中断结果；计划、实际文件与报告必须一致。
5. 记录失败和缺样本，不以零候选、模拟程序、只读证据或自动测试替代真实删除验收。逐规则证据审阅后，才在后续版本讨论静态能力开放。

QQNT 后续先取得签名程序和标准布局的独立测试样本，再形成专属规则；本轮没有 QQNT 删除路径或候选规则。Windows 三条只读规则沿用原验收要求。

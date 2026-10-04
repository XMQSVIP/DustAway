# 当前源码支持矩阵：网盘与下载工具准备

2026-10-01 更新：源码现为 35/16/19（用户规则/条件清理/只读），仅新增钉钉 `8.2.0.119` 网页与代码缓存许可，普通首页真实闭环待完成。以下是 09-29 的范围快照，网盘三款仍只读；最新证据见[首页清理验收](verification-home-cleanup.md)。

日期：2026-09-29；基于 0.18.0 的开发源码，未发布新包。共 35 条用户扫描规则：14 条既有条件清理、21 条只读。应用规则为 8 个应用、18 条，全部只读；另外三条 Windows 残留规则继续只读。系统规则不计入用户规则数。

扫描按可信签名、程序名、安装结构和固定数据布局识别；程序版本仅是观察信息。扫描依据 verified_version / recognized_layout 均不表示删除验收通过。新增规则采用 review、默认不选、两处静态能力关闭、无年龄限制。关闭应用或手工填写授权字段均不能解锁。

| 规则 | Known Folder 内固定范围 | 额外条件 |
| --- | --- | --- |
| weixin.logs | Roaming / Tencent/xwechat/log | .xlog，满 7 天 |
| weixin.dumps | Roaming / Tencent/xwechat/crashinfo | .dmp，满 30 天 |
| qq.http | Roaming / Tencent/QQ/webkitex_cache/数字目录/Cache/Cache_Data | 普通文件 LocalPrefs.json |
| qq.code | 同配置 / Code Cache | 同上，仅经典 QQ |
| qq.gpu | 同配置 / GPUCache | 同上，不套用 QQNT |
| wps.logs | Roaming / Kingsoft/office6/log | .log，满 7 天 |
| feishu.http | Roaming / LarkShell/aha/users/直接账号/profile_main 或 profile_explorer/Cache/Cache_Data | 普通文件 Preferences |
| feishu.code | 同配置 / Code Cache | 同上，多账号汇总 |
| feishu.gpu | 同配置 / GPUCache | 同上 |
| dingtalk.http | Local / DingTalk_数字/Default/Cache/Cache_Data | 普通文件 Preferences |
| dingtalk.code | 同配置 / Code Cache | 同上，数字后缀可变 |
| baidu_netdisk.http | Roaming / baidunetdisk/Cache/Cache_Data | 配置根普通文件 Preferences |
| baidu_netdisk.code | Roaming / baidunetdisk/Code Cache | 同上 |
| baidu_netdisk.gpu | Roaming / baidunetdisk/GPUCache | 同上 |
| quark_netdisk.code | Roaming / quark-cloud-drive/Code Cache | 配置根普通文件 Preferences |
| quark_netdisk.gpu | Roaming / quark-cloud-drive/GPUCache | 同上 |
| thunder.code | Roaming / thunder/Code Cache | 配置根普通文件 Preferences |
| thunder.gpu | Roaming / thunder/GPUCache | 同上 |

不读取标记正文。账号、缓存叶目录及父链沿用路径边界、文件身份、重解析点、云属性和排除检查。百度固定观察版本为 8.8.3.101（注册表 DisplayVersion 曾仍为 8.3.9，不作为依据），夸克为 3.19.0.0，迅雷为 12.4.10.3940。迅雷要求 Program/Thunder.exe；发布者必须与封闭注册表相符。签名失败或两个有效安装冲突均不建立可信身份。飞书启动器和内部程序保持同一安装归并，QQNT 仍单独待采样。

夸克、迅雷没有已核实的 Cache/Cache_Data，本轮不增加其网页缓存规则。下载文件、.xltd、任务数据库、Cookie、Local Storage、IndexedDB、Network、Session Storage、Storage、同步/离线资料、文档与备份都不进入新增候选。用途说明仅是路径线索，不推断任务状态、账号归属、云副本或删除权限。

## 证据格式与兼容

运行 pnpm applications:catalog 导出当前编译规则；pnpm check:applications 检查投影。v3 的 catalog_id 为编译目录规范 JSON 的 SHA-256，采集器与导出器共享实现。当前 scripts/application-catalog-v3.json 覆盖 18 条；冻结 v1 的 6 条和 v2 的 11 条目录分别保留，不能用当前规则重写历史目录。

```powershell
node scripts/application-evidence.mjs collect artifacts/my-new-netdisk-evidence
node scripts/application-evidence.mjs check artifacts/my-new-netdisk-evidence
```

输出目录必须不存在。校验只信任工具自带的已知目录；未知 catalog_id、重复/遗漏规则、跨格式多余字段和矛盾能力声明均拒绝。证据包不携带可自授权的目录，不输出文件路径、账号、数据库正文或原始错误。所有版本固定 deletion_performed=false、acceptance_status=not_run、may_enable_cleanup=false。哈希是完整性校验，不是授权或第三方签名。把一份材料人为重写成全部字段自洽的旧格式不能证明其来源，校验结果也永远不能开放删除。

## 后续逐规则真实验收操作单（全部未执行）

既有 11 条应用规则按[办公通信操作单](support-matrix-office-preparation.md#后续真实应用验收操作单未执行)逐条执行；三条 Windows 只读规则保留原验收要求。本轮不建设隔离环境，也不执行下列真实删除任务。

| 新规则 | 真实候选与复启任务 | 必须核对的保护资料 |
| --- | --- | --- |
| baidu_netdisk.http | 在专用测试账号使用页面功能；真实资源候选至少 1 个，清理后页面正常且资源再生成 | 下载/同步文件、离线资料、任务、登录、持久存储 |
| baidu_netdisk.code | 使用脚本界面；候选至少 1 个，清理后首次和再次加载正常、代码缓存再生成 | 同上，Preferences 不变 |
| baidu_netdisk.gpu | 使用界面渲染；候选至少 1 个，清理后显示正常、图形缓存再生成 | 同上 |
| quark_netdisk.code | 本机为空，先取得真实候选；随后独立复测加载及代码缓存再生成 | 下载、备份/离线资料、任务、登录和持久存储 |
| quark_netdisk.gpu | 本机为空，先取得真实候选；随后独立复测渲染及图形缓存再生成 | 同上，Preferences 不变 |
| thunder.code | 本机为空，先取得真实候选；随后独立复测加载及代码缓存再生成 | 完整文件、未完成下载、.xltd/.cfg、任务库、登录、持久存储 |
| thunder.gpu | 本机为空，先取得真实候选；随后独立复测显示及图形缓存再生成 | 同上，Preferences 不变 |

逐行记录系统、签名、程序版本/哈希、规则/目录标识、至少一个真实候选、清理前后保护样本哈希与时间、复启功能、缓存再生成、失败和未执行项。零候选与人工写入夹具不能代替真实缓存验收；日志/转储不可承诺恢复历史内容。运行中、身份/父路径变化、长期排除、链接/云文件、访问失败与中断均须核对。只有后续独立验收与证据审阅完成，才讨论静态删除开放。

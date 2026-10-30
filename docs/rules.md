# 清理规则维护指南

2026-10-25 识别扩展：`application-cache.json` 增加 `folder=discovered`，`paths` 为磁盘根下的产品数据目录种子，`base` 为稳定布局标识。Windows 的 `discovered_cache` 根据所选卷身份筛选位置，再结合安装登记、产品文件或升级清单生成封闭扫描根。当前仅支持钉钉升级包、自定义日志、GameViewer 日志、HBuilderX 升级备份；所有新规则只读。不得将此类型交给普通 Known Folder 路径拼接。详见[识别验收](verification-discovery-20261025.md)。

0.19.0：`windows.user_temp` / `windows.system_temp` 均为规则版本 2、`min_age_days=1`，按连续 86,400 秒计算，扫描、准备、删除一致复核；安装版提供系统清理。新增 `windows.iis_logs` 标准目录历史日志，保留最近 30 天且默认不选。保留观察记录与删除候选分离，详情见 [本轮验收](system-cleanup-acceptance.md)。下面 0.9 基线及早期开发说明为历史范围。

当前开发源码提供 72 条用户扫描规则：34 条按条件逐文件清理、1 条 uv 官方整理、37 条只读。18 条应用规则中，仅 `dingtalk.http`、`dingtalk.code` 对固定文件版本 `8.2.0.119` 各有独立真实验收许可；默认不选，普通首页真实闭环仍待完成。其余应用规则和版本继续受限，证据见[当前验收](verification-home-cleanup.md)。主动全面扫描与清理默认选择是两个概念。系统规则只供实验构建。

规则源文件是 [rules/builtin.toml](../rules/builtin.toml)，由 Rust 编译进程序。修改磁盘上的 TOML 不会热更新已发布的应用；首版不加载在线规则、外部规则包或用户删除脚本。

## 0.9 基线规则（后续扩展见当前支持矩阵）

| ID                    | 根类型        | 限定内容                            | 保留条件                   | 默认选择 |
| --------------------- | ------------- | ----------------------------------- | -------------------------- | -------- |
| `windows.user_temp`   | `user_temp`   | 当前用户标准 LocalAppData 下的 Temp | 修改至少满 7 天            | 是       |
| `windows.system_temp` | `system_temp` | Windows 目录下的 Temp               | 修改至少满 7 天，独立提权  | 否       |
| `edge.http`           | `edge`        | 标准配置的 `Cache/Cache_Data`       | Edge 退出并通过文件验证    | 是       |
| `edge.code`           | `edge`        | 标准配置的 `Code Cache`             | 同上                       | 是       |
| `edge.gpu`            | `edge`        | 标准配置的 `GPUCache`               | 同上                       | 是       |
| `chrome.http`         | `chrome`      | 标准配置的 `Cache/Cache_Data`       | Chrome 退出并通过文件验证  | 是       |
| `chrome.code`         | `chrome`      | 标准配置的 `Code Cache`             | 同上                       | 是       |
| `chrome.gpu`          | `chrome`      | 标准配置的 `GPUCache`               | 同上                       | 是       |
| `firefox.http`        | `firefox`     | 标准配置对应的本地 `cache2`         | Firefox 退出并通过文件验证 | 是       |

缓存规则的 `min_age_days` 为 0，不代表可以跳过进程、文件身份、链接和权限检查。

## TOML 字段

以下为现有用户临时文件规则的结构示例，不是供用户导入的配置：

```toml
[[rule]]
id = "windows.user_temp"
version = 2
title = "用户临时文件"
description = "当前用户标准临时目录中至少 24 小时未修改的文件"
impact = "删除较早的临时文件；正在使用或最近修改的文件会保留。"
root = "user_temp"
relative = ""
min_age_days = 1
requires_admin = false
default_selected = true
```

| 字段               | 类型             | 含义                                          |
| ------------------ | ---------------- | --------------------------------------------- |
| `id`               | string           | 唯一稳定标识；前端选择、排除和报告引用此 ID   |
| `version`          | 正整数           | 规则修订版本，影响范围或匹配语义改变时递增    |
| `title`            | string           | 类别标题                                      |
| `description`      | string           | 识别依据和范围                                |
| `impact`           | string           | 删除影响，进入确认计划                        |
| `root`             | 固定枚举字符串   | 交给 Windows 适配层解析的根类型，不是任意路径 |
| `relative`         | 固定白名单字符串 | 浏览器配置内的缓存子目录                      |
| `min_age_days`     | 非负整数         | 按修改时间判断的最短保留天数                  |
| `process`          | 可选 string      | 对应进程名，如 `chrome.exe`                   |
| `requires_admin`   | boolean          | 是否属于系统权限域                            |
| `default_selected` | boolean          | 新用户默认是否选择                            |

新增可选字段 `app_id` 表示固定应用归属，旧规则省略时为 null；`data_kind` 为 cache 或 diagnostics，旧规则省略时为 cache。类别不代表删除许可。

当前解析器拒绝未知字段，校验 ID 唯一、已知根类型、相对目录白名单、权限域匹配和有效版本。它不是任意根/相对路径组合的完整语义验证器；组合是否正确仍须平台实现和测试验证。不能只添加一行 TOML 就认为新增清理范围安全。

## 根目录识别

### 临时目录

用户目录通过 Known Folder API 取得 LocalAppData，然后限定 `Temp`；不根据任意环境变量 TMP/TEMP 扩大删除范围。系统目录通过 WindowsDirectory API 解析，然后限定 `Temp`。

### Chromium 浏览器

Chrome、Edge、Brave 和 Vivaldi 分别检查 LocalAppData 下的标准产品目录。配置名称仅识别 `Default` 或 `Profile ` 后接数字的形式，并要求可验证的 `Preferences` 文件。

匹配终点是明确的 HTTP、Code 或 GPU 缓存子目录，不能把整个配置目录作为候选范围。Bookmarks、History、Cookies、Login Data、Sessions 等不属于这些规则。

### Firefox

读取 RoamingAppData 下标准 Firefox 的 `profiles.ini`，文件需可验证且小于 1 MiB。仅接受 Profile 段中的 `IsRelative=1` 和 `Profiles/<单一目录名>` 布局，要求对应 `prefs.js`，随后映射到 LocalAppData 中同名配置的 `cache2`。

绝对自定义路径、父目录跳转、多级额外路径和缺失标记的配置不猜测。首版实现对不支持配置的提示可能是类别摘要，而非为每个异常配置创建独立错误项。

## 永远适用的执行约束

规则不能覆写固定 NTFS、根目录归属、重解析点、云占位、多硬链接、文件占用、父链身份与时间等检查。执行前还会重新派生允许根目录并验证扫描快照。

文件年龄当前用 Unix 秒比较：`modified_unix <= now - min_age_days * 86400`。边界测试应使用明确的时间差并考虑秒级表示，不能只把测试文件命名为“旧文件”。

只有扫描保存的文件会进入计划；新增文件不会自动加入。执行逐文件进行，不删除整个目录树，不强制修改属性、ACL 或所有权。

## 修改流程

1. 明确数据类别与删除影响：缓存需要证实可以重新生成；日志/转储是诊断资料，删除会失去过去的排障依据。记录权威路径依据及不应触及的相邻数据。
2. 在专用目录或可还原 VM 中构建标准、多配置、缺失、异常、自定义路径和受保护数据夹具。
3. 修改 TOML；需要新增根类型时，同时实现 `rules.rs` 的限制和 Windows 路径解析。
4. 调整受影响规则版本，更新说明与影响文案。
5. 运行 `pnpm applications:catalog` 从编译规则生成 `src/preview-rules.json` 与 v3 应用证据目录及 catalog_id（保留冻结 v1/v2 目录），再执行 `pnpm check:applications`。预览不是桌面删除的授权来源。
6. 更新本文件、需求范围和必要的用户说明。
7. 运行核心、Windows 和受影响界面测试，再在 VM 中验证真实产品布局。
8. 审核许可来源后随新程序版本发布，不把网上找到的规则直接复制入库。

## 回归重点

| 场景                              | 必须保持的结果                           |
| --------------------------------- | ---------------------------------------- |
| 标准多配置                        | 只识别对应的缓存子目录                   |
| 应用未安装或目录缺失              | 返回空结果或清晰说明，不扫描相似目录替代 |
| 非标准配置                        | 不越出首版识别范围                       |
| 受保护的相邻个人数据              | 内容与身份保持不变                       |
| 年龄边界、未来时间                | 按保留条件处理，不用文件名判断           |
| 排除文件、目录、整条规则          | 不进入最终计划或在执行时被拒绝           |
| Junction、硬链接、替换父目录/文件 | 不跟随链接，不能凭旧路径继续删除         |
| 浏览器重启、文件占用              | 跳过且可解释，不结束应用                 |

当前测试文件位于 `crates/core/tests/engine.rs`、`crates/windows/tests/filesystem.rs`，浏览器目录测试位于 `crates/windows/src/platform.rs` 的测试模块。

## 0.6 新增规则与注册表

| ID | 产品根目录（相对 LocalAppData） | 缓存叶目录 | 进程 | 默认 |
|---|---|---|---|---|
| brave.http / code / gpu | BraveSoftware/Brave-Browser/User Data | Cache/Cache_Data / Code Cache / GPUCache | brave.exe | 关闭 |
| vivaldi.http / code / gpu | Vivaldi/User Data | Cache/Cache_Data / Code Cache / GPUCache | vivaldi.exe | 关闭 |

规则年龄为 0 天；仍需通过全部占用、属性、文件身份和父链校验。`crates/core/src/browsers.rs` 是 Chromium 产品路径、进程名、标准配置命名和允许缓存子目录的共同来源。`rules::validate` 验证 ID/根/相对目录/进程/年龄/权限组合；Windows 再次验证后才派生路径。空间分析复用注册表，只有与所选分析目录相关的产品才查找元数据标记。

不清理 Service Worker、IndexedDB、Local Storage、扩展、钱包、Rewards、笔记、邮件和缩略图个性化数据。标准配置标记只读取元数据，不解析密码、聊天或文档内容。

## 0.9 应用规则

六条规则的历史精确目录、版本、保留期与逐项状态见 [0.9 支持矩阵](support-matrix-0.9.md)。当前 `crates/core/src/applications.rs` 固定应用身份、观察版本、扩展名、年龄；`cleanup_acceptance.rs` 单独记录逐规则真实验收版本与证据。TOML 必须与固定范围一致；不增加可从前端或配置传入的任意路径/扩展名。

RoamingAppData 经 Known Folder API 解析。微信只接受 `.xlog` / `.dmp`，WPS 只接受 `.log`；QQ 只接受数字配置且有可验证的 `LocalPrefs.json`，限定三个缓存叶目录。目录或文件名不单独构成授权。日志正文、聊天和文档均不读取。

程序版本从已验证主程序读取，注册表仅寻找位置。离线签名与发布者验证结果按文件身份、时间戳和路径在进程内缓存。扫描时记录应用身份；计划核对版本、标准根、标记、文件与父链，逐文件删除再次验证。应用升级或路径被替换须重扫。

应用运行允许只读扫描；主程序、同会话的已知组件或安装目录中的后台程序会阻止计划。无法查询同会话进程镜像时不能推断应用已退出。没有联网探测、后台服务或结束进程操作。

当时六条 `validated` 为 false；当前该能力由编译期真实验收记录派生，记录表为空。引擎阻止有效计划，Windows 删除边界也独立拒绝；改变前端勾选、退出应用或伪造快照均不能解除。只有逐规则完成隔离真实应用验收、记录证据并审阅后，才可在后续源码发行中同时开放描述符与对应验收记录。合成夹具授权仅位于 `cfg(test)` 内，不编入普通库或桌面程序，见[当前操作单](support-matrix-cleanup-preparation.md)。

## 0.17 策略与 Windows 残留

扫描范围独立于 default_selected。新增 cleanup_policy 控制推荐选择，cleanup_validated 控制本版本删除能力；详见[支持矩阵](support-matrix-0.17.md)。新增规则同时更新内置TOML、核心身份验证、Windows固定根与文件匹配、预览目录及测试。不得只改TOML布尔值绕过发布门槛。

## 上一轮办公软件只读扩展（历史阶段）

应用注册表增加 Known Folder 根类型、封闭的配置目录选择方式和布局 ID。微信、经典 QQ、WPS、飞书、钉钉当时共11条应用规则，静态验收均为false。只读扫描不要求单一版本相等，但必须验证应用签名与安装结构、明确目录及标记；删除仍保留版本与逐规则验收门槛。规则、目录、年龄与策略不得通过配置或前端扩展。

新增五条的详细边界、生成命令、v1兼容和真实验收操作单见[该阶段支持矩阵](support-matrix-office-preparation.md)。当前数量与范围以本文页首矩阵为准。

当前新增网盘/下载规则仅允许 Roaming 的固定配置根＋普通文件 Preferences＋明确缓存子目录，详见[支持矩阵](support-matrix-netdisk-preparation.md)。所有 18 条应用规则的注册表删除开关继续关闭；只读发现和用途说明不能产生删除权限。

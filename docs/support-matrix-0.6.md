# 0.6 应用支持矩阵

| 应用 | 本轮能力 | 数据范围 | 默认选择 | 支持布局 |
|---|---|---|---|---|
| 用户临时文件 | 直接清理 | 至少 7 天未修改且通过安全检查的文件 | 开启 | 当前用户 LocalAppData/Temp |
| Edge、Chrome | 直接清理 | 网页、代码、图形缓存 | 开启 | 稳定版标准 Default / Profile N |
| Firefox | 直接清理 | cache2 网页缓存 | 开启 | 标准相对 profiles.ini 配置 |
| Brave | 新增直接清理 | Cache/Cache_Data、Code Cache、GPUCache | 关闭 | LocalAppData/BraveSoftware/Brave-Browser/User Data 下标准配置 |
| Vivaldi | 新增直接清理 | Cache/Cache_Data、Code Cache、GPUCache | 关闭 | LocalAppData/Vivaldi/User Data 下标准配置 |
| 微信 3.x / 4.x | 分析与通用指引 | 不授权删除应用资料 | 无删除选项 | 指引菜单未逐版本核对 |
| QQ 经典版 / QQNT | 分析与通用指引 | 不授权删除聊天图片、视频、记录、表情 | 无删除选项 | 指引菜单未逐版本核对 |
| WPS Windows 桌面版 | 分析与通用指引 | 不授权删除备份、恢复数据、离线文档、插件 | 无删除选项 | 指引菜单未逐版本核对 |

所有直接清理均要求本地固定 NTFS 卷，当前用户范围，应用进程状态可以确认且已退出，逐文件通过身份与属性检查；不跟随重解析点，不清理多硬链接或云占位文件。便携版、测试渠道、自定义目录和不支持布局不根据名称猜测。

## Brave / Vivaldi 保护范围

密码、Cookies、历史、会话、标签页、扩展、Local Storage、IndexedDB、Service Worker、钱包、Rewards、笔记、邮件及个性化缩略图均不纳入规则。新增规则默认关闭，旧版偏好不会自动扩大选择。

## 验证证据

真实浏览器脚本：`scripts/browser-validation.mjs`。使用官方稳定版程序、独立临时用户目录与本地测试网站，不登录真实账号，不改系统默认浏览器，不清理日常资料目录。验证后台运行时拒绝计划、六类缓存实际生成/删除/重建、清理前后非目标文件内容和时间不变，以及重启后测试 Cookie 登录状态、Local Storage 和书签保留。

2026-09-26 已在 Windows 10 22H2 上通过以下真实应用验收（官方稳定版，独立配置，headless 模式）：

| 应用版本 | 删除缓存文件 | 清理逻辑字节 | 重启后重建文件 | 内容与修改时间保持不变的非目标文件 |
|---|---:|---:|---:|---:|
| Brave 1.96.59 / Chromium 154.0.8037.58 | 18 | 3,150,980 | 16 | 232 |
| Vivaldi 8.2.4133.76 | 25 | 9,414,039 | 17 | 472 |

两者均验证三类缓存实际生成和重建、运行中拒绝生成清理计划、重启后测试 Cookie、Local Storage 与书签保留。文件数量不是固定产品特征，随使用内容变化；删除逻辑字节不等于磁盘可用空间增量。

原始证据：[浏览器验收](../artifacts/browser-validation-0.6/evidence.json)、[程序版本、签名与哈希](../artifacts/browser-validation-0.6/binaries.json)、[本轮完整验收](verification.md)。Brave ZIP 同时核对官方 SHA-256；两款程序的 Authenticode 签名有效。没有用日常配置进行验收删除。合成钱包/邮件等边界测试不等于对真实钱包资产或邮件账号进行端到端验收；Windows 11、完整交互式浏览器功能和启停竞态矩阵尚未执行。

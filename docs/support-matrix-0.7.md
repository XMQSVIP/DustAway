# 0.7 应用支持矩阵

| 应用 | 本轮能力 | 数据范围 | 默认选择 | 支持布局 |
|---|---|---|---|---|
| 用户临时文件 | 直接清理 | 至少 7 天未修改且通过安全检查的文件 | 开启 | 当前用户 LocalAppData/Temp |
| Edge、Chrome | 直接清理 | 网页、代码、图形缓存 | 开启 | 稳定版标准 Default / Profile N |
| Firefox | 直接清理 | cache2 网页缓存 | 开启 | 标准相对 profiles.ini 配置 |
| Brave | 直接清理 | Cache/Cache_Data、Code Cache、GPUCache | 关闭 | LocalAppData/BraveSoftware/Brave-Browser/User Data 下标准配置 |
| Vivaldi | 直接清理 | Cache/Cache_Data、Code Cache、GPUCache | 关闭 | LocalAppData/Vivaldi/User Data 下标准配置 |
| 微信 3.x / 4.x | 分析与通用指引 | 不授权删除应用资料 | 无删除选项 | 指引菜单未逐版本核对 |
| QQ 经典版 / QQNT | 分析与通用指引 | 不授权删除聊天图片、视频、记录、表情 | 无删除选项 | 指引菜单未逐版本核对 |
| WPS Windows 桌面版 | 分析与通用指引 | 不授权删除备份、恢复数据、离线文档、插件 | 无删除选项 | 指引菜单未逐版本核对 |

所有直接清理均要求本地固定 NTFS 卷，当前用户范围，应用进程状态可以确认且已退出，逐文件通过身份与属性检查；不跟随重解析点，不清理多硬链接或云占位文件。便携版、测试渠道、自定义目录和不支持布局不根据名称猜测。

## Brave / Vivaldi 保护范围

密码、Cookies、历史、会话、标签页、扩展、Local Storage、IndexedDB、Service Worker、钱包、Rewards、笔记、邮件及个性化缩略图均不纳入规则。Brave / Vivaldi 六条规则仍默认关闭，旧版偏好不会自动扩大选择。

## 0.7 的扫描与清理边界

14 条规则和默认选择沿用 0.6。临时文件在七天内修改时可提前保留；其他候选仍完整核验，元数据本身不授权删除。读取部分受支持位置失败时显示不完整，允许确认清理已验证的候选；全部读取失败、规则无法验证、取消或应用状态未知时阻止相关清理。

当前应用状态检查不能消除原有读取问题，必须主动重扫才能重新确定完整程度。缺少标准配置与配置读取失败分别呈现。微信、QQ、WPS 仍仅提供只读分析及通用指引。

## 验证证据

所有实际执行、准确浏览器版本、缓存重建和保护数据检查见 [本轮验收](verification.md)。0.6 的原始版本和签名证据保留于 [浏览器程序清单](../artifacts/browser-validation-0.6/binaries.json)，0.7 的运行结果单独记录，不覆盖旧证据。未使用日常浏览器资料进行验收删除。测试 Cookie 登录状态、Local Storage、书签及非目标文件的检查不等于真实钱包资产、邮件账号或全部扩展功能的验证。

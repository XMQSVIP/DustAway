# 国内办公通信与剪映：用途规则核实记录

核对日期：2026-09-28。规则修订 `2026-09-28.4`，本轮新增 25 条规则、35 个主匹配项，总计 246 条、483 个主匹配项。本记录说明证据能支持到哪里，不是全版本认证或删除验收。

后续调整（修订 `2026-09-28.5`）：按用户要求，核实版本仅用于记录样本，不作为用途识别门槛。界面文案已去掉本机版本号；钉钉的 `_133` 规则推广为 `_{version}` 数字后缀模式，相同结构可跨版本匹配。下文的具体版本和原始验证范围仍保留为历史证据，不代表当前仅允许这些版本。

## 核实方式

开发阶段查阅公开资料，并对本机安装程序和目录做只读核对。读取了程序版本、目录名、文件类型统计及微信保存数据根目录的短配置；没有读取聊天数据库、消息、附件或日志正文，没有解密、上传、清理、安装或启动第三方工具。账号目录在记录中统一表示为 `<account>`，不保存真实账号或文档名。

证据分三类：厂商/底层项目的功能文档；原始项目实现或使用单位的部署记录；本机程序版本与目录样本。下面逐项标明。规则的 `source` 是公开参考链接，**并非每个链接都声明了对应应用的全部私有路径**；本机路径证据在本记录列出。厂商产品下载页仅用于标识产品，不作为路径证明。

识别仍只依赖扫描中的路径与文件/目录类型，不读取应用配置，不检测安装版本，也不联网。本轮开发核实不会成为用户运行时的额外步骤。

## 本机版本和路径证据

| 产品 | 实际观察 | 范围限制 |
| --- | --- | --- |
| 微信 | `Weixin.exe` ProductVersion 为 `4.1.13.65`，Company 为 Tencent；卸载项仍写 `4.0.6.17` | 以程序版本描述本机样本，不能用过时卸载项宣布验证了旧版 |
| 经典 QQ | `QQ.exe` ProductVersion 为 `9.7.23.29394`，Company 为 Tencent | 不扩展为 QQNT 全版本支持 |
| 飞书 | 根启动程序 `Feishu.exe` 为 `7.64.6`，`app/Feishu.exe` 为 `7.64.0.56`；另有 `7.69.9` 更新目录 | 保留启动器与内部程序的差异，没有运行更新，也未验证更新目录版本 |
| 钉钉 | `DingTalk.exe` 为 `8.5.5.260903002`，Company 为 Alibaba Group.；进程参数明确使用 `%LOCALAPPDATA%/DingTalk_133` | `_133` 为实测样本；当前按结构推广到数字后缀，不要求精确版本一致 |
| 企业微信、剪映 | 本轮没有本机程序样本 | 使用下面列出的部署记录、原始项目文档与实现，不能称为本机实测 |

路径观察是存在性和结构核对，不证明所有目录都由当前版本创建；长期安装环境可能保留旧版本资料。

### 微信：9 条

规则：`app.weixin_data`、`weixin_database`、`weixin_attachments`、`weixin_files`、`weixin_video`、`weixin_profile`、`weixin_logs`、`weixin_crash`、`weixin_plugins`（后八项同样带 `app.` 前缀）。

- [wx-cli-again Windows 配置定位源码（固定提交）](https://github.com/jackwener/wx-cli-again/blob/077a54cbfe679bda963cd038d8440422907fc797/src/config.rs) 将 `Tencent/xwechat/config` 中保存的位置与 `xwechat_files/<account>/db_storage` 关联；这是第三方原始实现，不是腾讯承诺的路径契约。本轮仅阅读源码，没有使用它的提取或解密功能。
- 按该定位方式只读取本机不足 1 KiB 的 INI 数据根目录值，实际找到 `xwechat_files`。账号下观察到 `db_storage`，其子目录包括 `message`、`contact`、`session`、`favorite` 等；`msg` 下观察到 `attach`、`file`、`video`，部分另有 `migrate`。后者仅归入资料根规则，不猜测是否可删除。
- [同项目附件说明](https://github.com/jackwener/wx-cli-again/blob/077a54cbfe679bda963cd038d8440422907fc797/README.md) 将 `msg/attach` 下的 DAT 与图片附件关联。因此描述保留图片/附件用途，不按未知二进制或可重建缓存处理。
- 本机 `%APPDATA%/Tencent/xwechat` 下有 `log`、`crashinfo`、`XPlugin/plugins` 等目录。日志根中观察到 `.xlog`、`.mmap3`、`.ini`；不将后两类混同为可清理日志。插件中存在小程序运行环境及备份程序的可执行文件。
- [Tencent Mars 的日志说明](https://github.com/Tencent/mars/wiki/Mars-%E8%87%AA%E5%AE%9A%E4%B9%89%E6%89%A9%E5%B1%95) 仅补充日志机制背景，具体微信目录来自本机元数据与既有 [0.9 样本](support-matrix-0.9.md)。

`xwechat_files` 保持名称时可以识别迁移后的盘符与父目录。目录被完全重命名、微信 3.x 旧布局、硬链接关系和迁移完成状态不在新增规则的验证范围。

### 经典 QQ：4 条

规则：`app.qq_data`、`app.qq_profile`、`app.qq_web_profile`、`app.qq_web_cache`。

本机文档目录中的 `Tencent Files/<account>` 下观察到 `Video`、`MyCollection`、`GroupCollection` 等；`%APPDATA%/Tencent/QQ` 下观察到组件、配置和 `webkitex_cache`。后者的三个缓存子目录已有 [0.9 版本化样本与范围说明](support-matrix-0.9.md)。

资料根、内嵌网页资料、具体缓存子目录分别说明。`Cache/Cache_Data`、`Code Cache`、`GPUCache` 需要 `AppData/Roaming/Tencent/QQ/webkitex_cache` 祖先条件。[CEF 原始定义](https://github.com/chromiumembedded/cef/blob/master/include/internal/cef_types.h) 说明网页缓存根还能承载跨会话的 localStorage 等数据，因此整个网页资料根不会标为纯缓存。

本轮不把仅观察到残留目录的 QQNT 声称为已验证，也未加入自定义且完全重命名的聊天根路径。用途识别不替代现有清理规则的版本、标记文件和只读限制。

### 飞书：3 条

规则：`app.feishu_profile`、`app.feishu_logs`、`app.feishu_web_cache`。

本机 `%APPDATA%/LarkShell` 中观察到 `aha/users`、`sdk_storage`、账号状态及资源目录。账号下的 `profile_main` 和 `profile_explorer` 均具有 `Cache`、`Code Cache`、`GPUCache`，也有 `IndexedDB`、`Local Storage` 等持久数据。日志位于 `sdk_storage/log`，其下有 `xlog`、`ttnet`、`monitor` 等。

[飞书官方邮件缓存说明](https://www.feishu.cn/hc/zh-CN/articles/096022810836) 确认离线邮件会占用本机空间且由客户端管理，但没有声明 LarkShell 内哪一个文件是邮件数据库。因此只给资料根、明确日志区域和网页缓存子目录用途，不猜测邮箱数据库、附件下载完成状态或云端是否还有副本。网页存储语义另参照 CEF 原始定义；路径来自本机样本。

### 钉钉：5 条

规则：`app.dingtalk_web_profile`、`app.dingtalk_web_cache`、`app.dingtalk_diagnostics`、`app.dingtalk_live`、`app.dingtalk_live_logs`。

本机钉钉进程的 `--user-data-dir` 明确指向 `%LOCALAPPDATA%/DingTalk_133`。该目录 `Default` 下实际存在 `Cache`、`Code Cache`，同时存在 `IndexedDB`、`Local Storage`、`Network` 等；后几类保留为网页资料，不归入具体缓存规则。

另观察到 `%LOCALAPPDATA%/DingTalk/dumps`、`DingTalkReal/crashes`，其中前者当前为空，后者有 JSON 元数据；仅称故障诊断目录，不声称有可释放的大型转储。`%APPDATA%/dinglive` 下观察到 `cef_cache_133`、`crashes`、`file_cache`、`logs`、`model`、`portrait`；日志目录有 TXT 文件。对直播组件整体只做保守的资料用途提示，未读取文件以判定录制内容，也未验证具体文件由哪个组件版本写入。

[钉钉产品页](https://www.dingtalk.com/download) 用于产品参考；上述路径来自本机观察，网页持久存储语义参照 CEF 原始定义。没有把任意 `ding_cache`、`dingcache`、数据库或整个直播目录加入清理范围。

### 企业微信：2 条

规则：`app.wecom_data`、`app.wecom_received`。

[北京农业职业学院信息中心的企业微信部署说明（2024-01-02）](https://www.bvca.edu.cn/xxzx/info/1181/1005.htm) 给出 `Documents/WXWork/…/Cache` 存放接收文件、图片和语音的布局，并说明可以在应用内更改保存位置。这是实际使用单位发布的说明，**不是厂商全版本说明**，也没有具体客户端构建号。

因此规则只覆盖 `documents/wxwork` 目录片段及其下 `Cache`，使用“可能”措辞并提醒保留工作附件；不将该 Cache 等同于可重建网页缓存。自定义位置不含 `Documents/WXWork` 时可能不命中；未知版本、微盘、邮件及更细数据库结构仍待样本核对。

### 剪映专业版：2 条

规则：`app.jianying_drafts`、`app.jianying_draft_file`，分类为设计/媒体。

[JyDraft 原项目说明（固定提交）](https://github.com/HTWMedia/JyDraft/blob/94249b21a71a2526e47e04c90e08d1515bac7c77/README.md) 列出 Windows 草稿位置 `JianyingPro/User Data/Projects/com.lveditor.draft/<project>/draft_content.json`。这里只用于核对工程路径，没有使用项目的云端服务、API Key 或上传功能。

[pyJianYingDraft 原始草稿实现（固定提交）](https://github.com/GuanYixuan/pyJianYingDraft/blob/6bb2d06a713f7b58e1375269daec835a03882086/pyJianYingDraft/draft_folder.py) 创建 `draft_meta_info.json` 与 `draft_content.json`；[示例](https://github.com/GuanYixuan/pyJianYingDraft/blob/6bb2d06a713f7b58e1375269daec835a03882086/demo.py) 引用外部音视频素材。由此保留草稿和原素材的依赖提醒，不将工程视为导出视频或临时缓存。

没有本机剪映样本；不能声称核验了所有剪映版本。完整文件名规则可以提示自定义草稿位置，但同名普通文件仍可能误匹配，说明保留“剪映或兼容工具”“单靠文件名不能确认”。代理、预渲染、自动恢复、素材下载目录仍未确认，不猜测加入。

## 验证与边界

- 所有 483 个主匹配项及 `within` 条件通过可达性检查。
- 新增测试覆盖资料与缓存优先级、DAT 附件、数据库、跨盘位置、相似目录名、数字版本后缀、普通同名文件/目录，以及 `.git` 等现有保护优先级。
- `within` 的目录条件现在必须位于主匹配目录之前，避免 `D:/Cache/Documents/WXWork/...` 被倒序误判为企业微信的接收文件区域；文件后缀/文件名仍要求位于指定祖先下。
- 两种分析查询使用 35 个样本文件验证用途说明，不产生清理规则或删除资格。
- 最新跨版本调整后，核心库 17 项单元测试、11 项分析集成测试通过；相关核心库/分析测试/识别基准示例的 Clippy、改动 Rust 文件的格式检查、规则 JSON/Schema 的 Prettier 和覆盖文档同步检查通过。

尚未进行应用内逐菜单操作、删除后恢复测试、用户试用或发行打包。规则及路径单元测试不能替代这些验收。现有 0.15.0 发行包的覆盖范围仍以其打包记录为准。

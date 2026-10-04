# 应用缓存覆盖与处理依据（0.18.0 测试版）

当前源码取消应用版本白名单，按实际身份、布局、格式或官方命令能力核验；以下版本数字保留为历史样本。未开放规则不因此开放，详见 [兼容性策略](cleanup-compatibility.md)。

本表保留应用覆盖轮的范围说明；后续开放的 Cherry Studio/PyCharm 代码与图形缓存以同包 `readonly-cache-evidence.md` 为准，其余范围仍按下表执行。

核查日期：2026-10-02。网络研究只在开发阶段进行；程序离线使用编译期固定目录和许可，不下载规则，不上传文件名、路径或内容。安装登记、已读取文件和清理许可是三个不同的判断。

| 项目 | 处理方式与条件 | 限制与保护 |
|---|---|---|
| VS Code / Insiders 扩展下载缓存 | 固定 `CachedExtensionVSIXs` 直接子文件；最后修改满 30 天；ZIP 结构、清单 CRC、名称和版本一致；主动选择 | 当前适配产品包版本 1.134.0；Microsoft 签名、安装版本与产品构建一致且应用退出。以后可能重新下载。临时下载、签名包、平台后缀包、已安装扩展和用户配置保留。 |
| VS Code / Insiders 旧代码缓存 | 固定 `CachedData/<commit>/chrome/{js,wasm}`；排除当前构建；构建目录修改超过稳定版 90 天 / Insiders 7 天，文件也须满足各自期限 | 首批只适配 Simple Cache v5 条目及 v9 标记；其他索引、旧格式和伪造文件名保留。准备和执行重新核对安装、构建、目录和文件身份。 |
| Trae、Trae CN、Kiro、通义灵码、WorkBuddy、HBuilderX | 固定应用数据位置及已观察到的分区缓存；只读识别 | 不把通用 Electron 知识视为对应应用版本许可。对话、配置、恢复内容、项目和插件不进入这些规则。 |
| 阿里云盘、123 云盘、Cherry Studio、Apipost、Another Redis | 各自固定缓存子目录；只读识别 | 下载、同步内容、数据库、账号和持久存储不处理。 |
| 紫鸟、必剪、OpenCode | 已知根与窄缓存位置；紫鸟逐个检查 `instances/userdata<数字>`，OpenCode 仅固定 WebView 缓存 | 实例资料、视频工程、素材和账号信息保留；缺少具体应用版本依据，不开放删除。 |
| JetBrains IDE / PyCharm | 各版本系统目录中的缓存、索引、Python stubs 与 JCEF 缓存只读识别 | 不直接删除系统根。使用 IDE 自带 Invalidate Caches；Local History、fileHistory、配置、插件、项目和恢复资料保留。 |
| Visual Studio | 固定版本目录中的 ImageLibrary、TextMateCache、WebView2Cache；只读 | 已安装组件、SDK、项目、恢复资料不纳入；已有 ComponentModelCache 规则保持原限制。 |
| 应用更新下载目录 | 按所属应用固定 updater 目录单独展示 | 可能是待安装包或重试所需内容，不能仅凭时间判断废弃；全部只读，不计入可清理容量。 |

资料依据：

- [VS Code 扩展下载与缓存源码（本机产品构建）](https://raw.githubusercontent.com/microsoft/vscode/110a328ea54b42367b803ec53ee0bf52ef26b419/src/vs/platform/extensionManagement/node/extensionDownloader.ts)：下载缓存与扩展安装内容分开管理，临时下载完成后才改为缓存名称。30 天是本产品保守的主动选择门槛，不是官方统一期限。
- [VS Code 旧构建缓存清理源码](https://raw.githubusercontent.com/microsoft/vscode/110a328ea54b42367b803ec53ee0bf52ef26b419/src/vs/code/electron-utility/sharedProcess/contrib/codeCacheCleaner.ts)：按构建区分缓存，保护当前构建，稳定版与 Insiders 使用不同目录年龄。本产品额外限制文件格式、文件年龄和身份。
- [Chromium Simple Cache 格式](https://raw.githubusercontent.com/chromium/chromium/main/net/disk_cache/simple/simple_entry_format.h)：仅用于已获得 VS Code 目录依据后的格式核验，不给其他 Chromium 应用授予清理许可。
- [JetBrains 缓存整理说明](https://www.jetbrains.com/help/pycharm/invalidate-caches.html)：通过 IDE 整理并重建缓存；清除 Local History 是另外的选项，应单独判断。
- [electron-updater 下载状态源码](https://raw.githubusercontent.com/electron-userland/electron-builder/master/packages/electron-updater/src/DownloadedUpdateHelper.ts)：下载缓存参与后续安装与恢复，不是通用垃圾目录。

36 条新增规则中，只有 VS Code / Insiders 的上述四条获得按条件逐文件处理许可，全部默认不选。其余 32 条仅识别。原有年龄门槛以及 pnpm、Bun、百度等限制不变。逐项元数据见随程序内置的 `rules/cleanup-evidence.json`，与规则 ID、版本、布局和处理方式绑定。

“已识别”是实际读到的逻辑大小，包含符合期限的候选及已经读到的近期保留内容；读取失败不估算容量。只读内容、待安装更新、近期内容均不计入可选或已选容量。实际删除逻辑量与磁盘可用空间变化分别呈现。

本机仅做只读核查。新 VS Code 规则通过隔离夹具删除与保护测试，并不等于已经完成真实 VS Code 使用验收。测试机尚未核验到可用的 VS Code / Insiders 安装，不为验收安装软件。真实用户研究继续待验证；最终程序哈希与实际通过情况以本轮验证记录为准。

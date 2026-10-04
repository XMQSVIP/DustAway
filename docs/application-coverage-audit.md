# 本机应用覆盖对照表

核查日期：2026-10-02。本机只读：扫描未准备清理计划、未提交删除，也未执行任何整理命令。原始输入保留在 `artifacts/application-coverage-20261002/host-audit-delivery.json`。

现有软件清单返回 **194 个登记项**（包括桌面软件、商店包和组件，不等于独立应用数量），读取警告 0 项。另盘点 Local/Roaming 下 **322 个一级数据目录**；不搜索任意 Cache 目录来授予许可。

安装在其他盘不影响 C 盘数据目录识别。表中“已有规则”只表示同一软件家族有部分固定位置适配；“用途待确认”是明确缺口，不能解读为没有缓存。“新增适配”也可能只有只读能力。未读到容量不估算，不把这些逻辑大小当成可释放空间。

- 新增适配，按规则核对扫描结果：16 项。
- 用途待确认；尚无固定缓存适配，不代表没有缓存：109 项。
- 已有规则，按实际位置核对：19 项。
- 系统或开发组件；没有独立垃圾许可：50 项。

## 本轮新增规则在本机的实际结果

| 应用 / 规则 | 已读文件（含量化保留） | 已读逻辑大小 | 其中保留 | 许可与本机状态 |
|---|---:|---:|---:|---|
| Trae 缓存 / `appcache.trae` | 128 | 51.17 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Trae CN 缓存 / `appcache.trae_cn` | 437 | 88.52 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Kiro 缓存 / `appcache.kiro` | 64 | 36.41 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| 通义灵码 缓存 / `appcache.lingma` | 14 | 10.59 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| WorkBuddy 缓存 / `appcache.workbuddy` | 23 | 14.76 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| HBuilder X 缓存 / `appcache.hbuilder` | 0 | 0.00 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| 阿里云盘 缓存 / `appcache.adrive` | 476 | 141.41 MiB | 20 文件 / 23.88 MiB | 仅识别 / 只读；validation_pending |
| 123 云盘 缓存 / `appcache.123pan` | 305 | 71.46 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Cherry Studio 缓存 / `appcache.cherry` | 27 | 10.24 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Apipost 缓存 / `appcache.apipost` | 16 | 6.16 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Apipost 7 缓存 / `appcache.apipost7` | 0 | 0.00 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Another Redis Desktop Manager 缓存 / `appcache.redis_manager` | 9 | 0.53 MiB | 1 文件 / 0.26 MiB | 仅识别 / 只读；validation_pending |
| 紫鸟 缓存 / `appcache.ziniao` | 818 | 131.31 MiB | 143 文件 / 87.32 MiB | 仅识别 / 只读；validation_pending |
| 必剪 缓存 / `appcache.bcut` | 0 | 0.00 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| OpenCode 缓存 / `appcache.opencode` | 69 | 9.49 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| VS Code 扩展下载缓存 / `appcache.vscode_vsix` | 6 | 204.67 MiB | 0 文件 / 0.00 MiB | 按条件主动选择；completed |
| VS Code 旧构建代码缓存 / `appcache.vscode_code` | 20 | 24.09 MiB | 13 文件 / 12.75 MiB | 按条件主动选择；completed |
| VS Code Insiders 扩展下载缓存 / `appcache.vscode_insiders_vsix` | 0 | 0.00 MiB | 0 文件 / 0.00 MiB | 按条件主动选择；application_unverified |
| VS Code Insiders 旧构建代码缓存 / `appcache.vscode_insiders_code` | 0 | 0.00 MiB | 0 文件 / 0.00 MiB | 按条件主动选择；application_unverified |
| JetBrains 索引与缓存 / `appcache.jetbrains` | 10523 | 897.18 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Visual Studio 模板与渲染缓存 / `appcache.visual_studio` | 257 | 32.25 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Cherry Studio 更新下载内容 / `appcache.update_cherry` | 3 | 245.06 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| 阿里云盘 更新下载内容 / `appcache.update_adrive` | 2 | 143.65 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| 123 云盘 更新下载内容 / `appcache.update_123pan` | 2 | 136.66 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Another Redis Desktop Manager 更新下载内容 / `appcache.update_redis_manager` | 3 | 240.53 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| 紫鸟 更新下载内容 / `appcache.update_ziniao` | 3 | 256.72 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Xmind 更新下载内容 / `appcache.update_xmind` | 1 | 158.30 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Apipost 更新下载内容 / `appcache.update_apipost` | 1 | 89.59 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Apipost 7 更新下载内容 / `appcache.update_apipost7` | 1 | 149.62 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| 夸克网盘 更新下载内容 / `appcache.update_quark_netdisk` | 2 | 124.71 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| 精臣打印 更新下载内容 / `appcache.update_niimbot` | 1 | 124.24 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| RPA 流程管理 更新下载内容 / `appcache.update_rpa` | 1 | 88.62 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| 打印客户端 更新下载内容 / `appcache.update_hiprint` | 1 | 81.65 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| SwitchHosts 更新下载内容 / `appcache.update_switchhosts` | 1 | 69.50 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Bob VPN 更新下载内容 / `appcache.update_bob_vpn` | 1 | 65.97 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |
| Xanadu 更新下载内容 / `appcache.update_xanadu` | 1 | 93.78 MiB | 0 文件 / 0.00 MiB | 仅识别 / 只读；validation_pending |

更新下载内容与 IDE 系统占用仅供查看。新应用未取得对应版本依据的保持只读。VS Code 候选量也需要用户选择及执行前复核；本机没有进行删除验收。

## 全部软件登记项

| 登记名称 | 版本 | 来源 | 覆盖结论 | 新增规则 |
|---|---|---|---|---|
| 阿里云盘 | 6.9.3 | desktop | 新增适配，按规则核对扫描结果 | appcache.adrive, appcache.update_adrive |
| DbVisualizer | 23.2.5 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| FastOrange | 3.0.0.43 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Xmind 25.7.3033 | 25.7.3033 | desktop | 新增适配，按规则核对扫描结果 | appcache.update_xmind |
| 飞书 | 7.64.6 | desktop | 已有规则，按实际位置核对 | — |
| UU远程 | 4.27.2.8374 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| ima | 2.1.3.3576 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 图片重命名工具 1.0.0 | 1.0.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Rustup: the Rust toolchain installer | 1.29.1 | desktop | 已有规则，按实际位置核对 | — |
| 360清理Pro | 1.0.0.1161 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| zTasker 版本 2.3.3 | 2.3.3 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft Visual Studio Code (User) | 1.134.0 | desktop | 新增适配，按规则核对扫描结果 | appcache.vscode_vsix, appcache.vscode_code, appcache.visual_studio |
| Trae CN (User) | 3.3.17 | desktop | 新增适配，按规则核对扫描结果 | appcache.trae, appcache.trae_cn |
| Python 3.12.10 (64-bit) | 3.12.10150.0 | desktop | 已有规则，按实际位置核对 | — |
| Qt | 4.10.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 流程粘粘粘 版本 1.5.12 | 1.5.12 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| NVM for Windows 1.2.2 | 1.2.2 | desktop | 已有规则，按实际位置核对 | — |
| Cherry Studio | 1.8.1 | desktop | 新增适配，按规则核对扫描结果 | appcache.cherry, appcache.update_cherry |
| 7-Zip 24.08 (x64) | 24.08 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows 驱动程序包 - Apple, Inc. (USBAAPL) USB  (05/19/2017 6.0.9999.69) | 05/19/2017 6.0.9999.69 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| 紫鸟 | 6.27.8-latest.17 | desktop | 新增适配，按规则核对扫描结果 | appcache.ziniao, appcache.update_ziniao |
| Another Redis Desktop Manager 1.7.4 | 1.7.4 | desktop | 新增适配，按规则核对扫描结果 | appcache.redis_manager, appcache.update_redis_manager |
| Windows 驱动程序包 - Apple, Inc. (USBAAPL64) USB  (05/19/2017 6.0.9999.69) | 05/19/2017 6.0.9999.69 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| Apipost 8.2.6 | 8.2.6 | desktop | 新增适配，按规则核对扫描结果 | appcache.apipost, appcache.update_apipost |
| Clash Verge | 2.5.6 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| C盘管家 | 0.7.6 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Docker Desktop | 4.50.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| DustAway | 0.1.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Git | 2.42.0.2 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Mozilla Firefox (x64 zh-CN) | 156.0.1 | desktop | 已有规则，按实际位置核对 | — |
| Mozilla Maintenance Service | 139.0.1 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| PremiumSoft Navicat Premium 16.0 | 16.0.11 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Notepad++ (64-bit x64) | 8.5.8 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| PyCharm 2026.1 | 261.22158.340 | desktop | 新增适配，按规则核对扫描结果 | appcache.jetbrains |
| WinRAR 7.00 (64-位) | 7.00.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| LocalSend version 1.15.3 | 1.15.3 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| GitHub CLI | 2.98.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Java(TM) SE Development Kit 17.0.11 (64-bit) | 17.0.11.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Python 2.7.15 (64-bit) | 2.7.15150 | desktop | 已有规则，按实际位置核对 | — |
| Alacritty | 0.16.1 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft Update Health Tools | 3.74.0.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| ugreen_wifi_driver | 1.1.8 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| Microsoft System CLR Types for SQL Server 2019 | 15.0.2000.5 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| MariaDB Connector C 64-bit | 3.4.3 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| MySQL Connector/C++ 9.1 | 9.1.0 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| Reqable version 3.2.23 | 3.2.23 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft Visual Studio Installer | 3.12.2149.20818 | desktop | 新增适配，按规则核对扫描结果 | appcache.visual_studio |
| MySQL Server 8.0 | 8.0.36 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Tenda无线网卡 | 1.0.5.5 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| TortoiseGit 2.19.1.0 (64 bit) | 2.19.1.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Tailscale | 1.102.2 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Update for x64-based Windows Systems (KB5001716) | 8.94.0.0 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| WifiAutoInstall version 2.0.1.4 | 2.0.1.4 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| CC Switch | 3.20.4 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 产品对比 版本 3.7.0 | 3.7.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 影刀 | 6.3.31 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Redis on Windows | 3.2.100 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft .NET SDK 9.0.100 (x64) from Visual Studio | 9.1.24.52909 | desktop | 新增适配，按规则核对扫描结果 | appcache.visual_studio |
| Go Programming Language amd64 go1.23.4 | 1.23.4 | desktop | 已有规则，按实际位置核对 | — |
| Lively Wallpaper 版本 2.1.0.8 | 2.1.0.8 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| TortoiseSVN 1.14.5.29465 (64 bit) | 1.14.29465 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 节点小宝 | 4.5.2.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Visual Studio 15 生成工具 2017 | 15.9.61 | desktop | 新增适配，按规则核对扫描结果 | appcache.visual_studio |
| 123云盘 | 3.2.3.0 | desktop | 新增适配，按规则核对扫描结果 | appcache.123pan, appcache.update_123pan |
| Visual Studio Community 2022 | 17.12.1 | desktop | 新增适配，按规则核对扫描结果 | appcache.visual_studio |
| 钉钉 | 8.5.5-Release.260903002 | desktop | 已有规则，按实际位置核对 | — |
| Enigma Virtual Box |  | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Google Chrome | 154.0.8037.95 | desktop | 已有规则，按实际位置核对 | — |
| Inno Setup version 6.3.1 | 6.3.1 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| jcPrinterSdk 3.2.3.4 | 3.2.3.4 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| WPS Office 2023 增强版 | 12.8.2.18205 | desktop | 已有规则，按实际位置核对 | — |
| 水星无线网卡产品 | 1.0.0.2 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft Edge | 154.0.4258.48 | desktop | 已有规则，按实际位置核对 | — |
| nat123_V1.260108 | V1.260108 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Npcap | 1.78 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 向日葵远程控制 | 16.5.0.30905 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 迅雷 | 12.4.10.3940 | desktop | 已有规则，按实际位置核对 | — |
| ToDesk | 5.1.0.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| WD Discovery | 5.1.618 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 微信 | 4.0.6.17 | desktop | 已有规则，按实际位置核对 | — |
| 微信输入法 | 2.1.4.6 | desktop | 已有规则，按实际位置核对 | — |
| Wireshark 4.2.3 x64 | 4.2.3 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 悟空 | 0.9.15-1774610421 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 分区助手 10.7.0 | 10.7.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 腾讯QQ | 9.7.23.29394 | desktop | 已有规则，按实际位置核对 | — |
| Microsoft Windows Desktop Runtime - 6.0.36 (x64) | 6.0.36.34217 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| vs_CoreEditorFonts | 17.7.40001 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows SDK AddOn | 10.1.0.0 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| Windows Software Development Kit - Windows 10.0.17763.132 | 10.1.17763.132 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Python Launcher | 3.12.10150.0 | desktop | 已有规则，按实际位置核对 | — |
| Windows Software Development Kit - Windows 10.0.22621.3233 | 10.1.22621.3233 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 夸克网盘 | 3.19.0 | desktop | 新增适配，按规则核对扫描结果 | appcache.update_quark_netdisk |
| Google Chrome 版本 107.0.5304.122 | 107.0.5304.122 | desktop | 已有规则，按实际位置核对 | — |
| MySQL Installer - Community | 1.6.9.0 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| EV录屏 | 5.2.5 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft Visual C++ 2015-2022 Redistributable (x64) - 14.44.35211 | 14.44.35211.0 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| WinFsp 2023 | 2.0.23075 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft Visual C++ 2015-2022 Redistributable (x86) - 14.42.34433 | 14.42.34433.0 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| 有线快猫 | 2.0.2 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft Visual C++ 2010  x86 Redistributable - 10.0.40219 | 10.0.40219 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| Microsoft Windows Desktop Runtime - 8.0.5 (x64) | 8.0.5.33617 | desktop | 系统或开发组件；没有独立垃圾许可 | — |
| 必剪 | 3.11.25.100 | desktop | 新增适配，按规则核对扫描结果 | appcache.bcut |
| 搞机助手 | 4.9.1 | desktop | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 百度网盘 | 8.3.9 | desktop | 已有规则，按实际位置核对 | — |
| 设置 | 10.0.2.1000 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Microsoft Pay | 2.4.18324.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Xbox Game Speech Window | 1.21.13002.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Xbox TCUI | 1.24.10001.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Xbox Game Bar Plugin | 1.54.4001.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Cortana | 4.2308.1005.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 工作或学校帐户 | 1000.19041.4239.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Microsoft Edge DevTools 客户端 | 1000.19041.4239.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Windows Defender SmartScreen | 1000.19041.4239.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| “指定访问锁定”应用 | 1000.19041.4239.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 通话 | 1000.19041.4239.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Microsoft 家庭功能 | 1000.19041.4239.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| PinningConfirmationDialog | 1000.19041.4239.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Xbox Game UI | 1000.19041.4239.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| NcsiUwpApp | 1000.19041.4239.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 中文(简体)本地体验包 | 19041.81.277.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 画图 3D | 6.2410.13017.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 邮件和日历 | 16005.14326.22342.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| OneNote for Windows 10 | 16001.14326.22594.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft Edge | 141.0.3537.92 | store | 已有规则，按实际位置核对 | — |
| Windows Hello 设置 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 网络连接流 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 强制网络门户流 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Windows Feature Experience Pack | 1000.19062.1000.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| UDK Package | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 开始 | 10.0.19041.5438 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Windows Shell Experience | 10.0.19041.5072 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Windows Search | 1.14.18.19041 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Microsoft 内容 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Windows 条形码预览 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 安全删除设备 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 参加测验 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Windows 安全中心 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Windows Shell Experience | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 讲述人 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| CapturePicker | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 桌面应用 Web 查看器 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Windows 默认锁屏界面 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 目视控制 | 10.0.19041.4597 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 凭据对话框 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| AsyncTextService | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 电子邮件和帐户 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| “添加文件夹建议”对话框 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 应用解析程序 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 文件资源管理器 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 1527c705-839a-4832-9118-54d4Bd6a0c89 | 10.0.19041.4239 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 你的帐户 | 10.0.19041.6396 | store | 系统或开发组件；没有独立垃圾许可 | — |
| Windows Print | 6.2.1.0 | store | 系统或开发组件；没有独立垃圾许可 | — |
| 3D 查看器 | 7.2602.8012.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 获取帮助 | 10.2409.41132.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| VP9 视频扩展 | 1.2.20.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Xbox Identity Provider | 12.130.16001.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows 录音机 | 10.2103.28.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Xbox 主机小帮手 | 48.104.4001.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft 使用技巧 | 10.2312.1.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows 终端 | 1.24.11911.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| HEIF 图像扩展 | 1.2.48.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| WebP 映像扩展 | 1.2.31.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Outlook for Windows | 1.2026.818.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft 照片 | 2026.11080.24002.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows Package Manager Source (winget) V2 | 2026.905.459.51 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Web 媒体扩展 | 1.2.42.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| PowerShell | 7.6.6.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 应用安装程序 | 1.29.380.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows Advanced Settings | 0.2100.858.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| BaiduNetdiskService | 1.0.18.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft 便笺 | 6.1.4.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft 人脉 | 10.2202.100.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows 地图 | 11.2506.3.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| MSN 天气 | 4.54.63045.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 混合现实门户 | 2000.21051.1282.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 电影和电视 | 10.26061.10051.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 英特尔® 显卡控制中心 | 1.100.5688.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows 计算器 | 11.2607.0.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Game Bar | 7.326.8061.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows 媒体播放器 | 11.2607.16.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows 时钟 | 11.2607.6.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft 必应 | 1.1.40.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Windows 相机 | 2026.2607.1.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 截图和草图 | 10.2008.3001.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 反馈中心 | 1.2608.17301.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 跨设备体验主机 | 0.25072.59.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft Store 体验主机 | 22608.1401.1.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 手机连接 | 1.25072.81.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft 365 Copilot | 19.2609.52011.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 电脑管家 | 3.22.6.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Solitaire & Casual Games | 4.27.9181.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| 适用于 Linux 的 Windows 子系统 | 3.0.1.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| Microsoft Store | 22608.1401.5.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |
| ChatGPT | 26.930.2377.0 | store | 用途待确认；尚无固定缓存适配，不代表没有缓存 | — |

## 全部一级应用数据目录

| 位置 | 目录 | 结论 | 新增规则 |
|---|---|---|---|
| %LOCALAPPDATA% | .IdentityService | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | .lingma | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | 123pan-dll | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | 123pan-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_123pan |
| %LOCALAPPDATA% | 360Safe | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | @docmirrordev-sidecar-gui-updater | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | @guanjia-openclawelectron-updater | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | @phoenixdesktop-updater | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ASP.NET | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Androws | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | AnthropicClaude | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Apple Computer | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Apple | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Apps | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Awespykit | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | BCUT | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | BeeWare | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | BuiltinBrowser | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Bypass | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | BypassRuntm | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | CEF | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Catime | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ChromeExtensionCache | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Chromium | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | CodeBuddyExtension | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Codex | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Comms | 系统或安装数据；不将整个目录视作垃圾 | — |
| %LOCALAPPDATA% | ConnectedDevicesPlatform | 系统或安装数据；不将整个目录视作垃圾 | — |
| %LOCALAPPDATA% | CrashDumps | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | D3DSCache | 系统或安装数据；不将整个目录视作垃圾 | — |
| %LOCALAPPDATA% | Diagnostics | 系统或安装数据；不将整个目录视作垃圾 | — |
| %LOCALAPPDATA% | DingTalk | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | DingTalkReal | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | DingTalk_133 | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Docker | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | DustAway | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | EVCapture | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | EVSuperPlayer | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ElevatedDiagnostics | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Feishu | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Flux | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | GameViewer | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | GitHub CLI | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Google | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | HBuilder X | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | HDLocal | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | IdentityNexusIntegration | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Jedi | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | JetBrains | 新增固定位置适配；候选及读取情况见规则表 | appcache.jetbrains |
| %LOCALAPPDATA% | LiuChengZzz | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Lively Wallpaper | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Microsoft SDKs | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Microsoft | 新增固定位置适配；候选及读取情况见规则表 | appcache.visual_studio |
| %LOCALAPPDATA% | Mozilla | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | Notepad | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | NuGet | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | Nuitka | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | OneDrive | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | OpenAI | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | PC Manager Store | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Package Cache | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | PackageManagement | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Packages | 系统或安装数据；不将整个目录视作垃圾 | — |
| %LOCALAPPDATA% | Parso | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | PeerDistRepub | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | PlaceholderTileLogoFolder | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Programs | 系统或安装数据；不将整个目录视作垃圾 | — |
| %LOCALAPPDATA% | Progress_Software_Corpora | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Pub | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Publishers | 系统或安装数据；不将整个目录视作垃圾 | — |
| %LOCALAPPDATA% | Pygrower | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | QtProject | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | RustAIBridge | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ServiceHub | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ShadowBot | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ShadowBotBrowser | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | SourceServer | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | SquirrelTemp | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | TSVNCache | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Tailscale | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | TapType | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Temp | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | Tencent | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | Text-Grab | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Thunder Network | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ToDesk | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ToDesk_Lite | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ToastNotificationManagerCompat | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | TortoiseGit | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | TortoiseSVN | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | TuringShield | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Upgrade | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | VirtualStore | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | WebToApi | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | WinSparkle | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Windows Master Store | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Windows Performance Analyzer | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | WorkBuddyExtension | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | Yarn | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | Yodao | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | aardio | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | adrive-desktop-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_adrive |
| %LOCALAPPDATA% | aha_doctor | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | another-redis-desktop-manager-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_redis_manager |
| %LOCALAPPDATA% | apipost-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_apipost |
| %LOCALAPPDATA% | apipost7-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_apipost7 |
| %LOCALAPPDATA% | asin_pd_compare | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | bilidown-updater | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | bob_vpn-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_bob_vpn |
| %LOCALAPPDATA% | cache | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | chaquopy | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | cherrystudio-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_cherry |
| %LOCALAPPDATA% | chfsgui | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | clangd | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | claude-cli-nodejs | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | clink | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | com.bigpizzav3.codexplusplus.manager | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | com.ccswitch.desktop | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | com.cjddsoft.wms | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | com.diskbutler.app | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | com.pake.e81c1f | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | cua-driver | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | do.linux.jbcrack | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | docker-secrets-engine | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | electron-hiprint-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_hiprint |
| %LOCALAPPDATA% | flutter_webview_windows | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | frida | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | gif123 | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | go-build | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | gopls | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | hiroi-sora | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | iflyrpa.org | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ima.copilot | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | img_rename | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | io.dustaway.desktop | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | io.dustaway.desktop.fixture | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | io.github.clash-verge-rev.clash-verge-rev | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | kingsoft | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | ms-playwright | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | neatreader-updater | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | niimbot-print-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_niimbot |
| %LOCALAPPDATA% | node-gyp | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | npm-cache | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | ode-updater | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | pip | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | pip-audit | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | pnpm | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | pnpm-cache | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | pnpm-state | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | pygubu-designer | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | pygwalker | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | pyinstaller | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | pypa | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | python | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | qmlpuppet-18 | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | qmlpuppet-4 | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | qq-chat-updater | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | quark-cloud-drive-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_quark_netdisk |
| %LOCALAPPDATA% | rpa-process-manager-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_rpa |
| %LOCALAPPDATA% | speech | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | staticcheck | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | switchhosts-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_switchhosts |
| %LOCALAPPDATA% | tauri | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | torch_extensions | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | uv | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %LOCALAPPDATA% | vitest | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | xanadu-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_xanadu |
| %LOCALAPPDATA% | xbot_activity | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | xbot_app | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | xmind-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_xmind |
| %LOCALAPPDATA% | zTasker | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ziniaobrowser | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %LOCALAPPDATA% | ziniaobrowser-updater | 新增固定位置适配；候选及读取情况见规则表 | appcache.update_ziniao |
| %APPDATA% | .dart-tool | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | .mamba | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 123pan | 新增固定位置适配；候选及读取情况见规则表 | appcache.123pan |
| %APPDATA% | 360DownLoad | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 360Login | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 360Quarant | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 360Safe | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 360SuperKiller | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 360browser | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 360huabao | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | @docmirror | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Accio | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Adobe | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | AmazonPromotionsViewer | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | ApiPost7 | 新增固定位置适配；候选及读取情况见规则表 | appcache.apipost7 |
| %APPDATA% | Apipost | 新增固定位置适配；候选及读取情况见规则表 | appcache.apipost |
| %APPDATA% | Apple Computer | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | AweSun | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | BCUT | 新增固定位置适配；候选及读取情况见规则表 | appcache.bcut |
| %APPDATA% | Baidu | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | BaiduYunGuanjia | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | BaiduYunKernel | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | BdTranslateClient | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | BtLog | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | BytertcLog | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Charles | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | CherryStudio | 新增固定位置适配；候选及读取情况见规则表 | appcache.cherry |
| %APPDATA% | Claude | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | CleanTrash | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Code | 新增固定位置适配；候选及读取情况见规则表 | appcache.vscode_vsix, appcache.vscode_code |
| %APPDATA% | Codex | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Codex++ | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | CoreLog | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Coze | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | DX12 memory diagnostic | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | DaBaiCai | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | DingTalk | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %APPDATA% | Docker Desktop | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Docker | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | EasySpider | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | HBuilder X | 新增固定位置适配；候选及读取情况见规则表 | appcache.hbuilder |
| %APPDATA% | HuDunData | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | HuoYuYan | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | ICSharpCode | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | IndiePython | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | InstallLog | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Jedi | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | JetBrains | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Kiro | 新增固定位置适配；候选及读取情况见规则表 | appcache.kiro |
| %APPDATA% | LarkShell | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Lenovo | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Lingma | 新增固定位置适配；候选及读取情况见规则表 | appcache.lingma |
| %APPDATA% | LocalSend | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Microsoft FxCop | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Microsoft | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %APPDATA% | Mozilla | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %APPDATA% | Notepad++ | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | NuGet | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %APPDATA% | ODE 多功能助手 | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | OctopusRPA | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | OctopusRPA.BrowserBridge | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | OpenArk | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | OpenCode | 新增固定位置适配；候选及读取情况见规则表 | appcache.opencode |
| %APPDATA% | Oray | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | PLogs | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | PeaZip | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Promotion | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | PyCharm | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | PyMe | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Python | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | PythonStudio | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | QQ | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Qt | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | QtProject | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Reqable | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Rust 图片预览器 | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | SoftIcon | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Subversion | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | SwitchHosts | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | SysCleanPro | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | TPDownloadProxy | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | TaskBuilder | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Tencent | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %APPDATA% | TortoiseGit | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | TortoiseSVN | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Trae CN | 新增固定位置适配；候选及读取情况见规则表 | appcache.trae_cn |
| %APPDATA% | Trae | 新增固定位置适配；候选及读取情况见规则表 | appcache.trae |
| %APPDATA% | Ultralytics | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | UninstallLog | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | VMware | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Visual Studio Setup | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | WD Discovery | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | WinRAR | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Wireshark | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Wondershare | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | WorkBuddy | 新增固定位置适配；候选及读取情况见规则表 | appcache.workbuddy |
| %APPDATA% | Xbot Deployer | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Xiaomi | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | Xmind | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | aDrive | 新增固定位置适配；候选及读取情况见规则表 | appcache.adrive |
| %APPDATA% | another-redis-desktop-manager | 新增固定位置适配；候选及读取情况见规则表 | appcache.redis_manager |
| %APPDATA% | baidunetdisk | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %APPDATA% | bob_vpn | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | clash_win | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | codex-plusplus | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | com.ccswitch.desktop | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | com.diskbutler.app | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | com.wdc.softwareInstaller | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | ding_cache | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | dingcache | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | dinglive | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | dingtalk-rewind-server | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | electron-hiprint | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | go | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | gopls | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | greencore | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | huoyuyan_editor | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | io.github.clash-verge-rev.clash-verge-rev | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | ivx-electron-app | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | kando | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | kingsoft | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %APPDATA% | mssdk | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | notepad | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | npm | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | pip | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %APPDATA% | pywebview | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | qq_guild | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | quark-cloud-drive | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | quickfixtoolcenter | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | rclone | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | realx | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | roaming | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | secoresdk | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | thunder | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %APPDATA% | tongyi | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | utForpc | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | uv | 已有部分固定位置规则；不代表整个目录可清理 | — |
| %APPDATA% | wetype_renderer | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | wetype_update | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | window_w | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | xlgame | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | xybot-go | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | zen | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | ziniaobrowser | 新增固定位置适配；候选及读取情况见规则表 | appcache.ziniao |
| %APPDATA% | 图海速览 20260814 | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 图海速览 20260905 | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 图海速览 20260906 | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 图海速览 20260907 | 用途待确认；未开放缓存规则，保留全部内容 | — |
| %APPDATA% | 图海速览 | 用途待确认；未开放缓存规则，保留全部内容 | — |

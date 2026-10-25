# 0.22.0 应用占用识别验收（2026-10-23）

真实应用数据只读。测试机仅 SSH，未启动微信或其他应用、未操作远程桌面，未修改网络、防火墙或 UAC。新增识别不授予删除许可。

## 固定识别范围

| 规则 | 当前用户固定范围 |
| --- | --- |
| `appcache.shadowbot_web` | Local `ShadowBot/{cef,cef_main}/cache` 内 Cache、Code Cache、GPUCache、DawnCache |
| `appcache.shadowbot_browser` | Local `ShadowBotBrowser/User Data/Default` 内 Cache、Code Cache、GPUCache、DawnCache、DawnGraphiteCache、DawnWebGPUCache |
| `appcache.shadowbot_logs` | Local `ShadowBot/log` |
| `appcache.codex_web` | Roaming `Codex/web/Codex/Default` 内 Cache、Code Cache、GPUCache、DawnGraphiteCache、DawnWebGPUCache；`codex-browser-app` 内 Cache、Code Cache |
| `appcache.claude_web` | Roaming `Claude` 内 Cache、Code Cache、GPUCache、DawnGraphiteCache、DawnWebGPUCache |
| `appcache.docker_web` | Roaming `Docker Desktop` 内与 Claude 相同的五个子目录 |
| `appcache.docker_logs` | Local `Docker/log/host` 与 `Docker/log/vm` |
| `appcache.reqable_logs` | Roaming `Reqable/log` |

八项规则均统计所有年龄，默认不选、生产清单拒绝。日志为 Diagnostics，旧元数据缺省仍为 Cache。格式未确认的普通文件可量化保留占用；不因此接受删除。按文件身份去重，同规则多个固定范围和硬链接不重复计数。目录联接和云占位文件不跟随。

不扩大到 artifact-sessions、登录/聊天资料、数据库、项目、备份或更新包；不把整个应用根作为垃圾。Xmind 已观察到的缓存目录为空，没有因此把其约 177 MiB 应用根纳入垃圾规则。安装登记只是来源线索，缺登记不表示文件没有占用。

## 用途说明与依据

- 影刀 `ShadowBot/cache`：本机观察到活动包、资源及离线执行资料，仅提示“可能”，保留整体；[产品资料](https://www.yingdao.com/qa)不作为删除许可。
- `node-gyp/Cache`：可能是 Node 原生模块编译使用的头文件和 SDK，当前构建可能依赖；[官方说明](https://github.com/nodejs/node-gyp)。
- Reqable `backup`、`tmp/backup`：本机观察到的备份位置，不能因为位于 tmp 就认为无用；[官方 Windows 说明](https://reqable.com/en-US/docs/faq/windows/)提供日志背景，不证明备份已失效。
- 复用已有 Playwright 浏览器依赖与 Docker WSL 用途规则，不重复新增。依赖可能仍在使用，虚拟磁盘不可直接删除；[Playwright](https://playwright.dev/docs/browsers)、[Docker](https://docs.docker.com/desktop/features/wsl/)。
- 缓存机制参考 [Chromium](https://chromium.googlesource.com/chromium/src/+/main/net/disk_cache/)；具体应用目录来自本机与测试机观察，不把通用缓存说明当成对应软件的恢复验收。
- Docker 日志位置依据 [Docker 官方说明](https://docs.docker.com/desktop/troubleshoot-and-support/faqs/general/)。

## 验证与交付

证据位于 `artifacts/coverage-022/`。生产探针只输出规则、来源状态、已知容量、保留容量、读取完整性及原因，不导出私人文件名、路径或内容。远程任务以 MSN 普通用户执行，五阶段独立 120 秒期限、30 秒取消等待，schema 3 绑定运行、二进制和实际证据哈希。schema 2 微信历史证据不能代替本轮覆盖验收。

当前编译目录为 123 条用户规则：42 条逐文件清理、4 条官方工具、77 条只读。新增识别容量全部保留，可清理容量增量为 0。

### 两机生产扫描对照

| 新增范围 | 开发机已识别字节 | 测试机已识别字节 |
| --- | ---: | ---: |
| 影刀网页缓存 | 23,928,281 | 14,590,874 |
| 影刀内置浏览器缓存 | 2,365,713 | 0（标准位置缺失） |
| 影刀日志 | 4,511,168 | 101,270,512 |
| Codex 网页缓存 | 16,838,347 | 11,791,993 |
| Claude 网页缓存 | 1,114,848 | 0（标准位置缺失） |
| Docker 网页缓存 | 1,672,416 | 0（标准位置缺失） |
| Docker 日志 | 31,642,150 | 0（标准位置缺失） |
| Reqable 日志 | 198,773 | 8,827 |
| 合计新增已识别 / 已保留 | 82,271,696（约 78.5 MiB） | 127,662,206（约 121.7 MiB） |
| 可清理容量增量 | 0 | 0 |

来源为 `local-readonly.json`、`coverage-20261023-022-01/preflight.log`；两机各八项删除计划均拒绝。所有本次读取项完整，缺失位置单独标记，不声称对应软件没有占用。运行期间的正常应用写入可使后续扫描数值变化。

### 自动检查

- 前端 31 个测试文件、372 项通过；Core 59 项通过 / 2 项需专门环境而忽略；桌面接口 7 项通过。
- Windows 首轮 161 项通过 / 2 失败 / 39 忽略。两项失败在固定范围以外的既有确认测试中，校验当前测试二进制时拒绝了构建目录联接。复制同一二进制至普通目录后，两项分别通过；没有放宽断言或安全边界。原失败日志及复测日志保留。
- SSH 运行 `coverage-20261023-022-01` 五阶段通过：只读预检、八规则边界、状态区分、链接保护、Core 回归。普通 MSN 令牌验证成功，schema 3 证据校验通过。
- 证据脚本：新增覆盖 3 项、既有应用证据 12 项、应用验收 6 项、微信证据 9 项、桌面驱动纯函数 2 项通过。
- 编译规则投影为 123 条用户规则；用途识别 259 条、514 个模式。全工作区 Clippy、TypeScript 构建、全仓 Prettier、Rust 格式及 PowerShell 语法检查通过。
- 修复并保留了首次编译/测试失败记录：元数据测试事件名称、旧规则数量断言、目录联接夹具路径写法及前端测试对象缺失字段。没有将首次失败记录改写为通过。
- 前端仍有单块超过 500 kB 的构建提示，非构建失败。

### 安装与包交付

测试机已通过 SSH 从已核验的 0.21.1 升级为 0.22.0，安装退出码 0。安装载荷 SHA256 为 `33AFA318AD7466A804FCB2848D7F912BC23BC0F65B17428E759345B2D9014FDB`，辅助组件为 `60E41C6F8B6C2668AE502FD53AFA5F44EC2E37CADEFB8C1793F76890F51B3907`，与构建结果一致。没有启动应用界面或修改安全设置。

安装包 SHA256：`641DC5742BC8856F9E1739AFA41EA717A4B393065CB415F6B1E5B03D5BB45BEF`。

免安装 ZIP SHA256：`2055123F2823EFEE09E7B22EBF4A5ACCBC79F230233DD9DED9C2B2B9D62FB38F`。测试机已核验 ZIP、版本和全部 17 个载荷文件；主程序 SHA256 为 `DDDDFE1E6A11B88FB1CDEFE8FDF48F2B274896BE91A88420EE4F4E99FDA58122`，辅助组件不存在，见 `portable-evidence.json`。

安装版启用现有 `system-cleanup` 特性并含辅助组件；免安装版不启用该特性、不含辅助组件，系统与混合权限请求仍被拒绝。两种版本新增八项均只读。包内文档是打包时快照，最终记录与 SHA256 清单位于 `artifacts/releases/0.22.0/`。

最终远程核对本轮临时任务剩余 0 个，安装载荷再次匹配。证据目录保留；现有未提交改动、历史包与历史记录未清理，0.21.1 交付目录原有八个文件的哈希仍匹配。

微信网页首次/再次加载、真实缓存功能恢复、760×580、浅深色和原生键盘体验未验收。隔离夹具通过、文件哈希和安装成功都不替代这些验证；微信代码/图形缓存继续只读。

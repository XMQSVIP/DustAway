# 只读缓存逐项开放依据（2026-10-02）

当前兼容性策略已调整：软件版本仅作记录，不作为清理白名单；已开放规则按实际身份、布局、组件和格式核验。扫描后更新仍会使旧清单失效。详见 [应用清理兼容性策略](cleanup-compatibility.md)。以下实机数据与旧包校验值保留为历史证据，不代表所有版本均已实测。

适用：拂尘 0.18.0 的 readonly-unlock-test 测试包。以下是编译期固定许可；网络研究仅发生在开发阶段，程序不联网下载规则，不上传用户路径或文件。

| 已开放项目 | 安装及格式条件 | 影响 |
|---|---|---|
| Cherry Studio 代码缓存 | 1.8.1 Windows x64 官方程序及 app.asar 的 SHA-256 匹配；固定主配置、webview 分区的 Code Cache；Simple Cache v5 条目与 v9 目录标记 | 下次打开页面可能重新编译；共享索引保留 |
| Cherry Studio 图形缓存 | 同一安装身份；固定 GPUCache、DawnGraphiteCache、DawnWebGPUCache 及 webview 对应位置；blockfile v2 的 data_0 至 data_3 与 index v3 | 下次显示界面可能重新编译着色器 |
| PyCharm 代码缓存 | 2026.1、PY-261.22158.340；JetBrains 签名、产品信息、构建及 JCEF 137.0.7151.104 一致；固定 jcef_cache/Default/Code Cache | 下次内嵌网页可能重新编译；不处理 IDE 索引 |
| PyCharm 图形缓存 | 同一安装身份；固定 JCEF 的 GPUCache、GrShaderCache、ShaderCache、GraphiteDawnCache 与 Default 下两个 Dawn 缓存位置；同一 blockfile 格式 | 图形缓存可重新生成；不处理 Local History、Python 环境或项目 |

四项均**默认不选、按文件最后修改时间满 7 天**。该期限是本产品的保守选择门槛，不表示 7 天前访问过的内容必然无用。应用与相关组件运行、状态无法确认、安装或格式不匹配时不能清理。准备与执行仍复核文件、安装、目录身份、排除项、链接、云文件、年龄与占用。取消或重连不会重复提交。

不把同一文件重复列入只读和可选择容量。近期缓存仍按已实际读取的数量与大小说明；释放空间不以这些逻辑大小推算。

## 官方依据

- [Cherry Studio v1.8.1 自身清缓存实现](https://raw.githubusercontent.com/CherryHQ/cherry-studio/v1.8.1/src/main/ipc.ts)：分别处理默认会话和 persist:webview。拂尘只采用上述窄缓存位置，不执行其 cookies、filesystem、serviceworkers 等存储清除操作。
- [该版本依赖清单](https://raw.githubusercontent.com/CherryHQ/cherry-studio/v1.8.1/package.json)与 [Electron 40.8.0 发布记录](https://releases.electronjs.org/release/v40.8.0)：对应 Chromium 144.0.7559.236。
- [Cherry 官方发布附件](https://github.com/CherryHQ/cherry-studio/releases/expanded_assets/v1.8.1)：Windows x64 portable 下载包 SHA-256 为 `a54fba2d81b978de51b31cc7d628cca279bf0b89d0ef2838a3f95162b60305b1`。仅解包核查，未安装或运行该应用。
- [PyCharm 对应构建 JCEF 清理源码](https://raw.githubusercontent.com/JetBrains/intellij-community/pycharm/261.22158.340/platform/ui.jcef/jcef/JBCefAppCache.kt)：确认系统目录下的 jcef_cache 是应用可失效重建的浏览器缓存。本产品不沿用整个目录清除，保留账号、历史、持久存储和数据库。
- [Chromium 137 的图形缓存处理](https://raw.githubusercontent.com/chromium/chromium/137.0.7151.104/gpu/command_buffer/service/gr_shader_cache.cc)、[Chromium 144 对应实现](https://raw.githubusercontent.com/chromium/chromium/144.0.7559.236/gpu/command_buffer/service/gr_shader_cache.cc)：缓存未命中返回空，着色器可重新生成。对应版本的 blockfile 与 Simple Cache 源码用于格式核查，不单独授予其他应用许可。

Cherry 官方解包与本机只读核验一致：程序 SHA-256 `a866793f98aace8f554fe714def1c51d9d1e857a0bad2457c6b88135391ffdbe`；app.asar SHA-256 `94936572c26f420780ca2f5ffae9de58f1e83ef783eec247278881a2328788d9`。其他版本不继承许可。

## 本轮没有开放的内容及具体原因

- **Cherry/PyCharm 网页响应文件**：已确认属于缓存，但单独删除 f_xxxxxx 会破坏共享索引所引用的响应。已安装 Edge 的隔离实测重现了首次访问 `ERR_CACHE_READ_FAILURE`，第二次访问才重新下载。因此撤回试验中的两项网页规则，保留原只读扫描；不能将“缓存可再生成”等同于当前逐文件处理方式已经可靠。失败原始记录位于 `artifacts/readonly-unlock-20261002/edge-cache-recovery-v2/result.json`。
- **PyCharm 索引与系统缓存**：整目录包含相互关联的 VFS 数据，本地历史依赖其中一部分。保持 IDE 内 Invalidate Caches 指引，不删除整个系统目录。[JetBrains 说明](https://www.jetbrains.com/help/pycharm/invalidate-caches.html)
- **Another Redis Desktop Manager**：v1.7.4 的“清缓存”实现调用 localStorage.clear()，并非仅删除磁盘缓存；不能直接照搬到需要保护连接和本地配置的清理规则。[对应源码](https://raw.githubusercontent.com/qishibo/AnotherRedisDesktopManager/v1.7.4/src/components/Setting.vue)
- **其他应用、更新下载、共享依赖**：沿用既有边界；此次许可不会扩散到百度、pnpm、Bun、项目依赖或待安装更新。

## 验证状态

四项通过官方用途与版本核验及生产扫描、计划、删除路径的隔离保护测试。另用已安装 Edge 的独立配置验证了部分 GPU 缓存删除后 WebGL 渲染一致、HTTP 缓存保持命中；这是缓存恢复检查，**不等于 Cherry/PyCharm 的真实应用验收**。

开发电脑仅对实际应用数据做只读覆盖核查。授权 Windows 11 测试机没有安装这两款应用，真实应用删除、再生成及正常使用仍标为未验证；不安装缺少的软件。最终程序哈希、桌面流程与完整自动检查详见本轮验证记录。用户研究仍待招募，不以自动化替代用户验收。

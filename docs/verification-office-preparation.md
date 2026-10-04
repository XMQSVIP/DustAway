# 办公通信软件扩展：开发验收记录

历史阶段记录：以下数量与结果对应上一轮办公通信扩展；当前源码见[网盘与下载工具矩阵](support-matrix-netdisk-preparation.md)。

日期：2026-09-29。范围为 0.18.0 基础上的开发源码，未发布新安装包或免安装包。当前共 28 条用户扫描规则：14 条既有条件清理、14 条只读；其中 11 条应用规则全部只读。真实应用删除验收和非开发者试用均未执行。

## 工程检查

| 检查 | 结果与证据 |
| --- | --- |
| 前端全量 | 17 文件、201 测试通过，`artifacts/office-extension-frontend.log`；后续支持信息与指引修改分别补跑 7 项、73 项并通过 |
| Rust workspace | 139 项通过、3 项按原配置忽略；`artifacts/office-extension-rust-final.log` |
| 证据工具 | 9 项通过，涵盖 v1/v2、跨版本布局、读取失败、字段/哈希/路径/伪造验收拒绝；`artifacts/office-extension-evidence-tests.log` |
| 真实 v1 历史包兼容 | `artifacts/applications-0.14/local-evidence` 校验通过，依然返回删除未执行；`artifacts/office-extension-legacy-evidence.log` |
| 前端构建、调试桌面构建 | 均通过；`office-extension-build-final.log`、`office-extension-desktop-build.log` |
| 格式、Clippy | Rust fmt、Prettier、workspace/all-targets Clippy `-D warnings` 通过；对应 `office-extension-*-format*.log`、`office-extension-clippy-final.log` |
| 注册表与用途识别 | 编译清单生成及一致性检查通过，11 条应用规则；用途识别库检查 246 条规则、483 个模式通过 |
| 许可清单 | 465 个包，0 项待许可正文审查，1 项已有文档说明的开发依赖例外；未新增依赖 |
| 桌面夹具 | 真正 WebView2/Tauri IPC 回归通过，扫描 28 条、应用 11 条；`artifacts/office-extension-desktop-final.log`、`office-extension-e2e/desktop-e2e-0.18.0.json` |

前端最后补充网页资源、脚本编译与图形渲染的具体用途文案后重新生成预览规则并构建，支持信息/只读决策/指引测试再次通过，见 `artifacts/office-extension-copy-tests.log`。该文案调整没有改变目录、候选、删除门槛或 IPC。

Rust 的三个忽略项分别要求真实浏览器验收夹具、专用扫描性能夹具和独立 release 查询性能测量；本轮没有把它们算作通过。桌面测试实际删除只使用原有专用调试夹具，真实应用目录仅读取元数据。

## 逐条应用规则

以下本机数据来自最终 v2 包 `artifacts/office-extension-local-evidence-final/evidence.json`，采样时间戳为 `1790677956`。所有 11 条均为 `read_only`，`coverage_complete=true`，`error_code=null`，`scan_basis=verified_version`；这表示本次指定布局可完整检查，不表示整个应用已完整统计，也不表示获得删除授权。数字为逻辑字节，不能解释为可释放容量。

| 规则 | 自动夹具验证 | 本机候选 / 逻辑字节 | 真实删除、复启与保护资料验收 |
| --- | --- | --- | --- |
| weixin.logs | 扩展名、7 天门槛、身份与静态拒绝通过 | 4 / 303067378 | 未执行 |
| weixin.dumps | 扩展名、30 天门槛、身份与静态拒绝通过 | 6 / 10410960 | 未执行 |
| qq.http | 数字配置、标记、白名单叶目录通过 | 5 / 594288 | 未执行 |
| qq.code | 数字配置、标记、白名单叶目录通过 | 4 / 144 | 未执行 |
| qq.gpu | 数字配置、标记、白名单叶目录通过 | 5 / 557424 | 未执行 |
| wps.logs | 扩展名、7 天门槛、保护相邻资料通过 | 0 / 0 | 未执行，需真实候选 |
| feishu.http | 2 账号 × 2 配置，发现 4 个合成候选；禁止删除通过 | 0 / 0 | 未执行，需真实候选 |
| feishu.code | 2 账号 × 2 配置，发现 4 个合成候选；禁止删除通过 | 0 / 0 | 未执行，需真实候选 |
| feishu.gpu | 2 账号 × 2 配置，发现 4 个合成候选；禁止删除通过 | 0 / 0 | 未执行，需真实候选 |
| dingtalk.http | 2 个数字目录，发现 2 个合成候选；禁止删除通过 | 492 / 56086080 | 未执行 |
| dingtalk.code | 2 个数字目录，发现 2 个合成候选；禁止删除通过 | 1828 / 145488617 | 未执行 |

本机微信、QQ、WPS、钉钉的阻止原因是 `application_running`；飞书为 `validation_pending`。全部应用的静态删除验收仍为 false；运行状态不改变此限制。本次钉钉两类合计约 192.2 MiB；飞书三类目标目录存在但没有候选文件，其中代码缓存检查了 16 个目录项，没有保留或读取失败项。WPS 检查 6 个目录项，没有符合条件的日志。这些零结果不算真实删除验收通过。

版本使用 Windows 固定文件版本：微信 `4.1.13.65`、经典 QQ `9.7.23.29394`、WPS `12.8.2.18205`、飞书 `7.64.0.0`、钉钉 `8.5.5.903`。飞书启动器/内部程序的 ProductVersion 可不同；钉钉 ProductVersion 也不同于固定文件版本。早期观察值已更正到实际 API 读取的字段，初次采集保留在 `artifacts/office-extension-local-evidence`，最终结论使用 `-final` 包。

## 既有条件清理与 Windows 只读回归

以下 14 条均逐条通过核心 Fake 平台的扫描、保留指定文件、生成计划、执行及一次性计划拒绝回归。Windows 文件系统夹具另外覆盖真实句柄删除、父目录替换、链接、占用和文件身份；这不等于本轮逐个真实浏览器的删除验收。

| 规则 | 自动回归 | 本轮真实应用删除 |
| --- | --- | --- |
| windows.user_temp | 通过，另有真实桌面专用夹具删除 | 未执行个人目录删除 |
| edge.http | 通过 | 未执行 |
| edge.code | 通过 | 未执行 |
| edge.gpu | 通过 | 未执行 |
| chrome.http | 通过 | 未执行 |
| chrome.code | 通过 | 未执行 |
| chrome.gpu | 通过 | 未执行 |
| firefox.http | 通过 | 未执行 |
| brave.http | 通过 | 未执行 |
| brave.code | 通过 | 未执行 |
| brave.gpu | 通过 | 未执行 |
| vivaldi.http | 通过 | 未执行 |
| vivaldi.code | 通过 | 未执行 |
| vivaldi.gpu | 通过 | 未执行 |
| windows.crash_dumps | 既有窄范围、身份、只读门槛回归通过 | 未执行，仍只读 |
| windows.wer_archive | 标记、相邻资料保留、只读门槛回归通过 | 未执行，仍只读 |
| windows.directx_cache | 配对文件、期限、只读门槛回归通过 | 未执行，仍只读 |

## 边界与界面

- 可信版本升级但布局不变仍可只读识别；未知布局、签名失败、经典 QQ 与 QQNT 混淆保持拒绝。飞书同一安装的双程序归并，内部程序版本变化会改变身份快照；多个有效安装仍拒绝。
- 飞书仅直接账号子目录的两种固定配置；钉钉仅数字后缀和 Default。缺失标记、非白名单配置、相邻持久资料不进入候选。
- 多配置读取失败保留部分覆盖；配置父目录替换、junction、硬链接、身份变化、长期排除和取消均已覆盖。新增规则即使收到手工复核或伪造静态字段，也不能获得实际删除权限。
- 桌面实际发出有效格式、带 `reviewed_rule_ids` 的飞书准备请求，后端明确返回“真实应用验收尚未完成”，没有生成计划。只读类别没有勾选框，运行提示不提供关闭解锁入口；发现量不进入已选容量。
- 760×580 与 1100×780 下没有横向溢出，主要操作保持可见；浅色、深色、减少动效和部分读取状态沿用桌面回归。另检查飞书/钉钉分组截图 `artifacts/office-extension-e2e/{feishu,dingtalk}-group-{760,1100}.png`，仅扫描专用夹具。

首次桌面回归因测试点击了未展开父层中的 summary 而超时，修正选择器后全程通过；新增父目录测试先后遇到显示路径分隔符比较、mklink 参数分隔符问题，统一测试路径表达后全量通过。首轮全局格式检查发现原有 `scripts/desk-release.mjs` 格式问题，仅格式化该文件。原日志保留，不把失败轮次当作成功证据；最终无未解决自动检查失败。

## 交付与待执行项

源码修改前备份为 `artifacts/office-extension-baseline.zip`，逐文件哈希见同名 JSON；没有覆盖已发布的 0.18.0 包。桌面回归程序的 SHA-256 保存在 E2E JSON 中，只对应该调试夹具构建，不能作为发布程序或删除解锁凭据。

只读证据不输出个人文件名、账号、路径或原始错误，校验器始终返回 `may_enable_cleanup=false`。本轮没有新增任意路径清理入口、遥测、数据库迁移或在线规则。

后续真实删除操作单见[支持矩阵](support-matrix-office-preparation.md)，每条规则都需要真实候选、复启、缓存再生成/新日志与保护资料核对。QQNT 独立布局采样、Windows 只读规则真实验收，以及[五名非开发者试用](usability-office-preparation.md)继续待执行；本轮不建设隔离环境，不宣称用户试用或新增清理能力已经开放。

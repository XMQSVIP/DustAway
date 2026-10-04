# 0.19.0 系统清理实施与验收

本轮采用当前用户标准 Temp 和 Windows 标准 Temp 的连续 24 小时保留期。ZIP、MSI、VSIX 不按后缀放宽保护，不扩展到 Downloads、Windows Installer 或持久安装缓存。标准 IIS `inetpub/logs/LogFiles/W3SVC*` 仅处理命名、内容及路径通过核验且超过 30 天的历史访问日志，自定义日志位置未覆盖；删除会失去相应历史访问记录。

保留记录与删除候选分开存储。保留明细显示路径、已知大小、修改时间及原因，按大小降序分页；查询或查看明细不会改变选择。无权限、读取中断及未知大小保持独立状态。升级后必须重新扫描，旧扫描与未执行计划不沿用。

## 安装版与免安装版

| 能力 | 安装版 | 免安装版 |
|---|---|---|
| 当前用户文件清理及保留明细 | 支持 | 支持 |
| 缩略图、DirectX 着色器官方维护 | 原用户权限执行 | 原用户权限执行 |
| Windows Temp / IIS 标准历史日志 | 用户主动授权独立组件；首次不预选 | Windows 存储设置入口 |
| 系统官方维护 | 用户主动授权独立组件 | Windows 存储设置入口 |

前端仅传固定操作 ID、后端会话和计划。官方维护使用机器级注册、固定 CLSID 与系统 DLL，核验签名（含 Windows 目录签名）、路径及注册范围；辅助组件在提升权限前核验编译时摘要。用户类别保持原用户权限。排除设置冲突整项禁用。

估算使用 GetSpaceUsed，确认后使用 Purge。计划绑定组件身份、范围、会话及有效期，执行前估算增加需重新确认。逐文件清理先执行，官方维护顺序执行。取消请求通过回调传递，返回前保持“正在停止”。执行意图先落盘，断连或崩溃后未完成记录恢复为待核实，不自动重试。

## 已授权远程 Windows 11 实测

测试电脑 DESKTOP-24NJTT6，Windows 11 x64 26200.9457；2026-10-03 通过 Tailscale SSH 严格主机指纹核验。未修改防火墙、网络、UAC 或执行策略。用户处理器使用 MSN 的交互式普通权限令牌；系统处理器使用该测试机已有管理员令牌。只对测试机执行下列已授权官方处理器及专属临时夹具。

| 处理器 | 结果与估算（字节） | 生产状态 |
|---|---|---|
| 缩略图 | 普通权限完成；23,214,808 → 0 | 开放 |
| DirectX 着色器 | 普通权限完成；3,246,616 → 0 | 开放 |
| 传递优化 | 完成；667,200,058 → 0 | 开放 |
| 安装与升级日志 | 完成；之前 1,221,246,976，之后处理器路径不存在，显示未知 | 开放 |
| 系统错误转储 | 完成；专属 64 KiB 旧转储被官方处理器删除 | 开放 |
| 小型错误转储 | 完成；专属 64 KiB 旧转储被官方处理器删除 | 开放 |
| 安装日志 | 完成；专属 64 KiB 旧日志被官方处理器删除 | 开放 |
| Windows 更新 / 过期组件 | Purge 返回 0x80070005；CBS 记录组件目录文件无法删除 | 仅估算，未开放执行 |
| Defender 支持文件 | 返回成功，但前后均为 45,255,812，未证实有效删除 | 仅估算，未开放执行 |
| Windows 错误报告 | 测试机注册包含未支持的前置操作 | 不可用并说明原因 |

估算下降不称为“实际删除字节数”。磁盘可用空间另行记录，也不能归因为精确删除量；升级日志此次磁盘可用空间增加 343,830,528 字节，不能声称删除了全部 1.2 GB。

Defender 扫描历史和定义备份、Search 事务日志、Prefetch，以及未覆盖的 CBS/DISM 日志均显示保留状态，不提供直接删除。WinSxS 不直接遍历删除，不使用 ResetBase。

远程真实 Windows Temp 验收只选择专属夹具的过期 MSI，删除 26 字节；近期 ZIP 与未来时间 VSIX 保留。实际报告 ID `caa6fd48-ce0d-4054-aa2f-e8cf8ba4e11b`。诊断转储及日志三个专属夹具删除后均核验不存在。

MSN 普通权限下的标准用户 Temp 也完成同一夹具验证，删除 1 个过期 MSI / 26 字节，近期 ZIP 与未来时间 VSIX 保留。首次测试使用单元夹具的 10 秒等待期限，在扫描完成前超时；仅将专用远程测试等待期限延长后通过，未修改生产扫描或删除规则。汇总回执见 `artifacts/system-cleanup/remote-evidence.json`。

缩略图重新生成已验证：普通权限下创建专属 PNG，先使用 `SIIGBF_THUMBNAILONLY | SIIGBF_INCACHEONLY` 查询返回缓存未命中，再生成缩略图返回成功，随后仅缓存查询也成功。记录为 `artifacts/system-cleanup/thumbnail-regeneration.json`。该测试不再次清理缓存。

开发机的隔离夹具复现 269 个旧文件 / 39.1 MB 与额外安装包，验证连续 24 小时边界、未来时间、锁定文件、身份变化、链接、排除、拒绝访问和只读记录不能进入删除计划。新统计兼容旧缺失值，显示未知。前端覆盖选择独立、Windows 单独确认、授权取消保留普通结果及分页只读。

原始测试脚本及回执保存在 `artifacts/system-cleanup` 和测试机 `C:/Users/MSN/AppData/Local/DustAwayAcceptance/20261003-system-cleanup`。远程删除测试具有固定主机、专属标记及调试测试入口限制，生产程序没有该验收绕过入口。

## 自动检查与交付验证

- `pnpm test`：30 个测试文件、359 项通过。`pnpm check:applications`：规则投影一致性通过。
- `cargo test --workspace --locked`：核心、集成及桌面命令检查通过；Windows 库 152 项通过、36 项显式跳过，两项既有应用夹具达到 10 秒等待上限，使用同一测试二进制逐项串行复验后均通过。文档测试另行通过。未将这两次超时记为首轮通过。
- `pnpm test:cleanup-desktop` 对应 0.19.0 清理专项原生测试通过：真实 Tauri IPC、保留记录只读、760×580 / 1100×780 浅深色布局、键盘操作、确认取消、专属夹具删除、排除、历史及丢失响应恢复。证据为 `artifacts/cleanup-preparation-e2e/system-cleanup-0.19.0.json`。旧版完整桌面脚本末尾的应用覆盖分支不计为本轮通过项；其旧文案和结果等待时序已更新。
- 安装版于指定远程测试机成功安装到 Program Files，目录所有者为 Administrators，版本与 helper 摘要匹配；MSN 普通权限启动、112 条用户规则只读扫描及统计界面通过。安装回执 `artifacts/system-cleanup/installed-evidence.json`，启动扫描回执 `installed-smoke.json`；此流程未执行所扫描文件的删除。
- 免安装包在同一远程普通权限会话通过启动和 112 条用户规则扫描，确认不具备系统提权能力；随后通过交付程序估算缩略图与着色器、仅选择 Windows 维护、生成独立确认页并取消。逐文件清单为 0、取消后维护回执仍为空，证据为 `portable-smoke.json` 与 `portable-windows-estimates.json`。
- 安装包：`target/release/bundle/nsis/拂尘_0.19.0_x64-setup.exe`。免安装目录：`artifacts/portable/拂尘_0.19.0_windows-x64`，附 SHA256SUMS.txt。未签名测试版本。

构建时开发盘空间不足，仅将项目 `target` 构建缓存迁至 `F:/DustAwayBuildCache/20261003/target`，原路径以目录联接保留；未清理开发机个人文件。远程测试数据与回执保留，结束后移除本轮创建的已完成测试计划任务。

## 验证限制

尚未验证 Windows 10、使用另一管理员账户的真实 UAC 流程、从旧安装版升级及所有操作系统更新组合。回调取消与崩溃恢复由自动测试覆盖，未声称完成真实维护中断或跨账户授权验收。当前 Windows 11 已授权会话无法代替这些验证。更新、Defender 和错误报告处理器按上述结果保持关闭，不通过扩大目录范围或修改系统权限来强行开放。

清理后的后续只读估算观察到缩略图 5,258,264 字节、着色器 8,200 字节，表明缓存重新产生；缩略图另有上述明确的缓存命中检查，着色器未做游戏性能人工验收。

实现依据：[IEmptyVolumeCache](https://learn.microsoft.com/en-us/windows/win32/api/emptyvc/nn-emptyvc-iemptyvolumecache)、[IIS 日志管理](https://learn.microsoft.com/en-us/iis/manage/provisioning-and-managing-iis/managing-iis-log-file-storage)、[组件存储清理](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/clean-up-the-winsxs-folder)。

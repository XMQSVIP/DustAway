# 跨盘应用占用识别（2026-10-25）

本轮重点是补齐实际占用识别。当前开发源码仍为 0.24.0，新增四条规则，编译目录共 145 条用户规则：42 条逐文件清理、4 条官方工具、99 条只读。已有安装包未在本轮重新发布。

## 发现方式

普通垃圾扫描原先主要依赖用户 AppData，选择 D 盘也不会自动发现安装目录和迁移的数据目录。本轮新增 `folder=discovered`，复用现有扫描、容量汇总、应用分组与文件明细：

- 只枚举本地固定磁盘，按所选卷的身份筛选，盘符相同但卷身份不同不回退。
- 钉钉从磁盘根下的 `dingcache`、`DingTalkAppData/DingTalk`、`DingTalk`，以及安装登记位置发现。升级包同时补充 Local/Roaming AppData 的 `DingTalk`。
- 升级包只接受 `DingTalkUpdate_v<数字>_<版本>[-<版本>]` 直属子目录，并要求普通文件 `upgrade_manifest.json`。仅检查元数据，不解析清单正文；名称相似但没有清单的目录不进入识别。
- 自定义钉钉日志需有程序文件，或 `config` 加 `globalStorage` / `defEmotion` 目录组合，只统计 `log`、`holmeslogs`、`recordLog`、`updaterlogs`。标准 Roaming 日志仍由原有规则负责，路径规范化后去重。
- GameViewer 从磁盘根的 `GameViewer` 和安装登记位置发现，核对 `GameViewer.exe` 或 `bin/GameViewer.exe` 后只统计 `log`。支持登记指向 `bin` 的布局。
- HBuilderX 从磁盘根的 `HBuilderX` / `HBuilder X` 和安装登记位置发现，核对 `HBuilderX.exe` 后只统计 `update/backup`。

安装登记用于寻找位置，不执行登记中的命令，也不证明软件身份可信。目录及文件身份去重、排除项、取消、部分读取失败、链接及云文件处理沿用原有边界。不会递归搜索整个磁盘的所有 `cache`、`log` 或 `update` 名称；未知自定义位置仍可能未覆盖。

升级包及备份归入更新内容分组，日志归入日志分组。只读识别量可查看，不能当作已经确认可释放的容量；不会自动选择或删除。

## D 盘生产扫描实测

使用普通库的 `discovered_scan` 示例调用生产 `scan_on_volumes`，只读扫描 D 盘。原始结果位于 `artifacts/recognition-discovery-20261025/d-drive-scan.json`，未停止应用、未执行清理。

| 新规则 | 命中非空目录 | 文件数 | 逻辑字节 | GiB |
| --- | ---: | ---: | ---: | ---: |
| 钉钉升级包与补丁 | 26 | 104,299 | 19,931,234,221 | 18.5624 |
| 钉钉自定义目录日志 | 8 | 231 | 563,502,423 | 0.5248 |
| GameViewer / UU 远程日志 | 1 | 336 | 831,769,651 | 0.7746 |
| HBuilderX 升级备份 | 1 | 32,544 | 419,068,598 | 0.3903 |
| 合计 | 36 | 137,410 | 21,745,574,893 | 20.2521 |

四项扫描均成功，无读取问题，均不可自动清理。逐文件元数据扫描耗时约 192 秒，其中钉钉补丁约 131 秒，HBuilderX 备份约 60 秒。

对照此前 D 盘清单：26 个非空钉钉升级目录全部命中，字节总量与此前 18.562408 GiB 一致。第 27 个 `DingTalkUpdate_v2_8.3.10.260413002` 是空目录且无升级清单，不增加占用。日志包含本次新增的运行记录及后来产生的文件，因此与旧清单数值不同。这里只核对文件逻辑大小，不声称实际可释放同等磁盘空间。

## 验证入口

```powershell
cargo test -p dustaway-core --locked --lib
cargo test -p dustaway-windows --locked --lib --test cleanup_release --target-dir F:\DustAwayBuildCache\20261003\target
cargo clippy -p dustaway-core -p dustaway-windows --all-targets --locked --target-dir F:\DustAwayBuildCache\20261003\target -- -D warnings
node scripts/application-catalog.mjs --check
pnpm exec vitest run src/test/application-cache.test.tsx --pool forks
pnpm build
cargo run -p dustaway-windows --locked --example discovered_scan -- D:\
```

本机 `target` 是目录联接，测试程序从其真实目录执行，以免将工程缓存联接误当作夹具访问失败。

新增夹具覆盖完整生产扫描与计划拒绝、升级清单和版本目录识别、相邻个人数据隔离、自定义安装登记、重复登记与标准日志去重、所选卷身份变化、排除项、取消、部分权限失败和目录联接。使用合成文件，无真实应用删除测试。

前端首次使用默认线程池时工作线程启动超时、没有运行断言；切换独立进程池后同一文件 5 项测试通过，未修改超时设置。TypeScript/Vite 构建通过，保留已有的大文件块提示。

最终验证结果：

| 检查 | 结果 |
| --- | --- |
| Core 规则及既有库测试 | 61 通过，2 个原有性能测试忽略 |
| Windows 全库回归 | 190 通过，39 个原有专用环境测试忽略；包括 6 个新增发现夹具 |
| 普通构建清理边界集成测试 | 3 通过，新规则均拒绝伪造清理许可 |
| 前端应用规则分组与计量 | 5 通过，独立进程池 |
| TypeScript / Vite 构建 | 通过 |
| Clippy | Core / Windows 初次全目标通过；最终 Windows 全目标再次通过，均 `-D warnings` |
| 编译规则投影、差异空白检查 | 通过 |
| 本轮 Rust 文件格式 | 定向 rustfmt 通过 |

第一轮全库回归暴露两处问题，已修复并通过上述回归：历史日志夹具仍断言旧的 141 条规则；自定义日志与标准日志比较前未统一路径分隔符，导致标准路径可能重复统计。排除位置另保留“已排除”状态，避免显示成位置缺失。

验证期间工作区另有并行开发的文件用途分类模块；一次全工作区格式化因其尚未创建的测试模块中断。本轮改用定向格式化，没有修改或删除该模块。

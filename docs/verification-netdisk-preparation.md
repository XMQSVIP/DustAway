# 网盘与下载工具扩展：开发验收

日期：2026-09-29。基于 0.18.0 的源码开发，未发布新包。共 35 条用户扫描规则，14 条条件清理、21 条只读；18 条应用规则全部只读。真实应用删除验收与非开发者试用均未执行。

## 工程验证

前端 17 文件、222 项通过（artifacts/netdisk-frontend-tests-final.log）；证据工具 12 项通过（netdisk-evidence-tests-final.log）；v1/v2 历史真实包兼容通过（netdisk-legacy-v1.log / netdisk-legacy-v2.log），历史目录与修改前基线逐字节相同。前端构建通过（netdisk-build-final.log）。用途库为 256 条、510 个主匹配项，覆盖文档同步检查通过。Prettier 全量检查与许可清单生成通过。Rust workspace 最终 144 项通过、3 项按原配置忽略（netdisk-rust-final.log）；Clippy workspace/all-targets -D warnings、Rust fmt、调试桌面构建均通过（netdisk-clippy.log、netdisk-rust-format.log、netdisk-desktop-build.log）。编译目录投影校验通过（netdisk-catalog-check.log）。桌面完整回归通过（netdisk-e2e.log；[结果 JSON](../artifacts/netdisk-extension-e2e/desktop-e2e-0.18.0.json)），覆盖 35 条用户扫描、18 条应用夹具、只读授权拒绝、长期排除、分页与单次确认、取消确认、保护文件、历史与任务恢复。10 万条分析夹具验证通过；实际删除仅涉及专用夹具的既有清理路径。首轮发现 .xltd 匹配项缺少前导点，以及新增规则/夹具前置改变既有顺序；已修正，失败轮次日志保留，不计作通过证据。

首次桌面回归仅完成初始截图，进程结束且没有完成报告；中间产物保存在 artifacts/netdisk-extension-e2e-interrupted/ 与 netdisk-e2e-interrupted.log，不计为通过。重新运行的完整桌面夹具已通过。

忽略项为隔离真实浏览器验收、标记目录扫描性能基准、最大文件查询性能基准；没有把这些未执行项计入通过数量。

## 桌面显示与既有清理回归

三款新增软件在 1100×780、760×580 两种窗口下均无横向溢出，类别标题与只读行可见、无勾选控件，已选与预计释放仍为 0 B。定位后的独立截图和布局断言见 [application-group-layouts.json](../artifacts/netdisk-extension-e2e/application-group-layouts.json)、[百度窄窗口](../artifacts/netdisk-extension-e2e/baidu_netdisk-group-760.png)、[迅雷标准窗口](../artifacts/netdisk-extension-e2e/thunder-group-1100.png)。已人工查看实际截图。显示尺寸测试不等于另行完成所有真实 Windows DPI 验收。

原 overview 截图函数回到页首，未展示对应软件；新增 scripts/application-visual-checks.mjs 定位可见分组补测，复用已通过 E2E 的带标记夹具并核对可执行文件哈希，只执行扫描与页面操作。首轮补测暴露测试脚本未处理恢复后的隐藏页面/隐藏底栏，修正定位后 三款应用×两种尺寸的 6 个组合全部通过（netdisk-application-visual-final.log）；没有改动产品的删除权限。该补测已接入 pnpm test:e2e。

| 既有条件清理规则 | 本轮自动回归 | 真实浏览器删除验收 |
| --- | --- | --- |
| windows.user_temp | 引擎选择、一次性计划、Windows 文件夹具和桌面流程通过 | 不适用；仅删除专用夹具 |
| edge.http / edge.code / edge.gpu | 3 条引擎夹具回归通过 | 本轮未执行 |
| chrome.http / chrome.code / chrome.gpu | 3 条引擎夹具回归通过 | 本轮未执行 |
| firefox.http | 引擎夹具回归通过 | 本轮未执行 |
| brave.http / brave.code / brave.gpu | 3 条引擎夹具回归通过 | 本轮未执行 |
| vivaldi.http / vivaldi.code / vivaldi.gpu | 3 条引擎夹具回归通过 | 本轮未执行 |

## 本机只读与逐规则结果

本轮只读取真实应用身份与文件元数据，不启动应用生成缓存、不读取数据库/配置正文、不删除真实应用数据。7 条新规则使用专用合成文件验证候选和保护边界；夸克、迅雷本机目标目录空样本不能替代真实候选与删除验收。

采样时间戳 1790689262；v3 包：[evidence.json](../artifacts/netdisk-local-evidence/evidence.json)，完整性清单见同目录 manifest.json。校验通过，catalog_id 为 sha256:7c4d903cd302488a9668df1eae6cfb9a11ac7691afabc6c0d234a274df192355。18 条均为 verified_version、coverage_complete=true、error_code=null、inspection_status=read_only；deletion_performed=false、acceptance_status=not_run，校验器返回 may_enable_cleanup=false。完整只指固定检查范围，逻辑字节不是可释放容量。

| 规则 | 本机候选数 | 逻辑字节 | 采样时阻止原因 | 真实删除验收 |
| --- | ---: | ---: | --- | --- |
| weixin.logs | 4 | 303067378 | application_running | 未执行 |
| weixin.dumps | 6 | 10410960 | application_running | 未执行 |
| qq.http | 5 | 594288 | application_running | 未执行 |
| qq.code | 4 | 144 | application_running | 未执行 |
| qq.gpu | 5 | 557424 | application_running | 未执行 |
| wps.logs | 0 | 0 | application_running | 未执行 |
| feishu.http | 0 | 0 | validation_pending | 未执行 |
| feishu.code | 0 | 0 | validation_pending | 未执行 |
| feishu.gpu | 0 | 0 | validation_pending | 未执行 |
| dingtalk.http | 492 | 56086080 | application_running | 未执行 |
| dingtalk.code | 1828 | 145488617 | application_running | 未执行 |
| baidu_netdisk.http | 66 | 17251456 | validation_pending | 未执行 |
| baidu_netdisk.code | 50 | 11932 | validation_pending | 未执行 |
| baidu_netdisk.gpu | 5 | 557424 | validation_pending | 未执行 |
| quark_netdisk.code | 0 | 0 | validation_pending | 未执行 |
| quark_netdisk.gpu | 0 | 0 | validation_pending | 未执行 |
| thunder.code | 0 | 0 | validation_pending | 未执行 |
| thunder.gpu | 0 | 0 | validation_pending | 未执行 |

新增 7 条规则均在合成样本发现候选、保护相邻资料并拒绝删除。百度三条在本机发现真实候选，合计 121 个、17,820,812 逻辑字节（约 17 MiB）；夸克、迅雷四条的真实非零样本未执行，后续必须补采。飞书与 WPS 本次也没有候选，不能据此判定功能完整或无需验收。真实签名、固定文件版本与 Preferences 普通文件检查另存 artifacts/netdisk-local-identity-metadata.json，不含账号或个人路径。

## 交付边界

修改前源码备份为 artifacts/netdisk-extension-baseline.zip，备份元数据及 SHA-256 见同名 JSON。相对基线的变更清单见 artifacts/netdisk-source-changes.json。既有办公通信阶段证据、历史发行程序保留。桌面夹具产生的程序与截图只代表调试回归，不是发布包或解锁凭据。

新增目录仍受封闭注册表、元数据标记、父路径与文件身份、重解析点/云文件、长期排除、取消和会话规则约束。只读容量不能进入选择与删除计划；没有新增任意路径删除入口、数据库迁移、遥测或在线规则。

后续真实任务见[逐规则操作单](support-matrix-netdisk-preparation.md)，每条缓存都须有至少一个真实候选、清理后正常运行与再生成、保护资料复核。五名非开发者的[试用任务](usability-netdisk-preparation.md)继续待执行，本轮不建设隔离环境。

# 拂尘 0.9 验收记录

日期：2026-09-27。环境：Windows 10 专业版 22H2 / 10.0.19045，x64；Rust 1.97.1、Node 24.12.0、pnpm 10.30.3。功能实现、夹具验证与真实应用验收分别记录。

**六条新增应用规则已经实现固定范围扫描、身份识别和阻止流程，但真实应用删除验收未完成，发行版保持只读。既有十四条清理规则继续按原边界工作。**

## 已通过检查

| 检查 | 结果与证据 |
| --- | --- |
| 前端完整测试 | 7 个文件、54 项通过；artifacts/frontend-tests-0.9.log |
| Rust 工作区 | 91 项通过，文档测试通过；3 项默认忽略，其中真实浏览器验收随后单独执行，另两项为专项性能；artifacts/rust-tests-0.9.log |
| 静态检查 | cargo fmt、Prettier、Clippy --workspace --all-targets -D warnings 通过；artifacts/rust-format-0.9.log、prettier-0.9.log、clippy-0.9.log |
| 实验编译兼容 | system-cleanup feature 检查通过；artifacts/experimental-check-0.9.log；没有执行 UAC 或安装器 |
| 构建与许可 | TypeScript / Vite 通过；第三方清单 465 项、0 项缺少许可正文、1 项已记录的开发工具例外；artifacts/notices-0.9.log |
| 桌面夹具 E2E | 完整通过；artifacts/desktop-e2e-0.9.json、e2e-0.9.log |
| 真实浏览器回归 | Brave/Vivaldi 各三类缓存生成、删除、重建及测试 Cookie/localStorage/其他保护数据验证通过；artifacts/browser-validation-0.9/evidence.json |
| 本机应用身份 | 三款主程序版本、离线签名与发布者匹配；artifacts/application-support-0.9.json；程序哈希与卸载项旧版本对照见 application-identities-0.9.json |

工作区测试覆盖新增规则默认关闭与固定字段、7/30 天边界、未来时间与扩展名、QQ 数字目录及标记、标记丢失和权限拒绝、程序身份变化、结构化阻止原因、准备计划后应用重启，以及原有链接、目录替换、锁定、排除、取消、部分扫描与中断恢复。

桌面 E2E 走通原有“扫描 → 明细 → 排除 → 计划 → 取消确认 → 确认删除 → 报告/历史”闭环；六条新规则走通“主动选择 → 扫描 → 模拟应用退出 → 重查 → 待验收阻止”，检查所有候选和相邻保护文件内容、修改时间保持。**新规则没有走删除成功路径，因为后端验收开关仍关闭。** 夹具中的进程状态由专用标记模拟；应用信息卡仍读取真实本机程序，不能将该测试称为真实三款应用的退出/重启验收。

截图包括明暗主题、760 × 580 CSS 像素最小窗口、长路径与禁用计划按钮；最小窗口没有横向溢出，操作栏保持可见。键盘选择、明细、排除与分析流程通过。主要证据：applications-gated-0.9.png、guides-minimum-0.9.png、guides-dark-0.9.png、desktop-results-0.9.png。

只读分析回归使用 100,002 个合成文件，包含共享硬链接、最大文件、搜索、目录定位、恢复、取消和任务互斥；本次是 Debug 功能验证，不用于宣传发行性能。具体耗时和资源原始数据保存在桌面 E2E JSON。

## 日常目录只读观察

以下来自 artifacts/application-scan-0.9.json，只输出计数、逻辑字节和原因，不记录候选路径，不执行删除。范围已通过扫描筛选；**仍不是可执行计划或保证可释放空间**。正在运行的应用可能继续改变数据。

| 规则 | 候选文件数 | 候选逻辑字节 | 当时阻止原因 |
| --- | --- | --- | --- |
| weixin.logs | 3 | 217,265,008 | application_running |
| weixin.dumps | 6 | 10,410,960 | application_running |
| qq.http | 5 | 594,288 | application_running |
| qq.code | 4 | 144 | application_running |
| qq.gpu | 5 | 557,424 | application_running |
| wps.logs | 0 | 0 | validation_pending |

微信日志另保留 22 个近期项及 5 个未允许格式项，转储保留 2 个其他格式项；WPS 保留 5 个近期项。原因项包含扫描遇到的文件/目录，不估算未遍历容量。程序安装于 D 盘，而本机规则范围经 Known Folder API 解析到 C 盘漫游数据。

微信卸载项 DisplayVersion 为 4.0.6.17，但签名有效主程序版本为 4.1.13.65；实际识别使用后者。经典 QQ 为 9.7.23.29394，WPS 为 12.8.2.18205。支持输出中的范围路径仅用于本地核对，产品诊断导出不加入这些路径。

## 发行验证

Release 构建与发行只读冒烟已通过。交付目录：`artifacts/portable/拂尘_0.9.0_windows-x64/`，6 个文件，不含 helper、安装器或 ZIP。EXE 版本 0.9.0，大小 12,889,088 字节（12.29 MiB），签名状态 NotSigned。

EXE SHA-256：`D868A4D1DA0EA028A6FF50CB0CC772C95C2F1C2F1EA6593C26AC2090C11D3288`。包内 SHA256SUMS 的五项逐一匹配，交付 EXE 与实际冒烟测试的 Release EXE 哈希一致。证据：artifacts/delivery-verification-0.9.json、portable-build-0.9.log、portable-smoke-0.9.json、portable-smoke-0.9.log。

发行版从中文/空格路径和禁止写入的程序目录成功启动；重复启动唤回、移动后设置保留、标题栏最大化/还原/双击/最小化/关闭均通过。确认内嵌前端与当前构建一致，运行的是 standard 后端、20 条规则且没有系统清理能力。三款应用支持接口核验成功，六条新规则默认关闭、验收限制仍生效。对六条新应用规则和六条 Brave/Vivaldi 规则做真实目录只读扫描，全部完成，无删除任务。测试恢复了自身写入的扫描类别偏好。

版面已人工查看浅色发行截图 portable-readonly-0.9.png 与深色最小窗口截图 applications-gated-0.9.png。原有浏览器完整清理闭环来自独立配置回归；本机普通资料没有用于删除测试。

## 修复与重跑说明

- 首次完整 Rust 检查中常规测试通过，但并行构建覆盖中间库，导致文档测试找不到库；已经串行重跑全部工作区与文档测试并通过。原始失败日志保留在 rust-tests-0.9-before-serial-retry.log。
- 桌面测试发现原脚本会对扫描收尾时仍禁用的按钮进行无效点击。点击助手现等待控件可见且启用，再实际点击；整套 E2E 重跑通过。没有用跳过断言或直接触发 DOM 点击替代操作。旧失败日志保留为 e2e-0.9-before-ready-wait.log 和 e2e-0.9-before-enabled-click.log。
- 测试同时发现待验收总览仍使用应用占用提示，已改为明确说明不能清理；前端完整测试与桌面场景重跑通过。
- 两份失败运行的合成临时目录（dustaway-e2e-dd5Wq6、dustaway-e2e-2orGdJ）清理被工具自动审批拒绝，仅返回 blocked by policy；目录保留。通过的 E2E 和浏览器夹具也保留供核对，没有对其他临时文件执行清理。

## 未执行与限制

微信/QQ/WPS 的隔离真实应用删除、缓存重建、正常启动、新日志生成和测试登录/资料保留未执行：当前没有可用的隔离 Windows 环境与专用登录测试账号。六条开关全部关闭，不能以夹具结果替代真实应用验收；逐项状态见 [支持矩阵](support-matrix-0.9.md)。

Windows 11、其他硬件、完整 DPI 与屏幕阅读器、缺少 WebView2、真实多卷/配额异常和独立安全审计继续待验证。没有将前版通过项目或单机数据当作本版已测成绩。进程检查与应用重启不是原子操作，不承诺冻结用户应用；离线签名校验不保证最新证书吊销信息。

源码仍为用户原有未提交工作区，未创建提交。0.8 文件哈希清单与历史证据保留；没有声称本轮开始前创建了新的完整源码快照。新增 WinTrust/Cryptography 的 Windows API feature，没有新增第三方包。

# IPC 接口说明

## 0.20.0 微信专项与附件统计增量

`AttachmentPage` 增加 `filtered_bytes`、`selectable_files`、`selectable_bytes` 和 `categories`。字节数使用十进制字符串；当前筛选汇总跨越全部分页，分类汇总覆盖本次已读取的全部附件，不受时间、类型、关键词或已选筛选影响。每个分类含 `kind/files/bytes/retained_files/retained_bytes`，后两者在界面标为“不可选”，包含只读、扫描未完成和本次已处理记录。历史响应缺少字段时显示未知，不推断为零。

微信专项复用普通扫描和附件命令，不接收新的任意路径删除参数。分类点击仅筛选；观察记录、只读附件和已处理附件不能进入清理计划。新增网页缓存规则仍逐项检查独立许可，详见 [微信专项验收](verification-weixin-special.md)。

## 0.19.0 扫描观察与 Windows 官方维护增量

- `list_scan_observations(scope, session, rule, offset, limit)` 返回只读 `ObservationPage`，接口限制每页 1–200 条，前端使用 50 条，按已知大小降序；记录含 path、可空 bytes、可空 modified_unix、reason，不能引用为删除候选。
- `CacheObservation.reasons` 增加按原因的 files / bytes 汇总，旧数据缺失字段保持可读；前端对未读取范围和未知大小单独标注。
- `scan_windows_maintenance(scope)` 只估算；`get_windows_maintenance(scope)` 返回状态与进度；`prepare_windows_maintenance(scope, session, ids)` 接受固定操作 ID 并产生五分钟计划。
- `execute_cleanup` 增加可选 `windows_plans: {scope, plan_id}[]`，支持仅 Windows 维护，也支持先逐文件、后顺序维护；不接受命令、CLSID、删除路径或自定义磁盘路径。
- `cancel_windows_maintenance` 发出取消请求，处理器回调返回前保持 stopping；`get_windows_maintenance_history(scope)` 返回独立回执。估算与可用空间读数均不是实际删除字节数。
- 上述写操作沿用 CommandRequest 协调。计划绑定处理器指纹、系统磁盘身份、会话、排除设置与有效期，执行前范围变化或估算增加需重新确认。安装版 helper IPC 协议升至 2；免安装版拒绝 system scope。

具体实现见 [维护命令](../src-tauri/src/windows_maintenance_commands.rs) 与 [维护状态机](../crates/windows/src/system_maintenance.rs)，支持范围与未验证条件见 [验收](system-cleanup-acceptance.md)。以下旧版说明仅保留兼容背景。

当前百万级分析增量见[实现说明](analysis-million-design.md)。`query_api_version=2` 会话使用 `read_analysis_page`：请求含 `request_id/session/view/node/search/metric/descending/discovery/cursor/offset`，响应含最多 200 条、可空 `total` 和 `next_cursor`。游标与筛选及索引修订绑定，旧游标返回 `stale_cursor`；`get_analysis_count(request_id)` 返回后台总数状态，`cancel_analysis_query(request_id)` 只取消对应读取。旧列表接口保留兼容，不是新 UI 的主查询路径。

`get_analysis_storage` 返回实际临时索引位置，`pick_analysis_storage` 只接受原生选择器结果；任务运行时拒绝更换。元数据索引存储、配额、回收和占用口径以当前实现说明为准，下面旧版“仅内存分析索引”描述保留为历史记录。

适用版本：0.18.0 基础上的应用清理能力开发源码。接口仅供本地 Tauri 主窗口使用，不是 HTTP API，不提供任意路径删除命令。

实现：[桌面命令](../src-tauri/src/commands.rs)、[请求协调器](../src/coordinator.ts)、[前端类型](../src/types.ts)、[核心模型](../crates/core/src/model.rs)。

## 通用约定

`ApplicationRuleSupport` 的 `cleanup_versions: string[] | null` 是本规则通过真实删除验收的准确应用文件版本集合，当前 `dingtalk.http`、`dingtalk.code` 分别返回 `["8.2.0.119"]`，各自绑定独立证据，其余应用规则为空数组；旧数据缺失为 null。`cleanup_block: ReasonCode | null` 是本规则当前阻止原因，旧数据缺失按 unverifiable 处理。前端收到缺失的新资格信息时不能以旧 true 值推断允许清理；旧 false 值仍显示只读。这些应用资格字段保持兼容。uv 新增固定官方整理命令，见 [uv 接口](uv-cache-ipc.md)；无数据库迁移。

`supported_version` 继续兼容为扫描观察样本版本，不能授权删除。逐规则 `cleanup_validated` 来自描述符开关及绑定规则版本/布局的编译期验收记录；应用汇总字段不参与单条规则决策。已有扫描/计划/执行命令不新增授权参数，用户历史数据库不变。v1–v3 只读证据导出显式投影旧字段，不夹带新增 IPC 字段。

命令名 snake_case；Tauri 顶层参数 camelCase，嵌套数据 snake_case。字节数与控制版本使用十进制字符串；前端字节运算使用 BigInt。时间戳为 Unix 秒，进度计时为单调时钟毫秒。分页最多 200 项，搜索最多 256 个字符并按字面包含匹配。

默认构建提供72条用户规则，其中34条按条件逐文件清理、38条不允许逐文件删除（uv单独接入官方整理）；包含37条开发工具规则，默认不选。18条应用规则中只有钉钉两条对固定版本许可，其他应用规则继续只读。默认全面扫描所有未排除用户规则，不根据清理选择或 last_scan_rule_ids 缩小范围。所有 system scope、混合扫描/计划/执行均在工作启动前拒绝；更改请求元数据不能获得系统能力。候选与计划必须来自当前会话，新增文件不会加入旧计划。

## 命令协调

下表标记 W 的命令都增加顶层 `request`：

```ts
interface CommandRequest {
  runtime_id: string;
  request_id: string; // UUID，每个新操作一个
  expected_revision: string;
}
interface ControlInfo {
  runtime_id: string;
  revision: string;
  last_request: {
    request_id: string;
    command: string;
    value: unknown;
    error: { code: string; message: string } | null;
  } | null;
}
```

W 命令的原始 IPC 返回 `ControlInfo`，业务值在 `last_request.value`；业务失败在回执的 error。元数据校验失败仍使用 `{code,message}` rejection。`api.ts` 将回执解包为原有业务返回值或带 code 的 Error，页面不要自行绕过协调器调用写命令。

在同一控制锁内校验预期版本、推进版本、执行命令并保存回执；即使业务拒绝也消费该版本。相同请求的回执可以再次读取，但不会重复执行。运行实例、请求协调信息仅存当前进程；最近回执可含当前设置或计划，不进入持久历史、诊断或在线服务。

`get_runtime_snapshot` 保持只读。`reconcile_task_request({request})` 若发现已处理请求就返回其回执；若请求尚未受理则推进版本、生成 request_expired 回执，迟到原请求无法执行。传 `request:null` 用于页面挂接/重载，推进版本使旧页面尚未受理命令失效；已经运行的任务不被取消。核对不会启动、重发删除或修改清理设置。

前端写入统一排队，受理等待 15 秒，超时后核对而非重发；核对失败时阻止后续写入。读取等待 10 秒。原生文件/保存对话框不设置等待上限。所有过期响应由请求序号、会话和控制版本过滤；运行实例变化要求重新加载。

## 命令目录

| 命令 | 参数（W 另加 request） | 业务值 / 行为 |
| --- | --- | --- |
| bootstrap | 无 | 规则、设置、版本、构建能力、last_scan_rule_ids 偏好 |
| get_application_support | 无 | 只读返回固定八款应用的身份、观察版本、逐规则清理资格与范围；不接受程序/清理路径 |
| start_scan W | ruleIds | sessions、warning；只读扫描启动后才保存实际类别偏好 |
| cancel_job W | targets: [{scope,id}] | void；全部校验目标后请求停止，不取消新会话 |
| get_runtime_snapshot | scope | RuntimeSnapshot：control、status、summary、report |
| reconcile_task_request | request 或 null | 用户范围一致 RuntimeSnapshot；封存未受理请求 |
| get_job_status | scope | JobStatus |
| get_scan_summary | scope | ScanSummary |
| list_candidates | scope、session、rule、offset、limit、query? | CandidatePage |
| refresh_rule_availability W | scope、session | ScanSummary；仅重查应用状态，不加入候选、不清除扫描失败 |
| prepare_cleanup W | preparationId: UUID、requests: [{scope,session,selection}]、manual?: {session,selected_ids,excluded_ids} | [{scope,plan}]；允许 requests 为空且仅自选；单次计划 |
| cancel_cleanup_preparation | preparationId: UUID | void；只取消匹配的准备操作，不等待写命令锁；缺失、非法或与当前操作不匹配的标识拒绝 |
| exclude_analysis_entry W | session、node | {settings,scan_invalidated}；从私有快照取得排除路径，旧分析标为 stale |
| execute_cleanup W | plans: [{scope,plan}] | scopes、warning；后端只信任已保存计划 ID |
| get_cleanup_report | scope | CleanupReport 或 null |
| get_history | 无 | 最近最多 100 条统计，查询时执行 30 天保留维护 |
| clear_history W | 无 | void；只清除历史，不撤销删除 |
| save_settings W | patch | {settings,scan_invalidated}；按字段合并 |
| add_candidate_exclusions W | scope、session、items: [{candidate_id,kind}] | {settings,scan_invalidated}；kind 为 file/parent，最多 500 项，整批验证后原子保存 |
| reveal_candidate | scope、session、candidateId | void；验证候选身份后在 Explorer 选中，不执行文件 |
| pick_exclusion | kind: file/folder | 原生对话框选定路径或 null |
| export_diagnostics | 无 | boolean；保存对话框导出不含个人路径的摘要 |
| open_storage_settings | 无 | void；固定 Windows 存储设置入口 |

`Selection` 为 rule_ids 与 excluded_ids；前端不提交任意删除路径。settings patch 仅含 theme/exclusions/excluded_rules 中需要改变的字段。主题不使扫描失效；范围变化成功保存后旧扫描、候选和计划全部失效。排除追加基于最新设置，不覆盖主题；旧版系统规则排除记录保留。

## 扫描与进度

`JobStatus` 保留 id、state、current_rule、visited（候选数）、completed、total、error，新增 `scan_progress`；`RuleSummary` 保留 phase、rule_id、files、bytes、skipped、blocked、note、scan_failed、scan_issue、availability、reasons，新增 `progress`。

```ts
interface ScanProgress {
  checked: number;
  elapsed_ms: number;
  last_progress_ms: number;
}
```

checked 统计实际遍历到的目录条目，不估算排除目录内部数量。进度与保留原因按 100 ms 合并，分类终态立即提交。last_progress_ms 是相对本次扫描/类别起点的最近检查时刻；elapsed_ms - last_progress_ms 超过 10000 时提示等待当前检查返回，不将此推断为已停止。

阶段为 pending/scanning/completed/partial/interrupted/failed。扫描失败或取消不能生成清理计划。候选 files/bytes 与前端当前选择量分开；尚未完成扫描不显示空结果成功。

## 文件查询与结果

query 包含 search、sort（size/modified/path）、direction（asc/desc）、status、view；最终稳定次序为候选 ID。

- `view:scan`：当前会话中该类别全部候选。
- `view:cleanup`：仅 JOIN 当前 plan_items；需要当前报告。
- `view:unplanned`：仅未出现在当前 plan_items 的候选；同样需要报告。
- status 为 null、candidate、deleted、skipped、failed、unknown。candidate 在清理视图显示为未处理。

返回 items、total、offset。Candidate 包含 id、rule_id、显示路径、允许根目录显示值、字节字符串、修改时间、状态及结构化原因。路径仅用于显示，不是操作授权。前端对当前报告结果数量提供状态明细按钮；历史只有统计，不提供路径入口。

CleanupReport 保留原 JSON 格式及默认兼容值。categories 的 total/deleted/skipped/failed/unknown/deleted_bytes/reasons/retained 用于类别结果。未处理 = total - deleted - skipped - failed - unknown；未选择文件不进入该等式。retained 是扫描期间保留原因，与执行跳过分开。

历史不新增原始路径或请求回执。状态不确定不能计为成功；不改变删除前意图、删除后结果与报告事务的持久化顺序。旧报告缺少类别明细时显示“旧版未记录原因明细”。

## 错误与权限

稳定错误包括 invalid_request、invalid_session、invalid_candidate、invalid_plan、busy、capability_unavailable、storage_failed、worker_failed、operation_failed，以及 0.4 的 response_timeout（前端）、request_expired、runtime_changed、state_uncertain（前端）、closing。展示使用 message，逻辑判断只使用 code。

新增核对命令在 build.rs 与主窗口 capability 中显式登记。默认窗口只加载打包资源；不开放通用 Shell 或文件删除权限。实验 helper 仍不随默认产物交付，系统运行环境未测不能视为通过。

## 0.5 空间分析接口增量

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `get_analysis_locations` | 无 | 常用位置的 token、title、path |
| `pick_analysis_location` | 无；原生目录对话框 | 已登记的位置或 null；取消不会启动分析 |
| `start_analysis` | request、token | ControlInfo 回执，value 为新分析会话 ID |
| `cancel_analysis` | request、session | ControlInfo 回执，停止只绑定指定会话 |
| `list_analysis_children` | session、node、search、offset、descending | 当前节点、面包屑、最多 200 个直接子项、total 与 offset |
| `reveal_analysis_entry` | session、node | 空值；重新验证后原生定位 |

默认根节点 ID 是 `0`；所有节点只在其分析会话内有效。查询 search 最长 256 字符，按字面查找当前层级名称；排序按完整 u64 大小，再按名称与 ID 稳定排序。start/cancel 沿用 UUID 请求、预期版本和超时核对。前端不能传入任意扫描或定位路径。

用户域 `RuntimeSnapshot.analysis` 为可选分析快照，包含 state（scanning/completed/partial/cancelled/failed）、位置、进度、逻辑大小、去重大小、原因、stale 和错误。字节数均为十进制字符串，state 终态不保证统计完整，需区分 completed 与其他状态。

`CleanupReport.volume_space_changes` 为兼容新增数组，项为 volume（磁盘显示标识）、before_bytes、after_bytes、change_bytes、error；数值缺失为 null，change_bytes 可负。旧报告继续使用旧系统盘字段。

## 0.6 兼容性

没有新增 IPC 命令或任意路径参数。bootstrap 返回 14 条普通用户规则，六个新增 rule_id 默认关闭；旧设置、类别偏好和报告仍可读取。分析的 rule_ids 可包含 brave/vivaldi 缓存规则，但分析会话不能授权删除。原生目录选择器保留无限等待用户操作，返回错误统一转为带 code 的 Error；取消仍返回 null。

## 0.7 扫描完整性

不增加命令或任意路径参数。ScanPhase 新增 `partial`；`completed` 只表示本次定义范围的遍历没有读取缺口，正常保留/排除仍可能存在。`interrupted` 和 `failed` 继续禁止生成计划；`partial` 中的已验证候选允许显式确认，应用运行或状态未知时仍被阻止。

RuleSummary、PlanCategory、RuleOutcome 新增 `coverage?: { complete: boolean, issues: { code: ReasonCode, count: number }[] } | null`。这是后端产生的只读输出，随计划固定并进入历史，不是前端授权输入。issues 的 count 是读取问题次数，不是漏读文件数；其中不包含路径。Rust `#[serde(default)]` 保证缺少字段的旧记录读为 None，界面显示未记录完整程度。字段经现有 RuntimeSnapshot 接回，刷新应用状态不修改它。


## 0.8 全范围文件排行与位置背景

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `list_analysis_files` | session、search、offset | items、total、offset；固定按逻辑大小降序、名称和 ID 稳定排序，最多 200 项 |
| `get_analysis_context` | locationToken（原生选择器或常用位置登记的令牌） | location_token、volume、available_bytes、sampled_at、error |
| `open_apps_settings` | 无 | 固定打开 `ms-settings:appsfeatures`，失败返回结构化错误 |

排行使用分析内存索引中已取得大小且无读取问题的文件，零字节文件仍纳入。目录、未知大小和未读取节点不参加；硬链接条目分别排列并保留共享提示，不汇总成可释放量。搜索最多 256 个字符，匹配文件名，不接受路径或 SQL；过期会话拒绝。

AnalysisEntry.advice 为可选 { kind、app_id、basis }，仅查询时产生。kind 为 cache/browser_data/app_data/system_data/personal/unknown/incomplete；旧数据缺失时界面显示通用用途未确定。rule_ids 仍是精确规则关联，配置根不能扩展为整组缓存。依据代码对应已验证规则、标准浏览器数据或具体 Known Folder；不是前端可声明的删除权限。分析路径和说明不写历史及诊断。

磁盘字节仍为十进制字符串。volume 与 available_bytes 采样失败为 null，error 包含可重试说明；sampled_at 为 Unix 秒。不存在的令牌直接拒绝。该命令不启动扫描、不枚举文件，不增加后台采样。应用设置与存储设置只允许两个后端固定 URI，无通用 Shell 路径参数。

## 0.9 应用识别与兼容

`get_application_support` 在阻塞工作线程读取本地信息，返回 `ApplicationSupport[]`：app_id、title、version（未知为 null）、supported_version、identity_verified、scan_supported、cleanup_validated、status、message、running_components、locations。locations 是后端固定规则派生的显示范围，不代表位置存在，也不是操作授权。该结果不加入持久历史或诊断导出。

`RuleDescriptor` 增加可选 app_id 与 data_kind（cache / diagnostics，默认 cache）。扩展名和真实应用验收开关仅位于后端固定表，前端无参数可改变它们。Platform.blocked 返回结构化 `{code,message}`；新增 application_running、application_unverified、unsupported_version、validation_pending，查询异常沿用 process_unknown。旧历史中的 browser_running 继续显示。

重新检查仅更新可用状态；验收未完成仍阻止计划，应用升级/文件变化/配置标记丢失须重新扫描。旧设置中的选择不自动增加新规则；旧 RuleDescriptor 缺少新增字段仍可反序列化。

## 0.10 只读整盘与空间复核

`get_analysis_locations` 新增带盘符的本地固定磁盘。每项增加 `kind: directory | volume`、`is_system: boolean`、`supported: boolean`、`unavailable_reason: string | null`。系统卷由 Windows 已知位置识别。首版卷入口要求 NTFS；后端记住注册时的卷身份，不向前端开放路径授权。

`start_analysis` 仍只提交 token，启动前检查根身份及卷 GUID；盘符重映射、磁盘消失和身份变化拒绝旧令牌。此时需重新启动获取新登记，不自动接受新磁盘。原生选择器仍不接受磁盘根目录作为任意路径入口，根目录只能通过后端登记入口。

新增 `refine_analysis_location({ session, node })`，返回已经验证的子目录位置，前端需另行主动开始分析。只接受当前分析会话中的真实目录 ID；重新校验根、父级和目录身份，不接收任意路径，不授权清理。节点无可验证身份时拒绝，用户可重新用原生目录选择器选择位置。

`AnalysisSnapshot` 新增 `phase: enumerating | walking | indexing | finished` 与 `current_directory: string | null`。`AnalysisEntry.pending` 表示目录尚未开始遍历；已开始但未完成的目录显示至少已统计大小，界面不显示为完整零大小。先列根层直接子项，随后深入；128 层深度超限只跳过对应分支，其他分支继续。50 万条上限、取消、读取问题保留可查询部分结果。

`get_analysis_context` 新增 `volume_key: string | null`。采样前后复核身份，失败字节为 null。前端仅对相同 volume_key 的有效样本用 BigInt 相减，称为观察变化，不进入清理报告。目标、返回位置、样本只存当前进程。旧报告、规则 ID、计划授权、删除协议与数据库格式不变。


## 0.11 分页兼容增量

Tauri 命令名、参数与位置令牌协议不变。AnalysisPage 新增 `generated_at`（Unix 秒）和 `coverage: { completed, incomplete, pending }`；计数针对当前层全部已发现目录，不受名称搜索或本页 200 项限制，不代表整盘覆盖率。AnalysisFilesPage 新增 `generated_at`。旧响应缺字段时前端不伪造生成时间或完整性计数。

AnalysisEntry 的 advice 继续返回后端位置用途，incomplete/pending/issue 独立表示统计状态；不再统一用 incomplete 用途覆盖可信用途。旧 kind=incomplete 仍可读取。新增 issue.code=`resource_limit`，表示内存索引资源限额，保留已提交结果；与 entry_limit 和 depth_limit 区分。phase=indexing 仍属 scanning，取消须等终态。字节字符串、清理规则、计划授权、删除协议与历史数据库均未改。

## 0.12 前端决策层

没有新增或更改 IPC 命令、Rust 数据类型或持久化格式。CleanupDecision 是前端派生类型，消费已有 RuleSummary、Session、Report 与选择排除项，不能授予删除权限。首页复用 get_analysis_locations/get_analysis_context，不启动 start_analysis；报告使用现有分类结果与空间采样。

## 0.13 结果入口兼容

没有 IPC 或历史结构变更。前端 ResultTarget 携带 reportId、scope、session、ruleId、status、count；核对状态统一使用现有 deleted/skipped/failed/unknown/candidate，candidate 在 cleanup 视图表示计划内未处理。单类别直接查询，多类别先显示统计，不跨类别后台请求。

查询仍通过 `list_candidates` 的 session、rule、offset 与 query（view/status/search/sort/direction）完成。打开前检查当前快照身份与类别，新报告或新会话清除旧目标，关闭返回触发入口。新目标重置搜索与页码，过期异步响应丢弃。invalid_session/invalid_plan 降级为统计说明；其他读取错误允许原位重试。

正常保留只依据 Report.categories.reasons 执行原因，不读取 retained 扫描保留统计。旧报告原因不全、类别计数不一致、取消/中断或存在问题时不作正常保留推断；历史仅统计，不绑定当前扫描来读取旧文件。

## 离线用途补充字段

分析条目的 `advice.recognition` 为可选对象，含 `rule_id`、`title`、`description`、`guidance`、`evidence`、`source`、`reviewed` 字符串。未命中时省略。它在目录分页和大文件分页查询时由内置知识库生成，仅描述可能用途，不修改 `advice.kind`、`rule_ids` 或清理资格。旧前端可忽略，旧数据缺少该字段时仍显示原有位置说明。

## 0.14 逐规则应用能力

`get_application_support` 的每个应用新增 `rules: [{ rule_id, cleanup_validated }]`，来自静态规则注册表。保留原 `cleanup_validated`（该应用所有规则均已验收才为 true）与 `status`，没有新增命令。`status` 继续描述本机检查结果，可能同时存在规则只读与应用运行。前端不以应用级旧字段推断单条规则权限；缺字段、重复条目或请求失败显示能力未确认，不计入可核对容量。刷新响应绑定请求轮次，旧响应不可覆盖新状态。计划、删除协议与历史数据库未改动。


## 0.15 前端核对状态

本轮不新增IPC或修改已有参数、计划和历史结构。`analysisDecision`只组织已有advice/recognition/rule_ids；`AnalysisReviewTarget`在内存中绑定分析session、entry、原location、分析时间和视图。应用指引映射不提供清理资格。

面板的空间观察继续调用`get_analysis_context(locationToken)`，使用目标自己的令牌和请求序号，不读取全局后来选择的位置；只有同volume_key的有效读数可比较。定位继续传原session与entry.id，返回invalid_session时清除目标。重新分析通过既有CommandCoordinator，受理未知先核对，不创建新的重试或删除通道。

## 0.16 全范围用途查询

新增只读 `list_analysis_discovery({ session, query })`。query 必含 kind（directory/file）、category（all/recognized/unclassified或十种规则类别键）、min_bytes（u64十进制字符串）、search（最多256字符）、include_nested、incomplete_only、offset（不超过索引上限）。文件查询不接受 incomplete_only=true；无效字段、类别、大小或会话返回错误。扫描与建立索引期间返回 analysis_busy。

响应复用 AnalysisFilesPage：generated_at、items、total、offset，每页最多200项；完整性以同会话 AnalysisSnapshot 为准。排序为已有大小在先、大小降序、名称和ID稳定排序，待统计目录在后。文件只查可排行记录；目录使用当前内存聚合值。最小大小大于0时不包含未知大小目录。

category 的十个键：windows、development、ai、virtualization、games、applications、media、documents、data、archives。来自规则定义，Recognition.category新增可选字段以兼容旧数据。紧凑规则编号仅用于当前内存索引，不作为跨进程标识或清理规则ID。

默认目录折叠在名称与大小过滤之前完成：同一识别规则匹配过祖先则视为内部目录，不影响不同规则嵌套；未归类不折叠。根目录只有具体用途时参与。元数据随条目事务提交，结束后按查询读取，没有第二次磁盘扫描。

前端 AnalysisDiscoveryQuery 与核对来源只在内存，绑定会话、查询键和请求序号；迟到响应不覆盖当前页。删除协议、历史数据库、权限校验和任务变更命令均不受此IPC影响。Tauri仅向本地main窗口开放此只读命令。

## 0.17 扫描与删除选择解耦

规则增加 `cleanup_policy: recommended | review`（缺失默认review）、`cleanup_validated: boolean`（旧规则缺失按true兼容；新增Windows规则固定false）。应用仍以逐规则能力与后端平台门槛为准。`default_selected` 是旧版兼容字段，不再控制扫描范围。

`Selection` 新增可选的 `included_ids`、`reviewed_rule_ids`，缺失按空数组处理。`rule_ids` 表示整类候选，`included_ids` 表示当前会话中的具体候选，二者取并集后应用 `excluded_ids`，重叠文件只计一次。included/excluded各限制100000项。review策略的每个参与规则必须同时出现在reviewed_rule_ids；该标记不绕过cleanup_validated、平台可用性、会话、路径和文件身份核验。

未知/旧会话候选拒绝，准备失败使旧计划失效。一次性plan只包含实际计划内文件的数量、容量和影响；现有IPC命令名、控制元数据与历史数据库保持兼容，无新增任意路径删除接口。

扫描原因增加 `location_missing`：未发现标准位置，不推断必然未安装。与 unsupported_layout、读取失败、完成但无符合条件文件分别呈现。

## 办公软件扩展：扫描依据与能力分离

`get_application_support` 的 `rules` 增量返回 `scan_basis: verified_version | recognized_layout | unverified` 与 `layout_id: string | null`。前者分别表示样本版本及目录匹配、其他观察版本但目录匹配、未确认；均不能替代 `cleanup_validated`。旧数据缺失时默认 unverified/null，前端保留保守能力检查。

`version` 继续为主程序固定文件版本，`supported_version` 保留兼容但在只读扫描中仅表示已核实样本版本；不再据二者相等推断 scan_supported。应用 scan_supported 表示至少一条规则识别了可信布局，各规则可能不同。locations 是固定范围说明，不是用户提供的授权路径。

文件候选依然携带后端私有身份快照；飞书启动器及内部程序共同参与身份比较。升级/路径替换需要重扫。没有新增删除命令、参数或数据库迁移。开发证据格式 v3 独立于产品 IPC，新增 catalog_id，按冻结目录兼容 v1/v2；产品 IPC 字段不变。

## 自选清单与兼容

`start_scan` 新增可选 `volumeTokens: string[]`（Rust 参数 `volume_tokens`）。省略为全部已支持垃圾位置，空数组拒绝；单个或多个 token 只能来自已登记的 `kind=volume` 位置，目录 token、任意路径、重复或失效身份均拒绝。首页默认提交 C 盘对应 token。Windows 在枚举垃圾文件前按卷身份过滤规则根，未选磁盘的目录不进入遍历；新会话 `JobStatus.scan_volumes` 记录盘符标签，null 为全盘范围，旧 JSON 缺失按 null 兼容。此字段只记录显示范围，不承载删除权限。

AnalysisEntry 新增 `manual: {allowed, reason}` 和 `ancestors: string[]`。缺少 manual 时前端不可勾选；用途解释不能授权。祖先 ID 只用于勾选显示，后台从索引重建祖先链，不接受客户端祖先、路径或文件快照。

ManualSelection 拒绝未知字段；selected_ids/excluded_ids 合计最多 100000 条显式覆盖，不能重复或冲突。最近的明确子项选择覆盖父项。目录选择要求子树完整，子树内保护及长期排除项保留。清单流式写入独立 SQLite 内存库，文件身份唯一，文件优先、目录深度降序；清单库最多 512 MiB，原分析仍限 50 万条、128 层、512 MiB。准备可取消，未完成不产生可执行计划。

CleanupPlan 的可选 `manual` 为 `{session,files,directories,bytes,retained}`；files/bytes 总数包含规则和自选文件，rule_ids/categories 只含内置规则。没有任意路径删除 IPC，也没有应用验收解锁参数。规则位置在自选入口保留，必须回到本程序的对应规则控制中选择。

CleanupReport 可选 `manual` 使用 RuleOutcome 的文件统计结构，rule_id 为空且不进入 categories；新增 `directories: {total,deleted,skipped,failed,unknown}`。文件统计不包含目录，字节数只统计文件。旧 JSON 缺少新字段仍可读取，历史数据库不重建。意图 ID 使用 `manual:file:<node>` / `manual:directory:<node>`，持久报告和意图不存个人路径。长期排除路径按既有设置格式保存。

Windows 私有分析记录补全 size、creation/write/change time、attributes、link count，删除前重新核对父链与身份。目录修改时间会因自身子文件删除改变，因此最终空目录检查核对身份、创建时间、属性及父链，交由 Windows handle disposition 拒绝非空目录。不调用递归删除。


## 自选只读摘要与磁盘读数

`get_manual_selection_preview({selection: ManualSelection, requestId: UUID})` 返回 `{request_id, session, files, directories, bytes, retained}`。bytes 为十进制字符串，前端用 BigInt。请求只接受已登记会话和节点 ID，不接受路径、文件快照或授权标志。统计根据扫描索引的私有身份、属性和现有资格计算，使用与准备计划相同的最近祖先覆盖规则、保护和排除策略；按文件身份去重，不相加目录大小。统计描述扫描记录，确认前仍由 `prepare_cleanup` 重新核验实际文件。

每次最多 100000 条显式选中/排除覆盖，继承原 50 万条、128 层分析上限。共享选择遍历器按节点 ID 查询文件、按 `(parent,id)` 索引遍历选定目录子树，排除子树直接裁剪，重选后代作为独立根。每批最多 256 条或 8 MiB 冻结记录，在批次之间释放索引锁，临时身份去重 SQLite 最多 64 MiB。摘要不创建清理计划，不写删除意图或持久历史；两条新增命令仅许可本地主 main 窗口调用。空选择返回零摘要（仍验证会话），空目录独立计数。

`cancel_manual_selection_preview({requestId})` 只终止匹配的统计；新摘要请求登记时终止旧摘要。取消标识独立于扫描、准备和删除任务，等待索引锁期间也可取消。前端连续选择延迟 300 ms 合并请求，校验 request_id / session 并丢弃旧版本响应；统计失败保留选择，显示未知而非零。

首页复用 `get_analysis_locations` 与 `get_analysis_context`，按所选 volume token 查询，最多两个并行请求。后端保持登记时卷身份核验及采样前后卷身份比较，未扩展热插拔登记。首页磁盘与分析位置分别存储；选择、手动刷新、清理完成触发更新，不轮询。失败保留并标识上次读数；不同盘可用空间不合计。

删除接口和历史 JSON 未改变，无数据库迁移。分析索引新增 file_key 列仅存在于进程内临时库。

## 清单准备取消与展示一致性

`prepare_cleanup({requests,manual?,preparationId,request})` 必须带本次新生成的 UUID；`cancel_cleanup_preparation({preparationId})` 使用同一标识。操作在写命令排队前登记，守卫覆盖核验和计划提交；未结束时拒绝新准备操作。取消只设置匹配令牌，接口返回表示请求已记录，不表示系统调用已经返回。当前存在其他准备操作时，不匹配请求返回 `invalid_operation`。没有当前操作时，用至多 128 个 UUID 的集合记住先到的停止请求，随后仅取消同一 UUID 的准备；集合满时明确拒绝，不淘汰已受理的停止标识。每次准备必须使用新 UUID，旧请求不影响其他标识的后续操作；客户端等准备调用结束后恢复操作，响应不确定仍按既有协调器处理。它与扫描、摘要和删除取消状态隔离。

准备从私有索引按批取出元数据、父链及属性，释放索引锁后调用真实文件核验。每批及提交前验证会话、失效状态与取消标志，任何失败废弃未完成清单，不把统计结果作为删除凭据。批次前沿、索引、去重库和清单保持既有资源上限；无永久索引或用户数据库迁移。

前端保留同一有效选择已完成的统计；进入准备时取消未完成统计，取消确认后只恢复未完成计算。选择、会话或排除设置导致分析失效时旧统计不再显示。等待超过两秒只显示已用时间，不产生额外后台轮询。

`cleanupResultSummary` 是前端共享展示模型，不修改 IPC 历史 JSON：先判断非法/不一致计数或文件/目录不确定，再判断停止、失败、未处理和不能解释为正常保留的结果。目录缺少原因明细的跳过使用“清理已结束”。非法数字显示“统计不可用”，合法但矛盾的原始数值保留；旧记录缺少目录或分类明细本身不构成不一致。


## 软件卸载

界面把确认卸载、自动核对与只读残留检查连接为一个流程，使用下列既有命令，不增加接口。前端在当前任务已移除且 `can_scan_residuals=true` 时仅自动提交一次扫描；页面切换不取消此衔接，其他任务忙碌时等待。恢复后无内存范围时不扫描。“暂不清理”“完成”“查看处理结果”只改变本地呈现，不调用卸载或删除命令；自动检查不自动选择、准备或执行清理。缓存批选仅提交当前页可选候选 ID，资料和保留项不参与。

新增 list_installed_software、prepare_uninstall、execute_uninstall、get_uninstall_task、verify_uninstall、scan_uninstall_residuals、list_uninstall_residuals、prepare_uninstall_residual_cleanup、execute_uninstall_residual_cleanup、cancel_uninstall_residual_task、get_uninstall_history。

列表/状态/分页/历史为读取接口，其余命令携带 CommandRequest，返回 ControlInfo 中的受理回执。卸载提交仅接受 softwareId 或 planId；复核、扫描和停止仅接受 taskId；残留准备接受 taskId 与 candidateIds；残留执行仅接受 planId。分页接受 taskId、offset、search，每页最多 200 项。前端不提交命令、包名或路径。RuntimeSnapshot 增加可选 uninstall 字段。clear_history 同时清除卸载与清理摘要。

列表返回 items 和 warnings。单个来源读取失败不能视为完整空列表。计划 ID 仅使用一次，列表刷新使旧卸载计划失效。只读候选不能进入残留计划，未知卸载结果不能启动残留扫描。数据模型与阶段说明见 [软件卸载](software-uninstall.md)。

`InstalledSoftware` 增加两个兼容的描述字段，旧 JSON 缺失均按空数组读取：

- `residual_capabilities: [{kind, inspection, cleanup}]`：kind 为 `cache | settings | local_data`；inspection 为 `fixed_locations | unsupported`，只描述已声明的固定位置，不证明本次安装归属或文件存在。cleanup 为 `unsupported | not_validated | requires_verification`；最后一项仅表示该类别至少一条范围有有效的编译期证据，仍须核验本次真实版本和逐个候选，不能据此启动扫描或删除。
- `missing_information: (publisher | version | estimated_bytes | installed_on | install_location)[]`：后端根据实际未提供字段生成；空白字符串视为缺失，已登记的零大小不视为缺失。它不改变 `can_uninstall` 和 `unavailable_reason`。

这两个字段不包含路径、执行命令或规则授权。能力来源不读取注册表显示版本来授权；商店证据按精确包族匹配。原 `residual_supported` 与 `residual_note` 保留兼容，旧数据不由总开关推断三类能力。描述可随任务摘要保存，但不用于恢复内存范围或可删除候选，无新 IPC 命令或数据库迁移。

商店包的系统签名和 `NonRemovable` 策略独立核对；策略未知时保留列表项并返回 warnings、can_uninstall=false 及 unavailable_reason。准备和实际执行前重新核验当前包身份与策略，不使用列表时的旧允许状态。

商店残留增加私有 `store_isolation` 上下文，绑定当前用户 SID、原完整包身份、包族、系统返回的安装位置和契约版本；只存在 `PreparedSoftware.context` 的后台内存，不写历史、诊断导出或恢复信息，也不接受 IPC 提交。当前契约版本为 2，旧上下文缺字段按 None 读取并保留残留。共享容器、关联包或查询失败使范围捕获/检查停止并返回既有 warnings，不能表示完整空结果；标准卸载能力独立。商店规则范围摘要包含 `current-user-exclusive-store-v2`；LocalCache 规则为版本 4，另外四条为版本 3，命令、白名单和数据库格式均未新增。

商店 LocalCache 可能包含持久应用资料，独立规则类型为 `local_data`、候选 `category=data`，不得在前端依据目录名重新归类为缓存。选择该范围内的候选时，`ResidualCleanupPlan.includes_data=true`，既有资料确认生效；TempState 的缓存分类不变。LocalCache 原版本 3 的缓存分类及范围摘要失效，不继承原许可；当前仍无商店规则删除许可。

安装位置仅供包进程与无包身份子程序的路径核对，不能成为残留删除根。进程仍运行使用既有 `ApplicationRunning` 保留原因，无法核实使用 `ProcessUnknown`；扫描返回相应 warnings 与不完整结果，删除在实际文件操作前拒绝。停止仍使用既有任务 ID 和取消标识，新增系统枚举在条目之间检查取消，不新增前端字段或“结束进程”接口。

残留目录枚举在读取每个条目前后检查既有取消标识。所有根目录共享十万条遍历预算，包括已入队但尚未处理、读取失败及随后保留的条目；停止或预算耗尽立即结束后续枚举，以既有 `warnings` 返回不完整结果。预算耗尽时未处理的路径不进入候选清单，不产生删除授权；已返回目录仍只能在重新核验且为空时删除。IPC、历史和恢复数据形状不变。

`ResidualCleanupPlan` 返回 `id, task_id, files, directories, bytes, includes_data, preview`。preview 最多包含前 20 个实际选中的候选（含路径与分类），用于确认页展示；未展示的数量独立提示，完整选择可返回残留列表检查。目录容量不重复计入 bytes，目录选择仅删除该目录本身且要求为空，不自动选择或递归删除后代。候选及计划在内存中，历史仅保留处理统计。

`UninstallTask` 增加 `can_scan_residuals: boolean` 和 `residual_scan_id: string | null`。前者依据已核对移除状态及仍在内存中的卸载前范围派生，不授予删除权限；后者标识当前残留清单，每次成功扫描生成新的 UUID。`ResidualScan` 增加对应的 `scan_id`。前端仅展示任务 ID、清单 ID 均匹配的分页，清单变化立即清除旧选择、页码和清理确认，丢弃迟到响应；不依赖秒级时间戳或轮询是否观察到扫描中阶段。

历史中的这两个字段固定为 false/null，旧 JSON 缺失时按 false/null 读取，无数据库迁移。不从磁盘恢复候选、清单或目录授权。重开程序可继续核对 `still_installed`、`unknown`、`restart_required` 任务；核对移除后若没有原内存范围，返回不可检查残留的明确提示。请求参数和受理回执协议不变。

软件列表开始刷新即废弃旧软件 ID 和卸载计划，刷新失败也不能沿用。新的卸载准备、残留清理准备会先废弃旧确认，任何校验失败都不能再次执行旧计划。启动卸载或清理前历史写入失败会消费该确认、拒绝启动原生操作；新卸载失败保留之前的任务，扫描/清理失败返回可重试的非活动阶段，避免无工作线程的“进行中”。前端仍按既有受理回执核对，不自动重新提交失败操作。

`UninstallTask.residual` 增加 `unprocessed`，旧 JSON 缺失按 0 读取。它累计已确认批次中尚未尝试的项目数量，不包含未选择的候选，也不计入删除字节数；`skipped` 是已检查但因保护条件拒绝的项目，`unknown` 是结果未能确认的在途项目。清理中断的统计在待核对状态下也展示。历史页按任务身份、阶段、消息和统计变化刷新，不只依赖秒级时间戳。

软件工作线程绑定任务 ID 与一次操作标识，退出停止跟踪后撤销标识和内存残留权限；迟到的进度、核对和扫描结果不能覆盖中断记录。退出中的实例拒绝新操作。线程启动失败或原生平台调用异常中断均进入待核对状态，不自动重试。主动重新核对前也须成功写入阶段记录。

残留执行在每个文件操作前保存仅含 `remaining`（未开始数量）和 `in_flight`（是否存在一项在途操作）的内部进度；保存失败不调用删除。结果保存失败立即停止后续处理。恢复将尚未开始的数量计入 `unprocessed`，仅将确有在途意图的一项计入 `unknown`；旧版清理中断记录没有该进度时保留原来的保守不确定性。该进度不包含路径、候选 ID、命令或可回放清单，不是新的 IPC 参数，也不恢复删除权限。

独立残留上下文更新为版本 2。候选仍只返回既有字段并由 candidate ID 提交；逐条规则 ID、范围摘要、卸载前的目录/文件身份和核验版本只存在后台内存。前端不能提交或修改这些字段。`category` 只用于分组，删除授权依据独立规则与编译期验收记录；未知范围及所有尚未通过实机验收的规则保持 `selectable=false`，并分别说明保留原因。IPC 命令和白名单无新增项，旧历史格式不受该内存版本影响。

编译期残留许可包含 Firefox 签名程序版本 `140.17.0.1158` 的八条规则，其中书签与会话备份另有独立实机证据；对应类别描述为 `requires_verification`，不能将类别说明直接当作 `selectable=true`。实际候选仍须匹配本次卸载前的独立范围和冻结身份。其他版本和其他产品继续只读。验收用的证据采集入口仅存在于 Rust 测试模块，未增加运行时开关、环境变量授权或 IPC 命令。

WPS 新增三条独立固定声明：默认备份（data）和两份加载项设置（data）。私有位置识别器只接受桌面 WPS 身份及内置 Roaming 路径，路径纳入各规则摘要，不接受前端指定或配置文件中的自定义路径；不继承日志规则的能力。新增范围仍按既有 candidate ID、计划 ID 和逐文件身份流程处理，IPC/数据库格式没有改变。数据选择与确认的影响说明包括文档备份。

`UninstallTask` / `UninstallReport` 新增兼容字段 `uninstall_status`，单独保存标准卸载状态：`uninstalling`、`verifying`、`removed`、`still_installed`、`restart_required`、`cancelled`、`failed` 或 `unknown`。`phase` 仍表示当前处理阶段。残留扫描/清理中断不会覆盖已核验的 `removed`；主动重新核对进入 `verifying`，随后按实际安装身份更新。此字段不是残留权限，恢复后即使仍为 `removed` 也不能重建目录授权。旧记录缺失字段时，仅由有明确依据的阶段推断；旧 `unknown` 不推断卸载成功。阶段、标准卸载结果、实际删除统计分别展示。

并发软件列表读取按后台生成的请求代次提交；迟到的旧结果既不能覆盖最新列表，也不能撤销最新确认。最新读取失败时保留空目录和失效确认，旧成功结果不能恢复权限。无需新增前端参数。

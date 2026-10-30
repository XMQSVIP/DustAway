# CPU 文件用途小模型

当前为 v4：原生 Rust 的朴素贝叶斯用途评分器，权重按软件组统计，训练和推理都使用 CPU。模型约 14 KB，无需 GPU、CUDA、Python、PyTorch、ONNX，也不增加 Cargo 或前端依赖。模型编译进应用，在首次需要时加载一次。

## 当前能力

根据已有扫描结果中的目录名、文件名、扩展名和文件/目录类型，推测缓存、日志与诊断资料、临时文件、个人资料、应用设置与数据五类用途。证据不足时不返回分类。应用不读取文件正文、不访问网络、不自动收集用户样本；不会根据文件年龄或大小判定文件无用。另提供显式离线元数据观察工具，见下方流程。

在空间分析中，已有规则和用途解释优先。只有未识别或一般应用数据、完整且未排除的结果才使用模型补充说明。列表显示“可能是…… · 模型推测”，详情展示名称线索及局限。推测不增加可清理容量、不改变自动选择、文件权限、清理规则或删除计划；用途筛选仍按现有识别库工作，模型结果不会被伪装成已验证规则。

## 训练与使用

在仓库根目录执行，使用项目既有 Rust 工具链：

```powershell
pnpm model:train
pnpm model:check
pnpm model:evaluate
pnpm model:validate
pnpm model:bench

# 只分类字符串，不读取或扫描这个路径
cargo run -p dustaway-core --locked --example file_purpose -- predict 'D:\Example\Cache\entry.cache'
cargo run -p dustaway-core --locked --example file_purpose -- predict 'D:\Example\Cache' --directory
# 查看为什么拒绝判断；不读取输入路径
cargo run -p dustaway-core --locked --example file_purpose -- explain 'D:\Cache\Documents\entry.cache'
```

- `model:train` 使用 [seed.json](../models/file-purpose/seed.json) 生成 [model.json](../models/file-purpose/model.json) 和 [evaluation.json](../models/file-purpose/evaluation.json)。
- `model:check` 重新训练并比对模型和报告，检查可复现性，不写文件。
- `model:evaluate` 使用同一 Rust 推理器生成 [validation.json](../models/file-purpose/validation.json)：固定旧回归、66 条 [challenge.json](../models/file-purpose/challenge.json) 反例及跨软件组验证。旧版输出保存在 [baseline-v1.json](../models/file-purpose/baseline-v1.json)，测试集指纹不一致时拒绝比较。
- `model:validate` 检查权重、旧回归报告和完整评估报告均可复现，不写文件。模型权重或标注变更后，先执行 `model:train` 与 `model:evaluate` 更新报告。
- `model:bench` 使用 release 构建对合成路径推理十万次。这个耗时不包含磁盘遍历和模型初次加载，也不代表整盘扫描时间。
- 修改模型后需要重新构建应用。发行程序不加载外部模型，也不在使用期间自动训练。

本机可生成独立命令行程序 `target/release/examples/file_purpose.exe`（构建产物不入库）。执行 `pnpm model:bench` 会先构建它，之后也可以直接运行，无需先执行 Cargo：

```powershell
.\target\release\examples\file_purpose.exe predict 'D:\Example\Cache\entry.cache'
```

原生界面专项检查使用已有 WebDriver 环境和专用夹具，不访问真实文件来验证模型：

```powershell
cargo build -p dustaway --locked --features e2e-fixture,custom-protocol
node scripts/file-purpose-desktop.mjs
```

如果并行开发会替换 `target/debug/dustaway.exe`，先复制夹具程序到独立位置，并将 `DUSTAWAY_MODEL_TEST_APP` 设置为该副本。脚本在扫描前核对夹具运行模式和实际目录，只执行分析，不发起删除。

自定义标注集沿用以下结构，`label` 必须是上述五种英文枚举之一；测试集中可用 `null` 表示应当拒绝判断：

```json
{
  "schema_version": 1,
  "provenance": "说明样本来源、授权和标注方法",
  "samples": [
    {
      "group": "某台机器或某软件家族",
      "split": "train",
      "path": "D:/Example/Cache/item.cache",
      "directory": false,
      "label": "cache"
    }
  ]
}
```

示例只展示字段；完整数据必须覆盖五类、每类至少来自两个软件组，并提供 `split: "test"` 的测试组。同一机器/软件家族应使用相同 `group`，不能跨训练和测试。不要把同一软件不同版本、重复目录或高度相关样本随机拆到两边。工具拒绝跨集合重复的 group 和重复路径（忽略大小写和分隔符）。分组交叉验证时，每个训练折仍须满足这些条件。

```powershell
cargo run -p dustaway-core --locked --example file_purpose -- train dataset.json candidate-model.json candidate-evaluation.json
cargo run -p dustaway-core --locked --example file_purpose -- evaluate test-dataset.json candidate-model.json
cargo run -p dustaway-core --locked --example file_purpose -- cross-validate dataset.json
```

这是显式离线训练入口，不会自动采集路径。未来使用真实样本时先脱敏用户名、账号、客户名和项目名；模型词表可能保留训练路径中的单词，不应直接发布未经审查的私人数据训练产物。正式替换内置模型前检查词表、回归和独立评估，保留应用的全部执行核验。

## 算法与拒绝判断

训练和推理共用 Rust 特征函数：最近四层的**完整目录名**、文件名称中的字词及扩展名，忽略大小写，使用集合去重。目录不再同时当作文件名重复计分；扩展名从文件名词项中剥离，只计一次。`service.log.1`、`service.log.20261025` 保留 `.log` 类型；普通数字后缀不会一概视为日志。不把 `cache-notes`、`not-cache-project` 这些复合目录名拆成缓存线索。

词表要求特征出现在至少两个训练软件组。同一类别、同一特征，每个软件组最多计一次，单个软件的大量重复文件不会主导权重。采用加性平滑（alpha=0.5）、等类别先验；忽略各类别权重差小于 0.5 的无区分力特征。模型 ID 绑定权重、阈值、特征版本与训练指纹；打乱样本行顺序仍产生相同模型。

普通文件至少需要两个有区分力的特征，最高分与次高分的差至少为 2.5；最近两层目录必须提供支持分类的线索。v3 对 `.log` 及已支持的数字轮转日志增加一个窄例外：最多六层目录参与特征提取，只有结论为日志时才允许使用这六层的日志上下文。因此 `logs/日期/window1/exthost/output/服务.log` 可以被解释，而缓存文件和其他扩展名仍保持原近层要求。所有保护与冲突检查继续执行，普通目录不会自动继承祖先日志类别。

目录只使用一次自身名称，其分差门槛为 1.0，并要求目录自身的特征支持结论。父目录叫 Cache 不能使一个未知子目录获得缓存标签。

所有祖先目录中的已学习个人资料/应用数据线索都参与冲突检查，但远层目录不会增加分类得分；资料上下文冲突时拒绝缓存、日志和临时文件推测。超过 8 KiB 或 128 段路径、含控制字符、父目录跳转、备用数据流及歧义末尾字符时拒绝判断。浏览器持久存储、聊天数据库、LocalCache、源码和 Git 等布局拒绝预测；文档、媒体、数据库、配置、源码、程序和密钥等扩展名阻止缓存/日志/临时文件推测。这些拒绝条件只减少建议，不构成完整的敏感数据检测器。

v4 增加完整目录名 `leveldb` 的敏感位置保护；`.ldb`、`.sst`、`.dbtmp`、`CURRENT`、`LOCK`、`MANIFEST-数字` 及纯数字文件名的 `.log`（含 `.log.N` 轮转）不能获得缓存/日志/临时用途提示。LevelDB 的数字日志可能保存数据库更新，不能按普通诊断日志处理。纯数字命名的正常诊断日志也会因此拒绝判断，这是仅靠元数据无法消除歧义时的保守取舍。`service123.log`、`service.log.17` 等仍正常识别；不把 `leveldb-notes` 当作 `leveldb`。

CLI `explain` 和评估报告区分路径无效、敏感位置、特征不足、分数接近、缺少近层目录依据、上下文冲突、保护扩展名和数据库文件保护八种拒绝原因。正常应用 IPC 仍只返回兼容的用途提示，拒绝时省略提示，不增加新的前端权限。

`score_margin` 不经过概率校准，不能解释为“可删除概率”或“准确率”。界面不展示百分比。

## 样本与局限

当前是实验性种子模型：225 条人工编写的合成训练记录，来自九个虚构产品组；原有 56 条回归记录保持不变，另增加 66 条中文目录、日志轮转、冲突和保护场景。没有读取本机真实文件来训练。测试组仍与训练组共享命名习惯，且特征、平滑参数及拒绝条件使用开发评估选择，因而这些结果都不是独立泛化准确率。

原有 56 条回归中，正确分类由 v1 的 33 条增加至 39 条，17 条拒绝（包括全部 16 条未知/保护项）。新增 66 条回归中，正确分类由 21 条增至 30 条；错误的缓存/日志/临时文件推测由 10 条降至 0 条，全部 33 条未知/冲突项拒绝，其余 3 条有类别但证据不足。

九折按软件组留出验证只使用训练记录，每折重新构建词表和权重；原 test 集不参与。225 条记录中 173 条给出正确分类、52 条拒绝，覆盖率 76.9%。该结果仍是开发指标，不能宣传为“100% 准确率”。未做多机器真实数据评估、真实用户清理验收或概率校准。

训练样本、模型和评估结果均随源码保留，采用仓库 MIT 许可。后续优先补充经人工核对的真实脱敏样本，保留完全未参与开发的独立测试组，再决定是否调整算法。

v4 保留原有合成回归结果与词权重，更新特征版本和模型 ID。新增 25 条数据库边界合成回归中，v3 的 9 条不应给出的提示全部被拦截，10 条正常对照保持分类。这里的保护样本空标签表示“应拒绝提示”，不是逐文件用途真值。[本轮验证](verification-file-purpose-model-v4.md)记录具体结果与来源辅助标注局限；[v3](verification-file-purpose-model-v3.md)、[v2](verification-file-purpose-model-v2.md)和[v1](verification-file-purpose-model.md)记录保留为历史结果。

随后完成的[140 条真实样本逐项复核](verification-file-purpose-model-review.md)仍采用冻结 v4：122 条有来源辅助用途依据，其中 33 条模型提示一致、89 条拒绝判断；另 18 条依据不足，模型提示对错未确定。未确认项从用途一致性统计中排除，不能将 `null` 当作已确认未知或自动计作误判。模型和训练数据未因这轮复核而改变。

## 本地真实元数据观察与盲标

`crates/windows/examples/file_purpose_observe.rs` 复用生产 `WindowsAnalysis` 的只读遍历、目录句柄和链接保护。必须显式提供应用子目录；不自动扫描整盘或整个个人目录。每个根最多 300 条记录、3,000 条检查，并在事件回调处检查 15 秒预算。时间限制属于协作式限制，不能中断已经阻塞的系统调用。输出目录必须全新，保留部分扫描、读取失败和达到上限的状态。

在 `artifacts/` 下创建 `manifest.private.json`，字段如下（路径仅作格式示例）：

```json
{
  "schema_version": 1,
  "machine": "machine-01",
  "roots": [
    {
      "group": "software-family-a",
      "partition": "development",
      "path": "D:/ExampleApp/logs",
      "reference": "logs",
      "reference_basis": "填写独立的目录来源依据，仅是范围参考，不是逐文件真值"
    }
  ]
}
```

同一软件家族所有版本必须归到同一 `group`，`partition` 选择 `development` 或 `holdout`。根不能重复或嵌套，分组不能跨两种分区。`reference` 支持五类用途或 `unknown`，仅用于范围一致性统计，工具不据此生成标签。新版采样器不对 holdout 条目进行预测。

```powershell
cargo run -p dustaway-windows --locked --example file_purpose_observe -- artifacts/manifest.private.json artifacts/new-observation
node scripts/file-purpose-observations.mjs report artifacts/new-observation artifacts/coverage.json
node scripts/file-purpose-observations.mjs review artifacts/new-observation artifacts/review.private.json
# 在同一快照上评估当前源码模型，只推理 development 部分
node scripts/file-purpose-observations.mjs rescore artifacts/new-observation artifacts/new-candidate-observation
```

私有观察文件包含路径、类型、大小与修改时间，不含正文。模型结果另存，以便盲标。模板按软件组、范围和文件/目录类型各抽最多四条，选择不依赖模型输出，不展示模型答案或范围参考。所有标签初始均为 `pending`；`label: null` 只有在明确标记 `reviewed`、填写 `reviewer`、`method` 和独立 `basis` 后才表示“已复核但无法确认用途”。不要把模型答案或范围参考复制成标签。

`method` 必须明确为 `human`（实际人工复核）或 `source_metadata`（来源辅助的元数据推断）。后者需要记录来源与具体结构依据，不能称为人工真值或用于宣称真实准确率。旧复核文件也须补填方法。导出 provenance 保留方法计数与完整复核文件的 SHA-256，以便追溯；不改变 Rust 评估数据接口。CLI 的分类一致率仅相对于提供的标签，是否独立、是否可信需按 provenance 判断。

```powershell
# 只导出已独立复核的记录；导出为评估集，不自动训练
node scripts/file-purpose-observations.mjs export artifacts/new-observation artifacts/review.private.json development artifacts/reviewed.private.json
cargo run -p dustaway-core --locked --example file_purpose -- evaluate artifacts/reviewed.private.json
# 模型和参数冻结后，才准备独立的留出标注
node scripts/file-purpose-observations.mjs review artifacts/new-observation artifacts/holdout-review.private.json --holdout
```

每个复核文件绑定原始观察指纹；不接受混合分区、重复/未知 ID、没有独立依据或复核方法的标签。导出始终从原快照取路径，忽略复核文件中修改过的路径。覆盖率报告不包含原始路径、不计算准确率，未复核记录不会被当作正确拒绝。查看留出结果前冻结模型、推理程序、标签及各自指纹；查看后该子集已消耗，进一步调参时不能继续当作全新独立测试。现有 `.gitignore` 排除 `artifacts/` 和 `*.private.json`；发布数据前仍须独立脱敏和检查。当前一台机器、同一 Electron/编辑器生态的样本不能代表所有 Windows 软件。

# Release 分析对比夹具

## 当前百万级分析验收

当前 harness 使用磁盘临时索引，默认上限 500 万条；`--comparison` 降到旧版的 50 万条。基线使用实施前备份中的 harness 和核心代码，新接口只在新版本附加测量，共同扫描起止、原有查询和内存取样口径保持一致。以下旧版段落中的“同一 harness”不适用于新增磁盘接口之后的源码。

`seed-million.mjs` 在新的专用根目录生成 300 万个 7 字节文件，其中 `3000000/first-million` 为百万文件子集。它拒绝复用已有目录，不删除夹具。该负载主要验证元数据吞吐和索引规模，不模拟真实文件大小分布。

```powershell
node scripts/analysis-benchmark/seed-million.mjs F:\DustAway-tests\dustaway-analysis-bench-million-v1
cargo build --manifest-path scripts/analysis-benchmark/Cargo.toml --release --locked --target-dir target/bench-million
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/analysis-benchmark/run-million.ps1
python scripts/analysis-benchmark/summarize-million.py
```

运行器内记录本机基线、新版和两组夹具位置，迁移到其他机器时先调整路径。原始目录不可覆盖；重复测量使用 `-Output` 指定新目录。旧版基线来自本轮验证过的备份，路径和哈希见[验收记录](../../docs/verification-analysis-million.md)。

本机的最终配置测量通过 `-IndexDirectory` 使用 D 盘 NVMe 索引，扫描夹具仍在原 F/G 机械硬盘。该参数只影响测试 harness，不修改应用设置。harness 接受进程环境变量 `DUSTAWAY_BENCH_INDEX_ROOT`：路径须绝对、末级目录名以 `dustaway-analysis-cache-` 开头，并预先放置内容精确为 `readonly-analysis-benchmark-index` 的 `.dustaway-test-only` 文件；不设置时仍在夹具根目录下创建索引。默认结果为 `formal-r3`；目录索引修复前的 SSD 完整序列保留在 `formal-ssd`，同一机械硬盘存储扫描数据和索引的受干扰记录保留在 `formal-final`，不可混合统计或宣称同盘目标全部达标。

10 万及 50 万负载各五轮交替，百万及三百万负载各三次完整运行，随后测量扫描、索引构建、扫描中查询三种取消。后端普通页目标 250 ms，名称包含查询首屏 2 s，取消 1 s，私有提交峰值 900 MiB，含 WAL 的会话索引 4 GiB。汇总脚本保留所有失败项，不把目标写成实测结果。

`cold_query` 单独记录扫描完成后第一次新接口页面查询，包含保护检查的首次初始化；它仍不等于冷盘或应用启动耗时。之后执行原有查询，最后测量 `read_queries` 中的 18 个新分页样本，因此这组样本已经预热用途保护检查及部分数据库页，不代表第一次查询或 UI 绘制。`discovery` 的有效搜索词来自嵌套查询，本轮为空；R3 日志已按有效值记录，更早日志的外层 `search` 可能为 `099`。R3 在计时结束后另存 `directory_query_plans`，对照自动选择和显式目录覆盖索引的查询计划，诊断连接在清理前关闭。私有提交包括读取阶段，索引磁盘峰值为 250 ms 采样值，不能证明瞬时严格上界。

## 查询耗时诊断

查询延迟诊断使用 `--features query-timing` 构建 benchmark，将连接、SQL、结果组装及资源释放的分段耗时缓存在内存，结束后统一写到标准错误；记录上限为 4,096 条。桌面默认构建不开启这个 core feature。`--query-rounds=5` 在一次扫描后重复五组查询。输出交给 `summarize-query-profile.py` 汇总，诊断结果与正式验收分开保存。

```powershell
cargo build --manifest-path scripts/analysis-benchmark/Cargo.toml --release --locked --features query-timing --target-dir target/bench-million-profile
# 设置前述专用索引位置后，将标准输出保存为 JSON、标准错误保存为 JSONL。
# 向基准命令附加 --query-rounds=5。
python scripts/analysis-benchmark/summarize-query-profile.py artifacts/analysis-million/query-profile-3m.jsonl
```

## 既有版本的测量方法与记录

此工具只读专用标记目录。需要根目录名以 `dustaway-analysis-bench-` 开头，标记 `.dustaway-test-only` 内容精确为 `readonly-analysis-benchmark`，并有 `100000` 和 `500000` 子目录。每个子目录按每组 1000 文件创建 100 / 500 组，每文件 7 字节 `fixture`。夹具包含目录节点，所以 50 万文件负载会达到 50 万条索引上限，应为 partial。

在空间充裕的测试盘执行 `node scripts/analysis-benchmark/seed.mjs G:\test\dustaway-analysis-bench-demo` 创建夹具。共 60 万文件，元数据也占用空间；脚本拒绝已有目录，不自动删除旧夹具。

`cargo build --manifest-path scripts/analysis-benchmark/Cargo.toml --release` 后运行：

```powershell
.\scripts\analysis-benchmark\target\release\dustaway-analysis-bench.exe G:\test\dustaway-analysis-bench-demo 100000
.\scripts\analysis-benchmark\target\release\dustaway-analysis-bench.exe G:\test\dustaway-analysis-bench-demo 500000 --cancel
```

可以设置 `CARGO_TARGET_DIR` 复用构建缓存。对旧版测量时复制此 harness 到归档外独立目录，将两个 path 依赖指向已经解压并验证的旧版源码，不修改归档源码。两版必须使用相同的 harness、编译选项、工具链、夹具与机器。

顺序交替运行旧/新版本各五次，不同时编译、运行其他测试或种植夹具。索引查询 10 次交替查询全部文件与名称包含 `099` 的文件，每次最多 200 项。取消在出现首批条目后请求，记录到终态的时间。峰值为进程私有提交高水位（GetProcessMemoryInfo 的 PeakPagefileUsage），包含查询阶段，不代表整个桌面应用总内存。`first_entries_ms` 仅首批条目可见时间，不代表整盘完成。

记录全部样本与环境、两版 EXE 哈希，展示中位数及范围；系统磁盘缓存未清空，结果不代表冷启动或其他硬件的整盘速度。不会删除夹具。

0.11 起同时记录根层目录分页（当前页及后两页各轮换）、50 万条索引构建取消 `--cancel-index`、至少 25000 条时的活跃查询 `--query-during`、查询后 2 ms 发起取消 `--cancel-query`。后两项必须检查 during_query.ok 和终态，不能将查询已结束后的取消冒充查询中断。完成后的查询不受已置位取消标记影响。取消目标 500 ms 是专用本地夹具目标，不保证所有设备。

附加负载：1 万个同层目录各一个 7 字节文件；96 层中文长路径各一文件，另有 1000 条指向同一 7000 字节文件的硬链接。Windows 长路径创建应使用原生扩展路径，不改变系统全局设置。分别记录第一次目录查询与后续分页、逻辑大小与去重大小。

0.11 本机原始运行器、夹具参数与 55 条记录保存在 artifacts/space-0.11；对应汇总、性能目标及结果见 docs/verification-0.11.md。在同一机器交替比较恢复的 0.10 与 0.11，不与编译、夹具种植、桌面测试并行；WebView2 成本另列，不混入分析 harness 的私有提交峰值。

## 0.16 用途查询对照

启用 `--features discovery` 后，同一个harness在终态额外记录12次目录/文件用途查询；JSON的`discovery_queries`保留条件、条数及微秒耗时。共同的扫描与旧查询代码不变；基线不启用该feature。此工具只读标记夹具，不创建清理计划。

旧分析实现从实施前归档恢复到独立目录，并复制同一份当前`rules/recognition.json`和`crates/core/src/recognition.rs`，控制规则库及匹配变化。归档原件不改动。基线harness依赖该副本，使用独立target目录；每次记录两个EXE、规则和匹配器哈希。十万/五十万夹具各交替五轮，大量目录和深层/硬链接各三轮，三种取消各三轮。测量期间本任务不编译、不种植、不执行其他测试；其他主机活动及缓存影响如实说明。

```powershell
cargo build --manifest-path scripts/analysis-benchmark/Cargo.toml --release --features discovery --locked --target-dir target/bench-016
& ./target/bench-016/release/dustaway-analysis-bench.exe (Resolve-Path ./artifacts/dustaway-analysis-bench-0.10).Path 500000
```

完整41轮结果、基线准备方法及环境见[0.16验收](../../docs/verification-0.16.md)和`artifacts/discovery-0.16/performance.py`；不将首次查询、吞吐和取消延迟混作同一指标。页索引限额不是全进程内存硬上限，WebView资源另列。

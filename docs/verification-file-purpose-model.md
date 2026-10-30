# CPU 用途模型验证（2026-10-25）

范围：原生 Rust 模型、离线训练工具、空间分析分页时的只读用途提示和现有详情展示。应用版本仍为 0.24.0，不增加删除许可。仓库原有的缓存、发现与历史日志开发改动保留。

## 模型与性能

- 模型：`purpose-nb-v1-276ec3a0d2a4`，JSON 18,412 字节，113 个词特征、五类输出。
- 144 条合成训练记录，56 条开发回归记录。33 条正确分类、23 条拒绝判断；后者包含 16 条未知/保护记录和 7 条有类别但证据不足的记录。
- 这些开发样本不构成独立真实准确率测试。未采集私人文件、未做多机器真实数据评估，未验证任何新增清理能力。
- 本机 Intel Core i5-10400 CPU @ 2.90GHz；debug 训练实测约 6.52 ms，release 十万次推理约 357.84 ms（约 3.58 µs/条）。均为单次测量；推理基准不含磁盘遍历和首次模型加载。
- 独立 release 命令行程序 576,512 字节；模型随程序嵌入。无需 GPU 或额外模型运行时。

## 已通过

- `cargo test -p dustaway-core --locked`：167 项通过，2 项显式资源基准按既有设置忽略。包括新模型预测、拒绝、损坏模型、旧接口兼容、以及结果进入分析列表但没有清理资格的测试。
- `cargo test -p dustaway-core --locked --example file_purpose`：2 项通过；重复训练一致，测试样本不进入训练权重，同组跨集合及重复路径被拒绝。
- `pnpm model:check`：重新训练的权重与开发评估文件均一致。
- 受影响前端：分析页面、用途决策、已有识别提示、新模型提示及详情，共 72 项通过；不显示概率、不增加选择权限、已识别及保护结果优先。
- 全量前端 `pnpm exec vitest run --reporter=verbose`：33 个文件、388 项通过。逐用例报告便于观察较长的应用测试文件进度。
- `pnpm build`：通过。已有前端单块体积超过 500 KB 的提示仍存在；本次没有新增前端依赖。
- `cargo fmt --all --check`、`cargo clippy -p dustaway-core -p dustaway-windows --locked --all-targets -- -D warnings`：通过。
- `cargo test -p dustaway-windows --locked --lib analysis_purpose`：2 项通过，Windows 标准用途映射保持。
- release CLI：`D:\Example\Cache\entry.cache` 返回缓存用途，`D:\Example\data.bin` 返回 `null`（未知），不读取给定路径。
- 原生桌面专项 `node scripts/file-purpose-desktop.mjs`：通过。三个新建的隔离文件验证真实 IPC 中的模型类别、未知/保护样本拒绝推测、详情中的模型提示和依据、没有自动选择、原有自选资格保持、三个文件内容不变；未执行删除。结果及截图位于 `artifacts/file-purpose-desktop-20261025/result.json`、`model-details.png`。本次使用复制到 `artifacts/file-purpose-isolated/dustaway-model-e2e.exe` 的夹具程序，并通过 `DUSTAWAY_MODEL_TEST_APP` 指定；脚本使用独立驱动端口，等待启动与扫描状态完成，必要时通过现有“重新检查状态”恢复界面状态。
- 原生检查后将依据中的相同单词分别标明“目录名含”“名称含”“扩展名”。最终 `pnpm build` 通过；新模型组件 4 项测试使用 `--pool=forks --maxWorkers=1` 通过（默认线程池的一次重跑发生 worker 启动超时，没有执行测试）。

## 完整桌面回归的未完成项

普通 `pnpm test:e2e` 对应的完整桌面流程没有全部通过：第一次缩小夹具缺少脚本依赖的“目录 000”；随后标准十万文件夹具完成生成，但窗口尺寸检查因测试窗口状态不符合调整条件而中断（`Restore the test window before resizing it`）。未修改全局窗口检查或其他清理行为来规避此限制。以上原生模型专项已经独立通过，但不能替代完整桌面回归、真实用户数据上的模型评估或新增清理验收。

完整训练与使用说明见 [CPU 用途小模型](file-purpose-model.md)。

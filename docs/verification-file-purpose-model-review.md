# v4 真实样本逐项复核

本轮已完成既有 140 条抽样记录的来源辅助复核，沿用冻结的 `purpose-nb-v4-e8149b38a69a`，没有修改模型或训练数据。这里的复核由 Codex 根据官方实现和冻结元数据完成，不能称为独立人工标注或正文核验。

## 结果

| 复核结果 | 开发分区 | 留出分区 | 合计 |
| --- | ---: | ---: | ---: |
| 已逐项检查 | 96 | 44 | 140 |
| 有来源辅助用途依据 | 84 | 38 | 122 |
| 上述记录中模型给出提示 | 25 | 8 | 33 |
| 提示与临时用途标注一致 | 25 | 8 | 33 |
| 提示与临时用途标注冲突 | 0 | 0 | 0 |
| 有用途依据但模型拒绝判断 | 59 | 30 | 89 |
| 依据不足，未确定用途 | 12 | 6 | 18 |

122 条临时用途标注包括 20 条缓存、41 条日志、61 条应用数据。140 条中包含 81 个文件和 59 个目录。来源辅助依据支持的 33 条预测均一致；89 条拒绝不是正确分类，其中 66 条属于既有持久存储位置保护。

其余 18 条均有模型提示，但目前无法判定这些提示对错，**没有把它们计作正确或错误，也没有将空标签当作已确认未知**。整体真实准确率保留为 null，不能从 33 条一致推导出“100% 准确”。

## 复核中确认的区别

- Chromium 的缓存目录中，`index`、`data_0` 至 `data_3`、十六进制 `f_` 文件与记录中的同目录结构吻合。它们可被临时标注为缓存用途，但索引与内容存在关联，不能据此授权逐文件删除。[Chromium blockfile 格式说明](https://raw.githubusercontent.com/chromium/chromium/144.0.7559.236/net/disk_cache/blockfile/disk_format.h)
- `Code Cache/js`、`wasm` 和 `index-dir/the-real-index` 与代码缓存实现相符；`electron-preload/<散列>.cache` 有 Electron 已合并实现作为依据。本次只核对元数据布局，没有验证本机安装版本或文件头。[Chromium 代码缓存](https://raw.githubusercontent.com/chromium/chromium/144.0.7559.236/content/browser/code_cache/generated_code_cache_context.cc)、[缓存索引](https://raw.githubusercontent.com/chromium/chromium/144.0.7559.236/net/disk_cache/simple/simple_index_file.cc)、[Electron 预加载缓存实现说明](https://releases.electronjs.org/pr/51602)
- VS Code 的会话、窗口、扩展和输出通道日志布局有上游实现支持。对 Trae、Kiro、WorkBuddy 的对应项采用结构推断，保留“未验证各厂商实际实现”的限制。[会话日志路径](https://raw.githubusercontent.com/microsoft/vscode/1.100.0/src/vs/platform/environment/common/environmentService.ts)、[扩展输出通道](https://raw.githubusercontent.com/microsoft/vscode/1.100.0/src/vs/workbench/api/common/extHostOutput.ts)
- Cherry Studio 两条隐藏的 `.<散列>-audit.json` 与日志轮转组件维护的状态文件命名相符，临时标注为应用数据；同目录的 `app.<日期>.log` 和 `app-error.<日期>.log` 标注为日志。这避免把根目录参考用途直接复制给所有子文件。两条 audit 文件的模型输出均为拒绝判断。[Cherry 日志实现](https://raw.githubusercontent.com/CherryHQ/cherry-studio/v1.8.1/src/main/services/LoggerService.ts)、[轮转状态文件说明](https://raw.githubusercontent.com/winstonjs/winston-daily-rotate-file/v5.0.0/README.md)

## 仍不能确认的 18 条

| 范围 | 数量 | 模型提示 | 缺少的依据 |
| --- | ---: | --- | --- |
| Trae、Cherry Studio、Claude、Kiro、WorkBuddy 各自的 Cache / Code Cache / GPUCache 根目录 | 15 | 缓存 | 采样完成时目录没有子项；只有通用名称，缺少具体安装或内容结构的独立佐证 |
| Trae Modular 下的一条 stdout 日志样式文件 | 1 | 日志 | 未找到对应专有子系统实现，只有路径与名称线索 |
| Claude 的 logs 目录及 main.log | 2 | 日志 | 本轮未建立该具体产品路径与官方实现的对应关系 |

这 18 条是“未确认”，不是“已发现误判”。不能为了让评估数字更好就按模型答案补标签，也不据此立即调整分类阈值。后续若继续验证，应优先取得这些具体位置的厂商依据、安装归属或隔离环境中的行为证据。

## 范围与可追溯性

本轮补完了此前 74 条待复核项，保留此前 66 条存储记录的来源辅助标注。样本仍来自同一台机器、相关 Electron/编辑器生态，并按应用/范围/文件或目录分层抽样；部分日志采集曾达到条数上限。因此不能推广为全盘或跨软件生态的准确率。

在核对本轮模型输出前，保存了完整复核文件、官方源码快照及 SHA-256；模型、推理源码和 release 程序与上一轮冻结值完全一致。留出分区本轮额外查看 22 条，共查看 44 条；原留出快照另 387 条未由 v4 推理。已查看的 44 条不能在后续调参后继续称为全新独立测试。

新增记录位于本机 `artifacts/file-purpose-full-review-20261029/`：

- `development-review.private.json`、`holdout-review.private.json`：逐条复核、用途或不确定性、来源和对应元数据 ID。
- `candidate-lock.json`、`review-lock.json`：模型、程序、样本、复核与来源快照的指纹。
- `review-details.private.json`、`review-details.private.md`：逐条与冻结模型输出对照的清单。
- `summary.json`：不含原始路径的计数汇总。
- `sources/`：成功取得的官方源码快照、地址、读取时间和指纹；失败的地址单独记录，不用于标注依据。

报告生成时检查了 140 个唯一 ID、预测完整性、所有分组计数、冻结文件指纹和已用源码快照指纹。原始路径文件保持 Git 忽略；没有读取用户文件正文、修改应用文件或执行清理。没有改动推理代码，因此本轮未重复构建或运行桌面回归。

本轮的结论是：现有来源辅助标注中没有发现相冲突的提示；模型仍较保守，同时有 18 条提示需要更强依据。建议保持 v4，优先补这 18 条的证据与不同机器样本。

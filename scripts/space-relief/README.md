# 0.18.0 空间复核验收脚本

仅用于已授权的 `DESKTOP-24NJTT6\MSN` 闲置测试机。固定标记目录为 `DustAwayAcceptance/20261002-space-relief`；没有标记、已有输出或已派发任务时拒绝运行。生产程序不包含这些脚本。

当前入口是 `Bundle.ps1`、`Prepare-Remote.ps1`、`Dispatch.ps1`。`Dispatch.ps1` 使用严格 SSH 主机校验，通过现有交互会话的 Limited 计划任务运行；不修改执行策略、安全设置或网络。`real` 只处理三项原有开发缓存，`uv` 单独使用官方整理，`chrome` 逐项核对生产界面与重开行为，`matrix` 使用专用夹具。

本轮任务已经执行，不能再次派发来覆盖结果。断线后使用 `Inspect-Remote.ps1` 查询，完成后使用 `Collect-Remote.ps1` 收集。所有文件删除均由生产程序的已核验计划执行；测试脚本不绕过确认或提交任意路径。

版本化的 `Run-Remote-v2/v3/v4.ps1`、`real-cache*.ps1`、`uv-real-v2.ps1`、`uv-postcheck*.ps1` 及 `Dispatch-Postcheck*.ps1` 保留修复过程。实际运行源文件与证据一同归档；不要把早期版本作为新测试入口。当前默认脚本已修复 PowerShell 5 数组错误判断与 Python 参数引号问题，并拆分普通缓存和 uv 两个阶段；这些整理后的入口只完成语法检查，没有重做已经完成的删除。

`uv-postcheck-v2.ps1` 仅重新核对原有保护快照并运行环境/工具启动检查，不调用整理接口。首次只读补查的 JSON 数组包装错误保留在 `uv-postcheck/failure.json`；最终有效结果在 `uv-postcheck2/result.json`。

本轮完整状态、失败原因、测试边界及程序哈希见 `docs/verification-space-relief.md` 和 `artifacts/space-relief-20261002/final-summary.json`。

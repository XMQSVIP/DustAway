# uv 官方整理接口

用户明确授权将 uv 接入独立 `cache prune` 操作。本扩展不改变既有逐文件计划、报告或数据库。

| 命令 | 参数 | 结果 |
| --- | --- | --- |
| prepare_uv_cache | CommandRequest、当前 session | 一次性 UvPlan（固定缓存、工具、120 秒有效期） |
| execute_uv_cache | CommandRequest、planId | 受理任务 ID；计划在启动前消耗 |
| cancel_uv_cache | CommandRequest | 停止本程序启动的 uv 任务，不结束用户原有进程 |
| get_uv_task | 无 | 当前任务状态 / 结果，只读 |
| get_uv_history | 无 | 独立 JSON 记录，只读 |

写命令使用既有 ControlState 的运行实例、请求版本、一次性请求和回执核对。重复 execute 请求返回旧回执，不再次启动；不同请求引用同一计划也会被拒绝。重新连接只读状态，不重放动作。共享互斥覆盖清理、分析和卸载。clear_history 同时清除 uv 记录，不删除缓存。

UvReport 保留命令说明、缓存位置、开始/结束时间、状态、工具输出、前后逻辑大小和磁盘可用空间；不伪造逐文件删除数量和实际释放字节。启动意图在进程启动前落盘并同步，终态写入独立新文件；崩溃后只显示待核对。

路径和命令参数全部由后端固定解析。前端不能提交绝对路径、额外参数、shell 命令或清理程序路径。具体位置与保护边界见 [开发工具覆盖](developer-cache-coverage.md)。

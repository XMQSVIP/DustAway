# 开发工具缓存覆盖

当前源码取消应用版本白名单，按实际身份、布局、格式或官方命令能力核验；以下版本数字保留为历史样本。未开放规则不因此开放，详见 [兼容性策略](cleanup-compatibility.md)。

本轮扩展为 37 项开发工具检查：18 项逐文件清理规则、4 项固定官方工具操作、15 项只读共享缓存检查。全部默认不勾选；未发现文件的工具不展示，卸载后仍存在的缓存继续展示。规则不以目录存在推断软件已安装。

| 语言 / 工具 | 可执行的处理 | 保留内容 |
| --- | --- | --- |
| JavaScript / TypeScript：npm | 30 天以上的内容寻址缓存、7 天以上日志 | node_modules、全局工具、临时写入、配置 |
| pnpm 10 常规 v10 存储 | 本轮 pnpm 10.30.3 常规存储出现项目登记链接，保护适配受阻，保持只读 | 项目依赖、未适配的全局虚拟存储 |
| Yarn Classic 1.x | 固定 `yarn cache clean` 清空全局 v6 缓存，后续安装可能重新下载 | Berry、PnP、项目 .yarn、配置 |
| Yarn Berry、Bun、Deno | 只读缓存占用，尚未适配共享存储处理 | 项目依赖、PnP 包、运行环境 |
| Python：pip | 30 天以上的 HTTP / wheel 缓存 | 解释器、虚拟环境、已安装库 |
| Python：uv | 单独确认后执行官方 `uv cache prune` | 普通项目 .venv、已安装工具、解释器；缓存中的集中式项目环境可能被清除并按需重建 |
| Poetry、PDM | 只读下载与元数据缓存 | Poetry virtualenvs、PDM packages 共享安装与虚拟环境 |
| Conda / Mamba | 30 天以上的 .conda / .tar.bz2 下载包 | 已解包依赖、环境、索引、锁 |
| Rust：Cargo、Rustup | 30 天以上的 .crate 下载包、完整哈希下载文件 | 工具链、Cargo bin、registry/src、Git 源、项目 target、凭据 |
| Go 1.21+ / 1.x | 固定 `go clean -cache`，下次构建变慢；模块下载仍只读 | 工具链、项目、模块、已安装命令、模糊测试语料 |
| Java / Kotlin：Gradle、Maven | Gradle 7 天以上 daemon 日志；共享构建缓存和 Maven 仓库只读 | wrapper、JDK、项目构建、可能无法重新获取的本地发布构件 |
| .NET：NuGet | 7 天以上 HTTP 缓存；全局 packages 只读 | SDK、全局共享包和项目依赖 |
| C / C++：vcpkg | 30 天以上哈希 zip 二进制缓存 | installed、源码、SDK、构建树 |
| ccache 4.7+ / 4.x | 固定 `--evict-older-than 30d`，按工具最近使用语义整理 | 近期缓存、配置、项目 |
| sccache、Conan | 只读共享编译和包缓存 | 工具维护的数据、项目、配置 |
| PHP：Composer | 30 天以上下载 zip 包 | vendor、全局工具、仓库元数据、认证配置 |
| Ruby：RubyGems、Bundler | 30 天以上 .gem 下载包；Bundler 用户缓存只读 | 已安装 gems、扩展、项目 vendor/cache、配置 |
| Dart / Flutter：Pub | hosted / git 共享包只读 | 全局命令、Flutter SDK、项目、构建目录 |
| VS Code / Insiders、Cursor | 7 天以上运行缓存和日志 | 扩展、设置、聊天、工作区、历史、未保存备份 |
| JetBrains IDE、Android Studio | 已识别版本中的 7 天以上日志 | SDK、模拟器、索引、本地历史、项目 |
| Visual Studio | 已识别实例的 ComponentModelCache 只读 | 扩展、设置、组件、项目、恢复数据 |

React、Vue、Angular、Next.js 等使用所属包管理器缓存；Pipenv / Hatch 的 pip 或 uv 缓存通过对应规则识别。CMake / Ninja 的项目构建目录、Git 仓库、Docker 镜像与卷、WSL 文件系统和任何 SDK 不属于本轮自动清理。不会递归搜索全部项目后删除依赖或构建目录。

## 位置和容量

通过 Windows Known Folders 解析 LocalAppData、RoamingAppData 和用户目录。支持表内工具的标准位置；扩展识别 `CARGO_HOME`、`RUSTUP_HOME`、`COMPOSER_CACHE_DIR`、`GEM_HOME` / `GEM_PATH`、`CONDA_PKGS_DIRS`、`VCPKG_DEFAULT_BINARY_CACHE`、`POETRY_CACHE_DIR`、`PDM_CACHE_DIR`、`GOCACHE` / `GOPATH` / `GOMODCACHE`、`GRADLE_USER_HOME`、`SCCACHE_DIR`、`CCACHE_DIR`、`BUN_INSTALL_CACHE_DIR`、`YARN_CACHE_FOLDER`、`BUNDLE_USER_CACHE`、`NUGET_PACKAGES`、`CONAN_HOME`、`PUB_CACHE` 和 `DENO_DIR`。这些是程序启动时继承的环境配置，修改后须重启并重新扫描。不会自动执行包管理器来发现路径；未枚举所有项目级或工具私有配置文件。

uv 位置优先使用 `UV_CACHE_DIR`，其次读取 `%APPDATA%/uv/uv.toml` 中的绝对 `cache-dir`，最后使用 `%LOCALAPPDATA%/uv/cache`。uv.exe 支持用户 `.local/bin`、`.cargo/bin` 和 LocalAppData/Programs/uv 的实体文件；未安装工具时缓存仍可查看，整理不会自动安装软件。

共享缓存仅统计安全读取的元数据；同一次扫描中按文件身份去重，允许观察硬链接和只读文件，不改变删除边界。只读发现量不进入可选、已选和逐文件清理报告。uv 结果分别记录工具输出、前后缓存逻辑大小和磁盘可用空间；逻辑大小差值不当作物理释放量。

## uv 操作边界

只接受后端产生的一次性计划标识；不接受路径、命令或参数。准备时绑定当前扫描、目录身份、父目录和工具文件身份，确认有效期为两分钟。执行前复查配置、运行进程、排除项、链接 / 云文件和读取完整性。缓存标记必须有效。禁用项目配置和联网，不使用 `--force` 或 `--ci`。

任何排除项与缓存整体重叠都会阻止整理。uv / Python 正在运行或运行状态无法确认时阻止启动，不强杀用户应用。用户停止或超时仅结束拂尘启动的工具进程；Windows Job Object 约束该进程。开始意图和结果写入独立 JSON 记录，不迁移数据库，不改变既有清理历史格式。崩溃后未完成意图显示“待核对”，不续跑。

## 依据

- [uv 缓存与安全约束](https://docs.astral.sh/uv/concepts/cache/)
- [Cargo Home](https://doc.rust-lang.org/cargo/guide/cargo-home.html)
- [Conda clean](https://docs.conda.io/projects/conda/en/latest/commands/clean.html)
- [pnpm store](https://pnpm.io/cli/store)、[Bun 全局缓存](https://bun.sh/docs/pm/global-cache)、[Yarn Classic cache](https://classic.yarnpkg.com/lang/en/docs/cli/cache/)
- [Poetry 配置](https://python-poetry.org/docs/configuration/)、[PDM 配置](https://pdm-project.org/en/latest/reference/configuration/)
- [Go 命令](https://pkg.go.dev/cmd/go)、[Gradle 依赖缓存](https://docs.gradle.org/current/userguide/dependency_caching.html)
- [Composer 配置](https://getcomposer.org/doc/06-config.md)、[RubyGems 缓存](https://guides.rubygems.org/caching-and-vendoring/)
- [vcpkg 二进制缓存](https://learn.microsoft.com/en-us/vcpkg/consume/binary-caching-default)、[sccache 本地缓存](https://github.com/mozilla/sccache/blob/main/docs/Local.md)、[ccache 手册](https://ccache.dev/manual/latest.html)

实机验收和交付哈希另见本轮验证记录；未安装工具不会被记录为真实应用清理通过。

逐文件规则的“30 天”按最后修改时间计算，近期及未来时间的文件保留。ccache 则由官方工具判断最近使用时间。固定工具操作不计入逐文件已选容量；安装、版本、实际目标、排除项、布局和占用均需通过准备及执行前检查。生产程序不会自动安装或下载工具。详见 [闭环验收](verification-cleanup-closure.md)。

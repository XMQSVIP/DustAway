# 雕塑设计应用与验证

日期：2026-09-29。版本仍为 0.18.0，本次为界面设计应用，后端及数据格式未改动。

## 已完成

- 将确认的雕塑原型接入 React / Tauri。四个导航、原生窗口控制、首页实际可用空间、清理汇总、分析、历史、设置和弹窗使用同一视觉。
- 三个悬浮小块分别动画，主体固定；支持深浅主题、系统主题、系统减少动效和应用内减少动效。
- 首页仅一个扫描入口。宽窗口使用右侧选择摘要，窄窗口固定底部主操作；结果及完成页压缩装饰占用。
- 保留真实扫描、文件选择、后端生成计划、永久删除确认、保留清单与历史。没有将原型模拟数据接入桌面应用。

## 自动化证据

- `pnpm build`：TypeScript 与生产构建通过。
- 前端测试合计 191 项通过：首次测试中 16 个文件的 153 项通过，主界面测试工作进程启动超时；随后以 `pnpm exec vitest run src/test/app.test.tsx --pool=threads --reporter=dot` 单独重跑，38 项全部通过。日志 `artifacts/sculpture-app-tests.log`。
- `node scripts/home-smoke.mjs`：1100×780 / 760×580、深浅主题首页主操作可见，无横向溢出；扫描范围展开收起、文件明细、Esc 关闭与焦点返回、取消确认保留文件通过。证据 `artifacts/sculpture-native/result.json`。
- `node scripts/e2e.mjs`：真实 Tauri IPC 的隔离夹具回归通过；含扫描、勾选、确认取消、夹具文件删除、保护文件保留、历史、键盘操作、关闭中断、重新加载、请求回复丢失与过期、只读限制和部分完成状态。结果 `artifacts/sculpture-e2e/desktop-e2e-0.18.0.json`，各任务阶段布局 `artifacts/sculpture-e2e/discovery-0.18.0/layouts.json`。
- 空间分析使用 100,002 个测试文件，检查大小去重、搜索排序分页、大文件/大目录、处理与复核、停止、重新加载与排他任务。结果 `artifacts/sculpture-e2e/analysis-performance-0.18.0.json`。这是隔离夹具测试，不代表所有用户机器的性能保证。
- 相关 TSX / CSS / 脚本的 Prettier 检查通过，日志 `artifacts/sculpture-format-check.log`。

测试所用 debug 程序启用 `e2e-fixture,custom-protocol`，测试根目录必须有 `.dustaway-test-only` 标记。发行程序不启用夹具功能。

第一次完整桌面回归因旧测试脚本寻找首页隐藏的旧快捷入口而停止。测试已改为使用始终显示的“空间分析”导航，随后完整运行通过；相关早期失败截图的时间早于最终通过的 JSON。

## 发行包最终检查

`node scripts/desk-release.mjs` 指向新交付目录的真实 exe 运行通过，结果在 `artifacts/sculpture-release-0.18.0/result.json`：

- 4 个页面 × 2 个主题 × 2 个窗口尺寸，共 16 个原生布局通过；主操作可见，无横向溢出，首页内容高度不超过可视区域。
- Chromium 的 100% / 125% / 150% 模拟在遵守 760×580 最小窗口的前提下共 12 个布局通过。当前原生显示器缩放为 125%；模拟不等于更改 Windows 显示缩放后的实机验收。
- 雕塑主体变换保持不变，三个小块的变换在采样间隔内分别变化；减少动效后四层全部停止。
- 标准运行模式、23 条规则与系统清理未启用的状态符合原版。检查前后后端控制版本不变，没有启动实际扫描或清理。
- 新交付程序与 `target/release/dustaway.exe` 的 SHA256 一致：`DE04E297B1614C3A46B1D8375CC0518DD48F56B11CEB314B503DFD8254F2776C`。文件夹内包含许可证、第三方声明、依赖清单、说明和校验文件。

## 构建与恢复

新视觉程序单独放在 `artifacts/portable/拂尘_0.18.0_雕塑设计版_windows-x64/`，原 `拂尘_0.18.0_windows-x64` 目录保留。修改前源码保存在 `artifacts/before-sculpture-application.zip`，原型仍在 `prototype/index.html`。旧视觉说明保存在 `docs/design-desk-0.18.md`。

重新构建使用 `pnpm tauri build --no-bundle`，随后运行 `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/portable.ps1 -Variant 雕塑设计版`。`Variant` 仅影响交付目录名，默认打包路径兼容原脚本。

如需回退，先关闭新程序，再运行原目录内的程序；源码回退可从 ZIP 恢复对应文件，并移除本次新加的 `src/sculpture.css`、`src/HomeStorage.tsx` 与 `src/assets/space-sculpture.png`。源码恢复前应保留后续修改，避免覆盖之后的工作。

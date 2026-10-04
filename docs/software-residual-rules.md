# 软件卸载独立残留规则

当前兼容性策略已调整：软件版本仅作记录，不作为清理白名单；已开放规则按实际身份、布局、组件和格式核验。扫描后更新仍会使旧清单失效。详见 [应用清理兼容性策略](cleanup-compatibility.md)。以下实机数据与旧包校验值保留为历史证据，不代表所有版本均已实测。

本表记录**规则声明与缺口**，声明不等于删除验收通过。当前仅 Firefox 八条规则具有生产删除许可，`140.17.0.1158` 是实机样本；其他版本按相同身份与范围核验，其余规则继续保留。Brave `154.1.96.60` 的 11 项已取得测试专用逐条删除保护报告，仍缺真实重新安装拦截与正式入口/页面验证，不开放权限。目标仍是原方案的五款浏览器、八款桌面应用及当前用户商店应用；微信的设置和本地资料位置尚未独立确认，不能把缓存范围当成它们的替代。WPS 已独立声明默认文档备份和两份加载项设置，其他设置/资料仍保留。

## 规则边界

`crates/core/src/software/residual_rules.rs` 声明 96 条固定范围，分别使用 `cache`、`settings`、`local_data` 类型。独立规则 ID 以 `uninstall.` 开头，与普通清理规则的许可和验收表无关。界面继续按“缓存与日志 / 设置与本地资料 / 已保留”分组，未匹配内容只读保留。

| 软件族 | 缓存/日志规则 | 设置规则 | 本地资料规则 | 位置识别要求 |
| --- | ---: | ---: | ---: | --- |
| Chrome / Edge / Brave / Vivaldi（各自） | 3 | 2 | 6 | 当前用户标准 User Data 下的 Default、Profile 数字目录，存在普通 Preferences 标记 |
| Firefox | 1 | 2 | 5 | profiles.ini 中登记的相对标准 Profiles 目录，存在普通 prefs.js 标记 |
| 微信 | 2 | 0 | 0 | Tencent/xwechat 下的 log/*.xlog 和 crashinfo/*.dmp；设置/资料待独立识别 |
| 经典 QQ | 3 | 1 | 2 | Tencent/QQ/webkitex_cache 下数字配置目录及 LocalPrefs.json 标记 |
| WPS | 1 | 2 | 1 | 日志保持原边界；独立识别 Roaming/Kingsoft/office6/backup，以及 Roaming/Kingsoft/wps/jsaddons 下 publish.xml、jsplugins.xml 两份文件 |
| 飞书 | 3 | 1 | 2 | LarkShell/aha/users 的直接账号子目录，profile_main/profile_explorer 及 Preferences 标记 |
| 钉钉 | 2 | 1 | 2 | **LocalAppData** 下 DingTalk_数字/Default 及 Preferences 标记 |
| 百度网盘 | 3 | 1 | 2 | RoamingAppData/baidunetdisk 及 Preferences 标记 |
| 夸克网盘 / 迅雷（各自） | 2 | 1 | 2 | 各自标准 RoamingAppData 配置目录及 Preferences 标记 |
| 商店包 | 1 | 1 | 3 | 当前用户主包须通过共享容器、关联包及唯一安装身份核对；TempState 为临时缓存，LocalCache 与 LocalState、RoamingState 按本地资料处理，Settings 为设置；相邻包不属于本规则 |

缓存仅匹配明确的 Cache/Cache_Data、Code Cache、GPUCache、Firefox cache2 或指定日志扩展名，不在任意后代路径中搜索 `Cache`/`logs` 名称。Chromium 设置仅声明 Preferences / Secure Preferences，本地资料分别声明 Bookmarks、History、Network/Cookies、Local Storage、IndexedDB、Sessions。Firefox 设置仅声明 prefs.js / user.js，本地资料分别声明 cookies.sqlite、places.sqlite、storage、bookmarkbackups、sessionstore-backups。其余软件的配置标记文件与 Local Storage / IndexedDB 分属独立规则，尚无对应真实删除证据。

这些文件名不是模糊前缀：例如声明 History 不包含 History-journal；未声明的伴随文件、其他配置文件、任意数据库或目录内容都不会继承权限。需要覆盖新的文件类型或位置时，应新增或修改明确规则并重新验证。

### WPS 独立范围依据

- `uninstall.wps.backups`：仅 RoamingAppData/Kingsoft/office6/backup 默认目录，分类为本地资料。WPS [官方默认备份位置说明](https://www.wps.cn/learning/room/d/243664)给出该位置；[备份设置说明](https://www.wps.cn/learning/room/d/329430)明确允许更改位置。当前规则不读取或跟随自定义备份设置，备份可能是未保存文档的唯一副本，不能作为缓存处理。
- `uninstall.wps.published_addins` 与 `uninstall.wps.legacy_addins`：只包含 RoamingAppData/Kingsoft/wps/jsaddons 下的 publish.xml 和 jsplugins.xml 普通文件，分类为设置。[WPS 开放平台](https://open.wps.cn/documents/app-integration-dev/wps365/client/wpsoffice/wps-integration-mode/wps-addin-development/wps-addin-development-instructions)描述了两种加载方式、配置生成位置及版本限制。未存在的模式不建立范围；不包含加载项代码、嵌套同名文件、整个 jsaddons 或 Kingsoft 父目录。

两个位置识别器独立于日志规则；不存在日志目录也可识别上述明确范围。所有范围仍先核验桌面软件签名/安装归属、目录与父链身份，再冻结具体文件。文档位置和本机存在性不是删除验收，三条规则的生产许可仍为空；其他 WPS 设置、云盘和资料不推断支持。真实验收还须确认并存的 WPS 套件或独立产品是否共享这些位置，未证明独占归属前不得开放删除。旧 93 条规则的范围摘要已逐一比较，保持不变。

软件详情按本表的三个类别显示“检查范围 / 清理能力”。类别没有独立声明时显示暂不支持，不能由另一类别推断；已声明但没有有效验收时显示尚未开放。某一范围存在验收也只显示需逐项核验，不以软件登记版本代替签名版本，不将其他商店包的证据通用化。该描述只帮助用户理解范围，后台仍独立检查每份冻结候选的权限。

## 冻结与再次核验

### 商店包数据独占性

包族名称本身不证明数据独占。[Windows 共享包容器](https://learn.microsoft.com/en-us/windows/msix/manage/shared-package-container)允许多个主包及其可选/修改包共享状态，成员定义可以在应用未安装时继续存在。当前检查通过 `SharedPackageContainerManager.GetDefault().FindContainers()` 读取当前用户全部容器成员，不以已安装清单代替成员定义，不操作其他用户或预配容器。

卸载前须核验同包族只有原完整安装身份、目标确为当前用户主包、不属于共享容器，并使用 [FindRelatedPackages](https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.package.findrelatedpackages?view=winrt-26100) 的双向查询排除可选包和宿主运行时等应用关联。普通框架与资源依赖不认领主包资料。卸载后重新查容器成员及当前用户 Main/Optional 包关联，同时确认原包族仍未安装。每轮包数、成员数及关联总数均有上限。

核验上下文绑定当前用户、原完整包身份、包族和独占性契约版本，只存在内存。缺少旧字段、系统 API 不可用、部分查询失败或共享关系变化时不遍历残留目录，返回保留告警；不能将其显示为“没有残留”，也不影响已经通过移除策略核验的标准卸载。较早 Windows 没有相关 API 时同样保留数据，不尝试提权绕过。

商店规则范围摘要纳入 `current-user-exclusive-store-v2`；其中 LocalCache 的分类修正后单独升为版本 4，另外四条保持版本 3，其余 91 条规则不受影响。私有契约版本 2 还保存 PackageManager 返回的本地安装位置，仅用于检查相关进程，绝不将安装位置变为删除范围。缺少该字段的旧证明不再有效。

`LocalCache` 不能由名称推断为可再生成缓存。[LocalCacheFolder 的系统定义](https://learn.microsoft.com/en-us/uwp/api/windows.storage.applicationdata.localcachefolder?view=winrt-26100)只保证不参与备份与恢复；[ApplicationData 文档](https://learn.microsoft.com/en-us/uwp/api/Windows.Storage.ApplicationData)说明它持久保留并跨更新存在。[MSIX 文件重定向](https://learn.microsoft.com/en-us/windows/msix/msix-troubleshooting-guide)也可能将应用文件置于其中。因此 `uninstall.store.local_cache` 保持原目录范围，但分类为 `local_data`，返回候选 `category=data`，选择后确认页必须包含资料删除提示。TempState 仍单独归为缓存；LocalCache 的旧版本 3 缓存分类和旧范围摘要不能重用。该修正不是实机删除证据，没有开放任何商店生产许可。

扫描和每次残留删除前，使用 [WTSEnumerateProcessesW](https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsenumerateprocessesw) 获取当前用户的进程，包括其他会话；普通权限下不认领系统隐藏归属的其他用户进程。只请求进程元数据和退出状态权限，打开句柄后再次核对所属用户。使用 [GetPackageFamilyName](https://learn.microsoft.com/en-us/windows/win32/api/appmodel/nf-appmodel-getpackagefamilyname) 匹配包族，并检查程序路径是否处于已捕获安装目录内，以覆盖没有包身份的子程序。目录比较使用 Windows 的不区分大小写比较及完整路径分量，不能将同名前缀的相邻目录认作本应用。

已确认运行与无法确认进程状态分别提示保留，不强制结束、不提权。进程在枚举期间退出时通过句柄退出状态处理；读取失败不能代替退出证明。共享成员、关联包和进程遍历都会在条目之间响应停止，不缓存一次查询来授权后续整批删除。原生系统调用本身不被强制中断。

这些保护并非通用删除许可。已取得实际包进程识别和专用无包身份子程序自然退出的实机证据；同用户多会话的受控样本、可选/宿主关联包正向样本、真实 MSIX 卸载前后进程与数据残留保护、通用系统契约仍须继续验收。商店包的生产许可仍为空，不能用查询测试解锁任意包。

卸载前同时保存只读观察范围和独立规则范围。后者保存规则 ID、范围摘要以及该文件/目录的卷、文件身份、创建信息和父目录身份。只有卸载前已经存在且核验通过的范围才参与分类；卸载后新建的同名缓存目录、路径替换、缺失配置标记和自定义目录不能获得权限。

扫描时重新核对独立范围身份；一个候选匹配多个规则时直接保留。设置/资料规则不覆盖配置根整体，日志扩展名规则不授权删除整个日志目录。目录候选仅在明确的 tree 规则内产生删除能力，执行仍要求目录为空，不自动选中或递归删除后代。

删除入口再次核对：上下文版本、规则及范围摘要、独立范围身份、软件未安装、同系列其他安装不存在、相关进程停止，以及原有的文件身份、父路径、排除、链接、硬链接、云文件和占用保护。旧只读规则和普通清理入口不会因此放宽。

## 验收绑定

卸载上下文版本为 2；每条规则还有自己的版本。许可记录必须同时匹配规则 ID、版本、完整范围 SHA-256、软件身份和真实验收证据 SHA-256；四段版本用于核验安装信息与记录。范围摘要包括目标类型、相对位置、扩展名、数据类型和配置识别布局。只改规则 ID 的其他字段不能沿用原证据。

桌面软件使用签名核验的程序版本，不能以注册表显示版本冒充；商店应用使用 PackageManager 的包版本。当前商店许可匹配精确包族，不允许用专用测试包的版本给其他包授权。实现通用独占包数据目录能力，仍须另行验证系统契约与适用边界，不能把只开放测试包算作原方案完成。

缓存证据不能解锁同软件的设置、资料或其他缓存规则；一个产品的证据不能解锁另一个产品。此前 Firefox 单个真实缓存文件补测不是现有逐条规则的许可，合成测试也不填入生产许可表。

可导出实际编译的规则和范围摘要用于后续实机验收：

```powershell
cargo run -p dustaway-core --example software_residual_catalog --locked --quiet
```

目前导出 `compiled_acceptances` 为八条 Firefox 记录，精确匹配上述程序版本和各自范围摘要。对应回归覆盖固定位置、配置标记、未登记/自定义位置、扩展名、目录替换、新增目录、相邻包、不同安装身份以及来源读取失败。缓存、user.js 和 Cookie 已有真实页面选择/确认/删除证据；书签与会话此次完成原生生产入口复测，未重跑它们的真实页面流程。其他产品、版本和云同步保护仍需独立证据。

## 实机卸载前预检查（2026-10-01）

在指定闲置电脑的普通用户会话中，新的 Firefox 测试安装已核验 **PE 版本 `140.17.0.1158`**（软件显示版本为 140.17.0）。上下文第二版独立捕获并重新核验 cache2、prefs.js、user.js、cookies.sqlite、places.sqlite、storage、bookmarkbackups 共 7 个范围；sessionstore-backups 未存在，记录为未捕获，不能计作该规则的产品证据。共享父目录和未知后代 Cache 名称未获分类，仍安装时的残留扫描被拒绝。测试 Cookie 和本地存储通过真实浏览器在回环地址读取。

`artifacts/software-remote-acceptance/evidence/firefox-v2-scope-preflight.json` 只证明卸载前识别和范围捕获，不证明卸载后的身份保持、删除或资料保护。未调用卸载/删除，未保存 PreparedSoftware 或文件身份授权，不能用该报告恢复处理。后续实际卸载仍须在同一运行实例中重新准备并保留新上下文。生产验收表不因此增加记录。

## 首批真实删除许可（2026-10-01）

后续在同一专用安装和标记配置中重新取得卸载前上下文，以 Mozilla 官方静默卸载准备残留场景。逐条验证了 cache2、prefs.js、user.js、cookies.sqlite、places.sqlite、storage：排除、占用、内容变化、硬链接和路径替换均拒绝；每次删除后的未选择文件哈希及共享见证保持一致，新增文件和非空目录保留。书签备份目录为空，首轮整项测试因此失败；只将已经完成全部断言并落盘的六条独立结果列入[证据文件](evidence/software/firefox-rules-20261001.json)，没有将失败批次描述为全部通过。

再次建立新上下文后，六条规则通过了 `WindowsSoftware.delete` 正式入口复测，并在真实重新安装 Firefox 时确认旧记录因安装状态而被拒绝；再次完成官方卸载后重新扫描，才继续删除。完整第二轮报告为 `firefox-production-ini-20261001-report.json`。证据采集的替代判定函数只编译进测试程序；生产入口始终使用固定审批表，不存在前端或环境变量绕过。

许可精确限定签名程序版本 `140.17.0.1158` 和六个范围摘要，不包含书签备份、会话、整份配置或其他版本。此次静默卸载用于资料测试，不替代交互向导、真实页面选择/确认、云同步提供程序或其他产品的完整验收。普通清理的权限不改变。

## 书签与会话独立许可（2026-10-01）

后续在同一专用标记配置内，由真实 Firefox 的书签与会话组件生成压缩备份，解码检查测试书签、两个回环页面和重新打开后的上一会话。`bookmarkbackups` 与 `sessionstore-backups` 分别完成文件删除及排除、占用、变化、硬链接、路径替换、部分扫描、未选哈希和新增文件保护验证。新的[独立证据](evidence/software/firefox-history-rules-20261001.json)仅绑定这两个范围，首批六条的原证据及摘要不变。

正式 `WindowsSoftware.delete` 入口随后通过同样检查，并验证真实重新安装时拒绝旧记录；再次官方卸载并重新扫描后才处理资料。此次删除 5 份已冻结测试文件共 4,210 字节，含前轮测试见证文件，不把它们全部描述为浏览器正文。两个非空目录和本轮新增文件均保留。当前共八条许可，仍严格限定签名程序版本 `140.17.0.1158`；不新增整个配置、其他版本、普通清理或其他产品权限。

# 更新日志 (Changelog)

本项目所有重要变更都将记录在本文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 规范，版本号遵循 [语义化版本 (Semantic Versioning)](https://semver.org/lang/zh-CN/)。

---

## [Unreleased]

---

## [1.1.1] - 2026-11-25

**BreezeMusic Desktop `v1.1.1`** 带来沉浸播放页「光晕浮升 · 声浪呼吸」每字独立弹簧与 120Hz 行级瀑布滚动、全站 1040px 容器自适应单/双列空间 FLIP 归位、默认与最大化窗口进入 Mini 小窗的零闪屏三段式 GPU 过渡，以及自动隐藏任务栏下的灵动岛防消失与顶栏悬停体验全面加固。

### 新增 (Added)
- **「设置 → 常规与窗口」新增「全屏视频/游戏时自动隐藏 Mini 灵动岛」开关**：默认开启，同屏进入真正的 F11 浏览器全屏、全屏视频、PPT 放映或全屏/无边框全屏游戏时自动无痕隐身礼让，退出全屏瞬时恢复，亦支持用户按需关闭以始终保持灵动岛置顶；同步纳入设置中心中英文实时搜索索引。
- **Mini 灵动岛贴边停靠位置记忆**：贴边吸附与沿边滑动后自动持久化沿边归一化停靠比例（`bm_mini_dock_anchor_v1`），重启或冷启动再次切入小窗时精准回到上次自定义贴边位置。

### 优化 (Changed)
- **沉浸播放页歌词全面重构为「光晕浮升 · 声浪呼吸」每字独立弹簧 + 120Hz 行级瀑布错峰滚动引擎**：
  - 新增 [lyricMotionEngine.ts](src/utils/lyricMotionEngine.ts) 并重构 [NowPlayingView.vue](src/views/NowPlayingView.vue) 歌词舞台：彻底告别“进度条式左右擦字”与每帧主线程 `linear-gradient` / `drop-shadow` 重绘，升级为**双层字粒 + 静态预光栅化辉光伪层**纯 GPU 合成管线（每帧仅更新 `transform` 与 `opacity`，60/120/144Hz 全刷帧率一致丝滑，字缘锐利零发毛）。
  - 为活跃行每个音节引入独立二阶物理弹簧（提前 90ms 轻柔预热联动、105–240ms 广域柔光快速浸润、快连音自动衔接成字浪、长尾音 ≥ 480ms 高点 ~1.35Hz 呼吸声浪微振与柔光吞吐），并支持无缝移交的 **380ms 退场余韵池**（换行时刚唱完行从容回落与辉光渐熄，消除 1 帧硬切变灰）。
  - 升级整行歌词滚动为 Apple Music / AMLL 式**行级弹性瀑布错峰追随**（当前行率先跃升，后续邻行依次滞后 ~42ms 波浪跟进）并接入 `scrollTop` 子像素残差吸收补偿（消除慢速收尾 1px 阶梯微抖），完整兼容滚轮随时介入、点击跳转、浅/深色双舞台及三档动效设置。
- **全站歌曲列表升级为「1040px 容器宽度感知 + 空间 FLIP 真实归位」自适应单/双列系统，并根治悬停操作按钮压字**：
  - 新增 [adaptiveSongGrid.ts](src/utils/adaptiveSongGrid.ts) 并统一全站 8 处 `.frameless-songs-grid-2col`（发现页日推/猜你喜欢/新歌/排行榜、歌单/专辑/歌手详情、搜索、云盘、本地与电台队列弹窗）为 **1040px 容器宽均衡档**（保证双列每列 ≥ 510px）：默认窗口（`1240px`）下无侧栏的日推/猜你喜欢/新歌/云盘/本地自然保持宽裕双列，带侧边栏的排行榜与歌单/专辑/歌手详情自动切为舒展单列；窗口拉宽或最大化跨过滞回阈值（`1056px` 进双列 / `1020px` 退单列，36px 防抖缓冲带）时平滑自适应。
  - 手动拖拽窗口或最大化切换单/双列时，自动对视口内可见曲目行执行 GPU 合成 `translate3d` 空间 FLIP 真实归位动画（高阻尼弹簧 + 12ms 轻微波浪错峰、绝不压扁封面与字形、`O(log N)` 二分视口扫描、锁定首行滚动锚点防视野跳飞），并完整尊重三档动效（`best` / `standard` / `none`）与系统减弱动态效果。
  - 全面打磨 `.unified-song-row` 悬停交互：单列宽裕态在右侧预留 `184px` 专属操作通道实现 100% 零压字；双列态升级按钮间距并对超长歌名/歌手尾端施加四主题通用的 `mask-image` 平滑渐隐羽化遮罩（区分 4 键与 5 键行宽，且自动避让内部键盘焦点环）。
- **彻底重塑主窗口进入 Mini 小窗的零闪屏 GPU 空间收束过渡动效（兼容默认窗口与最大化窗口）**：
  - 升级 [window_transition.rs](src-tauri/src/window_transition.rs) 与 [lib.rs](src-tauri/src/lib.rs) 为**三段式严格原子交接**（前景极速 `BitBlt` ~1.5ms 截帧 → GPU 静态 100% 不透明锁帧覆盖并先 `Commit` 再显示 → 掩护下零闪烁撤离真实主窗 → 干净单层 120Hz+ 收束起飞），根除“双影叠加 → 底窗骤灭”闪屏及 `PrintWindow` 回读 WebView2 卡顿。
  - 重调 `EnterMini` 空间动效曲线为 `310ms` 有力等比收束（最大化 `1.0 → 0.54` / 默认窗 `1.0 → 0.48`）、鲜明指向灵动岛的方向牵引飞掠、`0/8px → 36px` 硬件抗锯齿动态圆角与「前 40% 高实体锚定 + 后 60% 柔化消融」两段式透明度曲线；同步接入启动空闲预热 `DCompStage`、`IMMUTABLE + CopySubresourceRegion` 驱动全兼容上传、三级独立变换修正降采样居中对齐，以及 `MiniPlayerWindow` 在 `DWMWA_CLOAK` 隐身斗篷下预合成 `entering` 透明首帧并于 `125ms` 黄金接力点零双闪弹性滑入。

### 修复 (Fixed)
- **彻底根治自动隐藏任务栏及无任务栏副屏下，浏览器/Web 应用点击中键快速滑动（AutoScroll）或触发弹窗导致 Mini 灵动岛消失的问题**：
  - 重构全屏检测引擎为 `classify_foreground_is_true_fullscreen`，改用 `DWMWA_EXTENDED_FRAME_BOUNDS` 真实可视边框并结合 `IsZoomed`、`WS_CAPTION` / `WS_THICKFRAME` 与 `SHQueryUserNotificationState`（排除易误报的 `QUNS_BUSY`），100% 剔除自动隐藏任务栏与副屏无任务栏下普通最大化窗口被误判为全屏并遭静默剥离 `WS_EX_TOPMOST` 的缺陷。
  - 将真全屏礼让升级为 `DWMWA_CLOAK = 1` 原位零闪烁隐身 + 点击穿透（退出全屏瞬时解 cloak 复现），消除全屏礼让期 `300ms` 后台空转唤醒，并新增轻量 `GW_HWNDPREV` Z-order 遮挡探针以防其他实窗压盖灵动岛。
- **彻底修复点击灵动岛展开小窗时，自动隐藏顶栏经常不随鼠标悬停正常显示的问题**：
  - 引入跨线程状态世代号 `mini_mode_gen` 并将展开/收起/吸附的 `GWL_EXSTYLE` 穿透与置顶操作统一收敛至 UI 主线程，杜绝后台 16ms 热区轮询线程的迟到闭包将已展开卡片误设回点击穿透或发射过期的 `bm-mini-island-cursor-leave`。
  - 精细化 `set_overlay_click_through_raw`，纯穿透位切换不再强发 `SWP_FRAMECHANGED` 冲掉 WebView2 悬停态；前端将悬停事件精准收敛到可见卡片实体 `.bm-mini-shell`，构建「展开即显（含 340ms 防误触）+ `@pointermove` 轻量自愈 + 离场事件模式门控 + CSS `:hover` / `:focus-within` / `@media (hover: none)` 原生兜底」四重保障。
- **加固 Mini 小窗后台预热定位与拖拽指针捕获**：隐藏预热期间仅静默同步贴边偏好与停靠比例，绝不以 `(0,0)` 提前污染定位标志或误设置顶；拖拽新增 `setPointerCapture` 与 `lostpointercapture` / `blur` 安全兜底，杜绝快速甩拖出窗松手卡死。

---

## [1.1.0] - 2026-11-23

**BreezeMusic Desktop `v1.1.0`** 带来酷狗双端分享与链接解析体系、D3D11 + DirectComposition GPU 空间势能过渡动效、原生 Win32 单实例唤醒、全栈流畅度与后台功耗深度优化，以及深色主题设置中心与防切歌串台体验全面加固。

### 新增 (Added)
- **酷狗双端分享与口令/链接解析体系**：
  - 支持单曲与歌单三档目标唤醒版本生成（默认「酷狗音乐」标准版 / 「酷狗概念版」 / 「概念版 + 标准版」双端直达），同步生成官方 H5 落地页、桌面试听页及 `kugou://`、`kugouyouth://` 唤醒协议（彻底剔除 `a.app.qq.com` 干扰跳转），并在弹窗展示前异步校准标准 128k MP3 hash / `album_audio_id` 与歌单 `specialId` / `globalCollectionId`。
  - 集成纯 Rust 多级入站解析引擎：离线 Base35 `gcid_xxx` 与 Base36 `e_album_audio_id` 校验和解码、逐跳 SSRF 校验短链重定向追踪（最大 6 跳 / 1MB 限制）与网页 `window.$output` / `phpParam`（优先提取 `album_img` 过滤歌手头像）解析。
  - 打通全站分享与识别闭环：支持切回前台剪贴板自动识别酷狗分享链接（含自复制去重与「设置 → 常规与窗口」独立开关）、搜索框粘贴口令/链接直达单曲或歌单、50 首分批快速导入自建歌单（解除原 60 首截断上限），并新增四主题无框原生 `<dialog>` 分享窗与识别弹窗。
- **原生 Win32 进程级单实例守卫与跨进程唤醒**：新增 `src-tauri/src/single_instance.rs`，在 WebView2、托盘与 Tauri Builder 初始化之前通过 `Local\` 会话级命名 Mutex + 0% CPU 休眠的 `HWND_MESSAGE` 宿主窗口完成原子单实例判定；小窗模式、托盘常驻或最小化状态下点击任务栏固定图标、桌面快捷方式或连点双击时，不再拉起新窗口与重复托盘图标，而是通过 `AllowSetForegroundWindow` 移交前台焦点令牌并平滑唤醒/恢复已有主窗口（含 260ms 连点防抖与 `AttachThreadInput` 置前加固）。

### 优化 (Changed)
- **全新重做主窗口 ↔ Mini 小窗 GPU 空间势能过渡动效引擎**：彻底弃用旧版硬切隐藏/显示与事后弹跳补丁，新增 `src-tauri/src/window_transition.rs` 原生 D3D11 + DirectComposition (`WS_EX_NOREDIRECTIONBITMAP`) 硬件合成舞台，通过分段三次 Hermite 样条精准驱动 Apple 高阻尼曲线、动态抗锯齿圆角（最大化 `0px ↔ 22px`、默认窗 `8px ↔ 22px`）、等比缩放与指向小窗实体的空间牵引位移，并与 `MiniPlayerWindow.vue` 外层弹簧入场 / 释能退场动效及启动空闲后台预热严密锁相；全面支持三档动效 (`best` / `standard` / `none`) 与系统减弱动态效果。
- **全栈流畅度、CPU/GPU 功耗与后台待机深度优化**：
  - 新增统一前后台电源与可见性协调器 (`src/utils/windowPowerState.ts`)，与 Rust 端托盘隐藏 / 最小化 / Mini 小窗切换事件 (`bm-main-window-visibility`)、WebView2 `MEMORY_USAGE_TARGET_LEVEL_LOW` 及 `html.bm-window-hidden` 联动，后台隐藏期间 0ms 暂停全部视觉 rAF、WebGL2/Canvas 渲染、CSS 过渡/动画、高频进度响应式写入与每小时后台轮询（回前台后自动延迟补偿同步），同时 100% 保持后台音频连续播放不断流。
  - 重构沉浸播放页 KRC 逐字歌词渲染链路（[NowPlayingView.vue](src/views/NowPlayingView.vue)），以普通 JS 时钟 + `O(log N)` 二分定位当前行 + `Float32Array` 差量缓存直接更新活动行变化词元 DOM，彻底消除 60–120Hz 全量 VDOM 重绘抖动。
  - 优化 WebGL2 动态光学背景（`maxDpr: 1.5`、`fluid_aurora` 渲染倍率 `0.72`、播放 60fps / 暂停 ~12fps 帧率封顶与预分配 uniform 缓冲区），为全局 `MutationObserver` 增加快速路径过滤，降低 WebAudio 频谱采样与高刷屏桌面歌词计算开销。
- **设置中心全面视觉与交互重构（Atelier 静谧画廊 × 2 层清阶无框架构）**：
  - 重做左侧分类导览列为彻底透明无底色悬浮样式，去除冗余抬头与彩虹方盒图标，顶部直出带 `Ctrl F` 快捷聚焦的命令搜索框；导航激活态改用清透主色微染滑块 `.bms-nav-glider` 配左侧 3px 主色渐变音柱（满足 WCAG 1.4.1 非纯色标识），状态注脚极简降噪，仅告警冲突保留珊瑚圆点提示。
  - 重构右侧详情区为「画布直出分节抬头 + 单层圆角分组面板」2 层清阶架构，面板内设置行以 1px 渐隐柔光微带分隔，并为 D1 / D2 / L1 / L2 校准 5 级无框表面高程令牌与 `--primary-text` 高可读主色字阶，彻底消除四层嵌套与暗色选择砖同色融化问题；宽屏全宽舒展对齐顶栏与底栏边界。
  - 升级全设置中心分段控制器与子标签（`.bm-seg-glider`）为高阶中性浮起滑块，收束高饱和主色仅用于核心 CTA 与开关激活态；精修四套主题预览卡（预览窗圆勾徽章 + 容器比例自适应画布）、动效/三线密度选择砖与渐进增强 `base-select` 无边框下拉框。

### 修复 (Fixed)
- **彻底根治评论切歌串台及全项目 6 类同构上下文串台/异步竞态缺陷**：
  - 解耦当前播放歌曲评论数角标与评论弹窗会话快照，后台自动切歌、Crossfade 淡变接力或冷启动恢复时不再清零或篡改正在浏览的评论区，全链路翻页/排序/楼中楼/发评补齐 `reqSeq` + `songKey` 双重守卫。
  - 同步加固高清封面预览灯箱（右键非播放曲精准展示目标原图）、2K/4K 歌词海报弹窗（制作/导出期间冻结目标曲与色板）、歌曲详情元数据、候选歌词搜索、电台切台及歌手详情排序的快照锁定与竞态防串台校验。
- **修复贴边灵动岛幽灵按钮透出与误触**：为 `MiniPlayerWindow.vue` 双层形态切换补齐原生 `inert`、`visibility: hidden`、不可击穿后代 `pointer-events: none !important` 与卡片动作状态门控，彻底消除贴边收起态悬浮/点击右侧播放钮时错误弹出「返回主窗口」等展开层提示并触发对应功能的问题。
- **加固贴边灵动岛切屏/切窗/`Win+D` 稳定性并支持全屏智能礼让**：为 `mini_player` 补齐 Win32 `WS_EX_TOOLWINDOW` 透明叠层样式加固、磁吸动画代号防竞态、多显示器切屏越界自动重吸附与后台低开销切窗自愈守护；普通切窗与显示桌面下始终稳定不消失，同屏运行全屏视频/游戏时智能礼让并在退出全屏后自动恢复置顶。
- **彻底根治最大化进入小窗/托盘再恢复的底部白线、生硬双跳、透明幽灵窗遮挡与默认窗两边渲染异常**：
  - 全面绕开 `tao::set_visible` 在最大化隐藏/恢复时破坏 `WS_VISIBLE` / `GWL_STYLE` 的底层缺陷，改为在 `DWMWA_CLOAK = 1` + `DWMWA_TRANSITIONS_FORCEDISABLED = 1` 隐身掩护下原生一步 `SW_SHOWMAXIMIZED` 恢复并原子校准客户区、无描边 DWM 与 WebView2 `Bounds`，再经 GPU 空间舒展与 65ms 交叉溶解无痕揭幕，彻底杜绝「先变回默认窗口大小再快速最大化」与底部白边。
  - 修复 `mini_player` 经由原生 `ShowWindow` 显示导致 `tao` 内部 `WindowFlags::VISIBLE` 缓存漂移、退出小窗调用 `win.hide()` 静默失效并残留 480×228 全透明置顶窗口遮挡主界面与桌面点击的严重缺陷；新增四重防线原生强制隐藏（退出第 0ms 立即开启点击穿透、同步 `win.hide()` + Win32 `SW_HIDE` + 剥离 `HWND_TOPMOST` + 后台幽灵窗看门狗自愈），并将贴边点击穿透切换改为原子 `GWL_EXSTYLE` 操作（零抢焦、保 `WS_EX_TOOLWINDOW`、绝不误隐藏）。
  - 彻底根治「最大化 → 小窗 → 退出回最大化 → 还原为默认窗口大小」后左右/底部边缘退化为 Windows 7 Aero Basic 蓝灰粗框或实体色带的底层缺陷：修正 `main_window_nc_subclass_proc` 中最大化态吞掉 `WM_NCPAINT`（未先调用 `DefSubclassProc` 即返回 `0`）以及 `WM_NCACTIVATE` 传入 `-1` 导致 Windows 11 DWM 永久撤销 `DWMWA_EXTENDED_FRAME_BOUNDS`（~7px 透明扩展阴影帧）的问题；现无论最大化还是默认窗口态均先原样透传原生 `DefSubclassProc`，仅在最大化态返回后补刷 1px 自动隐藏任务栏防白边底色，四套主题反复切换均 100% 保持无白线与无瑕 Win11 原生圆角柔影。

---

## [1.0.1] - 2026-11-23

**BreezeMusic Desktop `v1.0.1`** 全面焕新桌面端品牌图标、系统托盘小尺寸光学清晰度与应用内品牌标识体验。

### 新增 (Added)
- **专属纯矢量品牌徽标与 Favicon**：新增 [BreezeBrandLogo.vue](src/components/BreezeBrandLogo.vue) 矢量品牌组件升级「设置 → 关于微风音乐」视觉卡，并补齐 `public/favicon.svg` 与 `index.html` 图标声明。

### 优化 (Changed)
- **全新「微风浮雕」极简双色桌面图标**：承袭安卓端 BreezeMusic「微风丝带 + 黑胶空心风翼音符」家族基因，全面替换早期占位图标为晴空鸢尾靛 × 冰川天青 × 薄荷青玉无框极光超椭圆图标，对齐全软件 100% Zero-Border 与 `PlumpIcon` 软润美学。
- **Windows 全尺度 7 帧 ICO 与系统托盘清晰度特化**：重写 `src-tauri/icons/icon.ico`（严格遵循 Tauri 2.0 `32×32` 置首规范，完整内嵌 `32 / 16 / 24 / 48 / 64 / 128 / 256` 七档分辨率），并为 `16×16`、`24×24` 及 `32×32` 托盘与任务栏尺寸制作独立光学清晰度特化母版，小尺寸下黑胶中孔与微风弧带依然清晰锐利。
- **NSIS 安装程序自定义图标同步**：在 `src-tauri/tauri.conf.json` 中显式配置 `bundle.windows.nsis.installerIcon = "icons/icon.ico"`，使安装程序 `.exe` 文件本身与主应用视觉完全统一。

---

## [1.0.0] - 2026-11-22

**BreezeMusic Desktop（微风音乐 Windows 桌面版）** 首个正式版本发布。

### 新增 (Added)
- **四套高定无框主题体系**：内置 `D1 黑曜石极光`、`D2 丝绒星云`、`L1 棉纸水彩`、`L2 晨雾燕麦` 四套特化主题与系统明暗跟随，全软件贯彻 100% Zero-Border 无边框设计，支持三档动效引擎与三档界面密度调节。
- **沉浸播放舞台与 OKLab 双主色取色**：
  - 上线「液态丝绸 / 长虹声幕 / 声致驻波 / 星尘梦境」四大 WebGL2 + Canvas 2D 光学背景，切歌过程物理运动零中断。
  - 集成 Rust 原生封面代理与 `Wu 量化 + OKLab WSMeans` 双主色提取，支持「封面 · 随明暗」「封面 · 锁暗室」「主题配色」三态切换及黑白封面「素宣墨韵」中性灰阶锁定。
  - 支持黑胶转盘/海报切换、KRC 逐字高亮歌词、双语译文与罗马音注音、歌词跳转、KTV 伴奏切换、二层楼中楼热评抽屉与 2K 超清歌词海报导出。
- **母带级 WebAudio DSP 调音台**：
  - 支持 10 / 15 / 31 段参量均衡器（PEQ）、5 大声学目标曲线、A/B/C 自定义记忆槽与蝰蛇音效预设。
  - 集成 Bauer 耳机交叉馈送、卷积空间混响、模拟谐波染色、ReplayGain 响度均衡、夜间等响度补偿、50 dB 对数听觉音量曲线及 `0–8s` 双 Deck 跨曲无缝淡变。
- **完整在线生态、云盘、本地音乐与心流专注舱**：
  - 探索发现页（横向展开每日推荐 / 私人 FM 双卡舞台、新歌速递、官方排行榜、电台广场）、音乐库与歌单双向同步、歌手/专辑详情、综合搜索。
  - 独立云盘音乐视图（配额环形统计、音质分层过滤与批量操作）与本地音乐管理（多目录自动扫描、过滤去重与元数据缓存）。
  - 心流专注舱：番茄钟/倒计时/正计时与雨声、黑胶、篝火、海浪 4 通道程序化环境白噪音混音。
- **三窗体桌面协同与多窗口排版中心**：
  - 主窗体 + 透明穿透桌面悬浮歌词窗 + 四边磁吸停靠 Mini 灵动岛小窗，支持系统托盘与 Win32 全局自定义快捷键。
  - 全局字体排版中心支持扫描系统字体、导入 `.ttf / .otf / .woff2 / .ttc` 字体文件、`75%–145%` 宽幅字号缩放与字重/行距精细调节。
- **纯 Rust 原生后端与本地安全加固**：
  - 基于 Rust + Tauri 2.0 实现端侧协议通信、本地 Range 音频流代理 (`AudioStreamProxy`) 与 FLAC/MP3/OGG 元数据及封面处理。
  - 采用 Windows DPAPI + AES-256-CBC / HMAC-SHA256 本地凭证加密存储、`zeroize` 内存清零、代理反 SSRF 校验及多窗体 IPC 权限守卫。

[Unreleased]: https://github.com/reneryi7/BreezeMusic-Desktop/compare/v1.1.1...HEAD
[1.1.1]: https://github.com/reneryi7/BreezeMusic-Desktop/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/reneryi7/BreezeMusic-Desktop/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/reneryi7/BreezeMusic-Desktop/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/reneryi7/BreezeMusic-Desktop/releases/tag/v1.0.0

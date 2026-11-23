# 更新日志 (Changelog)

本项目所有重要变更都将记录在本文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 规范，版本号遵循 [语义化版本 (Semantic Versioning)](https://semver.org/lang/zh-CN/)。

---

## [Unreleased]

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
- **深色主题（D1 / D2）设置中心阅读焦点重构与全站 L1 滑块统一**：
  - 升级 D1 / D2 深色正文字阶并引入高可读主色文字令牌 `--primary-text` 与四级无边框正向表面色阶，剥离关闭态开关及次要控件的主题色染色污染，正文说明与控件对比度全面满足 WCAG 2.2 ≥ 4.5:1。
  - 为设置页左侧主分类导航新增流体滑块 `.bms-nav-glider`，并将设置页全部二级子标签、互斥分段胶囊、字体选择器作用域栏、下载筛选条与分享版本选择栏全面接入全软件统一的 L1 弹簧滑块（`.bm-seg-glider`）。

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

[Unreleased]: https://github.com/reneryi7/BreezeMusic-Desktop/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/reneryi7/BreezeMusic-Desktop/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/reneryi7/BreezeMusic-Desktop/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/reneryi7/BreezeMusic-Desktop/releases/tag/v1.0.0

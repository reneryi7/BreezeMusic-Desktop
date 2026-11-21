# 更新日志 (Changelog)

本项目所有重要变更都将记录在本文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 规范，版本号遵循 [语义化版本 (Semantic Versioning)](https://semver.org/lang/zh-CN/)。

---

## [Unreleased]

- 暂无待发布变更。

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

[Unreleased]: https://github.com/reneryi7/BreezeMusic-Desktop/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/reneryi7/BreezeMusic-Desktop/releases/tag/v1.0.0

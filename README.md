# BreezeMusic · 微风音乐 (Desktop)

<p align="center">
  <strong>耳畔微风起 · 帧享真无损</strong><br>
  一款基于 <strong>Rust + Tauri 2.0 + Vue 3 + TypeScript</strong> 打造的轻量级 Windows 高解析音乐桌面客户端
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Version-v1.1.2-4F46E5?style=flat-square" alt="Version" />
  <img src="https://img.shields.io/badge/Platform-Windows%2010%2F11%20(x64)-0078D4?style=flat-square" alt="Platform" />
  <img src="https://img.shields.io/badge/Core-Rust%20%2B%20Tauri%202.0-EA580C?style=flat-square" alt="Tauri 2.0" />
  <img src="https://img.shields.io/badge/UI-Vue%203%20%2B%20100%25%20Zero--Border-10B981?style=flat-square" alt="Vue 3" />
  <img src="https://img.shields.io/badge/Audio-Dual--Deck%20%2B%20WebAudio%20DSP-0EA5E9?style=flat-square" alt="Audio DSP" />
  <img src="https://img.shields.io/badge/License-GPL--3.0-blue?style=flat-square" alt="License" />
</p>

---

## 一、项目简介

**BreezeMusic Desktop（微风音乐桌面版）** 承袭并源自安卓端 **BreezeMusic** 的视觉气质与声学理念，并针对 Windows 桌面交互形态、原生性能、多窗体协同与 100% 无框美学进行了自底向上的桌面端独立构建。

客户端将第三方概念版（`appid = 3116`）协议通信、签名加解密、本地 Range 音频流代理与元数据解析全部收敛于 **纯 Rust 原生后端**，不依赖任何额外的 Node.js / Python 中转服务；前端采用 **Vue 3 + TypeScript + Pinia + WebGL2 + WebAudio API** 构建，兼顾极低内存占用、细腻物理微动效与发烧级声学可玩性。

> [!NOTE]
> 界面预览截图位已预留，欢迎在发布后补充浅/深色主题、沉浸播放页、母带调音台与桌面歌词等实机截图。

---

## 二、核心特性与功能亮点

### 1. 四套高定主题 × 100% 无框美学 (`Zero-Border Architecture`)
* **全软件零边框设计**：彻底摒弃生硬的 `1px` 线框与分割线，以表面底色阶差、冰霜毛玻璃折射或手抄纸/天鹅绒微肌理构建空间层次。
* **四套深度特化主题（支持跟随系统自动切换）**：
  * **D1 黑曜石极光 (`Obsidian Aurora`)**：极夜冷曜石基底叠加冰川青蓝极光散景与高透冰霜玻璃。
  * **D2 丝绒星云 (`Velvet Nebula`)**：苦巧天鹅绒实体块面、香槟暖羊皮纸字阶与落日玫瑰/胆机琥珀流金。
  * **L1 棉纸水彩 (`Cotton Watercolor`)**：云宣瓷白基底叠加樱粉/晴空天青湿画法水彩晕染与晴空鸢尾主色。
  * **L2 晨雾燕麦 (`Nordic Oat`)**：零彩光三阶烘焙暖纸材质配北欧矿物青玉与赤陶点缀。
* **动效分级与信息密度**：提供「最佳效果 / 普通效果 / 无效果」三档动效引擎（联动 WebGL 帧率与界面过渡）及「紧凑 / 标准 / 舒展」全局密度控制，配合自定义圆润 `PlumpIcon` 双色图标体系。

### 2. 沉浸播放舞台 × OKLab 双主色取色引擎
* **四大高定光学背景**：内置 **「液态丝绸」 / 「长虹声幕」 / 「声致驻波」 / 「星尘梦境」** WebGL2 + Canvas 2D 双管线渲染，切歌期间物理运动零中断，并配备选择性高光柔膝压限与三角抖动防色带。
* **OKLab 双主色三态联动**：由 Rust 原生代理安全采集封面像素，通过 `Wu 量化 + OKLab WSMeans` 精准提取主色与辅助色（纯黑白封面自动锁定「素宣墨韵」中性灰阶零偏色），支持顶栏一键循环切换：
  * **封面 · 随明暗**：深色主题进入暗室剧场；浅色主题自动演算出高显色水彩染宣纸舞台。
  * **封面 · 锁暗室**：无视全局浅色主题，全屏播放页与关联浮窗始终保持深色暗室沉浸态。
  * **主题配色**：与当前全局四套主题保持完全统一。
* **全功能歌词与社交体验**：支持黑胶唱机转盘与画廊海报切换、120Hz 纯 GPU 合成四套逐字动效引擎（丝绸羽化扫光 / 光晕浸润浮升 / 跃动脉冲 / 静谧直觉）与五套一键场景预设、行级弹性瀑布滚动、手滚中线参考尺跳播、和声轻量化排版、双语译文与日/韩罗马音注音、KTV 伴奏音轨切换、二层楼中楼热评抽屉以及 **2K 超清歌词海报** 一键导出。

### 3. 母带级 WebAudio DSP 调音台与双轨播放引擎
* **多段参量均衡器 (PEQ)**：支持 **10 段 / 15 段 (ISO 2/3 倍频程) / 31 段 (ISO 1/3 倍频程)** 切换，配备 5 大发烧目标参考曲线、A/B/C 自定义记忆槽与蝰蛇音效预设库。
* **声场与模拟染色链路**：内置 Bauer 耳机交叉馈送（缓解头中效应）、卷积空间混响矩阵、胆机/黑胶/磁带谐波染色、ReplayGain 跨曲响度均衡与夜间小音量等响度补偿。
* **听觉工程细节**：采用 **50 dB Weber-Fechner 对数听觉音量曲线**（彻底解决低音量区间调节过陡问题），支持 `0–8s` 双 Deck 跨曲无缝淡变、本地 HTTP Range 音频流代理 (`AudioStreamProxy`) 与真实音频流采样率/位深探测。

### 4. 完整音乐生态、本地曲库与心流专注舱
* **探索发现、在线生态与双端分享解析**：横向可展开「每日推荐 / 私人 FM」沉浸双卡舞台、新歌速递、官方巅峰排行榜、随心听电台广场、综合搜索、歌手/专辑/歌单详情及歌单双向同步；支持酷狗标准版 / 概念版 / 双端直达单曲与歌单分享口令生成、原生剪贴板自动识别、搜索框口令直达与分批快速导入歌单。
* **云盘音乐管理**：独立云盘视图，支持空间配额环形统计、Hi-Res / SQ / HQ 音质分层筛选、关键词检索、排序与批量加入歌单/管理。
* **独立本地音乐中心**：支持多目录添加、启动自动扫描、递归子目录、短音频/小文件过滤、自动去重、内嵌标签与文件名优先级切换以及元数据缓存重建。
* **心流专注舱 (`Focus Pod`)**：集番茄钟、倒计时、自由正计时于一体，内置 **雨声 / 黑胶底噪 / 篝火 / 海浪** 4 通道实时程序化环境白噪音混音器与多套场景预设。

### 5. 三窗体桌面协同、排版中心与本地安全加固
* **三窗体联动与零闪屏 GPU 空间过渡**：无边框主窗口 + 独立透明纯 GPU 引擎桌面悬浮歌词窗（支持 5 套无底浮空预设、SVG 圆角抗锯齿轮廓、KTV 交替、超长句随唱推镜与锁定态鼠标柔和避让）+ 四边磁吸停靠 Mini 灵动岛小窗（配备三段式 D3D11 + DirectComposition 零闪屏 GPU 空间过渡动效、`WS_EX_NOREDIRECTIONBITMAP` 零残留原生无框防护、可配置全屏视频/游戏自动礼让、贴边停靠位置记忆、Win32 原生单实例跨进程唤醒与系统级自定义全局热键）。
* **极低后台功耗与自适应丝滑交互**：前后台电源协调器在隐藏至托盘/最小化/切入小窗时瞬停视觉渲染与定时刷新（100% 保持音频连续播放）；全站歌曲列表支持 1040px 容器宽自适应单/双列切换与 GPU 空间 FLIP 归位动效，配合纯合成 120Hz 逐字弹簧歌词引擎，兼顾高刷流畅度与低 CPU/GPU 占用。
* **全局多窗口字体排版中心**：自动扫描本机系统字体并支持导入 `.ttf / .otf / .woff2 / .ttc` 本地字体（`.ttc` 自动拆包），支持 `75%–145%` 全局字号缩放、五级联动派生字重、行距与等宽数字对齐。
* **可检索 Master-Detail 设置中心**：覆盖外观、排版、常规窗口、播放音质、歌词、本地音乐、快捷键、存储网络与关于页，支持中英文关键词实时搜索定位。
* **本地隐私与安全架构**：使用 **Windows DPAPI + AES-256-CBC / HMAC-SHA256** 加密持久化账号会话凭证（支持多账号无缝切换与右上角头像悬浮面板），敏感内存使用 `zeroize` 即用即清零；支持封面代理反 SSRF 校验与全量 IPC 窗口权限守卫。

---

## 三、技术栈总览

| 层级 | 核心技术与方案 |
| :--- | :--- |
| **桌面宿主与原生后端** | Rust (`edition 2021`) · Tauri 2.0 · Tokio · Reqwest (`rustls-tls`) · Windows DPAPI · Win32 `RegisterHotKey` |
| **前端框架与状态管理** | Vue 3 (`<script setup>`) · TypeScript 5 · Pinia · Vite 5 · Lucide Icons & 自绘 `PlumpIcon` |
| **音频播放与元数据** | HTML5 Dual-Deck Audio · WebAudio API DSP · 本地 Range 音频代理 (`AudioStreamProxy`) · `lofty` + 纯 Rust FLAC/MP3/OGG 标签与封面引擎 |
| **视觉与色彩渲染** | WebGL2 Fragment Shaders · Canvas 2D · OKLab + Wu Quantization + WSMeans 双主色算法 |

---

## 四、下载安装与快速开始

### 方式一：普通用户直接安装（推荐）

前往本仓库右侧的 [**Releases**](https://github.com/reneryi7/BreezeMusic-Desktop/releases) 页面下载最新正式版安装包（支持 **Windows 10 / 11 64 位**）：

| 安装包类型 | 文件名示例 | 适用场景 |
| :--- | :--- | :--- |
| **NSIS 安装包（推荐）** | `BreezeMusic_1.1.2_x64-setup.exe` | 体积更小，支持单用户或全体用户安装与自定义路径 |
| **MSI 安装包** | `BreezeMusic_1.1.2_x64_en-US.msi` | 标准 Windows Installer 格式，适合系统级安装与静默部署 |

> [!TIP]
> **关于 Windows Defender SmartScreen 提示**
> 由于个人开源项目未购买商业代码签名证书，首次运行安装包时 Windows 可能会弹出「Windows 已保护你的电脑 / 未知发布者」拦截提示。你可以核对 Release 页面公布的 **SHA-256 校验值** 确认文件未被篡改，随后点击 **「更多信息 → 仍要运行」** 即可正常安装。

### 方式二：开发者从源码克隆构建

#### 1. 环境准备
* **Node.js** `>= 20`（含 npm）
* **Rust 工具链**（`rustup` + `cargo`，建议使用最新稳定版）
* **Windows 10 / 11**（需具备 WebView2 Runtime 与 Visual Studio C++ Build Tools）

#### 2. 安装依赖并启动开发模式

```bash
# 克隆仓库
git clone https://github.com/reneryi7/BreezeMusic-Desktop.git
cd BreezeMusic-Desktop

# 安装前端依赖
npm install

# 启动 Tauri 桌面开发模式（自动拉起 Vite 热更新与原生桌面窗口）
npm run tauri dev
```

> 说明：在源码仓库目录运行开发模式时，运行时会话、缓存与本地配置会自动隔离至 `%LOCALAPPDATA%\BreezeMusic-Dev`，不会污染项目源码目录。

#### 3. 类型检查、Rust 单元测试与生产打包

```bash
# 前端 TypeScript 类型检查与生产资源构建
npm run build

# 运行 Rust 后端单元测试（涵盖凭证加密、SSRF 拦截、标签解析与字体处理等）
cargo test --lib --manifest-path src-tauri/Cargo.toml

# 打包 Windows 正式安装包（NSIS .exe 与 MSI）
npm run tauri build
```

打包完成后，安装包产物位于 `src-tauri/target/release/bundle/` 目录下。

---

## 五、项目目录结构

```text
BreezeMusic-Desktop/
├── .github/
│   └── ISSUE_TEMPLATE/             # Bug 反馈与功能建议表单模板
├── index.html                      # 首屏入口与防闪烁主题/排版/动效预加载脚本
├── public/                         # 静态 Favicon 图标资源
├── package.json                    # 前端依赖、脚本与版本定义
├── vite.config.ts                  # Vite 构建配置
├── tsconfig.json                   # TypeScript 编译配置
├── src/                            # Vue 3 + TypeScript 前端源码
│   ├── api/                        # Tauri IPC 通信封装与数据模型规范化
│   ├── components/                 # 顶栏、播控底栏、桌面歌词、Mini小窗、心流舱、热评、设置子面板与弹窗
│   ├── composables/                # 浮窗主题联动、无限滚动与视口错峰动效组合式函数
│   ├── directives/                 # 自定义指令 (3D 倾斜交互等)
│   ├── stores/                     # Pinia 核心状态与 DSP / 曲库 / 发现 / 搜索 / 专注舱子模块
│   ├── styles/                     # 四主题 Design Tokens、零边框组件与动效/设置中心样式
│   ├── utils/                      # OKLab 取色、WebGL 光学背景、120Hz 歌词弹簧/自适应网格、DSP 算法、字体/动效/快捷键运行时
│   └── views/                      # 探索发现、音乐库、云盘音乐、本地音乐、调音台、搜索、设置中心、沉浸播放页
├── src-tauri/                      # Rust + Tauri 2.0 原生后端源码
│   ├── Cargo.toml                  # Rust Crate 依赖与 Release 优化配置
│   ├── Cargo.lock                  # 锁定依赖版本以确保可复现构建
│   ├── tauri.conf.json             # 窗口、CSP 安全策略、多窗体权限与 Windows 打包配置
│   ├── capabilities/               # 主窗体、桌面歌词、Mini 灵动岛独立权限清单
│   ├── icons/                      # 应用与托盘图标资源
│   └── src/
│       ├── main.rs                 # Windows 桌面入口
│       ├── lib.rs                  # Tauri IPC 命令注册、系统托盘、全局热键与多窗体调度
│       ├── single_instance.rs      # Win32 原生会话级单实例守卫与跨进程焦点唤醒
│       ├── window_transition.rs    # D3D11 + DirectComposition 主窗/小窗 GPU 空间过渡引擎
│       └── kugou/                  # 纯 Rust 协议签名、传输网关、音频代理、KRC 解密、分享解析、DPAPI 凭证存储与标签引擎
├── CHANGELOG.md                    # 版本更新日志 (Keep a Changelog)
├── THIRD_PARTY_NOTICES.md          # 第三方开源组件与上游协议致谢声明
└── LICENSE                         # GNU General Public License v3.0
```

---

## 六、使用小贴士

* **关于实验性离线拓展模块**：为保持界面默认纯净合规，客户端默认隐藏所有离线下载入口。如个人本地学习测试确需使用离线缓存与内嵌标签/封面功能，可前往 **「设置 → 关于微风音乐」**，连续点击版本号徽章（`v1.1.2`）**5 次** 解锁「下载管理」入口；解锁后可在同一页面的「项目说明与本地安全」卡片中随时一键关闭并重新隐藏。
* **版本更新记录**：所有历史版本的完整变更明细均同步维护在 [CHANGELOG.md](CHANGELOG.md) 中，并与 [GitHub Releases](https://github.com/reneryi7/BreezeMusic-Desktop/releases) 保持一致。

---

## 七、免责与版权声明

1. **非商业学习性质**：本项目（**BreezeMusic Desktop / 微风音乐桌面版**）为完全开源、非营利性质的个人软件工程与跨端架构技术探索项目，旨在实践 Rust + Tauri 2.0 桌面原生开发、WebGL2 着色器渲染、WebAudio 实时数字信号处理（DSP）及现代化无框界面设计。本项目仅供个人学习、技术交流与编程研究使用，严禁用于任何商业牟利、付费打包售卖或违规分发。
2. **版权归属声明**：本客户端与任何商业音乐平台及原厂均无从属或官方关联，应用本身不存储、不托管、不分发任何受版权保护的音频、歌词或封面资源。播放与展示内容均由用户端直连公开网络接口获取，其著作权与邻接权均归原版权方与唱片公司所有。请支持正版音乐，自觉遵守当地法律法规与相关服务条款；任何个人学习测试产生的本地缓存文件请于 24 小时内自行清理。

---

## 八、开源致谢

* [MakcRe/KuGouMusicApi](https://github.com/MakcRe/KuGouMusicApi)：提供了详实的酷狗概念版协议研究基础（MIT License）。
* [Tauri](https://tauri.app/) & [Rust](https://www.rust-lang.org/)、[Vue 3](https://vuejs.org/) & [Pinia](https://pinia.vuejs.org/)、[lofty-rs](https://github.com/Serial-ATA/lofty-rs)：提供了强大的桌面端、前端与音频元数据生态支持。
* 更多上游开源依赖与协议说明请参见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

---

## 九、开源许可证 (License)

本项目基于 **[GNU General Public License v3.0 (GPL-3.0)](LICENSE)** 协议开源，与安卓端 BreezeMusic 保持完全一致。任何人均可自由学习、修改与分享，但任何基于本项目的衍生作品或二次分发版本均须保持相同的 GPL-3.0 开源协议，且不得闭源用于商业牟利。

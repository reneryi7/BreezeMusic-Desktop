import type { SettingsSectionId } from "../types";

export const APP_VERSION = "1.1.1";
export const APP_VERSION_TAG = `v${APP_VERSION}`;

export interface SettingsNavItem {
  id: SettingsSectionId;
  group: "core" | "media" | "system";
  title: string;
  subtitle: string;
  icon: string;
  accentVar: string;
  hiddenUnlessUnlocked?: boolean;
}

export interface SettingsSearchEntry {
  id: string;
  section: SettingsSectionId;
  sectionTitle: string;
  title: string;
  description: string;
  keywords: string[];
}

export const SETTINGS_NAV_GROUPS: ReadonlyArray<{
  id: "core" | "media" | "system";
  label: string;
}> = [
  { id: "core", label: "核心偏好" },
  { id: "media", label: "影音体验" },
  { id: "system", label: "资产与系统" },
];

export const SETTINGS_NAV_ITEMS: ReadonlyArray<SettingsNavItem> = [
  {
    id: "appearance",
    group: "core",
    title: "外观与主题",
    subtitle: "四套氛围 · 动效与密度 · 播放舞台",
    icon: "palette-orb",
    accentVar: "var(--primary)",
  },
  {
    id: "typography",
    group: "core",
    title: "字体与排版",
    subtitle: "字体库 · 字号缩放 · 字重与行距",
    icon: "font-plus",
    accentVar: "var(--violet)",
  },
  {
    id: "general",
    group: "core",
    title: "常规与窗口",
    subtitle: "默认首页 · 托盘 · 账号与自启动",
    icon: "settings",
    accentVar: "var(--cyan)",
  },
  {
    id: "playback",
    group: "media",
    title: "播放与音质",
    subtitle: "无损 · 声卡 · 淡变与音量保护",
    icon: "headphones",
    accentVar: "var(--mint)",
  },
  {
    id: "lyrics",
    group: "media",
    title: "歌词与桌面歌词",
    subtitle: "桌面悬浮窗 · 播放页排版动效",
    icon: "lyric",
    accentVar: "var(--violet)",
  },
  {
    id: "local_music",
    group: "system",
    title: "本地音乐",
    subtitle: "多目录管理 · 扫描过滤 · 元数据缓存",
    icon: "music",
    accentVar: "var(--mint)",
  },
  {
    id: "downloads",
    group: "system",
    title: "下载管理",
    subtitle: "并发队列 · 下载记录 · 标签与命名",
    icon: "download",
    accentVar: "var(--gold)",
    hiddenUnlessUnlocked: true,
  },
  {
    id: "shortcuts",
    group: "system",
    title: "快捷键",
    subtitle: "系统全局热键 · 应用内按键",
    icon: "sliders",
    accentVar: "var(--coral)",
  },
  {
    id: "storage_network",
    group: "system",
    title: "存储与网络",
    subtitle: "磁盘缓存 · 配额 · 网关诊断",
    icon: "shield",
    accentVar: "var(--cyan)",
  },
  {
    id: "about",
    group: "system",
    title: "关于微风音乐",
    subtitle: `版本 ${APP_VERSION_TAG} · 架构与说明`,
    icon: "sparkles",
    accentVar: "var(--primary)",
  },
];

export const SETTINGS_SEARCH_INDEX: ReadonlyArray<SettingsSearchEntry> = [
  // 外观与主题
  {
    id: "appearance-theme-matrix",
    section: "appearance",
    sectionTitle: "外观与主题",
    title: "主题氛围与明暗模式 (D1/D2/L1/L2)",
    description: "切换黑曜石极光、丝绒星云、棉纸水彩、晨雾燕麦或跟随系统",
    keywords: [
      "主题",
      "浅色",
      "深色",
      "暗黑",
      "水彩",
      "燕麦",
      "曜石",
      "丝绒",
      "星云",
      "跟随系统",
      "外观",
      "颜色",
      "d1",
      "d2",
      "l1",
      "l2",
    ],
  },
  {
    id: "appearance-motion-density",
    section: "appearance",
    sectionTitle: "外观与主题",
    title: "界面动效等级与信息密度",
    description: "切换最佳效果 / 普通效果 / 无效果动效档位，以及紧凑 / 标准 / 舒展列表密度",
    keywords: [
      "动效",
      "动画",
      "最佳效果",
      "普通效果",
      "无效果",
      "减弱动态",
      "流畅",
      "密度",
      "信息密度",
      "紧凑",
      "标准",
      "舒展",
      "间距",
      "motion",
      "density",
    ],
  },
  {
    id: "appearance-palette-preview",
    section: "appearance",
    sectionTitle: "外观与主题",
    title: "当前主题色彩氛围与徽章预览",
    description: "展开查看当前主题主调、辅色与音质状态徽章视觉对照",
    keywords: ["色彩", "色板", "徽章", "hires", "sq", "vip", "主色", "配色"],
  },
  {
    id: "appearance-cover-color-mode",
    section: "appearance",
    sectionTitle: "外观与主题",
    title: "播放页封面动态取色策略",
    description: "关闭跟随四主题 / 随明暗动态漫染 / 始终锁定深色暗室",
    keywords: ["封面取色", "取色", "漫染", "暗室", "oklab", "播放页颜色", "双主色"],
  },
  {
    id: "appearance-player-bg",
    section: "appearance",
    sectionTitle: "外观与主题",
    title: "播放页高定光学背景",
    description: "选择液态丝绸、长虹声幕、声致驻波或星尘梦境",
    keywords: ["背景", "液态丝绸", "长虹声幕", "声致驻波", "星尘梦境", "webgl", "光影"],
  },
  {
    id: "appearance-player-stage",
    section: "appearance",
    sectionTitle: "外观与主题",
    title: "默认播放舞台形态",
    description: "在黑胶唱机转盘与画廊海报舞台之间切换",
    keywords: ["黑胶", "唱片", "转盘", "海报", "舞台", "封面形态"],
  },

  // 字体与排版
  {
    id: "typography-preview",
    section: "typography",
    sectionTitle: "字体与排版",
    title: "字体选择弹窗与三作用域配置",
    description: "通过独立字体预览窗口挑选系统字体或导入本地字体，分别作用于全局界面、播放页歌词与桌面歌词",
    keywords: [
      "字体",
      "排版",
      "字形",
      "预览",
      "弹窗",
      "全局字体",
      "界面字体",
      "歌词字体",
      "桌面歌词字体",
      "系统字体",
      "导入字体",
      "ttf",
      "otf",
      "woff2",
      "ttc",
    ],
  },
  {
    id: "typography-weight-scale",
    section: "typography",
    sectionTitle: "字体与排版",
    title: "字号缩放 (75%–145%)、字重粗细与全局行距",
    description: "自由调节全局文字缩放比例、五级联动字重粗细、正文行距、西文混排与等宽数字",
    keywords: [
      "字重",
      "粗细",
      "可变字体",
      "缩放",
      "字号",
      "行距",
      "行高",
      "等宽数字",
      "混排",
      "weight",
      "scale",
      "line-height",
      "tabular",
    ],
  },

  // 常规与窗口
  {
    id: "general-account-summary",
    section: "general",
    sectionTitle: "常规与窗口",
    title: "账号会话与概念 VIP 状态",
    description: "管理多账号、查看概念 VIP 权益并同步云端听歌等级与时长",
    keywords: ["账号", "登录", "多账号", "切换账号", "vip", "会员", "等级", "听歌时长"],
  },
  {
    id: "general-startup-home",
    section: "general",
    sectionTitle: "常规与窗口",
    title: "启动默认首页与自动签到",
    description: "设定冷启动默认打开探索、音乐库、本地音乐或上次浏览页，以及每日概念 VIP 自动签到",
    keywords: [
      "启动页",
      "默认首页",
      "首页",
      "签到",
      "自动签到",
      "启动",
    ],
  },
  {
    id: "general-close-to-tray",
    section: "general",
    sectionTitle: "常规与窗口",
    title: "窗口关闭策略与开机自启动",
    description: "关闭主窗口时最小化至系统托盘，或配置 Windows 登录自动启动",
    keywords: ["托盘", "关闭", "最小化", "后台", "退出", "开机", "自启动", "autostart"],
  },
  {
    id: "general-mini-yield-fullscreen",
    section: "general",
    sectionTitle: "常规与窗口",
    title: "全屏视频/游戏时自动隐藏 Mini 灵动岛",
    description: "进入真正的 F11、全屏视频或全屏游戏时自动礼让隐藏灵动岛，退出后瞬时恢复",
    keywords: ["全屏", "礼让", "自动隐藏", "灵动岛", "小窗", "mini", "置顶", "游戏", "视频"],
  },
  {
    id: "general-auto-refresh",
    section: "general",
    sectionTitle: "常规与窗口",
    title: "后台静默同步与 Mini 灵动岛",
    description: "查看最近自动同步时间、立即全量刷新或切入置顶 Mini 小窗",
    keywords: ["刷新", "同步", "mini", "迷你", "灵动岛", "小窗"],
  },

  // 播放与音质
  {
    id: "playback-quality",
    section: "playback",
    sectionTitle: "播放与音质",
    title: "默认在线流媒体音质",
    description: "设定优先播放 Hi-Res 高解析、SQ 无损 FLAC、HQ 320K 或标准 128K",
    keywords: ["音质", "无损", "flac", "hires", "hi-res", "320k", "128k", "码率", "sq", "hq"],
  },
  {
    id: "playback-output-device",
    section: "playback",
    sectionTitle: "播放与音质",
    title: "音频输出设备（声卡 / 耳机）",
    description: "枚举并实时切换系统声卡、外置 DAC 或蓝牙耳机输出通道",
    keywords: ["声卡", "输出设备", "耳机", "扬声器", "dac", "音频设备", "通道"],
  },
  {
    id: "playback-crossfade",
    section: "playback",
    sectionTitle: "播放与音质",
    title: "跨曲无缝淡变、防爆音与响度均衡",
    description: "配置 0–8 秒交叉淡入淡出、暂停淡变、ReplayGain 跨曲响度与夜间等响度补偿",
    keywords: [
      "淡入淡出",
      "交叉",
      "crossfade",
      "无缝",
      "防爆音",
      "音量平衡",
      "响度",
      "replaygain",
      "lufs",
      "夜间",
      "等响度",
    ],
  },
  {
    id: "playback-bluetooth-delay",
    section: "playback",
    sectionTitle: "播放与音质",
    title: "蓝牙耳机音频延迟校准",
    description: "调节 -500ms 至 +1000ms 补偿蓝牙编码延时，对齐逐字歌词",
    keywords: ["蓝牙", "延迟", "延时", "声画同步", "校准", "耳机延迟"],
  },
  {
    id: "playback-auto-play-startup",
    section: "playback",
    sectionTitle: "播放与音质",
    title: "启动时自动继续播放",
    description: "默认静默记忆上次播放队列与进度，可选开启启动时自动从上次时间点继续出声播放",
    keywords: ["自动播放", "继续播放", "启动播放", "恢复播放", "续播", "开机播放"],
  },
  {
    id: "playback-behavior",
    section: "playback",
    sectionTitle: "播放与音质",
    title: "播放模式、切歌恢复倍速与开机安全音量",
    description: "管理循环/随机模式、切歌自动重置 1.0x 倍速、启动防炸耳音量上限与母带调音台",
    keywords: [
      "循环",
      "随机",
      "顺序",
      "副歌",
      "高潮",
      "调音台",
      "eq",
      "倍速",
      "速度",
      "安全音量",
      "防炸耳",
      "音量上限",
    ],
  },

  // 歌词与桌面歌词
  {
    id: "lyrics-desktop-main",
    section: "lyrics",
    sectionTitle: "歌词与桌面歌词",
    title: "桌面悬浮歌词总控、穿透锁定与布局",
    description: "开关独立透明置顶歌词窗、鼠标穿透、布局样式、字号、描边与显隐控制",
    keywords: [
      "桌面歌词",
      "悬浮歌词",
      "穿透",
      "锁定",
      "归位",
      "置顶",
      "描边",
      "透明度",
      "ktv",
      "单行",
      "双行",
      "逐字",
      "翻译",
    ],
  },
  {
    id: "lyrics-stage-prefs",
    section: "lyrics",
    sectionTitle: "歌词与桌面歌词",
    title: "沉浸播放页歌词排版与动效偏好",
    description: "设置播放页歌词字体预设、字重、字号、对齐、失焦模糊、点击跳转与时间偏移",
    keywords: [
      "播放页歌词",
      "歌词字体",
      "对齐",
      "模糊",
      "点击跳转",
      "时间偏移",
      "偏移",
      "歌词设置",
    ],
  },

  // 本地音乐（常驻独立）
  {
    id: "local-music-dirs",
    section: "local_music",
    sectionTitle: "本地音乐",
    title: "本地音乐文件夹与多目录管理",
    description: "添加、切换或移除本地扫描目录，一键扫描并直接进入本地曲库",
    keywords: ["本地音乐", "本地曲库", "本地文件夹", "扫描目录", "导入本地", "flac", "mp3", "wav"],
  },
  {
    id: "local-music-rules",
    section: "local_music",
    sectionTitle: "本地音乐",
    title: "本地扫描过滤规则、去重与元数据优先级",
    description: "配置启动自动扫描、递归子目录、过滤短音频/小文件、自动去重与内嵌标签优先",
    keywords: [
      "子目录",
      "短音频",
      "过滤",
      "去重",
      "标签",
      "文件名",
      "元数据",
      "重建缓存",
      "封面缓存",
      "自动扫描",
    ],
  },

  // 下载管理（仅解锁后可见）
  {
    id: "downloads-queue",
    section: "downloads",
    sectionTitle: "下载管理",
    title: "并发下载队列与实时任务调度",
    description: "支持 1–3 路并发、实时速率与剩余时间、暂停恢复、置顶优先与取消清理",
    keywords: ["下载队列", "任务", "下载进度", "并发", "速率", "暂停", "取消", "重试"],
  },
  {
    id: "downloads-history",
    section: "downloads",
    sectionTitle: "下载管理",
    title: "下载记录归档与直接播放",
    description: "检索历史下载曲目、按音质筛选、直接播放本地文件、定位目录或重新下载",
    keywords: ["下载记录", "下载历史", "已下载", "定位文件", "重新下载"],
  },
  {
    id: "downloads-prefs",
    section: "downloads",
    sectionTitle: "下载管理",
    title: "下载目录、命名规则与纯 Rust 标签内嵌",
    description: "配置下载音质、保存路径、文件命名归档方式与 FLAC/MP3 封面及双语歌词写入",
    keywords: ["下载目录", "保存路径", "命名", "归档", "内嵌封面", "lrc", "双语歌词", "跳过已存在"],
  },

  // 快捷键
  {
    id: "shortcuts-global-toggle",
    section: "shortcuts",
    sectionTitle: "快捷键",
    title: "系统级全局热键总开关",
    description: "开启后在最小化或切换其他程序时仍可响应自定义全局热键与多媒体键",
    keywords: ["全局热键", "快捷键", "热键", "多媒体键", "后台控制"],
  },
  {
    id: "shortcuts-global-list",
    section: "shortcuts",
    sectionTitle: "快捷键",
    title: "自定义系统级全局热键",
    description: "点击任意键帽组合即可录制新快捷键，实时同步 Win32 并检测占用冲突",
    keywords: ["改键", "录制快捷键", "自定义快捷键", "冲突", "桌面歌词解锁快捷键", "mini快捷键"],
  },
  {
    id: "shortcuts-inapp-list",
    section: "shortcuts",
    sectionTitle: "快捷键",
    title: "自定义应用内快捷键",
    description: "自定义窗口聚焦时的播控、快速搜索、音量微调、收藏与返回键位",
    keywords: ["应用内快捷键", "窗口快捷键", "搜索快捷键", "空格", "space", "ctrl+k"],
  },

  // 存储与网络
  {
    id: "storage-cache-dashboard",
    section: "storage_network",
    sectionTitle: "存储与网络",
    title: "四维磁盘缓存占用与配额管理",
    description: "实时查看并分项清理音频流、高清封面、歌单快照与歌词缓存，配置 LRU 磁盘配额",
    keywords: ["缓存", "清理缓存", "磁盘", "空间", "存储", "音频缓存", "封面缓存", "配额", "lru"],
  },
  {
    id: "storage-gateway-diag",
    section: "storage_network",
    sectionTitle: "存储与网络",
    title: "音源网关延迟诊断与 DFID 设备标识",
    description: "检测酷狗概念版网关连通性与耗时，查看或重签 24 位 DFID 设备身份",
    keywords: ["网络", "网关", "诊断", "延迟", "dfid", "guid", "mid", "重签", "设备标识"],
  },
  {
    id: "storage-data-maintenance",
    section: "storage_network",
    sectionTitle: "存储与网络",
    title: "历史与偏好数据维护",
    description: "清空搜索历史或将歌词排版恢复为出厂默认",
    keywords: ["重置", "恢复默认", "搜索历史", "清空记录"],
  },

  // 关于
  {
    id: "about-brand",
    section: "about",
    sectionTitle: "关于微风音乐",
    title: `BreezeMusic ${APP_VERSION_TAG}、项目渊源与引擎架构`,
    description: `查看当前软件版本 ${APP_VERSION_TAG}、安卓微风音乐 Windows 重构说明、Rust/Tauri/WebAudio 引擎与数据目录`,
    keywords: ["关于", "版本", APP_VERSION_TAG, APP_VERSION, "开源", "安卓", "tauri", "数据目录", "架构"],
  },
];

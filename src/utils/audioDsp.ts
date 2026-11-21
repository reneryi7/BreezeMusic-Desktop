import type {
  AcousticFieldSettings,
  AcousticFrequencyZone,
  AudiophileTargetCurve,
  EqBandCountMode,
  PeqFilterType,
  ReverbPresetId,
  ViperEffectItem,
} from "../types";

/**
 * 50 dB 人耳听觉对数音量响应曲线 (Weber-Fechner / 专业调音台音频电位器)
 * 彻底解决线性音量下耳机 1% 就震耳、0%~2% 占满可用区间的问题：
 *  - 0%   -> 0.0000 (-∞ dB)
 *  - 1%   -> 0.00019 (-74.5 dB，极轻耳语)
 *  - 10%  -> 0.00247 (-52.1 dB)
 *  - 28%  -> 0.0127  (-37.9 dB，约等价原线性 1% 响度，但下方拥有 28 档细腻微调)
 *  - 50%  -> 0.0532  (-25.5 dB，日常耳机舒适听音区)
 *  - 80%  -> 0.314   (-10.1 dB)
 *  - 100% -> 1.000   (0 dB 满幅输出)
 */
const PERCEPTUAL_VOL_EXPONENT = 2.5; // 10^2.5 ≈ 316.23 -> 50 dB 动态范围
const PERCEPTUAL_VOL_DENOM = Math.pow(10, PERCEPTUAL_VOL_EXPONENT) - 1;

export function sliderToPerceptualGain(sliderVal: number): number {
  const v = Math.max(0, Math.min(1, Number(sliderVal) || 0));
  if (v <= 0) return 0;
  if (v >= 1) return 1;
  return (Math.pow(10, PERCEPTUAL_VOL_EXPONENT * v) - 1) / PERCEPTUAL_VOL_DENOM;
}

export function perceptualGainToSlider(linearGain: number): number {
  const g = Math.max(0, Math.min(1, Number(linearGain) || 0));
  if (g <= 0) return 0;
  if (g >= 1) return 1;
  return Math.log10(1 + g * PERCEPTUAL_VOL_DENOM) / PERCEPTUAL_VOL_EXPONENT;
}

export const DEFAULT_TEN_BAND_HZ: number[] = [
  31.25, 62.5, 125, 250, 500, 1000, 2000, 4000, 8000, 16000,
];

export const DEFAULT_TEN_BAND_LABELS: string[] = [
  "31",
  "62",
  "125",
  "250",
  "500",
  "1k",
  "2k",
  "4k",
  "8k",
  "16k",
];

export const DEFAULT_TEN_BAND_Q: number[] = [
  1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2,
];

export const DEFAULT_TEN_BAND_TYPES: PeqFilterType[] = [
  "peaking",
  "peaking",
  "peaking",
  "peaking",
  "peaking",
  "peaking",
  "peaking",
  "peaking",
  "peaking",
  "peaking",
];

/** 15 段 ISO 2/3 倍频程标准中心频率 (Hz) */
export const DEFAULT_FIFTEEN_BAND_HZ: number[] = [
  25, 40, 63, 100, 160, 250, 400, 630, 1000, 1600, 2500, 4000, 6300, 10000, 16000,
];

export const DEFAULT_FIFTEEN_BAND_LABELS: string[] = [
  "25",
  "40",
  "63",
  "100",
  "160",
  "250",
  "400",
  "630",
  "1k",
  "1.6k",
  "2.5k",
  "4k",
  "6.3k",
  "10k",
  "16k",
];

export const DEFAULT_FIFTEEN_BAND_Q: number[] = Array(15).fill(2.15);

export const DEFAULT_FIFTEEN_BAND_TYPES: PeqFilterType[] = Array(15).fill("peaking");

/** 31 段母带级 ISO 1/3 倍频程标准中心频率 (Hz) */
export const DEFAULT_THIRTY_ONE_BAND_HZ: number[] = [
  20, 25, 31.5, 40, 50, 63, 80, 100, 125, 160, 200, 250, 315, 400, 500, 630, 800,
  1000, 1250, 1600, 2000, 2500, 3150, 4000, 5000, 6300, 8000, 10000, 12500, 16000,
  20000,
];

export const DEFAULT_THIRTY_ONE_BAND_LABELS: string[] = [
  "20",
  "25",
  "31.5",
  "40",
  "50",
  "63",
  "80",
  "100",
  "125",
  "160",
  "200",
  "250",
  "315",
  "400",
  "500",
  "630",
  "800",
  "1k",
  "1.25k",
  "1.6k",
  "2k",
  "2.5k",
  "3.15k",
  "4k",
  "5k",
  "6.3k",
  "8k",
  "10k",
  "12.5k",
  "16k",
  "20k",
];

export const DEFAULT_THIRTY_ONE_BAND_Q: number[] = Array(31).fill(4.32);

export const DEFAULT_THIRTY_ONE_BAND_TYPES: PeqFilterType[] =
  Array(31).fill("peaking");

/** 依据频段分辨率获取标准中心频率 / 标签 / 默认 Q 值 / 默认拓扑 */
export function getBandPresetConfig(count: EqBandCountMode): {
  count: EqBandCountMode;
  centerHz: number[];
  labels: string[];
  qFactors: number[];
  bandTypes: PeqFilterType[];
  defaultQ: number;
} {
  if (count === 31) {
    return {
      count: 31,
      centerHz: [...DEFAULT_THIRTY_ONE_BAND_HZ],
      labels: [...DEFAULT_THIRTY_ONE_BAND_LABELS],
      qFactors: [...DEFAULT_THIRTY_ONE_BAND_Q],
      bandTypes: [...DEFAULT_THIRTY_ONE_BAND_TYPES],
      defaultQ: 4.32,
    };
  }
  if (count === 15) {
    return {
      count: 15,
      centerHz: [...DEFAULT_FIFTEEN_BAND_HZ],
      labels: [...DEFAULT_FIFTEEN_BAND_LABELS],
      qFactors: [...DEFAULT_FIFTEEN_BAND_Q],
      bandTypes: [...DEFAULT_FIFTEEN_BAND_TYPES],
      defaultQ: 2.15,
    };
  }
  return {
    count: 10,
    centerHz: [...DEFAULT_TEN_BAND_HZ],
    labels: [...DEFAULT_TEN_BAND_LABELS],
    qFactors: [...DEFAULT_TEN_BAND_Q],
    bandTypes: [...DEFAULT_TEN_BAND_TYPES],
    defaultQ: 1.2,
  };
}

/** 将数值频率格式化为紧凑的声学读数标签 */
export function formatFrequencyLabel(hz: number): string {
  const safe = Math.max(16, Math.min(24000, Number(hz) || 1000));
  if (safe >= 10000) return `${(safe / 1000).toFixed(safe % 1000 === 0 ? 0 : 1)}k`;
  if (safe >= 1000) {
    const k = safe / 1000;
    return `${Number.isInteger(k) ? k : Number(k.toFixed(2))}k`;
  }
  return safe < 45 && !Number.isInteger(safe) ? safe.toFixed(1) : `${Math.round(safe)}`;
}

/** 声学 5 大频段语义功能区，用于在调音台频响图上提供人性化听感引导 */
export const ACOUSTIC_FREQUENCY_ZONES: AcousticFrequencyZone[] = [
  {
    id: "sub",
    label: "超低潜深",
    subLabel: "SUB",
    minHz: 20,
    maxHz: 60,
    colorToken: "--violet",
    description: "胸腔震动与极低频下潜感，适度微调可增加重量，过多易触发防削波挤压。",
  },
  {
    id: "bass",
    label: "鼓点质感",
    subLabel: "BASS",
    minHz: 60,
    maxHz: 250,
    colorToken: "--coral",
    description: "底鼓弹性、贝斯轮廓与暖度核心；200Hz 附近轻微衰减可消除发闷箱音。",
  },
  {
    id: "lowmid",
    label: "醇厚中频",
    subLabel: "BODY",
    minHz: 250,
    maxHz: 1200,
    colorToken: "--gold",
    description: "决定乐器骨架与人声磁性，300–500Hz 控制过度堆积，800Hz+ 支撑音色饱满。",
  },
  {
    id: "presence",
    label: "临场解析",
    subLabel: "PRESENCE",
    minHz: 1200,
    maxHz: 6000,
    colorToken: "--primary",
    description: "人声咬字清晰度与吉他拨弦质感核心区，3kHz 附近极具存在感，5–6kHz 需防齿音。",
  },
  {
    id: "air",
    label: "空气泛音",
    subLabel: "AIR",
    minHz: 6000,
    maxHz: 20000,
    colorToken: "--cyan",
    description: "决定声场空间空气感、擦片细腻度与泛音光泽；10kHz+ 适合微幅平滑高频架起。",
  },
];

/**
 * 在对数频率坐标轴上将源频点增益平滑重采样到目标频点（如 10段 <-> 15段 <-> 31段）
 * 使用对数域单调平滑三次插值（分段 Catmull-Rom + 端点钳位），切换频段数时不丢失听感曲线
 */
export function resampleEqCurveToTargetFreqs(
  sourceHz: number[],
  sourceGains: number[],
  targetHz: number[]
): number[] {
  const len = Math.min(sourceHz.length, sourceGains.length);
  if (len <= 0) return targetHz.map(() => 0);

  const pts = Array.from({ length: len }, (_, i) => ({
    x: Math.log10(Math.max(20, Number(sourceHz[i]) || 1000)),
    y: Math.max(-15, Math.min(15, Number(sourceGains[i]) || 0)),
  })).sort((a, b) => a.x - b.x);

  return targetHz.map((hz) => {
    const tx = Math.log10(Math.max(20, hz));
    if (tx <= pts[0].x) return Number(pts[0].y.toFixed(1));
    if (tx >= pts[pts.length - 1].x) {
      return Number(pts[pts.length - 1].y.toFixed(1));
    }

    let idx = 0;
    while (idx < pts.length - 2 && pts[idx + 1].x < tx) {
      idx++;
    }
    const p0 = pts[Math.max(0, idx - 1)];
    const p1 = pts[idx];
    const p2 = pts[Math.min(pts.length - 1, idx + 1)];
    const p3 = pts[Math.min(pts.length - 1, idx + 2)];
    const span = Math.max(1e-6, p2.x - p1.x);
    const t = Math.max(0, Math.min(1, (tx - p1.x) / span));

    const t2 = t * t;
    const t3 = t2 * t;
    const raw =
      0.5 *
      (2 * p1.y +
        (-p0.y + p2.y) * t +
        (2 * p0.y - 5 * p1.y + 4 * p2.y - p3.y) * t2 +
        (-p0.y + 3 * p1.y - 3 * p2.y + p3.y) * t3);
    const localMin = Math.min(p1.y, p2.y) - 0.8;
    const localMax = Math.max(p1.y, p2.y) + 0.8;
    const clamped = Math.max(
      -15,
      Math.min(15, Math.max(localMin, Math.min(localMax, raw)))
    );
    return Number((Math.round(clamped * 10) / 10).toFixed(1));
  });
}

/** 切换均衡器分辨率时重映射中心频率、增益、Q 值与端带拓扑 */
export function resampleBandParameters(params: {
  oldHz: number[];
  oldGains: number[];
  oldQ: number[];
  oldTypes: PeqFilterType[];
  targetCount: EqBandCountMode;
}): {
  centerHz: number[];
  gains: number[];
  qFactors: number[];
  bandTypes: PeqFilterType[];
} {
  const cfg = getBandPresetConfig(params.targetCount);
  const gains = resampleEqCurveToTargetFreqs(
    params.oldHz,
    params.oldGains,
    cfg.centerHz
  );
  const bandTypes = [...cfg.bandTypes];
  if (params.oldTypes[0] === "lowshelf") {
    bandTypes[0] = "lowshelf";
  }
  if (
    params.oldTypes.length > 0 &&
    params.oldTypes[params.oldTypes.length - 1] === "highshelf"
  ) {
    bandTypes[bandTypes.length - 1] = "highshelf";
  }
  return {
    centerHz: [...cfg.centerHz],
    gains,
    qFactors: [...cfg.qFactors],
    bandTypes,
  };
}

export const BASS_CROSSOVER_OPTIONS: Array<{
  hz: number;
  label: string;
  tag: string;
}> = [
  { hz: 40, label: "40Hz", tag: "极深下潜" },
  { hz: 55, label: "55Hz", tag: "影院轰鸣" },
  { hz: 60, label: "60Hz", tag: "黄金基准" },
  { hz: 75, label: "75Hz", tag: "流行弹性" },
  { hz: 95, label: "95Hz", tag: "紧凑鼓点" },
  { hz: 110, label: "110Hz", tag: "暖厚包围" },
];

export interface ReverbRoomPhysics {
  id: ReverbPresetId;
  label: string;
  subtitle: string;
  rt60Sec: number;
  defaultDamping: number;
  wetBase: number;
  hpHz: number;
  baseLpHz: number;
  tailGain: number;
  earlyScale: number;
  earlyTaps: Array<[number, number, number]>;
}

export const REVERB_ROOM_MODELS: Array<{
  id: ReverbPresetId;
  label: string;
  subtitle: string;
  rt60Sec: number;
  defaultDamping: number;
}> = [
  {
    id: "none",
    label: "无混响",
    subtitle: "纯净干声直通",
    rt60Sec: 0,
    defaultDamping: 30,
  },
  {
    id: "small_room",
    label: "温馨小房",
    subtitle: "近场木质反射",
    rt60Sec: 0.44,
    defaultDamping: 52,
  },
  {
    id: "medium_room",
    label: "声学中房",
    subtitle: "家庭试听空间",
    rt60Sec: 0.78,
    defaultDamping: 44,
  },
  {
    id: "live_house",
    label: "LiveHouse",
    subtitle: "前排紧致现场",
    rt60Sec: 1.08,
    defaultDamping: 38,
  },
  {
    id: "concert_hall",
    label: "交响音乐厅",
    subtitle: "金色大厅包络",
    rt60Sec: 1.82,
    defaultDamping: 28,
  },
  {
    id: "arena_stadium",
    label: "万人体育场",
    subtitle: "恢弘远场回响",
    rt60Sec: 2.48,
    defaultDamping: 22,
  },
  {
    id: "cathedral",
    label: "神圣大教堂",
    subtitle: "穹顶长尾余韵",
    rt60Sec: 3.18,
    defaultDamping: 18,
  },
  {
    id: "studio_plate",
    label: "录音室板式",
    subtitle: "丝滑密实尾音",
    rt60Sec: 1.32,
    defaultDamping: 42,
  },
];

export const REVERB_ROOM_PHYSICS: Record<ReverbPresetId, ReverbRoomPhysics> = {
  none: {
    id: "none",
    label: "无混响",
    subtitle: "纯净干声直通",
    rt60Sec: 0,
    defaultDamping: 30,
    wetBase: 0,
    hpHz: 200,
    baseLpHz: 6200,
    tailGain: 0,
    earlyScale: 0,
    earlyTaps: [],
  },
  small_room: {
    id: "small_room",
    label: "温馨小房",
    subtitle: "近场木质反射",
    rt60Sec: 0.44,
    defaultDamping: 52,
    wetBase: 0.24,
    hpHz: 210,
    baseLpHz: 5400,
    tailGain: 0.28,
    earlyScale: 0.85,
    earlyTaps: [
      [6.5, 0.36, 0.24],
      [11.2, -0.26, 0.34],
      [16.8, 0.22, -0.21],
      [23.5, 0.16, 0.19],
      [31.0, -0.12, 0.14],
    ],
  },
  medium_room: {
    id: "medium_room",
    label: "声学中房",
    subtitle: "家庭试听空间",
    rt60Sec: 0.78,
    defaultDamping: 44,
    wetBase: 0.27,
    hpHz: 195,
    baseLpHz: 5800,
    tailGain: 0.34,
    earlyScale: 0.78,
    earlyTaps: [
      [9.5, 0.38, 0.28],
      [16.4, -0.29, 0.35],
      [25.2, 0.24, -0.22],
      [35.8, 0.19, 0.23],
      [47.5, -0.15, 0.16],
    ],
  },
  large_room: {
    id: "large_room",
    label: "宏阔客厅",
    subtitle: "开阔木石折射",
    rt60Sec: 1.02,
    defaultDamping: 40,
    wetBase: 0.30,
    hpHz: 185,
    baseLpHz: 6000,
    tailGain: 0.37,
    earlyScale: 0.72,
    earlyTaps: [
      [12.0, 0.36, 0.29],
      [21.0, -0.28, 0.34],
      [32.5, 0.25, -0.24],
      [46.0, 0.20, 0.22],
      [62.0, -0.15, 0.18],
    ],
  },
  live_house: {
    id: "live_house",
    label: "LiveHouse",
    subtitle: "前排紧致现场",
    rt60Sec: 1.08,
    defaultDamping: 38,
    wetBase: 0.31,
    hpHz: 180,
    baseLpHz: 6600,
    tailGain: 0.40,
    earlyScale: 0.92,
    earlyTaps: [
      [10.8, 0.44, 0.31],
      [18.6, -0.35, 0.42],
      [27.4, 0.31, -0.28],
      [39.2, 0.25, 0.29],
      [54.0, -0.20, 0.21],
    ],
  },
  medium_hall: {
    id: "medium_hall",
    label: "室内音乐厅",
    subtitle: "均衡立体漫反射",
    rt60Sec: 1.45,
    defaultDamping: 34,
    wetBase: 0.35,
    hpHz: 175,
    baseLpHz: 6400,
    tailGain: 0.43,
    earlyScale: 0.76,
    earlyTaps: [
      [15.0, 0.38, 0.28],
      [25.5, -0.30, 0.36],
      [38.2, 0.26, -0.25],
      [53.4, 0.22, 0.24],
      [71.0, -0.17, 0.19],
    ],
  },
  concert_hall: {
    id: "concert_hall",
    label: "交响音乐厅",
    subtitle: "金色大厅包络",
    rt60Sec: 1.82,
    defaultDamping: 28,
    wetBase: 0.38,
    hpHz: 165,
    baseLpHz: 6800,
    tailGain: 0.46,
    earlyScale: 0.74,
    earlyTaps: [
      [17.5, 0.39, 0.30],
      [29.8, -0.32, 0.38],
      [44.6, 0.28, -0.26],
      [62.4, 0.23, 0.27],
      [83.5, -0.18, 0.20],
    ],
  },
  large_hall: {
    id: "large_hall",
    label: "大剧院穹顶",
    subtitle: "深邃宽广残响",
    rt60Sec: 2.18,
    defaultDamping: 24,
    wetBase: 0.42,
    hpHz: 160,
    baseLpHz: 6900,
    tailGain: 0.49,
    earlyScale: 0.68,
    earlyTaps: [
      [21.0, 0.37, 0.28],
      [36.5, -0.31, 0.36],
      [54.0, 0.27, -0.25],
      [76.0, 0.22, 0.25],
      [102.0, -0.18, 0.21],
    ],
  },
  arena_stadium: {
    id: "arena_stadium",
    label: "万人体育场",
    subtitle: "恢弘远场回响",
    rt60Sec: 2.48,
    defaultDamping: 22,
    wetBase: 0.45,
    hpHz: 155,
    baseLpHz: 6300,
    tailGain: 0.52,
    earlyScale: 0.96,
    earlyTaps: [
      [31.0, 0.42, 0.26],
      [54.5, -0.33, 0.40],
      [82.0, 0.30, -0.29],
      [114.0, 0.24, 0.28],
      [152.0, -0.21, 0.23],
    ],
  },
  cathedral: {
    id: "cathedral",
    label: "神圣大教堂",
    subtitle: "穹顶长尾余韵",
    rt60Sec: 3.18,
    defaultDamping: 18,
    wetBase: 0.52,
    hpHz: 150,
    baseLpHz: 7400,
    tailGain: 0.56,
    earlyScale: 0.62,
    earlyTaps: [
      [26.0, 0.34, 0.27],
      [44.5, -0.29, 0.33],
      [68.0, 0.26, -0.24],
      [96.0, 0.23, 0.24],
      [132.0, -0.19, 0.21],
    ],
  },
  studio_plate: {
    id: "studio_plate",
    label: "录音室板式",
    subtitle: "丝滑密实尾音",
    rt60Sec: 1.32,
    defaultDamping: 42,
    wetBase: 0.30,
    hpHz: 255,
    baseLpHz: 7800,
    tailGain: 0.38,
    earlyScale: 0.55,
    earlyTaps: [
      [3.2, 0.34, -0.32],
      [6.8, -0.28, 0.31],
      [11.4, 0.25, 0.24],
      [17.6, -0.20, -0.22],
      [24.8, 0.17, -0.18],
    ],
  },
  plate: {
    id: "plate",
    label: "经典钢板混响",
    subtitle: "高频晶莹延音",
    rt60Sec: 1.42,
    defaultDamping: 38,
    wetBase: 0.31,
    hpHz: 265,
    baseLpHz: 8000,
    tailGain: 0.39,
    earlyScale: 0.52,
    earlyTaps: [
      [2.8, 0.35, -0.33],
      [6.1, -0.29, 0.32],
      [10.9, 0.25, 0.23],
      [16.8, -0.21, -0.22],
      [23.5, 0.16, -0.18],
    ],
  },
};

export function getReverbRoomPhysics(preset: ReverbPresetId): ReverbRoomPhysics {
  return REVERB_ROOM_PHYSICS[preset] || REVERB_ROOM_PHYSICS.small_room;
}

/**
 * 发烧友级目标频响参考曲线（1:1 迁移自 Android AudioEffectManager.kt AudiophileTargetCurve）
 */
export const AUDIOPHILE_TARGET_CURVES: AudiophileTargetCurve[] = [
  {
    id: "harman_2019",
    label: "哈曼曲线 Harman 2019",
    subtitle: "双耳参考低频下潜与耳廓 3kHz 自然补偿",
    badge: "HARMAN IE",
    gains: [5.2, 4.4, 2.2, 0.2, -0.4, 0.8, 2.8, 3.8, 2.4, 1.4],
    qFactors: [0.85, 0.9, 1.1, 1.2, 1.3, 1.2, 1.1, 1.25, 1.4, 0.95],
  },
  {
    id: "diffuse_field",
    label: "弥散场 Diffuse Field",
    subtitle: "录音室开阔空间声场均衡参考",
    badge: "DIFFUSE",
    gains: [1.2, 1.0, 0.6, 0.0, 0.4, 1.6, 3.2, 4.2, 3.4, 2.6],
    qFactors: [1.0, 1.0, 1.1, 1.2, 1.2, 1.1, 1.0, 1.15, 1.3, 1.0],
  },
  {
    id: "vocal_silk",
    label: "人声丝绒 Vocal Silk",
    subtitle: "突出温润贴耳中频，柔化 6kHz 齿音毛刺",
    badge: "VOCAL PRO",
    gains: [-1.8, -0.8, 0.8, 2.2, 3.8, 4.6, 4.2, 1.8, -1.6, 1.2],
    qFactors: [0.9, 1.0, 1.1, 1.1, 1.0, 0.95, 1.1, 1.4, 2.2, 1.0],
  },
  {
    id: "hifi_sub_bass",
    label: "HiFi 深潜低频",
    subtitle: "仅增强 60Hz 以下极低频氛围，绝不污染中频",
    badge: "SUB-BASS",
    gains: [6.8, 5.6, 2.4, -0.8, -0.4, 0.2, 1.2, 2.0, 2.6, 2.8],
    qFactors: [0.75, 0.85, 1.35, 1.6, 1.2, 1.2, 1.15, 1.2, 1.1, 0.9],
  },
  {
    id: "studio_monitor",
    label: "监听平直 Studio Flat",
    subtitle: "微调两端空气感与极低频滚降补偿",
    badge: "MONITOR",
    gains: [1.6, 1.2, 0.4, 0.0, 0.0, 0.2, 0.8, 1.4, 1.8, 2.0],
    qFactors: [0.9, 1.0, 1.2, 1.2, 1.2, 1.2, 1.2, 1.2, 1.1, 0.9],
  },
];

/**
 * 10 款经典流派均衡器预设（对齐 Android EffectPreset isEqPreset=true）
 */
export const CLASSIC_EQ_PRESETS: Record<
  string,
  { label: string; desc: string; gains: number[] }
> = {
  flat: {
    label: "平直原声",
    desc: "全频段 0dB 监听基准直通",
    gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  },
  dynamic: {
    label: "动态增强",
    desc: "宽动态瞬态强化，高低频两端延伸清晰有力",
    gains: [5.8, 4.8, 2.2, -0.4, -1.2, 0.8, 3.0, 4.8, 6.2, 6.8],
  },
  pop: {
    label: "现代流行",
    desc: "人声与底鼓黄金比例，中低频紧致且人声通透贴耳",
    gains: [4.2, 3.6, 1.4, -0.2, 1.2, 3.4, 4.6, 4.0, 3.2, 2.6],
  },
  rock: {
    label: "摇滚动感",
    desc: "经典 V 型频响曲线，架子鼓与电吉他冲击力强",
    gains: [5.6, 4.8, 2.6, -1.4, -3.8, -2.2, 2.4, 4.8, 6.0, 6.4],
  },
  classical: {
    label: "古典交响",
    desc: "宽广动态范围与弦乐空气泛音延伸，还原交响乐团层次",
    gains: [2.4, 2.2, 1.0, 0.2, 0.0, 0.8, 2.4, 4.0, 5.6, 6.8],
  },
  jazz: {
    label: "慵懒爵士",
    desc: "突出木质贝斯弹拨质感与萨克斯管温润光泽",
    gains: [3.2, 4.0, 3.4, 2.2, 1.4, 2.4, 3.4, 2.6, 1.6, 0.8],
  },
  dance: {
    label: "动感舞曲",
    desc: "强化 60Hz 底鼓冲击与 10kHz 打击乐脆度",
    gains: [7.6, 6.6, 3.8, 0.4, -2.8, -1.6, 2.0, 4.6, 6.2, 6.8],
  },
  hiphop: {
    label: "说唱嘻哈",
    desc: "厚重 808 Sub-Bass 下潜与人声中高频清晰咬字",
    gains: [8.4, 7.2, 4.2, 1.2, -0.6, 1.6, 3.6, 4.0, 3.8, 3.6],
  },
  vocal: {
    label: "清澈人声",
    desc: "滤除低频驻波浑浊，聚焦中频人声唇齿细节",
    gains: [-3.8, -2.0, -0.4, 1.8, 4.2, 5.8, 6.2, 4.0, 2.0, 1.0],
  },
  soft: {
    label: "轻柔助眠",
    desc: "柔化中高频刺激感，营造松弛温暖的睡前听感",
    gains: [2.8, 2.6, 1.8, 0.8, -0.4, -1.6, -3.2, -4.6, -5.8, -6.8],
  },
};

/**
 * 本地 36 款官方母带蝰蛇音效基准库（覆盖官方旗舰、3D空间、曲风人声、复古介质与桌面/耳机/车载全场景）
 */
export const FALLBACK_VIPER_PRESETS: ViperEffectItem[] = [
  {
    id: "viper_3d",
    modeId: "1001",
    label: "3D 丽音",
    desc: "环绕立体声场与晶莹高频泛音，给你极致听觉感受",
    category: "官方旗舰",
    usageBadge: "5.4亿人使用",
    authorName: "蝰蛇声学实验室",
    acousticTags: ["3D声场", "高频晶莹", "旗舰王牌"],
    gains: [4.4, 3.4, 1.0, -1.6, -2.4, 1.0, 3.8, 5.6, 6.8, 7.2],
    subBass: 40,
    bassPunch: 28,
    bassCrossoverHz: 60,
    surround: 68,
    stageDepth: 44,
    crossfeedLevel: 16,
    vocalClarity: 52,
    deEsserLevel: 24,
    airHarmonics: 52,
    tubeWarmth: 12,
    transientPunch: 34,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.8,
    reverbPreset: "small_room",
    reverbWetPercent: 20,
    reverbPreDelayMs: 18,
    reverbDamping: 34,
  },
  {
    id: "viper_bass",
    modeId: "1002",
    label: "超重低音",
    desc: "澎湃低频下潜与弹性瞬态冲击，低音浑厚不轰头",
    category: "官方旗舰",
    usageBadge: "5.8亿人使用",
    authorName: "蝰蛇声学实验室",
    acousticTags: ["深沉下潜", "55Hz分频", "节奏澎湃"],
    gains: [8.4, 7.6, 4.6, 1.4, -2.2, -2.8, -1.2, 0.8, 1.8, 2.2],
    subBass: 82,
    bassPunch: 74,
    bassCrossoverHz: 55,
    surround: 20,
    stageDepth: 16,
    crossfeedLevel: 14,
    vocalClarity: 22,
    deEsserLevel: 24,
    airHarmonics: 16,
    tubeWarmth: 36,
    transientPunch: 56,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.6,
    reverbPreset: "none",
    reverbWetPercent: 0,
    reverbPreDelayMs: 14,
    reverbDamping: 35,
  },
  {
    id: "viper_vocal",
    modeId: "1003",
    label: "纯净人声",
    desc: "还原音频中每一个人声呼吸细节，中频温润贴耳",
    category: "官方旗舰",
    usageBadge: "4.2亿人使用",
    authorName: "蝰蛇声学实验室",
    acousticTags: ["人声贴耳", "齿音柔化", "通透中频"],
    gains: [-4.8, -2.8, -0.6, 2.0, 4.6, 6.6, 7.0, 4.8, 2.4, 1.2],
    subBass: 4,
    bassPunch: 8,
    bassCrossoverHz: 95,
    surround: 18,
    stageDepth: 16,
    crossfeedLevel: 28,
    vocalClarity: 78,
    deEsserLevel: 44,
    airHarmonics: 28,
    tubeWarmth: 30,
    transientPunch: 18,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.2,
    reverbPreset: "small_room",
    reverbWetPercent: 14,
    reverbPreDelayMs: 22,
    reverbDamping: 46,
  },
  {
    id: "viper_hifi",
    modeId: "1004",
    label: "HiFi 现场",
    desc: "逼真丰富的现场微动态与宽阔舞台结像感",
    category: "官方旗舰",
    usageBadge: "3.1亿人使用",
    authorName: "蝰蛇声学实验室",
    acousticTags: ["临场还原", "微动态", "高保真"],
    gains: [2.6, 2.2, 1.0, 0.4, 0.8, 1.8, 3.2, 4.4, 5.2, 5.6],
    subBass: 26,
    bassPunch: 30,
    bassCrossoverHz: 60,
    surround: 48,
    stageDepth: 54,
    crossfeedLevel: 26,
    vocalClarity: 48,
    deEsserLevel: 24,
    airHarmonics: 48,
    tubeWarmth: 18,
    transientPunch: 48,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.4,
    reverbPreset: "medium_room",
    reverbWetPercent: 22,
    reverbPreDelayMs: 22,
    reverbDamping: 32,
  },
  {
    id: "viper_panorama_51",
    modeId: "1005",
    label: "5.1 全景声",
    desc: "多声道球形全息影院声场，沉浸式环绕听感",
    category: "官方旗舰",
    usageBadge: "2.5亿人使用",
    authorName: "蝰蛇全景声实验室",
    acousticTags: ["5.1多声道", "分轨空间", "影院包裹"],
    gains: [4.8, 3.8, 1.6, 0.0, 0.4, 2.4, 4.8, 5.8, 6.4, 6.8],
    subBass: 44,
    bassPunch: 36,
    bassCrossoverHz: 55,
    surround: 78,
    stageDepth: 76,
    crossfeedLevel: 46,
    vocalClarity: 66,
    deEsserLevel: 24,
    airHarmonics: 44,
    tubeWarmth: 16,
    transientPunch: 34,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.0,
    reverbPreset: "medium_room",
    reverbWetPercent: 26,
    reverbPreDelayMs: 26,
    reverbDamping: 26,
  },
  {
    id: "viper_black_diamond",
    modeId: "1006",
    label: "黑钻蝰蛇 · 母带",
    desc: "多重谐波修复技术，还原黑胶母带光泽与微细节",
    category: "官方旗舰",
    usageBadge: "1.9亿人使用",
    authorName: "酷狗母带工程组",
    acousticTags: ["谐波修复", "母带重塑", "VIP臻享"],
    gains: [4.6, 3.8, 2.0, 1.2, 1.8, 2.8, 4.4, 5.8, 6.4, 5.8],
    subBass: 34,
    bassPunch: 46,
    bassCrossoverHz: 60,
    surround: 32,
    stageDepth: 28,
    crossfeedLevel: 20,
    vocalClarity: 58,
    deEsserLevel: 28,
    airHarmonics: 64,
    tubeWarmth: 44,
    transientPunch: 50,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.0,
    reverbPreset: "studio_plate",
    reverbWetPercent: 12,
    reverbPreDelayMs: 14,
    reverbDamping: 36,
  },
  {
    id: "viper_clear_master",
    modeId: "1007",
    label: "蝰蛇 · 超清母带",
    desc: "192kHz 采样率声学补偿，展现极高频空气泛音",
    category: "官方旗舰",
    usageBadge: "2.2亿人使用",
    authorName: "酷狗母带工程组",
    acousticTags: ["192kHz感", "空气泛音", "超高解析"],
    gains: [2.0, 1.6, 0.6, 0.2, 1.4, 3.4, 5.6, 7.2, 8.6, 9.2],
    subBass: 18,
    bassPunch: 24,
    bassCrossoverHz: 60,
    surround: 36,
    stageDepth: 24,
    crossfeedLevel: 16,
    vocalClarity: 74,
    deEsserLevel: 38,
    airHarmonics: 82,
    tubeWarmth: 6,
    transientPunch: 58,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.6,
    reverbPreset: "none",
    reverbWetPercent: 0,
    reverbPreDelayMs: 14,
    reverbDamping: 28,
  },
  {
    id: "viper_3d_rotate",
    modeId: "2001",
    label: "3D 旋转环绕",
    desc: "声源 360° 环绕头部动态轨迹旋转，灵动空间漫游",
    category: "3D空间",
    usageBadge: "1.6亿人使用",
    authorName: "空间声学团队",
    acousticTags: ["360°轨迹", "双耳渲染", "头部环绕"],
    gains: [4.4, 3.2, 0.8, -1.6, -2.2, 1.4, 4.0, 5.6, 6.6, 6.4],
    subBass: 30,
    bassPunch: 24,
    bassCrossoverHz: 60,
    surround: 82,
    stageDepth: 62,
    crossfeedLevel: 10,
    vocalClarity: 36,
    deEsserLevel: 22,
    airHarmonics: 34,
    tubeWarmth: 12,
    transientPunch: 22,
    surroundRotationSpeed: 76,
    loudnessBoost: 2.8,
    reverbPreset: "concert_hall",
    reverbWetPercent: 28,
    reverbPreDelayMs: 22,
    reverbDamping: 32,
  },
  {
    id: "viper_surround",
    modeId: "2002",
    label: "全景环绕",
    desc: "360° 沉浸式声场扩展，打破耳机头中效应",
    category: "3D空间",
    usageBadge: "2.1亿人使用",
    authorName: "空间声学团队",
    acousticTags: ["全息声场", "消除头中感", "宽阔包裹"],
    gains: [4.4, 3.4, 1.2, -1.6, -2.4, 1.4, 4.2, 5.8, 7.0, 7.4],
    subBass: 34,
    bassPunch: 24,
    bassCrossoverHz: 60,
    surround: 88,
    stageDepth: 72,
    crossfeedLevel: 12,
    vocalClarity: 36,
    deEsserLevel: 22,
    airHarmonics: 42,
    tubeWarmth: 12,
    transientPunch: 26,
    surroundRotationSpeed: 18,
    loudnessBoost: 3.0,
    reverbPreset: "concert_hall",
    reverbWetPercent: 36,
    reverbPreDelayMs: 28,
    reverbDamping: 26,
  },
  {
    id: "viper_concert",
    modeId: "2003",
    label: "大型演唱会",
    desc: "万人露天体育场恢弘声学反射与舞台纵深感",
    category: "3D空间",
    usageBadge: "1.3亿人使用",
    authorName: "空间声学团队",
    acousticTags: ["万人场馆", "远场混响", "恢弘声场"],
    gains: [6.4, 5.4, 2.8, 0.2, -1.2, 1.8, 4.2, 5.4, 6.0, 5.4],
    subBass: 54,
    bassPunch: 48,
    bassCrossoverHz: 60,
    surround: 72,
    stageDepth: 76,
    crossfeedLevel: 20,
    vocalClarity: 44,
    deEsserLevel: 24,
    airHarmonics: 36,
    tubeWarmth: 22,
    transientPunch: 40,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.6,
    reverbPreset: "arena_stadium",
    reverbWetPercent: 44,
    reverbPreDelayMs: 42,
    reverbDamping: 22,
  },
  {
    id: "viper_livehouse",
    modeId: "2004",
    label: "Livehouse 前排",
    desc: "近场舞台监听与热烈低频空气感，犹如置身第一排",
    category: "3D空间",
    usageBadge: "9400万人使用",
    authorName: "现场声学调音师",
    acousticTags: ["近场舞台", "乐队现场", "鼓点扎实"],
    gains: [6.4, 5.4, 2.8, 0.0, -1.8, 1.4, 4.0, 5.2, 5.6, 4.6],
    subBass: 46,
    bassPunch: 70,
    bassCrossoverHz: 60,
    surround: 50,
    stageDepth: 42,
    crossfeedLevel: 16,
    vocalClarity: 46,
    deEsserLevel: 24,
    airHarmonics: 38,
    tubeWarmth: 26,
    transientPunch: 68,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.4,
    reverbPreset: "live_house",
    reverbWetPercent: 28,
    reverbPreDelayMs: 14,
    reverbDamping: 32,
  },
  {
    id: "viper_studio",
    modeId: "2005",
    label: "录音棚监听",
    desc: "近场声学消音室精准结像，还原混音师母带监听台",
    category: "3D空间",
    usageBadge: "7600万人使用",
    authorName: "母带录音室团队",
    acousticTags: ["精准结像", "近场监听", "三频均衡"],
    gains: [1.2, 0.8, 0.2, 0.0, 0.4, 1.2, 2.2, 3.0, 3.4, 3.2],
    subBass: 10,
    bassPunch: 18,
    bassCrossoverHz: 60,
    surround: 14,
    stageDepth: 18,
    crossfeedLevel: 44,
    vocalClarity: 52,
    deEsserLevel: 28,
    airHarmonics: 30,
    tubeWarmth: 8,
    transientPunch: 44,
    surroundRotationSpeed: 0,
    loudnessBoost: 1.4,
    reverbPreset: "studio_plate",
    reverbWetPercent: 10,
    reverbPreDelayMs: 10,
    reverbDamping: 48,
  },
  {
    id: "viper_vinyl",
    modeId: "3001",
    label: "黑胶唱片",
    desc: "温润复古模拟偶次谐波，柔和顺滑久听不累",
    category: "复古介质",
    usageBadge: "1.8亿人使用",
    authorName: "复古模拟实验室",
    acousticTags: ["偶次谐波", "温润模拟", "黑胶声底"],
    gains: [5.4, 4.8, 3.2, 2.2, 1.8, 1.4, 0.4, -1.8, -4.4, -6.8],
    subBass: 34,
    bassPunch: 22,
    bassCrossoverHz: 60,
    surround: 26,
    stageDepth: 28,
    crossfeedLevel: 28,
    vocalClarity: 38,
    deEsserLevel: 38,
    airHarmonics: 12,
    tubeWarmth: 72,
    transientPunch: 14,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.2,
    reverbPreset: "small_room",
    reverbWetPercent: 16,
    reverbPreDelayMs: 18,
    reverbDamping: 52,
  },
  {
    id: "viper_tape",
    modeId: "3002",
    label: "复古磁带机",
    desc: "80 年代 Walkman 二氧化铬磁带微饱和与温暖中低频",
    category: "复古介质",
    usageBadge: "1.2亿人使用",
    authorName: "复古模拟实验室",
    acousticTags: ["Walkman感", "磁带微饱和", "暖厚人声"],
    gains: [1.4, 4.6, 5.6, 4.2, 2.4, 0.6, -2.6, -5.2, -7.2, -8.8],
    subBass: 24,
    bassPunch: 30,
    bassCrossoverHz: 85,
    surround: 18,
    stageDepth: 20,
    crossfeedLevel: 26,
    vocalClarity: 16,
    deEsserLevel: 54,
    airHarmonics: 0,
    tubeWarmth: 88,
    transientPunch: 10,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.0,
    reverbPreset: "small_room",
    reverbWetPercent: 12,
    reverbPreDelayMs: 14,
    reverbDamping: 60,
  },
  {
    id: "viper_tube_amp",
    modeId: "3003",
    label: "黄金胆机电子管",
    desc: "300B 真空电子管甲类放大音色，人声醇厚富有磁性",
    category: "复古介质",
    usageBadge: "9600万人使用",
    authorName: "复古模拟实验室",
    acousticTags: ["300B胆味", "甲类放大", "醇厚磁性"],
    gains: [4.6, 5.0, 4.2, 3.4, 4.2, 4.4, 3.4, 2.4, 1.4, 0.6],
    subBass: 30,
    bassPunch: 26,
    bassCrossoverHz: 60,
    surround: 42,
    stageDepth: 48,
    crossfeedLevel: 22,
    vocalClarity: 54,
    deEsserLevel: 30,
    airHarmonics: 40,
    tubeWarmth: 94,
    transientPunch: 20,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.8,
    reverbPreset: "studio_plate",
    reverbWetPercent: 18,
    reverbPreDelayMs: 20,
    reverbDamping: 42,
  },
  {
    id: "viper_gramophone",
    modeId: "3004",
    label: "岁月留声机",
    desc: "复古号角中频聚焦与老唱片温暖年代质感",
    category: "复古介质",
    usageBadge: "5200万人使用",
    authorName: "复古模拟实验室",
    acousticTags: ["年代号角", "复古胶片", "叙事感"],
    gains: [-8.8, -6.6, -2.2, 3.8, 7.6, 8.6, 5.6, -2.4, -7.2, -9.2],
    subBass: 0,
    bassPunch: 6,
    bassCrossoverHz: 110,
    surround: 6,
    stageDepth: 14,
    crossfeedLevel: 44,
    vocalClarity: 42,
    deEsserLevel: 48,
    airHarmonics: 0,
    tubeWarmth: 80,
    transientPunch: 8,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.0,
    reverbPreset: "small_room",
    reverbWetPercent: 18,
    reverbPreDelayMs: 14,
    reverbDamping: 68,
  },
  {
    id: "viper_guofeng",
    modeId: "4001",
    label: "声乐古风",
    desc: "强化竹笛古筝空灵泛音与国风戏腔清亮穿透力",
    category: "曲风人声",
    usageBadge: "1.4亿人使用",
    authorName: "东方声学实验室",
    acousticTags: ["国风器乐", "戏腔清亮", "空灵余韵"],
    gains: [-2.8, -1.2, 0.8, 2.6, 5.0, 6.4, 7.0, 6.0, 5.0, 4.4],
    subBass: 10,
    bassPunch: 16,
    bassCrossoverHz: 75,
    surround: 52,
    stageDepth: 62,
    crossfeedLevel: 22,
    vocalClarity: 68,
    deEsserLevel: 30,
    airHarmonics: 58,
    tubeWarmth: 20,
    transientPunch: 30,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.4,
    reverbPreset: "concert_hall",
    reverbWetPercent: 32,
    reverbPreDelayMs: 30,
    reverbDamping: 28,
  },
  {
    id: "viper_folk",
    modeId: "4002",
    label: "民谣木吉他",
    desc: "突出木吉他拨弦箱体共鸣与温暖叙事男女声",
    category: "曲风人声",
    usageBadge: "9600万人使用",
    authorName: "原声声学实验室",
    acousticTags: ["原声吉他", "箱体共鸣", "温暖叙事"],
    gains: [0.6, 2.2, 3.6, 2.4, 3.8, 4.8, 5.2, 4.6, 3.8, 2.8],
    subBass: 12,
    bassPunch: 20,
    bassCrossoverHz: 60,
    surround: 34,
    stageDepth: 36,
    crossfeedLevel: 28,
    vocalClarity: 58,
    deEsserLevel: 28,
    airHarmonics: 46,
    tubeWarmth: 38,
    transientPunch: 52,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.2,
    reverbPreset: "small_room",
    reverbWetPercent: 22,
    reverbPreDelayMs: 20,
    reverbDamping: 36,
  },
  {
    id: "viper_acg",
    modeId: "4003",
    label: "ACG 燃曲女毒",
    desc: "高密度瞬态响应与清甜高频延伸，专为二次元定制",
    category: "曲风人声",
    usageBadge: "1.5亿人使用",
    authorName: "ACG调音研究社",
    acousticTags: ["清甜女毒", "高速瞬态", "华丽高频"],
    gains: [3.2, 2.4, 0.4, 0.8, 3.4, 5.8, 7.4, 7.8, 7.2, 6.4],
    subBass: 26,
    bassPunch: 50,
    bassCrossoverHz: 60,
    surround: 44,
    stageDepth: 30,
    crossfeedLevel: 18,
    vocalClarity: 74,
    deEsserLevel: 44,
    airHarmonics: 66,
    tubeWarmth: 8,
    transientPunch: 64,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.8,
    reverbPreset: "none",
    reverbWetPercent: 0,
    reverbPreDelayMs: 12,
    reverbDamping: 30,
  },
  {
    id: "viper_edm",
    modeId: "4004",
    label: "电音俱乐部",
    desc: "极速 40Hz 超低频冲程与合成器高频空气感，燃爆节拍",
    category: "曲风人声",
    usageBadge: "1.5亿人使用",
    authorName: "电音低频实验室",
    acousticTags: ["40Hz极深潜", "电音冲击", "高频空气"],
    gains: [8.8, 7.6, 4.4, 0.4, -3.4, -2.6, 1.8, 5.2, 7.4, 8.2],
    subBass: 84,
    bassPunch: 78,
    bassCrossoverHz: 40,
    surround: 60,
    stageDepth: 42,
    crossfeedLevel: 12,
    vocalClarity: 48,
    deEsserLevel: 26,
    airHarmonics: 62,
    tubeWarmth: 14,
    transientPunch: 74,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.8,
    reverbPreset: "live_house",
    reverbWetPercent: 22,
    reverbPreDelayMs: 20,
    reverbDamping: 26,
  },
  {
    id: "viper_rnb",
    modeId: "4005",
    label: "R&B 丝滑律动",
    desc: "饱满弹性的 808 贝斯线条与丝滑转音细节",
    category: "曲风人声",
    usageBadge: "7800万人使用",
    authorName: "都市律动工程组",
    acousticTags: ["808弹性", "丝滑转音", "律动氛围"],
    gains: [7.2, 6.4, 4.8, 2.8, 2.2, 3.4, 3.8, 2.2, 1.4, 0.8],
    subBass: 68,
    bassPunch: 42,
    bassCrossoverHz: 50,
    surround: 36,
    stageDepth: 34,
    crossfeedLevel: 24,
    vocalClarity: 52,
    deEsserLevel: 44,
    airHarmonics: 20,
    tubeWarmth: 62,
    transientPunch: 26,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.8,
    reverbPreset: "small_room",
    reverbWetPercent: 16,
    reverbPreDelayMs: 18,
    reverbDamping: 52,
  },
  {
    id: "viper_iem",
    modeId: "5001",
    label: "入耳式耳塞特调",
    desc: "平滑耳道 6kHz 刺耳谐振峰，拓宽入耳式横向声场",
    category: "设备车载",
    usageBadge: "1.1亿人使用",
    authorName: "参考声学实验室",
    acousticTags: ["耳道补偿", "去齿音毛刺", "Bauer互馈"],
    gains: [4.2, 3.2, 1.8, 0.8, 1.2, 2.0, -1.6, -4.6, 2.8, 5.4],
    subBass: 28,
    bassPunch: 24,
    bassCrossoverHz: 60,
    surround: 48,
    stageDepth: 40,
    crossfeedLevel: 58,
    vocalClarity: 36,
    deEsserLevel: 60,
    airHarmonics: 38,
    tubeWarmth: 24,
    transientPunch: 24,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.0,
    reverbPreset: "none",
    reverbWetPercent: 0,
    reverbPreDelayMs: 15,
    reverbDamping: 36,
  },
  {
    id: "viper_headphone_pro",
    modeId: "5002",
    label: "头戴式大耳声场",
    desc: "模拟开放式旗舰头戴大耳自然声场与顺滑哈曼三频",
    category: "设备车载",
    usageBadge: "1.1亿人使用",
    authorName: "参考声学实验室",
    acousticTags: ["开放式声场", "Bauer交叉馈送", "监听均衡"],
    gains: [5.2, 3.8, 2.0, 0.6, 0.4, 1.6, 3.2, 4.4, 5.2, 5.4],
    subBass: 42,
    bassPunch: 30,
    bassCrossoverHz: 45,
    surround: 60,
    stageDepth: 58,
    crossfeedLevel: 74,
    vocalClarity: 44,
    deEsserLevel: 26,
    airHarmonics: 40,
    tubeWarmth: 28,
    transientPunch: 30,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.4,
    reverbPreset: "small_room",
    reverbWetPercent: 18,
    reverbPreDelayMs: 18,
    reverbDamping: 36,
  },
  {
    id: "viper_tws",
    modeId: "5003",
    label: "TWS 真无线增强",
    desc: "补偿蓝牙 AAC/SBC 高频衰减，强化低频下潜与凝聚力",
    category: "设备车载",
    usageBadge: "1.7亿人使用",
    authorName: "无线声学工程组",
    acousticTags: ["蓝牙补偿", "高频修复", "低频凝聚"],
    gains: [5.6, 4.6, 2.2, 0.4, 1.2, 3.0, 4.8, 6.2, 7.6, 8.2],
    subBass: 48,
    bassPunch: 48,
    bassCrossoverHz: 60,
    surround: 44,
    stageDepth: 34,
    crossfeedLevel: 30,
    vocalClarity: 56,
    deEsserLevel: 28,
    airHarmonics: 68,
    tubeWarmth: 18,
    transientPunch: 54,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.0,
    reverbPreset: "none",
    reverbWetPercent: 0,
    reverbPreDelayMs: 14,
    reverbDamping: 30,
  },
  {
    id: "viper_car_sub",
    modeId: "5004",
    label: "豪车柏林之声",
    desc: "针对车厢玻璃反射声学优化的皇帝位环绕声场",
    category: "设备车载",
    usageBadge: "1.6亿人使用",
    authorName: "车载声学工程组",
    acousticTags: ["车载皇帝位", "多扬声器", "座舱声学"],
    gains: [6.2, 5.0, 2.4, 0.8, 1.0, 2.4, 4.2, 5.6, 6.8, 7.0],
    subBass: 52,
    bassPunch: 50,
    bassCrossoverHz: 55,
    surround: 68,
    stageDepth: 58,
    crossfeedLevel: 12,
    vocalClarity: 48,
    deEsserLevel: 26,
    airHarmonics: 58,
    tubeWarmth: 38,
    transientPunch: 42,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.4,
    reverbPreset: "medium_room",
    reverbWetPercent: 24,
    reverbPreDelayMs: 20,
    reverbDamping: 34,
  },
  {
    id: "viper_car_subwoofer",
    modeId: "5005",
    label: "车载重低音炮",
    desc: "强化车门与后备箱 40Hz 低音炮共鸣，高速行车依然澎湃",
    category: "设备车载",
    usageBadge: "1.0亿人使用",
    authorName: "车载声学工程组",
    acousticTags: ["车载低音炮", "胎噪补偿", "胸腔共鸣"],
    gains: [9.2, 8.2, 5.2, 1.8, -1.8, -1.4, 1.4, 2.8, 3.6, 3.8],
    subBass: 90,
    bassPunch: 74,
    bassCrossoverHz: 40,
    surround: 34,
    stageDepth: 30,
    crossfeedLevel: 14,
    vocalClarity: 28,
    deEsserLevel: 24,
    airHarmonics: 20,
    tubeWarmth: 38,
    transientPunch: 58,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.8,
    reverbPreset: "small_room",
    reverbWetPercent: 14,
    reverbPreDelayMs: 14,
    reverbDamping: 42,
  },
  {
    id: "viper_dolby_cinema",
    modeId: "2006",
    label: "杜比全景影院",
    desc: "模拟 IMAX 巨幕影院超宽声像与 35Hz 震撼低频下潜",
    category: "3D空间",
    usageBadge: "1.4亿人使用",
    authorName: "空间声学团队",
    acousticTags: ["巨幕声场", "35Hz极低频", "多维环绕"],
    gains: [7.6, 6.4, 3.2, 0.0, -1.6, 1.8, 4.2, 5.4, 6.6, 7.2],
    subBass: 80,
    bassPunch: 62,
    bassCrossoverHz: 45,
    surround: 84,
    stageDepth: 78,
    crossfeedLevel: 24,
    vocalClarity: 56,
    deEsserLevel: 24,
    airHarmonics: 52,
    tubeWarmth: 20,
    transientPunch: 56,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.8,
    reverbPreset: "concert_hall",
    reverbWetPercent: 30,
    reverbPreDelayMs: 30,
    reverbDamping: 28,
  },
  {
    id: "viper_symphony_hall",
    modeId: "2007",
    label: "维也纳金色大厅",
    desc: "1.9 秒黄金自然堂音混响，展现管弦交响恢弘层次",
    category: "3D空间",
    usageBadge: "8600万人使用",
    authorName: "古典母带工程组",
    acousticTags: ["交响混响", "大动态", "器乐分离"],
    gains: [2.2, 2.8, 2.0, 1.0, 0.4, 1.4, 2.8, 4.4, 5.8, 6.8],
    subBass: 20,
    bassPunch: 18,
    bassCrossoverHz: 55,
    surround: 66,
    stageDepth: 84,
    crossfeedLevel: 32,
    vocalClarity: 42,
    deEsserLevel: 20,
    airHarmonics: 68,
    tubeWarmth: 26,
    transientPunch: 34,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.4,
    reverbPreset: "cathedral",
    reverbWetPercent: 38,
    reverbPreDelayMs: 36,
    reverbDamping: 22,
  },
  {
    id: "viper_suona_soul",
    modeId: "4006",
    label: "唢呐灵魂震荡",
    desc: "精准强化 1.8kHz~4.5kHz 金属簧片穿透力与瞬态爆发感",
    category: "曲风人声",
    usageBadge: "9200万人使用",
    authorName: "东方声学实验室",
    acousticTags: ["灵魂穿透", "高频爆发", "国潮民乐"],
    gains: [2.0, 1.6, 1.0, 2.8, 5.8, 8.6, 8.2, 5.8, 4.4, 3.4],
    subBass: 24,
    bassPunch: 38,
    bassCrossoverHz: 65,
    surround: 50,
    stageDepth: 44,
    crossfeedLevel: 16,
    vocalClarity: 82,
    deEsserLevel: 30,
    airHarmonics: 66,
    tubeWarmth: 18,
    transientPunch: 78,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.8,
    reverbPreset: "live_house",
    reverbWetPercent: 22,
    reverbPreDelayMs: 20,
    reverbDamping: 30,
  },
  {
    id: "viper_midnight_warm",
    modeId: "3005",
    label: "深夜助眠暖耳",
    desc: "柔化高频毛刺与瞬态冲击，注入温暖模拟空气底色",
    category: "复古介质",
    usageBadge: "8900万人使用",
    authorName: "心理声学实验室",
    acousticTags: ["等响补偿", "柔顺护耳", "助眠伴侣"],
    gains: [4.2, 4.4, 3.2, 1.6, 0.4, -0.8, -2.6, -4.4, -5.8, -7.2],
    subBass: 36,
    bassPunch: 14,
    bassCrossoverHz: 65,
    surround: 32,
    stageDepth: 48,
    crossfeedLevel: 64,
    vocalClarity: 30,
    deEsserLevel: 72,
    airHarmonics: 4,
    tubeWarmth: 74,
    transientPunch: 6,
    surroundRotationSpeed: 0,
    loudnessBoost: 1.8,
    reverbPreset: "small_room",
    reverbWetPercent: 24,
    reverbPreDelayMs: 24,
    reverbDamping: 64,
  },
  {
    id: "viper_harman_ref",
    modeId: "5006",
    label: "哈曼参考曲线",
    desc: "遵循 Harman Target 2019 黄金声学包络，低频丰满高频顺滑",
    category: "设备车载",
    usageBadge: "1.3亿人使用",
    authorName: "参考声学实验室",
    acousticTags: ["Harman 2019", "参考级均衡", "高保真"],
    gains: [5.6, 4.8, 2.0, 0.0, -0.6, 1.2, 3.6, 4.4, 2.4, 1.2],
    subBass: 44,
    bassPunch: 38,
    bassCrossoverHz: 60,
    surround: 16,
    stageDepth: 14,
    crossfeedLevel: 26,
    vocalClarity: 48,
    deEsserLevel: 28,
    airHarmonics: 22,
    tubeWarmth: 10,
    transientPunch: 36,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.2,
    reverbPreset: "none",
    reverbWetPercent: 0,
    reverbPreDelayMs: 14,
    reverbDamping: 34,
  },
  {
    id: "viper_pop_studio",
    modeId: "4007",
    label: "流行人声臻享",
    desc: "流行金曲黄金三频比例，人声立体饱满且鼓点清脆悦耳",
    category: "曲风人声",
    usageBadge: "2.4亿人使用",
    authorName: "流行母带工程组",
    acousticTags: ["黄金流行", "人声质感", "均衡耐听"],
    gains: [4.2, 3.4, 1.6, 0.8, 2.2, 4.0, 4.4, 3.8, 3.4, 3.0],
    subBass: 36,
    bassPunch: 46,
    bassCrossoverHz: 60,
    surround: 28,
    stageDepth: 22,
    crossfeedLevel: 24,
    vocalClarity: 62,
    deEsserLevel: 34,
    airHarmonics: 40,
    tubeWarmth: 24,
    transientPunch: 46,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.4,
    reverbPreset: "studio_plate",
    reverbWetPercent: 14,
    reverbPreDelayMs: 14,
    reverbDamping: 40,
  },
  {
    id: "viper_rock_stage",
    modeId: "4008",
    label: "经典摇滚现场",
    desc: "凌厉电吉他失真张力与底鼓军鼓瞬态冲击，张弛澎湃",
    category: "曲风人声",
    usageBadge: "1.4亿人使用",
    authorName: "现场声学调音师",
    acousticTags: ["摇滚冲击", "电吉他泛音", "现场张力"],
    gains: [5.6, 4.8, 2.4, -1.6, -3.4, -1.6, 2.8, 5.2, 6.2, 6.6],
    subBass: 48,
    bassPunch: 66,
    bassCrossoverHz: 55,
    surround: 46,
    stageDepth: 38,
    crossfeedLevel: 18,
    vocalClarity: 40,
    deEsserLevel: 26,
    airHarmonics: 52,
    tubeWarmth: 34,
    transientPunch: 68,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.2,
    reverbPreset: "live_house",
    reverbWetPercent: 22,
    reverbPreDelayMs: 18,
    reverbDamping: 32,
  },
  {
    id: "viper_jazz_lounge",
    modeId: "4009",
    label: "爵士蓝调酒廊",
    desc: "突出萨克斯温润光泽与低音提琴木质拨弦弹性，松弛惬意",
    category: "曲风人声",
    usageBadge: "9800万人使用",
    authorName: "原声声学实验室",
    acousticTags: ["爵士酒廊", "管乐光泽", "木质低频"],
    gains: [3.4, 4.2, 3.8, 2.4, 1.8, 2.6, 3.6, 3.0, 2.0, 1.2],
    subBass: 30,
    bassPunch: 32,
    bassCrossoverHz: 65,
    surround: 44,
    stageDepth: 48,
    crossfeedLevel: 36,
    vocalClarity: 46,
    deEsserLevel: 34,
    airHarmonics: 26,
    tubeWarmth: 62,
    transientPunch: 28,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.0,
    reverbPreset: "studio_plate",
    reverbWetPercent: 26,
    reverbPreDelayMs: 24,
    reverbDamping: 44,
  },
  {
    id: "viper_laptop_speaker",
    modeId: "5007",
    label: "笔记本外放增强",
    desc: "补偿 Windows 笔记本微扬声器低频缺失与狭小声场，响亮不破音",
    category: "设备车载",
    usageBadge: "1.5亿人使用",
    authorName: "桌面声学工程组",
    acousticTags: ["外放补偿", "谐波丰满", "防破音"],
    gains: [-1.0, 4.2, 5.8, 3.2, 1.4, 3.0, 4.6, 4.8, 3.4, 2.0],
    subBass: 36,
    bassPunch: 56,
    bassCrossoverHz: 110,
    surround: 72,
    stageDepth: 46,
    crossfeedLevel: 6,
    vocalClarity: 64,
    deEsserLevel: 34,
    airHarmonics: 44,
    tubeWarmth: 38,
    transientPunch: 50,
    surroundRotationSpeed: 0,
    loudnessBoost: 3.6,
    reverbPreset: "small_room",
    reverbWetPercent: 14,
    reverbPreDelayMs: 12,
    reverbDamping: 42,
  },
  {
    id: "viper_desktop_hifi",
    modeId: "5008",
    label: "桌面 2.0 音箱",
    desc: "抑制 200Hz 桌面低频驻波浑浊，呈现近场脱箱立体声结像",
    category: "设备车载",
    usageBadge: "1.2亿人使用",
    authorName: "桌面声学工程组",
    acousticTags: ["近场脱箱", "去桌面驻波", "立体结像"],
    gains: [4.4, 3.2, -2.2, -2.4, 0.8, 2.0, 3.2, 4.0, 4.4, 4.6],
    subBass: 36,
    bassPunch: 32,
    bassCrossoverHz: 55,
    surround: 40,
    stageDepth: 44,
    crossfeedLevel: 10,
    vocalClarity: 48,
    deEsserLevel: 26,
    airHarmonics: 46,
    tubeWarmth: 18,
    transientPunch: 42,
    surroundRotationSpeed: 0,
    loudnessBoost: 2.2,
    reverbPreset: "none",
    reverbWetPercent: 0,
    reverbPreDelayMs: 16,
    reverbDamping: 36,
  },
];

/**
 * 64-Bit 二阶数字滤波器 (Biquad) 离散时间傅里叶变换幅频响应求解器
 * 严格对齐 Robert Bristow-Johnson《Audio EQ Cookbook》与 Android AudioEffectManager.evaluatePeqMagnitudeDbAtFrequency
 */
export function evaluatePeqMagnitudeDbAtFrequency(params: {
  freqHz: number;
  gains: number[];
  centerHzList: number[];
  qFactors: number[];
  bandTypes: PeqFilterType[];
  hpfHz: number;
  lpfHz: number;
  spectralTiltDb?: number;
  bypass?: boolean;
  acousticField?: AcousticFieldSettings;
  includeAcousticField?: boolean;
  equalLoudnessActive?: boolean;
  volume?: number;
  sampleRate?: number;
}): number {
  if (params.bypass) return 0;

  const sampleRate = params.sampleRate || 48000;
  const f = Math.max(18, Math.min(sampleRate * 0.48, params.freqHz));
  const w = (2 * Math.PI * f) / sampleRate;
  const cosW = Math.cos(w);
  const cos2W = Math.cos(2 * w);
  const sinW = Math.sin(w);
  const sin2W = Math.sin(2 * w);

  const biquadMagDb = (
    b0: number,
    b1: number,
    b2: number,
    a1: number,
    a2: number
  ): number => {
    const numRe = b0 + b1 * cosW + b2 * cos2W;
    const numIm = -(b1 * sinW + b2 * sin2W);
    const denRe = 1 + a1 * cosW + a2 * cos2W;
    const denIm = -(a1 * sinW + a2 * sin2W);
    const numSq = numRe * numRe + numIm * numIm;
    const denSq = Math.max(1e-18, denRe * denRe + denIm * denIm);
    return 10 * Math.log10(Math.max(1e-12, numSq / denSq));
  };

  const evalPeakingDb = (fc: number, q: number, gainDb: number): number => {
    if (Math.abs(gainDb) < 0.03) return 0;
    const w0 = (2 * Math.PI * fc) / sampleRate;
    const cosW0 = Math.cos(w0);
    const sinW0 = Math.sin(w0);
    const alpha = sinW0 / (2 * Math.max(0.25, q));
    const A = Math.pow(10, gainDb / 40);
    const b0 = 1 + alpha * A;
    const b1 = -2 * cosW0;
    const b2 = 1 - alpha * A;
    const a0 = 1 + alpha / A;
    const a1 = -2 * cosW0;
    const a2 = 1 - alpha / A;
    return biquadMagDb(b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
  };

  const evalLowShelfDb = (fc: number, q: number, gainDb: number): number => {
    if (Math.abs(gainDb) < 0.03) return 0;
    const w0 = (2 * Math.PI * fc) / sampleRate;
    const cosW0 = Math.cos(w0);
    const sinW0 = Math.sin(w0);
    const A = Math.pow(10, gainDb / 40);
    const sSlope = Math.max(0.35, Math.min(2.5, q / 1.2));
    const alphaS =
      (sinW0 / 2) * Math.sqrt((A + 1 / A) * (1 / sSlope - 1) + 2);
    const twoSqrtAAlpha = 2 * Math.sqrt(A) * alphaS;
    const b0 = A * (A + 1 - (A - 1) * cosW0 + twoSqrtAAlpha);
    const b1 = 2 * A * (A - 1 - (A + 1) * cosW0);
    const b2 = A * (A + 1 - (A - 1) * cosW0 - twoSqrtAAlpha);
    const a0 = A + 1 + (A - 1) * cosW0 + twoSqrtAAlpha;
    const a1 = -2 * (A - 1 + (A + 1) * cosW0);
    const a2 = A + 1 + (A - 1) * cosW0 - twoSqrtAAlpha;
    return biquadMagDb(b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
  };

  const evalHighShelfDb = (fc: number, q: number, gainDb: number): number => {
    if (Math.abs(gainDb) < 0.03) return 0;
    const w0 = (2 * Math.PI * fc) / sampleRate;
    const cosW0 = Math.cos(w0);
    const sinW0 = Math.sin(w0);
    const A = Math.pow(10, gainDb / 40);
    const sSlope = Math.max(0.35, Math.min(2.5, q / 1.2));
    const alphaS =
      (sinW0 / 2) * Math.sqrt((A + 1 / A) * (1 / sSlope - 1) + 2);
    const twoSqrtAAlpha = 2 * Math.sqrt(A) * alphaS;
    const b0 = A * (A + 1 + (A - 1) * cosW0 + twoSqrtAAlpha);
    const b1 = -2 * A * (A - 1 + (A + 1) * cosW0);
    const b2 = A * (A + 1 + (A - 1) * cosW0 - twoSqrtAAlpha);
    const a0 = A + 1 - (A - 1) * cosW0 + twoSqrtAAlpha;
    const a1 = 2 * (A - 1 - (A + 1) * cosW0);
    const a2 = A + 1 - (A - 1) * cosW0 - twoSqrtAAlpha;
    return biquadMagDb(b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
  };

  let totalDb = 0;

  // 1. 多段参量均衡器级联响应（支持 10 / 15 / 31 段）
  const activeBandCount = Math.min(
    31,
    Math.max(0, Math.min(params.gains.length, params.centerHzList.length))
  );
  for (let i = 0; i < activeBandCount; i++) {
    const gainDb = Number(params.gains[i] ?? 0);
    const fc = Math.max(
      20,
      Math.min(
        sampleRate * 0.46,
        Number(params.centerHzList[i] ?? DEFAULT_THIRTY_ONE_BAND_HZ[i] ?? 1000)
      )
    );
    const q = Math.max(0.25, Math.min(10, Number(params.qFactors[i] ?? 1.2)));
    const type: PeqFilterType = params.bandTypes[i] || "peaking";

    if (type === "peaking") {
      totalDb += evalPeakingDb(fc, q, gainDb);
    } else if (type === "lowshelf") {
      totalDb += evalLowShelfDb(fc, q, gainDb);
    } else if (type === "highshelf") {
      totalDb += evalHighShelfDb(fc, q, gainDb);
    } else if (type === "notch") {
      const cutDb =
        gainDb <= -0.25 ? gainDb : gainDb >= 0.25 ? -gainDb : -1.5;
      totalDb += evalPeakingDb(fc, Math.max(2.2, q * 2.2), cutDb);
    } else if (type === "bandpass") {
      const bpGain = Math.max(-10, Math.min(10, gainDb));
      totalDb += evalPeakingDb(fc, Math.max(1.4, q * 1.35), bpGain);
    }
  }

  // 1.5 非破坏性 Baxandall 频谱倾斜 (围绕 ~1kHz 枢纽逆向跷跷板架设双搁架)
  const tiltDb = Number(params.spectralTiltDb || 0);
  if (Math.abs(tiltDb) > 0.04) {
    totalDb += evalLowShelfDb(350, 1.2, -tiltDb);
    totalDb += evalHighShelfDb(2850, 1.2, tiltDb);
  }

  // 2. 高通滤波器 (HPF 2nd-Order Butterworth)
  if (params.hpfHz > 20 && params.hpfHz <= 250) {
    const w0 = (2 * Math.PI * params.hpfHz) / sampleRate;
    const cosW0 = Math.cos(w0);
    const alpha = Math.sin(w0) / (2 * 0.7071);
    const b0 = (1 + cosW0) / 2;
    const b1 = -(1 + cosW0);
    const b2 = (1 + cosW0) / 2;
    const a0 = 1 + alpha;
    const a1 = -2 * cosW0;
    const a2 = 1 - alpha;
    totalDb += biquadMagDb(b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
  }

  // 3. 低通滤波器 (LPF 2nd-Order Butterworth)
  if (params.lpfHz >= 6000 && params.lpfHz < 19800) {
    const w0 = (2 * Math.PI * params.lpfHz) / sampleRate;
    const cosW0 = Math.cos(w0);
    const alpha = Math.sin(w0) / (2 * 0.7071);
    const b0 = (1 - cosW0) / 2;
    const b1 = 1 - cosW0;
    const b2 = (1 - cosW0) / 2;
    const a0 = 1 + alpha;
    const a1 = -2 * cosW0;
    const a2 = 1 - alpha;
    totalDb += biquadMagDb(b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
  }

  // 4. 声场雕塑模块频响贡献 (可选叠加显示在总频响曲线上)
  const af = params.acousticField;
  if (params.includeAcousticField && af && af.enabled) {
    const cutoff = Math.max(40, Math.min(120, af.bassCrossoverHz || 60));
    const subNorm = Math.max(0, Math.min(1, (af.subBassDepth || 0) / 100));
    const punchNorm = Math.max(
      0,
      Math.min(1, (af.bassPunch ?? af.transientPunch ?? 0) / 100)
    );
    const transNorm = Math.max(0, Math.min(1, (af.transientPunch || 0) / 100));
    const vocalNorm = Math.max(0, Math.min(1, (af.vocalClarity || 0) / 100));
    const deEssNorm = Math.max(0, Math.min(1, (af.deEsserLevel ?? 20) / 100));
    const airNorm = Math.max(0, Math.min(1, (af.airHarmonics || 0) / 100));
    const warmthNorm = Math.max(
      0,
      Math.min(1, (af.vinylTubeWarmth || 0) / 100)
    );

    if (subNorm > 0.01) {
      totalDb += evalLowShelfDb(cutoff, 0.68, subNorm * 6.8 * 0.45);
    }
    if (punchNorm > 0.01) {
      totalDb += evalPeakingDb(cutoff * 1.32, 1.18, punchNorm * 4.5 * 0.45);
    }
    if (subNorm > 0.05 || punchNorm > 0.05) {
      const mudCut = -(subNorm * 1.6 + punchNorm * 1.0) * 0.45;
      totalDb += evalPeakingDb(235, 1.45, mudCut);
    }
    if (transNorm > 0.01) {
      totalDb += evalPeakingDb(3150, 1.15, transNorm * 2.8 * 0.45);
    }
    if (vocalNorm > 0.01) {
      totalDb += evalPeakingDb(265, 1.35, -vocalNorm * 1.6 * 0.45);
      totalDb += evalPeakingDb(2750, 1.08, vocalNorm * 3.8 * 0.45);
    }
    if (deEssNorm > 0.01) {
      totalDb += evalPeakingDb(6200, 2.35, -deEssNorm * 3.2 * 0.45);
    }
    if (warmthNorm > 0.01) {
      totalDb += evalPeakingDb(170, 0.82, warmthNorm * 2.4 * 0.45);
      totalDb += evalPeakingDb(350, 0.95, warmthNorm * 1.8 * 0.45);
    }
    if (airNorm > 0.01 || warmthNorm > 0.01) {
      const airDb = (airNorm * 5.2 - warmthNorm * 1.8) * 0.45;
      totalDb += evalHighShelfDb(11500, 0.68, airDb);
    }
  }

  // 5. Fletcher-Munson / 深夜等响度轮廓实时补偿预览
  if (params.equalLoudnessActive) {
    const vol = Math.max(0, Math.min(1, Number(params.volume ?? 0.6)));
    const loudnessFactor =
      vol < 0.85 ? Math.pow(Math.max(0.18, 1 - vol / 0.9), 1.1) : 0.35;
    const nightBassCompDb = loudnessFactor * 5.2;
    const nightAirCompDb = loudnessFactor * 3.4;
    const elCutoff = Math.max(40, Math.min(120, af?.bassCrossoverHz || 60));
    totalDb += evalLowShelfDb(elCutoff, 0.68, nightBassCompDb * 0.55);
    totalDb += evalHighShelfDb(10500, 0.68, nightAirCompDb * 0.55);
  }

  return Math.max(-15, Math.min(15, totalDb));
}

/**
 * 构建带通净化立体声空间混响脉冲响应 (IR)，针对 ConvolverNode.normalize = false 进行 Parseval 能量标定
 * 每个房间模型拥有独立的早期反射几何抽头、RT60 指数衰减包络、带通滤波与立体声去相关扩散核
 */
export function buildBandPurifiedReverbImpulse(
  ctx: AudioContext,
  preset: ReverbPresetId,
  dampingPercent = 35
): AudioBuffer {
  const sampleRate = ctx.sampleRate;
  const physics = getReverbRoomPhysics(preset);
  const durationSec = preset === "none" ? 0.08 : Math.max(0.32, physics.rt60Sec);
  const length = Math.max(256, Math.floor(sampleRate * durationSec));
  const impulse = ctx.createBuffer(2, length, sampleRate);
  if (preset === "none") return impulse;

  const dampNorm = Math.max(0, Math.min(1, dampingPercent / 100));
  const hpCutoff = physics.hpHz;
  const lpCutoff = Math.max(2200, physics.baseLpHz - dampNorm * 3100);
  const hpAlpha =
    1 / (1 + (2 * Math.PI * hpCutoff) / Math.max(8000, sampleRate));
  const lpDt = 1 / Math.max(8000, sampleRate);
  const lpRc = 1 / (2 * Math.PI * lpCutoff);
  const lpAlpha = lpDt / (lpRc + lpDt);

  // 使用与房间绑定且确定性的伪随机种子，消除切换或重建闪烁并强化左右声道扩散去相关
  const presetHash = preset.split("").reduce((acc, ch) => acc * 33 + ch.charCodeAt(0), 5381) >>> 0;
  let seedL = (1357911 ^ presetHash) >>> 0;
  let seedR = (24681013 ^ (presetHash * 2654435761)) >>> 0;
  const nextRandL = () => {
    seedL = (seedL * 1664525 + 1013904223) >>> 0;
    return (seedL / 4294967295) * 2 - 1;
  };
  const nextRandR = () => {
    seedR = (seedR * 22695477 + 1) >>> 0;
    return (seedR / 4294967295) * 2 - 1;
  };

  // Schroeder -60dB 物理指数衰减速率随阻尼略加快
  const decayK = 5.4 + dampNorm * 2.2;
  // 早期反射淡入窗，保证首段直达声与反射边界分离不浑浊
  const tailRiseSamples = Math.max(16, Math.floor(sampleRate * 0.008));

  for (let ch = 0; ch < 2; ch++) {
    const data = impulse.getChannelData(ch);
    let hpPrevIn = 0;
    let hpPrevOut = 0;
    let lpPrevOut = 0;
    let tailEnergySq = 0;

    for (let i = 0; i < length; i++) {
      const progress = i / length;
      const rise = i < tailRiseSamples ? i / tailRiseSamples : 1.0;
      const env = Math.exp(-decayK * progress) * (1 - progress * 0.15) * rise;
      const rawNoise = (ch === 0 ? nextRandL() : nextRandR()) * env;

      const hpOut = hpAlpha * (hpPrevOut + rawNoise - hpPrevIn);
      hpPrevIn = rawNoise;
      hpPrevOut = hpOut;

      lpPrevOut = lpPrevOut + lpAlpha * (hpOut - lpPrevOut);
      data[i] = lpPrevOut;
      tailEnergySq += lpPrevOut * lpPrevOut;
    }

    // 基于 Parseval 能量定理标定漫反射尾音基准功率 (使小房到教堂呈现真实空间密度差异)
    const tailRms = Math.sqrt(Math.max(1e-8, tailEnergySq));
    const roomGainScale = physics.tailGain / 0.42;
    const tailTargetNorm = 0.72 * roomGainScale;
    const normMul = tailTargetNorm / tailRms;
    for (let i = 0; i < length; i++) {
      data[i] *= normMul;
    }

    // 注入该房间专属离散早期反射脉冲（塑造房间大小、材质与墙面距离听感）
    const tapGainScale = physics.earlyScale * 0.24 * Math.min(1.25, 0.75 + roomGainScale * 0.35);
    for (let tIdx = 0; tIdx < physics.earlyTaps.length; tIdx++) {
      const [ms, gL, gR] = physics.earlyTaps[tIdx];
      // 右声道施加 0.7ms 空间偏置以增强天然立体宽度
      const chOffsetMs = ch === 1 ? (tIdx % 2 === 0 ? 0.65 : -0.45) : 0;
      const idx = Math.floor(((ms + chOffsetMs) / 1000) * sampleRate);
      if (idx > 0 && idx < length) {
        const tapVal = (ch === 0 ? gL : gR) * tapGainScale;
        data[idx] += tapVal;
        if (idx + 1 < length) data[idx + 1] += tapVal * 0.35 * (1 - dampNorm * 0.6);
      }
    }
  }

  return impulse;
}


/**
 * 实时原速独立移调器 (AudioWorklet 双读头汉宁窗颗粒移调算法)
 * 解决取消「音高与倍速联动」后 HTMLMediaElement.preservesPitch 无法独立升降半音的缺陷。
 * 采用 4096 采样点环形缓冲区 + 180° 相位差双读头 + 余弦汉宁窗平滑交叉淡入淡出，实现 1.0x 原速下 ±12 半音无损移调。
 */
export const PITCH_SHIFTER_WORKLET_CODE = `
class BreezePitchShifterProcessor extends AudioWorkletProcessor {
  static get parameterDescriptors() {
    return [
      {
        name: "pitchRatio",
        defaultValue: 1.0,
        minValue: 0.5,
        maxValue: 2.0,
        automationRate: "k-rate"
      }
    ];
  }

  constructor() {
    super();
    this.bufSize = 4096;
    this.bufMask = this.bufSize - 1;
    this.grainSize = 2048;
    this.ringL = new Float32Array(this.bufSize);
    this.ringR = new Float32Array(this.bufSize);
    this.writeIndex = 0;
    this.phase = 0.0;
  }

  readInterpolated(buf, pos) {
    let base = Math.floor(pos);
    const frac = pos - base;
    const idx0 = base & this.bufMask;
    const idx1 = (base + 1) & this.bufMask;
    return buf[idx0] * (1.0 - frac) + buf[idx1] * frac;
  }

  process(inputs, outputs, parameters) {
    const input = inputs[0];
    const output = outputs[0];
    if (!output || output.length === 0) return true;

    const outL = output[0];
    const outR = output.length > 1 ? output[1] : outL;
    if (!input || input.length === 0 || !input[0]) {
      outL.fill(0);
      if (outR !== outL) outR.fill(0);
      return true;
    }

    const inL = input[0];
    const inR = input.length > 1 && input[1] ? input[1] : inL;
    const ratioArr = parameters.pitchRatio;
    const pitchRatio = ratioArr && ratioArr.length > 0 ? ratioArr[0] : 1.0;

    if (Math.abs(pitchRatio - 1.0) < 0.002) {
      for (let i = 0; i < outL.length; i++) {
        const sL = inL[i] || 0;
        const sR = inR[i] || 0;
        this.ringL[this.writeIndex] = sL;
        this.ringR[this.writeIndex] = sR;
        this.writeIndex = (this.writeIndex + 1) & this.bufMask;
        outL[i] = sL;
        if (outR !== outL) outR[i] = sR;
      }
      return true;
    }

    const phaseStep = (1.0 - pitchRatio) / this.grainSize;

    for (let i = 0; i < outL.length; i++) {
      const sL = inL[i] || 0;
      const sR = inR[i] || 0;
      this.ringL[this.writeIndex] = sL;
      this.ringR[this.writeIndex] = sR;

      this.phase += phaseStep;
      if (this.phase >= 1.0) this.phase -= 1.0;
      else if (this.phase < 0.0) this.phase += 1.0;

      const phaseB = (this.phase + 0.5) % 1.0;
      const delayA = this.phase * this.grainSize + 128;
      const delayB = phaseB * this.grainSize + 128;

      const readPosA = (this.writeIndex - delayA + this.bufSize) % this.bufSize;
      const readPosB = (this.writeIndex - delayB + this.bufSize) % this.bufSize;

      const winA = 0.5 * (1.0 - Math.cos(2.0 * Math.PI * this.phase));
      const winB = 1.0 - winA;

      outL[i] =
        this.readInterpolated(this.ringL, readPosA) * winA +
        this.readInterpolated(this.ringL, readPosB) * winB;
      if (outR !== outL) {
        outR[i] =
          this.readInterpolated(this.ringR, readPosA) * winA +
          this.readInterpolated(this.ringR, readPosB) * winB;
      }

      this.writeIndex = (this.writeIndex + 1) & this.bufMask;
    }

    return true;
  }
}
registerProcessor("breeze-pitch-shifter-processor", BreezePitchShifterProcessor);
`;

export interface DelayPitchShifterNodes {
  input: GainNode;
  output: GainNode;
  setSemitones: (semitones: number) => void;
}

/**
 * 纯原生 Web Audio 节点双延迟线调制移调器 (Fallback 当 AudioWorklet 尚未就绪时实时生效)
 * 基于双正交锯齿波调制 DelayNode + 三角包络交叉混音 (BBC Jungle 算法)
 */
export function createDelayLinePitchShifterGraph(
  ctx: AudioContext
): DelayPitchShifterNodes {
  const input = ctx.createGain();
  const output = ctx.createGain();
  const dryGain = ctx.createGain();
  const wetGain = ctx.createGain();
  dryGain.gain.value = 1.0;
  wetGain.gain.value = 0.0;

  const delayA = ctx.createDelay(0.2);
  const delayB = ctx.createDelay(0.2);
  const fadeGainA = ctx.createGain();
  const fadeGainB = ctx.createGain();
  const modGainA = ctx.createGain();
  const modGainB = ctx.createGain();

  const windowSec = 0.065;
  delayA.delayTime.value = windowSec * 0.5;
  delayB.delayTime.value = windowSec * 0.5;
  modGainA.gain.value = 0;
  modGainB.gain.value = 0;

  // 构造 2 秒循环锯齿调制缓冲与余弦包络缓冲
  const sr = ctx.sampleRate || 48000;
  const bufLen = Math.floor(sr * 1.0);
  const sawBufA = ctx.createBuffer(1, bufLen, sr);
  const sawBufB = ctx.createBuffer(1, bufLen, sr);
  const winBufA = ctx.createBuffer(1, bufLen, sr);
  const winBufB = ctx.createBuffer(1, bufLen, sr);

  const dSawA = sawBufA.getChannelData(0);
  const dSawB = sawBufB.getChannelData(0);
  const dWinA = winBufA.getChannelData(0);
  const dWinB = winBufB.getChannelData(0);

  for (let i = 0; i < bufLen; i++) {
    const pA = i / bufLen;
    const pB = (pA + 0.5) % 1.0;
    dSawA[i] = pA - 0.5;
    dSawB[i] = pB - 0.5;
    const wA = 0.5 * (1 - Math.cos(2 * Math.PI * pA));
    dWinA[i] = wA;
    dWinB[i] = 1 - wA;
  }

  const makeLoopSrc = (buf: AudioBuffer) => {
    const src = ctx.createBufferSource();
    src.buffer = buf;
    src.loop = true;
    src.playbackRate.value = 1.0;
    src.start(0);
    return src;
  };

  const modSrcA = makeLoopSrc(sawBufA);
  const modSrcB = makeLoopSrc(sawBufB);
  const winSrcA = makeLoopSrc(winBufA);
  const winSrcB = makeLoopSrc(winBufB);

  fadeGainA.gain.value = 0;
  fadeGainB.gain.value = 0;

  modSrcA.connect(modGainA);
  modSrcB.connect(modGainB);
  modGainA.connect(delayA.delayTime);
  modGainB.connect(delayB.delayTime);

  winSrcA.connect(fadeGainA.gain);
  winSrcB.connect(fadeGainB.gain);

  input.connect(dryGain);
  dryGain.connect(output);

  input.connect(delayA);
  input.connect(delayB);
  delayA.connect(fadeGainA);
  delayB.connect(fadeGainB);
  fadeGainA.connect(wetGain);
  fadeGainB.connect(wetGain);
  wetGain.connect(output);

  function setSemitones(semitones: number) {
    const clamped = Math.max(-12, Math.min(12, semitones));
    if (Math.abs(clamped) < 0.02) {
      dryGain.gain.value = 1.0;
      wetGain.gain.value = 0.0;
      modGainA.gain.value = 0;
      modGainB.gain.value = 0;
      return;
    }
    const pitchRatio = Math.pow(2, clamped / 12);
    const slope = 1.0 - pitchRatio;
    const rateHz = Math.max(0.15, Math.min(18.0, Math.abs(slope) / windowSec));
    const sign = slope >= 0 ? 1 : -1;
    modSrcA.playbackRate.value = rateHz;
    modSrcB.playbackRate.value = rateHz;
    winSrcA.playbackRate.value = rateHz;
    winSrcB.playbackRate.value = rateHz;
    modGainA.gain.value = sign * windowSec * 0.48;
    modGainB.gain.value = sign * windowSec * 0.48;
    dryGain.gain.value = 0.0;
    wetGain.gain.value = 1.0;
  }

  return {
    input,
    output,
    setSemitones,
  };
}

/**
 * IEEE-754 64-Bit 双精度浮点 RBJ Biquad 级联传递函数频响计算器 H(e^{jw})
 * 为多段参量均衡器与母带防削波 Headroom 评估提供真实 64-bit 双精度复数极零点频响曲线
 */
export function compute64BitCascadedFrequencyResponseDb(
  evalFreqsHz: number[],
  bandCenterHz: number[],
  bandGainsDb: number[],
  bandQFactors: number[],
  bandTypes: PeqFilterType[],
  sampleRate = 48000,
  spectralTiltDb = 0
): number[] {
  const nyquist = sampleRate * 0.5;
  const buildShelfCoeffs = (fcHz: number, gainDb: number, isHigh: boolean) => {
    const w0 = (2.0 * Math.PI * fcHz) / sampleRate;
    const cosW0 = Math.cos(w0);
    const sinW0 = Math.sin(w0);
    const A = Math.pow(10.0, gainDb / 40.0);
    const alphaS = (sinW0 / 2.0) * Math.SQRT2;
    const twoSqrtAAlpha = 2.0 * Math.sqrt(A) * alphaS;
    if (!isHigh) {
      const b0 = A * (A + 1.0 - (A - 1.0) * cosW0 + twoSqrtAAlpha);
      const b1 = 2.0 * A * (A - 1.0 - (A + 1.0) * cosW0);
      const b2 = A * (A + 1.0 - (A - 1.0) * cosW0 - twoSqrtAAlpha);
      const a0 = A + 1.0 + (A - 1.0) * cosW0 + twoSqrtAAlpha;
      const a1 = -2.0 * (A - 1.0 + (A + 1.0) * cosW0);
      const a2 = A + 1.0 + (A - 1.0) * cosW0 - twoSqrtAAlpha;
      return [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0] as const;
    }
    const b0 = A * (A + 1.0 + (A - 1.0) * cosW0 + twoSqrtAAlpha);
    const b1 = -2.0 * A * (A - 1.0 + (A + 1.0) * cosW0);
    const b2 = A * (A + 1.0 + (A - 1.0) * cosW0 - twoSqrtAAlpha);
    const a0 = A + 1.0 - (A - 1.0) * cosW0 + twoSqrtAAlpha;
    const a1 = 2.0 * (A - 1.0 - (A + 1.0) * cosW0);
    const a2 = A + 1.0 - (A - 1.0) * cosW0 - twoSqrtAAlpha;
    return [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0] as const;
  };

  const biquadCoeffs = bandCenterHz.map((fcRaw, idx) => {
    const fc = Math.max(
      20,
      Math.min(
        nyquist * 0.96,
        Number(fcRaw) || DEFAULT_THIRTY_ONE_BAND_HZ[idx] || 1000
      )
    );
    const gainDb = Number(bandGainsDb[idx]) || 0;
    const q = Math.max(0.25, Math.min(12.0, Number(bandQFactors[idx]) || 1.2));
    const type = bandTypes[idx] || "peaking";

    const w0 = (2.0 * Math.PI * fc) / sampleRate;
    const cosW0 = Math.cos(w0);
    const sinW0 = Math.sin(w0);
    const A = Math.pow(10.0, gainDb / 40.0);
    const alpha = sinW0 / (2.0 * q);

    let b0 = 1.0, b1 = 0.0, b2 = 0.0, a0 = 1.0, a1 = 0.0, a2 = 0.0;

    if (type === "lowshelf") {
      const twoSqrtAAlpha = 2.0 * Math.sqrt(A) * alpha;
      b0 = A * (A + 1.0 - (A - 1.0) * cosW0 + twoSqrtAAlpha);
      b1 = 2.0 * A * (A - 1.0 - (A + 1.0) * cosW0);
      b2 = A * (A + 1.0 - (A - 1.0) * cosW0 - twoSqrtAAlpha);
      a0 = A + 1.0 + (A - 1.0) * cosW0 + twoSqrtAAlpha;
      a1 = -2.0 * (A - 1.0 + (A + 1.0) * cosW0);
      a2 = A + 1.0 + (A - 1.0) * cosW0 - twoSqrtAAlpha;
    } else if (type === "highshelf") {
      const twoSqrtAAlpha = 2.0 * Math.sqrt(A) * alpha;
      b0 = A * (A + 1.0 + (A - 1.0) * cosW0 + twoSqrtAAlpha);
      b1 = -2.0 * A * (A - 1.0 + (A + 1.0) * cosW0);
      b2 = A * (A + 1.0 + (A - 1.0) * cosW0 - twoSqrtAAlpha);
      a0 = A + 1.0 - (A - 1.0) * cosW0 + twoSqrtAAlpha;
      a1 = 2.0 * (A - 1.0 - (A + 1.0) * cosW0);
      a2 = A + 1.0 - (A - 1.0) * cosW0 - twoSqrtAAlpha;
    } else if (type === "notch") {
      const cutA = Math.pow(10.0, (gainDb < -0.5 ? gainDb : -10.0) / 40.0);
      const notchAlpha = sinW0 / (2.0 * Math.max(2.2, q * 2.2));
      b0 = 1.0 + notchAlpha * cutA;
      b1 = -2.0 * cosW0;
      b2 = 1.0 - notchAlpha * cutA;
      a0 = 1.0 + notchAlpha / cutA;
      a1 = -2.0 * cosW0;
      a2 = 1.0 - notchAlpha / cutA;
    } else {
      b0 = 1.0 + alpha * A;
      b1 = -2.0 * cosW0;
      b2 = 1.0 - alpha * A;
      a0 = 1.0 + alpha / A;
      a1 = -2.0 * cosW0;
      a2 = 1.0 - alpha / A;
    }

    return [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0] as const;
  });

  if (Math.abs(spectralTiltDb) > 0.04) {
    biquadCoeffs.push(buildShelfCoeffs(350, -spectralTiltDb, false));
    biquadCoeffs.push(buildShelfCoeffs(2850, spectralTiltDb, true));
  }

  return evalFreqsHz.map((fHz) => {
    const w = (2.0 * Math.PI * Math.max(10, Math.min(nyquist, fHz))) / sampleRate;
    const cos1 = Math.cos(w);
    const sin1 = Math.sin(w);
    const cos2 = Math.cos(2.0 * w);
    const sin2 = Math.sin(2.0 * w);

    let totalMagSq = 1.0;
    for (const [b0, b1, b2, a1, a2] of biquadCoeffs) {
      const numRe = b0 + b1 * cos1 + b2 * cos2;
      const numIm = -(b1 * sin1 + b2 * sin2);
      const denRe = 1.0 + a1 * cos1 + a2 * cos2;
      const denIm = -(a1 * sin1 + a2 * sin2);
      const numSq = numRe * numRe + numIm * numIm;
      const denSq = Math.max(1e-12, denRe * denRe + denIm * denIm);
      totalMagSq *= numSq / denSq;
    }

    return Number((10.0 * Math.log10(Math.max(1e-9, totalMagSq))).toFixed(2));
  });
}

/**
 * 构建带限电子管 / 黑胶 / 磁带模拟谐波非线性传递曲线 (WaveShaperNode 专用)
 * 非对称偶次 (2nd/4th) 与软削波饱和组合，经后级高通隔直后可生成醇厚温润泛音
 */
export function createTubeWarmthCurve(
  warmthNorm: number,
  mode: "tube" | "vinyl" | "tape" = "tube"
): Float32Array<ArrayBuffer> {
  const nSamples = 2049;
  const curve = new Float32Array(new ArrayBuffer(nSamples * 4));
  const drive = Math.max(0, Math.min(1, warmthNorm));
  if (drive <= 0.005) {
    for (let i = 0; i < nSamples; i++) {
      curve[i] = (i * 2) / (nSamples - 1) - 1;
    }
    return curve;
  }

  // 三种模拟声学介质差异化谐波结构：
  // - tube: 高偶次谐波 (x^2 非对称项) + 柔顺甲类饱和
  // - vinyl: 中等偶次温润泛音 + 极低硬削波
  // - tape: 强对称奇次软饱和 (tanh 磁滞压缩) + 适度 2nd 暖声
  const k =
    mode === "tape"
      ? 1.55 + drive * 2.1
      : mode === "vinyl"
      ? 1.2 + drive * 1.45
      : 1.3 + drive * 1.75;
  const asym =
    mode === "tape"
      ? 0.08 * drive
      : mode === "vinyl"
      ? 0.24 * drive
      : 0.34 * drive;
  const normDenom = Math.max(0.25, Math.tanh(k));

  for (let i = 0; i < nSamples; i++) {
    const x = (i * 2) / (nSamples - 1) - 1;
    const warped = k * x + asym * x * x * (1 - 0.18 * x * x);
    curve[i] = Math.max(-1, Math.min(1, Math.tanh(warped) / normDenom));
  }
  return curve;
}

/**
 * 构建高频空气激励器 (Aphex Aural Exciter 原理) 谐波曲线
 * 仅在 >4.6kHz 并行高频支路注入微弱晶莹上泛音，零低频污染
 */
export function createAirExciterCurve(airNorm: number): Float32Array<ArrayBuffer> {
  const nSamples = 2049;
  const curve = new Float32Array(new ArrayBuffer(nSamples * 4));
  const drive = Math.max(0, Math.min(1, airNorm));
  const k = 1.15 + drive * 1.65;
  const normDenom = Math.max(0.25, Math.tanh(k));

  for (let i = 0; i < nSamples; i++) {
    const x = (i * 2) / (nSamples - 1) - 1;
    // 微量偶次+奇次泛音增量，剔除基波直通保留纯净高频丝滑闪烁感
    const harmonic = Math.tanh(k * x + 0.16 * drive * x * x) / normDenom - x * 0.45;
    curve[i] = Math.max(-1, Math.min(1, harmonic));
  }
  return curve;
}

const CURATED_VIPER_PRESET_MAP = new Map<string, ViperEffectItem>(
  FALLBACK_VIPER_PRESETS.map((p) => [p.id, p])
);

/**
 * 将后端下发预设与前端精心调校的 17 维母带预设合并，防止旧版 Rust 内置表覆盖新版声学差异，
 * 同时对动态获取的云端实验室预设 (`viper_cloud_*`) 补齐完整且具辨识度的声学画像
 */
export function mergeAndEnrichViperPreset(item: ViperEffectItem): ViperEffectItem {
  const curated = CURATED_VIPER_PRESET_MAP.get(item.id);
  if (curated) {
    return {
      ...item,
      ...curated,
      usageBadge: item.usageBadge || curated.usageBadge,
      badge: item.badge || item.usageBadge || curated.badge || curated.usageBadge,
      modeId: item.modeId || curated.modeId,
    };
  }

  // 云端动态预设启发式声学增强（确保每个云端预设参数完整且风格有区分）
  const text = `${item.label || ""} ${item.desc || ""} ${(item.acousticTags || []).join(" ")}`.toLowerCase();
  const isBass = /低音|bass|dj|重低|摇|电音|hip|808/.test(text);
  const isVocal = /人声|vocal|清唱|民谣|古风|国风|女毒/.test(text);
  const isSpatial = /3d|环绕|全景|空间|音乐厅|剧场|现场|concert|hifi/.test(text);
  const isWarm = /黑胶|磁带|胆机|暖|助眠|复古|vinyl|tape|tube|jazz/.test(text);

  const safeGains =
    Array.isArray(item.gains) && item.gains.length === 10
      ? item.gains
      : [2.4, 2.0, 1.0, 0.2, 0.8, 2.0, 3.4, 4.2, 4.6, 4.4];

  return {
    ...item,
    name: item.name || item.label,
    badge: item.badge || item.usageBadge,
    gains: safeGains,
    subBass: item.subBass ?? (isBass ? 70 : isWarm ? 34 : 28),
    bassPunch: item.bassPunch ?? (isBass ? 64 : 30),
    bassCrossoverHz: item.bassCrossoverHz ?? (isBass ? 50 : 60),
    surround: item.surround ?? (isSpatial ? 70 : 38),
    stageDepth: item.stageDepth ?? (isSpatial ? 60 : 32),
    crossfeedLevel: item.crossfeedLevel ?? (isVocal ? 28 : 22),
    vocalClarity: item.vocalClarity ?? (isVocal ? 70 : 42),
    deEsserLevel: item.deEsserLevel ?? (isWarm ? 42 : 28),
    airHarmonics: item.airHarmonics ?? (isWarm ? 12 : 46),
    tubeWarmth: item.tubeWarmth ?? (isWarm ? 72 : 20),
    transientPunch: item.transientPunch ?? (isBass ? 58 : isWarm ? 16 : 36),
    surroundRotationSpeed: item.surroundRotationSpeed ?? 0,
    loudnessBoost: item.loudnessBoost ?? (isBass ? 3.4 : 2.5),
    reverbPreset:
      (item.reverbPreset as ReverbPresetId) ||
      (isSpatial ? "concert_hall" : isVocal || isWarm ? "small_room" : "none"),
    reverbWetPercent:
      item.reverbWetPercent ?? (isSpatial ? 28 : isVocal || isWarm ? 16 : 0),
    reverbPreDelayMs: item.reverbPreDelayMs ?? (isSpatial ? 26 : 16),
    reverbDamping: item.reverbDamping ?? (isWarm ? 52 : 34),
  };
}

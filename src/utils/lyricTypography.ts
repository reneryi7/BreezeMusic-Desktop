/**
 * BreezeMusic 统一歌词双通道配色、Windows 多字重探测与光学描边引擎
 * ============================================================================
 * 供播放页 (`NowPlayingView.vue`)、播放页歌词设置 (`NowPlayingDialogs.vue`)
 * 与桌面歌词 (`DesktopLyricWindow.vue`) 三端 100% 共享，确保字形、字重阶梯与高定色库完全一致，
 * 同时各端持久化状态彼此独立互不串扰。
 */

import type {
  DesktopLyricStrokeColorMode,
  DesktopLyricStrokeWidth,
  LyricAlignment,
  LyricFontFamily,
  LyricFontWeightLevel,
  LyricHighlightThemeId,
  LyricMotionStyle,
  LyricScenePresetId,
  LyricScrollCascadeMode,
  LyricSungColorMode,
  LyricUnsungPresetId,
} from "../types";
import { hexWithAlpha, lerpOklabHex } from "./coverPalette";

// ============================================================================
// 1. 已唱部分：8 款全新高定流光预设 + 历史 ID 无缝迁移
// ============================================================================

export interface LyricSungPresetOption {
  id: Exclude<LyricHighlightThemeId, "custom">;
  label: string;
  darkPrimary: string;
  darkSecondary: string;
  lightPrimary: string;
  lightSecondary: string;
  isDynamic?: boolean;
}

export const LYRIC_SUNG_PRESETS: LyricSungPresetOption[] = [
  {
    id: "dynamic_cover",
    label: "封面流光",
    darkPrimary: "#7C8CF8",
    darkSecondary: "#38BDF8",
    lightPrimary: "#4F46E5",
    lightSecondary: "#0284C7",
    isDynamic: true,
  },
  {
    id: "glacier_aurora",
    label: "冰川极光",
    darkPrimary: "#5EEAD4",
    darkSecondary: "#60A5FA",
    lightPrimary: "#0F766E",
    lightSecondary: "#2563EB",
  },
  {
    id: "sunset_amber",
    label: "落日琥珀",
    darkPrimary: "#FB7185",
    darkSecondary: "#FBBF24",
    lightPrimary: "#E11D48",
    lightSecondary: "#D97706",
  },
  {
    id: "moonlight_frost",
    label: "月白银霜",
    darkPrimary: "#F8FAFC",
    darkSecondary: "#CBD5E1",
    lightPrimary: "#1E2430",
    lightSecondary: "#4B5565",
  },
  {
    id: "jade_mineral",
    label: "青玉翡翠",
    darkPrimary: "#34D399",
    darkSecondary: "#22D3EE",
    lightPrimary: "#047857",
    lightSecondary: "#0E7490",
  },
  {
    id: "rose_champagne",
    label: "玫瑰香槟",
    darkPrimary: "#F9A8D4",
    darkSecondary: "#FDE68A",
    lightPrimary: "#BE185D",
    lightSecondary: "#B45309",
  },
  {
    id: "amethyst_mist",
    label: "紫晶幻境",
    darkPrimary: "#C084FC",
    darkSecondary: "#F472B6",
    lightPrimary: "#7E22CE",
    lightSecondary: "#DB2777",
  },
  {
    id: "ink_vermilion",
    label: "墨韵丹青",
    darkPrimary: "#FF785A",
    darkSecondary: "#F59E0B",
    lightPrimary: "#B91C1C",
    lightSecondary: "#B45309",
  },
];

export function normalizeLyricThemeId(raw?: string | null): LyricHighlightThemeId {
  const v = (raw || "").trim();
  if (
    v === "dynamic_cover" ||
    v === "glacier_aurora" ||
    v === "sunset_amber" ||
    v === "moonlight_frost" ||
    v === "jade_mineral" ||
    v === "rose_champagne" ||
    v === "amethyst_mist" ||
    v === "ink_vermilion" ||
    v === "custom"
  ) {
    return v;
  }
  if (v === "ice_blue" || v === "cyan") return "glacier_aurora";
  if (v === "pure_white") return "moonlight_frost";
  if (v === "vip_gold" || v === "amber_gold" || v === "gold") return "sunset_amber";
  if (v === "sakura_pink" || v === "neon_coral" || v === "pink") return "rose_champagne";
  if (v === "aurora_mint" || v === "emerald_mint" || v === "emerald") return "jade_mineral";
  if (v === "aurora_purple") return "amethyst_mist";
  if (v === "theme_follow") return "dynamic_cover";
  return "dynamic_cover";
}

export function resolveSungLyricPalette(params: {
  themeId: string;
  sungMode: LyricSungColorMode;
  customPrimary: string;
  customSecondary: string;
  isLightStage: boolean;
  dynamicPrimary: string;
  dynamicSecondary: string;
  dynamicTertiary?: string;
}): { primary: string; secondary: string; tertiary: string } {
  const id = normalizeLyricThemeId(params.themeId);
  let primary: string;
  let secondary: string;
  let tertiary: string;

  if (id === "custom") {
    primary = params.customPrimary || "#5EEAD4";
    secondary =
      params.sungMode === "solid"
        ? primary
        : params.customSecondary || params.customPrimary || "#60A5FA";
    tertiary = lerpOklabHex(primary, secondary, 0.5);
  } else if (id === "dynamic_cover") {
    primary = params.dynamicPrimary || (params.isLightStage ? "#4F46E5" : "#7C8CF8");
    secondary =
      params.sungMode === "solid"
        ? primary
        : params.dynamicSecondary || primary;
    tertiary =
      params.sungMode === "solid"
        ? primary
        : params.dynamicTertiary || lerpOklabHex(primary, secondary, 0.5);
  } else {
    const preset =
      LYRIC_SUNG_PRESETS.find((p) => p.id === id) || LYRIC_SUNG_PRESETS[1];
    primary = params.isLightStage ? preset.lightPrimary : preset.darkPrimary;
    secondary =
      params.sungMode === "solid"
        ? primary
        : params.isLightStage
        ? preset.lightSecondary
        : preset.darkSecondary;
    tertiary = lerpOklabHex(primary, secondary, 0.5);
  }

  return { primary, secondary, tertiary };
}

// ============================================================================
// 2. 未唱部分：6 款精选预设 + 自定义取色 + 透明度
// ============================================================================

export interface LyricUnsungPresetOption {
  id: Exclude<LyricUnsungPresetId, "custom">;
  label: string;
  swatchDark: string;
  swatchLight: string;
}

export const LYRIC_UNSUNG_PRESETS: LyricUnsungPresetOption[] = [
  { id: "auto", label: "智能随境", swatchDark: "#E6ECF5", swatchLight: "#2A2E37" },
  { id: "snow", label: "霜雪素白", swatchDark: "#F8FAFC", swatchLight: "#F8FAFC" },
  { id: "warm_ash", label: "香槟暖灰", swatchDark: "#EDE0D0", swatchLight: "#6B5D4F" },
  { id: "cool_mist", label: "青烟冷灰", swatchDark: "#CBD5E1", swatchLight: "#4B596B" },
  { id: "ink", label: "水墨炭黑", swatchDark: "#252932", swatchLight: "#1F242D" },
  { id: "tint_soft", label: "主色薄雾", swatchDark: "#A5B4FC", swatchLight: "#4F46E5" },
];

export function normalizeUnsungPresetId(raw?: string | null): LyricUnsungPresetId {
  const v = (raw || "").trim();
  if (
    v === "auto" ||
    v === "snow" ||
    v === "warm_ash" ||
    v === "cool_mist" ||
    v === "ink" ||
    v === "tint_soft" ||
    v === "custom"
  ) {
    return v;
  }
  if (v === "warm") return "warm_ash";
  return "auto";
}

export function resolveUnsungLyricHex(params: {
  preset: LyricUnsungPresetId;
  customHex: string;
  isLightStage: boolean;
  sungPrimaryHex: string;
  stageInkHex?: string;
}): string {
  const p = normalizeUnsungPresetId(params.preset);
  if (p === "custom") {
    return params.customHex || (params.isLightStage ? "#374151" : "#E5EAF2");
  }
  if (p === "snow") return "#F6F8FC";
  if (p === "ink") return "#20252E";
  if (p === "warm_ash") {
    return params.isLightStage ? "#66594C" : "#EFE2D2";
  }
  if (p === "cool_mist") {
    return params.isLightStage ? "#4B596B" : "#C9D6E8";
  }
  if (p === "tint_soft") {
    return params.isLightStage
      ? lerpOklabHex(params.stageInkHex || "#232730", params.sungPrimaryHex, 0.42)
      : lerpOklabHex("#F1F5FA", params.sungPrimaryHex, 0.36);
  }
  // auto：深底清霜白 / 浅底染色深墨
  return params.isLightStage ? params.stageInkHex || "#1E222B" : "#EEF3FA";
}

// ============================================================================
// 3. Windows 多字重中文字体探测 + 4 档肉眼分明光学字重阶梯（0 体积增量）
// ============================================================================

export const LYRIC_FONT_FAMILY_OPTIONS: Array<{
  id: LyricFontFamily;
  label: string;
  desc: string;
}> = [
  { id: "sans", label: "现代无衬", desc: "纯净几何" },
  { id: "serif", label: "典雅宋韵", desc: "人文衬线" },
  { id: "rounded", label: "温润圆体", desc: "柔和亲切" },
  { id: "editorial", label: "先锋黑体", desc: "画报张力" },
];

export const LYRIC_FONT_WEIGHT_OPTIONS: Array<{
  id: LyricFontWeightLevel;
  label: string;
}> = [
  { id: 500, label: "细雅" },
  { id: 650, label: "标准" },
  { id: 760, label: "醒目" },
  { id: 880, label: "厚重" },
];

const MULTI_WEIGHT_CJK_CANDIDATES = [
  "HarmonyOS Sans SC",
  "MiSans",
  "Source Han Sans SC",
  "Noto Sans SC",
  "Alibaba PuHuiTi 3.0",
  "Alibaba PuHuiTi 2.0",
  "PingFang SC",
];

let cachedMultiWeightSans: string | null | undefined;

function detectSystemMultiWeightCjkSans(): string | null {
  if (cachedMultiWeightSans !== undefined) return cachedMultiWeightSans;
  if (typeof document === "undefined") {
    cachedMultiWeightSans = null;
    return null;
  }
  try {
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (!ctx) {
      cachedMultiWeightSans = null;
      return null;
    }
    const sample = "微风音乐测试字重ABCDEFG0123";
    ctx.font = "400 32px monospace";
    const baseW = ctx.measureText(sample).width;
    ctx.font = "400 32px 'Microsoft YaHei UI', sans-serif";
    const yaheiW = ctx.measureText(sample).width;

    for (const fontName of MULTI_WEIGHT_CJK_CANDIDATES) {
      ctx.font = `400 32px '${fontName}', monospace`;
      const w400 = ctx.measureText(sample).width;
      if (Math.abs(w400 - baseW) < 0.5) continue;
      // 验证该字体确实存在 300/700 或 900 多字重文件，而非映射到同一张微软雅黑
      ctx.font = `300 32px '${fontName}', sans-serif`;
      const w300 = ctx.measureText(sample).width;
      ctx.font = `900 32px '${fontName}', sans-serif`;
      const w900 = ctx.measureText(sample).width;
      if (Math.abs(w400 - yaheiW) > 0.4 || Math.abs(w900 - w300) > 1.2) {
        cachedMultiWeightSans = fontName;
        return fontName;
      }
    }
  } catch {
    // fallback to Windows optical ladder
  }
  cachedMultiWeightSans = null;
  return null;
}

/**
 * 解析兼容 Windows 原生与本机多字重字体的准确 font-family + font-weight + 微光学加粗样式：
 * - `500`（细雅）→ 300（在无多字重字体时显式优先 `Microsoft YaHei Light` / `DengXian Light`）
 * - `650`（标准）→ 400（Regular）
 * - `760`（醒目）→ 700（Bold）
 * - `880`（厚重）→ 900（若无原生 Black 字重，附加微量光学加粗确保与 760 肉眼明显拉开差距！）
 */
export function resolveLyricTypographyStyle(params: {
  family: LyricFontFamily;
  weight: LyricFontWeightLevel;
  /** 若用户在「字体与排版」中为歌词指定了系统/自定义字体堆栈，优先使用该堆栈 */
  customFontFamily?: string | null;
  /** 若用户在「字体与排版」中为自定义歌词字体指定了精确 CSS 字重 (100..900)，优先使用该字重 */
  customFontWeight?: number | null;
  /** 若当前元素使用 `-webkit-background-clip: text` 透明填色渐变，请传 true + strokeTintHex 避免实色描边吃掉渐变 */
  isGradientText?: boolean;
  strokeTintHex?: string;
}): {
  fontFamily: string;
  fontWeight: number;
  opticalStrokeCss: string;
  isHeavyOptical: boolean;
} {
  const multiSans = detectSystemMultiWeightCjkSans();
  const w = params.weight;
  const hasCustomFamily = Boolean(params.customFontFamily && params.customFontFamily.trim());

  // 4 档字重映射（或采用字体设置中指定的精确字重）
  const cssWeight =
    typeof params.customFontWeight === "number" &&
    Number.isFinite(params.customFontWeight) &&
    params.customFontWeight >= 100
      ? Math.round(Math.min(900, Math.max(100, params.customFontWeight)))
      : w <= 500
      ? 300
      : w <= 650
      ? 400
      : w <= 760
      ? 700
      : 900;

  let fontFamily: string;
  if (hasCustomFamily) {
    fontFamily = params.customFontFamily!.trim();
  } else if (params.family === "serif") {
    fontFamily =
      "'Source Han Serif SC', 'Noto Serif SC', 'STSong', 'NSimSun', 'SimSun', Georgia, serif";
  } else if (params.family === "rounded") {
    fontFamily =
      "'YouYuan', 'STXihei', 'KaiTi', 'PingFang SC', 'Microsoft YaHei UI', sans-serif";
  } else if (params.family === "editorial") {
    fontFamily = multiSans
      ? `'${multiSans}', 'SimHei', 'Arial Black', 'Microsoft YaHei UI', sans-serif`
      : "'SimHei', 'Arial Black', 'Microsoft YaHei UI', sans-serif";
  } else {
    // sans（现代无衬）：500 档显式触发 Windows 原生 Light 细体文件！
    if (multiSans) {
      fontFamily = `'${multiSans}', 'Inter', 'Microsoft YaHei UI', sans-serif`;
    } else if (w <= 500) {
      fontFamily =
        "'Microsoft YaHei Light', 'DengXian Light', 'Segoe UI Light', 'Microsoft YaHei UI', sans-serif";
    } else {
      fontFamily =
        "'Inter', 'Microsoft YaHei UI', 'Microsoft YaHei', 'Segoe UI', sans-serif";
    }
  }

  // 厚重 (880) 在无原生 900 Black 字重的 Windows 字体上附加极精准的 0.55px 光学加粗
  // （`paint-order: stroke fill` 或同色轻描边，保证 500 < 650 < 760 < 880 四级清晰递进）
  const needsOpticalHeavy =
    !hasCustomFamily && w >= 880 && (!multiSans || params.family !== "sans");
  let opticalStrokeCss = "0px transparent";
  if (needsOpticalHeavy) {
    if (params.isGradientText) {
      const tint = params.strokeTintHex || "currentColor";
      opticalStrokeCss = `0.55px ${hexWithAlpha(tint, 0.72)}`;
    } else {
      opticalStrokeCss = "0.55px currentColor";
    }
  }

  return {
    fontFamily,
    fontWeight: cssWeight,
    opticalStrokeCss,
    isHeavyOptical: needsOpticalHeavy,
  };
}

// ============================================================================
// 4. 桌面歌词：真光学双层描边解析器（底层矢量描边 + 顶层纯净渐变，零吃字零重影）
// ============================================================================

export const DESKTOP_STROKE_WIDTH_OPTIONS: Array<{
  id: DesktopLyricStrokeWidth;
  label: string;
  px: number;
}> = [
  { id: "none", label: "无", px: 0 },
  { id: "thin", label: "纤细", px: 1.6 },
  { id: "medium", label: "标准", px: 3.0 },
  { id: "bold", label: "醒目", px: 4.6 },
];

export const DESKTOP_STROKE_COLOR_OPTIONS: Array<{
  id: DesktopLyricStrokeColorMode;
  label: string;
  swatch: string;
}> = [
  { id: "auto", label: "智能反差", swatch: "linear-gradient(135deg, #111827 50%, #F8FAFC 50%)" },
  { id: "black", label: "深邃墨黑", swatch: "#090D16" },
  { id: "white", label: "纯白亮边", swatch: "#FFFFFF" },
];

export function resolveDesktopStrokeStyle(params: {
  widthMode: DesktopLyricStrokeWidth;
  colorMode: DesktopLyricStrokeColorMode;
  customHex: string;
  shadowEnabled: boolean;
  isLightText: boolean;
  fontSizePx: number;
}): {
  strokePx: number;
  strokeColor: string;
  textShadow: string;
} {
  const opt =
    DESKTOP_STROKE_WIDTH_OPTIONS.find((o) => o.id === params.widthMode) ||
    DESKTOP_STROKE_WIDTH_OPTIONS[2];
  // 描边粗细随字号温和自适应，小字不糊、大字有分量
  const sizeScale = Math.max(0.75, Math.min(1.45, params.fontSizePx / 30));
  const strokePx = opt.px > 0 ? Number((opt.px * sizeScale).toFixed(2)) : 0;

  let strokeColor: string;
  if (params.colorMode === "black") {
    strokeColor = "rgba(6, 10, 18, 0.94)";
  } else if (params.colorMode === "white") {
    strokeColor = "rgba(255, 255, 255, 0.96)";
  } else if (params.colorMode === "custom") {
    strokeColor = hexWithAlpha(params.customHex || "#090D16", 0.94);
  } else {
    // auto：亮色字体配深墨黑描边，深色字体配纯净白描边
    strokeColor = params.isLightText
      ? "rgba(6, 10, 18, 0.92)"
      : "rgba(255, 255, 255, 0.96)";
  }

  let textShadow = "none";
  if (params.shadowEnabled) {
    textShadow = params.isLightText
      ? "0 2px 10px rgba(0, 0, 0, 0.56), 0 1px 3px rgba(0, 0, 0, 0.42)"
      : "0 2px 10px rgba(255, 255, 255, 0.68), 0 1px 3px rgba(255, 255, 255, 0.5)";
  }

  return { strokePx, strokeColor, textShadow };
}

// ============================================================================
// 5. 播放页歌词动效引擎风格、滚屏质感与 5 套高定场景预设（全端单一事实源）
// ============================================================================

export interface LyricMotionStyleOption {
  id: LyricMotionStyle;
  label: string;
  desc: string;
  badge?: string;
}

export const LYRIC_MOTION_STYLE_OPTIONS: ReadonlyArray<LyricMotionStyleOption> = [
  {
    id: "silk_flow",
    label: "丝绸羽化扫光",
    desc: "宽柔羽化遮罩逐字流动 · 克制微浮与长音柔光呼吸",
    badge: "推荐",
  },
  {
    id: "luminous_bloom",
    label: "光晕浸润浮升",
    desc: "整字广域柔光漫染点亮 · 温柔阻尼托举与退场余韵",
  },
  {
    id: "kinetic_pop",
    label: "跃动脉冲",
    desc: "快节奏羽化扫光叠加灵动弹性微跃与连音字浪",
  },
  {
    id: "apple_minimal",
    label: "静谧直觉",
    desc: "零单字跳动，纯以柔光扫过与景深聚焦呈现极致阅读",
  },
];

export const LYRIC_SCROLL_CASCADE_OPTIONS: ReadonlyArray<{
  id: LyricScrollCascadeMode;
  label: string;
  desc: string;
}> = [
  { id: "wave", label: "波浪错峰", desc: "邻行依次弹性追随" },
  { id: "smooth", label: "柔顺平推", desc: "整列平滑同步推进" },
  { id: "instant", label: "极速直达", desc: "零延迟干脆到位" },
];

export function normalizeLyricMotionStyle(raw?: string | null): LyricMotionStyle {
  if (
    raw === "silk_flow" ||
    raw === "luminous_bloom" ||
    raw === "kinetic_pop" ||
    raw === "apple_minimal"
  ) {
    return raw;
  }
  return "silk_flow";
}

export function normalizeLyricScrollCascadeMode(
  raw?: string | null
): LyricScrollCascadeMode {
  if (raw === "wave" || raw === "smooth" || raw === "instant") {
    return raw;
  }
  return "wave";
}

export interface LyricScenePresetConfig {
  motionStyle: LyricMotionStyle;
  scrollCascadeMode: LyricScrollCascadeMode;
  sungMode: LyricSungColorMode;
  fontFamily: LyricFontFamily;
  fontWeight: LyricFontWeightLevel;
  fontSize: number;
  lineSpacing: number;
  alignment: LyricAlignment;
  focusScaleEnabled: boolean;
  focusScaleRatio: number;
  blurEnabled: boolean;
  blurIntensity: number;
  glowIntensity: number;
  wordByWord: boolean;
  wordBounceEnabled: boolean;
  harmonyStyleEnabled: boolean;
}

export interface LyricScenePresetOption {
  id: Exclude<LyricScenePresetId, "custom">;
  label: string;
  tagline: string;
  accentSwatch: string;
  config: LyricScenePresetConfig;
}

export const LYRIC_SCENE_PRESETS: ReadonlyArray<LyricScenePresetOption> = [
  {
    id: "aurora_silk",
    label: "极光丝绸",
    tagline: "羽化流光 · 柔波",
    accentSwatch: "linear-gradient(135deg, #6366F1 0%, #38BDF8 100%)",
    config: {
      motionStyle: "silk_flow",
      scrollCascadeMode: "wave",
      sungMode: "gradient",
      fontFamily: "sans",
      fontWeight: 760,
      fontSize: 28,
      lineSpacing: 16,
      alignment: "left",
      focusScaleEnabled: true,
      focusScaleRatio: 1.08,
      blurEnabled: true,
      blurIntensity: 1.0,
      glowIntensity: 100,
      wordByWord: true,
      wordBounceEnabled: true,
      harmonyStyleEnabled: true,
    },
  },
  {
    id: "apple_minimal",
    label: "苹果静谧",
    tagline: "纯净聚焦 · 零跳",
    accentSwatch: "linear-gradient(135deg, #E2E8F0 0%, #94A3B8 100%)",
    config: {
      motionStyle: "apple_minimal",
      scrollCascadeMode: "wave",
      sungMode: "solid",
      fontFamily: "sans",
      fontWeight: 760,
      fontSize: 30,
      lineSpacing: 18,
      alignment: "left",
      focusScaleEnabled: true,
      focusScaleRatio: 1.06,
      blurEnabled: true,
      blurIntensity: 1.15,
      glowIntensity: 35,
      wordByWord: true,
      wordBounceEnabled: false,
      harmonyStyleEnabled: true,
    },
  },
  {
    id: "neon_stage",
    label: "霓虹舞台",
    tagline: "跃动脉冲 · 居中",
    accentSwatch: "linear-gradient(135deg, #FB7185 0%, #FBBF24 100%)",
    config: {
      motionStyle: "kinetic_pop",
      scrollCascadeMode: "wave",
      sungMode: "gradient",
      fontFamily: "editorial",
      fontWeight: 880,
      fontSize: 31,
      lineSpacing: 18,
      alignment: "center",
      focusScaleEnabled: true,
      focusScaleRatio: 1.12,
      blurEnabled: true,
      blurIntensity: 0.9,
      glowIntensity: 145,
      wordByWord: true,
      wordBounceEnabled: true,
      harmonyStyleEnabled: true,
    },
  },
  {
    id: "ink_poetry",
    label: "纸墨诗笺",
    tagline: "宋韵光晕 · 雅致",
    accentSwatch: "linear-gradient(135deg, #34D399 0%, #0D9488 100%)",
    config: {
      motionStyle: "luminous_bloom",
      scrollCascadeMode: "smooth",
      sungMode: "solid",
      fontFamily: "serif",
      fontWeight: 650,
      fontSize: 27,
      lineSpacing: 20,
      alignment: "center",
      focusScaleEnabled: true,
      focusScaleRatio: 1.05,
      blurEnabled: false,
      blurIntensity: 0.8,
      glowIntensity: 60,
      wordByWord: true,
      wordBounceEnabled: true,
      harmonyStyleEnabled: true,
    },
  },
  {
    id: "large_comfort",
    label: "大字护眼",
    tagline: "清晰锐利 · 舒展",
    accentSwatch: "linear-gradient(135deg, #38BDF8 0%, #34D399 100%)",
    config: {
      motionStyle: "silk_flow",
      scrollCascadeMode: "smooth",
      sungMode: "solid",
      fontFamily: "rounded",
      fontWeight: 760,
      fontSize: 36,
      lineSpacing: 22,
      alignment: "left",
      focusScaleEnabled: false,
      focusScaleRatio: 1.0,
      blurEnabled: false,
      blurIntensity: 0.6,
      glowIntensity: 45,
      wordByWord: true,
      wordBounceEnabled: false,
      harmonyStyleEnabled: true,
    },
  },
];

export function matchActiveLyricScenePreset(
  current: LyricScenePresetConfig
): LyricScenePresetId {
  for (const preset of LYRIC_SCENE_PRESETS) {
    const c = preset.config;
    if (
      current.motionStyle === c.motionStyle &&
      current.scrollCascadeMode === c.scrollCascadeMode &&
      current.sungMode === c.sungMode &&
      current.fontFamily === c.fontFamily &&
      current.fontWeight === c.fontWeight &&
      Math.round(current.fontSize) === c.fontSize &&
      Math.round(current.lineSpacing) === c.lineSpacing &&
      current.alignment === c.alignment &&
      current.focusScaleEnabled === c.focusScaleEnabled &&
      (!c.focusScaleEnabled ||
        Math.abs(current.focusScaleRatio - c.focusScaleRatio) < 0.015) &&
      current.blurEnabled === c.blurEnabled &&
      (!c.blurEnabled || Math.abs(current.blurIntensity - c.blurIntensity) < 0.06) &&
      Math.abs(current.glowIntensity - c.glowIntensity) <= 6 &&
      current.wordByWord === c.wordByWord &&
      current.wordBounceEnabled === c.wordBounceEnabled &&
      current.harmonyStyleEnabled === c.harmonyStyleEnabled
    ) {
      return preset.id;
    }
  }
  return "custom";
}

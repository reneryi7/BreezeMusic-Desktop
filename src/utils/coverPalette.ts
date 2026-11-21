/**
 * 封面动态取色与 OKLab 感知色阶引擎 (Optimized Kugou Concept Edition)
 * ============================================================================
 * 基于对 `kugougainianban` (NowPlayingSheet.kt & Theme.kt) 取色系统的深度剖析与重构，
 * 彻底替换原取色引擎并针对性解决以下五大核心痛点：
 *   1. 区域加权色相直方图（2D 中心主体高斯核 + 三区平滑对角边缘氛围双通路加权，
 *      四重抑制纯白高光、纯黑死影、人像低饱和皮肤灰噪点与暗黄绿浑浊泥色）；
 *   2. 桶内连续圆周均值 `(cos, sin)` + 邻域高斯圆周合成 (`atan2`) 计算精确主/次/三色相，
 *      彻底消除 24 桶 15° 硬编码桶中心量化阶梯跳变与跨桶边界分裂；
 *   3. 自适应和谐邻近色 / 分裂补色点缀回退算法（针对纯黑白、低饱和莫兰迪/胶片、单色相封面
 *      分级自适应生成谐波伴色），告别生硬固定 `+24°/-28°/+36°/+288°` 偏移导致的撞色与假彩斑；
 *   4. `normalizeForDarkMesh` 暗背景流光色域保护（感知色度阻尼保护低饱和/黑白封面 + sRGB 软膝裁剪）
 *      与 `readableOnDarkSurface` 文字/歌词对比度安全保底（OKLab 零色相漂移 + 深冷紫补偿 + WCAG 校验）；
 *   5. 基于 OKLab 感知均匀色彩空间的全令牌连续平滑插值 (`lerpPalette`)，支持 ~680ms 丝滑切歌过渡，
 *      彻底消除 sRGB 中点浑浊带及布尔阈值在 `t=0.5` 导致的突变闪烁。
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  CoverColorRoles,
  CoverExtractedPalette,
  CoverTonalPalette,
} from "../types";

export type { CoverColorRoles, CoverExtractedPalette, CoverTonalPalette };

// ============================================================================
// 1. 全局类型扩展与 100% 向后兼容接口定义
// ============================================================================

declare module "../types" {
  export interface CoverExtractedPalette {
    /** 封面聚类原始高保真主/次强调色（保留原汁原味色彩张力，用于双极光晕与胶片质感） */
    rawDominantAccent: string;
    rawSecondaryAccent: string;
    /** 落在主/次强调色纯色填充之上的高对比前景色（自动在深炭黑与纯白间按 WCAG >= 4.5:1 择优） */
    onDominantAccent: string;
    onSecondaryAccent: string;
    /** 次强调色柔和辉光与轻柔胶囊底色 rgba */
    accentSecondaryGlowRgba: string;
    accentSecondarySubtleRgba: string;
    /** 第四流体极光渐变色（与主/次/第三强调色构成四元流体色场） */
    quaternaryAccent: string;
    /** 主强调色柔和辉光 rgba */
    accentGlowRgba: string;
    /** 主强调色轻柔胶囊填充底色 rgba */
    accentSubtleRgba: string;

    /** 深邃背景基调多级渐变色（带封面真实色相浸润，杜绝死黑与浑浊灰雾） */
    bgPrimary: string;
    bgSecondary: string;
    bgTertiary: string;
    bgAbyss: string;

    /** 朦胧画境（cover_blur）通透深邃分层色彩 */
    canvasPrimary: string;
    canvasSecondary: string;
    canvasAccentGlowRgba: string;
    canvasVignetteRgba: string;

    /** 流体极光（fluid_aurora）四元无规则运动流体色块与交融高光 */
    fluidOrb1: string;
    fluidOrb2: string;
    fluidOrb3: string;
    fluidOrb4: string;
    fluidBlendGlowRgba: string;

    /** 高对比度歌词高亮色、次级流光色与辉光阴影色 */
    lyricActiveHex: string;
    lyricSecondaryHex: string;
    lyricGlowRgba: string;
    lyricInactiveRgba: string;
    lyricTranslationRgba: string;

    /** 声域浮光（rhythm_halo）高清晰度声波主次色与几何环描边 */
    soundwavePrimary: string;
    soundwaveSecondary: string;
    soundwaveTertiary: string;
    soundwaveCoreGlowRgba: string;
    soundwaveRingStrokeRgba: string;

    /** 暗夜星尘（starlight_nebula）多层星云与璀璨星粒色彩 */
    starlightPrimary: string;
    starlightSecondary: string;
    starlightTertiary: string;
    starlightWarmGold: string;
    nebulaGlow: string;
    nebulaGlowRgba: string;
    nebulaSecondaryGlowRgba: string;

    /** 卡片 / 队列 / 评论 / 音效弹窗表面分级与高光折射边框 */
    cardSurfacePrimary: string;
    cardSurfaceElevated: string;
    cardSurfaceLowest: string;
    cardSurfaceGlassRgba: string;
    cardBorderHighlightRgba: string;
    cardBorderSubtleRgba: string;
    cardHoverBgRgba: string;
    cardActiveBgRgba: string;
    cardAmbientGlowRgba: string;

    /** 圆角封面 2.5D 棱镜色散光晕四通道 */
    prismCyan: string;
    prismMagenta: string;
    prismViolet: string;
    prismGold: string;

    /** 封面色彩属性元数据 */
    dominantHue: number;
    isMonochrome: boolean;
    isWarmDominated: boolean;
    /** 关闭取色且为浅色主题（L1/L2）时的浅色舞台扩展字段 */
    isLightStage?: boolean;
    inkPrimary?: string;
    inkSecondary?: string;
    inkMuted?: string;
  }
}

export interface RgbTuple {
  r: number; // 0..255
  g: number; // 0..255
  b: number; // 0..255
}

export interface HsvTuple {
  h: number; // 0..360
  s: number; // 0..1
  v: number; // 0..1
}

export interface RgbColor {
  r: number;
  g: number;
  b: number;
}

export interface HsvColor {
  h: number;
  s: number;
  v: number;
}

export interface OklabColor {
  L: number; // 0..1
  a: number; // ~ -0.4..0.4
  b: number; // ~ -0.4..0.4
}

export interface OklchColor {
  L: number; // 0..1
  C: number; // 0..0.37
  h: number; // 0..360
}

export interface SurfaceContainerTones {
  surfaceDim: string;
  surfaceContainerLowest: string;
  surfaceContainerLow: string;
  surfaceContainer: string;
  surfaceContainerHigh: string;
  surfaceContainerHighest: string;
  surfaceBright: string;
}

export interface ExtendedCoverTonalPalette
  extends CoverTonalPalette,
    SurfaceContainerTones {
  seedChroma: number;
  oklchHue: number;
  isMonochrome: boolean;
  tone4: string;
  tone6: string;
  tone12: string;
  tone13: string;
  tone17: string;
  tone22: string;
  tone24: string;
}

export interface ExtendedCoverColorRoles
  extends CoverColorRoles,
    SurfaceContainerTones {
  palette: ExtendedCoverTonalPalette;
  nebulaPrimary: string;
  nebulaSecondary: string;
  nebulaTertiary: string;
  nebulaAbyss: string;
  bgPrimary: string;
  bgSecondary: string;
  bgTertiary: string;
  bgAbyss: string;
  dominantAccent: string;
  secondaryAccent: string;
  tertiaryAccent: string;
  quaternaryAccent: string;
  rawDominantAccent: string;
  rawSecondaryAccent: string;
  onDominantAccent: string;
  onSecondaryAccent: string;
  onSurface: string;
  onSurfaceVariant: string;
  onSurfaceMuted: string;
  lyricPrimary: string;
  lyricSecondary: string;
  lyricActiveHex: string;
  lyricGlowRgba: string;
  soundwavePrimary: string;
  soundwaveSecondary: string;
  soundwaveTertiary: string;
  soundwaveCoreGlowRgba: string;
  soundwaveRingStrokeRgba: string;
  starlightPrimary: string;
  starlightSecondary: string;
  starlightTertiary: string;
  starlightWarmGold: string;
  nebulaGlow: string;
  nebulaGlowRgba: string;
  nebulaSecondaryGlowRgba: string;
  fluidOrb1: string;
  fluidOrb2: string;
  fluidOrb3: string;
  fluidOrb4: string;
  fluidBlendGlowRgba: string;
  cardSurfacePrimary: string;
  cardSurfaceElevated: string;
  cardSurfaceLowest: string;
  cardSurfaceGlassRgba: string;
  cardBorderHighlightRgba: string;
  cardBorderSubtleRgba: string;
  cardHoverBgRgba: string;
  cardActiveBgRgba: string;
  ambientShadowColor: string;
  isMonochrome: boolean;
  isWarmDominated: boolean;
  dominantHue: number;
}

export interface CoreCoverPaletteSeeds {
  meshPrimary: string;
  meshSecondary: string;
  meshTertiary: string;
  dominantAccent: string;
  secondaryAccent: string;
  tertiaryAccent: string;
  quaternaryAccent?: string;
  rawDominantAccent?: string;
  rawSecondaryAccent?: string;
  isMonochrome?: boolean;
  isWarmDominated?: boolean;
  dominantHue?: number;
  isLightStage?: boolean;
  bgPrimaryOverride?: string;
  bgSecondaryOverride?: string;
  bgTertiaryOverride?: string;
  bgAbyssOverride?: string;
  fluidOrbOverrides?: [string, string, string, string];
  inkPrimary?: string;
  inkSecondary?: string;
  inkMuted?: string;
  cardSurfacePrimaryOverride?: string;
  cardSurfaceElevatedOverride?: string;
  cardSurfaceLowestOverride?: string;
}

// ============================================================================
// 2. 基础色彩空间数学 (sRGB / HSV / OKLab / OKLCH / CIE L* / WCAG 2.1)
// ============================================================================

const MAX_CACHE_ENTRIES = 128;
const coverPaletteCache = new Map<string, CoverExtractedPalette>();

function clamp(val: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, val));
}

function normalizeHueDeg(hue: number): number {
  const mod = hue % 360;
  return mod < 0 ? mod + 360 : mod;
}

/**
 * 计算两个色相角 (0~360°) 之间的最短圆周夹角 (0~180°)
 */
export function circularHueDistance(hueA: number, hueB: number): number {
  const diff = Math.abs(normalizeHueDeg(hueA) - normalizeHueDeg(hueB)) % 360;
  return diff > 180 ? 360 - diff : diff;
}

/**
 * 沿圆周最短路径插值两个色相角 (0~360°)
 */
export function lerpCircularHue(
  fromHue: number,
  toHue: number,
  t: number
): number {
  const a = normalizeHueDeg(fromHue);
  const b = normalizeHueDeg(toHue);
  const delta = ((b - a + 540) % 360) - 180;
  return normalizeHueDeg(a + delta * clamp(t, 0, 1));
}

export function hexToRgb(hex?: string | null): RgbTuple {
  const clean = (hex || "").trim().replace(/^#/, "");
  if (clean.length === 3) {
    const r = parseInt(clean[0] + clean[0], 16) || 0;
    const g = parseInt(clean[1] + clean[1], 16) || 0;
    const b = parseInt(clean[2] + clean[2], 16) || 0;
    return { r, g, b };
  }
  if (clean.length >= 6) {
    const offset = clean.length === 8 ? 2 : 0;
    const r = parseInt(clean.slice(offset, offset + 2), 16) || 0;
    const g = parseInt(clean.slice(offset + 2, offset + 4), 16) || 0;
    const b = parseInt(clean.slice(offset + 4, offset + 6), 16) || 0;
    return { r, g, b };
  }
  return { r: 43, g: 140, b: 255 };
}

export function rgbToHex(r: number, g: number, b: number): string {
  const rr = clamp(Math.round(r), 0, 255)
    .toString(16)
    .padStart(2, "0");
  const gg = clamp(Math.round(g), 0, 255)
    .toString(16)
    .padStart(2, "0");
  const bb = clamp(Math.round(b), 0, 255)
    .toString(16)
    .padStart(2, "0");
  return `#${rr}${gg}${bb}`.toUpperCase();
}

export function hexToRgbTriplet(hex?: string | null): string {
  const { r, g, b } = hexToRgb(hex);
  return `${r}, ${g}, ${b}`;
}

export function rgbToHsv(r: number, g: number, b: number): HsvTuple {
  const rf = clamp(r / 255, 0, 1);
  const gf = clamp(g / 255, 0, 1);
  const bf = clamp(b / 255, 0, 1);
  const max = Math.max(rf, gf, bf);
  const min = Math.min(rf, gf, bf);
  const d = max - min;
  let h = 0;
  const s = max === 0 ? 0 : d / max;
  const v = max;

  if (max !== min) {
    switch (max) {
      case rf:
        h = (gf - bf) / d + (gf < bf ? 6 : 0);
        break;
      case gf:
        h = (bf - rf) / d + 2;
        break;
      case bf:
        h = (rf - gf) / d + 4;
        break;
    }
    h *= 60;
  }
  return { h: normalizeHueDeg(h), s, v };
}

export function hsvToRgb(h: number, s: number, v: number): RgbTuple {
  const hue = normalizeHueDeg(h);
  const sat = clamp(s, 0, 1);
  const val = clamp(v, 0, 1);
  const c = val * sat;
  const x = c * (1 - Math.abs(((hue / 60) % 2) - 1));
  const m = val - c;
  let rf = 0;
  let gf = 0;
  let bf = 0;
  if (hue < 60) {
    rf = c;
    gf = x;
  } else if (hue < 120) {
    rf = x;
    gf = c;
  } else if (hue < 180) {
    gf = c;
    bf = x;
  } else if (hue < 240) {
    gf = x;
    bf = c;
  } else if (hue < 300) {
    rf = x;
    bf = c;
  } else {
    rf = c;
    bf = x;
  }
  return {
    r: Math.round((rf + m) * 255),
    g: Math.round((gf + m) * 255),
    b: Math.round((bf + m) * 255),
  };
}

export function hsvToHex(h: number, s: number, v: number): string {
  const { r, g, b } = hsvToRgb(h, s, v);
  return rgbToHex(r, g, b);
}

function srgbChannelToLinear(c255: number): number {
  const v = clamp(c255 / 255, 0, 1);
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
}

function linearChannelToSrgb255(lin: number): number {
  const v = clamp(lin, 0, 1);
  const s =
    v <= 0.0031308 ? 12.92 * v : 1.055 * Math.pow(v, 1 / 2.4) - 0.055;
  return clamp(Math.round(s * 255), 0, 255);
}

export function perceptualLuminanceRgb(rgb: RgbTuple): number {
  return (
    0.2126 * srgbChannelToLinear(rgb.r) +
    0.7152 * srgbChannelToLinear(rgb.g) +
    0.0722 * srgbChannelToLinear(rgb.b)
  );
}

export function perceptualLuminance(hex?: string | null): number {
  return perceptualLuminanceRgb(hexToRgb(hex));
}

export function wcagContrastRatio(hexA: string, hexB: string): number {
  const l1 = perceptualLuminance(hexA);
  const l2 = perceptualLuminance(hexB);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

export function toneToRelativeLuminance(tone: number): number {
  const t = clamp(tone, 0, 100);
  return t <= 8 ? t / 903.296296 : Math.pow((t + 16) / 116, 3);
}

export function relativeLuminanceToTone(y: number): number {
  const clampedY = clamp(y, 0, 1);
  return clampedY <= 0.008856452
    ? clampedY * 903.296296
    : 116 * Math.pow(clampedY, 1 / 3) - 16;
}

export function rgbToOklab(r: number, g: number, b: number): OklabColor {
  const lr = srgbChannelToLinear(r);
  const lg = srgbChannelToLinear(g);
  const lb = srgbChannelToLinear(b);

  const l = Math.cbrt(
    0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb
  );
  const m = Math.cbrt(
    0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb
  );
  const s = Math.cbrt(
    0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb
  );

  return {
    L: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}

function oklabToLinearRgbChannels(
  L: number,
  a: number,
  b: number
): { lr: number; lg: number; lb: number } {
  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;

  const l = l_ * l_ * l_;
  const m = m_ * m_ * m_;
  const s = s_ * s_ * s_;

  const lr = +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
  const lg = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
  const lb = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;

  return { lr, lg, lb };
}

function isLinearRgbInGamut(lr: number, lg: number, lb: number): boolean {
  const eps = 1e-4;
  return (
    lr >= -eps &&
    lr <= 1 + eps &&
    lg >= -eps &&
    lg <= 1 + eps &&
    lb >= -eps &&
    lb <= 1 + eps
  );
}

/**
 * 给定 OKLab 感知明度 L 与单位色相方向 (cosH, sinH)，二分求解 sRGB 安全色域边界最大色度 C_max，
 * 避免单通道硬截断引发暖橙偏荧光黄、紫蓝偏靛青等色相漂移。
 */
function findMaxInGamutChroma(
  L: number,
  cosH: number,
  sinH: number,
  upperBound = 0.36
): number {
  const cL = clamp(L, 0, 1);
  if (cL <= 0.002 || cL >= 0.998) return 0;

  let lo = 0;
  let hi = upperBound;
  for (let i = 0; i < 14; i++) {
    const mid = (lo + hi) * 0.5;
    const { lr, lg, lb } = oklabToLinearRgbChannels(
      cL,
      mid * cosH,
      mid * sinH
    );
    if (isLinearRgbInGamut(lr, lg, lb)) {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  return lo;
}

export function oklabToRgb(L: number, a: number, b: number): RgbColor {
  const cL = clamp(L, 0, 1);
  const initial = oklabToLinearRgbChannels(cL, a, b);
  if (isLinearRgbInGamut(initial.lr, initial.lg, initial.lb)) {
    return {
      r: linearChannelToSrgb255(initial.lr),
      g: linearChannelToSrgb255(initial.lg),
      b: linearChannelToSrgb255(initial.lb),
    };
  }

  const C = Math.hypot(a, b);
  if (C < 1e-6) {
    const g255 = linearChannelToSrgb255(cL * cL * cL);
    return { r: g255, g: g255, b: g255 };
  }
  const cosH = a / C;
  const sinH = b / C;
  const maxC = findMaxInGamutChroma(cL, cosH, sinH, C);
  const safeC = Math.min(C, maxC * 0.992);
  const { lr, lg, lb } = oklabToLinearRgbChannels(
    cL,
    safeC * cosH,
    safeC * sinH
  );
  return {
    r: linearChannelToSrgb255(lr),
    g: linearChannelToSrgb255(lg),
    b: linearChannelToSrgb255(lb),
  };
}

export function rgbToOklch(r: number, g: number, b: number): OklchColor {
  const lab = rgbToOklab(r, g, b);
  const C = Math.hypot(lab.a, lab.b);
  const h =
    C < 1e-5
      ? 0
      : normalizeHueDeg((Math.atan2(lab.b, lab.a) * 180) / Math.PI);
  return { L: lab.L, C, h };
}

export function oklchToRgb(L: number, C: number, h: number): RgbColor {
  const rad = (normalizeHueDeg(h) * Math.PI) / 180;
  return oklabToRgb(L, Math.max(0, C) * Math.cos(rad), Math.max(0, C) * Math.sin(rad));
}

export function hexToOklch(hex?: string | null): OklchColor {
  const { r, g, b } = hexToRgb(hex);
  return rgbToOklch(r, g, b);
}

export function oklchToHex(L: number, C: number, h: number): string {
  const { r, g, b } = oklchToRgb(L, C, h);
  return rgbToHex(r, g, b);
}

/**
 * 在 OKLab 感知均匀色彩空间内平滑插值两色，消除 sRGB 插值经由暗灰泥带的缺点
 */
export function lerpOklabHex(
  hexA?: string | null,
  hexB?: string | null,
  t = 0.5
): string {
  const f = clamp(t, 0, 1);
  const rgbA = hexToRgb(hexA);
  const rgbB = hexToRgb(hexB);
  if (f <= 0) return rgbToHex(rgbA.r, rgbA.g, rgbA.b);
  if (f >= 1) return rgbToHex(rgbB.r, rgbB.g, rgbB.b);

  const labA = rgbToOklab(rgbA.r, rgbA.g, rgbA.b);
  const labB = rgbToOklab(rgbB.r, rgbB.g, rgbB.b);
  const outRgb = oklabToRgb(
    labA.L + (labB.L - labA.L) * f,
    labA.a + (labB.a - labA.a) * f,
    labA.b + (labB.b - labA.b) * f
  );
  return rgbToHex(outRgb.r, outRgb.g, outRgb.b);
}

export function lerpHex(
  hexA?: string | null,
  hexB?: string | null,
  t = 0.5
): string {
  return lerpOklabHex(hexA, hexB, t);
}

export function compositeOver(
  fgHex: string,
  fgAlpha: number,
  bgHex: string
): string {
  const a = clamp(fgAlpha, 0, 1);
  const fg = hexToRgb(fgHex);
  const bg = hexToRgb(bgHex);
  return rgbToHex(
    fg.r * a + bg.r * (1 - a),
    fg.g * a + bg.g * (1 - a),
    fg.b * a + bg.b * (1 - a)
  );
}

export function hexWithAlpha(hex?: string | null, alpha = 1): string {
  const { r, g, b } = hexToRgb(hex);
  const a = clamp(alpha, 0, 1);
  return `rgba(${r}, ${g}, ${b}, ${Number(a.toFixed(3))})`;
}

function lerpRgbaOrHexToken(tokenA: string, tokenB: string, t: number): string {
  const f = clamp(t, 0, 1);
  const parse = (
    str: string
  ): { hex: string; alpha: number; hasAlpha: boolean } => {
    const s = (str || "").trim();
    if (s.startsWith("#")) {
      return { hex: s, alpha: 1, hasAlpha: false };
    }
    const m = s.match(
      /rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)(?:\s*,\s*([\d.]+))?\s*\)/i
    );
    if (m) {
      const hex = rgbToHex(
        Number(m[1]) || 0,
        Number(m[2]) || 0,
        Number(m[3]) || 0
      );
      const alpha = m[4] !== undefined ? clamp(Number(m[4]), 0, 1) : 1;
      return { hex, alpha, hasAlpha: true };
    }
    return { hex: "#7C6AF6", alpha: 0.28, hasAlpha: true };
  };
  const a = parse(tokenA);
  const b = parse(tokenB);
  const hex = lerpOklabHex(a.hex, b.hex, f);
  if (!a.hasAlpha && !b.hasAlpha) return hex;
  const alpha = a.alpha + (b.alpha - a.alpha) * f;
  return hexWithAlpha(hex, alpha);
}

// ============================================================================
// 3. 零色相漂移 CIE L* 色阶调色板与暗背景色域/可读性双保底
// ============================================================================

function solveLockedHueToneHex(
  cosH: number,
  sinH: number,
  desiredChroma: number,
  tone: number
): string {
  const t = clamp(tone, 0, 100);
  if (t <= 0) return "#000000";
  if (t >= 100) return "#FFFFFF";

  const targetY = toneToRelativeLuminance(t);
  let lowL = 0.0;
  let highL = 1.0;

  for (let i = 0; i < 16; i++) {
    const midL = (lowL + highL) * 0.5;
    const maxC =
      desiredChroma > 1e-5
        ? findMaxInGamutChroma(midL, cosH, sinH, desiredChroma * 1.05)
        : 0;
    const safeC = Math.min(desiredChroma, maxC * 0.98);
    const { lr, lg, lb } = oklabToLinearRgbChannels(
      midL,
      safeC * cosH,
      safeC * sinH
    );
    const y =
      0.2126 * clamp(lr, 0, 1) +
      0.7152 * clamp(lg, 0, 1) +
      0.0722 * clamp(lb, 0, 1);
    if (y < targetY) {
      lowL = midL;
    } else {
      highL = midL;
    }
  }

  const finalL = (lowL + highL) * 0.5;
  const maxC =
    desiredChroma > 1e-5
      ? findMaxInGamutChroma(finalL, cosH, sinH, desiredChroma * 1.05)
      : 0;
  const safeC = Math.min(desiredChroma, maxC * 0.98);
  const rgb = oklabToRgb(finalL, safeC * cosH, safeC * sinH);
  return rgbToHex(rgb.r, rgb.g, rgb.b);
}

export function getSurfaceContainerTone(
  seedHex: string,
  tone: number,
  isDark = true
): string {
  const rgb = hexToRgb(seedHex);
  const hsv = rgbToHsv(rgb.r, rgb.g, rgb.b);
  const lab = rgbToOklab(rgb.r, rgb.g, rgb.b);
  const seedChroma = Math.hypot(lab.a, lab.b);
  const isMono = hsv.s < 0.065 || seedChroma < 0.018;

  const cosH = seedChroma > 1e-5 ? lab.a / seedChroma : 0;
  const sinH = seedChroma > 1e-5 ? lab.b / seedChroma : 0;

  const baseContainerChroma = isMono
    ? Math.min(seedChroma * 0.35, 0.004)
    : isDark
    ? clamp(seedChroma * 0.22, 0.012, 0.028)
    : clamp(seedChroma * 0.14, 0.008, 0.018);

  const t = clamp(tone, 0, 100);
  const toneAttenuation =
    t <= 6 ? 0.68 : t <= 10 ? 0.84 : t >= 90 ? 0.55 : 1.0;

  return solveLockedHueToneHex(
    cosH,
    sinH,
    baseContainerChroma * toneAttenuation,
    t
  );
}

export function buildSurfaceContainerPalette(
  seedHex: string,
  isDark = true
): SurfaceContainerTones {
  if (isDark) {
    return {
      surfaceDim: getSurfaceContainerTone(seedHex, 5, true),
      surfaceContainerLowest: getSurfaceContainerTone(seedHex, 6, true),
      surfaceContainerLow: getSurfaceContainerTone(seedHex, 10, true),
      surfaceContainer: getSurfaceContainerTone(seedHex, 13, true),
      surfaceContainerHigh: getSurfaceContainerTone(seedHex, 17, true),
      surfaceContainerHighest: getSurfaceContainerTone(seedHex, 22, true),
      surfaceBright: getSurfaceContainerTone(seedHex, 26, true),
    };
  }
  return {
    surfaceDim: getSurfaceContainerTone(seedHex, 87, false),
    surfaceContainerLowest: getSurfaceContainerTone(seedHex, 100, false),
    surfaceContainerLow: getSurfaceContainerTone(seedHex, 96, false),
    surfaceContainer: getSurfaceContainerTone(seedHex, 94, false),
    surfaceContainerHigh: getSurfaceContainerTone(seedHex, 92, false),
    surfaceContainerHighest: getSurfaceContainerTone(seedHex, 90, false),
    surfaceBright: getSurfaceContainerTone(seedHex, 98, false),
  };
}

export function buildCoverTonalPalette(
  seedHex: string
): ExtendedCoverTonalPalette {
  const rgb = hexToRgb(seedHex);
  const hsv = rgbToHsv(rgb.r, rgb.g, rgb.b);
  const lab = rgbToOklab(rgb.r, rgb.g, rgb.b);
  const seedHue = hsv.h;
  const seedSat = hsv.s;
  const seedChroma = Math.hypot(lab.a, lab.b);
  const oklchHue =
    seedChroma < 1e-5
      ? 0
      : normalizeHueDeg((Math.atan2(lab.b, lab.a) * 180) / Math.PI);
  const isMonochrome = seedSat < 0.065 || seedChroma < 0.018;

  const cosH = seedChroma > 1e-5 ? lab.a / seedChroma : 0;
  const sinH = seedChroma > 1e-5 ? lab.b / seedChroma : 0;

  const computeToneHex = (tone: number): string => {
    const t = clamp(tone, 0, 100);
    if (t <= 0) return "#000000";
    if (t >= 100) return "#FFFFFF";

    if (isMonochrome) {
      return solveLockedHueToneHex(
        cosH,
        sinH,
        Math.min(seedChroma * 0.5, 0.006),
        t
      );
    }

    const baseC = clamp(seedChroma, 0.065, 0.21);
    const envelope =
      t >= 99
        ? 0.1
        : t >= 95
        ? 0.24
        : t >= 90
        ? 0.44
        : t >= 80
        ? 0.72
        : t <= 8
        ? 0.42
        : t <= 15
        ? 0.58
        : t <= 25
        ? 0.76
        : 1.0;

    return solveLockedHueToneHex(cosH, sinH, baseC * envelope, t);
  };

  const containers = buildSurfaceContainerPalette(seedHex, true);

  return {
    seedHue,
    seedSat,
    seedChroma,
    oklchHue,
    isMonochrome,
    tone0: "#000000",
    tone4: computeToneHex(4),
    tone6: computeToneHex(6),
    tone10: computeToneHex(10),
    tone12: computeToneHex(12),
    tone13: computeToneHex(13),
    tone17: computeToneHex(17),
    tone20: computeToneHex(20),
    tone22: computeToneHex(22),
    tone24: computeToneHex(24),
    tone30: computeToneHex(30),
    tone40: computeToneHex(40),
    tone50: computeToneHex(50),
    tone60: computeToneHex(60),
    tone70: computeToneHex(70),
    tone80: computeToneHex(80),
    tone90: computeToneHex(90),
    tone95: computeToneHex(95),
    tone99: computeToneHex(99),
    tone100: "#FFFFFF",
    ...containers,
  };
}

export function getToneFromSeed(seedHex: string, tone: number): string {
  const t = clamp(tone, 0, 100);
  if (t <= 0) return "#000000";
  if (t >= 100) return "#FFFFFF";

  const rgb = hexToRgb(seedHex);
  const hsv = rgbToHsv(rgb.r, rgb.g, rgb.b);
  const lab = rgbToOklab(rgb.r, rgb.g, rgb.b);
  const seedChroma = Math.hypot(lab.a, lab.b);
  const isMonochrome = hsv.s < 0.065 || seedChroma < 0.018;
  const cosH = seedChroma > 1e-5 ? lab.a / seedChroma : 0;
  const sinH = seedChroma > 1e-5 ? lab.b / seedChroma : 0;

  if (isMonochrome) {
    return solveLockedHueToneHex(
      cosH,
      sinH,
      Math.min(seedChroma * 0.5, 0.006),
      t
    );
  }

  const baseC = clamp(seedChroma, 0.065, 0.21);
  const envelope =
    t >= 99
      ? 0.1
      : t >= 95
      ? 0.24
      : t >= 90
      ? 0.44
      : t >= 80
      ? 0.72
      : t <= 8
      ? 0.42
      : t <= 15
      ? 0.58
      : t <= 25
      ? 0.76
      : 1.0;

  return solveLockedHueToneHex(cosH, sinH, baseC * envelope, t);
}

/**
 * 【优化支柱 4A】暗背景流光色域保护 (`normalizeForDarkMesh` / `normalizeForDarkMeshHex`)
 * ----------------------------------------------------------------------------
 * 修复原 `kugougainianban` `normalizeForDarkMesh` 的缺陷：
 *   - 取消对黑白灰封面强加 `sat >= 0.38` + 硬编码 `+24°/-28°` 偏移导致的虚假彩色晕染；
 *   - 引入低饱和莫兰迪色度连续阻尼，且在 OKLab/OKLCH 空间控制深邃明度 (`L ≈ 0.16..0.28`)
 *     与安全色域边界，确保流体网格底色通透深邃、绝不刺眼也绝不浑浊发灰。
 */
export function normalizeForDarkMesh(
  color: RgbTuple | string,
  minSat = 0.34,
  maxSat = 0.68,
  minVal = 0.16,
  maxVal = 0.28,
  hueShiftDeg = 0
): RgbTuple {
  const rgb = typeof color === "string" ? hexToRgb(color) : color;
  const hsv = rgbToHsv(rgb.r, rgb.g, rgb.b);
  const lch = rgbToOklch(rgb.r, rgb.g, rgb.b);

  // 1. 纯黑白 / 无彩色封面保护：保留纯净石墨灰阶，严禁拉高饱和度制造伪彩斑
  if (lch.C < 0.016 || hsv.s < 0.06) {
    const targetL = clamp(lch.L, 0.16, 0.24);
    const subtleC = Math.min(lch.C * 0.65, 0.009);
    const out = oklchToRgb(targetL, subtleC, lch.h);
    return { r: out.r, g: out.g, b: out.b };
  }

  // 2. 低饱和莫兰迪 / 复古胶片阻尼：饱和度越低，收敛偏移角与目标色度上限
  const lowSatDamping =
    hsv.s < 0.22 ? clamp((hsv.s - 0.06) / 0.16, 0.22, 1.0) : 1.0;
  const adaptiveHueShift = hueShiftDeg * (0.35 + 0.65 * lowSatDamping);
  const targetHue = normalizeHueDeg(lch.h + adaptiveHueShift);

  const minL = clamp(0.15 + (minVal - 0.14) * 0.55, 0.15, 0.27);
  const maxL = clamp(0.24 + (maxVal - 0.22) * 0.60, minL + 0.04, 0.35);
  const targetL = clamp(lch.L * 0.42 + (minL + maxL) * 0.29, minL, maxL);

  const minC = clamp(minSat * 0.135 * lowSatDamping, 0.012, 0.09);
  const maxC = clamp(
    maxSat * 0.175 * (0.45 + 0.55 * lowSatDamping),
    minC + 0.014,
    0.132
  );
  const desiredC = clamp(lch.C * 0.80, minC, maxC);

  const rad = (targetHue * Math.PI) / 180;
  const maxGamutC = findMaxInGamutChroma(
    targetL,
    Math.cos(rad),
    Math.sin(rad),
    0.24
  );
  const safeC = Math.min(desiredC, maxGamutC * 0.92);
  const out = oklabToRgb(targetL, safeC * Math.cos(rad), safeC * Math.sin(rad));
  return { r: out.r, g: out.g, b: out.b };
}

export function normalizeForDarkMeshHex(
  color: RgbTuple | string,
  minSat = 0.34,
  maxSat = 0.68,
  minVal = 0.16,
  maxVal = 0.28,
  hueShiftDeg = 0
): string {
  const res = normalizeForDarkMesh(
    color,
    minSat,
    maxSat,
    minVal,
    maxVal,
    hueShiftDeg
  );
  return rgbToHex(res.r, res.g, res.b);
}

/**
 * 【优化支柱 4B】暗底文字/歌词可读性安全保底 (`readableOnDarkSurface`)
 * ----------------------------------------------------------------------------
 * 修复原 `kugougainianban` `readableOnDarkSurface` 的两大缺陷：
 *   1. 原版 HSV 强改 S/V 导致深红发粉、深蓝偏紫、部分色相亮后过曝刺眼；
 *   2. 严格锁定 OKLab 色相角 (ΔHue = 0°)，并针对冷蓝/紫罗兰区 (`235°~305°`)
 *      自动叠加感知明度补偿 + WCAG 4.6:1 暗底对比度安全下限，确保任何深色底上
 *      高亮歌词与控件图标始终清晰锐利且温润高级。
 */
export function readableOnDarkSurface(
  hex?: string | null,
  minLuminance = 0.42,
  surfaceHex = "#080C14"
): string {
  const safeHex = (hex || "").trim() || "#7C6AF6";
  const rgb = hexToRgb(safeHex);
  const lab = rgbToOklab(rgb.r, rgb.g, rgb.b);
  const C = Math.hypot(lab.a, lab.b);
  const hueDeg =
    C < 1e-5
      ? 220
      : normalizeHueDeg((Math.atan2(lab.b, lab.a) * 180) / Math.PI);

  // 对深蓝/靛紫色相区进行轻微感知明度补偿，抵消人眼对深蓝色通道灵敏度较低的问题
  const isDeepCoolHue = hueDeg >= 235 && hueDeg <= 305;
  const coolBoost = isDeepCoolHue ? 0.035 : 0;

  const surfaceLum = perceptualLuminance(surfaceHex);
  const wcagFloorLum = (surfaceLum + 0.05) * 4.65 - 0.05;
  const targetLumFloor = clamp(
    Math.max(minLuminance + coolBoost, wcagFloorLum),
    0.26,
    0.84
  );

  const currentLum = perceptualLuminanceRgb(rgb);
  if (currentLum >= targetLumFloor && currentLum <= 0.82 && C <= 0.21) {
    return rgbToHex(rgb.r, rgb.g, rgb.b);
  }

  if (C < 0.015) {
    const targetTone = clamp(
      Math.ceil(relativeLuminanceToTone(clamp(targetLumFloor, 0.48, 0.82))),
      72,
      93
    );
    return getToneFromSeed(safeHex, targetTone);
  }

  const cosH = lab.a / C;
  const sinH = lab.b / C;
  const clampedC = clamp(C * 0.94, 0.072, 0.185);
  const requiredTone = clamp(
    Math.ceil(relativeLuminanceToTone(targetLumFloor)),
    64,
    92
  );
  return solveLockedHueToneHex(cosH, sinH, clampedC, requiredTone);
}

/**
 * 暗底次级柔和可读色（确保副标题、次级提示在深色表面柔和而清晰）
 */
export function readableSubtleOnDarkSurface(
  hex?: string | null,
  minLuminance = 0.34
): string {
  const safeHex = (hex || "").trim() || "#94A3B8";
  const lum = perceptualLuminance(safeHex);
  if (lum >= minLuminance && lum <= 0.74) return safeHex;
  const requiredTone = clamp(
    Math.ceil(relativeLuminanceToTone(clamp(minLuminance, 0.24, 0.65))),
    58,
    82
  );
  return getToneFromSeed(safeHex, requiredTone);
}

/**
 * 依据强调色亮度自动在纯白与深炭黑间择优高对比前景色（保证按钮/标签内图标与文字 WCAG >= 4.5:1）
 */
export function pickReadableOnAccent(accentHex: string): string {
  const safeHex = (accentHex || "").trim() || "#7C6AF6";
  const darkInk = "#080B11";
  const whiteInk = "#FFFFFF";
  const whiteRatio = wcagContrastRatio(safeHex, whiteInk);
  const darkRatio = wcagContrastRatio(safeHex, darkInk);
  if (whiteRatio >= 4.5) return whiteInk;
  return darkRatio >= whiteRatio ? darkInk : whiteInk;
}

/**
 * 封面真实强调色在暗色沉浸舞台上的最小干预可读性保护（保真不漂白）：
 * 若原始聚类主/次色已满足深舞台可读下限且未过曝，直接返回原色调；
 * 仅对过暗/过亮簇沿 OKLab 锁色相方向 (ΔHue = 0°) 微步调节亮度以达标。
 */
export function tuneAccentForDarkStage(
  rawHex?: string | null,
  options: {
    surfaceHex?: string;
    minWcagContrast?: number;
    minLuminance?: number;
    maxLuminance?: number;
    preserveChroma?: boolean;
  } = {}
): string {
  const safeHex = (rawHex || "").trim() || "#7C6AF6";
  const surfaceHex = options.surfaceHex || "#0A0E17";
  const minContrast = options.minWcagContrast ?? 3.85;
  const maxLuminance = options.maxLuminance ?? 0.85;

  const rgb = hexToRgb(safeHex);
  const lab = rgbToOklab(rgb.r, rgb.g, rgb.b);
  const C = Math.hypot(lab.a, lab.b);
  const hueDeg =
    C < 1e-5
      ? 220
      : normalizeHueDeg((Math.atan2(lab.b, lab.a) * 180) / Math.PI);
  const coolBoost = hueDeg >= 235 && hueDeg <= 305 ? 0.018 : 0;

  const surfaceLum = perceptualLuminance(surfaceHex);
  const wcagFloorLum = (surfaceLum + 0.05) * minContrast - 0.05;
  const targetLumFloor = clamp(
    Math.max(options.minLuminance ?? 0.145, wcagFloorLum + coolBoost),
    0.12,
    0.72
  );

  const currentLum = perceptualLuminanceRgb(rgb);
  if (currentLum >= targetLumFloor && currentLum <= maxLuminance) {
    return rgbToHex(rgb.r, rgb.g, rgb.b);
  }

  if (C < 0.015) {
    const clampedLum = clamp(currentLum, targetLumFloor, maxLuminance);
    const tone = clamp(Math.round(relativeLuminanceToTone(clampedLum)), 46, 93);
    return getToneFromSeed(safeHex, tone);
  }

  const cosH = lab.a / C;
  const sinH = lab.b / C;
  const desiredLum = clamp(currentLum, targetLumFloor, maxLuminance);
  const desiredTone = clamp(relativeLuminanceToTone(desiredLum), 42, 92);
  const preservedC =
    options.preserveChroma === false
      ? clamp(C * 0.94, 0.06, 0.21)
      : clamp(C, 0.055, 0.27);
  return solveLockedHueToneHex(cosH, sinH, preservedC, desiredTone);
}

// ============================================================================
// 4. 核心算法突破：桶内连续圆周均值 (cos, sin) + 自适应和谐伴色回退
// ============================================================================

/**
 * 【优化支柱 2】桶内连续圆周均值 `(cos, sin)` 计算精确主色相 (`computeContinuousCircularHueMean`)
 * ----------------------------------------------------------------------------
 * 彻底替代 `kugougainianban` 的 `(bestBin * 15f + 7.5f) % 360f` 粗粒度量化：
 * 将像素精确色相角的单位圆向量 `w * (cos θ, sin θ)` 累加至 24 桶内，
 * 提取峰值桶时聚合 `[peakBin - 1, peakBin, peakBin + 1]` 邻域高斯加权向量和并通过 `atan2`
 * 解算连续色相角，消除 15° 阶梯跳变与跨桶边界分裂误差。
 */
export function computeContinuousCircularHueMean(
  peakBin: number,
  rawHueWeights: Float64Array,
  hueCosWeightedSum: Float64Array,
  hueSinWeightedSum: Float64Array,
  neighborhoodRadius = 1
): number {
  const binCount = rawHueWeights.length;
  const kernelWeights = neighborhoodRadius === 1 ? [0.42, 1.0, 0.42] : [1.0];

  let totalCos = 0;
  let totalSin = 0;
  let totalWeight = 0;

  for (
    let offset = -neighborhoodRadius;
    offset <= neighborhoodRadius;
    offset++
  ) {
    const idx = (peakBin + offset + binCount) % binCount;
    const w = rawHueWeights[idx];
    if (w <= 1e-7) continue;
    const kw = kernelWeights[offset + neighborhoodRadius] ?? 0.35;
    totalCos += hueCosWeightedSum[idx] * kw;
    totalSin += hueSinWeightedSum[idx] * kw;
    totalWeight += w * kw;
  }

  if (totalWeight <= 1e-7 || Math.hypot(totalCos, totalSin) <= 1e-7) {
    return normalizeHueDeg(peakBin * (360 / binCount) + 180 / binCount);
  }

  return normalizeHueDeg(
    (Math.atan2(totalSin, totalCos) * 180) / Math.PI
  );
}

export interface HuePeakCandidate {
  hue: number;
  sat: number;
  val: number;
  chroma: number;
  lightness: number;
  weight: number;
}

export interface AdaptiveHarmonicAccentsResult {
  dominantAccent: string;
  secondaryAccent: string;
  tertiaryAccent: string;
  quaternaryAccent: string;
  rawDominantAccent: string;
  rawSecondaryAccent: string;
  isMonochrome: boolean;
  isWarmDominated: boolean;
  dominantHue: number;
}

function isWarmHueDeg(hueDeg: number): boolean {
  const h = normalizeHueDeg(hueDeg);
  return h < 66 || h >= 325;
}

/**
 * 基于无压制真实候选峰值的主次配色解析器（用于纯 seedHex 回退或直方图候选输入）
 * 严格取消了对暖色封面上冷辅色的压制，并取消对亮色度原色的强制漂白。
 */
export function resolveAdaptiveHarmonicAccents(params: {
  chromaticRatio: number;
  meanChroma: number;
  meanLightness: number;
  ambientHueDeg: number;
  primaryCandidate: HuePeakCandidate | null;
  secondaryCandidate: HuePeakCandidate | null;
  tertiaryCandidate: HuePeakCandidate | null;
}): AdaptiveHarmonicAccentsResult {
  const {
    chromaticRatio,
    meanChroma,
    ambientHueDeg,
    primaryCandidate,
    secondaryCandidate,
    tertiaryCandidate,
  } = params;

  const isStrictlyMonochrome =
    !primaryCandidate ||
    (chromaticRatio < 0.025 &&
      meanChroma < 0.018 &&
      primaryCandidate.sat < 0.12);

  if (isStrictlyMonochrome) {
    return {
      dominantAccent: "#E2E6EE",
      secondaryAccent: "#9AA3B5",
      tertiaryAccent: "#C4CCDA",
      quaternaryAccent: "#7E889C",
      rawDominantAccent: "#E2E6EE",
      rawSecondaryAccent: "#9AA3B5",
      isMonochrome: true,
      isWarmDominated: false,
      dominantHue: Math.round(normalizeHueDeg(ambientHueDeg || 220)),
    };
  }

  const domHue = normalizeHueDeg(primaryCandidate.hue);
  const domWarm = isWarmHueDeg(domHue);

  const rawDom = oklchToHex(
    clamp(primaryCandidate.lightness, 0.28, 0.88),
    clamp(primaryCandidate.chroma, 0.022, 0.28),
    domHue
  );
  const dominantAccent = tuneAccentForDarkStage(rawDom, {
    minWcagContrast: 3.85,
    minLuminance: 0.145,
    maxLuminance: 0.85,
  });

  // 安全同色相谐波兜底仅在无真实第二高峰时触发
  let shiftA = 28;
  let shiftB = -28;
  if (domHue >= 0 && domHue < 65) {
    shiftA = 26;
    shiftB = -22;
  } else if (domHue >= 65 && domHue < 165) {
    shiftA = 30;
    shiftB = -24;
  } else if (domHue >= 165 && domHue < 265) {
    shiftA = -26;
    shiftB = 30;
  } else {
    shiftA = 26;
    shiftB = -28;
  }

  let secHueResolved: number;
  let rawSec: string;
  const secAllowed =
    secondaryCandidate &&
    secondaryCandidate.weight >= primaryCandidate.weight * 0.075 &&
    (circularHueDistance(domHue, secondaryCandidate.hue) >= 22 ||
      Math.abs(primaryCandidate.lightness - secondaryCandidate.lightness) >= 0.22);

  if (secAllowed && secondaryCandidate) {
    secHueResolved = normalizeHueDeg(secondaryCandidate.hue);
    rawSec = oklchToHex(
      clamp(secondaryCandidate.lightness, 0.28, 0.88),
      clamp(secondaryCandidate.chroma, 0.022, 0.27),
      secHueResolved
    );
  } else {
    secHueResolved = normalizeHueDeg(domHue + shiftA);
    const altL =
      primaryCandidate.lightness > 0.68
        ? primaryCandidate.lightness - 0.12
        : primaryCandidate.lightness + 0.12;
    rawSec = oklchToHex(
      clamp(altL, 0.40, 0.84),
      clamp(primaryCandidate.chroma * 0.92, 0.04, 0.22),
      secHueResolved
    );
  }
  const secondaryAccent = tuneAccentForDarkStage(rawSec, {
    minWcagContrast: 3.85,
    minLuminance: 0.145,
    maxLuminance: 0.86,
  });

  const terAllowed =
    tertiaryCandidate &&
    tertiaryCandidate.weight >= primaryCandidate.weight * 0.055 &&
    circularHueDistance(domHue, tertiaryCandidate.hue) >= 22 &&
    circularHueDistance(secHueResolved, tertiaryCandidate.hue) >= 20;

  let tertiaryAccent: string;
  if (terAllowed && tertiaryCandidate) {
    tertiaryAccent = tuneAccentForDarkStage(
      oklchToHex(
        clamp(tertiaryCandidate.lightness, 0.30, 0.86),
        clamp(tertiaryCandidate.chroma, 0.03, 0.24),
        normalizeHueDeg(tertiaryCandidate.hue)
      ),
      { minWcagContrast: 3.6, minLuminance: 0.13, maxLuminance: 0.84 }
    );
  } else {
    const terHueResolved =
      circularHueDistance(secHueResolved, normalizeHueDeg(domHue + shiftB)) >= 22
        ? normalizeHueDeg(domHue + shiftB)
        : normalizeHueDeg(domHue + shiftA * 1.65);
    tertiaryAccent = tuneAccentForDarkStage(
      oklchToHex(
        clamp(primaryCandidate.lightness, 0.42, 0.80),
        clamp(primaryCandidate.chroma * 0.85, 0.04, 0.18),
        terHueResolved
      ),
      { minWcagContrast: 3.6, minLuminance: 0.13, maxLuminance: 0.84 }
    );
  }

  const quaternaryAccent = lerpOklabHex(dominantAccent, secondaryAccent, 0.5);

  return {
    dominantAccent,
    secondaryAccent,
    tertiaryAccent,
    quaternaryAccent,
    rawDominantAccent: rawDom,
    rawSecondaryAccent: rawSec,
    isMonochrome: false,
    isWarmDominated: domWarm,
    dominantHue: Math.round(domHue),
  };
}

// ============================================================================
// 5. 全量色彩派生引擎 (`deriveFullCoverPalette`) 与默认/单色调色板
// ============================================================================

export function deriveFullCoverPalette(
  core: CoreCoverPaletteSeeds
): CoverExtractedPalette {
  const rawDomHex = rgbToHex(
    hexToRgb(core.rawDominantAccent || core.dominantAccent).r,
    hexToRgb(core.rawDominantAccent || core.dominantAccent).g,
    hexToRgb(core.rawDominantAccent || core.dominantAccent).b
  );
  const rawSecHex = rgbToHex(
    hexToRgb(core.rawSecondaryAccent || core.secondaryAccent).r,
    hexToRgb(core.rawSecondaryAccent || core.secondaryAccent).g,
    hexToRgb(core.rawSecondaryAccent || core.secondaryAccent).b
  );

  const domRgb = hexToRgb(rawDomHex);
  const domHsv = rgbToHsv(domRgb.r, domRgb.g, domRgb.b);
  const domLab = rgbToOklab(domRgb.r, domRgb.g, domRgb.b);
  const domChroma = Math.hypot(domLab.a, domLab.b);

  const isMonochrome =
    core.isMonochrome !== undefined
      ? core.isMonochrome
      : domHsv.s < 0.065 || domChroma < 0.018;
  const isWarmDominated =
    core.isWarmDominated !== undefined
      ? core.isWarmDominated
      : !isMonochrome && isWarmHueDeg(domHsv.h);
  const dominantHue =
    core.dominantHue !== undefined ? core.dominantHue : Math.round(domHsv.h);

  const isLightStage = Boolean(core.isLightStage);

  // 1. 保真直出双主色（黑白灰水墨锁定白金银白 #E4E6EB + 冷炭银灰 #A1A6B0，绝对零伪蓝偏色）
  const rawDominantAccent = isMonochrome ? "#E4E6EB" : rawDomHex;
  const rawSecondaryAccent = isMonochrome ? "#A1A6B0" : rawSecHex;

  const dominantAccent = isLightStage
    ? core.dominantAccent
    : isMonochrome
    ? "#E4E6EB"
    : tuneAccentForDarkStage(core.dominantAccent, {
        surfaceHex: "#0A0D14",
        minWcagContrast: 3.85,
        minLuminance: 0.145,
        maxLuminance: 0.85,
        preserveChroma: true,
      });
  const secondaryAccent = isLightStage
    ? core.secondaryAccent
    : isMonochrome
    ? "#A1A6B0"
    : tuneAccentForDarkStage(core.secondaryAccent, {
        surfaceHex: "#0A0D14",
        minWcagContrast: 3.85,
        minLuminance: 0.145,
        maxLuminance: 0.86,
        preserveChroma: true,
      });
  const tertiaryAccent = isLightStage
    ? core.tertiaryAccent
    : isMonochrome
    ? "#C4C9D2"
    : tuneAccentForDarkStage(core.tertiaryAccent, {
        surfaceHex: "#0B0E17",
        minWcagContrast: 3.6,
        minLuminance: 0.13,
        maxLuminance: 0.84,
        preserveChroma: true,
      });

  const rawQuat =
    core.quaternaryAccent || lerpOklabHex(dominantAccent, secondaryAccent, 0.5);
  const quaternaryAccent = isLightStage
    ? rawQuat
    : isMonochrome
    ? "#7E889C"
    : tuneAccentForDarkStage(rawQuat, {
        surfaceHex: "#0B0E17",
        minWcagContrast: 3.6,
        minLuminance: 0.13,
        maxLuminance: 0.84,
        preserveChroma: true,
      });

  const onDominantAccent = pickReadableOnAccent(dominantAccent);
  const onSecondaryAccent = pickReadableOnAccent(secondaryAccent);

  // 2. 流光网格三底色（深色维持暗色丝绒质感；浅色主题保留云宣/燕麦暖纸层级）
  const cleanMeshPrimary = isLightStage
    ? core.meshPrimary
    : isMonochrome
    ? "#161920"
    : normalizeForDarkMeshHex(core.meshPrimary, 0.28, 0.68, 0.08, 0.145, 0);
  const cleanMeshSecondary = isLightStage
    ? core.meshSecondary
    : isMonochrome
    ? "#11141A"
    : normalizeForDarkMeshHex(core.meshSecondary, 0.26, 0.64, 0.075, 0.135, 0);
  const cleanMeshTertiary = isLightStage
    ? core.meshTertiary
    : isMonochrome
    ? "#1D212A"
    : normalizeForDarkMeshHex(core.meshTertiary, 0.24, 0.60, 0.08, 0.14, 0);

  // 3. 背景多级基调
  const bgPrimary =
    core.bgPrimaryOverride ||
    (isMonochrome
      ? "#0E1117"
      : lerpOklabHex(getToneFromSeed(dominantAccent, 6.2), cleanMeshPrimary, 0.36));
  const bgSecondary =
    core.bgSecondaryOverride ||
    (isMonochrome
      ? "#0B0D13"
      : lerpOklabHex(
          getToneFromSeed(secondaryAccent, 5.5),
          cleanMeshSecondary,
          0.34
        ));
  const bgTertiary =
    core.bgTertiaryOverride ||
    (isMonochrome
      ? "#080A0E"
      : lerpOklabHex(
          getToneFromSeed(tertiaryAccent, 4.8),
          cleanMeshTertiary,
          0.34
        ));
  const bgAbyss =
    core.bgAbyssOverride ||
    (isMonochrome
      ? "#040508"
      : getSurfaceContainerTone(dominantAccent, 3.5, true));

  // 4. (a) 朦胧画境 (cover_blur)
  const canvasPrimary = isLightStage
    ? cleanMeshPrimary
    : isMonochrome
    ? "#161A22"
    : lerpOklabHex(cleanMeshPrimary, getToneFromSeed(dominantAccent, 18), 0.48);
  const canvasSecondary = isLightStage
    ? cleanMeshSecondary
    : isMonochrome
    ? "#10131A"
    : lerpOklabHex(
        cleanMeshSecondary,
        getToneFromSeed(secondaryAccent, 16),
        0.46
      );
  const canvasAccentGlowRgba = hexWithAlpha(rawDominantAccent, isLightStage ? 0.14 : 0.22);
  const canvasVignetteRgba = hexWithAlpha(bgAbyss, isLightStage ? 0.42 : 0.78);

  // 5. (b) 流体极光 (fluid_aurora) 四元流体色块 + 交融高光
  const fluidOrb1 =
    core.fluidOrbOverrides?.[0] ||
    (isMonochrome
      ? "#363D4B"
      : lerpOklabHex(getToneFromSeed(rawDominantAccent, 38), rawDominantAccent, 0.56));
  const fluidOrb2 =
    core.fluidOrbOverrides?.[1] ||
    (isMonochrome
      ? "#272C37"
      : lerpOklabHex(getToneFromSeed(rawSecondaryAccent, 36), rawSecondaryAccent, 0.56));
  const fluidOrb3 =
    core.fluidOrbOverrides?.[2] ||
    (isMonochrome
      ? "#20252F"
      : lerpOklabHex(getToneFromSeed(tertiaryAccent, 30), cleanMeshTertiary, 0.32));
  const fluidOrb4 =
    core.fluidOrbOverrides?.[3] ||
    (isMonochrome
      ? "#424959"
      : lerpOklabHex(fluidOrb1, fluidOrb2, 0.5));
  const fluidBlendGlowRgba = hexWithAlpha(
    lerpOklabHex(rawDominantAccent, rawSecondaryAccent, 0.5),
    isLightStage ? 0.18 : 0.30
  );

  // 6. 高对比歌词高亮与次级柔和流光
  const lyricActiveHex = isLightStage
    ? dominantAccent
    : isMonochrome
    ? "#EFF2F8"
    : tuneAccentForDarkStage(dominantAccent, {
        surfaceHex: bgPrimary,
        minWcagContrast: 4.85,
        minLuminance: 0.24,
        maxLuminance: 0.88,
        preserveChroma: true,
      });
  const lyricSecondaryHex = isLightStage
    ? secondaryAccent
    : isMonochrome
    ? "#BCC4D4"
    : tuneAccentForDarkStage(secondaryAccent, {
        surfaceHex: bgPrimary,
        minWcagContrast: 4.65,
        minLuminance: 0.22,
        maxLuminance: 0.88,
        preserveChroma: true,
      });
  const lyricGlowRgba = hexWithAlpha(lyricActiveHex, isLightStage ? 0.16 : 0.28);
  const inkPrimary = core.inkPrimary || (isLightStage ? "#18181B" : "#EEF2F6");
  const inkSecondary = core.inkSecondary || (isLightStage ? "#475569" : "rgba(226, 234, 246, 0.72)");
  const inkMuted = core.inkMuted || (isLightStage ? "#64748B" : "rgba(238, 243, 250, 0.44)");
  const lyricInactiveRgba = isLightStage
    ? hexWithAlpha(inkPrimary, 0.42)
    : "rgba(238, 243, 250, 0.34)";
  const lyricTranslationRgba = isLightStage
    ? hexWithAlpha(inkSecondary, 0.82)
    : "rgba(226, 234, 246, 0.62)";

  // 7. (c) 声域浮光 (rhythm_halo)
  const soundwavePrimary = isLightStage
    ? dominantAccent
    : isMonochrome
    ? "#E2E6EE"
    : tuneAccentForDarkStage(dominantAccent, {
        surfaceHex: bgPrimary,
        minWcagContrast: 4.5,
        minLuminance: 0.22,
        maxLuminance: 0.88,
        preserveChroma: true,
      });
  const soundwaveSecondary = isLightStage
    ? secondaryAccent
    : isMonochrome
    ? "#9AA3B5"
    : tuneAccentForDarkStage(secondaryAccent, {
        surfaceHex: bgPrimary,
        minWcagContrast: 4.5,
        minLuminance: 0.22,
        maxLuminance: 0.88,
        preserveChroma: true,
      });
  const soundwaveTertiary = isLightStage
    ? tertiaryAccent
    : isMonochrome
    ? "#C4CCDA"
    : tuneAccentForDarkStage(lerpOklabHex(dominantAccent, secondaryAccent, 0.5), {
        surfaceHex: bgPrimary,
        minWcagContrast: 4.2,
        minLuminance: 0.20,
        maxLuminance: 0.86,
      });
  const soundwaveCoreGlowRgba = hexWithAlpha(soundwavePrimary, isLightStage ? 0.18 : 0.30);
  const soundwaveRingStrokeRgba = hexWithAlpha(soundwaveSecondary, isLightStage ? 0.32 : 0.46);

  // 8. (d) 暗夜星尘 (starlight_nebula)
  const starlightPrimary = isLightStage
    ? dominantAccent
    : isMonochrome
    ? "#E8ECF4"
    : tuneAccentForDarkStage(dominantAccent, {
        surfaceHex: bgAbyss,
        minWcagContrast: 5.2,
        minLuminance: 0.28,
        maxLuminance: 0.90,
        preserveChroma: true,
      });
  const starlightSecondary = isLightStage
    ? secondaryAccent
    : isMonochrome
    ? "#AEB7C8"
    : tuneAccentForDarkStage(secondaryAccent, {
        surfaceHex: bgAbyss,
        minWcagContrast: 5.0,
        minLuminance: 0.26,
        maxLuminance: 0.90,
        preserveChroma: true,
      });
  const starlightTertiary = isLightStage
    ? tertiaryAccent
    : isMonochrome
    ? "#8791A4"
    : tuneAccentForDarkStage(tertiaryAccent, {
        surfaceHex: bgAbyss,
        minWcagContrast: 4.6,
        minLuminance: 0.24,
        maxLuminance: 0.88,
      });
  const starlightWarmGold = isLightStage
    ? tertiaryAccent
    : isMonochrome
    ? "#ECEFF6"
    : lerpOklabHex("#FFF6E4", rawSecondaryAccent, 0.35);
  const nebulaGlow = isLightStage
    ? fluidOrb1
    : isMonochrome
    ? "#232833"
    : lerpOklabHex(rawDominantAccent, cleanMeshPrimary, 0.48);
  const nebulaGlowRgba = hexWithAlpha(nebulaGlow, isLightStage ? 0.18 : 0.28);
  const nebulaSecondaryGlowRgba = hexWithAlpha(
    isLightStage
      ? fluidOrb2
      : isMonochrome
      ? "#1B1F29"
      : lerpOklabHex(rawSecondaryAccent, cleanMeshSecondary, 0.48),
    isLightStage ? 0.16 : 0.24
  );

  // 9. 卡片 / 弹窗 OKLab 无框表面分级
  const cardSurfaceLowest =
    core.cardSurfaceLowestOverride || getSurfaceContainerTone(dominantAccent, isLightStage ? 94 : 5, !isLightStage);
  const cardSurfacePrimary =
    core.cardSurfacePrimaryOverride || getSurfaceContainerTone(dominantAccent, isLightStage ? 96 : 10, !isLightStage);
  const cardSurfaceElevated =
    core.cardSurfaceElevatedOverride || getSurfaceContainerTone(dominantAccent, isLightStage ? 99 : 15, !isLightStage);
  const cardSurfaceGlassRgba = hexWithAlpha(cardSurfacePrimary, isLightStage ? 0.84 : 0.86);
  const cardBorderHighlightRgba = hexWithAlpha(
    lerpOklabHex(dominantAccent, isLightStage ? "#000000" : "#FFFFFF", 0.65),
    isLightStage ? 0.06 : 0.15
  );
  const cardBorderSubtleRgba = hexWithAlpha(
    lerpOklabHex(secondaryAccent, isLightStage ? "#000000" : "#FFFFFF", 0.72),
    isLightStage ? 0.04 : 0.09
  );
  const cardHoverBgRgba = hexWithAlpha(dominantAccent, isLightStage ? 0.08 : 0.11);
  const cardActiveBgRgba = hexWithAlpha(dominantAccent, isLightStage ? 0.14 : 0.17);
  const cardAmbientGlowRgba = hexWithAlpha(dominantAccent, isLightStage ? 0.12 : 0.16);

  // 10. 封面 2.5D 棱镜珠光四通道
  const prismCyan = isLightStage
    ? secondaryAccent
    : isMonochrome
    ? "#E2E6EE"
    : lerpOklabHex(secondaryAccent, "#FFFFFF", 0.30);
  const prismMagenta = isLightStage
    ? tertiaryAccent
    : isMonochrome
    ? "#C4CCDA"
    : lerpOklabHex(dominantAccent, "#FFFFFF", 0.28);
  const prismViolet = isLightStage
    ? dominantAccent
    : isMonochrome
    ? "#9AA3B5"
    : lerpOklabHex(tertiaryAccent, "#FFFFFF", 0.32);
  const prismGold = isLightStage
    ? quaternaryAccent
    : isMonochrome
    ? "#ECEFF6"
    : lerpOklabHex(
        lerpOklabHex(dominantAccent, secondaryAccent, 0.5),
        "#FFFFFF",
        0.34
      );

  return {
    meshPrimary: cleanMeshPrimary,
    meshSecondary: cleanMeshSecondary,
    meshTertiary: cleanMeshTertiary,
    dominantAccent,
    secondaryAccent,
    tertiaryAccent,
    quaternaryAccent,
    rawDominantAccent,
    rawSecondaryAccent,
    onDominantAccent,
    onSecondaryAccent,
    accentGlowRgba: hexWithAlpha(dominantAccent, isLightStage ? 0.20 : 0.30),
    accentSubtleRgba: hexWithAlpha(dominantAccent, isLightStage ? 0.11 : 0.15),
    accentSecondaryGlowRgba: hexWithAlpha(secondaryAccent, isLightStage ? 0.18 : 0.28),
    accentSecondarySubtleRgba: hexWithAlpha(secondaryAccent, isLightStage ? 0.10 : 0.14),
    bgPrimary,
    bgSecondary,
    bgTertiary,
    bgAbyss,
    canvasPrimary,
    canvasSecondary,
    canvasAccentGlowRgba,
    canvasVignetteRgba,
    fluidOrb1,
    fluidOrb2,
    fluidOrb3,
    fluidOrb4,
    fluidBlendGlowRgba,
    lyricActiveHex,
    lyricSecondaryHex,
    lyricGlowRgba,
    lyricInactiveRgba,
    lyricTranslationRgba,
    soundwavePrimary,
    soundwaveSecondary,
    soundwaveTertiary,
    soundwaveCoreGlowRgba,
    soundwaveRingStrokeRgba,
    starlightPrimary,
    starlightSecondary,
    starlightTertiary,
    starlightWarmGold,
    nebulaGlow,
    nebulaGlowRgba,
    nebulaSecondaryGlowRgba,
    cardSurfacePrimary,
    cardSurfaceElevated,
    cardSurfaceLowest,
    cardSurfaceGlassRgba,
    cardBorderHighlightRgba,
    cardBorderSubtleRgba,
    cardHoverBgRgba,
    cardActiveBgRgba,
    cardAmbientGlowRgba,
    prismCyan,
    prismMagenta,
    prismViolet,
    prismGold,
    dominantHue,
    isMonochrome,
    isWarmDominated,
    isLightStage,
    inkPrimary,
    inkSecondary,
    inkMuted,
  };
}

export const DEFAULT_COVER_EXTRACTED_PALETTE: CoverExtractedPalette =
  deriveFullCoverPalette({
    meshPrimary: "#121218",
    meshSecondary: "#1A1624",
    meshTertiary: "#16121C",
    dominantAccent: "#7C6AF6",
    secondaryAccent: "#2DD4BF",
    tertiaryAccent: "#C084FC",
    quaternaryAccent: "#FF6B8B",
    rawDominantAccent: "#7C6AF6",
    rawSecondaryAccent: "#2DD4BF",
    isMonochrome: false,
    isWarmDominated: false,
  });

export const MONOCHROME_COVER_EXTRACTED_PALETTE: CoverExtractedPalette =
  deriveFullCoverPalette({
    meshPrimary: "#161920",
    meshSecondary: "#111419",
    meshTertiary: "#1F232C",
    dominantAccent: "#E4E6EB",
    secondaryAccent: "#A1A6B0",
    tertiaryAccent: "#C4C9D2",
    quaternaryAccent: "#7E8490",
    rawDominantAccent: "#E4E6EB",
    rawSecondaryAccent: "#A1A6B0",
    isMonochrome: true,
    isWarmDominated: false,
  });

/** 关闭智能取色时，四套全局主题各自的原生播放页沉浸调色板（D1 / D2 / L1 / L2） */
export const THEME_FALLBACK_COVER_PALETTES = {
  D1_obsidian_aurora: deriveFullCoverPalette({
    meshPrimary: "#0C0E14",
    meshSecondary: "#121826",
    meshTertiary: "#101724",
    dominantAccent: "#5E6AD2",
    secondaryAccent: "#38BDF8",
    tertiaryAccent: "#2DD4BF",
    quaternaryAccent: "#818CF8",
    rawDominantAccent: "#5E6AD2",
    rawSecondaryAccent: "#38BDF8",
    bgPrimaryOverride: "#0C0E14",
    bgSecondaryOverride: "#101521",
    bgTertiaryOverride: "#141A29",
    bgAbyssOverride: "#07090E",
    fluidOrbOverrides: ["#4F46E5", "#38BDF8", "#2DD4BF", "#818CF8"],
    cardSurfaceLowestOverride: "#0D1018",
    cardSurfacePrimaryOverride: "#141925",
    cardSurfaceElevatedOverride: "#1B2233",
    inkPrimary: "#EEF2F6",
    inkSecondary: "#CBD5E1",
    inkMuted: "#94A3B8",
    isMonochrome: false,
    isWarmDominated: false,
    isLightStage: false,
  }),
  D2_velvet_nebula: deriveFullCoverPalette({
    meshPrimary: "#1A1116",
    meshSecondary: "#24161F",
    meshTertiary: "#22141B",
    dominantAccent: "#F43F5E",
    secondaryAccent: "#F59E0B",
    tertiaryAccent: "#E879F9",
    quaternaryAccent: "#FBBF24",
    rawDominantAccent: "#F43F5E",
    rawSecondaryAccent: "#F59E0B",
    bgPrimaryOverride: "#1A1116",
    bgSecondaryOverride: "#21141B",
    bgTertiaryOverride: "#291821",
    bgAbyssOverride: "#110A0E",
    fluidOrbOverrides: ["#F43F5E", "#F59E0B", "#E879F9", "#FB7185"],
    cardSurfaceLowestOverride: "#181014",
    cardSurfacePrimaryOverride: "#25171F",
    cardSurfaceElevatedOverride: "#32202B",
    inkPrimary: "#F9EFE6",
    inkSecondary: "#E8D7CE",
    inkMuted: "#B99FA8",
    isMonochrome: false,
    isWarmDominated: true,
    isLightStage: false,
  }),
  L1_watercolor: deriveFullCoverPalette({
    meshPrimary: "#F6F8FC",
    meshSecondary: "#FDF2F8",
    meshTertiary: "#EEF4FC",
    dominantAccent: "#4F46E5",
    secondaryAccent: "#0284C7",
    tertiaryAccent: "#EC4899",
    quaternaryAccent: "#0D9488",
    rawDominantAccent: "#4F46E5",
    rawSecondaryAccent: "#0284C7",
    bgPrimaryOverride: "#F6F8FC",
    bgSecondaryOverride: "#F2F6FD",
    bgTertiaryOverride: "#EEF3FA",
    bgAbyssOverride: "#E4EBF7",
    fluidOrbOverrides: ["#818CF8", "#F472B6", "#38BDF8", "#2DD4BF"],
    cardSurfaceLowestOverride: "#EFF3FA",
    cardSurfacePrimaryOverride: "#FFFFFF",
    cardSurfaceElevatedOverride: "#FFFFFF",
    inkPrimary: "#18181B",
    inkSecondary: "#475569",
    inkMuted: "#64748B",
    isMonochrome: false,
    isWarmDominated: false,
    isLightStage: true,
  }),
  L2_nordic_oat: deriveFullCoverPalette({
    meshPrimary: "#E8E0D0",
    meshSecondary: "#F2ECE1",
    meshTertiary: "#FAF6EE",
    dominantAccent: "#0F766E",
    secondaryAccent: "#B45309",
    tertiaryAccent: "#A16207",
    quaternaryAccent: "#047857",
    rawDominantAccent: "#0F766E",
    rawSecondaryAccent: "#B45309",
    bgPrimaryOverride: "#EDE6D8",
    bgSecondaryOverride: "#E8E0D0",
    bgTertiaryOverride: "#F2ECE1",
    bgAbyssOverride: "#DFD5C3",
    fluidOrbOverrides: ["#14B8A6", "#D97706", "#CA8A04", "#0D9488"],
    cardSurfaceLowestOverride: "#E8E0D0",
    cardSurfacePrimaryOverride: "#F2ECE1",
    cardSurfaceElevatedOverride: "#FAF6EE",
    inkPrimary: "#231D16",
    inkSecondary: "#574B3E",
    inkMuted: "#786B5D",
    isMonochrome: false,
    isWarmDominated: true,
    isLightStage: true,
  }),
} as const;

export function getThemeFallbackCoverPalette(
  colorScheme: "abyssal_dark" | "watercolor_light" | "system",
  darkVariant: "obsidian_aurora" | "velvet_nebula" = "obsidian_aurora",
  lightVariant: "watercolor" | "nordic_oat" = "watercolor"
): CoverExtractedPalette {
  const isLight =
    colorScheme === "watercolor_light" ||
    (colorScheme === "system" &&
      typeof window !== "undefined" &&
      window.matchMedia?.("(prefers-color-scheme: light)").matches);
  if (isLight) {
    return {
      ...(lightVariant === "nordic_oat"
        ? THEME_FALLBACK_COVER_PALETTES.L2_nordic_oat
        : THEME_FALLBACK_COVER_PALETTES.L1_watercolor),
    };
  }
  return {
    ...(darkVariant === "velvet_nebula"
      ? THEME_FALLBACK_COVER_PALETTES.D2_velvet_nebula
      : THEME_FALLBACK_COVER_PALETTES.D1_obsidian_aurora),
  };
}

/**
 * 在 OKLab 空间把封面强调色调优为浅色宣纸舞台可读前景色（WCAG >= minWcag，保留原色相与彩度）
 */
function tuneAccentForLightPaper(
  hex: string,
  surfaceHex: string,
  minWcag = 4.65
): string {
  const lch = hexToOklch(hex);
  const c = clamp(lch.C * 1.04, 0.068, 0.24);
  let L = clamp(lch.L, 0.34, 0.52);
  let candidate = oklchToHex(L, c, lch.h);
  for (let i = 0; i < 20 && wcagContrastRatio(candidate, surfaceHex) < minWcag; i++) {
    L = Math.max(0.22, L - 0.015);
    candidate = oklchToHex(L, clamp(c * (0.94 + L * 0.14), 0.055, 0.23), lch.h);
  }
  return candidate;
}

/**
 * 为浅色封面舞台生成浓郁水彩/彩色玻璃背景颜料（保留鲜活彩度，避免浅底混白发淡发灰）
 */
function tunePigmentForLightBg(hex: string): string {
  const lch = hexToOklch(hex);
  const c = clamp(lch.C * 1.12, 0.085, 0.27);
  const L = clamp(lch.L * 0.88, 0.44, 0.68);
  return oklchToHex(L, c, lch.h);
}

/**
 * 封面动态取色 · 浅色漫染舞台生成器（OKLab Light Tinted Cover Stage）：
 * - 当取色处于「随明暗」模式且全局为浅色主题时调用；
 * - 基底生成随封面主副色相联动的极高明度低彩度染宣纸底色（L = 0.93~0.96），明显区隔于固定 L1/L2 主题；
 * - 背景颜料保持浓郁水彩/彩色玻璃显色，文字与高亮主辅色自动压暗满足 WCAG >= 4.5:1；
 * - 纯黑白/水墨封面走专属「素宣墨韵」中性色阶，零假偏色。
 */
export function buildLightTintedCoverPalette(
  darkPalInput?: Partial<CoverExtractedPalette> | null
): CoverExtractedPalette {
  const darkPal = ensureFullCoverPalette(darkPalInput);

  if (darkPal.isMonochrome) {
    const bg1 = "#F3F4F6";
    const bg2 = "#EBEDF1";
    const bg3 = "#E4E7EC";
    const abyss = "#D8DCE4";
    const dom = "#272B34";
    const sec = "#4E5563";
    const ter = "#646C7C";
    const quat = "#3B4250";
    return deriveFullCoverPalette({
      meshPrimary: bg1,
      meshSecondary: bg2,
      meshTertiary: bg3,
      dominantAccent: dom,
      secondaryAccent: sec,
      tertiaryAccent: ter,
      quaternaryAccent: quat,
      rawDominantAccent: dom,
      rawSecondaryAccent: sec,
      bgPrimaryOverride: bg1,
      bgSecondaryOverride: bg2,
      bgTertiaryOverride: bg3,
      bgAbyssOverride: abyss,
      fluidOrbOverrides: ["#4A5160", "#687080", "#848C9C", "#3A414E"],
      cardSurfaceLowestOverride: "#EAECEF",
      cardSurfacePrimaryOverride: "#F7F8FA",
      cardSurfaceElevatedOverride: "#FFFFFF",
      inkPrimary: "#1C1F26",
      inkSecondary: "#4B5260",
      inkMuted: "#6D7584",
      isMonochrome: true,
      isWarmDominated: false,
      isLightStage: true,
      dominantHue: darkPal.dominantHue,
    });
  }

  const rawDomHex = darkPal.rawDominantAccent || darkPal.dominantAccent;
  const rawSecHex = darkPal.rawSecondaryAccent || darkPal.secondaryAccent;
  const rawTerHex = darkPal.tertiaryAccent || darkPal.secondaryAccent;
  const rawQuatHex = darkPal.quaternaryAccent || darkPal.tertiaryAccent;

  const domLch = hexToOklch(rawDomHex);
  const secLch = hexToOklch(rawSecHex);
  const terLch = hexToOklch(rawTerHex);

  // 动态封面染宣纸底色（L = 0.93~0.96，温和色度 14%~22%，随专辑色温柔美流转）
  const bg1 = oklchToHex(0.962, clamp(domLch.C * 0.14, 0.013, 0.028), domLch.h);
  const bg2 = oklchToHex(0.946, clamp(secLch.C * 0.15, 0.014, 0.030), secLch.h);
  const bg3 = oklchToHex(0.934, clamp(terLch.C * 0.14, 0.013, 0.028), terLch.h);
  const abyss = oklchToHex(0.896, clamp(domLch.C * 0.17, 0.015, 0.032), domLch.h);

  // 浅底可读前景色（全部严格满足对 bg1 对比度 >= 4.65:1）
  const dom = tuneAccentForLightPaper(rawDomHex, bg1, 4.75);
  const sec = tuneAccentForLightPaper(rawSecHex, bg1, 4.65);
  const ter = tuneAccentForLightPaper(rawTerHex, bg1, 4.55);
  const quat = tuneAccentForLightPaper(rawQuatHex, bg1, 4.55);

  // 浅色背景水彩/长虹玻璃颜料（保持鲜明宝石感，告别发白寡淡）
  const orb1 = tunePigmentForLightBg(rawDomHex);
  const orb2 = tunePigmentForLightBg(rawSecHex);
  const orb3 = tunePigmentForLightBg(rawTerHex);
  const orb4 = tunePigmentForLightBg(rawQuatHex);

  // 轻度染色深墨字阶（高对比且与封面色温和谐统一）
  const ink1 = oklchToHex(0.20, clamp(domLch.C * 0.08, 0.005, 0.016), domLch.h);
  const ink2 = oklchToHex(0.38, clamp(domLch.C * 0.09, 0.006, 0.018), domLch.h);
  const ink3 = oklchToHex(0.52, clamp(domLch.C * 0.08, 0.005, 0.016), domLch.h);

  const cardLow = oklchToHex(0.942, clamp(domLch.C * 0.11, 0.010, 0.022), domLch.h);
  const cardMid = oklchToHex(0.974, clamp(domLch.C * 0.08, 0.007, 0.016), domLch.h);
  const cardHigh = oklchToHex(0.990, clamp(domLch.C * 0.05, 0.004, 0.011), domLch.h);

  return deriveFullCoverPalette({
    meshPrimary: bg1,
    meshSecondary: bg2,
    meshTertiary: bg3,
    dominantAccent: dom,
    secondaryAccent: sec,
    tertiaryAccent: ter,
    quaternaryAccent: quat,
    rawDominantAccent: orb1,
    rawSecondaryAccent: orb2,
    bgPrimaryOverride: bg1,
    bgSecondaryOverride: bg2,
    bgTertiaryOverride: bg3,
    bgAbyssOverride: abyss,
    fluidOrbOverrides: [orb1, orb2, orb3, orb4],
    cardSurfaceLowestOverride: cardLow,
    cardSurfacePrimaryOverride: cardMid,
    cardSurfaceElevatedOverride: cardHigh,
    inkPrimary: ink1,
    inkSecondary: ink2,
    inkMuted: ink3,
    isMonochrome: false,
    isWarmDominated: darkPal.isWarmDominated,
    isLightStage: true,
    dominantHue: darkPal.dominantHue,
  });
}

let latestActivePaletteSnapshot: CoverExtractedPalette =
  DEFAULT_COVER_EXTRACTED_PALETTE;

export function ensureFullCoverPalette(
  partial?: Partial<CoverExtractedPalette> | null
): CoverExtractedPalette {
  if (!partial) {
    return { ...DEFAULT_COVER_EXTRACTED_PALETTE };
  }
  if (
    partial.quaternaryAccent &&
    partial.rawDominantAccent &&
    partial.onDominantAccent &&
    partial.bgPrimary &&
    partial.soundwavePrimary &&
    partial.starlightPrimary &&
    partial.lyricActiveHex &&
    partial.cardSurfacePrimary
  ) {
    const full = {
      ...DEFAULT_COVER_EXTRACTED_PALETTE,
      ...partial,
    } as CoverExtractedPalette;
    latestActivePaletteSnapshot = full;
    return full;
  }
  const derived = deriveFullCoverPalette({
    meshPrimary:
      partial.meshPrimary || DEFAULT_COVER_EXTRACTED_PALETTE.meshPrimary,
    meshSecondary:
      partial.meshSecondary || DEFAULT_COVER_EXTRACTED_PALETTE.meshSecondary,
    meshTertiary:
      partial.meshTertiary || DEFAULT_COVER_EXTRACTED_PALETTE.meshTertiary,
    dominantAccent:
      partial.dominantAccent || DEFAULT_COVER_EXTRACTED_PALETTE.dominantAccent,
    secondaryAccent:
      partial.secondaryAccent ||
      DEFAULT_COVER_EXTRACTED_PALETTE.secondaryAccent,
    tertiaryAccent:
      partial.tertiaryAccent || DEFAULT_COVER_EXTRACTED_PALETTE.tertiaryAccent,
    quaternaryAccent: partial.quaternaryAccent,
    rawDominantAccent: partial.rawDominantAccent || partial.dominantAccent,
    rawSecondaryAccent: partial.rawSecondaryAccent || partial.secondaryAccent,
    isMonochrome: partial.isMonochrome,
    isWarmDominated: partial.isWarmDominated,
    dominantHue: partial.dominantHue,
  });
  latestActivePaletteSnapshot = derived;
  return derived;
}

export function toCoverColorRoles(
  seedHex: string,
  isDark = true,
  extractedPalette?: Partial<CoverExtractedPalette> | null
): ExtendedCoverColorRoles {
  const safeSeed =
    (seedHex || "").trim() || DEFAULT_COVER_EXTRACTED_PALETTE.dominantAccent;
  const normSeedHex = rgbToHex(
    hexToRgb(safeSeed).r,
    hexToRgb(safeSeed).g,
    hexToRgb(safeSeed).b
  );
  const effectiveExtracted =
    extractedPalette ||
    (latestActivePaletteSnapshot &&
    latestActivePaletteSnapshot.dominantAccent.toUpperCase() === normSeedHex &&
    normSeedHex !== DEFAULT_COVER_EXTRACTED_PALETTE.dominantAccent
      ? latestActivePaletteSnapshot
      : null);

  const palette = buildCoverTonalPalette(safeSeed);
  const fullExtracted = effectiveExtracted
    ? ensureFullCoverPalette({
        ...effectiveExtracted,
        dominantAccent: effectiveExtracted.dominantAccent || safeSeed,
        isMonochrome: effectiveExtracted.isMonochrome ?? palette.isMonochrome,
      })
    : (() => {
        const seedLch = hexToOklch(safeSeed);
        const harmonic = resolveAdaptiveHarmonicAccents({
          chromaticRatio: seedLch.C < 0.018 ? 0.01 : 0.45,
          meanChroma: seedLch.C,
          meanLightness: seedLch.L,
          ambientHueDeg: seedLch.h,
          primaryCandidate: {
            hue: seedLch.h,
            sat: clamp(seedLch.C * 4.2, 0, 1),
            val: clamp(seedLch.L, 0.2, 0.95),
            chroma: seedLch.C,
            lightness: seedLch.L,
            weight: 1,
          },
          secondaryCandidate: null,
          tertiaryCandidate: null,
        });
        return deriveFullCoverPalette({
          meshPrimary: safeSeed,
          meshSecondary: harmonic.secondaryAccent,
          meshTertiary: harmonic.tertiaryAccent,
          dominantAccent: harmonic.dominantAccent,
          secondaryAccent: harmonic.secondaryAccent,
          tertiaryAccent: harmonic.tertiaryAccent,
          quaternaryAccent: harmonic.quaternaryAccent,
          rawDominantAccent: harmonic.rawDominantAccent,
          rawSecondaryAccent: harmonic.rawSecondaryAccent,
          isMonochrome: harmonic.isMonochrome,
          isWarmDominated: harmonic.isWarmDominated,
          dominantHue: harmonic.dominantHue,
        });
      })();

  const containers = isDark
    ? {
        surfaceDim: palette.surfaceDim,
        surfaceContainerLowest: palette.surfaceContainerLowest,
        surfaceContainerLow: palette.surfaceContainerLow,
        surfaceContainer: palette.surfaceContainer,
        surfaceContainerHigh: palette.surfaceContainerHigh,
        surfaceContainerHighest: palette.surfaceContainerHighest,
        surfaceBright: palette.surfaceBright,
      }
    : buildSurfaceContainerPalette(safeSeed, false);

  const domAccent = fullExtracted.dominantAccent;
  const secAccent = fullExtracted.secondaryAccent;
  const terAccent = fullExtracted.tertiaryAccent;
  const quatAccent = fullExtracted.quaternaryAccent;

  const nebulaPrimary =
    effectiveExtracted?.meshPrimary ||
    fullExtracted.meshPrimary ||
    (palette.isMonochrome ? "#1B1D22" : palette.tone20);
  const nebulaSecondary =
    effectiveExtracted?.meshSecondary ||
    fullExtracted.meshSecondary ||
    (palette.isMonochrome ? "#121418" : getToneFromSeed(secAccent, 18));
  const nebulaTertiary =
    effectiveExtracted?.meshTertiary ||
    fullExtracted.meshTertiary ||
    (palette.isMonochrome ? "#23262D" : getToneFromSeed(terAccent, 15));
  const nebulaAbyss = getSurfaceContainerTone(safeSeed, 4, true);

  const lyricPrimary = fullExtracted.lyricActiveHex;
  const lyricSecondary = fullExtracted.lyricSecondaryHex;

  if (isDark) {
    const baseSurface = containers.surfaceContainerLow;
    const primaryContainer = compositeOver(palette.tone30, 0.68, baseSurface);
    const secondaryContainer = compositeOver(palette.tone20, 0.72, baseSurface);

    return {
      palette,
      ...containers,
      primary: palette.tone80,
      onPrimary: palette.tone10,
      primaryContainer,
      onPrimaryContainer: palette.tone95,
      onPrimaryContainerSubtle: palette.tone80,
      secondaryContainer,
      onSecondaryContainer: palette.tone90,
      stageLyricPrimary: lyricPrimary,
      panelTopColor: lerpOklabHex(
        containers.surfaceContainer,
        primaryContainer,
        0.42
      ),
      panelMidColor: containers.surfaceContainerLow,
      panelBottomColor: containers.surfaceContainerLowest,
      nebulaPrimary,
      nebulaSecondary,
      nebulaTertiary,
      nebulaAbyss,
      bgPrimary: fullExtracted.bgPrimary,
      bgSecondary: fullExtracted.bgSecondary,
      bgTertiary: fullExtracted.bgTertiary,
      bgAbyss: fullExtracted.bgAbyss,
      dominantAccent: fullExtracted.dominantAccent,
      secondaryAccent: fullExtracted.secondaryAccent,
      tertiaryAccent: fullExtracted.tertiaryAccent,
      quaternaryAccent: fullExtracted.quaternaryAccent,
      rawDominantAccent: fullExtracted.rawDominantAccent,
      rawSecondaryAccent: fullExtracted.rawSecondaryAccent,
      onDominantAccent: fullExtracted.onDominantAccent,
      onSecondaryAccent: fullExtracted.onSecondaryAccent,
      onSurface: palette.tone95,
      onSurfaceVariant: palette.tone80,
      onSurfaceMuted: getSurfaceContainerTone(safeSeed, 62, true),
      lyricPrimary,
      lyricSecondary,
      lyricActiveHex: fullExtracted.lyricActiveHex,
      lyricGlowRgba: fullExtracted.lyricGlowRgba,
      soundwavePrimary: fullExtracted.soundwavePrimary,
      soundwaveSecondary: fullExtracted.soundwaveSecondary,
      soundwaveTertiary: fullExtracted.soundwaveTertiary,
      soundwaveCoreGlowRgba: fullExtracted.soundwaveCoreGlowRgba,
      soundwaveRingStrokeRgba: fullExtracted.soundwaveRingStrokeRgba,
      starlightPrimary: fullExtracted.starlightPrimary,
      starlightSecondary: fullExtracted.starlightSecondary,
      starlightTertiary: fullExtracted.starlightTertiary,
      starlightWarmGold: fullExtracted.starlightWarmGold,
      nebulaGlow: fullExtracted.nebulaGlow,
      nebulaGlowRgba: fullExtracted.nebulaGlowRgba,
      nebulaSecondaryGlowRgba: fullExtracted.nebulaSecondaryGlowRgba,
      fluidOrb1: fullExtracted.fluidOrb1,
      fluidOrb2: fullExtracted.fluidOrb2,
      fluidOrb3: fullExtracted.fluidOrb3,
      fluidOrb4: fullExtracted.fluidOrb4,
      fluidBlendGlowRgba: fullExtracted.fluidBlendGlowRgba,
      cardSurfacePrimary: fullExtracted.cardSurfacePrimary,
      cardSurfaceElevated: fullExtracted.cardSurfaceElevated,
      cardSurfaceLowest: fullExtracted.cardSurfaceLowest,
      cardSurfaceGlassRgba: fullExtracted.cardSurfaceGlassRgba,
      cardBorderHighlightRgba: fullExtracted.cardBorderHighlightRgba,
      cardBorderSubtleRgba: fullExtracted.cardBorderSubtleRgba,
      cardHoverBgRgba: fullExtracted.cardHoverBgRgba,
      cardActiveBgRgba: fullExtracted.cardActiveBgRgba,
      ambientShadowColor: hexWithAlpha(nebulaAbyss, 0.68),
      isMonochrome: fullExtracted.isMonochrome,
      isWarmDominated: fullExtracted.isWarmDominated,
      dominantHue: fullExtracted.dominantHue,
    };
  }

  const baseSurface = containers.surfaceContainerLow;
  const primaryContainer = compositeOver(palette.tone90, 0.88, baseSurface);
  const secondaryContainer = compositeOver(palette.tone90, 0.76, baseSurface);

    const usePresetLight = Boolean(fullExtracted.isLightStage);
    const lightPrimary = usePresetLight ? fullExtracted.dominantAccent : palette.tone40;
    const lightOnSurface = usePresetLight && fullExtracted.inkPrimary ? fullExtracted.inkPrimary : palette.tone10;
    const lightOnVariant = usePresetLight && fullExtracted.inkSecondary ? fullExtracted.inkSecondary : palette.tone30;
    const lightOnMuted = usePresetLight && fullExtracted.inkMuted ? fullExtracted.inkMuted : palette.tone40;

    return {
      palette,
      ...containers,
      primary: lightPrimary,
      onPrimary: pickReadableOnAccent(lightPrimary),
      primaryContainer,
      onPrimaryContainer: lightOnSurface,
      onPrimaryContainerSubtle: lightOnVariant,
      secondaryContainer,
      onSecondaryContainer: lightOnVariant,
      stageLyricPrimary: usePresetLight ? fullExtracted.lyricActiveHex : lyricPrimary,
      panelTopColor: usePresetLight ? fullExtracted.cardSurfaceElevated : lerpOklabHex(baseSurface, primaryContainer, 0.55),
      panelMidColor: usePresetLight ? fullExtracted.cardSurfacePrimary : containers.surfaceContainer,
      panelBottomColor: usePresetLight ? fullExtracted.cardSurfaceLowest : containers.surfaceContainerLowest,
      nebulaPrimary: usePresetLight ? fullExtracted.meshPrimary : nebulaPrimary,
      nebulaSecondary: usePresetLight ? fullExtracted.meshSecondary : nebulaSecondary,
      nebulaTertiary: usePresetLight ? fullExtracted.meshTertiary : nebulaTertiary,
      nebulaAbyss: usePresetLight ? fullExtracted.bgAbyss : nebulaAbyss,
      bgPrimary: usePresetLight ? fullExtracted.bgPrimary : containers.surfaceContainer,
      bgSecondary: usePresetLight ? fullExtracted.bgSecondary : containers.surfaceContainerLow,
      bgTertiary: usePresetLight ? fullExtracted.bgTertiary : containers.surfaceContainerLowest,
      bgAbyss: usePresetLight ? fullExtracted.bgAbyss : containers.surfaceDim,
      dominantAccent: lightPrimary,
      secondaryAccent: secAccent,
      tertiaryAccent: terAccent,
      quaternaryAccent: quatAccent,
      rawDominantAccent: fullExtracted.rawDominantAccent,
      rawSecondaryAccent: fullExtracted.rawSecondaryAccent,
      onDominantAccent: pickReadableOnAccent(lightPrimary),
      onSecondaryAccent: pickReadableOnAccent(secAccent),
      onSurface: lightOnSurface,
      onSurfaceVariant: lightOnVariant,
      onSurfaceMuted: lightOnMuted,
      lyricPrimary: lightPrimary,
      lyricSecondary: lyricSecondary,
      lyricActiveHex: usePresetLight ? fullExtracted.lyricActiveHex : palette.tone40,
      lyricGlowRgba: hexWithAlpha(lightPrimary, 0.2),
      soundwavePrimary: fullExtracted.soundwavePrimary,
      soundwaveSecondary: fullExtracted.soundwaveSecondary,
      soundwaveTertiary: fullExtracted.soundwaveTertiary,
      soundwaveCoreGlowRgba: fullExtracted.soundwaveCoreGlowRgba,
      soundwaveRingStrokeRgba: fullExtracted.soundwaveRingStrokeRgba,
      starlightPrimary: fullExtracted.starlightPrimary,
      starlightSecondary: fullExtracted.starlightSecondary,
      starlightTertiary: fullExtracted.starlightTertiary,
      starlightWarmGold: fullExtracted.starlightWarmGold,
      nebulaGlow: fullExtracted.nebulaGlow,
      nebulaGlowRgba: fullExtracted.nebulaGlowRgba,
      nebulaSecondaryGlowRgba: fullExtracted.nebulaSecondaryGlowRgba,
      fluidOrb1: fullExtracted.fluidOrb1,
      fluidOrb2: fullExtracted.fluidOrb2,
      fluidOrb3: fullExtracted.fluidOrb3,
      fluidOrb4: fullExtracted.fluidOrb4,
      fluidBlendGlowRgba: fullExtracted.fluidBlendGlowRgba,
      cardSurfacePrimary: usePresetLight ? fullExtracted.cardSurfacePrimary : containers.surfaceContainer,
      cardSurfaceElevated: usePresetLight ? fullExtracted.cardSurfaceElevated : containers.surfaceContainerHigh,
      cardSurfaceLowest: usePresetLight ? fullExtracted.cardSurfaceLowest : containers.surfaceContainerLowest,
      cardSurfaceGlassRgba: hexWithAlpha(
        usePresetLight ? fullExtracted.cardSurfacePrimary : containers.surfaceContainer,
        0.92
      ),
      cardBorderHighlightRgba: hexWithAlpha(lightPrimary, 0.14),
      cardBorderSubtleRgba: hexWithAlpha(lightPrimary, 0.08),
      cardHoverBgRgba: hexWithAlpha(lightPrimary, 0.08),
      cardActiveBgRgba: hexWithAlpha(lightPrimary, 0.15),
      ambientShadowColor: hexWithAlpha(lightOnSurface, 0.14),
      isMonochrome: fullExtracted.isMonochrome,
      isWarmDominated: fullExtracted.isWarmDominated,
      dominantHue: fullExtracted.dominantHue,
    };
}

export function resolveSemanticColorRoles(
  paletteOrSeed?: Partial<CoverExtractedPalette> | string | null,
  isDark = true
): ExtendedCoverColorRoles {
  if (!paletteOrSeed) {
    return toCoverColorRoles(
      DEFAULT_COVER_EXTRACTED_PALETTE.dominantAccent,
      isDark,
      DEFAULT_COVER_EXTRACTED_PALETTE
    );
  }
  if (typeof paletteOrSeed === "string") {
    return toCoverColorRoles(paletteOrSeed, isDark);
  }
  const seed =
    paletteOrSeed.dominantAccent ||
    paletteOrSeed.meshPrimary ||
    DEFAULT_COVER_EXTRACTED_PALETTE.dominantAccent;
  return toCoverColorRoles(seed, isDark, paletteOrSeed);
}

export function paletteToCssVars(
  paletteOrSeed?: Partial<CoverExtractedPalette> | string | null,
  isDark = true
): Record<string, string> {
  const full =
    typeof paletteOrSeed === "string"
      ? ensureFullCoverPalette({ dominantAccent: paletteOrSeed })
      : ensureFullCoverPalette(paletteOrSeed);
  const roles = resolveSemanticColorRoles(full, isDark);

  const accentMidpoint = lerpOklabHex(
    roles.dominantAccent,
    roles.secondaryAccent,
    0.5
  );

  return {
    "--np-accent-primary": roles.dominantAccent,
    "--np-accent-dominant": roles.dominantAccent,
    "--np-accent-secondary": roles.secondaryAccent,
    "--np-accent-tertiary": roles.tertiaryAccent,
    "--np-accent-quaternary": roles.quaternaryAccent,
    "--np-accent-primary-raw": roles.rawDominantAccent,
    "--np-accent-secondary-raw": roles.rawSecondaryAccent,
    "--np-on-accent-primary": roles.onDominantAccent,
    "--np-on-accent-secondary": roles.onSecondaryAccent,
    "--np-accent-midpoint": accentMidpoint,
    "--np-dual-gradient": `linear-gradient(135deg, ${roles.dominantAccent} 0%, ${accentMidpoint} 50%, ${roles.secondaryAccent} 100%)`,
    "--np-primary": roles.dominantAccent,
    "--np-secondary": roles.secondaryAccent,
    "--np-tertiary": roles.tertiaryAccent,
    "--np-quaternary": roles.quaternaryAccent,
    "--np-accent-glow": full.accentGlowRgba,
    "--np-accent-subtle": full.accentSubtleRgba,
    "--np-secondary-glow": full.accentSecondaryGlowRgba,
    "--np-secondary-subtle": full.accentSecondarySubtleRgba,
    "--np-cover-aura-a": hexWithAlpha(roles.rawDominantAccent, 0.34),
    "--np-cover-aura-b": hexWithAlpha(roles.rawSecondaryAccent, 0.28),
    "--np-primary-glow": full.accentGlowRgba,
    "--np-primary-soft": full.accentSubtleRgba,
    "--np-accent-primary-rgb": hexToRgbTriplet(roles.dominantAccent),
    "--np-accent-secondary-rgb": hexToRgbTriplet(roles.secondaryAccent),
    "--np-accent-tertiary-rgb": hexToRgbTriplet(roles.tertiaryAccent),
    "--np-accent-quaternary-rgb": hexToRgbTriplet(roles.quaternaryAccent),

    "--np-bg-primary": roles.bgPrimary,
    "--np-bg-secondary": roles.bgSecondary,
    "--np-bg-tertiary": roles.bgTertiary,
    "--np-bg-abyss": roles.bgAbyss,
    "--np-mesh-primary": full.meshPrimary,
    "--np-mesh-secondary": full.meshSecondary,
    "--np-mesh-tertiary": full.meshTertiary,
    "--np-nebula-primary": roles.nebulaPrimary,
    "--np-nebula-secondary": roles.nebulaSecondary,
    "--np-nebula-tertiary": roles.nebulaTertiary,
    "--np-nebula-abyss": roles.nebulaAbyss,

    "--np-canvas-primary": full.canvasPrimary,
    "--np-canvas-secondary": full.canvasSecondary,
    "--np-canvas-glow": full.canvasAccentGlowRgba,
    "--np-canvas-vignette": full.canvasVignetteRgba,

    "--np-fluid-1": roles.fluidOrb1,
    "--np-fluid-2": roles.fluidOrb2,
    "--np-fluid-3": roles.fluidOrb3,
    "--np-fluid-4": roles.fluidOrb4,
    "--np-fluid-blend-glow": roles.fluidBlendGlowRgba,

    "--np-soundwave-1": roles.soundwavePrimary,
    "--np-soundwave-2": roles.soundwaveSecondary,
    "--np-soundwave-3": roles.soundwaveTertiary,
    "--np-soundwave-primary": roles.soundwavePrimary,
    "--np-soundwave-secondary": roles.soundwaveSecondary,
    "--np-soundwave-tertiary": roles.soundwaveTertiary,
    "--np-soundwave-core-glow": roles.soundwaveCoreGlowRgba,
    "--np-soundwave-ring-stroke": roles.soundwaveRingStrokeRgba,

    "--np-starlight-1": roles.starlightPrimary,
    "--np-starlight-2": roles.starlightSecondary,
    "--np-starlight-3": roles.starlightTertiary,
    "--np-starlight-gold": roles.starlightWarmGold,
    "--np-starlight-primary": roles.starlightPrimary,
    "--np-starlight-secondary": roles.starlightSecondary,
    "--np-starlight-tertiary": roles.starlightTertiary,
    "--np-nebula-glow": roles.nebulaGlow,
    "--np-nebula-glow-rgba": roles.nebulaGlowRgba,
    "--np-nebula-secondary-glow": roles.nebulaSecondaryGlowRgba,

    "--np-lyric-active": roles.lyricActiveHex,
    "--np-lyric-primary": roles.lyricPrimary,
    "--np-lyric-secondary": roles.lyricSecondary,
    "--np-lyric-glow": roles.lyricGlowRgba,
    "--np-lyric-inactive": full.lyricInactiveRgba,
    "--np-lyric-translation": full.lyricTranslationRgba,

    "--np-surface-dim": roles.surfaceDim,
    "--np-surface-container-lowest": roles.surfaceContainerLowest,
    "--np-surface-container-low": roles.surfaceContainerLow,
    "--np-surface-container": roles.surfaceContainer,
    "--np-surface-container-high": roles.surfaceContainerHigh,
    "--np-surface-container-highest": roles.surfaceContainerHighest,
    "--np-surface-bright": roles.surfaceBright,
    "--np-card-bg": roles.cardSurfacePrimary,
    "--np-card-surface": roles.cardSurfacePrimary,
    "--np-card-elevated": roles.cardSurfaceElevated,
    "--np-card-surface-elevated": roles.cardSurfaceElevated,
    "--np-card-lowest": roles.cardSurfaceLowest,
    "--np-card-glass": roles.cardSurfaceGlassRgba,
    "--np-card-border-highlight": roles.cardBorderHighlightRgba,
    "--np-border-highlight": roles.cardBorderHighlightRgba,
    "--np-card-border-subtle": roles.cardBorderSubtleRgba,
    "--np-border-subtle": roles.cardBorderSubtleRgba,
    "--np-card-hover-bg": roles.cardHoverBgRgba,
    "--np-card-active-bg": roles.cardActiveBgRgba,
    "--np-card-ambient-glow": full.cardAmbientGlowRgba,
    "--np-ambient-shadow": roles.ambientShadowColor,
    "--np-on-surface": roles.onSurface,
    "--np-on-surface-variant": roles.onSurfaceVariant,
    "--np-on-surface-muted": roles.onSurfaceMuted,
    "--np-ink-1": roles.onSurface,
    "--np-ink-2": roles.onSurfaceVariant,
    "--np-ink-3": roles.onSurfaceMuted,

    "--np-prism-cyan": full.prismCyan,
    "--np-prism-magenta": full.prismMagenta,
    "--np-prism-violet": full.prismViolet,
    "--np-prism-gold": full.prismGold,
  };
}

export function buildBorderlessCssVars(
  paletteOrSeed?: Partial<CoverExtractedPalette> | string | null,
  isDark = true
): Record<string, string> {
  return paletteToCssVars(paletteOrSeed, isDark);
}

// ============================================================================
// 6. 多级容错图片采样 + 区域加权色相直方图 + 桶内连续圆周均值主引擎
// ============================================================================

export function normalizeCoverUrl(
  rawUrl?: string | null,
  size = 480
): string {
  const trimmed = (rawUrl || "").trim();
  if (!trimmed) return "";
  let normalized = trimmed
    .replace(/\{size\}/gi, String(size))
    .replace(/%7Bsize%7D/gi, String(size))
    .replace(/\{param\}/gi, String(size));
  if (normalized.startsWith("//")) {
    normalized = `https:${normalized}`;
  }
  return normalized;
}

/**
 * 构建多档位高清封面 URL 探针序列（优先剥离缩略段直取母版原图，再按 2000 -> 1440 -> 1080 -> 720 -> 480 降级）
 */
export function buildCoverResolutionCandidates(
  rawUrl?: string | null,
  extraCandidates: Array<string | null | undefined> = []
): string[] {
  const out: string[] = [];
  const pushUnique = (u: string) => {
    const clean = u.trim();
    if (!clean) return;
    const httpsClean = clean.startsWith("//") ? `https:${clean}` : clean;
    if (!out.includes(httpsClean)) out.push(httpsClean);
  };

  const seeds = [rawUrl, ...extraCandidates].filter(
    (s): s is string => typeof s === "string" && s.trim().length > 0
  );

  for (const seed of seeds) {
    const cleanSeed = seed.trim().startsWith("//")
      ? `https:${seed.trim()}`
      : seed.trim();
    // 1. 尝试去除 /{size}/ 或 /{120..1000}/ 缩放段，请求音源原图
    const unscaled = cleanSeed
      .replace(/\/\{size\}\//gi, "/")
      .replace(/\/%7Bsize%7D\//gi, "/")
      .replace(/\/\{param\}\//gi, "/")
      .replace(/\/(?:120|150|200|240|320|400|480|640|720|1000)\//g, "/");
    pushUnique(unscaled);

    // 2. 从高到低构造标准缩放规格（同时兼容含 {size} 模板与已被替换成 /{n}/ 的现有链接）
    const templateLike = cleanSeed.replace(
      /\/(?:120|150|200|240|320|400|480|640|720|1000)\//g,
      "/{size}/"
    );
    for (const sz of [2000, 1440, 1080, 1000, 720, 480]) {
      pushUnique(normalizeCoverUrl(templateLike, sz));
    }
    pushUnique(normalizeCoverUrl(cleanSeed, 1080));
    pushUnique(cleanSeed);
  }
  return out;
}

export interface ResolvedCoverMetrics {
  url: string;
  width: number;
  height: number;
}

function probeSingleImageDimensions(
  url: string,
  timeoutMs = 2200
): Promise<ResolvedCoverMetrics | null> {
  if (typeof window === "undefined") return Promise.resolve(null);
  return new Promise((resolve) => {
    const img = new Image();
    let done = false;
    const finish = (res: ResolvedCoverMetrics | null) => {
      if (done) return;
      done = true;
      resolve(res);
    };
    const timer = window.setTimeout(() => finish(null), timeoutMs);
    img.onload = () => {
      window.clearTimeout(timer);
      const w = img.naturalWidth || 0;
      const h = img.naturalHeight || 0;
      if (w >= 64 && h >= 64) {
        finish({ url, width: w, height: h });
      } else {
        finish(null);
      }
    };
    img.onerror = () => {
      window.clearTimeout(timer);
      finish(null);
    };
    img.src = url;
  });
}

/**
 * 并行探针选出实际像素面积最大的最高清可用封面 URL
 */
export async function resolveBestCoverWithMetrics(
  rawUrl?: string | null,
  extraCandidates: Array<string | null | undefined> = []
): Promise<ResolvedCoverMetrics | null> {
  const candidates = buildCoverResolutionCandidates(rawUrl, extraCandidates).slice(0, 8);
  if (candidates.length === 0) return null;

  // 第一批优先探测原图、2000、1440、1080；命中 >= 1000px 直接择优返回
  const primaryBatch = await Promise.all(
    candidates.slice(0, 4).map((u) => probeSingleImageDimensions(u, 2200))
  );
  let best: ResolvedCoverMetrics | null = null;
  for (const item of primaryBatch) {
    if (!item) continue;
    if (!best || item.width * item.height > best.width * best.height) {
      best = item;
    }
  }
  if (best && Math.max(best.width, best.height) >= 960) {
    return best;
  }

  // 兜底探测剩余规格
  if (candidates.length > 4) {
    const fallbackBatch = await Promise.all(
      candidates.slice(4).map((u) => probeSingleImageDimensions(u, 1800))
    );
    for (const item of fallbackBatch) {
      if (!item) continue;
      if (!best || item.width * item.height > best.width * best.height) {
        best = item;
      }
    }
  }
  return best;
}

function drawSourceToImageData(
  source: CanvasImageSource,
  w: number,
  h: number
): Uint8ClampedArray | null {
  try {
    const canvas = document.createElement("canvas");
    canvas.width = w;
    canvas.height = h;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) return null;
    ctx.drawImage(source, 0, 0, w, h);
    return ctx.getImageData(0, 0, w, h).data;
  } catch {
    return null;
  }
}

function loadViaHtmlImage(
  url: string,
  useCrossOrigin: boolean,
  timeoutMs = 2600
): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image();
    if (useCrossOrigin) {
      img.crossOrigin = "anonymous";
    }
    let done = false;
    const finish = (res: HTMLImageElement | null) => {
      if (done) return;
      done = true;
      resolve(res);
    };
    const timer = window.setTimeout(() => finish(null), timeoutMs);
    img.onload = () => {
      window.clearTimeout(timer);
      finish(img);
    };
    img.onerror = () => {
      window.clearTimeout(timer);
      finish(null);
    };
    img.src = url;
  });
}

async function fetchBlobImageData(
  url: string,
  w: number,
  h: number
): Promise<Uint8ClampedArray | null> {
  const candidateUrls: string[] = [url];
  if (url.startsWith("http://")) {
    candidateUrls.push(`https://${url.slice("http://".length)}`);
  } else if (url.startsWith("https://")) {
    candidateUrls.push(`http://${url.slice("https://".length)}`);
  }

  for (const targetUrl of candidateUrls) {
    try {
      const controller = new AbortController();
      const timeoutId = window.setTimeout(() => controller.abort(), 2800);
      const resp = await fetch(targetUrl, {
        mode: "cors",
        credentials: "omit",
        signal: controller.signal,
      });
      window.clearTimeout(timeoutId);
      if (!resp.ok) continue;
      const blob = await resp.blob();
      if (!blob || blob.size === 0) continue;

      if (typeof createImageBitmap === "function") {
        try {
          const bmp = await createImageBitmap(blob);
          const data = drawSourceToImageData(bmp, w, h);
          bmp.close?.();
          if (data) return data;
        } catch {
          // fallback to ObjectURL below
        }
      }

      const objUrl = URL.createObjectURL(blob);
      try {
        const img = await loadViaHtmlImage(objUrl, false, 2000);
        if (img) {
          const data = drawSourceToImageData(img, w, h);
          if (data) return data;
        }
      } finally {
        URL.revokeObjectURL(objUrl);
      }
    } catch {
      // try next candidate url
    }
  }
  return null;
}

async function fetchNativeCoverViaTauri(
  url: string,
  w: number,
  h: number
): Promise<Uint8ClampedArray | null> {
  if (typeof window === "undefined") return null;
  if (!url.startsWith("https://") && !url.startsWith("http://")) return null;
  try {
    const dataUrl = await invoke<string>("fetch_cover_image_base64_cmd", {
      url,
    });
    if (!dataUrl || !dataUrl.startsWith("data:image/")) return null;
    const img = await loadViaHtmlImage(dataUrl, false, 2200);
    if (!img) return null;
    return drawSourceToImageData(img, w, h);
  } catch {
    return null;
  }
}

async function loadCoverImageDataResilient(
  normalizedUrl: string,
  w = 80,
  h = 80
): Promise<Uint8ClampedArray | null> {
  const corsImg = await loadViaHtmlImage(normalizedUrl, true, 2400);
  if (corsImg) {
    const data = drawSourceToImageData(corsImg, w, h);
    if (data) return data;
  }

  const blobData = await fetchBlobImageData(normalizedUrl, w, h);
  if (blobData) return blobData;

  // Rust 原生 SSRF 受控旁路拉取：绕开桌面 WebView2 对无 CORS 唱片封面 CDN 的像素读取拦截
  const nativeData = await fetchNativeCoverViaTauri(normalizedUrl, w, h);
  if (nativeData) return nativeData;

  if (normalizedUrl.startsWith("http://")) {
    const httpsUrl = `https://${normalizedUrl.slice("http://".length)}`;
    const httpsImg = await loadViaHtmlImage(httpsUrl, true, 2000);
    if (httpsImg) {
      const data = drawSourceToImageData(httpsImg, w, h);
      if (data) return data;
    }
    const nativeHttpsData = await fetchNativeCoverViaTauri(httpsUrl, w, h);
    if (nativeHttpsData) return nativeHttpsData;
  }

  return null;
}

const SOFT_COVER_TEX_CACHE = new Map<
  string,
  { width: number; height: number; data: Uint8ClampedArray }
>();
const SOFT_COVER_TEX_CACHE_LIMIT = 8;

function boxBlurRgbaOnce(
  src: Uint8ClampedArray,
  w: number,
  h: number
): Uint8ClampedArray {
  const tmp = new Uint8ClampedArray(src.length);
  const dst = new Uint8ClampedArray(src.length);
  // 水平 3-tap 柔化
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const xl = Math.max(0, x - 1);
      const xr = Math.min(w - 1, x + 1);
      const il = (y * w + xl) * 4;
      const ic = (y * w + x) * 4;
      const ir = (y * w + xr) * 4;
      tmp[ic] = (src[il] + src[ic] * 2 + src[ir]) >> 2;
      tmp[ic + 1] = (src[il + 1] + src[ic + 1] * 2 + src[ir + 1]) >> 2;
      tmp[ic + 2] = (src[il + 2] + src[ic + 2] * 2 + src[ir + 2]) >> 2;
      tmp[ic + 3] = 255;
    }
  }
  // 垂直 3-tap 柔化
  for (let y = 0; y < h; y++) {
    const yt = Math.max(0, y - 1);
    const yb = Math.min(h - 1, y + 1);
    for (let x = 0; x < w; x++) {
      const it = (yt * w + x) * 4;
      const ic = (y * w + x) * 4;
      const ib = (yb * w + x) * 4;
      dst[ic] = (tmp[it] + tmp[ic] * 2 + tmp[ib]) >> 2;
      dst[ic + 1] = (tmp[it + 1] + tmp[ic + 1] * 2 + tmp[ib + 1]) >> 2;
      dst[ic + 2] = (tmp[it + 2] + tmp[ic + 2] * 2 + tmp[ib + 2]) >> 2;
      dst[ic + 3] = 255;
    }
  }
  return dst;
}

/**
 * 为 WebGL2「液态丝绸」封面丝绸底层拉取低频柔化 RGBA 像素（64×64）：
 * - 复用 `loadCoverImageDataResilient` + Rust `fetch_cover_image_base64_cmd` 安全兜底，100% 零 CORS 污染；
 * - 经过 2 遍轻柔盒式滤波抹除锐利文字/噪点，化作纯粹的专辑色彩丝绸底纹；
 * - 内置 8 槽 LRU 缓存，重复切回同一首歌 0ms 瞬时命中。
 */
export async function loadCoverSoftPixelsResilient(
  rawUrl?: string | null,
  size = 64
): Promise<{ width: number; height: number; data: Uint8ClampedArray } | null> {
  const candidates = buildCoverResolutionCandidates(rawUrl);
  if (candidates.length === 0) return null;
  const cacheKey = `${candidates[0]}@${size}`;
  const cached = SOFT_COVER_TEX_CACHE.get(cacheKey);
  if (cached) {
    SOFT_COVER_TEX_CACHE.delete(cacheKey);
    SOFT_COVER_TEX_CACHE.set(cacheKey, cached);
    return cached;
  }

  for (const url of candidates) {
    const raw = await loadCoverImageDataResilient(url, size, size);
    if (!raw || raw.length < size * size * 4) continue;
    const pass1 = boxBlurRgbaOnce(raw, size, size);
    const soft = boxBlurRgbaOnce(pass1, size, size);
    const entry = { width: size, height: size, data: soft };
    SOFT_COVER_TEX_CACHE.set(cacheKey, entry);
    while (SOFT_COVER_TEX_CACHE.size > SOFT_COVER_TEX_CACHE_LIMIT) {
      const oldestKey = SOFT_COVER_TEX_CACHE.keys().next().value;
      if (oldestKey !== undefined) SOFT_COVER_TEX_CACHE.delete(oldestKey);
    }
    return entry;
  }
  return null;
}
// ============================================================================

const WU_DIM = 33; // 5-bit quantized RGB (1..32, index 0 is prefix-sum zero border)
const WU_TABLE_SIZE = WU_DIM * WU_DIM * WU_DIM;

interface WuBox {
  r0: number;
  r1: number;
  g0: number;
  g1: number;
  b0: number;
  b1: number;
  vol: number;
}

function wuIdx(r: number, g: number, b: number): number {
  return (r * WU_DIM + g) * WU_DIM + b;
}

function wuVolume(cube: WuBox, m: Float64Array): number {
  return (
    m[wuIdx(cube.r1, cube.g1, cube.b1)] -
    m[wuIdx(cube.r1, cube.g1, cube.b0)] -
    m[wuIdx(cube.r1, cube.g0, cube.b1)] +
    m[wuIdx(cube.r1, cube.g0, cube.b0)] -
    m[wuIdx(cube.r0, cube.g1, cube.b1)] +
    m[wuIdx(cube.r0, cube.g1, cube.b0)] +
    m[wuIdx(cube.r0, cube.g0, cube.b1)] -
    m[wuIdx(cube.r0, cube.g0, cube.b0)]
  );
}

function wuBottom(cube: WuBox, dir: 0 | 1 | 2, m: Float64Array): number {
  if (dir === 0) {
    return (
      -m[wuIdx(cube.r0, cube.g1, cube.b1)] +
      m[wuIdx(cube.r0, cube.g1, cube.b0)] +
      m[wuIdx(cube.r0, cube.g0, cube.b1)] -
      m[wuIdx(cube.r0, cube.g0, cube.b0)]
    );
  }
  if (dir === 1) {
    return (
      -m[wuIdx(cube.r1, cube.g0, cube.b1)] +
      m[wuIdx(cube.r1, cube.g0, cube.b0)] +
      m[wuIdx(cube.r0, cube.g0, cube.b1)] -
      m[wuIdx(cube.r0, cube.g0, cube.b0)]
    );
  }
  return (
    -m[wuIdx(cube.r1, cube.g1, cube.b0)] +
    m[wuIdx(cube.r1, cube.g0, cube.b0)] +
    m[wuIdx(cube.r0, cube.g1, cube.b0)] -
    m[wuIdx(cube.r0, cube.g0, cube.b0)]
  );
}

function wuTop(
  cube: WuBox,
  dir: 0 | 1 | 2,
  pos: number,
  m: Float64Array
): number {
  if (dir === 0) {
    return (
      m[wuIdx(pos, cube.g1, cube.b1)] -
      m[wuIdx(pos, cube.g1, cube.b0)] -
      m[wuIdx(pos, cube.g0, cube.b1)] +
      m[wuIdx(pos, cube.g0, cube.b0)]
    );
  }
  if (dir === 1) {
    return (
      m[wuIdx(cube.r1, pos, cube.b1)] -
      m[wuIdx(cube.r1, pos, cube.b0)] -
      m[wuIdx(cube.r0, pos, cube.b1)] +
      m[wuIdx(cube.r0, pos, cube.b0)]
    );
  }
  return (
    m[wuIdx(cube.r1, cube.g1, pos)] -
    m[wuIdx(cube.r1, cube.g0, pos)] -
    m[wuIdx(cube.r0, cube.g1, pos)] +
    m[wuIdx(cube.r0, cube.g0, pos)]
  );
}

interface ColorCell5Bit {
  L: number;
  a: number;
  b: number;
  w: number;
  areaW: number;
  z1W: number;
  z2W: number;
  z3W: number;
}

function wuQuantizeToOkLabSeeds(
  cells: Array<{ rSum: number; gSum: number; bSum: number; w: number }>,
  maxClusters = 20
): OklabColor[] {
  const vwt = new Float64Array(WU_TABLE_SIZE);
  const vmr = new Float64Array(WU_TABLE_SIZE);
  const vmg = new Float64Array(WU_TABLE_SIZE);
  const vmb = new Float64Array(WU_TABLE_SIZE);
  const vm2 = new Float64Array(WU_TABLE_SIZE);

  for (let i = 0; i < cells.length; i++) {
    const c = cells[i];
    if (c.w <= 1e-7) continue;
    const meanR = c.rSum / c.w;
    const meanG = c.gSum / c.w;
    const meanB = c.bSum / c.w;
    const qr = clamp(Math.floor(meanR) >> 3, 0, 31) + 1;
    const qg = clamp(Math.floor(meanG) >> 3, 0, 31) + 1;
    const qb = clamp(Math.floor(meanB) >> 3, 0, 31) + 1;
    const idx = wuIdx(qr, qg, qb);
    vwt[idx] += c.w;
    vmr[idx] += c.w * meanR;
    vmg[idx] += c.w * meanG;
    vmb[idx] += c.w * meanB;
    vm2[idx] += c.w * (meanR * meanR + meanG * meanG + meanB * meanB);
  }

  const areaW = new Float64Array(WU_DIM);
  const areaR = new Float64Array(WU_DIM);
  const areaG = new Float64Array(WU_DIM);
  const areaB = new Float64Array(WU_DIM);
  const area2 = new Float64Array(WU_DIM);

  for (let r = 1; r < WU_DIM; r++) {
    areaW.fill(0);
    areaR.fill(0);
    areaG.fill(0);
    areaB.fill(0);
    area2.fill(0);
    for (let g = 1; g < WU_DIM; g++) {
      let lineW = 0;
      let lineR = 0;
      let lineG = 0;
      let lineB = 0;
      let line2 = 0;
      for (let b = 1; b < WU_DIM; b++) {
        const i = wuIdx(r, g, b);
        const prev = wuIdx(r - 1, g, b);
        lineW += vwt[i];
        lineR += vmr[i];
        lineG += vmg[i];
        lineB += vmb[i];
        line2 += vm2[i];

        areaW[b] += lineW;
        areaR[b] += lineR;
        areaG[b] += lineG;
        areaB[b] += lineB;
        area2[b] += line2;

        vwt[i] = vwt[prev] + areaW[b];
        vmr[i] = vmr[prev] + areaR[b];
        vmg[i] = vmg[prev] + areaG[b];
        vmb[i] = vmb[prev] + areaB[b];
        vm2[i] = vm2[prev] + area2[b];
      }
    }
  }

  const variance = (cube: WuBox): number => {
    const dw = wuVolume(cube, vwt);
    if (dw <= 1e-8) return 0;
    const dr = wuVolume(cube, vmr);
    const dg = wuVolume(cube, vmg);
    const db = wuVolume(cube, vmb);
    const dm2 = wuVolume(cube, vm2);
    return Math.max(0, dm2 - (dr * dr + dg * dg + db * db) / dw);
  };

  const maximize = (
    cube: WuBox,
    dir: 0 | 1 | 2,
    first: number,
    last: number,
    wholeR: number,
    wholeG: number,
    wholeB: number,
    wholeW: number
  ): { max: number; cut: number } => {
    const baseR = wuBottom(cube, dir, vmr);
    const baseG = wuBottom(cube, dir, vmg);
    const baseB = wuBottom(cube, dir, vmb);
    const baseW = wuBottom(cube, dir, vwt);

    let max = 0;
    let cut = -1;
    for (let i = first; i < last; i++) {
      let halfW = baseW + wuTop(cube, dir, i, vwt);
      if (halfW <= 1e-7) continue;
      let halfR = baseR + wuTop(cube, dir, i, vmr);
      let halfG = baseG + wuTop(cube, dir, i, vmg);
      let halfB = baseB + wuTop(cube, dir, i, vmb);
      let temp = (halfR * halfR + halfG * halfG + halfB * halfB) / halfW;

      halfW = wholeW - halfW;
      if (halfW <= 1e-7) continue;
      halfR = wholeR - halfR;
      halfG = wholeG - halfG;
      halfB = wholeB - halfB;
      temp += (halfR * halfR + halfG * halfG + halfB * halfB) / halfW;

      if (temp > max) {
        max = temp;
        cut = i;
      }
    }
    return { max, cut };
  };

  const tryCutBox = (set1: WuBox, set2: WuBox): boolean => {
    const wholeR = wuVolume(set1, vmr);
    const wholeG = wuVolume(set1, vmg);
    const wholeB = wuVolume(set1, vmb);
    const wholeW = wuVolume(set1, vwt);
    if (wholeW <= 1e-7) return false;

    const rCut = maximize(
      set1,
      0,
      set1.r0 + 1,
      set1.r1,
      wholeR,
      wholeG,
      wholeB,
      wholeW
    );
    const gCut = maximize(
      set1,
      1,
      set1.g0 + 1,
      set1.g1,
      wholeR,
      wholeG,
      wholeB,
      wholeW
    );
    const bCut = maximize(
      set1,
      2,
      set1.b0 + 1,
      set1.b1,
      wholeR,
      wholeG,
      wholeB,
      wholeW
    );

    let dir: 0 | 1 | 2;
    let cutPos: number;
    if (rCut.max >= gCut.max && rCut.max >= bCut.max) {
      dir = 0;
      cutPos = rCut.cut;
    } else if (gCut.max >= rCut.max && gCut.max >= bCut.max) {
      dir = 1;
      cutPos = gCut.cut;
    } else {
      dir = 2;
      cutPos = bCut.cut;
    }
    if (cutPos < 0) return false;

    set2.r1 = set1.r1;
    set2.g1 = set1.g1;
    set2.b1 = set1.b1;
    if (dir === 0) {
      set2.r0 = set1.r1 = cutPos;
      set2.g0 = set1.g0;
      set2.b0 = set1.b0;
    } else if (dir === 1) {
      set2.g0 = set1.g1 = cutPos;
      set2.r0 = set1.r0;
      set2.b0 = set1.b0;
    } else {
      set2.b0 = set1.b1 = cutPos;
      set2.r0 = set1.r0;
      set2.g0 = set1.g0;
    }

    set1.vol =
      (set1.r1 - set1.r0) * (set1.g1 - set1.g0) * (set1.b1 - set1.b0);
    set2.vol =
      (set2.r1 - set2.r0) * (set2.g1 - set2.g0) * (set2.b1 - set2.b0);
    return true;
  };

  const cubes: WuBox[] = Array.from({ length: maxClusters }, () => ({
    r0: 0,
    r1: 0,
    g0: 0,
    g1: 0,
    b0: 0,
    b1: 0,
    vol: 0,
  }));
  const vv = new Float64Array(maxClusters);
  cubes[0] = {
    r0: 0,
    r1: 32,
    g0: 0,
    g1: 32,
    b0: 0,
    b1: 32,
    vol: 32 * 32 * 32,
  };

  let next = 0;
  let actualK = maxClusters;
  for (let i = 1; i < maxClusters; i++) {
    if (tryCutBox(cubes[next], cubes[i])) {
      vv[next] = cubes[next].vol > 1 ? variance(cubes[next]) : 0;
      vv[i] = cubes[i].vol > 1 ? variance(cubes[i]) : 0;
    } else {
      vv[next] = 0;
      i--;
    }
    next = 0;
    let bestVar = vv[0];
    for (let k = 1; k <= i; k++) {
      if (vv[k] > bestVar) {
        bestVar = vv[k];
        next = k;
      }
    }
    if (bestVar <= 1e-6) {
      actualK = i + 1;
      break;
    }
  }

  const seeds: OklabColor[] = [];
  for (let k = 0; k < actualK; k++) {
    const wt = wuVolume(cubes[k], vwt);
    if (wt <= 1e-7) continue;
    const r = clamp(wuVolume(cubes[k], vmr) / wt, 0, 255);
    const g = clamp(wuVolume(cubes[k], vmg) / wt, 0, 255);
    const b = clamp(wuVolume(cubes[k], vmb) / wt, 0, 255);
    seeds.push(rgbToOklab(r, g, b));
  }
  return seeds;
}

interface ColorCluster {
  L: number;
  a: number;
  b: number;
  w: number;
  areaW: number;
  z1W: number;
  z2W: number;
  z3W: number;
  C: number;
  hue: number;
  hsv: HsvTuple;
  hex: string;
}

function oklabDist(
  x: { L: number; a: number; b: number },
  y: { L: number; a: number; b: number }
): number {
  const dL = x.L - y.L;
  const da = x.a - y.a;
  const db = x.b - y.b;
  return Math.sqrt(dL * dL + da * da + db * db);
}

/**
 * 【第二代核心引擎】视觉显著度加权 + 3D Wu 量化 + OKLab WSMeans 聚类 + 双主角提取 (`analyzeCoverPixels`)
 */
export function analyzeCoverPixels(
  imageData: Uint8ClampedArray,
  w: number,
  h: number
): CoverExtractedPalette {
  const pixelCount = w * h;
  if (pixelCount <= 0 || imageData.length < pixelCount * 4) {
    return { ...DEFAULT_COVER_EXTRACTED_PALETTE };
  }

  const bufL = new Float32Array(pixelCount);
  const bufA = new Float32Array(pixelCount);
  const bufB = new Float32Array(pixelCount);
  const bufValid = new Uint8Array(pixelCount);

  for (let p = 0; p < pixelCount; p++) {
    const off = p * 4;
    if (imageData[off + 3] < 64) continue;
    const lab = rgbToOklab(imageData[off], imageData[off + 1], imageData[off + 2]);
    bufL[p] = lab.L;
    bufA[p] = lab.a;
    bufB[p] = lab.b;
    bufValid[p] = 1;
  }

  let validPixelCount = 0;
  let chromaticPixelCount = 0;
  let globalChromaSum = 0;
  let globalValidWeight = 0;
  let totalFgAreaW = 0;

  interface CellAccum {
    L: number;
    a: number;
    b: number;
    rSum: number;
    gSum: number;
    bSum: number;
    w: number;
    areaW: number;
    z1W: number;
    z2W: number;
    z3W: number;
  }
  const cellMap = new Map<number, CellAccum>();

  for (let y = 0; y < h; y++) {
    const ny = (y + 0.5) / h;
    const edgeMarginY = Math.min(ny, 1 - ny);
    const edgeGuardY =
      edgeMarginY < 0.05 ? 0.30 + (edgeMarginY / 0.05) * 0.70 : 1.0;

    for (let x = 0; x < w; x++) {
      const p = y * w + x;
      if (!bufValid[p]) continue;

      const off = p * 4;
      const r = imageData[off];
      const g = imageData[off + 1];
      const b = imageData[off + 2];

      const L = bufL[p];
      const a = bufA[p];
      const bOk = bufB[p];
      const chroma = Math.hypot(a, bOk);
      const { s: sat, v: val } = rgbToHsv(r, g, b);

      validPixelCount++;
      globalChromaSum += chroma;
      globalValidWeight += 1;

      if (chroma >= 0.036 && sat >= 0.12 && L > 0.06 && L < 0.96) {
        chromaticPixelCount++;
      }

      const isExtremeDark = L < 0.055 || val < 0.045;
      const isExtremeWhite = L > 0.962 && chroma < 0.022;
      if (isExtremeDark || isExtremeWhite) continue;

      const nx = (x + 0.5) / w;
      const edgeMarginX = Math.min(nx, 1 - nx);
      const edgeGuardX =
        edgeMarginX < 0.05 ? 0.30 + (edgeMarginX / 0.05) * 0.70 : 1.0;
      const edgeGuard = edgeGuardX * edgeGuardY;

      const dx = (nx - 0.5) / 0.44;
      const dy = (ny - 0.46) / 0.44;
      const wSpatial =
        edgeGuard * (0.52 + 0.98 * Math.exp(-(dx * dx + dy * dy) * 0.85));

      let grad = 0;
      if (x + 1 < w && bufValid[p + 1]) {
        grad +=
          Math.abs(bufL[p + 1] - L) +
          1.5 * (Math.abs(bufA[p + 1] - a) + Math.abs(bufB[p + 1] - bOk));
      }
      if (y + 1 < h && bufValid[p + w]) {
        grad +=
          Math.abs(bufL[p + w] - L) +
          1.5 * (Math.abs(bufA[p + w] - a) + Math.abs(bufB[p + w] - bOk));
      }
      const wEdge = 0.85 + clamp(grad * 2.8, 0, 0.75);

      const wChroma =
        0.24 + Math.pow(clamp(chroma / 0.22, 0, 1.35), 1.05) * 1.45;
      const wTone = L >= 0.14 && L <= 0.90 ? 1.0 : 0.45;

      const fgWeight = wSpatial * wEdge * wChroma * wTone;
      if (fgWeight <= 1e-6) continue;

      const diag = (nx + ny) * 0.5;
      const z1W = Math.exp(-Math.pow((diag - 0.20) / 0.26, 2)) * fgWeight;
      const z2W = Math.exp(-Math.pow((diag - 0.50) / 0.24, 2)) * fgWeight;
      const z3W = Math.exp(-Math.pow((diag - 0.80) / 0.26, 2)) * fgWeight;

      totalFgAreaW += 1;

      const qr = r >> 3;
      const qg = g >> 3;
      const qb = b >> 3;
      const key = (qr << 10) | (qg << 5) | qb;
      let cell = cellMap.get(key);
      if (!cell) {
        cell = {
          L: 0,
          a: 0,
          b: 0,
          rSum: 0,
          gSum: 0,
          bSum: 0,
          w: 0,
          areaW: 0,
          z1W: 0,
          z2W: 0,
          z3W: 0,
        };
        cellMap.set(key, cell);
      }
      cell.L += fgWeight * L;
      cell.a += fgWeight * a;
      cell.b += fgWeight * bOk;
      cell.rSum += fgWeight * r;
      cell.gSum += fgWeight * g;
      cell.bSum += fgWeight * b;
      cell.w += fgWeight;
      cell.areaW += 1;
      cell.z1W += z1W;
      cell.z2W += z2W;
      cell.z3W += z3W;
    }
  }

  if (validPixelCount === 0) {
    return { ...DEFAULT_COVER_EXTRACTED_PALETTE };
  }

  const chromaticRatio = chromaticPixelCount / Math.max(1, validPixelCount);
  const meanChroma = globalChromaSum / Math.max(1, globalValidWeight);
  if (
    (chromaticRatio < 0.024 && meanChroma < 0.020) ||
    cellMap.size === 0
  ) {
    return { ...MONOCHROME_COVER_EXTRACTED_PALETTE };
  }

  const rawCells = Array.from(cellMap.values());
  const cells: ColorCell5Bit[] = rawCells.map((c) => ({
    L: c.L / c.w,
    a: c.a / c.w,
    b: c.b / c.w,
    w: c.w,
    areaW: c.areaW,
    z1W: c.z1W,
    z2W: c.z2W,
    z3W: c.z3W,
  }));

  const centroids = wuQuantizeToOkLabSeeds(rawCells, 20);
  if (centroids.length === 0) {
    return { ...MONOCHROME_COVER_EXTRACTED_PALETTE };
  }

  const assign = new Int32Array(cells.length);
  const kCount = centroids.length;
  const sumW = new Float64Array(kCount);
  const sumAreaW = new Float64Array(kCount);
  const sumL = new Float64Array(kCount);
  const sumA = new Float64Array(kCount);
  const sumB = new Float64Array(kCount);
  const sumZ1W = new Float64Array(kCount);
  const sumZ2W = new Float64Array(kCount);
  const sumZ3W = new Float64Array(kCount);

  for (let iter = 0; iter < 8; iter++) {
    sumW.fill(0);
    sumAreaW.fill(0);
    sumL.fill(0);
    sumA.fill(0);
    sumB.fill(0);
    sumZ1W.fill(0);
    sumZ2W.fill(0);
    sumZ3W.fill(0);

    for (let i = 0; i < cells.length; i++) {
      const cell = cells[i];
      let bestK = 0;
      let bestD2 = Infinity;
      for (let k = 0; k < kCount; k++) {
        const ck = centroids[k];
        const dL = cell.L - ck.L;
        const da = cell.a - ck.a;
        const db = cell.b - ck.b;
        const d2 = dL * dL + da * da + db * db;
        if (d2 < bestD2) {
          bestD2 = d2;
          bestK = k;
        }
      }
      assign[i] = bestK;
      sumW[bestK] += cell.w;
      sumAreaW[bestK] += cell.areaW;
      sumL[bestK] += cell.w * cell.L;
      sumA[bestK] += cell.w * cell.a;
      sumB[bestK] += cell.w * cell.b;
      sumZ1W[bestK] += cell.z1W;
      sumZ2W[bestK] += cell.z2W;
      sumZ3W[bestK] += cell.z3W;
    }

    for (let k = 0; k < kCount; k++) {
      if (sumW[k] > 1e-7) {
        centroids[k] = {
          L: sumL[k] / sumW[k],
          a: sumA[k] / sumW[k],
          b: sumB[k] / sumW[k],
        };
      }
    }
  }

  const toCluster = (
    L: number,
    a: number,
    b: number,
    wVal: number,
    areaVal: number,
    z1Val: number,
    z2Val: number,
    z3Val: number
  ): ColorCluster => {
    const rgb = oklabToRgb(L, a, b);
    const hsv = rgbToHsv(rgb.r, rgb.g, rgb.b);
    const C = Math.hypot(a, b);
    const hue =
      C < 1e-5
        ? hsv.h
        : normalizeHueDeg((Math.atan2(b, a) * 180) / Math.PI);
    return {
      L,
      a,
      b,
      w: wVal,
      areaW: areaVal,
      z1W: z1Val,
      z2W: z2Val,
      z3W: z3Val,
      C,
      hue,
      hsv,
      hex: rgbToHex(rgb.r, rgb.g, rgb.b),
    };
  };

  const clusters: ColorCluster[] = [];
  for (let k = 0; k < kCount; k++) {
    if (sumW[k] <= 1e-6) continue;
    clusters.push(
      toCluster(
        sumL[k] / sumW[k],
        sumA[k] / sumW[k],
        sumB[k] / sumW[k],
        sumW[k],
        sumAreaW[k],
        sumZ1W[k],
        sumZ2W[k],
        sumZ3W[k]
      )
    );
  }

  let mergedAny = true;
  while (mergedAny && clusters.length > 1) {
    mergedAny = false;
    let bestI = -1;
    let bestJ = -1;
    let minD = 0.055;
    for (let i = 0; i < clusters.length; i++) {
      for (let j = i + 1; j < clusters.length; j++) {
        const d = oklabDist(clusters[i], clusters[j]);
        if (d < minD) {
          minD = d;
          bestI = i;
          bestJ = j;
        }
      }
    }
    if (bestI >= 0 && bestJ >= 0) {
      const ci = clusters[bestI];
      const cj = clusters[bestJ];
      const nw = ci.w + cj.w;
      const merged = toCluster(
        (ci.L * ci.w + cj.L * cj.w) / nw,
        (ci.a * ci.w + cj.a * cj.w) / nw,
        (ci.b * ci.w + cj.b * cj.w) / nw,
        nw,
        ci.areaW + cj.areaW,
        ci.z1W + cj.z1W,
        ci.z2W + cj.z2W,
        ci.z3W + cj.z3W
      );
      clusters.splice(bestJ, 1);
      clusters[bestI] = merged;
      mergedAny = true;
    }
  }

  if (clusters.length === 0) {
    return { ...MONOCHROME_COVER_EXTRACTED_PALETTE };
  }

  const hasRichColor = chromaticRatio >= 0.055;
  const rankPrimaryScore = (c: ColorCluster): number => {
    const chromaFactor = 0.45 + 1.15 * clamp(c.C / 0.14, 0.12, 1.35);
    const toneFactor = c.L < 0.17 || c.L > 0.92 ? 0.42 : 1.0;
    const grayDamp = hasRichColor && c.C < 0.022 ? 0.25 : 1.0;
    return c.w * chromaFactor * toneFactor * grayDamp;
  };

  clusters.sort((a, b) => rankPrimaryScore(b) - rankPrimaryScore(a));
  const c1 = clusters[0];
  const domHue = normalizeHueDeg(c1.hue);
  const domWarm = isWarmHueDeg(domHue);

  let c2: ColorCluster | null = null;
  let bestSecScore = -1;
  for (let i = 1; i < clusters.length; i++) {
    const cand = clusters[i];
    const hasSupport =
      cand.w >= c1.w * 0.08 || cand.areaW >= totalFgAreaW * 0.045;
    if (!hasSupport) continue;

    const de = oklabDist(cand, c1);
    const dh = circularHueDistance(cand.hue, c1.hue);
    const dL = Math.abs(cand.L - c1.L);
    if (de < 0.12 || (dh < 24 && dL < 0.16)) continue;

    const colorFactor = cand.C >= 0.042 ? 1.18 : 0.65;
    const sepFactor = de >= 0.19 ? 1.18 : 1.0;
    const toneFactor =
      cand.L < 0.18 && cand.C < 0.035
        ? 0.35
        : cand.L < 0.15 || cand.L > 0.93
        ? 0.52
        : 1.0;
    const score = cand.w * colorFactor * sepFactor * toneFactor;
    if (score > bestSecScore) {
      bestSecScore = score;
      c2 = cand;
    }
  }

  let rawSecondaryHex: string;
  if (c2) {
    rawSecondaryHex = c2.hex;
  } else {
    const shift =
      domHue >= 0 && domHue < 65
        ? 24
        : domHue >= 65 && domHue < 165
        ? 28
        : domHue >= 165 && domHue < 265
        ? -24
        : domHue >= 265 && domHue < 318
        ? -22
        : 22;
    const secHue = normalizeHueDeg(domHue + shift);
    const secL = c1.L > 0.68 ? c1.L - 0.14 : Math.min(0.84, c1.L + 0.14);
    rawSecondaryHex = oklchToHex(
      clamp(secL, 0.32, 0.84),
      clamp(c1.C * 0.92, 0.04, 0.24),
      secHue
    );
  }

  let c3: ColorCluster | null = null;
  let bestTerScore = -1;
  for (let i = 1; i < clusters.length; i++) {
    const cand = clusters[i];
    if (cand === c2) continue;
    if (cand.w < c1.w * 0.05) continue;
    const de1 = oklabDist(cand, c1);
    const de2 = c2 ? oklabDist(cand, c2) : 0.3;
    if (de1 < 0.12 || de2 < 0.12) continue;
    const score = cand.w * (cand.C >= 0.04 ? 1.1 : 0.75);
    if (score > bestTerScore) {
      bestTerScore = score;
      c3 = cand;
    }
  }
  const tertiaryHex = c3
    ? c3.hex
    : lerpOklabHex(c1.hex, rawSecondaryHex, 0.52);

  const pickZoneWinnerHex = (
    zoneKey: "z1W" | "z2W" | "z3W",
    fallbackHex: string
  ): string => {
    let bestCluster: ColorCluster | null = null;
    let bestZoneScore = -1;
    for (let i = 0; i < clusters.length; i++) {
      const c = clusters[i];
      const zScore = c[zoneKey] * (c.C >= 0.03 ? 1.15 : 0.75);
      if (zScore > bestZoneScore) {
        bestZoneScore = zScore;
        bestCluster = c;
      }
    }
    if (!bestCluster) return fallbackHex;
    return lerpOklabHex(bestCluster.hex, fallbackHex, 0.22);
  };

  const meshSeed1 = pickZoneWinnerHex("z1W", c1.hex);
  const meshSeed2 = pickZoneWinnerHex("z2W", rawSecondaryHex);
  const meshSeed3 = pickZoneWinnerHex("z3W", tertiaryHex);

  return deriveFullCoverPalette({
    meshPrimary: meshSeed1,
    meshSecondary: meshSeed2,
    meshTertiary: meshSeed3,
    dominantAccent: c1.hex,
    secondaryAccent: rawSecondaryHex,
    tertiaryAccent: tertiaryHex,
    quaternaryAccent: lerpOklabHex(c1.hex, rawSecondaryHex, 0.5),
    rawDominantAccent: c1.hex,
    rawSecondaryAccent: rawSecondaryHex,
    isMonochrome: false,
    isWarmDominated: domWarm,
    dominantHue: Math.round(domHue),
  });
}

export async function extractPaletteFromImageUrl(
  coverUrl?: string | null
): Promise<CoverExtractedPalette> {
  const normalizedUrl = normalizeCoverUrl(coverUrl, 480);
  if (!normalizedUrl || typeof document === "undefined") {
    return { ...DEFAULT_COVER_EXTRACTED_PALETTE };
  }

  const cached = coverPaletteCache.get(normalizedUrl);
  if (cached) {
    coverPaletteCache.delete(normalizedUrl);
    coverPaletteCache.set(normalizedUrl, cached);
    latestActivePaletteSnapshot = cached;
    return { ...cached };
  }

  const w = 80;
  const h = 80;
  const imageData = await loadCoverImageDataResilient(normalizedUrl, w, h);
  if (!imageData) {
    return { ...DEFAULT_COVER_EXTRACTED_PALETTE };
  }

  try {
    const extracted = analyzeCoverPixels(imageData, w, h);
    if (coverPaletteCache.size >= MAX_CACHE_ENTRIES) {
      const oldestKey = coverPaletteCache.keys().next().value;
      if (oldestKey) coverPaletteCache.delete(oldestKey);
    }
    coverPaletteCache.set(normalizedUrl, extracted);
    latestActivePaletteSnapshot = extracted;
    return { ...extracted };
  } catch {
    return { ...DEFAULT_COVER_EXTRACTED_PALETTE };
  }
}

export async function extractCoverMeshColors(
  coverUrl?: string | null
): Promise<CoverExtractedPalette> {
  return extractPaletteFromImageUrl(coverUrl);
}

// ============================================================================
// 7. OKLab 全调色板连续平滑插值 (`lerpPalette`)
// ============================================================================

/**
 * 【优化支柱 5】基于 OKLab 感知均匀空间的调色板全令牌插值 (`lerpPalette`)
 * ----------------------------------------------------------------------------
 * - 对全部 40+ 个 Hex 与 RGBA 色彩令牌直接在 OKLab 空间内逐一插值；
 * - 彻底摒弃旧版在切歌途中重新 `deriveFullCoverPalette` 造成 `isMonochrome` 于 `t=0.5`
 *   跳变闪烁以及 sRGB 线性混色过灰带的问题；
 * - 配合 `appStore.ts` (~650ms cubic-out) 与 `NowPlayingView.vue` (~680ms 视觉稳定)
 *   呈现丝滑流动的换曲光影过渡。
 */
export function lerpPalette(
  from: CoverExtractedPalette,
  to: CoverExtractedPalette,
  t: number
): CoverExtractedPalette {
  const f = clamp(t, 0, 1);
  const fullFrom = ensureFullCoverPalette(from);
  const fullTo = ensureFullCoverPalette(to);

  if (f <= 0) {
    latestActivePaletteSnapshot = fullFrom;
    return { ...fullFrom };
  }
  if (f >= 1) {
    latestActivePaletteSnapshot = fullTo;
    return { ...fullTo };
  }

  const interpolated: CoverExtractedPalette = {
    meshPrimary: lerpOklabHex(fullFrom.meshPrimary, fullTo.meshPrimary, f),
    meshSecondary: lerpOklabHex(
      fullFrom.meshSecondary,
      fullTo.meshSecondary,
      f
    ),
    meshTertiary: lerpOklabHex(fullFrom.meshTertiary, fullTo.meshTertiary, f),
    dominantAccent: lerpOklabHex(
      fullFrom.dominantAccent,
      fullTo.dominantAccent,
      f
    ),
    secondaryAccent: lerpOklabHex(
      fullFrom.secondaryAccent,
      fullTo.secondaryAccent,
      f
    ),
    tertiaryAccent: lerpOklabHex(
      fullFrom.tertiaryAccent,
      fullTo.tertiaryAccent,
      f
    ),
    quaternaryAccent: lerpOklabHex(
      fullFrom.quaternaryAccent,
      fullTo.quaternaryAccent,
      f
    ),
    rawDominantAccent: lerpOklabHex(
      fullFrom.rawDominantAccent,
      fullTo.rawDominantAccent,
      f
    ),
    rawSecondaryAccent: lerpOklabHex(
      fullFrom.rawSecondaryAccent,
      fullTo.rawSecondaryAccent,
      f
    ),
    onDominantAccent: lerpOklabHex(
      fullFrom.onDominantAccent,
      fullTo.onDominantAccent,
      f
    ),
    onSecondaryAccent: lerpOklabHex(
      fullFrom.onSecondaryAccent,
      fullTo.onSecondaryAccent,
      f
    ),
    accentGlowRgba: lerpRgbaOrHexToken(
      fullFrom.accentGlowRgba,
      fullTo.accentGlowRgba,
      f
    ),
    accentSubtleRgba: lerpRgbaOrHexToken(
      fullFrom.accentSubtleRgba,
      fullTo.accentSubtleRgba,
      f
    ),
    accentSecondaryGlowRgba: lerpRgbaOrHexToken(
      fullFrom.accentSecondaryGlowRgba,
      fullTo.accentSecondaryGlowRgba,
      f
    ),
    accentSecondarySubtleRgba: lerpRgbaOrHexToken(
      fullFrom.accentSecondarySubtleRgba,
      fullTo.accentSecondarySubtleRgba,
      f
    ),
    bgPrimary: lerpOklabHex(fullFrom.bgPrimary, fullTo.bgPrimary, f),
    bgSecondary: lerpOklabHex(fullFrom.bgSecondary, fullTo.bgSecondary, f),
    bgTertiary: lerpOklabHex(fullFrom.bgTertiary, fullTo.bgTertiary, f),
    bgAbyss: lerpOklabHex(fullFrom.bgAbyss, fullTo.bgAbyss, f),
    canvasPrimary: lerpOklabHex(
      fullFrom.canvasPrimary,
      fullTo.canvasPrimary,
      f
    ),
    canvasSecondary: lerpOklabHex(
      fullFrom.canvasSecondary,
      fullTo.canvasSecondary,
      f
    ),
    canvasAccentGlowRgba: lerpRgbaOrHexToken(
      fullFrom.canvasAccentGlowRgba,
      fullTo.canvasAccentGlowRgba,
      f
    ),
    canvasVignetteRgba: lerpRgbaOrHexToken(
      fullFrom.canvasVignetteRgba,
      fullTo.canvasVignetteRgba,
      f
    ),
    fluidOrb1: lerpOklabHex(fullFrom.fluidOrb1, fullTo.fluidOrb1, f),
    fluidOrb2: lerpOklabHex(fullFrom.fluidOrb2, fullTo.fluidOrb2, f),
    fluidOrb3: lerpOklabHex(fullFrom.fluidOrb3, fullTo.fluidOrb3, f),
    fluidOrb4: lerpOklabHex(fullFrom.fluidOrb4, fullTo.fluidOrb4, f),
    fluidBlendGlowRgba: lerpRgbaOrHexToken(
      fullFrom.fluidBlendGlowRgba,
      fullTo.fluidBlendGlowRgba,
      f
    ),
    lyricActiveHex: lerpOklabHex(
      fullFrom.lyricActiveHex,
      fullTo.lyricActiveHex,
      f
    ),
    lyricSecondaryHex: lerpOklabHex(
      fullFrom.lyricSecondaryHex,
      fullTo.lyricSecondaryHex,
      f
    ),
    lyricGlowRgba: lerpRgbaOrHexToken(
      fullFrom.lyricGlowRgba,
      fullTo.lyricGlowRgba,
      f
    ),
    lyricInactiveRgba: lerpRgbaOrHexToken(
      fullFrom.lyricInactiveRgba,
      fullTo.lyricInactiveRgba,
      f
    ),
    lyricTranslationRgba: lerpRgbaOrHexToken(
      fullFrom.lyricTranslationRgba,
      fullTo.lyricTranslationRgba,
      f
    ),
    soundwavePrimary: lerpOklabHex(
      fullFrom.soundwavePrimary,
      fullTo.soundwavePrimary,
      f
    ),
    soundwaveSecondary: lerpOklabHex(
      fullFrom.soundwaveSecondary,
      fullTo.soundwaveSecondary,
      f
    ),
    soundwaveTertiary: lerpOklabHex(
      fullFrom.soundwaveTertiary,
      fullTo.soundwaveTertiary,
      f
    ),
    soundwaveCoreGlowRgba: lerpRgbaOrHexToken(
      fullFrom.soundwaveCoreGlowRgba,
      fullTo.soundwaveCoreGlowRgba,
      f
    ),
    soundwaveRingStrokeRgba: lerpRgbaOrHexToken(
      fullFrom.soundwaveRingStrokeRgba,
      fullTo.soundwaveRingStrokeRgba,
      f
    ),
    starlightPrimary: lerpOklabHex(
      fullFrom.starlightPrimary,
      fullTo.starlightPrimary,
      f
    ),
    starlightSecondary: lerpOklabHex(
      fullFrom.starlightSecondary,
      fullTo.starlightSecondary,
      f
    ),
    starlightTertiary: lerpOklabHex(
      fullFrom.starlightTertiary,
      fullTo.starlightTertiary,
      f
    ),
    starlightWarmGold: lerpOklabHex(
      fullFrom.starlightWarmGold,
      fullTo.starlightWarmGold,
      f
    ),
    nebulaGlow: lerpOklabHex(fullFrom.nebulaGlow, fullTo.nebulaGlow, f),
    nebulaGlowRgba: lerpRgbaOrHexToken(
      fullFrom.nebulaGlowRgba,
      fullTo.nebulaGlowRgba,
      f
    ),
    nebulaSecondaryGlowRgba: lerpRgbaOrHexToken(
      fullFrom.nebulaSecondaryGlowRgba,
      fullTo.nebulaSecondaryGlowRgba,
      f
    ),
    cardSurfacePrimary: lerpOklabHex(
      fullFrom.cardSurfacePrimary,
      fullTo.cardSurfacePrimary,
      f
    ),
    cardSurfaceElevated: lerpOklabHex(
      fullFrom.cardSurfaceElevated,
      fullTo.cardSurfaceElevated,
      f
    ),
    cardSurfaceLowest: lerpOklabHex(
      fullFrom.cardSurfaceLowest,
      fullTo.cardSurfaceLowest,
      f
    ),
    cardSurfaceGlassRgba: lerpRgbaOrHexToken(
      fullFrom.cardSurfaceGlassRgba,
      fullTo.cardSurfaceGlassRgba,
      f
    ),
    cardBorderHighlightRgba: lerpRgbaOrHexToken(
      fullFrom.cardBorderHighlightRgba,
      fullTo.cardBorderHighlightRgba,
      f
    ),
    cardBorderSubtleRgba: lerpRgbaOrHexToken(
      fullFrom.cardBorderSubtleRgba,
      fullTo.cardBorderSubtleRgba,
      f
    ),
    cardHoverBgRgba: lerpRgbaOrHexToken(
      fullFrom.cardHoverBgRgba,
      fullTo.cardHoverBgRgba,
      f
    ),
    cardActiveBgRgba: lerpRgbaOrHexToken(
      fullFrom.cardActiveBgRgba,
      fullTo.cardActiveBgRgba,
      f
    ),
    cardAmbientGlowRgba: lerpRgbaOrHexToken(
      fullFrom.cardAmbientGlowRgba,
      fullTo.cardAmbientGlowRgba,
      f
    ),
    prismCyan: lerpOklabHex(fullFrom.prismCyan, fullTo.prismCyan, f),
    prismMagenta: lerpOklabHex(fullFrom.prismMagenta, fullTo.prismMagenta, f),
    prismViolet: lerpOklabHex(fullFrom.prismViolet, fullTo.prismViolet, f),
    prismGold: lerpOklabHex(fullFrom.prismGold, fullTo.prismGold, f),
    dominantHue: Math.round(
      lerpCircularHue(fullFrom.dominantHue, fullTo.dominantHue, f)
    ),
    isMonochrome: f < 0.5 ? fullFrom.isMonochrome : fullTo.isMonochrome,
    isWarmDominated:
      f < 0.5 ? fullFrom.isWarmDominated : fullTo.isWarmDominated,
    isLightStage: f < 0.5 ? fullFrom.isLightStage : fullTo.isLightStage,
    inkPrimary:
      fullFrom.inkPrimary || fullTo.inkPrimary
        ? lerpOklabHex(
            fullFrom.inkPrimary || (fullFrom.isLightStage ? "#18181B" : "#EEF2F6"),
            fullTo.inkPrimary || (fullTo.isLightStage ? "#18181B" : "#EEF2F6"),
            f
          )
        : undefined,
    inkSecondary:
      fullFrom.inkSecondary || fullTo.inkSecondary
        ? lerpRgbaOrHexToken(
            fullFrom.inkSecondary || (fullFrom.isLightStage ? "#475569" : "rgba(226, 234, 246, 0.72)"),
            fullTo.inkSecondary || (fullTo.isLightStage ? "#475569" : "rgba(226, 234, 246, 0.72)"),
            f
          )
        : undefined,
    inkMuted:
      fullFrom.inkMuted || fullTo.inkMuted
        ? lerpRgbaOrHexToken(
            fullFrom.inkMuted || (fullFrom.isLightStage ? "#64748B" : "rgba(238, 243, 250, 0.44)"),
            fullTo.inkMuted || (fullTo.isLightStage ? "#64748B" : "rgba(238, 243, 250, 0.44)"),
            f
          )
        : undefined,
  };

  latestActivePaletteSnapshot = interpolated;
  return interpolated;
}

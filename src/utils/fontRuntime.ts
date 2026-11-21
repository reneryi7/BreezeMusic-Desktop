/**
 * BreezeMusic 全局与多窗口字体运行时加载引擎
 * =========================================================================
 * - 统一管理「全局界面 / 播放页歌词 / 桌面悬浮歌词」三作用域字体堆栈与字重；
 * - 通过 FontFace API 将 Rust 托管的本地导入字体安全注入当前 WebView2 文档；
 * - 通过 localStorage + BroadcastChannel 实现主窗口、桌面歌词、迷你播放器即时同步，
 *   并在 index.html 首屏内联脚本中实现冷启动零闪烁恢复。
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  AppTypographyConfig,
  CustomFontBinaryPayload,
  CustomFontRecord,
  SystemFontFamilyInfo,
  TypographyDesktopLyricMode,
  TypographyLatinFallbackId,
  TypographyStageLyricMode,
} from "../types";

export const TYPOGRAPHY_STORAGE_KEY = "bm_typography_config_v1";
export const TYPOGRAPHY_SYNC_CHANNEL = "bm_typography_sync_v1";

export const DEFAULT_UI_FONT_STACK =
  '"Segoe UI Variable Display", "Segoe UI", "PingFang SC", "Microsoft YaHei UI", sans-serif';

export const DEFAULT_TYPOGRAPHY_CONFIG: AppTypographyConfig = {
  uiFontFamily: "",
  uiCustomFontId: "",
  uiFontDisplayName: "系统默认",
  uiBaseWeight: 400,
  uiFontScale: 1.0,
  uiLineHeight: 1.48,
  latinFallback: "auto",
  tabularNumbers: true,

  stageLyricMode: "preset",
  stageLyricFontFamily: "",
  stageLyricCustomFontId: "",
  stageLyricFontDisplayName: "",
  stageLyricWeight: 700,

  desktopLyricMode: "preset",
  desktopLyricFontFamily: "",
  desktopLyricCustomFontId: "",
  desktopLyricFontDisplayName: "",
  desktopLyricWeight: 700,
};

/** 仅在纯浏览器预览模式（非 Tauri 宿主）下使用的 Windows 常见内置字体列表 */
export const BROWSER_PREVIEW_FALLBACK_FONTS: SystemFontFamilyInfo[] = [
  {
    familyName: "Microsoft YaHei UI",
    displayName: "微软雅黑 UI",
    weights: [300, 400, 700],
    isVariable: false,
    weightMin: 300,
    weightMax: 700,
    supportsCjk: true,
    isMonospace: false,
    fileCount: 3,
    source: "system",
  },
  {
    familyName: "Microsoft YaHei",
    displayName: "微软雅黑",
    weights: [300, 400, 700],
    isVariable: false,
    weightMin: 300,
    weightMax: 700,
    supportsCjk: true,
    isMonospace: false,
    fileCount: 3,
    source: "system",
  },
  {
    familyName: "DengXian",
    displayName: "等线",
    weights: [300, 400, 700],
    isVariable: false,
    weightMin: 300,
    weightMax: 700,
    supportsCjk: true,
    isMonospace: false,
    fileCount: 3,
    source: "system",
  },
  {
    familyName: "SimSun",
    displayName: "宋体",
    weights: [400],
    isVariable: false,
    weightMin: 400,
    weightMax: 400,
    supportsCjk: true,
    isMonospace: false,
    fileCount: 1,
    source: "system",
  },
  {
    familyName: "KaiTi",
    displayName: "楷体",
    weights: [400],
    isVariable: false,
    weightMin: 400,
    weightMax: 400,
    supportsCjk: true,
    isMonospace: false,
    fileCount: 1,
    source: "system",
  },
  {
    familyName: "FangSong",
    displayName: "仿宋",
    weights: [400],
    isVariable: false,
    weightMin: 400,
    weightMax: 400,
    supportsCjk: true,
    isMonospace: false,
    fileCount: 1,
    source: "system",
  },
  {
    familyName: "SimHei",
    displayName: "黑体",
    weights: [400],
    isVariable: false,
    weightMin: 400,
    weightMax: 400,
    supportsCjk: true,
    isMonospace: false,
    fileCount: 1,
    source: "system",
  },
  {
    familyName: "Segoe UI Variable Display",
    displayName: "Segoe UI Variable Display",
    weights: [300, 400, 600, 700],
    isVariable: true,
    weightMin: 100,
    weightMax: 900,
    supportsCjk: false,
    isMonospace: false,
    fileCount: 1,
    source: "system",
  },
  {
    familyName: "Consolas",
    displayName: "Consolas",
    weights: [400, 700],
    isVariable: false,
    weightMin: 400,
    weightMax: 700,
    supportsCjk: false,
    isMonospace: true,
    fileCount: 4,
    source: "system",
  },
];

export function quoteFontFamilyName(raw: string): string {
  const cleaned = (raw || "").replace(/["\\;\r\n]/g, "").trim();
  if (!cleaned) return "";
  return `"${cleaned}"`;
}

function normalizeLatinFallback(raw: unknown): TypographyLatinFallbackId {
  if (
    raw === "segoe" ||
    raw === "bahnschrift" ||
    raw === "inter" ||
    raw === "mono"
  ) {
    return raw;
  }
  return "auto";
}

function normalizeStageMode(raw: unknown): TypographyStageLyricMode {
  if (raw === "follow_ui" || raw === "custom") return raw;
  return "preset";
}

function normalizeDesktopMode(raw: unknown): TypographyDesktopLyricMode {
  if (raw === "follow_stage" || raw === "follow_ui" || raw === "custom") return raw;
  return "preset";
}

export function clampFontWeight(val: unknown, fallback = 400): number {
  const n = Number(val);
  if (!Number.isFinite(n)) return fallback;
  return Math.round(Math.min(900, Math.max(100, n)));
}

export function clampUiFontScale(val: unknown): number {
  const n = Number(val);
  if (!Number.isFinite(n)) return 1.0;
  return Number(Math.min(1.45, Math.max(0.75, n)).toFixed(2));
}

export function clampUiLineHeight(val: unknown): number {
  const n = Number(val);
  if (!Number.isFinite(n)) return 1.48;
  return Number(Math.min(1.8, Math.max(1.25, n)).toFixed(2));
}

/** 计算全软件 5 级派生字重矩阵，使基础字重调整能真实带动正文/标签/副标题/标题整体视觉粗细 */
export function computeDerivedFontWeights(baseWeight: number): {
  regular: number;
  medium: number;
  semibold: number;
  bold: number;
  heavy: number;
} {
  const base = clampFontWeight(baseWeight, 400);
  return {
    regular: base,
    medium: clampFontWeight(base + 100, 500),
    semibold: clampFontWeight(base + 200, 600),
    bold: clampFontWeight(base + 300, 700),
    heavy: clampFontWeight(base + 400, 800),
  };
}

export function normalizeTypographyConfig(
  raw?: Partial<AppTypographyConfig> | null
): AppTypographyConfig {
  const src = raw && typeof raw === "object" ? raw : {};
  return {
    uiFontFamily: typeof src.uiFontFamily === "string" ? src.uiFontFamily.trim() : "",
    uiCustomFontId: typeof src.uiCustomFontId === "string" ? src.uiCustomFontId.trim() : "",
    uiFontDisplayName:
      typeof src.uiFontDisplayName === "string" && src.uiFontDisplayName.trim()
        ? src.uiFontDisplayName.trim()
        : typeof src.uiFontFamily === "string" && src.uiFontFamily.trim()
        ? src.uiFontFamily.trim()
        : "系统默认",
    uiBaseWeight: clampFontWeight(src.uiBaseWeight, 400),
    uiFontScale: clampUiFontScale(src.uiFontScale),
    uiLineHeight: clampUiLineHeight(src.uiLineHeight),
    latinFallback: normalizeLatinFallback(src.latinFallback),
    tabularNumbers: src.tabularNumbers !== false,

    stageLyricMode: normalizeStageMode(src.stageLyricMode),
    stageLyricFontFamily:
      typeof src.stageLyricFontFamily === "string" ? src.stageLyricFontFamily.trim() : "",
    stageLyricCustomFontId:
      typeof src.stageLyricCustomFontId === "string" ? src.stageLyricCustomFontId.trim() : "",
    stageLyricFontDisplayName:
      typeof src.stageLyricFontDisplayName === "string"
        ? src.stageLyricFontDisplayName.trim()
        : "",
    stageLyricWeight: clampFontWeight(src.stageLyricWeight, 700),

    desktopLyricMode: normalizeDesktopMode(src.desktopLyricMode),
    desktopLyricFontFamily:
      typeof src.desktopLyricFontFamily === "string" ? src.desktopLyricFontFamily.trim() : "",
    desktopLyricCustomFontId:
      typeof src.desktopLyricCustomFontId === "string"
        ? src.desktopLyricCustomFontId.trim()
        : "",
    desktopLyricFontDisplayName:
      typeof src.desktopLyricFontDisplayName === "string"
        ? src.desktopLyricFontDisplayName.trim()
        : "",
    desktopLyricWeight: clampFontWeight(src.desktopLyricWeight, 700),
  };
}

export function loadPersistedTypographyConfig(): AppTypographyConfig {
  try {
    const raw = localStorage.getItem(TYPOGRAPHY_STORAGE_KEY);
    if (!raw) return { ...DEFAULT_TYPOGRAPHY_CONFIG };
    return normalizeTypographyConfig(JSON.parse(raw));
  } catch {
    return { ...DEFAULT_TYPOGRAPHY_CONFIG };
  }
}

export function persistTypographyConfig(cfg: AppTypographyConfig): void {
  try {
    localStorage.setItem(TYPOGRAPHY_STORAGE_KEY, JSON.stringify(cfg));
  } catch {
    // ignore storage quota errors
  }
}

/** 根据配置拼装全局 UI 字体堆栈（含西文/等宽混排策略、Windows 细体字面自动前置与中文安全兜底） */
export function buildUiFontFamilyStack(cfg: AppTypographyConfig): string {
  const mainRaw = (cfg.uiFontFamily || "").replace(/["\\;\r\n]/g, "").trim();
  const mainQuoted = quoteFontFamilyName(mainRaw);
  const baseWeight = clampFontWeight(cfg.uiBaseWeight, 400);
  const latinPrefix =
    cfg.latinFallback === "segoe"
      ? '"Segoe UI Variable Display", "Segoe UI"'
      : cfg.latinFallback === "bahnschrift"
      ? '"Bahnschrift", "Segoe UI Variable Display"'
      : cfg.latinFallback === "inter"
      ? '"Inter", "Segoe UI Variable Display"'
      : cfg.latinFallback === "mono"
      ? '"Cascadia Mono", "Consolas", ui-monospace'
      : "";

  // Windows 下微软雅黑 / 等线的 300 细体位于独立家族名中，<=350 时前置以确保细体真实可见
  let lightPrefix = "";
  if (baseWeight <= 350) {
    if (!mainRaw || mainRaw === "Microsoft YaHei UI" || mainRaw === "Microsoft YaHei") {
      lightPrefix = '"Microsoft YaHei UI Light", "Microsoft YaHei Light", "Segoe UI Light"';
    } else if (mainRaw === "DengXian") {
      lightPrefix = '"DengXian Light"';
    }
  }

  const parts: string[] = [];
  if (latinPrefix) parts.push(latinPrefix);
  if (lightPrefix) parts.push(lightPrefix);

  if (!mainQuoted) {
    if (latinPrefix || lightPrefix) {
      parts.push('"Segoe UI Variable Display"', '"Segoe UI"', '"PingFang SC"', '"Microsoft YaHei UI"', "sans-serif");
      return parts.join(", ");
    }
    return DEFAULT_UI_FONT_STACK;
  }

  parts.push(mainQuoted);
  if (!latinPrefix) {
    parts.push('"Segoe UI Variable Display"', '"Segoe UI"');
  }
  parts.push('"Microsoft YaHei UI"', "sans-serif");
  return parts.join(", ");
}

/** 解析播放页歌词实际应使用的自定义字体覆写（null 表示沿用原本 4 套预设） */
export function resolveStageLyricFontOverride(cfg: AppTypographyConfig): {
  familyStack: string | null;
  weightOverride: number | null;
  customFontId: string;
  label: string;
} {
  if (cfg.stageLyricMode === "follow_ui") {
    if (!cfg.uiFontFamily) {
      return {
        familyStack: null,
        weightOverride: null,
        customFontId: "",
        label: "跟随全局界面",
      };
    }
    return {
      familyStack: buildUiFontFamilyStack(cfg),
      weightOverride: null,
      customFontId: cfg.uiCustomFontId,
      label: cfg.uiFontDisplayName || cfg.uiFontFamily,
    };
  }
  if (cfg.stageLyricMode === "custom" && cfg.stageLyricFontFamily) {
    const q = quoteFontFamilyName(cfg.stageLyricFontFamily);
    return {
      familyStack: `${q}, "Microsoft YaHei UI", sans-serif`,
      weightOverride: clampFontWeight(cfg.stageLyricWeight, 700),
      customFontId: cfg.stageLyricCustomFontId,
      label: cfg.stageLyricFontDisplayName || cfg.stageLyricFontFamily,
    };
  }
  return {
    familyStack: null,
    weightOverride: null,
    customFontId: "",
    label: "沿用歌词预设",
  };
}

/** 解析桌面悬浮歌词实际应使用的自定义字体覆写 */
export function resolveDesktopLyricFontOverride(cfg: AppTypographyConfig): {
  familyStack: string | null;
  weightOverride: number | null;
  customFontId: string;
  label: string;
} {
  if (cfg.desktopLyricMode === "follow_stage") {
    return resolveStageLyricFontOverride(cfg);
  }
  if (cfg.desktopLyricMode === "follow_ui") {
    if (!cfg.uiFontFamily) {
      return {
        familyStack: null,
        weightOverride: null,
        customFontId: "",
        label: "跟随全局界面",
      };
    }
    return {
      familyStack: buildUiFontFamilyStack(cfg),
      weightOverride: null,
      customFontId: cfg.uiCustomFontId,
      label: cfg.uiFontDisplayName || cfg.uiFontFamily,
    };
  }
  if (cfg.desktopLyricMode === "custom" && cfg.desktopLyricFontFamily) {
    const q = quoteFontFamilyName(cfg.desktopLyricFontFamily);
    return {
      familyStack: `${q}, "Microsoft YaHei UI", sans-serif`,
      weightOverride: clampFontWeight(cfg.desktopLyricWeight, 700),
      customFontId: cfg.desktopLyricCustomFontId,
      label: cfg.desktopLyricFontDisplayName || cfg.desktopLyricFontFamily,
    };
  }
  return {
    familyStack: null,
    weightOverride: null,
    customFontId: "",
    label: "沿用歌词预设",
  };
}

const loadedFontFaces = new Map<string, FontFace>();
const loadingFontPromises = new Map<string, Promise<boolean>>();

function base64ToArrayBuffer(b64: string): ArrayBuffer {
  const binary = atob(b64);
  const len = binary.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

/** 将指定的已导入自定义字体通过 FontFace API 注入当前文档（带并发去重与缓存） */
export async function ensureCustomFontLoaded(fontId: string): Promise<boolean> {
  const id = (fontId || "").trim();
  if (!id || typeof document === "undefined" || typeof FontFace === "undefined") {
    return false;
  }
  if (loadedFontFaces.has(id)) {
    return true;
  }
  const existing = loadingFontPromises.get(id);
  if (existing) {
    return existing;
  }

  const job = (async () => {
    try {
      const payload = await invoke<CustomFontBinaryPayload>(
        "read_custom_font_binary_cmd",
        { fontId: id }
      );
      const buf = base64ToArrayBuffer(payload.base64Data);
      const descriptors: FontFaceDescriptors = payload.isVariable
        ? {
            weight: `${payload.weightMin || 100} ${payload.weightMax || 900}`,
            style: "normal",
            display: "swap",
          }
        : {
            style: "normal",
            display: "swap",
          };
      const face = new FontFace(payload.cssFamilyName, buf, descriptors);
      await face.load();
      document.fonts.add(face);
      loadedFontFaces.set(id, face);
      return true;
    } catch {
      return false;
    } finally {
      loadingFontPromises.delete(id);
    }
  })();

  loadingFontPromises.set(id, job);
  return job;
}

/** 批量预热全部已导入的自定义字体（用于字体设置列表即时以真实字形预览） */
export async function ensureAllCustomFontsLoaded(
  records: CustomFontRecord[]
): Promise<void> {
  if (!records || records.length === 0) return;
  await Promise.allSettled(records.map((r) => ensureCustomFontLoaded(r.id)));
}

/** 卸载并释放指定自定义字体 */
export function unloadCustomFontFace(fontId: string): void {
  const face = loadedFontFaces.get(fontId);
  if (face && typeof document !== "undefined") {
    try {
      document.fonts.delete(face);
    } catch {
      // ignore
    }
  }
  loadedFontFaces.delete(fontId);
}

/** 将排版配置实时应用到当前窗口的 :root CSS 变量与数据属性（含 5 级派生字重与全局行距） */
export function applyTypographyToDocument(cfg: AppTypographyConfig): void {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  const stack = buildUiFontFamilyStack(cfg);
  const derived = computeDerivedFontWeights(cfg.uiBaseWeight);
  root.style.setProperty("--bm-font-ui", stack);
  root.style.setProperty("--bm-ui-font-scale", String(clampUiFontScale(cfg.uiFontScale)));
  root.style.setProperty("--bm-ui-font-weight", String(derived.regular));
  root.style.setProperty("--bm-fw-regular", String(derived.regular));
  root.style.setProperty("--bm-fw-medium", String(derived.medium));
  root.style.setProperty("--bm-fw-semibold", String(derived.semibold));
  root.style.setProperty("--bm-fw-bold", String(derived.bold));
  root.style.setProperty("--bm-fw-heavy", String(derived.heavy));
  root.style.setProperty("--bm-ui-line-height", String(clampUiLineHeight(cfg.uiLineHeight)));
  root.dataset.tabularNums = cfg.tabularNumbers ? "true" : "false";
  root.dataset.mixedFonts = cfg.latinFallback !== "auto" ? "true" : "false";
}

/** 自动预加载配置中引用到的全部自定义字体并应用到当前文档 */
export async function hydrateAndApplyTypography(cfg: AppTypographyConfig): Promise<void> {
  const ids = new Set<string>();
  if (cfg.uiCustomFontId) ids.add(cfg.uiCustomFontId);
  if (cfg.stageLyricCustomFontId) ids.add(cfg.stageLyricCustomFontId);
  if (cfg.desktopLyricCustomFontId) ids.add(cfg.desktopLyricCustomFontId);
  if (ids.size > 0) {
    await Promise.allSettled(Array.from(ids).map((id) => ensureCustomFontLoaded(id)));
  }
  applyTypographyToDocument(cfg);
}

/** 通过 BroadcastChannel 广播最新字体配置给同源独立子窗口（桌面歌词、迷你播放器） */
export function broadcastTypographyConfig(cfg: AppTypographyConfig): void {
  if (typeof BroadcastChannel === "undefined") return;
  try {
    const ch = new BroadcastChannel(TYPOGRAPHY_SYNC_CHANNEL);
    ch.postMessage({ type: "TYPOGRAPHY_UPDATED", config: cfg });
    ch.close();
  } catch {
    // ignore
  }
}

export function formatWeightLabel(w: number): string {
  if (w <= 150) return "极细 100";
  if (w <= 250) return "特细 200";
  if (w <= 350) return "细体 300";
  if (w <= 450) return "常规 400";
  if (w <= 550) return "中等 500";
  if (w <= 650) return "半粗 600";
  if (w <= 750) return "粗体 700";
  if (w <= 850) return "特粗 800";
  return "黑体 900";
}

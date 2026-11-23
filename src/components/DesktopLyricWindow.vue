<script setup lang="ts">
import {
  ref,
  computed,
  watch,
  nextTick,
  onMounted,
  onBeforeUnmount,
} from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import PlumpIcon from "./PlumpIcon.vue";
import type {
  DesktopLyricLayoutMode,
  DesktopLyricStrokeColorMode,
  DesktopLyricStrokeWidth,
  LyricFontFamily,
  LyricFontWeightLevel,
  LyricHighlightThemeId,
  LyricSungColorMode,
  LyricUnsungPresetId,
} from "../types";
import {
  DESKTOP_STROKE_COLOR_OPTIONS,
  DESKTOP_STROKE_WIDTH_OPTIONS,
  LYRIC_FONT_FAMILY_OPTIONS,
  LYRIC_FONT_WEIGHT_OPTIONS,
  LYRIC_SUNG_PRESETS,
  LYRIC_UNSUNG_PRESETS,
  normalizeLyricThemeId,
  normalizeUnsungPresetId,
  resolveDesktopStrokeStyle,
  resolveLyricTypographyStyle,
  resolveSungLyricPalette,
  resolveUnsungLyricHex,
} from "../utils/lyricTypography";
import {
  createBrowserDevShowcase,
  isBrowserDevPreviewEnv,
} from "../utils/devPreviewShowcase";
import {
  TYPOGRAPHY_SYNC_CHANNEL,
  ensureCustomFontLoaded,
  hydrateAndApplyTypography,
  loadPersistedTypographyConfig,
  normalizeTypographyConfig,
  resolveDesktopLyricFontOverride,
} from "../utils/fontRuntime";

interface LyricWordItem {
  text: string;
  startTimeMs: number;
  durationMs: number;
}

interface LyricLineItem {
  startTimeMs: number;
  durationMs: number;
  text: string;
  translation?: string | null;
  phonetic?: string | null;
  words?: LyricWordItem[];
}

interface DisplayRow {
  key: string;
  text: string;
  words: LyricWordItem[];
  lineStartMs: number;
  lineDurationMs: number;
  isActiveKaraoke: boolean;
  isSecondary: boolean;
  isPreviewNext?: boolean;
  verticalPct: number;
  alignOverride?: "left" | "center";
  fitScale: number;
}

type SettingsPodTab = "color" | "typography" | "layout";

const CHANNEL_NAME = "bm_desktop_lyric_channel";
const SNAPSHOT_STORAGE_KEY = "bm_desktop_lyric_snapshot_v2";
const POS_STORAGE_KEY = "bm_desktop_lyric_pos_v2";
const TOOLBAR_HEIGHT_PX = 40;
const DIVIDER_HEIGHT_PX = 2;
const PROGRESS_HEIGHT_PX = 4;
const SHELL_VPAD_PX = 28;
const MIN_WIDTH_PX = 440;
const MAX_WIDTH_PX = 1280;

// 立即声明透明窗体根标记，杜绝首帧出现 tokens-base.css 默认深色底色
if (typeof document !== "undefined") {
  document.documentElement.classList.add("is-desktop-lyric-win");
  document.body.classList.add("is-desktop-lyric-win");
  document.documentElement.style.background = "transparent";
  document.body.style.background = "transparent";
}

function clamp(val: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, val));
}

function readStoredNumber(
  key: string,
  fallback: number,
  min: number,
  max: number
): number {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null) return fallback;
    const n = Number(raw);
    return Number.isFinite(n) ? clamp(n, min, max) : fallback;
  } catch {
    return fallback;
  }
}

function readStoredString(key: string, fallback: string): string {
  try {
    return localStorage.getItem(key) || fallback;
  } catch {
    return fallback;
  }
}

function readStoredWeight(
  key: string,
  fallback: LyricFontWeightLevel
): LyricFontWeightLevel {
  const n = Number(readStoredString(key, String(fallback)));
  if (n === 500 || n === 650 || n === 760 || n === 880) return n;
  return fallback;
}

function hexToRgb(hex: string): [number, number, number] {
  const clean = (hex || "#5EEAD4").replace("#", "").trim();
  const full =
    clean.length === 3
      ? clean
          .split("")
          .map((c) => c + c)
          .join("")
      : clean.padEnd(6, "0").slice(0, 6);
  const r = parseInt(full.slice(0, 2), 16) || 0;
  const g = parseInt(full.slice(2, 4), 16) || 198;
  const b = parseInt(full.slice(4, 6), 16) || 212;
  return [r, g, b];
}

function hexToRgba(hex: string, alpha: number): string {
  const [r, g, b] = hexToRgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${clamp(alpha, 0, 1).toFixed(3)})`;
}

function getRelativeLuminance(hex: string): number {
  const [r, g, b] = hexToRgb(hex).map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function isBrightHex(hex: string): boolean {
  return getRelativeLuminance(hex) > 0.09;
}

function pickContrastInk(hex: string): string {
  const [r, g, b] = hexToRgb(hex);
  const luma = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return luma > 0.66 ? "#181C24" : "#FFFFFF";
}

// ==================== 独立持久化状态（严格隔离于播放页歌词） ====================
const isWindowVisible = ref<boolean>(
  readStoredString("bm_desktop_lyric", "false") === "true"
);
const isLocked = ref<boolean>(
  readStoredString("bm_desktop_lyric_locked", "false") === "true"
);

// 已唱双通道独立状态
const colorSchemeId = ref<LyricHighlightThemeId>(
  normalizeLyricThemeId(
    readStoredString("bm_desktop_lyric_scheme", "dynamic_cover")
  )
);
const sungMode = ref<LyricSungColorMode>(
  readStoredString("bm_dl_sung_mode_v2", "gradient") === "solid"
    ? "solid"
    : "gradient"
);
const customPrimaryHex = ref<string>(
  readStoredString(
    "bm_dl_custom_primary_v2",
    readStoredString("bm_desktop_lyric_color", "#5EEAD4")
  )
);
const customSecondaryHex = ref<string>(
  readStoredString("bm_dl_custom_secondary_v2", "#60A5FA")
);
const dynamicPrimaryHex = ref<string>(
  readStoredString("bm_desktop_lyric_dyn_p", "#5EEAD4")
);
const dynamicSecondaryHex = ref<string>(
  readStoredString("bm_desktop_lyric_dyn_s", "#60A5FA")
);
const dynamicCoverAccentEnabled = ref<boolean>(true);

// 未唱独立状态
const unsungPreset = ref<LyricUnsungPresetId>(
  normalizeUnsungPresetId(
    readStoredString(
      "bm_dl_unsung_preset_v2",
      readStoredString("bm_desktop_lyric_unsung_tone", "auto")
    )
  )
);
const unsungCustomHex = ref<string>(
  readStoredString("bm_dl_unsung_custom_v2", "#E6ECF5")
);
const unsungOpacity = ref<number>(
  readStoredNumber("bm_dl_unsung_opacity_v2", 0.78, 0.35, 0.98)
);

// 真光学矢量描边 & 立体柔影状态
const rawStoredStrokeW = readStoredString("bm_dl_stroke_width_v2", "medium");
const strokeWidth = ref<DesktopLyricStrokeWidth>(
  rawStoredStrokeW === "none" ||
    rawStoredStrokeW === "thin" ||
    rawStoredStrokeW === "medium" ||
    rawStoredStrokeW === "bold"
    ? rawStoredStrokeW
    : "medium"
);
const rawStoredStrokeC = readStoredString("bm_dl_stroke_color_mode_v2", "auto");
const strokeColorMode = ref<DesktopLyricStrokeColorMode>(
  rawStoredStrokeC === "auto" ||
    rawStoredStrokeC === "black" ||
    rawStoredStrokeC === "white" ||
    rawStoredStrokeC === "custom"
    ? rawStoredStrokeC
    : "auto"
);
const strokeCustomHex = ref<string>(
  readStoredString("bm_dl_stroke_custom_v2", "#090D16")
);
const shadowEnabled = ref<boolean>(
  readStoredString("bm_dl_shadow_enabled_v2", "true") !== "false"
);

// 字形、尺寸与排版状态
const fontSize = ref<number>(
  readStoredNumber("bm_desktop_lyric_size_v2", 32, 18, 56)
);
const widthPx = ref<number>(
  readStoredNumber("bm_desktop_lyric_width_v2", 820, MIN_WIDTH_PX, MAX_WIDTH_PX)
);
const showTranslation = ref<boolean>(
  readStoredString("bm_desktop_lyric_trans", "true") !== "false"
);
const wordByWordEnabled = ref<boolean>(
  readStoredString("bm_desktop_lyric_krc", "true") !== "false"
);
const layoutMode = ref<DesktopLyricLayoutMode>(
  (["center", "left", "ktv", "single"].includes(
    readStoredString("bm_desktop_lyric_layout_v2", "center")
  )
    ? readStoredString("bm_desktop_lyric_layout_v2", "center")
    : "center") as DesktopLyricLayoutMode
);
const lastDoubleLayout = ref<"center" | "ktv" | "left">(
  layoutMode.value === "single" ? "center" : layoutMode.value
);
const glowIntensity = ref<number>(
  readStoredNumber("bm_desktop_lyric_glow_v2", 80, 0, 150)
);
const lyricOpacity = ref<number>(
  readStoredNumber("bm_desktop_lyric_opacity", 1.0, 0.35, 1.0)
);
const lineSpacing = ref<number>(
  readStoredNumber("bm_desktop_lyric_line_spacing", 6, 0, 16)
);
const idleFade = ref<boolean>(
  readStoredString("bm_desktop_lyric_idle_fade", "false") === "true"
);
const showIntro = ref<boolean>(
  readStoredString("bm_desktop_lyric_intro", "true") !== "false"
);
const showPhonetic = ref<boolean>(
  readStoredString("bm_desktop_lyric_phonetic", "false") === "true"
);
const fontFamily = ref<LyricFontFamily>(
  (["sans", "serif", "rounded", "editorial"].includes(
    readStoredString("bm_desktop_lyric_font_family", "sans")
  )
    ? readStoredString("bm_desktop_lyric_font_family", "sans")
    : "sans") as LyricFontFamily
);
const fontWeight = ref<LyricFontWeightLevel>(
  readStoredWeight("bm_desktop_lyric_font_weight", 760)
);
const playMode = ref<string>("repeat_all");
const themeColorScheme = ref<string>(
  readStoredString("bm_color_scheme_kugou_v2", "abyssal_dark") ===
    "watercolor_light"
    ? "watercolor_light"
    : "abyssal_dark"
);
const lightThemeVariant = ref<string>(
  readStoredString("bm_light_theme_variant_v1", "watercolor")
);
const darkThemeVariant = ref<string>(
  readStoredString("bm_dark_theme_variant_v1", "obsidian_aurora")
);
const introVisible = ref<boolean>(false);
let introHideTimer: ReturnType<typeof setTimeout> | null = null;

// ==================== 播放与同步状态 ====================
const isPlaying = ref<boolean>(false);
const livePosMs = ref<number>(0);
const songDurationMs = ref<number>(0);
const playbackRate = ref<number>(1);
const lyricOffsetMs = ref<number>(
  readStoredNumber("bm_lyric_offset_ms", 0, -15000, 15000)
);
const bluetoothDelayMs = ref<number>(
  readStoredNumber("bm_bt_delay_ms", 0, -5000, 5000)
);
const currentSong = ref<{
  hash: string;
  title: string;
  artist: string;
  coverUrl: string;
} | null>(null);
const lyricLines = ref<LyricLineItem[]>([]);
const isKrc = ref<boolean>(false);

let lastSyncedFontSize: number | null = null;
let lastSyncedWidth: number | null = null;
let lastSyncedTrans: boolean | null = null;
let lastLocalControlMutateAt = 0;
let lastAppliedStateSeq = 0;
let lastAppliedStateSentAtMs = 0;
let hasReceivedLiveState = false;
let lastHandshakeTriggerAt = 0;

// ==================== 交互、抽屉标签与平滑宽幅状态 ====================
const isHovering = ref<boolean>(false);
const isPointerInside = ref<boolean>(false);
const lockedHoverWindow = ref<boolean>(false);
const lockedHoverUnlock = ref<boolean>(false);
const isSettingsOpen = ref<boolean>(false);
const settingsTab = ref<SettingsPodTab>("color");
const isResizingEdge = ref<boolean>(false);
const isAdjustingWidth = ref<boolean>(false);
let isDraggingOsWindow = false;
const hoverTip = ref<string>("");
const hoverTipAlign = ref<"left" | "center" | "right">("center");
const viewportRef = ref<HTMLElement | null>(null);
const settingsInnerRef = ref<HTMLElement | null>(null);
const measuredViewportWidth = ref<number>(770);

let hoverLeaveTimer: ReturnType<typeof setTimeout> | null = null;
let shrinkWindowTimer: ReturnType<typeof setTimeout> | null = null;
let widthCmdDebounceTimer: ReturnType<typeof setTimeout> | null = null;
let widthResizeRafPending = false;
let handshakeTimers: Array<ReturnType<typeof setTimeout>> = [];
let channel: BroadcastChannel | null = null;
let unlistenTauriSync: UnlistenFn | null = null;
let unlistenLockedHover: UnlistenFn | null = null;
let unlistenLockSync: UnlistenFn | null = null;
let unlistenVisibility: UnlistenFn | null = null;
let viewportResizeObserver: ResizeObserver | null = null;
let settingsResizeObserver: ResizeObserver | null = null;
let rafId: number | null = null;

let syncAnchorPosMs = 0;
let syncAnchorPerfMs =
  typeof performance !== "undefined" ? performance.now() : 0;

// ==================== 四主题悬停控制台特化（零越界方影 · 100% 无框） ====================
const isLightShell = computed(
  () => themeColorScheme.value === "watercolor_light"
);
const isOatShell = computed(
  () => isLightShell.value && lightThemeVariant.value === "nordic_oat"
);
const isVelvetShell = computed(
  () => !isLightShell.value && darkThemeVariant.value === "velvet_nebula"
);

// 严格约束 hoverShadow 扩散半径 ≤ 10px（远小于壳体 14px/16px 内边距），彻底杜绝窗口边缘裁切方影
const shellUiTheme = computed(() => {
  if (isOatShell.value) {
    return {
      primary: "#0F766E",
      primaryText: "#0F766E",
      secondary: "#B45309",
      text1: "#231D16",
      text2: "#574B3E",
      text3: "#7D7061",
      iconDefault: "#332A20",
      lockColor: "#B45309",
      hoverBg:
        "linear-gradient(168deg, rgba(250, 246, 238, 0.94) 0%, rgba(242, 236, 225, 0.95) 64%, rgba(235, 226, 210, 0.96) 100%)",
      hoverBlur: "none",
      hoverShadow: "0 4px 10px rgba(42, 33, 22, 0.14)",
      cardBg: "rgba(232, 224, 208, 0.72)",
      chipBg: "rgba(35, 29, 22, 0.075)",
      chipHover: "rgba(35, 29, 22, 0.13)",
      divider: "rgba(35, 29, 22, 0.10)",
      tipBg: "#2A221A",
      tipText: "#FAF6EE",
      unlockBg: "#FAF6EE",
      unlockText: "#231D16",
      resizerColor: "rgba(35, 29, 22, 0.28)",
    };
  }
  if (isLightShell.value) {
    return {
      primary: "#4F46E5",
      primaryText: "#4338CA",
      secondary: "#0284C7",
      text1: "#18181B",
      text2: "#475569",
      text3: "#64748B",
      iconDefault: "#1E293B",
      lockColor: "#D97706",
      hoverBg: `
        radial-gradient(ellipse 72% 60% at 14% 0%, rgba(125, 211, 252, 0.28) 0%, transparent 64%),
        radial-gradient(ellipse 68% 58% at 88% 100%, rgba(244, 114, 182, 0.20) 0%, transparent 64%),
        rgba(248, 250, 255, 0.88)
      `,
      hoverBlur: "blur(24px) saturate(170%)",
      hoverShadow: "0 4px 10px rgba(30, 41, 59, 0.12)",
      cardBg: "rgba(255, 255, 255, 0.68)",
      chipBg: "rgba(30, 41, 59, 0.068)",
      chipHover: "rgba(30, 41, 59, 0.125)",
      divider: "rgba(30, 41, 59, 0.085)",
      tipBg: "rgba(24, 24, 27, 0.90)",
      tipText: "#F8FAFC",
      unlockBg: "rgba(248, 250, 255, 0.92)",
      unlockText: "#18181B",
      resizerColor: "rgba(30, 41, 59, 0.24)",
    };
  }
  if (isVelvetShell.value) {
    return {
      primary: "#F43F5E",
      primaryText: "#FDA4AF",
      secondary: "#F59E0B",
      text1: "#F9EFE6",
      text2: "rgba(249, 239, 230, 0.78)",
      text3: "rgba(249, 239, 230, 0.56)",
      iconDefault: "rgba(249, 239, 230, 0.92)",
      lockColor: "#F59E0B",
      hoverBg:
        "linear-gradient(168deg, rgba(40, 25, 34, 0.94) 0%, rgba(28, 18, 24, 0.96) 66%, rgba(22, 13, 19, 0.97) 100%)",
      hoverBlur: "none",
      hoverShadow: "0 4px 10px rgba(0, 0, 0, 0.30)",
      cardBg: "rgba(56, 35, 47, 0.74)",
      chipBg: "rgba(249, 239, 230, 0.085)",
      chipHover: "rgba(249, 239, 230, 0.15)",
      divider: "rgba(249, 239, 230, 0.10)",
      tipBg: "#120B0F",
      tipText: "#F9EFE6",
      unlockBg: "#24161E",
      unlockText: "#F9EFE6",
      resizerColor: "rgba(249, 239, 230, 0.28)",
    };
  }
  return {
    primary: "#5E6AD2",
    primaryText: "#A5B4FC",
    secondary: "#38BDF8",
    text1: "#F5F7FA",
    text2: "rgba(232, 237, 245, 0.78)",
    text3: "rgba(210, 218, 232, 0.58)",
    iconDefault: "rgba(245, 247, 250, 0.92)",
    lockColor: "#FFD166",
    hoverBg: `
      radial-gradient(circle at 14% 4%, rgba(94, 106, 210, 0.22) 0%, transparent 56%),
      radial-gradient(circle at 86% 96%, rgba(56, 189, 248, 0.14) 0%, transparent 52%),
      linear-gradient(168deg, rgba(13, 18, 30, 0.86) 0%, rgba(9, 13, 24, 0.91) 100%)
    `,
    hoverBlur: "blur(24px) saturate(170%)",
    hoverShadow: "0 4px 10px rgba(0, 0, 0, 0.28)",
    cardBg: "rgba(255, 255, 255, 0.068)",
    chipBg: "rgba(255, 255, 255, 0.09)",
    chipHover: "rgba(255, 255, 255, 0.16)",
    divider: "rgba(255, 255, 255, 0.10)",
    tipBg: "rgba(8, 12, 22, 0.92)",
    tipText: "#F5F7FA",
    unlockBg: "rgba(12, 16, 28, 0.90)",
    unlockText: "#F5F7FA",
    resizerColor: "rgba(255, 255, 255, 0.28)",
  };
});

const glassVisible = computed(() => {
  return (
    !isLocked.value &&
    (isHovering.value ||
      isSettingsOpen.value ||
      isResizingEdge.value ||
      isAdjustingWidth.value)
  );
});

// ==================== 共享双通道配色、真 4 档字重与光学矢量描边解析 ====================
const themeAccentPair = computed(() => {
  if (isOatShell.value) return { start: "#14B8A6", end: "#F59E0B" };
  if (isLightShell.value) return { start: "#6366F1", end: "#38BDF8" };
  if (isVelvetShell.value) return { start: "#F43F5E", end: "#F59E0B" };
  return { start: "#5E6AD2", end: "#38BDF8" };
});

const resolvedDynamicPair = computed(() => {
  if (dynamicCoverAccentEnabled.value && dynamicPrimaryHex.value) {
    return {
      primary: dynamicPrimaryHex.value,
      secondary: dynamicSecondaryHex.value || dynamicPrimaryHex.value,
    };
  }
  return {
    primary: themeAccentPair.value.start,
    secondary: themeAccentPair.value.end,
  };
});

const sungPresetsDisplay = computed(() =>
  LYRIC_SUNG_PRESETS.map((p) => {
    const start = p.isDynamic
      ? resolvedDynamicPair.value.primary
      : p.darkPrimary;
    const end = p.isDynamic
      ? resolvedDynamicPair.value.secondary
      : p.darkSecondary;
    return {
      ...p,
      label:
        p.isDynamic && !dynamicCoverAccentEnabled.value ? "主题流光" : p.label,
      swatchStart: start,
      swatchEnd: sungMode.value === "solid" ? start : end,
    };
  })
);

const activeSungPalette = computed(() =>
  resolveSungLyricPalette({
    themeId: colorSchemeId.value,
    sungMode: sungMode.value,
    customPrimary: customPrimaryHex.value,
    customSecondary: customSecondaryHex.value,
    isLightStage: false,
    dynamicPrimary: resolvedDynamicPair.value.primary,
    dynamicSecondary: resolvedDynamicPair.value.secondary,
  })
);

const activeSungLabel = computed(() => {
  if (colorSchemeId.value === "custom") {
    return sungMode.value === "solid" ? "自定义纯色" : "自定义渐变";
  }
  const hit = sungPresetsDisplay.value.find(
    (p) => p.id === colorSchemeId.value
  );
  return hit?.label || "封面流光";
});

const activeUnsungHex = computed(() =>
  resolveUnsungLyricHex({
    preset: unsungPreset.value,
    customHex: unsungCustomHex.value,
    isLightStage: false,
    sungPrimaryHex: activeSungPalette.value.primary,
  })
);

const activeUnsungLabel = computed(() => {
  if (unsungPreset.value === "custom") return "自定义";
  const hit = LYRIC_UNSUNG_PRESETS.find((p) => p.id === unsungPreset.value);
  return hit?.label || "智能随境";
});

const initialDesktopTypoOverride = resolveDesktopLyricFontOverride(
  loadPersistedTypographyConfig()
);
const customFontFamilyStack = ref<string>(
  initialDesktopTypoOverride.familyStack || ""
);
const customFontId = ref<string>(initialDesktopTypoOverride.customFontId || "");
const customFontWeightOverride = ref<number | null>(
  initialDesktopTypoOverride.weightOverride
);
let typoSyncChannel: BroadcastChannel | null = null;

// 真 4 档字重（与播放页完全同步，支持「字体与排版」自定义/导入字体覆盖）
const activeTypography = computed(() =>
  resolveLyricTypographyStyle({
    family: fontFamily.value,
    weight: fontWeight.value,
    customFontFamily: customFontFamilyStack.value || null,
    customFontWeight: customFontWeightOverride.value,
    isGradientText: true,
    strokeTintHex: activeSungPalette.value.primary,
  })
);

const activeUnsungTypography = computed(() =>
  resolveLyricTypographyStyle({
    family: fontFamily.value,
    weight: fontWeight.value,
    customFontFamily: customFontFamilyStack.value || null,
    customFontWeight: customFontWeightOverride.value,
    isGradientText: true,
    strokeTintHex: activeUnsungHex.value,
  })
);

// 真双层光学矢量描边（独立分别判断已唱 / 未唱亮度，auto 模式下智能配对黑/白外轮廓）
const sungStrokeSpec = computed(() =>
  resolveDesktopStrokeStyle({
    widthMode: strokeWidth.value,
    colorMode: strokeColorMode.value,
    customHex: strokeCustomHex.value,
    shadowEnabled: shadowEnabled.value,
    isLightText: isBrightHex(activeSungPalette.value.primary),
    fontSizePx: fontSize.value,
  })
);

const unsungStrokeSpec = computed(() =>
  resolveDesktopStrokeStyle({
    widthMode: strokeWidth.value,
    colorMode: strokeColorMode.value,
    customHex: strokeCustomHex.value,
    shadowEnabled: shadowEnabled.value,
    isLightText: isBrightHex(activeUnsungHex.value),
    fontSizePx: fontSize.value,
  })
);

const subStrokeSpec = computed(() =>
  resolveDesktopStrokeStyle({
    widthMode: strokeWidth.value,
    colorMode: strokeColorMode.value,
    customHex: strokeCustomHex.value,
    shadowEnabled: shadowEnabled.value,
    isLightText: isBrightHex(activeUnsungHex.value),
    fontSizePx: Math.max(14, Math.round(fontSize.value * 0.62)),
  })
);

const playModeMeta = computed(() => {
  if (playMode.value === "shuffle") {
    return { icon: "shuffle", tip: "随机播放（点击切换）", active: true };
  }
  if (playMode.value === "repeat_one") {
    return { icon: "repeat-one", tip: "单曲循环（点击切换）", active: true };
  }
  return { icon: "repeat", tip: "列表循环（点击切换）", active: false };
});

const playBtnInkColor = computed(() =>
  pickContrastInk(shellUiTheme.value.primary)
);

const progressPct = computed(() => {
  if (!songDurationMs.value || songDurationMs.value <= 0) return 0;
  return clamp((livePosMs.value / songDurationMs.value) * 100, 0, 100);
});

const widthRangePct = computed(
  () =>
    `${(((widthPx.value - MIN_WIDTH_PX) / (MAX_WIDTH_PX - MIN_WIDTH_PX)) * 100).toFixed(1)}%`
);
const fontRangePct = computed(
  () => `${(((fontSize.value - 18) / (56 - 18)) * 100).toFixed(1)}%`
);
const glowRangePct = computed(
  () => `${((glowIntensity.value / 150) * 100).toFixed(1)}%`
);
const unsungOpacityRangePct = computed(
  () => `${(((unsungOpacity.value - 0.35) / (0.98 - 0.35)) * 100).toFixed(1)}%`
);
const opacityRangePct = computed(
  () => `${(((lyricOpacity.value - 0.35) / (1.0 - 0.35)) * 100).toFixed(1)}%`
);
const lineSpacingRangePct = computed(
  () => `${((lineSpacing.value / 16) * 100).toFixed(1)}%`
);

// ==================== 视口与 OS 窗口尺寸联动 ====================
const lyricViewportHeightPx = computed(() => {
  if (layoutMode.value === "single") {
    return Math.max(60, Math.round(fontSize.value * 1.92 + 16));
  }
  return Math.max(
    102,
    Math.round(fontSize.value * 3.18 + lineSpacing.value * 1.55 + 16)
  );
});

const baseWindowHeightPx = computed(() => {
  const minBase = layoutMode.value === "single" ? 122 : 162;
  return Math.max(
    minBase,
    SHELL_VPAD_PX +
      TOOLBAR_HEIGHT_PX +
      DIVIDER_HEIGHT_PX +
      PROGRESS_HEIGHT_PX +
      lyricViewportHeightPx.value +
      8
  );
});

const showLockedUnlockCapsule = computed(() => {
  return (
    isLocked.value &&
    (lockedHoverWindow.value || lockedHoverUnlock.value || isHovering.value)
  );
});

// ==================== 离屏 Canvas 像素级宽度测量 ====================
let measureCtx: CanvasRenderingContext2D | null = null;

function getMeasureCtx(): CanvasRenderingContext2D | null {
  if (measureCtx) return measureCtx;
  if (typeof document === "undefined") return null;
  const canvas = document.createElement("canvas");
  measureCtx = canvas.getContext("2d");
  return measureCtx;
}

function measureTextWidthPx(
  text: string,
  sizePx: number,
  weight: number
): number {
  if (!text) return 0;
  const ctx = getMeasureCtx();
  if (!ctx) {
    let units = 0;
    for (const ch of text) {
      units += ch.charCodeAt(0) <= 0x7f ? 0.56 : 1.0;
    }
    return units * sizePx;
  }
  ctx.font = `${weight} ${sizePx}px ${activeTypography.value.fontFamily}`;
  return ctx.measureText(text).width;
}

// ==================== 60fps 零冻结高精度时钟插值（隐身/暂停立即停帧；纯 LRC 行式节流、KRC 逐字理性 60fps 封顶） ====================
const hasAnyKrcWords = computed(() =>
  lyricLines.value.some((l) => Boolean(l.words?.length))
);
let lastLrcStepPerfMs = 0;
let lastKrcStepPerfMs = 0;

function startClockLoop() {
  if (
    rafId !== null ||
    !isPlaying.value ||
    !isWindowVisible.value ||
    (typeof document !== "undefined" && document.hidden)
  ) {
    return;
  }
  lastLrcStepPerfMs = 0;
  lastKrcStepPerfMs = 0;
  const tick = (nowPerf: number) => {
    if (
      !isPlaying.value ||
      !isWindowVisible.value ||
      (typeof document !== "undefined" && document.hidden)
    ) {
      rafId = null;
      return;
    }
    const rawElapsed = Math.max(0, nowPerf - syncAnchorPerfMs);
    const elapsed =
      Math.min(rawElapsed, 2500) * Math.max(0.25, playbackRate.value || 1);
    const nextRaw = syncAnchorPosMs + elapsed;
    const clampedPos =
      songDurationMs.value > 0
        ? Math.min(songDurationMs.value, nextRaw)
        : nextRaw;

    const lines = lyricLines.value;
    const curIdx = activeLineIndex.value;
    const activeHasKrcWords =
      wordByWordEnabled.value &&
      ((curIdx >= 0 && Boolean(lines[curIdx]?.words?.length)) ||
        hasAnyKrcWords.value);

    if (activeHasKrcWords) {
      // 封顶 60fps（16ms），防止 144Hz/240Hz 显示器在悬浮歌词独立窗口上无意义高倍重渲染
      if (lastKrcStepPerfMs === 0 || nowPerf - lastKrcStepPerfMs >= 16) {
        lastKrcStepPerfMs = nowPerf;
        livePosMs.value = clampedPos;
      }
    } else {
      const nextLine = curIdx + 1 < lines.length ? lines[curIdx + 1] : null;
      const effNext = clampedPos + lyricOffsetMs.value - bluetoothDelayMs.value;
      const crossedLine = Boolean(nextLine && effNext >= nextLine.startTimeMs);
      if (crossedLine || lastLrcStepPerfMs === 0 || nowPerf - lastLrcStepPerfMs >= 95) {
        lastLrcStepPerfMs = nowPerf;
        livePosMs.value = clampedPos;
      }
    }
    rafId = requestAnimationFrame(tick);
  };
  rafId = requestAnimationFrame(tick);
}

function stopClockLoop() {
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
}

function handleDesktopLyricVisibilityChange() {
  if (typeof document === "undefined") return;
  if (document.hidden) {
    stopClockLoop();
  } else if (isWindowVisible.value && isPlaying.value) {
    startClockLoop();
  }
}

function applySyncedPosition(
  posMs: number,
  playing: boolean,
  sentAtMs?: number,
  rate?: number
) {
  if (
    rate !== undefined &&
    Number.isFinite(Number(rate)) &&
    Number(rate) > 0
  ) {
    playbackRate.value = Number(rate);
  }
  let safePos = Math.max(0, Number(posMs) || 0);
  if (playing && sentAtMs && Number.isFinite(sentAtMs)) {
    const transitMs = clamp(Date.now() - Number(sentAtMs), 0, 180);
    safePos += transitMs * Math.max(0.25, playbackRate.value || 1);
  }
  const delta = safePos - livePosMs.value;
  if (!playing || Math.abs(delta) > 320 || safePos === 0) {
    livePosMs.value = safePos;
    syncAnchorPosMs = safePos;
  } else {
    const smoothAnchor =
      delta >= -32 && delta <= 0 ? livePosMs.value : safePos;
    syncAnchorPosMs = smoothAnchor;
    if (delta > 0) livePosMs.value = smoothAnchor;
  }
  syncAnchorPerfMs = performance.now();
  isPlaying.value = playing;
  if (
    playing &&
    isWindowVisible.value &&
    (typeof document === "undefined" || !document.hidden)
  ) {
    startClockLoop();
  } else {
    stopClockLoop();
  }
}

// ==================== 歌词数据结构标准化与跨窗口快照恢复 ====================
function normalizeLyricLines(rawLines: any[]): LyricLineItem[] {
  if (!Array.isArray(rawLines) || rawLines.length === 0) return [];
  const normalized: LyricLineItem[] = [];
  for (let i = 0; i < rawLines.length; i++) {
    const item = rawLines[i];
    if (!item || typeof item !== "object") continue;
    const startTimeMs = Math.max(
      0,
      Number(item.startTimeMs ?? item.timeMs ?? item.startMs ?? 0) || 0
    );
    const nextItem = rawLines[i + 1];
    const nextStartMs = nextItem
      ? Math.max(
          0,
          Number(
            nextItem.startTimeMs ?? nextItem.timeMs ?? nextItem.startMs ?? 0
          ) || 0
        )
      : 0;
    const rawDur = Number(item.durationMs) || 0;
    const durationMs =
      rawDur > 0
        ? rawDur
        : nextStartMs > startTimeMs
        ? Math.min(12000, nextStartMs - startTimeMs)
        : 3200;
    const words: LyricWordItem[] = Array.isArray(item.words)
      ? item.words
          .filter((w: any) => w && typeof w === "object")
          .map((w: any) => ({
            text: String(w.text ?? ""),
            startTimeMs: Math.max(
              0,
              Number(w.startTimeMs ?? w.startMs ?? w.timeMs ?? 0) || 0
            ),
            durationMs: Math.max(40, Number(w.durationMs) || 120),
          }))
      : [];
    normalized.push({
      startTimeMs,
      durationMs,
      text: String(item.text ?? ""),
      translation: item.translation ? String(item.translation) : undefined,
      phonetic: item.phonetic ? String(item.phonetic) : undefined,
      words,
    });
  }
  return normalized;
}

function applyIncomingSyncPayload(
  msg: any,
  options?: { fromStorage?: boolean }
) {
  if (!msg || typeof msg !== "object") return;
  const fromStorage = Boolean(options?.fromStorage);

  if (msg.type === "POSITION_SYNC") {
    const incomingHash = msg.songHash ? String(msg.songHash) : "";
    const activeHash = currentSong.value?.hash
      ? String(currentSong.value.hash)
      : "";
    if (incomingHash && activeHash && incomingHash !== activeHash) {
      const nowMs = performance.now();
      if (!fromStorage && nowMs - lastHandshakeTriggerAt > 420) {
        lastHandshakeTriggerAt = nowMs;
        sendCmd("CMD_REQUEST_SYNC");
      }
      return;
    }
    if (msg.lyricOffsetMs !== undefined)
      lyricOffsetMs.value = Number(msg.lyricOffsetMs || 0);
    if (msg.bluetoothDelayMs !== undefined)
      bluetoothDelayMs.value = Number(msg.bluetoothDelayMs || 0);
    if (msg.durationMs !== undefined && Number(msg.durationMs) > 0) {
      songDurationMs.value = Number(msg.durationMs);
    }
    applySyncedPosition(
      Number(msg.positionMs || 0),
      Boolean(msg.isPlaying),
      msg.sentAtMs,
      msg.playbackRate
    );
    return;
  }

  if (msg.type !== "STATE_SYNC") return;

  const incomingSeq = Number(msg.stateSeq) || 0;
  const incomingSentAt = Number(msg.sentAtMs) || 0;
  if (
    fromStorage &&
    hasReceivedLiveState &&
    incomingSentAt < lastAppliedStateSentAtMs
  ) {
    return;
  }
  if (
    !fromStorage &&
    incomingSeq > 0 &&
    lastAppliedStateSeq > 0 &&
    incomingSeq < lastAppliedStateSeq
  ) {
    return;
  }
  if (!fromStorage) {
    hasReceivedLiveState = true;
  }
  if (incomingSeq > lastAppliedStateSeq) {
    lastAppliedStateSeq = incomingSeq;
  }
  if (incomingSentAt > lastAppliedStateSentAtMs) {
    lastAppliedStateSentAtMs = incomingSentAt;
  }

  if (msg.enabled !== undefined) {
    isWindowVisible.value = Boolean(msg.enabled);
  }

  const recentlyMutatedLocally =
    performance.now() - lastLocalControlMutateAt < 380;

  if (msg.locked !== undefined && !recentlyMutatedLocally) {
    const nextLocked = Boolean(msg.locked);
    if (nextLocked && !isLocked.value) {
      isSettingsOpen.value = false;
      isHovering.value = false;
    }
    isLocked.value = nextLocked;
  }

  if (msg.dynamicPrimary) dynamicPrimaryHex.value = String(msg.dynamicPrimary);
  if (msg.dynamicSecondary)
    dynamicSecondaryHex.value = String(msg.dynamicSecondary);
  if (msg.dynamicCoverAccentEnabled !== undefined) {
    dynamicCoverAccentEnabled.value = Boolean(msg.dynamicCoverAccentEnabled);
  }
  if (msg.durationMs !== undefined && Number(msg.durationMs) > 0) {
    songDurationMs.value = Number(msg.durationMs);
  }

  if (!recentlyMutatedLocally) {
    if (msg.colorSchemeId) {
      const normalized = normalizeLyricThemeId(String(msg.colorSchemeId));
      colorSchemeId.value = normalized;
      localStorage.setItem("bm_desktop_lyric_scheme", normalized);
    }
    if (msg.sungMode === "gradient" || msg.sungMode === "solid") {
      sungMode.value = msg.sungMode;
      localStorage.setItem("bm_dl_sung_mode_v2", msg.sungMode);
    }
    if (msg.customPrimary) {
      customPrimaryHex.value = String(msg.customPrimary);
      localStorage.setItem("bm_dl_custom_primary_v2", customPrimaryHex.value);
    }
    if (msg.customSecondary) {
      customSecondaryHex.value = String(msg.customSecondary);
      localStorage.setItem(
        "bm_dl_custom_secondary_v2",
        customSecondaryHex.value
      );
    }
    if (msg.unsungPreset) {
      unsungPreset.value = normalizeUnsungPresetId(String(msg.unsungPreset));
      localStorage.setItem("bm_dl_unsung_preset_v2", unsungPreset.value);
    }
    if (msg.unsungCustomHex) {
      unsungCustomHex.value = String(msg.unsungCustomHex);
      localStorage.setItem("bm_dl_unsung_custom_v2", unsungCustomHex.value);
    }
    if (msg.unsungOpacity !== undefined) {
      unsungOpacity.value = Number(
        clamp(Number(msg.unsungOpacity), 0.35, 0.98).toFixed(2)
      );
      localStorage.setItem(
        "bm_dl_unsung_opacity_v2",
        String(unsungOpacity.value)
      );
    }
    if (
      msg.strokeWidth === "none" ||
      msg.strokeWidth === "thin" ||
      msg.strokeWidth === "medium" ||
      msg.strokeWidth === "bold"
    ) {
      strokeWidth.value = msg.strokeWidth;
      localStorage.setItem("bm_dl_stroke_width_v2", msg.strokeWidth);
    }
    if (
      msg.strokeColorMode === "auto" ||
      msg.strokeColorMode === "black" ||
      msg.strokeColorMode === "white" ||
      msg.strokeColorMode === "custom"
    ) {
      strokeColorMode.value = msg.strokeColorMode;
      localStorage.setItem("bm_dl_stroke_color_mode_v2", msg.strokeColorMode);
    }
    if (msg.strokeCustomHex) {
      strokeCustomHex.value = String(msg.strokeCustomHex);
      localStorage.setItem("bm_dl_stroke_custom_v2", strokeCustomHex.value);
    }
    if (msg.shadowEnabled !== undefined) {
      shadowEnabled.value = Boolean(msg.shadowEnabled);
      localStorage.setItem(
        "bm_dl_shadow_enabled_v2",
        String(shadowEnabled.value)
      );
    }
    if (msg.fontSize !== undefined) {
      const incomingFs = clamp(Number(msg.fontSize), 18, 56);
      if (lastSyncedFontSize === null || incomingFs !== lastSyncedFontSize) {
        lastSyncedFontSize = incomingFs;
        fontSize.value = incomingFs;
      }
    }
    if (
      msg.width !== undefined &&
      !isResizingEdge.value &&
      !isAdjustingWidth.value
    ) {
      const incomingW = clamp(Number(msg.width), MIN_WIDTH_PX, MAX_WIDTH_PX);
      if (lastSyncedWidth === null || incomingW !== lastSyncedWidth) {
        lastSyncedWidth = incomingW;
        widthPx.value = incomingW;
        syncOsWindowWithState();
      }
    }
    if (msg.showTranslation !== undefined) {
      const incomingTrans = Boolean(msg.showTranslation);
      if (lastSyncedTrans === null || incomingTrans !== lastSyncedTrans) {
        lastSyncedTrans = incomingTrans;
        showTranslation.value = incomingTrans;
      }
    }
    if (
      msg.layoutMode === "center" ||
      msg.layoutMode === "ktv" ||
      msg.layoutMode === "single" ||
      msg.layoutMode === "left"
    ) {
      layoutMode.value = msg.layoutMode;
      if (msg.layoutMode !== "single") lastDoubleLayout.value = msg.layoutMode;
    } else if (msg.lineMode === "single") {
      layoutMode.value = "single";
    }
    if (msg.opacity !== undefined) {
      lyricOpacity.value = clamp(Number(msg.opacity), 0.35, 1.0);
    }
    if (msg.lineSpacing !== undefined) {
      lineSpacing.value = clamp(Math.round(Number(msg.lineSpacing)), 0, 16);
    }
    if (msg.glowIntensity !== undefined) {
      glowIntensity.value = clamp(Math.round(Number(msg.glowIntensity)), 0, 150);
    }
    if (msg.idleFade !== undefined) {
      idleFade.value = Boolean(msg.idleFade);
    }
    if (msg.showIntro !== undefined) {
      showIntro.value = Boolean(msg.showIntro);
    }
    if (msg.showPhonetic !== undefined) {
      showPhonetic.value = Boolean(msg.showPhonetic);
    }
    if (msg.wordByWord !== undefined) {
      wordByWordEnabled.value = Boolean(msg.wordByWord);
    }
    if (
      msg.fontFamily === "sans" ||
      msg.fontFamily === "serif" ||
      msg.fontFamily === "rounded" ||
      msg.fontFamily === "editorial"
    ) {
      fontFamily.value = msg.fontFamily;
    }
    if (msg.fontWeight !== undefined) {
      const fw = Number(msg.fontWeight);
      if (fw === 500 || fw === 650 || fw === 760 || fw === 880) {
        fontWeight.value = fw;
      }
    }
    if (msg.customFontFamilyStack !== undefined) {
      customFontFamilyStack.value = String(msg.customFontFamilyStack || "");
    }
    if (msg.customFontId !== undefined) {
      const nextId = String(msg.customFontId || "");
      customFontId.value = nextId;
      if (nextId) {
        ensureCustomFontLoaded(nextId).catch(() => {});
      }
    }
    if (msg.customFontWeight !== undefined) {
      const cw = Number(msg.customFontWeight);
      customFontWeightOverride.value =
        msg.customFontWeight !== null && Number.isFinite(cw) && cw >= 100
          ? cw
          : null;
    }
  }

  if (msg.lyricOffsetMs !== undefined)
    lyricOffsetMs.value = Number(msg.lyricOffsetMs || 0);
  if (msg.bluetoothDelayMs !== undefined)
    bluetoothDelayMs.value = Number(msg.bluetoothDelayMs || 0);
  if (msg.playMode) playMode.value = String(msg.playMode);
  const incomingSchemeMode = msg.colorScheme || msg.globalColorScheme;
  if (incomingSchemeMode) {
    themeColorScheme.value = String(incomingSchemeMode);
  }
  if (msg.lightThemeVariant)
    lightThemeVariant.value = String(msg.lightThemeVariant);
  if (msg.darkThemeVariant)
    darkThemeVariant.value = String(msg.darkThemeVariant);

  const prevSongHash = currentSong.value?.hash || "";
  if (msg.song && typeof msg.song === "object") {
    currentSong.value = {
      hash: String(msg.song.hash || ""),
      title: String(msg.song.title || ""),
      artist: String(msg.song.artist || ""),
      coverUrl: String(msg.song.coverUrl || ""),
    };
  } else if (msg.song === null) {
    currentSong.value = null;
  }
  const songChanged = (currentSong.value?.hash || "") !== prevSongHash;

  if (
    songChanged &&
    currentSong.value?.title &&
    showIntro.value &&
    !fromStorage
  ) {
    introVisible.value = true;
    if (introHideTimer) clearTimeout(introHideTimer);
    introHideTimer = setTimeout(() => {
      introVisible.value = false;
      introHideTimer = null;
    }, 2200);
  }

  if (
    msg.lyric &&
    typeof msg.lyric === "object" &&
    Array.isArray(msg.lyric.lines)
  ) {
    const normalized = normalizeLyricLines(msg.lyric.lines);
    lyricLines.value = normalized;
    isKrc.value =
      Boolean(msg.lyric.isKrc) ||
      normalized.some((l) => (l.words?.length || 0) > 0);
  } else if (msg.lyric === null || songChanged) {
    isKrc.value = false;
    lyricLines.value = [];
  }

  if (songChanged) {
    livePosMs.value = Math.max(0, Number(msg.positionMs || 0));
  }
  applySyncedPosition(
    Number(msg.positionMs || 0),
    Boolean(msg.isPlaying),
    msg.sentAtMs,
    msg.playbackRate
  );
}

function hydrateFromStorageSnapshots() {
  let stateSnapSentAt = 0;
  let stateSnapSongHash = "";
  try {
    const rawState = localStorage.getItem(SNAPSHOT_STORAGE_KEY);
    if (rawState) {
      const parsedState = JSON.parse(rawState);
      if (parsedState && typeof parsedState === "object") {
        stateSnapSentAt = Number(parsedState.sentAtMs || 0);
        stateSnapSongHash = String(parsedState.song?.hash || "");
        applyIncomingSyncPayload(parsedState, { fromStorage: true });
      }
    }
  } catch {
    // ignore corrupted snapshot
  }
  try {
    const rawPos = localStorage.getItem(POS_STORAGE_KEY);
    if (rawPos) {
      const parsedPos = JSON.parse(rawPos);
      if (parsedPos && typeof parsedPos === "object") {
        const posSentAt = Number(parsedPos.sentAtMs || 0);
        const posHash = String(parsedPos.songHash || "");
        const expectedHash = currentSong.value?.hash || stateSnapSongHash;
        const hashMatches = !posHash || !expectedHash || posHash === expectedHash;
        const newerThanState =
          posSentAt >= Math.max(stateSnapSentAt, lastAppliedStateSentAtMs);
        if (hashMatches && newerThanState) {
          applyIncomingSyncPayload(parsedPos, { fromStorage: true });
        }
      }
    }
  } catch {
    // ignore corrupted pos
  }
}

hydrateFromStorageSnapshots();

if (
  isBrowserDevPreviewEnv() &&
  !currentSong.value &&
  lyricLines.value.length === 0
) {
  const showcase = createBrowserDevShowcase();
  const demoSong = showcase.songs[0];
  if (demoSong) {
    applyIncomingSyncPayload(
      {
        type: "STATE_SYNC",
        song: demoSong,
        lyric: showcase.lyric,
        positionMs: 11200,
        durationMs: demoSong.durationMs || 218000,
        isPlaying: false,
        playbackRate: 1,
        sentAtMs: Date.now(),
      },
      { fromStorage: true }
    );
  }
}

// ==================== 语义折行与自适应行距排版规划 ====================
const calibratedPosMs = computed(() => {
  return livePosMs.value + lyricOffsetMs.value - bluetoothDelayMs.value;
});

const activeLineIndex = computed(() => {
  const lines = lyricLines.value;
  if (!lines.length) return -1;
  const pos = calibratedPosMs.value;
  if (pos < lines[0].startTimeMs) return -1;
  let lo = 0;
  let hi = lines.length - 1;
  let ans = 0;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].startTimeMs <= pos) {
      ans = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return ans;
});

const interludeProgress = computed(() => {
  const lines = lyricLines.value;
  if (!lines.length) return -1;
  const pos = calibratedPosMs.value;
  const idx = activeLineIndex.value;

  if (idx < 0) {
    const firstStart = lines[0].startTimeMs;
    if (firstStart < 2500) return -1;
    return clamp(pos / Math.max(1, firstStart), 0, 1);
  }

  const cur = lines[idx];
  const next = lines[idx + 1];
  if (!next) return -1;
  const curWords = cur.words || [];
  const lastWordEnd =
    curWords.length > 0
      ? Math.max(
          ...curWords.map(
            (w) =>
              (w.startTimeMs >= cur.startTimeMs
                ? w.startTimeMs
                : cur.startTimeMs + w.startTimeMs) + Math.max(80, w.durationMs)
          )
        )
      : cur.startTimeMs + Math.max(1200, cur.durationMs || 2600);
  const gap = next.startTimeMs - lastWordEnd;
  if (gap >= 4500 && pos > lastWordEnd + 350 && pos < next.startTimeMs) {
    return clamp((pos - lastWordEnd) / gap, 0, 1);
  }
  return -1;
});

const SPLIT_PUNCTUATION = new Set([
  " ",
  ",",
  "，",
  ".",
  "。",
  "!",
  "！",
  "?",
  "？",
  ";",
  "；",
  "、",
  "~",
  "-",
  "—",
]);

function isAsciiWordChar(ch: string): boolean {
  return /[A-Za-z0-9''-]/.test(ch);
}

interface SplitPart {
  text: string;
  words: LyricWordItem[];
  startTimeMs: number;
  durationMs: number;
}

function splitLongLineSemantically(
  line: LyricLineItem,
  maxWidthPx: number,
  fontPx: number,
  weight: number
): [SplitPart, SplitPart] {
  const words = line.words || [];
  const lineStart = line.startTimeMs || 0;
  const lineDur = Math.max(400, line.durationMs || 3200);

  if (words.length >= 2) {
    const wordWidths = words.map((w) =>
      measureTextWidthPx(w.text, fontPx, weight)
    );
    const totalW = Math.max(1, wordWidths.reduce((a, b) => a + b, 0));
    const targetHalfW = Math.min(maxWidthPx * 0.94, totalW * 0.52);

    let acc = 0;
    let bestIdx = 1;
    let bestScore = Infinity;

    for (let i = 0; i < words.length - 1; i++) {
      acc += wordWidths[i];
      const curText = words[i].text;
      const nextText = words[i + 1].text;
      const curLast = curText.slice(-1);
      const nextFirst = nextText.slice(0, 1);

      const breaksAsciiWord =
        isAsciiWordChar(curLast) && isAsciiWordChar(nextFirst);
      const isNaturalBreak =
        SPLIT_PUNCTUATION.has(curLast) ||
        SPLIT_PUNCTUATION.has(nextFirst) ||
        curLast.charCodeAt(0) > 0x7f;

      const row1FitPenalty = acc > maxWidthPx ? (acc - maxWidthPx) * 4 : 0;
      const row2W = totalW - acc;
      const row2FitPenalty = row2W > maxWidthPx ? (row2W - maxWidthPx) * 3 : 0;
      const balanceDist = Math.abs(acc - targetHalfW);
      const semanticPenalty = breaksAsciiWord
        ? 180
        : isNaturalBreak
        ? -18
        : 24;
      const orphanPenalty = i === 0 || i === words.length - 2 ? 32 : 0;

      const score =
        balanceDist +
        row1FitPenalty +
        row2FitPenalty +
        semanticPenalty +
        orphanPenalty;
      if (score < bestScore) {
        bestScore = score;
        bestIdx = i + 1;
      }
    }

    bestIdx = clamp(bestIdx, 1, words.length - 1);
    const p1Words = words.slice(0, bestIdx);
    const p2Words = words.slice(bestIdx);
    const absStartOf = (w: LyricWordItem) =>
      w.startTimeMs >= lineStart ? w.startTimeMs : lineStart + w.startTimeMs;
    const p1Start = absStartOf(p1Words[0]);
    const p1Last = p1Words[p1Words.length - 1];
    const p1End = absStartOf(p1Last) + Math.max(80, p1Last.durationMs);
    const p2Start = absStartOf(p2Words[0]);
    const p2Last = p2Words[p2Words.length - 1];
    const p2End = absStartOf(p2Last) + Math.max(80, p2Last.durationMs);

    return [
      {
        text: p1Words
          .map((w) => w.text)
          .join("")
          .trim(),
        words: p1Words,
        startTimeMs: p1Start,
        durationMs: Math.max(120, p1End - p1Start),
      },
      {
        text: p2Words
          .map((w) => w.text)
          .join("")
          .trim(),
        words: p2Words,
        startTimeMs: p2Start,
        durationMs: Math.max(120, p2End - p2Start),
      },
    ];
  }

  const full = line.text.trim();
  const totalW = Math.max(1, measureTextWidthPx(full, fontPx, weight));
  const midChar = Math.max(1, Math.floor(full.length * 0.52));
  let breakPos = -1;
  let bestDist = Infinity;

  for (let i = 2; i < full.length - 1; i++) {
    const ch = full[i];
    const prev = full[i - 1];
    const isSpaceOrPunc =
      SPLIT_PUNCTUATION.has(prev) || SPLIT_PUNCTUATION.has(ch);
    const isCjkBoundary = prev.charCodeAt(0) > 0x7f || ch.charCodeAt(0) > 0x7f;
    if (!isSpaceOrPunc && !isCjkBoundary) continue;

    const candidateLeft = full
      .slice(0, isSpaceOrPunc && SPLIT_PUNCTUATION.has(ch) ? i + 1 : i)
      .trim();
    const wLeft = measureTextWidthPx(candidateLeft, fontPx, weight);
    if (wLeft > maxWidthPx * 1.02) continue;

    const dist = Math.abs(wLeft - totalW * 0.52) + (isSpaceOrPunc ? -24 : 18);
    if (dist < bestDist) {
      bestDist = dist;
      breakPos = isSpaceOrPunc && SPLIT_PUNCTUATION.has(ch) ? i + 1 : i;
    }
  }

  if (breakPos <= 0 || breakPos >= full.length) {
    breakPos = midChar;
  }

  const part1 = full.slice(0, breakPos).trim();
  const part2 = full.slice(breakPos).trim();
  const w1 = measureTextWidthPx(part1, fontPx, weight);
  const w2 = measureTextWidthPx(part2, fontPx, weight);
  const ratio = clamp(w1 / Math.max(1, w1 + w2), 0.25, 0.75);
  const p1Dur = Math.max(200, Math.round(lineDur * ratio));

  return [
    {
      text: part1,
      words: [],
      startTimeMs: lineStart,
      durationMs: p1Dur,
    },
    {
      text: part2,
      words: [],
      startTimeMs: lineStart + p1Dur,
      durationMs: Math.max(200, lineDur - p1Dur),
    },
  ];
}

function computeRowFitScale(
  text: string,
  sizePx: number,
  weight: number,
  maxWidthPx: number
): number {
  const measured = measureTextWidthPx(text, sizePx, weight);
  if (measured <= maxWidthPx || measured <= 0) return 1;
  return clamp((maxWidthPx - 6) / measured, 0.56, 1);
}

const dualRowVerticalPct = computed(() => {
  const spread = clamp(20 + lineSpacing.value * 0.45, 20, 27);
  return { top: 50 - spread, bottom: 50 + spread };
});

const displayPlan = computed<{ rows: DisplayRow[]; isThreeRow: boolean }>(() => {
  const lines = lyricLines.value;
  const maxWidth = Math.max(280, measuredViewportWidth.value - 28);
  const mainPx = fontSize.value;
  const subPx = Math.max(14, Math.round(mainPx * 0.62));
  const weight = activeTypography.value.fontWeight;
  const align: "left" | "center" =
    layoutMode.value === "left" ? "left" : "center";
  const { top: topPct, bottom: bottomPct } = dualRowVerticalPct.value;

  if (!lines.length || activeLineIndex.value < 0) {
    const isPrelude = lines.length > 0 && activeLineIndex.value < 0;
    const titleText = currentSong.value?.title
      ? `${currentSong.value.title}${
          currentSong.value.artist ? ` · ${currentSong.value.artist}` : ""
        }`
      : "微风音乐 · 帧享级桌面流光歌词";
    const subText = isPrelude
      ? lines[0]?.text
        ? `♪ 即将演唱 · ${lines[0].text}`
        : currentSong.value?.artist || "前奏演奏中"
      : currentSong.value
      ? "纯音乐或暂无匹配歌词 · 尽享纯净声场"
      : "播放任意曲目即可毫秒级实时同步逐字歌词";

    if (layoutMode.value === "single") {
      return {
        isThreeRow: false,
        rows: [
          {
            key: `idle-single-${titleText}`,
            text: isPrelude && lines[0]?.text ? lines[0].text : titleText,
            words: [],
            lineStartMs: 0,
            lineDurationMs: 0,
            isActiveKaraoke: true,
            isSecondary: false,
            verticalPct: 50,
            alignOverride: align,
            fitScale: computeRowFitScale(titleText, mainPx, weight, maxWidth),
          },
        ],
      };
    }

    return {
      isThreeRow: false,
      rows: [
        {
          key: `idle-main-${titleText}`,
          text: titleText,
          words: [],
          lineStartMs: 0,
          lineDurationMs: 0,
          isActiveKaraoke: true,
          isSecondary: false,
          verticalPct: topPct,
          alignOverride: align,
          fitScale: computeRowFitScale(titleText, mainPx, weight, maxWidth),
        },
        {
          key: `idle-sub-${subText}`,
          text: subText,
          words: [],
          lineStartMs: 0,
          lineDurationMs: 0,
          isActiveKaraoke: false,
          isSecondary: true,
          verticalPct: bottomPct,
          alignOverride: align,
          fitScale: computeRowFitScale(subText, subPx, 600, maxWidth),
        },
      ],
    };
  }

  const idx = activeLineIndex.value;
  const active = lines[idx];
  const next = lines[idx + 1] || null;
  const mainText = (active.text || "").trim() || "♪";
  const transText = showTranslation.value
    ? (active.translation || "").trim()
    : "";
  const phoneticText = showPhonetic.value
    ? (active.phonetic || "").trim()
    : "";
  const nextText = (next?.text || "").trim();
  const isSingleLineOnly = layoutMode.value === "single";
  const hasBothSubLines =
    !isSingleLineOnly && Boolean(transText) && Boolean(phoneticText);
  const secondaryText = isSingleLineOnly
    ? ""
    : transText || phoneticText || (nextText ? nextText : "");

  const useWords = wordByWordEnabled.value ? active.words || [] : [];

  if (isSingleLineOnly) {
    return {
      isThreeRow: false,
      rows: [
        {
          key: `l-${idx}-single`,
          text: mainText,
          words: useWords,
          lineStartMs: active.startTimeMs,
          lineDurationMs: active.durationMs,
          isActiveKaraoke: true,
          isSecondary: false,
          verticalPct: 50,
          alignOverride: "center",
          fitScale: computeRowFitScale(mainText, mainPx, weight, maxWidth),
        },
      ],
    };
  }

  if (layoutMode.value === "ktv") {
    const activeOnTop = idx % 2 === 0;
    const rows: DisplayRow[] = [];
    rows.push({
      key: `ktv-line-${idx}`,
      text: mainText,
      words: useWords,
      lineStartMs: active.startTimeMs,
      lineDurationMs: active.durationMs,
      isActiveKaraoke: true,
      isSecondary: false,
      verticalPct: activeOnTop ? topPct : bottomPct,
      alignOverride: activeOnTop ? "left" : "center",
      fitScale: computeRowFitScale(mainText, mainPx, weight, maxWidth),
    });

    if (nextText) {
      rows.push({
        key: `ktv-line-${idx + 1}`,
        text: nextText,
        words: wordByWordEnabled.value ? next?.words || [] : [],
        lineStartMs: next?.startTimeMs || 0,
        lineDurationMs: next?.durationMs || 0,
        isActiveKaraoke: false,
        isSecondary: true,
        isPreviewNext: true,
        verticalPct: activeOnTop ? bottomPct : topPct,
        alignOverride: activeOnTop ? "center" : "left",
        fitScale: computeRowFitScale(nextText, subPx, 600, maxWidth),
      });
    }
    rows.sort((a, b) => a.verticalPct - b.verticalPct);
    return { isThreeRow: false, rows };
  }

  if (hasBothSubLines) {
    return {
      isThreeRow: true,
      rows: [
        {
          key: `l-${idx}-main`,
          text: mainText,
          words: useWords,
          lineStartMs: active.startTimeMs,
          lineDurationMs: active.durationMs,
          isActiveKaraoke: true,
          isSecondary: false,
          verticalPct: 22,
          alignOverride: align,
          fitScale: computeRowFitScale(mainText, mainPx, weight, maxWidth),
        },
        {
          key: `l-${idx}-phonetic`,
          text: phoneticText,
          words: [],
          lineStartMs: 0,
          lineDurationMs: 0,
          isActiveKaraoke: false,
          isSecondary: true,
          isPreviewNext: true,
          verticalPct: 58,
          alignOverride: align,
          fitScale: computeRowFitScale(phoneticText, subPx, 600, maxWidth),
        },
        {
          key: `l-${idx}-trans`,
          text: transText,
          words: [],
          lineStartMs: 0,
          lineDurationMs: 0,
          isActiveKaraoke: false,
          isSecondary: true,
          isPreviewNext: false,
          verticalPct: 84,
          alignOverride: align,
          fitScale: computeRowFitScale(transText, subPx, 600, maxWidth),
        },
      ],
    };
  }

  const mainWidth = measureTextWidthPx(mainText, mainPx, weight);
  const isLongMain = mainWidth > maxWidth * 0.98 && mainText.length >= 8;

  if (isLongMain) {
    const [p1, p2] = splitLongLineSemantically(
      { ...active, words: useWords },
      maxWidth,
      mainPx,
      weight
    );
    if (!secondaryText) {
      return {
        isThreeRow: false,
        rows: [
          {
            key: `l-${idx}-p1`,
            text: p1.text,
            words: p1.words,
            lineStartMs: p1.startTimeMs,
            lineDurationMs: p1.durationMs,
            isActiveKaraoke: true,
            isSecondary: false,
            verticalPct: topPct,
            alignOverride: align,
            fitScale: computeRowFitScale(p1.text, mainPx, weight, maxWidth),
          },
          {
            key: `l-${idx}-p2`,
            text: p2.text,
            words: p2.words,
            lineStartMs: p2.startTimeMs,
            lineDurationMs: p2.durationMs,
            isActiveKaraoke: true,
            isSecondary: false,
            verticalPct: bottomPct,
            alignOverride: align,
            fitScale: computeRowFitScale(p2.text, mainPx, weight, maxWidth),
          },
        ],
      };
    }

    return {
      isThreeRow: true,
      rows: [
        {
          key: `l-${idx}-p1`,
          text: p1.text,
          words: p1.words,
          lineStartMs: p1.startTimeMs,
          lineDurationMs: p1.durationMs,
          isActiveKaraoke: true,
          isSecondary: false,
          verticalPct: 18,
          alignOverride: align,
          fitScale: computeRowFitScale(p1.text, mainPx, weight, maxWidth),
        },
        {
          key: `l-${idx}-p2`,
          text: p2.text,
          words: p2.words,
          lineStartMs: p2.startTimeMs,
          lineDurationMs: p2.durationMs,
          isActiveKaraoke: true,
          isSecondary: false,
          verticalPct: 50,
          alignOverride: align,
          fitScale: computeRowFitScale(p2.text, mainPx, weight, maxWidth),
        },
        {
          key: `l-${idx}-sub`,
          text: secondaryText,
          words: [],
          lineStartMs: 0,
          lineDurationMs: 0,
          isActiveKaraoke: false,
          isSecondary: true,
          isPreviewNext: !transText && !phoneticText && Boolean(nextText),
          verticalPct: 82,
          alignOverride: align,
          fitScale: computeRowFitScale(secondaryText, subPx, 600, maxWidth),
        },
      ],
    };
  }

  const rows: DisplayRow[] = [
    {
      key: `l-${idx}-main`,
      text: mainText,
      words: useWords,
      lineStartMs: active.startTimeMs,
      lineDurationMs: active.durationMs,
      isActiveKaraoke: true,
      isSecondary: false,
      verticalPct: secondaryText ? topPct : 50,
      alignOverride: align,
      fitScale: computeRowFitScale(mainText, mainPx, weight, maxWidth),
    },
  ];

  if (secondaryText) {
    rows.push({
      key: `l-${idx}-sub-${transText ? "t" : phoneticText ? "p" : "n"}`,
      text: secondaryText,
      words: [],
      lineStartMs: 0,
      lineDurationMs: 0,
      isActiveKaraoke: false,
      isSecondary: true,
      isPreviewNext: !transText && !phoneticText && Boolean(nextText),
      verticalPct: bottomPct,
      alignOverride: align,
      fitScale: computeRowFitScale(secondaryText, subPx, 600, maxWidth),
    });
  }

  return { isThreeRow: false, rows };
});

// ==================== 真光学双层矢量描边 + 无极卡拉OK渲染引擎 ====================
function smootherstep(x: number): number {
  const t = clamp(x, 0, 1);
  return t * t * t * (t * (t * 6 - 15) + 10);
}

interface DualLayerRenderBundle {
  wrapStyle: Record<string, string>;
  strokeStyle: Record<string, string> | null;
  fillStyle: Record<string, string>;
}

function buildSoftShadowFilter(isBrightText: boolean): string {
  if (!shadowEnabled.value) return "";
  return isBrightText
    ? "drop-shadow(0 2px 6px rgba(0, 0, 0, 0.56))"
    : "drop-shadow(0 2px 6px rgba(255, 255, 255, 0.72))";
}

function getWordDualRender(
  row: DisplayRow,
  word: LyricWordItem
): DualLayerRenderBundle {
  const { primary: start, secondary: end } = activeSungPalette.value;
  const unsungSolid = activeUnsungHex.value;
  const uOp = unsungOpacity.value;
  const glowRatio = clamp(glowIntensity.value / 100, 0, 1.5);
  const sungStroke = sungStrokeSpec.value;
  const unsungStroke = unsungStrokeSpec.value;
  const heavySungStroke = activeTypography.value.opticalStrokeCss;
  const heavyUnsungStroke = activeUnsungTypography.value.opticalStrokeCss;

  const makeBackLayer = (
    spec: { strokePx: number; strokeColor: string },
    brightText: boolean
  ): Record<string, string> | null => {
    if (spec.strokePx <= 0) return null;
    const shadowF = buildSoftShadowFilter(brightText);
    return {
      WebkitTextStroke: `${spec.strokePx}px ${spec.strokeColor}`,
      color: "transparent",
      WebkitTextFillColor: "transparent",
      filter: shadowF || "none",
    };
  };

  if (!row.isActiveKaraoke) {
    const back = makeBackLayer(unsungStroke, isBrightHex(unsungSolid));
    const directShadow =
      !back && shadowEnabled.value
        ? buildSoftShadowFilter(isBrightHex(unsungSolid))
        : "none";
    return {
      wrapStyle: {
        opacity: uOp.toFixed(2),
        transform: "translate3d(0, 0px, 0) scale(1)",
      },
      strokeStyle: back,
      fillStyle: {
        backgroundImage: `linear-gradient(90deg, ${unsungSolid} 0%, ${unsungSolid} 100%)`,
        WebkitTextStroke: heavyUnsungStroke,
        filter: directShadow,
      },
    };
  }

  const pos = calibratedPosMs.value;
  const absStart =
    word.startTimeMs >= row.lineStartMs
      ? word.startTimeMs
      : row.lineStartMs + word.startTimeMs;
  const dur = Math.max(55, word.durationMs || 140);
  const rawP = clamp((pos - absStart) / dur, 0, 1);

  if (rawP <= 0) {
    const back = makeBackLayer(unsungStroke, isBrightHex(unsungSolid));
    const directShadow =
      !back && shadowEnabled.value
        ? buildSoftShadowFilter(isBrightHex(unsungSolid))
        : "none";
    return {
      wrapStyle: {
        opacity: uOp.toFixed(2),
        transform: "translate3d(0, 0px, 0) scale(1)",
      },
      strokeStyle: back,
      fillStyle: {
        backgroundImage: `linear-gradient(90deg, ${unsungSolid} 0%, ${unsungSolid} 100%)`,
        WebkitTextStroke: heavyUnsungStroke,
        filter: directShadow,
      },
    };
  }

  if (rawP >= 1) {
    const back = makeBackLayer(sungStroke, isBrightHex(start));
    const filters: string[] = [];
    if (!back && shadowEnabled.value) {
      filters.push(buildSoftShadowFilter(isBrightHex(start)));
    }
    if (glowRatio > 0.05) {
      const sungGlowRadius = Math.round(7 * glowRatio);
      const sungGlowAlpha = clamp(0.34 * glowRatio, 0, 0.72);
      filters.push(
        `drop-shadow(0 0 ${sungGlowRadius}px ${hexToRgba(start, sungGlowAlpha)})`
      );
    }
    return {
      wrapStyle: {
        opacity: "1",
        transform: "translate3d(0, -0.9px, 0) scale(1.008)",
      },
      strokeStyle: back,
      fillStyle: {
        backgroundImage: `linear-gradient(90deg, ${start} 0%, ${end} 100%)`,
        WebkitTextStroke: heavySungStroke,
        filter: filters.length ? filters.join(" ") : "none",
      },
    };
  }

  // 逐字扫光过渡态
  const smoothP = smootherstep(rawP);
  const bell = Math.sin(rawP * Math.PI);
  const feather = dur >= 400 ? 18 : 13;
  const center = -feather + smoothP * (100 + feather * 2);
  const leftPct = clamp(center - feather, 0, 100);
  const rightPct = clamp(center + feather, 0, 100);

  const ty = -0.9 * smoothP - 2.1 * bell;
  const scale = 1 + 0.008 * smoothP + 0.026 * bell;
  const isLongNote = dur >= 580;
  const back = makeBackLayer(sungStroke, isBrightHex(start));
  const filters: string[] = [];
  if (!back && shadowEnabled.value) {
    filters.push(buildSoftShadowFilter(isBrightHex(start)));
  }
  if (glowRatio > 0.05) {
    const activeGlowRadius = Math.round(
      ((isLongNote ? 14 : 10) * bell + 5 * smoothP) * Math.max(0.2, glowRatio)
    );
    const activeGlowAlpha = clamp(
      (0.34 + (isLongNote ? 0.38 : 0.24) * bell) * glowRatio,
      0,
      0.82
    );
    filters.push(
      `drop-shadow(0 0 ${activeGlowRadius}px ${hexToRgba(end, activeGlowAlpha)})`
    );
  }
  const unsungHalf = hexToRgba(unsungSolid, clamp(uOp, 0.45, 1));

  return {
    wrapStyle: {
      opacity: "1",
      transform: `translate3d(0, ${ty.toFixed(2)}px, 0) scale(${scale.toFixed(4)})`,
    },
    strokeStyle: back,
    fillStyle: {
      backgroundImage: `linear-gradient(90deg, ${start} 0%, ${end} ${leftPct.toFixed(
        2
      )}%, ${unsungHalf} ${rightPct.toFixed(2)}%, ${unsungHalf} 100%)`,
      WebkitTextStroke: heavySungStroke,
      filter: filters.length ? filters.join(" ") : "none",
    },
  };
}

function getPlainRowDualRender(row: DisplayRow): DualLayerRenderBundle {
  const { primary: start, secondary: end } = activeSungPalette.value;
  const unsungSolid = activeUnsungHex.value;
  const uOp = row.isPreviewNext
    ? clamp(unsungOpacity.value * 0.84, 0.3, 0.96)
    : unsungOpacity.value;
  const glowRatio = clamp(glowIntensity.value / 100, 0, 1.5);
  const spec = row.isSecondary
    ? subStrokeSpec.value
    : row.isActiveKaraoke
    ? sungStrokeSpec.value
    : unsungStrokeSpec.value;

  const makeBackLayer = (brightText: boolean): Record<string, string> | null => {
    if (spec.strokePx <= 0) return null;
    const shadowF = buildSoftShadowFilter(brightText);
    return {
      WebkitTextStroke: `${spec.strokePx}px ${spec.strokeColor}`,
      color: "transparent",
      WebkitTextFillColor: "transparent",
      filter: shadowF || "none",
    };
  };

  if (!row.isActiveKaraoke) {
    const bright = isBrightHex(unsungSolid);
    const back = makeBackLayer(bright);
    return {
      wrapStyle: { opacity: uOp.toFixed(2) },
      strokeStyle: back,
      fillStyle: {
        backgroundImage: `linear-gradient(90deg, ${unsungSolid} 0%, ${unsungSolid} 100%)`,
        WebkitTextStroke: row.isSecondary
          ? "0px transparent"
          : activeUnsungTypography.value.opticalStrokeCss,
        filter:
          !back && shadowEnabled.value ? buildSoftShadowFilter(bright) : "none",
      },
    };
  }

  const brightSung = isBrightHex(start);
  const back = makeBackLayer(brightSung);

  // 关闭逐字染色时整行纯净高亮
  if (
    !wordByWordEnabled.value ||
    !row.lineDurationMs ||
    row.lineDurationMs <= 0 ||
    activeLineIndex.value < 0
  ) {
    const filters: string[] = [];
    if (!back && shadowEnabled.value) {
      filters.push(buildSoftShadowFilter(brightSung));
    }
    if (glowRatio > 0.05) {
      const glowR = Math.round(9 * glowRatio);
      filters.push(
        `drop-shadow(0 0 ${glowR}px ${hexToRgba(start, 0.34 * glowRatio)})`
      );
    }
    return {
      wrapStyle: { opacity: "1" },
      strokeStyle: back,
      fillStyle: {
        backgroundImage: `linear-gradient(90deg, ${start} 0%, ${end} 100%)`,
        WebkitTextStroke: activeTypography.value.opticalStrokeCss,
        filter: filters.length ? filters.join(" ") : "none",
      },
    };
  }

  const pos = calibratedPosMs.value;
  const rawP = clamp(
    (pos - row.lineStartMs) / Math.max(400, row.lineDurationMs),
    0,
    1
  );
  const smoothP = smootherstep(rawP);
  const feather = 10;
  const center = -feather + smoothP * (100 + feather * 2);
  const leftPct = clamp(center - feather, 0, 100);
  const rightPct = clamp(center + feather, 0, 100);
  const filters: string[] = [];
  if (!back && shadowEnabled.value) {
    filters.push(buildSoftShadowFilter(brightSung));
  }
  if (glowRatio > 0.05) {
    const glowR = Math.round(10 * glowRatio);
    filters.push(
      `drop-shadow(0 0 ${glowR}px ${hexToRgba(start, 0.3 * glowRatio)})`
    );
  }
  const unsungHalf = hexToRgba(unsungSolid, clamp(unsungOpacity.value, 0.45, 1));

  return {
    wrapStyle: { opacity: "1" },
    strokeStyle: back,
    fillStyle: {
      backgroundImage: `linear-gradient(90deg, ${start} 0%, ${end} ${leftPct.toFixed(
        2
      )}%, ${unsungHalf} ${rightPct.toFixed(2)}%, ${unsungHalf} 100%)`,
      WebkitTextStroke: activeTypography.value.opticalStrokeCss,
      filter: filters.length ? filters.join(" ") : "none",
    },
  };
}

// ==================== Tauri 原生窗口尺寸（支持中心锚定平滑缩放）与拖拽管理 ====================
function getCurrentTargetWindowHeightPx(): number {
  const baseH = baseWindowHeightPx.value;
  if (isSettingsOpen.value && !isLocked.value) {
    const drawerH = settingsInnerRef.value?.offsetHeight || 226;
    return baseH + drawerH + 8;
  }
  return baseH;
}

async function resizeOsWindow(
  targetW: number,
  targetH: number,
  anchorCenter = false
) {
  const safeW = Math.round(clamp(targetW, MIN_WIDTH_PX, MAX_WIDTH_PX));
  const safeH = Math.round(clamp(targetH, 88, 680));
  try {
    await invoke("resize_desktop_lyric_window_cmd", {
      width: safeW,
      height: safeH,
      anchorCenter,
    });
    return;
  } catch {
    // fallback to WebviewWindow API
  }
  try {
    const win = getCurrentWindow();
    await win.setSize(new LogicalSize(safeW, safeH));
  } catch {
    // 浏览器预览下忽略
  }
}

async function syncOsWindowWithState(anchorCenter = false) {
  if (isSettingsOpen.value && !isLocked.value) {
    await nextTick();
  }
  await resizeOsWindow(
    widthPx.value,
    getCurrentTargetWindowHeightPx(),
    anchorCenter
  );
}

// 平滑宽幅滑块：中心锚定 + rAF 合并原生缩放 + 锁定悬停态不误关
function handleWidthSliderDown() {
  isAdjustingWidth.value = true;
  if (hoverLeaveTimer) {
    clearTimeout(hoverLeaveTimer);
    hoverLeaveTimer = null;
  }
  const finish = () => {
    if (!isAdjustingWidth.value) return;
    isAdjustingWidth.value = false;
    window.removeEventListener("pointerup", finish);
    window.removeEventListener("pointercancel", finish);
    window.removeEventListener("mouseup", finish);
    flushWidthCommandNow();
  };
  window.addEventListener("pointerup", finish, { once: true });
  window.addEventListener("pointercancel", finish, { once: true });
  window.addEventListener("mouseup", finish, { once: true });
}

function flushWidthCommandNow() {
  if (widthCmdDebounceTimer) {
    clearTimeout(widthCmdDebounceTimer);
    widthCmdDebounceTimer = null;
  }
  localStorage.setItem("bm_desktop_lyric_width_v2", String(widthPx.value));
  sendCmd("CMD_SET_WIDTH", { width: widthPx.value });
}

function handleWidthSliderInput(nextWidth: number) {
  lastLocalControlMutateAt = performance.now();
  const next = clamp(
    Math.round(nextWidth / 10) * 10,
    MIN_WIDTH_PX,
    MAX_WIDTH_PX
  );
  if (next === widthPx.value) return;
  widthPx.value = next;
  lastSyncedWidth = next;
  localStorage.setItem("bm_desktop_lyric_width_v2", String(next));

  if (!widthResizeRafPending) {
    widthResizeRafPending = true;
    requestAnimationFrame(() => {
      widthResizeRafPending = false;
      resizeOsWindow(widthPx.value, getCurrentTargetWindowHeightPx(), true);
    });
  }

  if (widthCmdDebounceTimer) clearTimeout(widthCmdDebounceTimer);
  widthCmdDebounceTimer = setTimeout(() => {
    widthCmdDebounceTimer = null;
    sendCmd("CMD_SET_WIDTH", { width: widthPx.value });
  }, 130);
}

function handleWidthSliderCommit(nextWidth: number) {
  handleWidthSliderInput(nextWidth);
  flushWidthCommandNow();
}

async function handleStartDrag(e: MouseEvent) {
  if (isLocked.value || e.button !== 0) return;
  const target = e.target as HTMLElement | null;
  if (
    target &&
    target.closest("button, input, label, [data-no-drag], .dl-edge-resizer")
  )
    return;
  isDraggingOsWindow = true;
  if (hoverLeaveTimer) {
    clearTimeout(hoverLeaveTimer);
    hoverLeaveTimer = null;
  }
  try {
    await getCurrentWindow().startDragging();
    await invoke("save_desktop_lyric_position_cmd").catch(() => {});
  } catch {
    // 浏览器预览忽略
  } finally {
    isDraggingOsWindow = false;
    if (!isLocked.value) {
      isPointerInside.value = true;
      isHovering.value = true;
    }
  }
}

function handleViewportDoubleClick(e: MouseEvent) {
  if (isLocked.value) return;
  const target = e.target as HTMLElement | null;
  if (target && target.closest("button, input, label, [data-no-drag]")) return;
  if (layoutMode.value === "single") {
    setLayoutModeVal(lastDoubleLayout.value || "center");
    setHoverTip("已切换至双行歌词显示", "center");
  } else {
    lastDoubleLayout.value = layoutMode.value;
    setLayoutModeVal("single");
    setHoverTip("已切换至纯净单行模式（再次双击恢复）", "center");
  }
}

function handleWheelZoom(e: WheelEvent) {
  if (isLocked.value || !e.ctrlKey) return;
  e.preventDefault();
  const step = e.deltaY < 0 ? 2 : -2;
  setFontSizeVal(fontSize.value + step);
  setHoverTip(`桌面字号：${Math.round(fontSize.value)}px`, "center");
}

function startEdgeResize(side: "left" | "right", e: MouseEvent) {
  if (isLocked.value || e.button !== 0) return;
  e.preventDefault();
  e.stopPropagation();
  isResizingEdge.value = true;
  const startX = e.screenX;
  const startW = widthPx.value;
  let rafPending = false;

  const onMove = (ev: MouseEvent) => {
    const dx = ev.screenX - startX;
    const deltaW = side === "right" ? dx * 1.6 : -dx * 1.6;
    const nextW = clamp(
      Math.round((startW + deltaW) / 4) * 4,
      MIN_WIDTH_PX,
      MAX_WIDTH_PX
    );
    if (nextW !== widthPx.value) {
      widthPx.value = nextW;
      lastSyncedWidth = nextW;
      lastLocalControlMutateAt = performance.now();
      if (!rafPending) {
        rafPending = true;
        requestAnimationFrame(() => {
          rafPending = false;
          resizeOsWindow(widthPx.value, getCurrentTargetWindowHeightPx(), true);
        });
      }
    }
  };

  const onUp = () => {
    isResizingEdge.value = false;
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    localStorage.setItem("bm_desktop_lyric_width_v2", String(widthPx.value));
    sendCmd("CMD_SET_WIDTH", { width: widthPx.value });
    syncOsWindowWithState(true);
  };

  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

// ==================== 顶部操作与双向同步命令 ====================
function sendCmd(type: string, extra: Record<string, any> = {}) {
  const payload = { type, ...extra };
  if (channel) {
    try {
      channel.postMessage(payload);
    } catch {
      // ignore
    }
  }
  invoke("send_desktop_lyric_command_cmd", { command: payload }).catch(
    () => {}
  );
}

function handlePointerEnter() {
  if (isLocked.value) return;
  isPointerInside.value = true;
  if (hoverLeaveTimer) {
    clearTimeout(hoverLeaveTimer);
    hoverLeaveTimer = null;
  }
  isHovering.value = true;
}

function handlePointerMove() {
  if (isLocked.value) return;
  isPointerInside.value = true;
  if (hoverLeaveTimer) {
    clearTimeout(hoverLeaveTimer);
    hoverLeaveTimer = null;
  }
  if (!isHovering.value) {
    isHovering.value = true;
  }
}

function handlePointerLeave() {
  isPointerInside.value = false;
  if (isResizingEdge.value || isDraggingOsWindow || isAdjustingWidth.value)
    return;
  if (hoverLeaveTimer) clearTimeout(hoverLeaveTimer);
  hoverLeaveTimer = setTimeout(() => {
    if (
      !isPointerInside.value &&
      !isResizingEdge.value &&
      !isDraggingOsWindow &&
      !isAdjustingWidth.value
    ) {
      isHovering.value = false;
      hoverTip.value = "";
    }
  }, 140);
}

function handleWindowBlur() {
  if (
    isPointerInside.value ||
    isResizingEdge.value ||
    isDraggingOsWindow ||
    isAdjustingWidth.value
  )
    return;
  if (isSettingsOpen.value) {
    closeSettingsPanel();
  }
  isHovering.value = false;
  hoverTip.value = "";
}

function handleLockClick() {
  lastLocalControlMutateAt = performance.now();
  isSettingsOpen.value = false;
  isHovering.value = false;
  isPointerInside.value = false;
  hoverTip.value = "";
  isLocked.value = true;
  localStorage.setItem("bm_desktop_lyric_locked", "true");
  resizeOsWindow(widthPx.value, baseWindowHeightPx.value, false);
  sendCmd("CMD_LOCK");
}

function handleUnlockClick() {
  lastLocalControlMutateAt = performance.now();
  isLocked.value = false;
  lockedHoverWindow.value = false;
  lockedHoverUnlock.value = false;
  isHovering.value = true;
  localStorage.setItem("bm_desktop_lyric_locked", "false");
  hoverTip.value = "";
  sendCmd("CMD_UNLOCK");
  resizeOsWindow(widthPx.value, baseWindowHeightPx.value, false);
}

function setHoverTip(text: string, align: "left" | "center" | "right") {
  hoverTip.value = text;
  hoverTipAlign.value = align;
}

function clearHoverTip() {
  hoverTip.value = "";
}

function toggleTranslation() {
  lastLocalControlMutateAt = performance.now();
  const next = !showTranslation.value;
  showTranslation.value = next;
  lastSyncedTrans = next;
  localStorage.setItem("bm_desktop_lyric_trans", String(next));
  sendCmd("CMD_SET_SHOW_TRANSLATION", { showTranslation: next });
  setHoverTip(next ? "双语译文：已开启" : "双语译文：已关闭", "left");
}

// 已唱双通道设置
function selectSungPreset(id: LyricHighlightThemeId) {
  lastLocalControlMutateAt = performance.now();
  colorSchemeId.value = id;
  localStorage.setItem("bm_desktop_lyric_scheme", id);
  sendCmd("CMD_SET_SUNG_COLOR", { schemeId: id });
}

function setSungModeVal(mode: LyricSungColorMode) {
  lastLocalControlMutateAt = performance.now();
  sungMode.value = mode;
  localStorage.setItem("bm_dl_sung_mode_v2", mode);
  sendCmd("CMD_SET_SUNG_COLOR", { sungMode: mode });
}

function pickCustomSungPrimary(hex: string) {
  lastLocalControlMutateAt = performance.now();
  customPrimaryHex.value = hex;
  colorSchemeId.value = "custom";
  localStorage.setItem("bm_dl_custom_primary_v2", hex);
  localStorage.setItem("bm_desktop_lyric_color", hex);
  localStorage.setItem("bm_desktop_lyric_scheme", "custom");
  sendCmd("CMD_SET_SUNG_COLOR", { schemeId: "custom", customPrimary: hex });
}

function pickCustomSungSecondary(hex: string) {
  lastLocalControlMutateAt = performance.now();
  customSecondaryHex.value = hex;
  colorSchemeId.value = "custom";
  localStorage.setItem("bm_dl_custom_secondary_v2", hex);
  localStorage.setItem("bm_desktop_lyric_scheme", "custom");
  sendCmd("CMD_SET_SUNG_COLOR", { schemeId: "custom", customSecondary: hex });
}

// 未唱独立设置
function selectUnsungPreset(id: LyricUnsungPresetId) {
  lastLocalControlMutateAt = performance.now();
  unsungPreset.value = id;
  localStorage.setItem("bm_dl_unsung_preset_v2", id);
  sendCmd("CMD_SET_UNSUNG_COLOR", { unsungPreset: id });
}

function pickCustomUnsungHex(hex: string) {
  lastLocalControlMutateAt = performance.now();
  unsungCustomHex.value = hex;
  unsungPreset.value = "custom";
  localStorage.setItem("bm_dl_unsung_custom_v2", hex);
  localStorage.setItem("bm_dl_unsung_preset_v2", "custom");
  sendCmd("CMD_SET_UNSUNG_COLOR", {
    unsungPreset: "custom",
    unsungCustomHex: hex,
  });
}

function setUnsungOpacityVal(val: number) {
  lastLocalControlMutateAt = performance.now();
  const clamped = Number(clamp(val, 0.35, 0.98).toFixed(2));
  unsungOpacity.value = clamped;
  localStorage.setItem("bm_dl_unsung_opacity_v2", String(clamped));
  sendCmd("CMD_SET_UNSUNG_COLOR", { unsungOpacity: clamped });
}

// 光学矢量描边与立体柔影设置
function setStrokeWidthVal(w: DesktopLyricStrokeWidth) {
  lastLocalControlMutateAt = performance.now();
  strokeWidth.value = w;
  localStorage.setItem("bm_dl_stroke_width_v2", w);
  sendCmd("CMD_SET_STROKE", { strokeWidth: w });
}

function setStrokeColorModeVal(c: DesktopLyricStrokeColorMode) {
  lastLocalControlMutateAt = performance.now();
  strokeColorMode.value = c;
  localStorage.setItem("bm_dl_stroke_color_mode_v2", c);
  sendCmd("CMD_SET_STROKE", { strokeColorMode: c });
}

function pickCustomStrokeHex(hex: string) {
  lastLocalControlMutateAt = performance.now();
  strokeCustomHex.value = hex;
  strokeColorMode.value = "custom";
  localStorage.setItem("bm_dl_stroke_custom_v2", hex);
  localStorage.setItem("bm_dl_stroke_color_mode_v2", "custom");
  sendCmd("CMD_SET_STROKE", {
    strokeColorMode: "custom",
    strokeCustomHex: hex,
  });
}

function toggleShadowEnabled() {
  lastLocalControlMutateAt = performance.now();
  const next = !shadowEnabled.value;
  shadowEnabled.value = next;
  localStorage.setItem("bm_dl_shadow_enabled_v2", String(next));
  sendCmd("CMD_SET_STROKE", { shadowEnabled: next });
}

// 字形、字重、字号与辉光设置
function setFontFamilyVal(ff: LyricFontFamily) {
  lastLocalControlMutateAt = performance.now();
  fontFamily.value = ff;
  localStorage.setItem("bm_desktop_lyric_font_family", ff);
  sendCmd("CMD_SET_FONT_FAMILY", { fontFamily: ff });
}

function setFontWeightVal(fw: LyricFontWeightLevel) {
  lastLocalControlMutateAt = performance.now();
  fontWeight.value = fw;
  localStorage.setItem("bm_desktop_lyric_font_weight", String(fw));
  sendCmd("CMD_SET_FONT_WEIGHT", { fontWeight: fw });
}

function setFontSizeVal(nextSize: number) {
  lastLocalControlMutateAt = performance.now();
  const clamped = Math.round(clamp(nextSize, 18, 56));
  fontSize.value = clamped;
  lastSyncedFontSize = clamped;
  localStorage.setItem("bm_desktop_lyric_size_v2", String(clamped));
  sendCmd("CMD_SET_FONT_SIZE", { fontSize: clamped });
}

function setGlowVal(val: number) {
  lastLocalControlMutateAt = performance.now();
  const clamped = clamp(Math.round(val), 0, 150);
  glowIntensity.value = clamped;
  localStorage.setItem("bm_desktop_lyric_glow_v2", String(clamped));
  sendCmd("CMD_SET_GLOW", { glowIntensity: clamped });
}

// 排版、行距、透明度与开关
function setLayoutModeVal(mode: DesktopLyricLayoutMode) {
  lastLocalControlMutateAt = performance.now();
  layoutMode.value = mode;
  if (mode !== "single") lastDoubleLayout.value = mode;
  localStorage.setItem("bm_desktop_lyric_layout_v2", mode);
  localStorage.setItem(
    "bm_desktop_lyric_line_mode",
    mode === "single" ? "single" : "double"
  );
  sendCmd("CMD_SET_LAYOUT_MODE", { layoutMode: mode });
}

function setLyricOpacityVal(val: number) {
  lastLocalControlMutateAt = performance.now();
  const clamped = Number(clamp(val, 0.35, 1.0).toFixed(2));
  lyricOpacity.value = clamped;
  localStorage.setItem("bm_desktop_lyric_opacity", String(clamped));
  sendCmd("CMD_SET_OPACITY", { opacity: clamped });
}

function setLineSpacingVal(val: number) {
  lastLocalControlMutateAt = performance.now();
  const clamped = clamp(Math.round(val), 0, 16);
  lineSpacing.value = clamped;
  localStorage.setItem("bm_desktop_lyric_line_spacing", String(clamped));
  sendCmd("CMD_SET_LINE_SPACING", { lineSpacing: clamped });
}

function toggleIdleFade() {
  lastLocalControlMutateAt = performance.now();
  const next = !idleFade.value;
  idleFade.value = next;
  localStorage.setItem("bm_desktop_lyric_idle_fade", String(next));
  sendCmd("CMD_SET_IDLE_FADE", { idleFade: next });
}

function toggleShowIntro() {
  lastLocalControlMutateAt = performance.now();
  const next = !showIntro.value;
  showIntro.value = next;
  localStorage.setItem("bm_desktop_lyric_intro", String(next));
  sendCmd("CMD_SET_SHOW_INTRO", { showIntro: next });
}

function togglePhonetic() {
  lastLocalControlMutateAt = performance.now();
  const next = !showPhonetic.value;
  showPhonetic.value = next;
  localStorage.setItem("bm_desktop_lyric_phonetic", String(next));
  sendCmd("CMD_SET_SHOW_PHONETIC", { showPhonetic: next });
}

function toggleWordByWord() {
  lastLocalControlMutateAt = performance.now();
  const next = !wordByWordEnabled.value;
  wordByWordEnabled.value = next;
  localStorage.setItem("bm_desktop_lyric_krc", String(next));
  sendCmd("CMD_SET_WORD_BY_WORD", { wordByWord: next });
}

function adjustOffsetStep(deltaMs: number) {
  lyricOffsetMs.value = clamp(lyricOffsetMs.value + deltaMs, -15000, 15000);
  localStorage.setItem("bm_lyric_offset_ms", String(lyricOffsetMs.value));
  sendCmd("CMD_ADJUST_OFFSET", { deltaMs });
}

function resetOffsetToZero() {
  lyricOffsetMs.value = 0;
  localStorage.setItem("bm_lyric_offset_ms", "0");
  sendCmd("CMD_RESET_OFFSET");
}

function handleResetDefaults() {
  lastLocalControlMutateAt = performance.now();
  colorSchemeId.value = "dynamic_cover";
  sungMode.value = "gradient";
  customPrimaryHex.value = "#5EEAD4";
  customSecondaryHex.value = "#60A5FA";
  unsungPreset.value = "auto";
  unsungCustomHex.value = "#E6ECF5";
  unsungOpacity.value = 0.78;
  strokeWidth.value = "medium";
  strokeColorMode.value = "auto";
  strokeCustomHex.value = "#090D16";
  shadowEnabled.value = true;
  fontFamily.value = "sans";
  fontWeight.value = 760;
  fontSize.value = 32;
  glowIntensity.value = 80;
  layoutMode.value = "center";
  lyricOpacity.value = 1.0;
  lineSpacing.value = 6;
  idleFade.value = false;
  showIntro.value = true;
  showTranslation.value = true;
  showPhonetic.value = false;
  wordByWordEnabled.value = true;
  sendCmd("CMD_RESET_DEFAULTS");
  setHoverTip("已恢复桌面歌词默认样式", "right");
}

function focusMainWindow() {
  sendCmd("CMD_FOCUS_MAIN_WINDOW");
  invoke("focus_main_window_cmd").catch(() => {});
}

watch(baseWindowHeightPx, () => {
  if (!shrinkWindowTimer && !isAdjustingWidth.value) {
    syncOsWindowWithState(false);
  }
});

watch(settingsTab, () => {
  if (isSettingsOpen.value && !isLocked.value) {
    nextTick(() => syncOsWindowWithState(false));
  }
});

async function openSettingsPanel() {
  if (shrinkWindowTimer) {
    clearTimeout(shrinkWindowTimer);
    shrinkWindowTimer = null;
  }
  isSettingsOpen.value = true;
  await nextTick();
  await resizeOsWindow(widthPx.value, getCurrentTargetWindowHeightPx(), false);
}

function closeSettingsPanel() {
  if (!isSettingsOpen.value) return;
  isSettingsOpen.value = false;
  if (shrinkWindowTimer) clearTimeout(shrinkWindowTimer);
  shrinkWindowTimer = setTimeout(() => {
    shrinkWindowTimer = null;
    if (!isSettingsOpen.value) {
      resizeOsWindow(widthPx.value, baseWindowHeightPx.value, false);
    }
  }, 260);
}

function toggleSettingsPanel() {
  if (isSettingsOpen.value) {
    closeSettingsPanel();
  } else {
    openSettingsPanel();
  }
}

async function handleCloseWindow() {
  await invoke("save_desktop_lyric_position_cmd").catch(() => {});
  closeSettingsPanel();
  isWindowVisible.value = false;
  sendCmd("CMD_CLOSE");
}

function handleKeyDown(e: KeyboardEvent) {
  if (e.key === "Escape" && isSettingsOpen.value) {
    e.preventDefault();
    closeSettingsPanel();
  }
}

function handleStorageEvent(e: StorageEvent) {
  if (!e.newValue) return;
  if (e.key === SNAPSHOT_STORAGE_KEY || e.key === POS_STORAGE_KEY) {
    try {
      const parsed = JSON.parse(e.newValue);
      applyIncomingSyncPayload(parsed, { fromStorage: true });
    } catch {
      // ignore
    }
  } else if (e.key === "bm_desktop_lyric_locked") {
    isLocked.value = e.newValue === "true";
  }
}

async function requestFullHandshake() {
  hydrateFromStorageSnapshots();
  sendCmd("CMD_DESKTOP_LYRIC_READY");
  try {
    const snap = await invoke<any>("pull_desktop_lyric_sync_cmd");
    if (snap && typeof snap === "object") {
      applyIncomingSyncPayload(snap);
    }
  } catch {
    // ignore in browser preview
  }
}

// ==================== 生命周期与跨窗口通信 ====================
onMounted(async () => {
  if (typeof document !== "undefined") {
    document.documentElement.classList.add("is-desktop-lyric-win");
    document.body.classList.add("is-desktop-lyric-win");
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }

  window.addEventListener("blur", handleWindowBlur);
  window.addEventListener("keydown", handleKeyDown);
  window.addEventListener("storage", handleStorageEvent);
  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", handleDesktopLyricVisibilityChange);
  }

  if (typeof ResizeObserver !== "undefined") {
    if (viewportRef.value) {
      measuredViewportWidth.value =
        viewportRef.value.clientWidth || widthPx.value - 44;
      viewportResizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) {
          if (entry.contentRect.width > 0) {
            measuredViewportWidth.value = entry.contentRect.width;
          }
        }
      });
      viewportResizeObserver.observe(viewportRef.value);
    }

    if (settingsInnerRef.value) {
      settingsResizeObserver = new ResizeObserver(() => {
        if (
          isSettingsOpen.value &&
          !isLocked.value &&
          !isAdjustingWidth.value &&
          !isResizingEdge.value
        ) {
          resizeOsWindow(
            widthPx.value,
            getCurrentTargetWindowHeightPx(),
            false
          );
        }
      });
      settingsResizeObserver.observe(settingsInnerRef.value);
    }
  }

  await syncOsWindowWithState(false);

  if (typeof BroadcastChannel !== "undefined") {
    channel = new BroadcastChannel(CHANNEL_NAME);
    channel.onmessage = (ev) => {
      applyIncomingSyncPayload(ev.data);
    };
    typoSyncChannel = new BroadcastChannel(TYPOGRAPHY_SYNC_CHANNEL);
    typoSyncChannel.onmessage = (ev) => {
      if (ev.data?.type === "TYPOGRAPHY_UPDATED" && ev.data?.config) {
        const cfg = normalizeTypographyConfig(ev.data.config);
        hydrateAndApplyTypography(cfg).catch(() => {});
        const ov = resolveDesktopLyricFontOverride(cfg);
        customFontFamilyStack.value = ov.familyStack || "";
        customFontId.value = ov.customFontId || "";
        customFontWeightOverride.value = ov.weightOverride;
      }
    };
  }
  hydrateAndApplyTypography(loadPersistedTypographyConfig()).catch(() => {});

  const listenersReady = await Promise.allSettled([
    listen<any>("bm-desktop-lyric-sync", (event) => {
      applyIncomingSyncPayload(event.payload);
    }),
    listen<{ hoveringWindow?: boolean; hoveringUnlock?: boolean }>(
      "bm-desktop-lyric-locked-hover",
      (event) => {
        const p = event.payload;
        if (!p || typeof p !== "object") return;
        lockedHoverWindow.value = Boolean(p.hoveringWindow);
        lockedHoverUnlock.value = Boolean(p.hoveringUnlock);
      }
    ),
    listen<{ locked?: boolean }>("bm-desktop-lyric-lock-sync", (event) => {
      const p = event.payload;
      if (!p || typeof p !== "object" || typeof p.locked !== "boolean") return;
      isLocked.value = p.locked;
      if (p.locked) {
        isSettingsOpen.value = false;
        isHovering.value = false;
        isPointerInside.value = false;
      } else {
        lockedHoverWindow.value = false;
        lockedHoverUnlock.value = false;
      }
    }),
    listen<{ visible?: boolean; locked?: boolean }>(
      "bm-desktop-lyric-visibility",
      (event) => {
        const p = event.payload;
        if (!p || typeof p !== "object") return;
        if (typeof p.visible === "boolean") {
          isWindowVisible.value = p.visible;
        }
        if (typeof p.locked === "boolean") {
          isLocked.value = p.locked;
          if (p.locked) {
            isSettingsOpen.value = false;
            isHovering.value = false;
            isPointerInside.value = false;
          } else {
            lockedHoverWindow.value = false;
            lockedHoverUnlock.value = false;
          }
        }
        if (p.visible) {
          syncOsWindowWithState(false);
          requestFullHandshake();
          if (isPlaying.value) {
            startClockLoop();
          }
        } else if (p.visible === false) {
          stopClockLoop();
        }
      }
    ),
  ]);

  if (listenersReady[0]?.status === "fulfilled")
    unlistenTauriSync = listenersReady[0].value;
  if (listenersReady[1]?.status === "fulfilled")
    unlistenLockedHover = listenersReady[1].value;
  if (listenersReady[2]?.status === "fulfilled")
    unlistenLockSync = listenersReady[2].value;
  if (listenersReady[3]?.status === "fulfilled")
    unlistenVisibility = listenersReady[3].value;

  await requestFullHandshake();
  handshakeTimers.push(
    setTimeout(() => {
      if (!hasReceivedLiveState) requestFullHandshake();
    }, 150)
  );
  handshakeTimers.push(
    setTimeout(() => {
      if (!hasReceivedLiveState) requestFullHandshake();
    }, 450)
  );
  handshakeTimers.push(
    setTimeout(() => {
      if (!hasReceivedLiveState) requestFullHandshake();
    }, 1100)
  );
});

onBeforeUnmount(() => {
  stopClockLoop();
  if (typoSyncChannel) {
    typoSyncChannel.close();
    typoSyncChannel = null;
  }
  if (hoverLeaveTimer) clearTimeout(hoverLeaveTimer);
  if (shrinkWindowTimer) clearTimeout(shrinkWindowTimer);
  if (widthCmdDebounceTimer) clearTimeout(widthCmdDebounceTimer);
  if (introHideTimer) clearTimeout(introHideTimer);
  for (const t of handshakeTimers) clearTimeout(t);
  handshakeTimers = [];
  if (viewportResizeObserver) {
    viewportResizeObserver.disconnect();
    viewportResizeObserver = null;
  }
  if (settingsResizeObserver) {
    settingsResizeObserver.disconnect();
    settingsResizeObserver = null;
  }
  if (unlistenTauriSync) {
    unlistenTauriSync();
    unlistenTauriSync = null;
  }
  if (unlistenLockedHover) {
    unlistenLockedHover();
    unlistenLockedHover = null;
  }
  if (unlistenLockSync) {
    unlistenLockSync();
    unlistenLockSync = null;
  }
  if (unlistenVisibility) {
    unlistenVisibility();
    unlistenVisibility = null;
  }
  window.removeEventListener("blur", handleWindowBlur);
  window.removeEventListener("keydown", handleKeyDown);
  window.removeEventListener("storage", handleStorageEvent);
  if (typeof document !== "undefined") {
    document.removeEventListener("visibilitychange", handleDesktopLyricVisibilityChange);
  }
  if (channel) {
    channel.close();
    channel = null;
  }
  if (typeof document !== "undefined") {
    document.documentElement.classList.remove("is-desktop-lyric-win");
    document.body.classList.remove("is-desktop-lyric-win");
  }
});
</script>

<template>
  <div
    class="dl-shell"
    :class="{
      'is-locked': isLocked,
      'is-hovering': isHovering,
      'is-glass-active': glassVisible,
      'is-settings-open': isSettingsOpen && !isLocked,
      'is-resizing': isResizingEdge || isAdjustingWidth,
      'is-compact': widthPx < 560,
      'is-ultra-compact': widthPx < 470,
      'is-light-shell': isLightShell,
      'is-oat-theme': isOatShell,
      'is-velvet-theme': isVelvetShell,
    }"
    :style="{
      '--dl-accent': activeSungPalette.primary,
      '--dl-accent-end': activeSungPalette.secondary,
      '--dl-accent-soft': hexToRgba(
        activeSungPalette.primary,
        isLightShell ? 0.14 : 0.22
      ),
      '--dl-accent-glow': hexToRgba(
        activeSungPalette.primary,
        isLightShell ? 0.2 : 0.36
      ),
      '--dl-ui-primary': shellUiTheme.primary,
      '--dl-ui-primary-text': shellUiTheme.primaryText,
      '--dl-ui-secondary': shellUiTheme.secondary,
      '--dl-ui-primary-soft': hexToRgba(
        shellUiTheme.primary,
        isLightShell ? 0.14 : 0.22
      ),
      '--dl-ui-on-primary': playBtnInkColor,
      '--dl-ui-text-1': shellUiTheme.text1,
      '--dl-ui-text-2': shellUiTheme.text2,
      '--dl-ui-text-3': shellUiTheme.text3,
      '--dl-ui-hover-bg': shellUiTheme.hoverBg,
      '--dl-ui-hover-blur': shellUiTheme.hoverBlur,
      '--dl-ui-hover-shadow': shellUiTheme.hoverShadow,
      '--dl-ui-card': shellUiTheme.cardBg,
      '--dl-ui-chip': shellUiTheme.chipBg,
      '--dl-ui-chip-hover': shellUiTheme.chipHover,
      '--dl-ui-divider': shellUiTheme.divider,
      '--dl-ui-tip-bg': shellUiTheme.tipBg,
      '--dl-ui-tip-text': shellUiTheme.tipText,
      '--dl-ui-unlock-bg': shellUiTheme.unlockBg,
      '--dl-ui-unlock-text': shellUiTheme.unlockText,
      '--dl-resizer-color': shellUiTheme.resizerColor,
      '--dl-lyric-opacity': lyricOpacity.toFixed(2),
      '--dl-font-size': `${fontSize}px`,
      '--dl-sub-font-size': `${Math.max(14, Math.round(fontSize * 0.62))}px`,
      '--dl-font-weight': String(activeTypography.fontWeight),
      '--dl-font-family': activeTypography.fontFamily,
      '--dl-viewport-h': `${lyricViewportHeightPx}px`,
      '--dl-target-w': `${widthPx}px`,
    }"
    @pointerenter="handlePointerEnter"
    @pointermove="handlePointerMove"
    @pointerleave="handlePointerLeave"
    @mouseenter="handlePointerEnter"
    @mousemove="handlePointerMove"
    @mouseleave="handlePointerLeave"
    @mousedown="handleStartDrag"
    @wheel="handleWheelZoom"
  >
    <!-- 主无框悬停卡片：常态 100% 纯透明无阴影浮空文字，悬停丝滑浮现四主题毛玻璃框 -->
    <div class="dl-glass-card">
      <!-- 左右边缘拖拽调宽热区（仅未锁定时激活） -->
      <div
        v-if="!isLocked"
        class="dl-edge-resizer is-left"
        data-no-drag
        title="左右拖拽平滑调节桌面歌词宽度"
        @mousedown="startEdgeResize('left', $event)"
      ></div>
      <div
        v-if="!isLocked"
        class="dl-edge-resizer is-right"
        data-no-drag
        title="左右拖拽平滑调节桌面歌词宽度"
        @mousedown="startEdgeResize('right', $event)"
      ></div>

      <!-- 40px 顶部极简控制顶栏（悬停浮现，四主题联动） -->
      <div class="dl-toolbar" :class="{ 'is-visible': glassVisible }">
        <!-- 左侧组：锁定 + 译文快捷切换 + 播放模式 -->
        <div class="dl-bar-group dl-bar-left" data-no-drag>
          <button
            type="button"
            class="dl-icon-btn"
            aria-label="锁定桌面歌词"
            @mouseenter="
              setHoverTip(
                '锁定鼠标穿透 · 悬停顶部或按 Ctrl+Shift+L 解锁',
                'left'
              )
            "
            @mouseleave="clearHoverTip"
            @click.stop="handleLockClick"
          >
            <PlumpIcon name="lock" :size="15" :color="shellUiTheme.lockColor" />
          </button>

          <button
            type="button"
            class="dl-icon-btn"
            :class="{ 'is-accented': showTranslation }"
            aria-label="切换双语译文"
            @mouseenter="
              setHoverTip(
                showTranslation ? '关闭双语译文' : '开启双语译文',
                'left'
              )
            "
            @mouseleave="clearHoverTip"
            @click.stop="toggleTranslation"
          >
            <PlumpIcon
              name="translate-bubble"
              :size="15"
              :color="
                showTranslation
                  ? shellUiTheme.primary
                  : shellUiTheme.iconDefault
              "
              :accent="shellUiTheme.secondary"
            />
          </button>

          <button
            type="button"
            class="dl-icon-btn"
            :class="{ 'is-accented': playModeMeta.active }"
            :aria-label="playModeMeta.tip"
            @mouseenter="setHoverTip(playModeMeta.tip, 'left')"
            @mouseleave="clearHoverTip"
            @click.stop="sendCmd('CMD_CYCLE_PLAY_MODE')"
          >
            <PlumpIcon
              :name="playModeMeta.icon"
              :size="15"
              :color="
                playModeMeta.active
                  ? shellUiTheme.primary
                  : shellUiTheme.iconDefault
              "
              :accent="shellUiTheme.secondary"
            />
          </button>
        </div>

        <!-- 正居中：上一首 / 播放暂停 / 下一首 -->
        <div class="dl-bar-group dl-bar-center" data-no-drag>
          <button
            type="button"
            class="dl-icon-btn"
            aria-label="上一首"
            @mouseenter="setHoverTip('上一首', 'center')"
            @mouseleave="clearHoverTip"
            @click.stop="sendCmd('CMD_PREV')"
          >
            <PlumpIcon
              name="prev"
              :size="15"
              :color="shellUiTheme.iconDefault"
            />
          </button>

          <button
            type="button"
            class="dl-icon-btn dl-play-btn"
            aria-label="播放或暂停"
            @mouseenter="
              setHoverTip(isPlaying ? '暂停播放' : '继续播放', 'center')
            "
            @mouseleave="clearHoverTip"
            @click.stop="sendCmd('CMD_TOGGLE_PLAY')"
          >
            <PlumpIcon
              :name="isPlaying ? 'pause' : 'play'"
              :size="17"
              :color="playBtnInkColor"
              :accent="shellUiTheme.secondary"
            />
          </button>

          <button
            type="button"
            class="dl-icon-btn"
            aria-label="下一首"
            @mouseenter="setHoverTip('下一首', 'center')"
            @mouseleave="clearHoverTip"
            @click.stop="sendCmd('CMD_NEXT')"
          >
            <PlumpIcon
              name="next"
              :size="15"
              :color="shellUiTheme.iconDefault"
            />
          </button>
        </div>

        <!-- 右侧组：A-/A+ + 快捷设置 + 回到主窗口 + 关闭 -->
        <div class="dl-bar-group dl-bar-right" data-no-drag>
          <button
            type="button"
            class="dl-icon-btn dl-hide-compact"
            aria-label="减小字号"
            @mouseenter="
              setHoverTip(
                `减小字号（当前 ${Math.round(fontSize)}px · 支持 Ctrl+滚轮）`,
                'right'
              )
            "
            @mouseleave="clearHoverTip"
            @click.stop="setFontSizeVal(fontSize - 2)"
          >
            <PlumpIcon
              name="font-minus"
              :size="14"
              :color="shellUiTheme.iconDefault"
              :accent="shellUiTheme.primary"
            />
          </button>

          <button
            type="button"
            class="dl-icon-btn dl-hide-compact"
            aria-label="增大字号"
            @mouseenter="
              setHoverTip(
                `增大字号（当前 ${Math.round(fontSize)}px · 支持 Ctrl+滚轮）`,
                'right'
              )
            "
            @mouseleave="clearHoverTip"
            @click.stop="setFontSizeVal(fontSize + 2)"
          >
            <PlumpIcon
              name="font-plus"
              :size="14"
              :color="shellUiTheme.iconDefault"
              :accent="shellUiTheme.primary"
            />
          </button>

          <button
            type="button"
            class="dl-icon-btn"
            :class="{ 'is-accented': isSettingsOpen }"
            aria-label="桌面歌词设置"
            @mouseenter="
              setHoverTip(
                isSettingsOpen ? '收起外观与排版设置' : '配色、字体、描边与排版设置',
                'right'
              )
            "
            @mouseleave="clearHoverTip"
            @click.stop="toggleSettingsPanel"
          >
            <PlumpIcon
              name="tuning"
              :size="15"
              :color="
                isSettingsOpen
                  ? shellUiTheme.primary
                  : shellUiTheme.iconDefault
              "
              :accent="shellUiTheme.secondary"
            />
          </button>

          <button
            type="button"
            class="dl-icon-btn dl-hide-compact"
            aria-label="回到主窗口"
            @mouseenter="setHoverTip('唤醒并回到主窗口', 'right')"
            @mouseleave="clearHoverTip"
            @click.stop="focusMainWindow"
          >
            <PlumpIcon
              name="restore"
              :size="14"
              :color="shellUiTheme.iconDefault"
              :accent="shellUiTheme.primary"
            />
          </button>

          <button
            type="button"
            class="dl-icon-btn dl-close-btn"
            aria-label="关闭桌面歌词"
            @mouseenter="setHoverTip('关闭桌面歌词', 'right')"
            @mouseleave="clearHoverTip"
            @click.stop="handleCloseWindow"
          >
            <PlumpIcon
              name="close"
              :size="14"
              :color="shellUiTheme.iconDefault"
            />
          </button>
        </div>
      </div>

      <!-- 切歌迷你提示胶囊 -->
      <transition name="dl-tip-fade">
        <div
          v-if="
            !glassVisible &&
            !isLocked &&
            introVisible &&
            showIntro &&
            currentSong?.title
          "
          class="dl-intro-pill"
          aria-hidden="true"
        >
          <span class="dl-intro-dot"></span>
          <span>
            {{ currentSong.title
            }}{{ currentSong.artist ? ` - ${currentSong.artist}` : "" }}
          </span>
        </div>
      </transition>

      <!-- 细微分割线与操作提示胶囊 -->
      <div class="dl-divider-wrap" :class="{ 'is-visible': glassVisible }">
        <div class="dl-hairline-divider"></div>
        <transition name="dl-tip-fade">
          <div
            v-if="glassVisible && hoverTip"
            class="dl-hover-tip"
            :class="`tip-${hoverTipAlign}`"
          >
            {{ hoverTip }}
          </div>
        </transition>
      </div>

      <!-- 锁定状态下悬停解锁胶囊 -->
      <transition name="dl-tip-fade">
        <div
          v-if="showLockedUnlockCapsule"
          class="dl-locked-unlock-dock"
          :class="{ 'is-zone-hot': lockedHoverUnlock }"
          data-no-drag
        >
          <button
            type="button"
            class="dl-unlock-pill"
            @click.stop="handleUnlockClick"
          >
            <PlumpIcon
              name="unlock"
              :size="14"
              :color="shellUiTheme.unlockText"
              :accent="shellUiTheme.primary"
            />
            <span>点击解锁歌词 · Ctrl+Shift+L</span>
          </button>
        </div>
      </transition>

      <!-- 歌词主舞台（双层真光学矢量描边 · 常态 100% 透明浮空 · 悬停 0px 位移） -->
      <div
        ref="viewportRef"
        class="dl-lyric-viewport"
        :class="{ 'is-idle-faded': idleFade && !isPlaying && !glassVisible }"
        @dblclick="handleViewportDoubleClick"
      >
        <transition name="dl-tip-fade">
          <div
            v-if="interludeProgress >= 0"
            class="dl-interlude-dots"
            aria-hidden="true"
          >
            <span
              v-for="i in 3"
              :key="i"
              class="dl-dot"
              :class="{ 'is-lit': interludeProgress >= (i - 1) * 0.28 }"
            ></span>
          </div>
        </transition>

        <TransitionGroup name="dl-row-glide" tag="div" class="dl-rows-stage">
          <div
            v-for="row in displayPlan.rows"
            :key="row.key"
            class="dl-row-slot"
            :class="[
              `align-${row.alignOverride || 'center'}`,
              {
                'is-secondary': row.isSecondary,
                'is-preview-next': row.isPreviewNext,
                'is-active': row.isActiveKaraoke,
              },
            ]"
            :style="{ top: `${row.verticalPct}%` }"
          >
            <div
              class="dl-row-scale-inner"
              :style="{
                transform:
                  row.fitScale < 0.995
                    ? `scale(${row.fitScale.toFixed(4)})`
                    : 'none',
                transformOrigin:
                  (row.alignOverride || 'center') === 'left'
                    ? 'left center'
                    : 'center center',
              }"
            >
              <!-- 逐字 KRC 双层矢量描边 + 无极流光填充 -->
              <div
                v-if="row.words.length > 0 && row.isActiveKaraoke"
                class="dl-line-text dl-krc-row"
              >
                <span
                  v-for="(w, wIdx) in row.words"
                  :key="wIdx"
                  class="dl-krc-word-wrap"
                  :style="getWordDualRender(row, w).wrapStyle"
                >
                  <span
                    v-if="getWordDualRender(row, w).strokeStyle"
                    class="dl-stroke-back"
                    aria-hidden="true"
                    :style="getWordDualRender(row, w).strokeStyle!"
                    >{{ w.text }}</span
                  >
                  <span
                    class="dl-fill-front"
                    :style="getWordDualRender(row, w).fillStyle"
                    >{{ w.text }}</span
                  >
                </span>
              </div>

              <!-- 整行 / 译文 / 预告行 双层矢量描边 + 纯净梯度填充 -->
              <div
                v-else
                class="dl-line-text dl-plain-stack"
                :class="{ 'is-sub': row.isSecondary }"
                :style="getPlainRowDualRender(row).wrapStyle"
              >
                <span
                  v-if="getPlainRowDualRender(row).strokeStyle"
                  class="dl-stroke-back"
                  aria-hidden="true"
                  :style="getPlainRowDualRender(row).strokeStyle!"
                  >{{ row.text }}</span
                >
                <span
                  class="dl-fill-front"
                  :style="getPlainRowDualRender(row).fillStyle"
                  >{{ row.text }}</span
                >
              </div>
            </div>
          </div>
        </TransitionGroup>
      </div>

      <!-- 底部 2px 实时播放进度流光细线（60fps GPU scaleX 无极滑动，仅悬停显露） -->
      <div class="dl-progress-track" :class="{ 'is-visible': glassVisible }">
        <div
          class="dl-progress-fill"
          :style="{
            transform: `scaleX(${(progressPct / 100).toFixed(5)})`,
          }"
        ></div>
      </div>

      <!-- 向下展开式 3-Tab 轻分栏设置抽屉（固定最大 580px 居中，调宽时滑块原地不动零跑偏） -->
      <div
        class="dl-settings-collapse"
        :class="{ 'is-open': isSettingsOpen && !isLocked }"
        data-no-drag
      >
        <div class="dl-settings-clip">
          <div ref="settingsInnerRef" class="dl-settings-inner">
            <!-- 顶部分栏导航条 + 一键重置 -->
            <div class="dl-pod-nav">
              <div class="dl-pod-tabs" role="tablist" aria-label="桌面歌词设置分栏">
                <button
                  type="button"
                  role="tab"
                  class="dl-pod-tab"
                  :class="{ 'is-active': settingsTab === 'color' }"
                  :aria-selected="settingsTab === 'color'"
                  @click.stop="settingsTab = 'color'"
                >
                  配色
                </button>
                <button
                  type="button"
                  role="tab"
                  class="dl-pod-tab"
                  :class="{ 'is-active': settingsTab === 'typography' }"
                  :aria-selected="settingsTab === 'typography'"
                  @click.stop="settingsTab = 'typography'"
                >
                  字体与描边
                </button>
                <button
                  type="button"
                  role="tab"
                  class="dl-pod-tab"
                  :class="{ 'is-active': settingsTab === 'layout' }"
                  :aria-selected="settingsTab === 'layout'"
                  @click.stop="settingsTab = 'layout'"
                >
                  排版与窗体
                </button>
              </div>

              <button
                type="button"
                class="dl-reset-btn"
                title="恢复桌面歌词默认配色与排版"
                @click.stop="handleResetDefaults"
              >
                恢复默认
              </button>
            </div>

            <!-- ================= 分栏 1：双通道独立配色（已唱 + 未唱） ================= -->
            <div v-if="settingsTab === 'color'" class="dl-pod-pane">
              <!-- 已唱高亮 -->
              <div class="dl-set-card">
                <div class="dl-set-head">
                  <span class="dl-set-label">已唱高亮</span>
                  <div class="dl-head-right">
                    <div class="dl-pill-group compact">
                      <button
                        type="button"
                        class="dl-pill-btn"
                        :class="{ 'is-active': sungMode === 'gradient' }"
                        @click.stop="setSungModeVal('gradient')"
                      >
                        渐变
                      </button>
                      <button
                        type="button"
                        class="dl-pill-btn"
                        :class="{ 'is-active': sungMode === 'solid' }"
                        @click.stop="setSungModeVal('solid')"
                      >
                        纯色
                      </button>
                    </div>
                    <span class="dl-value-pill">{{ activeSungLabel }}</span>
                  </div>
                </div>

                <div class="dl-swatch-strip">
                  <button
                    v-for="preset in sungPresetsDisplay"
                    :key="preset.id"
                    type="button"
                    class="dl-swatch-btn"
                    :class="{ 'is-selected': colorSchemeId === preset.id }"
                    :title="preset.label"
                    @click.stop="selectSungPreset(preset.id)"
                  >
                    <span
                      class="dl-swatch-orb"
                      :style="{
                        background: preset.isDynamic
                          ? `conic-gradient(from 30deg, ${preset.swatchStart}, ${preset.swatchEnd}, #f43f5e, ${preset.swatchStart})`
                          : sungMode === 'solid'
                          ? preset.swatchStart
                          : `linear-gradient(135deg, ${preset.swatchStart} 0%, ${preset.swatchEnd} 100%)`,
                      }"
                    ></span>
                  </button>

                  <span class="dl-v-sep" aria-hidden="true"></span>

                  <label
                    class="dl-color-chip"
                    :class="{ 'is-active': colorSchemeId === 'custom' }"
                    :title="sungMode === 'solid' ? '自定义已唱纯色' : '自定义已唱起色'"
                  >
                    <input
                      type="color"
                      class="dl-native-color"
                      aria-label="自定义已唱主色"
                      :value="customPrimaryHex"
                      @input="
                        pickCustomSungPrimary(
                          ($event.target as HTMLInputElement).value
                        )
                      "
                    />
                    <span>{{ sungMode === "solid" ? "自选色" : "起色" }}</span>
                  </label>

                  <label
                    v-if="sungMode === 'gradient'"
                    class="dl-color-chip"
                    :class="{ 'is-active': colorSchemeId === 'custom' }"
                    title="自定义已唱终色"
                  >
                    <input
                      type="color"
                      class="dl-native-color"
                      aria-label="自定义已唱副色"
                      :value="customSecondaryHex"
                      @input="
                        pickCustomSungSecondary(
                          ($event.target as HTMLInputElement).value
                        )
                      "
                    />
                    <span>终色</span>
                  </label>
                </div>
              </div>

              <!-- 未唱独立配色 + 透明度 -->
              <div class="dl-set-card">
                <div class="dl-set-head">
                  <span class="dl-set-label">未唱文字</span>
                  <div class="dl-head-right dl-head-slider">
                    <span class="dl-value-pill subtle">{{
                      activeUnsungLabel
                    }}</span>
                    <span class="dl-set-sublabel">透明</span>
                    <input
                      type="range"
                      class="dl-range-slider sm"
                      min="0.35"
                      max="0.98"
                      step="0.01"
                      aria-label="未唱歌词透明度"
                      :value="unsungOpacity"
                      :style="{ '--dl-range-pct': unsungOpacityRangePct }"
                      @input="
                        setUnsungOpacityVal(
                          Number(($event.target as HTMLInputElement).value)
                        )
                      "
                    />
                    <span class="dl-num-val">{{
                      Math.round(unsungOpacity * 100)
                    }}%</span>
                  </div>
                </div>

                <div class="dl-unsung-row">
                  <button
                    v-for="up in LYRIC_UNSUNG_PRESETS"
                    :key="up.id"
                    type="button"
                    class="dl-unsung-chip"
                    :class="{ 'is-active': unsungPreset === up.id }"
                    @click.stop="selectUnsungPreset(up.id)"
                  >
                    <span
                      class="dl-mini-dot"
                      :style="{ background: up.swatchDark }"
                    ></span>
                    <span>{{ up.label }}</span>
                  </button>

                  <label
                    class="dl-color-chip"
                    :class="{ 'is-active': unsungPreset === 'custom' }"
                    title="自定义未唱颜色"
                  >
                    <input
                      type="color"
                      class="dl-native-color"
                      aria-label="自定义未唱文字颜色"
                      :value="unsungCustomHex"
                      @input="
                        pickCustomUnsungHex(
                          ($event.target as HTMLInputElement).value
                        )
                      "
                    />
                    <span>自选</span>
                  </label>
                </div>
              </div>
            </div>

            <!-- ================= 分栏 2：字体、真 4 档字重与真光学矢量描边 ================= -->
            <div v-else-if="settingsTab === 'typography'" class="dl-pod-pane">
              <!-- 字形、字重与字号/辉光 -->
              <div class="dl-set-card">
                <div class="dl-set-row dl-split-row">
                  <div class="dl-inline-ctrl">
                    <span class="dl-set-label">字体</span>
                    <div class="dl-pill-group">
                      <button
                        v-for="ff in LYRIC_FONT_FAMILY_OPTIONS"
                        :key="ff.id"
                        type="button"
                        class="dl-pill-btn"
                        :class="{ 'is-active': fontFamily === ff.id }"
                        @click.stop="setFontFamilyVal(ff.id)"
                      >
                        {{ ff.label.slice(-2) }}
                      </button>
                    </div>
                  </div>

                  <div class="dl-inline-ctrl">
                    <span class="dl-set-sublabel">字重</span>
                    <div class="dl-pill-group">
                      <button
                        v-for="fw in LYRIC_FONT_WEIGHT_OPTIONS"
                        :key="fw.id"
                        type="button"
                        class="dl-pill-btn"
                        :class="{ 'is-active': fontWeight === fw.id }"
                        @click.stop="setFontWeightVal(fw.id)"
                      >
                        {{ fw.label }}
                      </button>
                    </div>
                  </div>
                </div>

                <div class="dl-set-row dl-split-row">
                  <div class="dl-slider-box">
                    <span class="dl-set-label">字号</span>
                    <input
                      type="range"
                      class="dl-range-slider"
                      min="18"
                      max="56"
                      step="1"
                      aria-label="桌面歌词字号"
                      :value="fontSize"
                      :style="{ '--dl-range-pct': fontRangePct }"
                      @input="
                        setFontSizeVal(
                          Number(($event.target as HTMLInputElement).value)
                        )
                      "
                    />
                    <span class="dl-num-val">{{ Math.round(fontSize) }}px</span>
                  </div>

                  <div class="dl-slider-box dl-compact-slider">
                    <span class="dl-set-sublabel">辉光</span>
                    <input
                      type="range"
                      class="dl-range-slider"
                      min="0"
                      max="150"
                      step="5"
                      aria-label="桌面歌词霓虹辉光"
                      :value="glowIntensity"
                      :style="{ '--dl-range-pct': glowRangePct }"
                      @input="
                        setGlowVal(
                          Number(($event.target as HTMLInputElement).value)
                        )
                      "
                    />
                    <span class="dl-num-val">{{ glowIntensity }}%</span>
                  </div>
                </div>
              </div>

              <!-- 光学矢量描边（双层外描边 + 颜色 + 立体柔影） -->
              <div class="dl-set-card">
                <div class="dl-set-row dl-split-row">
                  <div class="dl-inline-ctrl">
                    <span class="dl-set-label">描边</span>
                    <div class="dl-pill-group">
                      <button
                        v-for="sw in DESKTOP_STROKE_WIDTH_OPTIONS"
                        :key="sw.id"
                        type="button"
                        class="dl-pill-btn"
                        :class="{ 'is-active': strokeWidth === sw.id }"
                        @click.stop="setStrokeWidthVal(sw.id)"
                      >
                        {{ sw.label }}
                      </button>
                    </div>
                  </div>

                  <button
                    type="button"
                    class="dl-toggle-chip"
                    :class="{ 'is-active': shadowEnabled }"
                    @click.stop="toggleShadowEnabled"
                  >
                    立体柔影
                  </button>
                </div>

                <div class="dl-set-row">
                  <span class="dl-set-label">边色</span>
                  <div class="dl-unsung-row">
                    <button
                      v-for="sc in DESKTOP_STROKE_COLOR_OPTIONS"
                      :key="sc.id"
                      type="button"
                      class="dl-unsung-chip"
                      :class="{ 'is-active': strokeColorMode === sc.id }"
                      @click.stop="setStrokeColorModeVal(sc.id)"
                    >
                      <span
                        class="dl-mini-dot"
                        :style="{ background: sc.swatch }"
                      ></span>
                      <span>{{ sc.label }}</span>
                    </button>

                    <label
                      class="dl-color-chip"
                      :class="{ 'is-active': strokeColorMode === 'custom' }"
                      title="自定义描边色"
                    >
                      <input
                        type="color"
                        class="dl-native-color"
                        aria-label="自定义描边颜色"
                        :value="strokeCustomHex"
                        @input="
                          pickCustomStrokeHex(
                            ($event.target as HTMLInputElement).value
                          )
                        "
                      />
                      <span>自选</span>
                    </label>
                  </div>
                </div>
              </div>
            </div>

            <!-- ================= 分栏 3：排版、平滑宽幅与显示开关 ================= -->
            <div v-else class="dl-pod-pane">
              <!-- 形态、时间校准与平滑中心锚定宽幅 -->
              <div class="dl-set-card">
                <div class="dl-set-row dl-split-row">
                  <div class="dl-inline-ctrl">
                    <span class="dl-set-label">形态</span>
                    <div class="dl-pill-group">
                      <button
                        v-for="m in [
                          { id: 'center', label: '双语' },
                          { id: 'ktv', label: 'KTV' },
                          { id: 'single', label: '单行' },
                          { id: 'left', label: '居左' },
                        ]"
                        :key="m.id"
                        type="button"
                        class="dl-pill-btn"
                        :class="{ 'is-active': layoutMode === m.id }"
                        @click.stop="
                          setLayoutModeVal(m.id as DesktopLyricLayoutMode)
                        "
                      >
                        {{ m.label }}
                      </button>
                    </div>
                  </div>

                  <div class="dl-inline-ctrl">
                    <span class="dl-set-sublabel">校准</span>
                    <div class="dl-pill-group">
                      <button
                        type="button"
                        class="dl-pill-btn"
                        title="延后 0.2 秒"
                        @click.stop="adjustOffsetStep(-200)"
                      >
                        −0.2
                      </button>
                      <button
                        type="button"
                        class="dl-pill-btn"
                        :class="{ 'is-active': lyricOffsetMs !== 0 }"
                        title="点击归零"
                        @click.stop="resetOffsetToZero"
                      >
                        {{
                          lyricOffsetMs === 0
                            ? "0.0s"
                            : `${lyricOffsetMs > 0 ? "+" : ""}${(
                                lyricOffsetMs / 1000
                              ).toFixed(1)}s`
                        }}
                      </button>
                      <button
                        type="button"
                        class="dl-pill-btn"
                        title="提前 0.2 秒"
                        @click.stop="adjustOffsetStep(200)"
                      >
                        +0.2
                      </button>
                    </div>
                  </div>
                </div>

                <!-- 中心锚定平滑宽幅滑块 -->
                <div class="dl-set-row">
                  <div class="dl-slider-box dl-full-slider">
                    <span class="dl-set-label">宽幅</span>
                    <input
                      type="range"
                      class="dl-range-slider"
                      :min="MIN_WIDTH_PX"
                      :max="MAX_WIDTH_PX"
                      step="10"
                      aria-label="桌面歌词窗口宽幅"
                      :value="widthPx"
                      :style="{ '--dl-range-pct': widthRangePct }"
                      @pointerdown.stop="handleWidthSliderDown"
                      @input="
                        handleWidthSliderInput(
                          Number(($event.target as HTMLInputElement).value)
                        )
                      "
                      @change="
                        handleWidthSliderCommit(
                          Number(($event.target as HTMLInputElement).value)
                        )
                      "
                    />
                    <span class="dl-num-val wide">{{ widthPx }}px</span>
                  </div>
                </div>

                <div class="dl-set-row dl-split-row">
                  <div class="dl-slider-box">
                    <span class="dl-set-label">透明</span>
                    <input
                      type="range"
                      class="dl-range-slider"
                      min="0.35"
                      max="1.0"
                      step="0.05"
                      aria-label="整体透明度"
                      :value="lyricOpacity"
                      :style="{ '--dl-range-pct': opacityRangePct }"
                      @input="
                        setLyricOpacityVal(
                          Number(($event.target as HTMLInputElement).value)
                        )
                      "
                    />
                    <span class="dl-num-val">{{
                      Math.round(lyricOpacity * 100)
                    }}%</span>
                  </div>

                  <div class="dl-slider-box dl-compact-slider">
                    <span class="dl-set-sublabel">行距</span>
                    <input
                      type="range"
                      class="dl-range-slider"
                      min="0"
                      max="16"
                      step="1"
                      aria-label="双行间距"
                      :value="lineSpacing"
                      :style="{ '--dl-range-pct': lineSpacingRangePct }"
                      @input="
                        setLineSpacingVal(
                          Number(($event.target as HTMLInputElement).value)
                        )
                      "
                    />
                    <span class="dl-num-val">{{ lineSpacing }}px</span>
                  </div>
                </div>
              </div>

              <!-- 显示开关组 -->
              <div class="dl-set-card compact-card">
                <div class="dl-set-row">
                  <span class="dl-set-label">开关</span>
                  <div class="dl-toggle-row">
                    <button
                      type="button"
                      class="dl-toggle-chip"
                      :class="{ 'is-active': showTranslation }"
                      @click.stop="toggleTranslation"
                    >
                      双语译文
                    </button>
                    <button
                      type="button"
                      class="dl-toggle-chip"
                      :class="{ 'is-active': showPhonetic }"
                      @click.stop="togglePhonetic"
                    >
                      音译注音
                    </button>
                    <button
                      type="button"
                      class="dl-toggle-chip"
                      :class="{ 'is-active': wordByWordEnabled }"
                      @click.stop="toggleWordByWord"
                    >
                      逐字染色
                    </button>
                    <button
                      type="button"
                      class="dl-toggle-chip"
                      :class="{ 'is-active': showIntro }"
                      @click.stop="toggleShowIntro"
                    >
                      切歌小标
                    </button>
                    <button
                      type="button"
                      class="dl-toggle-chip"
                      :class="{ 'is-active': idleFade }"
                      @click.stop="toggleIdleFade"
                    >
                      暂停淡出
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 100% 透明根容器：常态零底色、零方形投影，悬浮置顶于任意桌面应用之上 */
.dl-shell {
  width: 100vw;
  height: 100vh;
  padding: 12px 14px 16px;
  box-sizing: border-box;
  background: transparent;
  display: flex;
  flex-direction: column;
  align-items: stretch;
  justify-content: flex-start;
  user-select: none;
  overflow: hidden;
  cursor: grab;
  color: var(--dl-ui-text-1, #f5f7fa);
}

.dl-shell.is-locked {
  background: transparent;
  cursor: default;
}

.dl-shell:active:not(.is-locked) {
  cursor: grabbing;
}

/* ================= 无框主容器（常态 100% 纯透明，悬停丝滑浮现四主题毛玻璃框） ================= */
.dl-glass-card {
  position: relative;
  width: min(100%, var(--dl-target-w, 820px));
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  border-radius: var(--bm-r-xl, 22px);
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow: none;
  transition:
    background 0.18s cubic-bezier(0.22, 1, 0.36, 1),
    backdrop-filter 0.18s ease,
    box-shadow 0.18s cubic-bezier(0.22, 1, 0.36, 1);
}

.dl-shell.is-glass-active .dl-glass-card {
  background: var(--dl-ui-hover-bg);
  backdrop-filter: var(--dl-ui-hover-blur, none);
  -webkit-backdrop-filter: var(--dl-ui-hover-blur, none);
  box-shadow: var(--dl-ui-hover-shadow);
}

.dl-shell.is-locked .dl-glass-card {
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow: none;
}

/* 左右边缘拖拽调宽手柄 */
.dl-edge-resizer {
  position: absolute;
  top: 12px;
  bottom: 12px;
  width: 8px;
  z-index: 28;
  cursor: ew-resize;
  opacity: 0;
  transition: opacity 0.16s ease;
}

.dl-edge-resizer.is-left {
  left: 0;
}

.dl-edge-resizer.is-right {
  right: 0;
}

.dl-edge-resizer::after {
  content: "";
  position: absolute;
  top: 50%;
  left: 50%;
  width: 3px;
  height: 28px;
  border-radius: 999px;
  transform: translate(-50%, -50%);
  background: var(--dl-resizer-color, rgba(255, 255, 255, 0.28));
  transition:
    background 0.16s ease,
    height 0.16s ease;
}

.dl-shell.is-glass-active .dl-edge-resizer:hover,
.dl-shell.is-resizing .dl-edge-resizer {
  opacity: 1;
}

.dl-edge-resizer:hover::after {
  height: 38px;
  background: var(--dl-ui-primary, #5e6ad2);
}

/* ================= 40px 顶部极简控制顶栏 ================= */
.dl-toolbar {
  height: 40px;
  padding: 0 14px;
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
  align-items: center;
  gap: 8px;
  opacity: 0;
  pointer-events: none;
  transform: translateY(-4px);
  transition:
    opacity 0.15s ease,
    transform 0.18s cubic-bezier(0.22, 1, 0.36, 1);
}

.dl-toolbar.is-visible {
  opacity: 1;
  pointer-events: auto;
  transform: translateY(0);
}

.dl-bar-group {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
}

.dl-bar-left {
  justify-self: start;
}

.dl-bar-center {
  justify-self: center;
  gap: 8px;
  flex-shrink: 0;
}

.dl-bar-right {
  justify-self: end;
}

.dl-shell.is-compact .dl-hide-compact {
  display: none;
}

.dl-shell.is-ultra-compact .dl-toolbar {
  padding: 0 8px;
}

.dl-icon-btn {
  width: 28px;
  height: 28px;
  border-radius: 999px;
  border: none;
  background: var(--dl-ui-chip);
  color: var(--dl-ui-text-1);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  transition:
    transform 0.16s cubic-bezier(0.22, 1.4, 0.36, 1),
    background 0.14s ease,
    box-shadow 0.16s ease;
}

.dl-icon-btn:hover {
  background: var(--dl-ui-chip-hover);
  transform: scale(1.08);
}

.dl-icon-btn:active {
  transform: scale(0.92);
}

.dl-icon-btn:focus-visible,
.dl-pill-btn:focus-visible,
.dl-pod-tab:focus-visible,
.dl-reset-btn:focus-visible,
.dl-toggle-chip:focus-visible,
.dl-unsung-chip:focus-visible,
.dl-swatch-btn:focus-visible,
.dl-unlock-pill:focus-visible {
  outline: 2px solid var(--dl-ui-primary, #5e6ad2);
  outline-offset: 2px;
}

.dl-icon-btn.is-accented {
  background: var(--dl-ui-primary-soft);
}

.dl-play-btn {
  width: 32px;
  height: 32px;
  background: linear-gradient(
    135deg,
    var(--dl-ui-primary, #5e6ad2) 0%,
    var(--dl-ui-secondary, #38bdf8) 100%
  );
  box-shadow: 0 3px 10px var(--dl-accent-glow, rgba(94, 106, 210, 0.3));
}

.dl-play-btn:hover {
  background: linear-gradient(
    135deg,
    var(--dl-ui-primary, #5e6ad2) 0%,
    var(--dl-ui-secondary, #38bdf8) 100%
  );
  filter: brightness(1.07);
}

.dl-close-btn:hover {
  background: rgba(239, 68, 68, 0.24);
}

/* 切歌迷你提示胶囊 */
.dl-intro-pill {
  position: absolute;
  top: 8px;
  left: 50%;
  transform: translateX(-50%);
  max-width: calc(100% - 72px);
  height: 24px;
  padding: 0 12px;
  border-radius: 999px;
  background: var(--dl-ui-unlock-bg);
  box-shadow: 0 4px 10px rgba(0, 0, 0, 0.22);
  color: var(--dl-ui-unlock-text);
  font-size: 12px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  pointer-events: none;
  z-index: 8;
}

.dl-intro-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--dl-accent, #5eead4);
  box-shadow: 0 0 6px var(--dl-accent, #5eead4);
  flex-shrink: 0;
}

/* 顶栏细分割线与悬浮微提示 */
.dl-divider-wrap {
  position: relative;
  height: 1px;
  padding: 0 16px;
  opacity: 0;
  transition: opacity 0.16s ease;
}

.dl-divider-wrap.is-visible {
  opacity: 1;
}

.dl-hairline-divider {
  width: 100%;
  height: 1px;
  background: linear-gradient(
    90deg,
    transparent 0%,
    var(--dl-ui-divider) 18%,
    var(--dl-ui-divider) 82%,
    transparent 100%
  );
}

.dl-hover-tip {
  position: absolute;
  top: 6px;
  padding: 3px 11px;
  border-radius: 999px;
  background: var(--dl-ui-tip-bg);
  box-shadow: 0 3px 10px rgba(0, 0, 0, 0.24);
  color: var(--dl-ui-tip-text);
  font-size: 12px;
  font-weight: 700;
  line-height: 1.4;
  white-space: nowrap;
  pointer-events: none;
  z-index: 25;
}

.dl-hover-tip.tip-left {
  left: 16px;
}

.dl-hover-tip.tip-center {
  left: 50%;
  transform: translateX(-50%);
}

.dl-hover-tip.tip-right {
  right: 16px;
}

/* 锁定状态悬停解锁胶囊 */
.dl-locked-unlock-dock {
  position: absolute;
  top: 6px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 40;
}

.dl-unlock-pill {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 6px 15px;
  border-radius: 999px;
  border: none;
  background: var(--dl-ui-unlock-bg);
  box-shadow: 0 4px 10px rgba(0, 0, 0, 0.24);
  color: var(--dl-ui-unlock-text);
  font-size: 13px;
  font-weight: 800;
  cursor: pointer;
  transition: transform 0.18s cubic-bezier(0.22, 1.4, 0.36, 1);
}

.dl-locked-unlock-dock.is-zone-hot .dl-unlock-pill,
.dl-unlock-pill:hover {
  transform: scale(1.05);
}

/* ================= 歌词主舞台（100% 纯透明浮空 · 悬停 0px 位移） ================= */
.dl-lyric-viewport {
  position: relative;
  width: 100%;
  height: var(--dl-viewport-h, 126px);
  padding: 4px 20px;
  box-sizing: border-box;
  overflow: hidden;
  border-radius: 14px;
  background: transparent;
  font-family: var(
    --dl-font-family,
    "Inter",
    "Microsoft YaHei UI",
    "PingFang SC",
    sans-serif
  );
  opacity: var(--dl-lyric-opacity, 1);
  transition:
    opacity 0.28s ease,
    height 0.22s ease;
}

.dl-lyric-viewport.is-idle-faded {
  opacity: calc(var(--dl-lyric-opacity, 1) * 0.45);
}

.dl-interlude-dots {
  position: absolute;
  top: 4px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 6px;
  z-index: 6;
  pointer-events: none;
}

.dl-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.36);
  transform: scale(0.82);
  transition:
    background 0.3s ease,
    transform 0.3s cubic-bezier(0.22, 1.4, 0.36, 1),
    box-shadow 0.3s ease;
}

.dl-dot.is-lit {
  background: var(--dl-accent, #5eead4);
  transform: scale(1.2);
  box-shadow: 0 0 8px var(--dl-accent, #5eead4);
}

.dl-rows-stage {
  position: relative;
  width: 100%;
  height: 100%;
}

.dl-row-slot {
  position: absolute;
  left: 0;
  right: 0;
  transform: translate3d(0, -50%, 0);
  display: flex;
  align-items: center;
  transition:
    top 0.36s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.36s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.26s ease;
}

.dl-row-slot.align-center {
  justify-content: center;
  text-align: center;
}

.dl-row-slot.align-left {
  justify-content: flex-start;
  text-align: left;
}

.dl-row-scale-inner {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  transition: transform 0.24s ease;
}

.dl-line-text {
  font-size: var(--dl-font-size, 32px);
  font-weight: var(--dl-font-weight, 700);
  line-height: 1.28;
  letter-spacing: 0.015em;
  white-space: nowrap;
  transition:
    font-size 0.2s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.22s ease;
}

.dl-krc-row {
  display: inline-flex;
  align-items: baseline;
  flex-wrap: nowrap;
}

/* 双层光学矢量描边堆栈：底层外描边 + 顶层纯净流光渐变，100% 锐利零吃字 */
.dl-krc-word-wrap,
.dl-plain-stack {
  position: relative;
  display: inline-block;
  white-space: pre;
}

.dl-plain-stack.is-sub {
  font-size: var(--dl-sub-font-size, 20px);
  font-weight: 600;
}

.dl-stroke-back {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  user-select: none;
  white-space: inherit;
  paint-order: stroke fill;
  stroke-linejoin: round;
  stroke-linecap: round;
}

.dl-fill-front {
  position: relative;
  z-index: 1;
  display: inline-block;
  white-space: inherit;
  -webkit-background-clip: text;
  background-clip: text;
  -webkit-text-fill-color: transparent;
  color: transparent;
  paint-order: stroke fill;
}

/* 切行阻尼滑动动画 */
.dl-row-glide-enter-active,
.dl-row-glide-leave-active {
  transition:
    opacity 0.28s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.32s cubic-bezier(0.22, 1, 0.36, 1),
    top 0.36s cubic-bezier(0.22, 1, 0.36, 1);
}

.dl-row-glide-enter-from {
  opacity: 0;
  transform: translate3d(0, calc(-50% + 9px), 0) scale(0.975);
}

.dl-row-glide-leave-to {
  opacity: 0;
  transform: translate3d(0, calc(-50% - 9px), 0) scale(0.975);
}

/* 底部 2px 实时播放进度流光细线（60fps 无极滑动，仅悬停显露） */
.dl-progress-track {
  height: 2px;
  margin: 0 22px 4px;
  border-radius: 999px;
  background: var(--dl-ui-divider);
  overflow: hidden;
  opacity: 0;
  transition: opacity 0.18s ease;
}

.dl-progress-track.is-visible {
  opacity: 1;
}

.dl-progress-fill {
  width: 100%;
  height: 100%;
  border-radius: 999px;
  transform-origin: left center;
  background: linear-gradient(
    90deg,
    var(--dl-ui-primary, #5e6ad2) 0%,
    var(--dl-ui-secondary, #38bdf8) 100%
  );
}

/* ================= 向下展开式 3-Tab 轻分栏设置抽屉 ================= */
.dl-settings-collapse {
  display: grid;
  grid-template-rows: 0fr;
  opacity: 0;
  transform: translateY(-4px);
  pointer-events: none;
  transition:
    grid-template-rows 0.24s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.18s ease,
    transform 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.dl-settings-collapse.is-open {
  grid-template-rows: 1fr;
  opacity: 1;
  transform: translateY(0);
  pointer-events: auto;
}

.dl-settings-clip {
  overflow: hidden;
  min-height: 0;
}

/* 核心居中锚定：宽幅 ≥ 620px 时滑块相对桌面屏幕位置绝对静止，拖拽绝不跑偏 */
.dl-settings-inner {
  width: min(100%, 620px);
  margin: 0 auto;
  padding: 6px 14px 12px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.dl-pod-nav {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.dl-pod-tabs {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 3px;
  border-radius: 999px;
  background: var(--dl-ui-chip);
}

.dl-pod-tab {
  height: 26px;
  padding: 0 13px;
  border-radius: 999px;
  border: none;
  background: transparent;
  color: var(--dl-ui-text-2);
  font-size: 12.5px;
  font-weight: 700;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background 0.18s ease,
    color 0.16s ease,
    box-shadow 0.18s ease;
}

.dl-pod-tab:hover {
  color: var(--dl-ui-text-1);
}

.dl-pod-tab.is-active {
  background: linear-gradient(
    135deg,
    var(--dl-ui-primary, #5e6ad2) 0%,
    var(--dl-ui-secondary, #38bdf8) 100%
  );
  color: var(--dl-ui-on-primary, #ffffff);
  font-weight: 800;
  box-shadow: 0 2px 8px var(--dl-accent-glow);
}

.dl-reset-btn {
  height: 26px;
  padding: 0 11px;
  border-radius: 999px;
  border: none;
  background: var(--dl-ui-chip);
  color: var(--dl-ui-text-2);
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
  white-space: nowrap;
  transition:
    background 0.16s ease,
    color 0.16s ease;
}

.dl-reset-btn:hover {
  background: var(--dl-ui-chip-hover);
  color: var(--dl-ui-text-1);
}

.dl-pod-pane {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.dl-set-card {
  border-radius: var(--bm-r-md, 14px);
  background: var(--dl-ui-card);
  padding: 9px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.dl-set-card.compact-card {
  padding: 8px 12px;
}

.dl-set-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-height: 26px;
}

.dl-head-right {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.dl-head-slider {
  flex: 1;
  justify-content: flex-end;
}

.dl-set-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 28px;
}

.dl-split-row {
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 8px 12px;
}

.dl-set-label {
  font-size: 12.5px;
  font-weight: 800;
  color: var(--dl-ui-text-1);
  white-space: nowrap;
  flex-shrink: 0;
}

.dl-set-sublabel {
  font-size: 12.5px;
  font-weight: 700;
  color: var(--dl-ui-text-2);
  white-space: nowrap;
  flex-shrink: 0;
}

.dl-swatch-strip {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
}

.dl-swatch-btn {
  width: 24px;
  height: 24px;
  padding: 3px;
  border-radius: 50%;
  background: transparent;
  border: none;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    transform 0.18s cubic-bezier(0.22, 1.4, 0.36, 1),
    background 0.16s ease;
}

.dl-swatch-btn:hover {
  transform: scale(1.14);
}

.dl-swatch-btn.is-selected {
  background: var(--dl-ui-primary-soft);
  box-shadow: 0 0 8px var(--dl-accent-glow);
  transform: scale(1.14);
}

.dl-swatch-orb {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  display: block;
}

.dl-v-sep {
  width: 1px;
  height: 16px;
  background: var(--dl-ui-divider);
  margin: 0 2px;
}

.dl-color-chip {
  height: 25px;
  padding: 0 8px 0 5px;
  border-radius: 999px;
  background: var(--dl-ui-chip);
  color: var(--dl-ui-text-2);
  font-size: 11.5px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  white-space: nowrap;
  transition:
    background 0.16s ease,
    color 0.16s ease;
}

.dl-color-chip:hover,
.dl-color-chip.is-active {
  background: var(--dl-ui-primary-soft);
  color: var(--dl-ui-primary-text, var(--dl-ui-primary));
}

.dl-native-color {
  width: 14px;
  height: 14px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  cursor: pointer;
  overflow: hidden;
  -webkit-appearance: none;
  appearance: none;
}

.dl-native-color::-webkit-color-swatch-wrapper {
  padding: 0;
}

.dl-native-color::-webkit-color-swatch {
  border: none;
  border-radius: 50%;
}

.dl-unsung-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 5px;
  flex: 1;
}

.dl-unsung-chip {
  height: 25px;
  padding: 0 8px 0 6px;
  border-radius: 999px;
  border: none;
  background: var(--dl-ui-chip);
  color: var(--dl-ui-text-2);
  font-size: 11.5px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  white-space: nowrap;
  transition:
    background 0.16s ease,
    color 0.16s ease;
}

.dl-unsung-chip:hover {
  background: var(--dl-ui-chip-hover);
  color: var(--dl-ui-text-1);
}

.dl-unsung-chip.is-active {
  background: var(--dl-ui-primary-soft);
  color: var(--dl-ui-primary-text, var(--dl-ui-primary));
  font-weight: 800;
}

.dl-mini-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
}

.dl-value-pill {
  font-size: 11.5px;
  font-weight: 800;
  color: var(--dl-ui-primary-text, var(--dl-ui-primary));
  background: var(--dl-ui-primary-soft);
  padding: 2px 9px;
  border-radius: 999px;
  white-space: nowrap;
}

.dl-value-pill.subtle {
  margin-right: auto;
}

.dl-inline-ctrl {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.dl-pill-group {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 2px;
  border-radius: 999px;
  background: var(--dl-ui-chip);
}

.dl-pill-btn {
  height: 24px;
  padding: 0 9px;
  border-radius: 999px;
  border: none;
  background: transparent;
  font-size: 12px;
  font-weight: 700;
  color: var(--dl-ui-text-2);
  white-space: nowrap;
  cursor: pointer;
  transition:
    background 0.16s ease,
    color 0.16s ease;
}

.dl-pill-btn:hover {
  color: var(--dl-ui-text-1);
  background: var(--dl-ui-chip-hover);
}

.dl-pill-btn.is-active {
  background: var(--dl-ui-primary-soft);
  color: var(--dl-ui-primary-text, var(--dl-ui-primary));
  font-weight: 800;
}

.dl-toggle-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  flex: 1;
}

.dl-toggle-chip {
  height: 27px;
  padding: 0 11px;
  border-radius: 999px;
  border: none;
  background: var(--dl-ui-chip);
  color: var(--dl-ui-text-2);
  font-size: 12.5px;
  font-weight: 700;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background 0.16s ease,
    color 0.16s ease;
}

.dl-toggle-chip:hover {
  background: var(--dl-ui-chip-hover);
  color: var(--dl-ui-text-1);
}

.dl-toggle-chip.is-active {
  background: var(--dl-ui-primary-soft);
  color: var(--dl-ui-primary-text, var(--dl-ui-primary));
  font-weight: 800;
}

.dl-slider-box {
  flex: 1;
  min-width: 195px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.dl-slider-box.dl-compact-slider {
  flex: 0.78;
  min-width: 160px;
}

.dl-slider-box.dl-full-slider {
  min-width: 100%;
}

.dl-range-slider {
  -webkit-appearance: none;
  appearance: none;
  flex: 1;
  height: 6px;
  border-radius: 999px;
  background: linear-gradient(
    90deg,
    var(--dl-ui-primary, #5e6ad2) 0%,
    var(--dl-ui-secondary, #38bdf8) var(--dl-range-pct, 50%),
    var(--dl-ui-chip) var(--dl-range-pct, 50%),
    var(--dl-ui-chip) 100%
  );
  cursor: pointer;
}

.dl-range-slider:focus-visible {
  outline: 2px solid var(--dl-ui-primary, #5e6ad2);
  outline-offset: 4px;
}

.dl-range-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #ffffff;
  border: none;
  box-shadow:
    0 1px 5px rgba(0, 0, 0, 0.34),
    0 0 6px var(--dl-accent-glow);
  cursor: pointer;
  transition: transform 0.16s cubic-bezier(0.22, 1.4, 0.36, 1);
}

.dl-range-slider::-moz-range-thumb {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #ffffff;
  border: none;
  box-shadow:
    0 1px 5px rgba(0, 0, 0, 0.34),
    0 0 6px var(--dl-accent-glow);
  cursor: pointer;
}

.dl-range-slider::-webkit-slider-thumb:hover {
  transform: scale(1.16);
}

.dl-range-slider.sm {
  max-width: 112px;
  height: 5px;
}

.dl-num-val {
  font-size: 12.5px;
  font-weight: 800;
  color: var(--dl-ui-primary-text, var(--dl-ui-primary));
  font-variant-numeric: tabular-nums;
  min-width: 38px;
  text-align: right;
  flex-shrink: 0;
}

.dl-num-val.wide {
  min-width: 48px;
}

/* 微提示淡入淡出 */
.dl-tip-fade-enter-active,
.dl-tip-fade-leave-active {
  transition:
    opacity 0.15s ease,
    transform 0.16s ease;
}

.dl-tip-fade-enter-from,
.dl-tip-fade-leave-to {
  opacity: 0;
  transform: translateY(-3px);
}
</style>

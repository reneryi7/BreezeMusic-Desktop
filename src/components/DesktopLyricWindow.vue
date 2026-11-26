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
  DesktopLyricScenePresetId,
  DesktopLyricStrokeColorMode,
  DesktopLyricStrokeWidth,
  LyricFontFamily,
  LyricFontWeightLevel,
  LyricHighlightThemeId,
  LyricMotionStyle,
  LyricSungColorMode,
  LyricUnsungPresetId,
} from "../types";
import {
  DESKTOP_LYRIC_SCENE_PRESETS,
  DESKTOP_STROKE_COLOR_OPTIONS,
  DESKTOP_STROKE_WIDTH_OPTIONS,
  LYRIC_FONT_FAMILY_OPTIONS,
  LYRIC_FONT_WEIGHT_OPTIONS,
  LYRIC_MOTION_STYLE_OPTIONS,
  LYRIC_SUNG_PRESETS,
  LYRIC_UNSUNG_PRESETS,
  matchActiveDesktopLyricPreset,
  normalizeLyricMotionStyle,
  normalizeLyricThemeId,
  normalizeUnsungPresetId,
  resolveDesktopStrokeStyle,
  resolveLyricTypographyStyle,
  resolveSungLyricPalette,
  resolveUnsungLyricHex,
} from "../utils/lyricTypography";
import {
  WordPhysicsBank,
  normalizeLyricLinePresentation,
  sweepToMaskPositionPct,
  type NormalizedLyricLinePresentation,
  type NormalizedLyricWord,
  type WordPhysicsOptions,
} from "../utils/lyricMotionEngine";
import { lerpOklabHex } from "../utils/coverPalette";
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
  lineIndex: number;
  text: string;
  words: LyricWordItem[];
  presentation: NormalizedLyricLinePresentation | null;
  lineStartMs: number;
  lineDurationMs: number;
  isActiveKaraoke: boolean;
  isLeavingKaraoke: boolean;
  isSecondary: boolean;
  isKtvPeer: boolean;
  isPreviewNext?: boolean;
  isHarmonyLine: boolean;
  verticalPct: number;
  alignOverride: "left" | "center" | "right";
  fitScale: number;
  overflowPx: number;
  rawWidthPx: number;
  maxWidthPx: number;
}

type SettingsPodTab = "preset" | "style" | "layout";

const CHANNEL_NAME = "bm_desktop_lyric_channel";
const SNAPSHOT_STORAGE_KEY = "bm_desktop_lyric_snapshot_v2";
const POS_STORAGE_KEY = "bm_desktop_lyric_pos_v2";
const TOOLBAR_HEIGHT_PX = 40;
const DIVIDER_HEIGHT_PX = 2;
const PROGRESS_HEIGHT_PX = 4;
const SHELL_VPAD_PX = 28;
const MIN_WIDTH_PX = 440;
const MAX_WIDTH_PX = 1280;
const MIN_ADAPTIVE_SCALE = 0.84;

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
    const val = localStorage.getItem(key);
    return val !== null && val !== "" ? val : fallback;
  } catch {
    return fallback;
  }
}

function readStoredBool(key: string, fallback: boolean): boolean {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null) return fallback;
    return raw === "true";
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

// ==================== 独立持久化状态（出厂默认：苹果清透，完全隔离于播放页） ====================
const isWindowVisible = ref<boolean>(readStoredBool("bm_desktop_lyric", false));
const isLocked = ref<boolean>(readStoredBool("bm_desktop_lyric_locked", false));

// 已唱双通道独立状态
const colorSchemeId = ref<LyricHighlightThemeId>(
  normalizeLyricThemeId(
    readStoredString("bm_desktop_lyric_scheme", "dynamic_cover")
  )
);
const sungMode = ref<LyricSungColorMode>(
  readStoredString("bm_dl_sung_mode_v2", "solid") === "gradient"
    ? "gradient"
    : "solid"
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

// 无尖刺圆角轮廓 & 多层光学空气柔影状态（出厂默认：无硬描边 + 空气柔影）
const rawStoredStrokeW = readStoredString("bm_dl_stroke_width_v2", "none");
const strokeWidth = ref<DesktopLyricStrokeWidth>(
  rawStoredStrokeW === "none" ||
    rawStoredStrokeW === "thin" ||
    rawStoredStrokeW === "medium" ||
    rawStoredStrokeW === "bold"
    ? rawStoredStrokeW
    : "none"
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
  readStoredBool("bm_dl_shadow_enabled_v2", true)
);

// 同源 4 套 GPU 动效引擎与交互偏好
const motionStyle = ref<LyricMotionStyle>(
  normalizeLyricMotionStyle(
    readStoredString("bm_dl_motion_style_v1", "apple_minimal")
  )
);
const wordBounceEnabled = ref<boolean>(
  readStoredBool("bm_dl_word_bounce_v1", false)
);
const harmonyStyleEnabled = ref<boolean>(
  readStoredBool("bm_dl_harmony_style_v1", true)
);
const hoverDodgeEnabled = ref<boolean>(
  readStoredBool("bm_dl_hover_dodge_v1", true)
);
const globalMotionLevel = ref<"best" | "standard" | "none">("best");

// 字形、尺寸与排版状态
const fontSize = ref<number>(
  readStoredNumber("bm_desktop_lyric_size_v2", 32, 18, 56)
);
const widthPx = ref<number>(
  readStoredNumber("bm_desktop_lyric_width_v2", 820, MIN_WIDTH_PX, MAX_WIDTH_PX)
);
const showTranslation = ref<boolean>(
  readStoredBool("bm_desktop_lyric_trans", true)
);
const wordByWordEnabled = ref<boolean>(
  readStoredBool("bm_desktop_lyric_krc", true)
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
  readStoredNumber("bm_desktop_lyric_glow_v2", 35, 0, 150)
);
const lyricOpacity = ref<number>(
  readStoredNumber("bm_desktop_lyric_opacity", 1.0, 0.35, 1.0)
);
const lineSpacing = ref<number>(
  readStoredNumber("bm_desktop_lyric_line_spacing", 6, 0, 16)
);
const idleFade = ref<boolean>(
  readStoredBool("bm_desktop_lyric_idle_fade", false)
);
const showIntro = ref<boolean>(readStoredBool("bm_desktop_lyric_intro", true));
const showPhonetic = ref<boolean>(
  readStoredBool("bm_desktop_lyric_phonetic", false)
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

// ==================== 播放与非响应式高精度时钟状态 ====================
const isPlaying = ref<boolean>(false);
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

// 非响应式高精度时钟（零 60fps Vue VDOM 重算！）
let clockPosMs = 0;
let syncAnchorPosMs = 0;
let syncAnchorPerfMs =
  typeof performance !== "undefined" ? performance.now() : 0;
let lastRafPerfMs = 0;

// 仅在真正切行/间奏显隐变更时触发 Vue 响应式更新
const activeLineIndex = ref<number>(-1);
const leavingLineIndex = ref<number>(-1);
const isInterludeVisible = ref<boolean>(false);
const interludeAlign = ref<"left" | "center" | "right">("center");

// 纯 GPU 物理引擎池与 DOM 直写缓存
const activeWordBanks = new Map<string, WordPhysicsBank>();
const leavingWordBank = new WordPhysicsBank();
let cachedRowWordEls = new Map<string, HTMLElement[]>();
let cachedPlainLitEls = new Map<string, HTMLElement>();
let cachedPanTrackEls = new Map<string, HTMLElement>();
let cachedLeavingWordEls: HTMLElement[] = [];
const rowPanCurrentX = new Map<string, number>();
let leavingActive = false;
let physicsSettled = true;
let forceDomFlushNextFrame = true;
let breathingPhase = 0;

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
const lockedHoverDwell = ref<boolean>(false);
let lockedDwellTimer: ReturnType<typeof setTimeout> | null = null;

const isSettingsOpen = ref<boolean>(false);
const settingsTab = ref<SettingsPodTab>("preset");
const isResizingEdge = ref<boolean>(false);
const isAdjustingWidth = ref<boolean>(false);
let isDraggingOsWindow = false;
const hoverTip = ref<string>("");
const hoverTipAlign = ref<"left" | "center" | "right">("center");

const viewportRef = ref<HTMLElement | null>(null);
const settingsInnerRef = ref<HTMLElement | null>(null);
const progressFillRef = ref<HTMLElement | null>(null);
const interludeBeadRefs = ref<HTMLElement[]>([]);
const measuredViewportWidth = ref<number>(770);

let hoverLeaveTimer: ReturnType<typeof setTimeout> | null = null;
let hoverTipAutoClearTimer: ReturnType<typeof setTimeout> | null = null;
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

// ==================== 预设匹配与动效派生状态 ====================
const activeScenePreset = computed<DesktopLyricScenePresetId>(() =>
  matchActiveDesktopLyricPreset({
    motionStyle: motionStyle.value,
    sungMode: sungMode.value,
    strokeWidth: strokeWidth.value,
    shadowEnabled: shadowEnabled.value,
    fontFamily: fontFamily.value,
    fontWeight: fontWeight.value,
    fontSize: fontSize.value,
    glowIntensity: glowIntensity.value,
    wordByWord: wordByWordEnabled.value,
    wordBounceEnabled: wordBounceEnabled.value,
    harmonyStyleEnabled: harmonyStyleEnabled.value,
  })
);

const activeScenePresetLabel = computed(() => {
  if (activeScenePreset.value === "custom") return "自定义";
  const hit = DESKTOP_LYRIC_SCENE_PRESETS.find(
    (p) => p.id === activeScenePreset.value
  );
  return hit ? hit.label : "自定义";
});

const isMaskSweepMotionStyle = computed(
  () => motionStyle.value !== "luminous_bloom"
);

const effectiveMotionLevel = computed<"best" | "standard" | "none">(() => {
  if (
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches
  ) {
    return "none";
  }
  return globalMotionLevel.value;
});

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
      tileBg: "rgba(250, 246, 238, 0.76)",
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
      tileBg: "rgba(255, 255, 255, 0.78)",
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
      tileBg: "rgba(68, 43, 57, 0.72)",
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
    tileBg: "rgba(255, 255, 255, 0.075)",
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

const showLockedUnlockCapsule = computed(() => {
  return (
    isLocked.value &&
    (lockedHoverUnlock.value || lockedHoverDwell.value)
  );
});

const isLockedDodged = computed(() => {
  return (
    isLocked.value &&
    hoverDodgeEnabled.value &&
    lockedHoverWindow.value &&
    !lockedHoverUnlock.value
  );
});

// ==================== 共享双通道配色、真 4 档字重与圆角无尖刺轮廓解析 ====================
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
    isGradientText: false,
    strokeTintHex: activeUnsungHex.value,
  })
);

// 圆角外扩轮廓（SVG feMorphology + 柔角陡化）+ 三层光学空气柔影
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
    fontSizePx: Math.max(14, Math.round(fontSize.value * 0.64)),
  })
);

const hasSvgOutline = computed(() => strokeWidth.value !== "none");

const glowTokens = computed(() => {
  const ratio = clamp(glowIntensity.value / 100, 0, 1.5);
  const c1 = activeSungPalette.value.primary;
  const c2 = activeSungPalette.value.secondary;
  return {
    ratio,
    r1: `${Math.round(8 + 6 * ratio)}px`,
    r2: `${Math.round(18 + 12 * ratio)}px`,
    c1: hexToRgba(c1, clamp(0.42 * Math.min(1.2, ratio + 0.15), 0, 0.72)),
    c2: hexToRgba(c2, clamp(0.24 * Math.min(1.2, ratio + 0.1), 0, 0.48)),
  };
});

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
    return Math.max(66, Math.round(fontSize.value * 2.02 + 18));
  }
  if (layoutMode.value === "ktv") {
    return Math.max(
      114,
      Math.round(fontSize.value * 3.42 + lineSpacing.value * 1.7 + 18)
    );
  }
  return Math.max(
    106,
    Math.round(fontSize.value * 3.24 + lineSpacing.value * 1.6 + 18)
  );
});

const baseWindowHeightPx = computed(() => {
  const minBase = layoutMode.value === "single" ? 128 : 168;
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

// ==================== 工具函数：整行连贯 OKLab 双色流光 + 时间查询 ====================
function getWordLitStyle(w: NormalizedLyricWord): Record<string, string> {
  const { primary, secondary } = activeSungPalette.value;
  if (sungMode.value === "solid" || primary === secondary) {
    return {
      backgroundImage: `linear-gradient(90deg, ${primary} 0%, ${primary} 100%)`,
    };
  }
  const c0 = lerpOklabHex(primary, secondary, w.lineStartFrac);
  const c1 = lerpOklabHex(primary, secondary, w.lineEndFrac);
  return {
    backgroundImage: `linear-gradient(108deg, ${c0} 0%, ${c1} 100%)`,
  };
}

function getPlainLitStyle(): Record<string, string> {
  const { primary, secondary } = activeSungPalette.value;
  if (sungMode.value === "solid" || primary === secondary) {
    return {
      backgroundImage: `linear-gradient(90deg, ${primary} 0%, ${primary} 100%)`,
    };
  }
  return {
    backgroundImage: `linear-gradient(108deg, ${primary} 0%, ${secondary} 100%)`,
  };
}

function resolveLineEndMs(line: LyricLineItem): number {
  const words = line.words || [];
  if (words.length > 0) {
    let maxEnd = line.startTimeMs + 120;
    for (let i = 0; i < words.length; i++) {
      const w = words[i];
      const absStart =
        w.startTimeMs >= line.startTimeMs
          ? w.startTimeMs
          : line.startTimeMs + w.startTimeMs;
      const end = absStart + Math.max(80, w.durationMs || 140);
      if (end > maxEnd) maxEnd = end;
    }
    return maxEnd;
  }
  return line.startTimeMs + Math.max(1200, line.durationMs || 2600);
}

function computeCalibratedPosMs(rawPosMs: number): number {
  return rawPosMs + lyricOffsetMs.value - bluetoothDelayMs.value;
}

function findLineIndexAtPos(
  lines: readonly LyricLineItem[],
  calibratedPos: number
): number {
  if (!lines.length) return -1;
  if (calibratedPos < lines[0].startTimeMs) return -1;
  let lo = 0;
  let hi = lines.length - 1;
  let ans = 0;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].startTimeMs <= calibratedPos) {
      ans = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return ans;
}

function computeInterludeState(
  lines: readonly LyricLineItem[],
  idx: number,
  calibratedPos: number
): { visible: boolean; progress: number; remainingMs: number } {
  if (!lines.length) {
    return { visible: false, progress: 0, remainingMs: 0 };
  }
  if (idx < 0) {
    const firstStart = lines[0].startTimeMs;
    if (firstStart < 2600 || calibratedPos >= firstStart) {
      return { visible: false, progress: 0, remainingMs: 0 };
    }
    return {
      visible: true,
      progress: clamp(calibratedPos / Math.max(1, firstStart), 0, 1),
      remainingMs: Math.max(0, firstStart - calibratedPos),
    };
  }
  const cur = lines[idx];
  const next = lines[idx + 1];
  if (!next) return { visible: false, progress: 0, remainingMs: 0 };
  const lastEnd = resolveLineEndMs(cur);
  const gap = next.startTimeMs - lastEnd;
  if (gap >= 4000 && calibratedPos > lastEnd + 280 && calibratedPos < next.startTimeMs) {
    return {
      visible: true,
      progress: clamp((calibratedPos - lastEnd) / gap, 0, 1),
      remainingMs: Math.max(0, next.startTimeMs - calibratedPos),
    };
  }
  return { visible: false, progress: 0, remainingMs: 0 };
}

function getWordPhysicsOpts(): WordPhysicsOptions {
  return {
    wordByWord: wordByWordEnabled.value,
    bounceEnabled: wordBounceEnabled.value,
    motionStyle: motionStyle.value,
    motionLevel: effectiveMotionLevel.value,
    glowRatio: glowTokens.value.ratio,
    isLightStage: false,
    // 桌面场景收敛微浮幅度，保持高级弹性且不扰乱办公/浏览余光
    liftScale: 0.72,
  };
}

// ==================== 语义折行与自适应缩放 + 超长句随唱平推规划 ====================
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

function measureRowFitMetrics(
  text: string,
  sizePx: number,
  weight: number,
  maxWidthPx: number,
  allowPan: boolean
): { fitScale: number; overflowPx: number; rawWidthPx: number } {
  const measured = measureTextWidthPx(text, sizePx, weight);
  if (measured <= maxWidthPx || measured <= 0) {
    return { fitScale: 1, overflowPx: 0, rawWidthPx: measured };
  }
  const minScale = allowPan ? MIN_ADAPTIVE_SCALE : 0.74;
  const rawFit = (maxWidthPx - 6) / measured;
  const fitScale = clamp(rawFit, minScale, 1);
  const scaledW = measured * fitScale;
  const overflowPx = allowPan ? Math.max(0, Math.ceil(scaledW - maxWidthPx + 14)) : 0;
  return { fitScale, overflowPx, rawWidthPx: measured };
}

function buildDisplayRow(params: {
  key: string;
  lineIndex: number;
  text: string;
  words: LyricWordItem[];
  lineStartMs: number;
  lineDurationMs: number;
  isActiveKaraoke: boolean;
  isLeavingKaraoke?: boolean;
  isSecondary: boolean;
  isKtvPeer?: boolean;
  isPreviewNext?: boolean;
  verticalPct: number;
  alignOverride: "left" | "center" | "right";
  sizePx: number;
  weight: number;
  maxWidthPx: number;
  allowPan: boolean;
}): DisplayRow {
  const presentation =
    params.words.length > 0
      ? normalizeLyricLinePresentation(params.words, params.text)
      : null;
  const isHarmonyLine =
    harmonyStyleEnabled.value &&
    Boolean(
      presentation
        ? presentation.isHarmonyLine
        : /^(\(|（|【).+(\)|）|】)$/.test(params.text.trim())
    );
  const effSizePx = isHarmonyLine
    ? Math.max(15, Math.round(params.sizePx * 0.84))
    : params.sizePx;
  const metrics = measureRowFitMetrics(
    params.text,
    effSizePx,
    params.weight,
    params.maxWidthPx,
    params.allowPan
  );
  return {
    key: params.key,
    lineIndex: params.lineIndex,
    text: params.text,
    words: params.words,
    presentation,
    lineStartMs: params.lineStartMs,
    lineDurationMs: params.lineDurationMs,
    isActiveKaraoke: params.isActiveKaraoke,
    isLeavingKaraoke: Boolean(params.isLeavingKaraoke),
    isSecondary: params.isSecondary,
    isKtvPeer: Boolean(params.isKtvPeer),
    isPreviewNext: params.isPreviewNext,
    isHarmonyLine,
    verticalPct: params.verticalPct,
    alignOverride: params.alignOverride,
    fitScale: metrics.fitScale,
    overflowPx: metrics.overflowPx,
    rawWidthPx: metrics.rawWidthPx,
    maxWidthPx: params.maxWidthPx,
  };
}

const dualRowVerticalPct = computed(() => {
  const spread = clamp(21 + lineSpacing.value * 0.48, 21, 28);
  return { top: 50 - spread, bottom: 50 + spread };
});

const displayPlan = computed<{ rows: DisplayRow[]; isThreeRow: boolean }>(() => {
  const lines = lyricLines.value;
  const maxWidth = Math.max(280, measuredViewportWidth.value - 32);
  const mainPx = fontSize.value;
  const subPx = Math.max(14, Math.round(mainPx * 0.64));
  const ktvPeerPx = Math.max(16, Math.round(mainPx * 0.9));
  const weight = activeTypography.value.fontWeight;
  const align: "left" | "center" =
    layoutMode.value === "left" ? "left" : "center";
  const { top: topPct, bottom: bottomPct } = dualRowVerticalPct.value;
  const idx = activeLineIndex.value;

  if (!lines.length || idx < 0) {
    const isPrelude = lines.length > 0 && idx < 0;
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
      const text = isPrelude && lines[0]?.text ? lines[0].text : titleText;
      return {
        isThreeRow: false,
        rows: [
          buildDisplayRow({
            key: `idle-single-${text}`,
            lineIndex: -1,
            text,
            words: [],
            lineStartMs: 0,
            lineDurationMs: 0,
            isActiveKaraoke: true,
            isSecondary: false,
            verticalPct: 50,
            alignOverride: align,
            sizePx: mainPx,
            weight,
            maxWidthPx: maxWidth,
            allowPan: false,
          }),
        ],
      };
    }

    return {
      isThreeRow: false,
      rows: [
        buildDisplayRow({
          key: `idle-main-${titleText}`,
          lineIndex: -1,
          text: titleText,
          words: [],
          lineStartMs: 0,
          lineDurationMs: 0,
          isActiveKaraoke: true,
          isSecondary: false,
          verticalPct: topPct,
          alignOverride: align,
          sizePx: mainPx,
          weight,
          maxWidthPx: maxWidth,
          allowPan: false,
        }),
        buildDisplayRow({
          key: `idle-sub-${subText}`,
          lineIndex: -1,
          text: subText,
          words: [],
          lineStartMs: 0,
          lineDurationMs: 0,
          isActiveKaraoke: false,
          isSecondary: true,
          verticalPct: bottomPct,
          alignOverride: align,
          sizePx: subPx,
          weight: 600,
          maxWidthPx: maxWidth,
          allowPan: false,
        }),
      ],
    };
  }

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

  // 1. 极简单行：绝不强拆双行，超长句启用随唱平滑横推 + 柔光边缘羽化
  if (isSingleLineOnly) {
    return {
      isThreeRow: false,
      rows: [
        buildDisplayRow({
          key: `l-${idx}-single`,
          lineIndex: idx,
          text: mainText,
          words: useWords,
          lineStartMs: active.startTimeMs,
          lineDurationMs: active.durationMs,
          isActiveKaraoke: true,
          isSecondary: false,
          verticalPct: 50,
          alignOverride: "center",
          sizePx: mainPx,
          weight,
          maxWidthPx: maxWidth,
          allowPan: true,
        }),
      ],
    };
  }

  // 2. 经典 KTV 交替：偶数行恒定「上行居左」、奇数行恒定「下行居右」，双行近似同级（0.90x）原地升温点亮
  if (layoutMode.value === "ktv") {
    const activeOnTop = idx % 2 === 0;
    const rows: DisplayRow[] = [];
    rows.push(
      buildDisplayRow({
        key: `ktv-line-${idx}`,
        lineIndex: idx,
        text: mainText,
        words: useWords,
        lineStartMs: active.startTimeMs,
        lineDurationMs: active.durationMs,
        isActiveKaraoke: true,
        isSecondary: false,
        isKtvPeer: false,
        verticalPct: activeOnTop ? topPct : bottomPct,
        alignOverride: activeOnTop ? "left" : "right",
        sizePx: mainPx,
        weight,
        maxWidthPx: maxWidth,
        allowPan: true,
      })
    );

    if (nextText) {
      const nextIsLeaving = leavingLineIndex.value === idx + 1;
      rows.push(
        buildDisplayRow({
          key: `ktv-line-${idx + 1}`,
          lineIndex: idx + 1,
          text: nextText,
          words: wordByWordEnabled.value ? next?.words || [] : [],
          lineStartMs: next?.startTimeMs || 0,
          lineDurationMs: next?.durationMs || 0,
          isActiveKaraoke: false,
          isLeavingKaraoke: nextIsLeaving,
          isSecondary: false,
          isKtvPeer: true,
          isPreviewNext: true,
          verticalPct: activeOnTop ? bottomPct : topPct,
          alignOverride: activeOnTop ? "right" : "left",
          sizePx: ktvPeerPx,
          weight,
          maxWidthPx: maxWidth,
          allowPan: false,
        })
      );
    }
    rows.sort((a, b) => a.verticalPct - b.verticalPct);
    return { isThreeRow: false, rows };
  }

  // 3. 双语 + 注音三行态：超长主行改用随唱横推，避免挤成四行
  if (hasBothSubLines) {
    return {
      isThreeRow: true,
      rows: [
        buildDisplayRow({
          key: `l-${idx}-main`,
          lineIndex: idx,
          text: mainText,
          words: useWords,
          lineStartMs: active.startTimeMs,
          lineDurationMs: active.durationMs,
          isActiveKaraoke: true,
          isSecondary: false,
          verticalPct: 21,
          alignOverride: align,
          sizePx: mainPx,
          weight,
          maxWidthPx: maxWidth,
          allowPan: true,
        }),
        buildDisplayRow({
          key: `l-${idx}-phonetic`,
          lineIndex: idx,
          text: phoneticText,
          words: [],
          lineStartMs: 0,
          lineDurationMs: 0,
          isActiveKaraoke: false,
          isSecondary: true,
          isPreviewNext: true,
          verticalPct: 57,
          alignOverride: align,
          sizePx: subPx,
          weight: 600,
          maxWidthPx: maxWidth,
          allowPan: false,
        }),
        buildDisplayRow({
          key: `l-${idx}-trans`,
          lineIndex: idx,
          text: transText,
          words: [],
          lineStartMs: 0,
          lineDurationMs: 0,
          isActiveKaraoke: false,
          isSecondary: true,
          isPreviewNext: false,
          verticalPct: 84,
          alignOverride: align,
          sizePx: subPx,
          weight: 600,
          maxWidthPx: maxWidth,
          allowPan: false,
        }),
      ],
    };
  }

  // 4. 双行居中 / 双行居左：仅在无副行且明显超宽(>1.18x)时自然语义拆行，其余保持稳定字号 + 超长随唱横推
  const mainWidth = measureTextWidthPx(mainText, mainPx, weight);
  const shouldSplitSolo =
    !secondaryText && mainWidth > maxWidth * 1.18 && mainText.length >= 10;

  if (shouldSplitSolo) {
    const [p1, p2] = splitLongLineSemantically(
      { ...active, words: useWords },
      maxWidth,
      mainPx,
      weight
    );
    return {
      isThreeRow: false,
      rows: [
        buildDisplayRow({
          key: `l-${idx}-p1`,
          lineIndex: idx,
          text: p1.text,
          words: p1.words,
          lineStartMs: p1.startTimeMs,
          lineDurationMs: p1.durationMs,
          isActiveKaraoke: true,
          isSecondary: false,
          verticalPct: topPct,
          alignOverride: align,
          sizePx: mainPx,
          weight,
          maxWidthPx: maxWidth,
          allowPan: true,
        }),
        buildDisplayRow({
          key: `l-${idx}-p2`,
          lineIndex: idx,
          text: p2.text,
          words: p2.words,
          lineStartMs: p2.startTimeMs,
          lineDurationMs: p2.durationMs,
          isActiveKaraoke: true,
          isSecondary: false,
          verticalPct: bottomPct,
          alignOverride: align,
          sizePx: mainPx,
          weight,
          maxWidthPx: maxWidth,
          allowPan: true,
        }),
      ],
    };
  }

  const rows: DisplayRow[] = [
    buildDisplayRow({
      key: `l-${idx}-main`,
      lineIndex: idx,
      text: mainText,
      words: useWords,
      lineStartMs: active.startTimeMs,
      lineDurationMs: active.durationMs,
      isActiveKaraoke: true,
      isSecondary: false,
      verticalPct: secondaryText ? topPct : 50,
      alignOverride: align,
      sizePx: mainPx,
      weight,
      maxWidthPx: maxWidth,
      allowPan: true,
    }),
  ];

  if (secondaryText) {
    rows.push(
      buildDisplayRow({
        key: `l-${idx}-sub-${transText ? "t" : phoneticText ? "p" : "n"}`,
        lineIndex: idx,
        text: secondaryText,
        words: [],
        lineStartMs: 0,
        lineDurationMs: 0,
        isActiveKaraoke: false,
        isSecondary: true,
        isPreviewNext: !transText && !phoneticText && Boolean(nextText),
        verticalPct: bottomPct,
        alignOverride: align,
        sizePx: subPx,
        weight: 600,
        maxWidthPx: maxWidth,
        allowPan: false,
      })
    );
  }

  return { isThreeRow: false, rows };
});

// ==================== 纯 GPU 差量 DOM 直写引擎（零每帧 VDOM 重算 · 暂停自动休眠） ====================
function refreshRowDomCaches() {
  cachedRowWordEls.clear();
  cachedPlainLitEls.clear();
  cachedPanTrackEls.clear();
  cachedLeavingWordEls = [];

  const vp = viewportRef.value;
  if (!vp) return;

  const rowEls = vp.querySelectorAll<HTMLElement>("[data-row-key]");
  for (let i = 0; i < rowEls.length; i++) {
    const rowEl = rowEls[i];
    const key = rowEl.dataset.rowKey;
    if (!key) continue;

    const words = Array.from(rowEl.querySelectorAll<HTMLElement>(".dl-kw-span"));
    if (words.length > 0) {
      cachedRowWordEls.set(key, words);
      if (rowEl.dataset.leavingRow === "1") {
        cachedLeavingWordEls = words;
      }
    }

    const plainLit = rowEl.querySelector<HTMLElement>(".dl-plain-lit");
    if (plainLit) {
      cachedPlainLitEls.set(key, plainLit);
    }

    const panTrack = rowEl.querySelector<HTMLElement>(".dl-row-pan-track");
    if (panTrack) {
      cachedPanTrackEls.set(key, panTrack);
    }
  }

  // 清理已离场行的物理池与横推缓存
  const validKeys = new Set(displayPlan.value.rows.map((r) => r.key));
  for (const k of Array.from(activeWordBanks.keys())) {
    if (!validKeys.has(k)) activeWordBanks.delete(k);
  }
  for (const k of Array.from(rowPanCurrentX.keys())) {
    if (!validKeys.has(k)) rowPanCurrentX.delete(k);
  }
  for (const bank of activeWordBanks.values()) {
    bank.invalidateDomCache();
  }
  leavingWordBank.invalidateDomCache();
  forceDomFlushNextFrame = true;
}

function flushWordBankToDom(
  bank: WordPhysicsBank,
  els: HTMLElement[],
  forceAll = false
) {
  const n = Math.min(bank.count, els.length);
  const useSweepMask = isMaskSweepMotionStyle.value;
  for (let i = 0; i < n; i++) {
    const el = els[i];
    if (!el) continue;

    const y = bank.y[i];
    if (
      forceAll ||
      Number.isNaN(bank.writtenY[i]) ||
      Math.abs(y - bank.writtenY[i]) >= 0.025
    ) {
      bank.writtenY[i] = y;
      el.style.setProperty("--kw-y", `${y.toFixed(2)}px`);
    }

    const s = bank.scale[i];
    if (
      forceAll ||
      Number.isNaN(bank.writtenScale[i]) ||
      Math.abs(s - bank.writtenScale[i]) >= 0.0008
    ) {
      bank.writtenScale[i] = s;
      el.style.setProperty("--kw-s", s.toFixed(4));
    }

    const lit = bank.lit[i];
    const sweep = bank.sweep[i];
    const litChanged =
      forceAll ||
      Number.isNaN(bank.writtenLit[i]) ||
      Math.abs(lit - bank.writtenLit[i]) >= 0.004;
    const sweepChanged =
      forceAll ||
      Number.isNaN(bank.writtenSweep[i]) ||
      Math.abs(sweep - bank.writtenSweep[i]) >= 0.003;

    if (litChanged) {
      bank.writtenLit[i] = lit;
      el.style.setProperty("--kw-lit", lit.toFixed(3));
    }

    if (sweepChanged) {
      bank.writtenSweep[i] = sweep;
      el.style.setProperty("--kw-mask-x", `${sweepToMaskPositionPct(sweep)}%`);
    }

    if (litChanged || sweepChanged) {
      const sweepCover = !useSweepMask
        ? 1
        : sweep <= 0.84
        ? 0
        : Math.min(1, (sweep - 0.84) / 0.14);
      const baseFade = lit * sweepCover * 0.94;
      el.style.setProperty("--kw-base-fade", baseFade.toFixed(3));
    }

    const glow = bank.glow[i];
    if (
      forceAll ||
      Number.isNaN(bank.writtenGlow[i]) ||
      Math.abs(glow - bank.writtenGlow[i]) >= 0.004
    ) {
      bank.writtenGlow[i] = glow;
      el.style.setProperty("--kw-glow", glow.toFixed(3));
    }
  }
}

function updateLiquidInterludeBeads(
  state: { visible: boolean; progress: number; remainingMs: number },
  dtSec: number
) {
  const targetVis = state.visible;
  if (isInterludeVisible.value !== targetVis) {
    isInterludeVisible.value = targetVis;
    interludeAlign.value =
      layoutMode.value === "left"
        ? "left"
        : layoutMode.value === "ktv"
        ? activeLineIndex.value % 2 === 0
          ? "right"
          : "left"
        : "center";
  }
  if (!targetVis) return;

  const beads = interludeBeadRefs.value;
  if (!beads || beads.length !== 3) return;
  breathingPhase += dtSec * 3.4;

  const { primary, secondary } = activeSungPalette.value;
  const midColor = lerpOklabHex(primary, secondary, 0.5);
  const colors = [primary, midColor, secondary];
  const cueShrink =
    state.remainingMs < 950 ? Math.max(0.18, state.remainingMs / 950) : 1;
  const reduced = effectiveMotionLevel.value === "none";

  for (let i = 0; i < 3; i++) {
    const el = beads[i];
    if (!el) continue;
    const dotProg = clamp((state.progress - i / 3) * 3, 0, 1);
    const wave = reduced
      ? 0.45
      : 0.5 + 0.5 * Math.sin(breathingPhase - i * 0.85);
    const ty = reduced ? 0 : -1.8 * wave * (0.28 + 0.72 * dotProg) * cueShrink;
    const sx =
      (0.84 + 0.36 * dotProg + (reduced ? 0 : 0.14 * wave * dotProg)) *
      (0.54 + 0.46 * cueShrink);
    const sy =
      (0.84 + 0.18 * dotProg + (reduced ? 0 : 0.12 * wave * dotProg)) *
      (0.54 + 0.46 * cueShrink);
    const alpha = clamp((0.26 + 0.74 * dotProg) * (0.45 + 0.55 * cueShrink), 0, 1);
    el.style.background = dotProg > 0.04 ? colors[i] : "rgba(255, 255, 255, 0.36)";
    el.style.opacity = alpha.toFixed(3);
    el.style.transform = `translate3d(0, ${ty.toFixed(2)}px, 0) scale(${sx.toFixed(3)}, ${sy.toFixed(3)})`;
    el.style.boxShadow =
      dotProg > 0.15
        ? `0 0 ${Math.round(8 * dotProg)}px ${hexToRgba(colors[i], 0.55 * dotProg)}`
        : "none";
  }
}

function updateRowHorizontalPan(
  row: DisplayRow,
  calibratedPos: number,
  dtSec: number,
  snap: boolean
): boolean {
  const track = cachedPanTrackEls.get(row.key);
  if (!track) return true;
  if (row.overflowPx <= 1) {
    if (rowPanCurrentX.get(row.key) !== 0) {
      rowPanCurrentX.set(row.key, 0);
      track.style.setProperty("--dl-pan-x", "0px");
    }
    return true;
  }

  let progressFrac = 0;
  const words = row.presentation?.words || [];
  if (words.length > 0 && row.isActiveKaraoke) {
    let foundFrac = 0;
    for (let i = 0; i < words.length; i++) {
      const w = words[i];
      const wStart =
        w.startTimeMs >= row.lineStartMs
          ? w.startTimeMs
          : row.lineStartMs + w.startTimeMs;
      const wDur = Math.max(60, w.durationMs || 160);
      if (calibratedPos < wStart) {
        foundFrac = w.lineStartFrac;
        break;
      }
      if (calibratedPos <= wStart + wDur) {
        const p = clamp((calibratedPos - wStart) / wDur, 0, 1);
        foundFrac = w.lineStartFrac + (w.lineEndFrac - w.lineStartFrac) * p;
        break;
      }
      foundFrac = w.lineEndFrac;
    }
    progressFrac = clamp(foundFrac, 0, 1);
  } else if (row.isActiveKaraoke && row.lineDurationMs > 0) {
    progressFrac = clamp(
      (calibratedPos - row.lineStartMs) / Math.max(400, row.lineDurationMs),
      0,
      1
    );
  }

  const scaledTotalW = row.rawWidthPx * row.fitScale;
  const activeCenterX = scaledTotalW * progressFrac;
  const desiredShift = -clamp(
    activeCenterX - row.maxWidthPx * 0.44,
    0,
    row.overflowPx
  );
  const prevX = rowPanCurrentX.get(row.key) ?? 0;
  let nextX = desiredShift;
  if (!snap && effectiveMotionLevel.value !== "none") {
    const rate = 1 - Math.exp(-dtSec * 6.5);
    const diff = desiredShift - prevX;
    nextX = Math.abs(diff) < 0.35 ? desiredShift : prevX + diff * rate;
  }
  if (snap || Math.abs(nextX - prevX) >= 0.25) {
    rowPanCurrentX.set(row.key, nextX);
    track.style.setProperty("--dl-pan-x", `${nextX.toFixed(1)}px`);
  }
  return Math.abs(nextX - desiredShift) < 0.4;
}

function stepRealtimeLyricVisuals(
  calibratedPos: number,
  dtSec: number,
  snap: boolean
): boolean {
  const plan = displayPlan.value;
  const opts = getWordPhysicsOpts();
  const force = snap || forceDomFlushNextFrame;
  let allSettled = true;

  for (let i = 0; i < plan.rows.length; i++) {
    const row = plan.rows[i];

    // A. 逐字 KRC GPU 物理池
    if (row.presentation && row.presentation.words.length > 0 && row.isActiveKaraoke) {
      let bank = activeWordBanks.get(row.key);
      if (!bank) {
        bank = new WordPhysicsBank();
        activeWordBanks.set(row.key, bank);
      }
      const words = row.presentation.words;
      if (snap || bank.count !== words.length) {
        bank.snapToTime(row.lineStartMs, words, calibratedPos, opts);
      } else {
        const settled = bank.stepActive(
          row.lineStartMs,
          words,
          calibratedPos,
          dtSec,
          opts
        );
        if (!settled) allSettled = false;
      }
      const els = cachedRowWordEls.get(row.key);
      if (els && els.length > 0) {
        flushWordBankToDom(bank, els, force);
      }
    }

    // B. 非 KRC 纯整行（LRC 宽羽化遮罩扫光）
    const plainLitEl = cachedPlainLitEls.get(row.key);
    if (plainLitEl && row.isActiveKaraoke) {
      let sweep = 1;
      if (
        wordByWordEnabled.value &&
        row.lineDurationMs > 0 &&
        activeLineIndex.value >= 0
      ) {
        const rawP = clamp(
          (calibratedPos - row.lineStartMs) / Math.max(400, row.lineDurationMs),
          0,
          1
        );
        sweep = rawP * 0.72 + rawP * rawP * (3 - 2 * rawP) * 0.28;
        if (rawP > 0 && rawP < 1 && isPlaying.value) {
          allSettled = false;
        }
      }
      plainLitEl.style.setProperty(
        "--dl-plain-mask-x",
        `${sweepToMaskPositionPct(sweep)}%`
      );
      plainLitEl.style.setProperty(
        "--dl-plain-lit",
        isMaskSweepMotionStyle.value ? "1" : sweep.toFixed(3)
      );
      const plainStack = plainLitEl.parentElement;
      if (plainStack) {
        const plainFade =
          !isMaskSweepMotionStyle.value
            ? sweep * 0.94
            : sweep >= 0.99
            ? 0.94
            : 0;
        plainStack.style.setProperty("--kw-base-fade", plainFade.toFixed(3));
      }
    }

    // C. 超长句随唱横向平滑推镜
    if (row.overflowPx > 1) {
      const panSettled = updateRowHorizontalPan(row, calibratedPos, dtSec, snap);
      if (!panSettled) allSettled = false;
    }
  }

  // D. 360ms 退场余韵平滑消融
  if (leavingActive) {
    const leaveSettled = leavingWordBank.stepLeaving(
      dtSec,
      effectiveMotionLevel.value
    );
    if (cachedLeavingWordEls.length > 0) {
      flushWordBankToDom(leavingWordBank, cachedLeavingWordEls, force);
    }
    if (leaveSettled) {
      leavingActive = false;
      leavingLineIndex.value = -1;
    } else {
      allSettled = false;
    }
  }

  // E. 液态光珠间奏倒计时
  const interlude = computeInterludeState(
    lyricLines.value,
    activeLineIndex.value,
    calibratedPos
  );
  updateLiquidInterludeBeads(interlude, dtSec);
  if (interlude.visible && isPlaying.value) {
    allSettled = false;
  }

  // F. 悬停进度条仅在卡片可见时直写 GPU scaleX
  if (progressFillRef.value && glassVisible.value) {
    const frac =
      songDurationMs.value > 0
        ? clamp(clockPosMs / songDurationMs.value, 0, 1)
        : 0;
    progressFillRef.value.style.transform = `scaleX(${frac.toFixed(5)})`;
  }

  forceDomFlushNextFrame = false;
  return allSettled;
}

function advanceClockAndLine(nowPerf: number, snap = false): number {
  if (isPlaying.value) {
    const rawElapsed = Math.max(0, nowPerf - syncAnchorPerfMs);
    const elapsed =
      Math.min(rawElapsed, 2500) * Math.max(0.25, playbackRate.value || 1);
    const nextRaw = syncAnchorPosMs + elapsed;
    clockPosMs =
      songDurationMs.value > 0
        ? Math.min(songDurationMs.value, nextRaw)
        : nextRaw;
  }

  const calibrated = computeCalibratedPosMs(clockPosMs);
  const nextIdx = findLineIndexAtPos(lyricLines.value, calibrated);
  if (nextIdx !== activeLineIndex.value) {
    const prevIdx = activeLineIndex.value;
    if (
      !snap &&
      prevIdx >= 0 &&
      Math.abs(nextIdx - prevIdx) === 1 &&
      layoutMode.value === "ktv"
    ) {
      const prevBank = activeWordBanks.get(`ktv-line-${prevIdx}`);
      if (prevBank && prevBank.count > 0) {
        leavingWordBank.copyFrom(prevBank);
        leavingLineIndex.value = prevIdx;
        leavingActive = true;
      }
    } else {
      leavingActive = false;
      leavingLineIndex.value = -1;
    }
    activeLineIndex.value = nextIdx;
    nextTick(() => {
      refreshRowDomCaches();
      stepRealtimeLyricVisuals(computeCalibratedPosMs(clockPosMs), 0.016, snap);
    });
  }
  return calibrated;
}

function startRenderLoop() {
  if (
    rafId !== null ||
    !isWindowVisible.value ||
    (typeof document !== "undefined" && document.hidden)
  ) {
    return;
  }
  lastRafPerfMs = typeof performance !== "undefined" ? performance.now() : 0;
  const tick = (nowPerf: number) => {
    if (
      !isWindowVisible.value ||
      (typeof document !== "undefined" && document.hidden)
    ) {
      rafId = null;
      return;
    }
    const dtSec = clamp((nowPerf - lastRafPerfMs) / 1000, 0.004, 0.05);
    lastRafPerfMs = nowPerf;

    const calibrated = advanceClockAndLine(nowPerf, false);
    const settled = stepRealtimeLyricVisuals(calibrated, dtSec, false);
    physicsSettled = settled;

    // 暂停态且物理弹簧与余韵全部收敛后自动停帧休眠（0% 空转）
    if (!isPlaying.value && settled) {
      rafId = null;
      return;
    }
    rafId = requestAnimationFrame(tick);
  };
  rafId = requestAnimationFrame(tick);
}

function stopRenderLoop() {
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
}

function requestImmediateVisualSync(snap = false) {
  forceDomFlushNextFrame = true;
  physicsSettled = false;
  nextTick(() => {
    refreshRowDomCaches();
    const calibrated = advanceClockAndLine(
      typeof performance !== "undefined" ? performance.now() : 0,
      snap
    );
    const settled = stepRealtimeLyricVisuals(calibrated, 0.016, snap);
    physicsSettled = settled;
    if (isPlaying.value || !settled) {
      startRenderLoop();
    }
  });
}

function handleDesktopLyricVisibilityChange() {
  if (typeof document === "undefined") return;
  if (document.hidden) {
    stopRenderLoop();
  } else if (isWindowVisible.value) {
    requestImmediateVisualSync(true);
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
  const delta = safePos - clockPosMs;
  const jumped = !playing || Math.abs(delta) > 320 || safePos === 0;
  if (jumped) {
    clockPosMs = safePos;
    syncAnchorPosMs = safePos;
  } else {
    const smoothAnchor =
      delta >= -32 && delta <= 0 ? clockPosMs : safePos;
    syncAnchorPosMs = smoothAnchor;
    if (delta > 0) clockPosMs = smoothAnchor;
  }
  syncAnchorPerfMs = performance.now();
  isPlaying.value = playing;

  if (
    isWindowVisible.value &&
    (typeof document === "undefined" || !document.hidden)
  ) {
    if (jumped) {
      requestImmediateVisualSync(true);
    } else if (playing || !physicsSettled) {
      startRenderLoop();
    } else {
      stepRealtimeLyricVisuals(computeCalibratedPosMs(clockPosMs), 0.016, true);
    }
  } else {
    stopRenderLoop();
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
  if (
    msg.motionLevel === "best" ||
    msg.motionLevel === "standard" ||
    msg.motionLevel === "none"
  ) {
    globalMotionLevel.value = msg.motionLevel;
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
    if (msg.motionStyle) {
      motionStyle.value = normalizeLyricMotionStyle(String(msg.motionStyle));
      localStorage.setItem("bm_dl_motion_style_v1", motionStyle.value);
    }
    if (msg.wordBounceEnabled !== undefined) {
      wordBounceEnabled.value = Boolean(msg.wordBounceEnabled);
      localStorage.setItem(
        "bm_dl_word_bounce_v1",
        String(wordBounceEnabled.value)
      );
    }
    if (msg.harmonyStyleEnabled !== undefined) {
      harmonyStyleEnabled.value = Boolean(msg.harmonyStyleEnabled);
      localStorage.setItem(
        "bm_dl_harmony_style_v1",
        String(harmonyStyleEnabled.value)
      );
    }
    if (msg.hoverDodge !== undefined) {
      hoverDodgeEnabled.value = Boolean(msg.hoverDodge);
      localStorage.setItem(
        "bm_dl_hover_dodge_v1",
        String(hoverDodgeEnabled.value)
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
    clockPosMs = Math.max(0, Number(msg.positionMs || 0));
    activeLineIndex.value = -1;
    leavingLineIndex.value = -1;
    leavingActive = false;
    activeWordBanks.clear();
    rowPanCurrentX.clear();
  }
  applySyncedPosition(
    Number(msg.positionMs || 0),
    Boolean(msg.isPlaying),
    msg.sentAtMs,
    msg.playbackRate
  );
  requestImmediateVisualSync(songChanged);
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
    isWindowVisible.value = true;
    applyIncomingSyncPayload(
      {
        type: "STATE_SYNC",
        enabled: true,
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

// ==================== Tauri 原生窗口尺寸（中心锚定平滑缩放）与拖拽管理 ====================
function getCurrentTargetWindowHeightPx(): number {
  const baseH = baseWindowHeightPx.value;
  if (isSettingsOpen.value && !isLocked.value) {
    const drawerH = settingsInnerRef.value?.offsetHeight || 256;
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
    setHoverTip("已切换至双行歌词显示", "center", 1600);
  } else {
    lastDoubleLayout.value = layoutMode.value;
    setLayoutModeVal("single");
    setHoverTip("已切换至极简单行模式（再次双击恢复）", "center", 1600);
  }
}

function handleWheelZoom(e: WheelEvent) {
  if (isLocked.value || !e.ctrlKey) return;
  e.preventDefault();
  const step = e.deltaY < 0 ? 2 : -2;
  setFontSizeVal(fontSize.value + step);
  setHoverTip(`桌面字号：${Math.round(fontSize.value)}px`, "center", 1400);
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
  if (progressFillRef.value) {
    const frac =
      songDurationMs.value > 0
        ? clamp(clockPosMs / songDurationMs.value, 0, 1)
        : 0;
    progressFillRef.value.style.transform = `scaleX(${frac.toFixed(5)})`;
  }
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
  lockedHoverDwell.value = false;
  if (lockedDwellTimer) {
    clearTimeout(lockedDwellTimer);
    lockedDwellTimer = null;
  }
  isHovering.value = true;
  localStorage.setItem("bm_desktop_lyric_locked", "false");
  hoverTip.value = "";
  sendCmd("CMD_UNLOCK");
  resizeOsWindow(widthPx.value, baseWindowHeightPx.value, false);
}

function setHoverTip(
  text: string,
  align: "left" | "center" | "right",
  autoClearMs = 0
) {
  if (hoverTipAutoClearTimer) {
    clearTimeout(hoverTipAutoClearTimer);
    hoverTipAutoClearTimer = null;
  }
  hoverTip.value = text;
  hoverTipAlign.value = align;
  if (autoClearMs > 0) {
    hoverTipAutoClearTimer = setTimeout(() => {
      hoverTipAutoClearTimer = null;
      if (hoverTip.value === text) {
        hoverTip.value = "";
      }
    }, autoClearMs);
  }
}

function clearHoverTip() {
  if (hoverTipAutoClearTimer) {
    clearTimeout(hoverTipAutoClearTimer);
    hoverTipAutoClearTimer = null;
  }
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

// 场景预设 & 动效引擎控制
function applyScenePresetVal(
  presetId: Exclude<DesktopLyricScenePresetId, "custom">
) {
  const preset = DESKTOP_LYRIC_SCENE_PRESETS.find((p) => p.id === presetId);
  if (!preset) return;
  lastLocalControlMutateAt = performance.now();
  const c = preset.config;
  motionStyle.value = c.motionStyle;
  sungMode.value = c.sungMode;
  strokeWidth.value = c.strokeWidth;
  shadowEnabled.value = c.shadowEnabled;
  fontFamily.value = c.fontFamily;
  fontWeight.value = c.fontWeight;
  fontSize.value = c.fontSize;
  lastSyncedFontSize = c.fontSize;
  glowIntensity.value = c.glowIntensity;
  wordByWordEnabled.value = c.wordByWord;
  wordBounceEnabled.value = c.wordBounceEnabled;
  harmonyStyleEnabled.value = c.harmonyStyleEnabled;

  localStorage.setItem("bm_dl_motion_style_v1", c.motionStyle);
  localStorage.setItem("bm_dl_sung_mode_v2", c.sungMode);
  localStorage.setItem("bm_dl_stroke_width_v2", c.strokeWidth);
  localStorage.setItem("bm_dl_shadow_enabled_v2", String(c.shadowEnabled));
  localStorage.setItem("bm_desktop_lyric_font_family", c.fontFamily);
  localStorage.setItem("bm_desktop_lyric_font_weight", String(c.fontWeight));
  localStorage.setItem("bm_desktop_lyric_size_v2", String(c.fontSize));
  localStorage.setItem("bm_desktop_lyric_glow_v2", String(c.glowIntensity));
  localStorage.setItem("bm_desktop_lyric_krc", String(c.wordByWord));
  localStorage.setItem("bm_dl_word_bounce_v1", String(c.wordBounceEnabled));
  localStorage.setItem("bm_dl_harmony_style_v1", String(c.harmonyStyleEnabled));

  sendCmd("CMD_APPLY_SCENE_PRESET", { presetId });
  requestImmediateVisualSync(true);
  setHoverTip(`已应用「${preset.label}」桌面场景预设`, "center", 1600);
}

function setMotionStyleVal(style: LyricMotionStyle) {
  lastLocalControlMutateAt = performance.now();
  motionStyle.value = style;
  localStorage.setItem("bm_dl_motion_style_v1", style);
  sendCmd("CMD_SET_MOTION_STYLE", { motionStyle: style });
  requestImmediateVisualSync(false);
}

function toggleWordBounce() {
  lastLocalControlMutateAt = performance.now();
  const next = !wordBounceEnabled.value;
  wordBounceEnabled.value = next;
  localStorage.setItem("bm_dl_word_bounce_v1", String(next));
  sendCmd("CMD_SET_WORD_BOUNCE", { wordBounceEnabled: next });
  requestImmediateVisualSync(false);
}

function toggleHarmonyStyle() {
  lastLocalControlMutateAt = performance.now();
  const next = !harmonyStyleEnabled.value;
  harmonyStyleEnabled.value = next;
  localStorage.setItem("bm_dl_harmony_style_v1", String(next));
  sendCmd("CMD_SET_HARMONY_STYLE", { harmonyStyleEnabled: next });
  requestImmediateVisualSync(true);
}

function toggleHoverDodge() {
  lastLocalControlMutateAt = performance.now();
  const next = !hoverDodgeEnabled.value;
  hoverDodgeEnabled.value = next;
  localStorage.setItem("bm_dl_hover_dodge_v1", String(next));
  sendCmd("CMD_SET_HOVER_DODGE", { hoverDodge: next });
}

function handleSyncFromNowPlaying() {
  lastLocalControlMutateAt = 0;
  sendCmd("CMD_SYNC_FROM_NOW_PLAYING");
  setHoverTip("已同步播放页配色、动效与字体风格", "center", 1600);
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

// 轮廓与空气柔影设置
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
  requestImmediateVisualSync(false);
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
  requestImmediateVisualSync(true);
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
  requestImmediateVisualSync(true);
}

function adjustOffsetStep(deltaMs: number) {
  lyricOffsetMs.value = clamp(lyricOffsetMs.value + deltaMs, -15000, 15000);
  localStorage.setItem("bm_lyric_offset_ms", String(lyricOffsetMs.value));
  sendCmd("CMD_ADJUST_OFFSET", { deltaMs });
  requestImmediateVisualSync(true);
}

function resetOffsetToZero() {
  lyricOffsetMs.value = 0;
  localStorage.setItem("bm_lyric_offset_ms", "0");
  sendCmd("CMD_RESET_OFFSET");
  requestImmediateVisualSync(true);
}

function handleResetDefaults() {
  lastLocalControlMutateAt = performance.now();
  colorSchemeId.value = "dynamic_cover";
  sungMode.value = "solid";
  customPrimaryHex.value = "#5EEAD4";
  customSecondaryHex.value = "#60A5FA";
  unsungPreset.value = "auto";
  unsungCustomHex.value = "#E6ECF5";
  unsungOpacity.value = 0.78;
  strokeWidth.value = "none";
  strokeColorMode.value = "auto";
  strokeCustomHex.value = "#090D16";
  shadowEnabled.value = true;
  motionStyle.value = "apple_minimal";
  wordBounceEnabled.value = false;
  harmonyStyleEnabled.value = true;
  hoverDodgeEnabled.value = true;
  fontFamily.value = "sans";
  fontWeight.value = 760;
  fontSize.value = 32;
  glowIntensity.value = 35;
  layoutMode.value = "center";
  lyricOpacity.value = 1.0;
  lineSpacing.value = 6;
  idleFade.value = false;
  showIntro.value = true;
  showTranslation.value = true;
  showPhonetic.value = false;
  wordByWordEnabled.value = true;
  sendCmd("CMD_RESET_DEFAULTS");
  requestImmediateVisualSync(true);
  setHoverTip("已恢复「苹果清透」默认风格", "right", 1600);
}

function focusMainWindow() {
  sendCmd("CMD_FOCUS_MAIN_WINDOW");
  invoke("focus_main_window_cmd").catch(() => {});
}

// Tablist 键盘方向键无障碍切换（符合 ARIA tabs 规范）
const TAB_ORDER: SettingsPodTab[] = ["preset", "style", "layout"];
function handlePodTabKeydown(e: KeyboardEvent, idx: number) {
  let nextIdx = idx;
  if (e.key === "ArrowRight") nextIdx = (idx + 1) % TAB_ORDER.length;
  else if (e.key === "ArrowLeft")
    nextIdx = (idx - 1 + TAB_ORDER.length) % TAB_ORDER.length;
  else if (e.key === "Home") nextIdx = 0;
  else if (e.key === "End") nextIdx = TAB_ORDER.length - 1;
  else return;
  e.preventDefault();
  settingsTab.value = TAB_ORDER[nextIdx];
  nextTick(() => {
    const btn = document.getElementById(`dl-pod-tab-${TAB_ORDER[nextIdx]}`);
    btn?.focus();
  });
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

watch(
  [
    displayPlan,
    motionStyle,
    wordBounceEnabled,
    wordByWordEnabled,
    glowIntensity,
    globalMotionLevel,
  ],
  () => {
    requestImmediateVisualSync(false);
  }
);

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
  stopRenderLoop();
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

function computeRowOffsetPx(verticalPct: number): string {
  const h = lyricViewportHeightPx.value;
  return `${Math.round((h * verticalPct) / 100)}px`;
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
    document.addEventListener(
      "visibilitychange",
      handleDesktopLyricVisibilityChange
    );
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
        const inWin = Boolean(p.hoveringWindow);
        const inUnlock = Boolean(p.hoveringUnlock);
        lockedHoverWindow.value = inWin;
        lockedHoverUnlock.value = inUnlock;

        if (!inWin) {
          lockedHoverDwell.value = false;
          if (lockedDwellTimer) {
            clearTimeout(lockedDwellTimer);
            lockedDwellTimer = null;
          }
        } else if (inUnlock) {
          lockedHoverDwell.value = true;
          if (lockedDwellTimer) {
            clearTimeout(lockedDwellTimer);
            lockedDwellTimer = null;
          }
        } else if (!lockedHoverDwell.value && !lockedDwellTimer) {
          // 鼠标日常划过不猛闪解锁药丸，停留 >=260ms 或移至顶部热区才轻柔浮现
          lockedDwellTimer = setTimeout(() => {
            lockedDwellTimer = null;
            if (isLocked.value && lockedHoverWindow.value) {
              lockedHoverDwell.value = true;
            }
          }, 260);
        }
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
        lockedHoverDwell.value = false;
        if (lockedDwellTimer) {
          clearTimeout(lockedDwellTimer);
          lockedDwellTimer = null;
        }
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
            lockedHoverDwell.value = false;
          }
        }
        if (p.visible) {
          syncOsWindowWithState(false);
          requestFullHandshake();
          requestImmediateVisualSync(true);
        } else if (p.visible === false) {
          stopRenderLoop();
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

  requestImmediateVisualSync(true);
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
  stopRenderLoop();
  if (typoSyncChannel) {
    typoSyncChannel.close();
    typoSyncChannel = null;
  }
  if (hoverLeaveTimer) clearTimeout(hoverLeaveTimer);
  if (hoverTipAutoClearTimer) clearTimeout(hoverTipAutoClearTimer);
  if (shrinkWindowTimer) clearTimeout(shrinkWindowTimer);
  if (widthCmdDebounceTimer) clearTimeout(widthCmdDebounceTimer);
  if (introHideTimer) clearTimeout(introHideTimer);
  if (lockedDwellTimer) clearTimeout(lockedDwellTimer);
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
    document.removeEventListener(
      "visibilitychange",
      handleDesktopLyricVisibilityChange
    );
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
      '--dl-unsung-color': activeUnsungHex,
      '--dl-unsung-opacity': unsungOpacity.toFixed(2),
      '--dl-sung-air-shadow': sungStrokeSpec.airTextShadow,
      '--dl-unsung-air-shadow': unsungStrokeSpec.airTextShadow,
      '--dl-sub-air-shadow': subStrokeSpec.airTextShadow,
      '--dl-sung-stroke-shadow': sungStrokeSpec.textShadow,
      '--dl-unsung-stroke-shadow': unsungStrokeSpec.textShadow,
      '--dl-sub-stroke-shadow': subStrokeSpec.textShadow,
      '--dl-kw-glow-r1': glowTokens.r1,
      '--dl-kw-glow-r2': glowTokens.r2,
      '--dl-kw-glow-c1': glowTokens.c1,
      '--dl-kw-glow-c2': glowTokens.c2,
      '--dl-heavy-sung-stroke': activeTypography.opticalStrokeCss,
      '--dl-heavy-unsung-stroke': activeUnsungTypography.opticalStrokeCss,
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
      '--dl-ui-tile': shellUiTheme.tileBg,
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
      '--dl-sub-font-size': `${Math.max(14, Math.round(fontSize * 0.64))}px`,
      '--dl-ktv-peer-size': `${Math.max(16, Math.round(fontSize * 0.9))}px`,
      '--dl-font-weight': String(activeTypography.fontWeight),
      '--dl-font-family': activeTypography.fontFamily,
      '--dl-viewport-h': `${lyricViewportHeightPx}px`,
      '--dl-target-w': `${widthPx}px`,
    }"
  >
    <!-- SVG 圆角抗锯齿外扩轮廓滤镜（彻底消灭 Chromium HTML text-stroke 汉字折角尖刺与吃字） -->
    <svg
      v-if="hasSvgOutline"
      class="dl-svg-defs"
      width="0"
      height="0"
      aria-hidden="true"
    >
      <defs>
        <filter
          id="dl-outline-sung"
          x="-20%"
          y="-28%"
          width="140%"
          height="156%"
          color-interpolation-filters="sRGB"
        >
          <feMorphology
            in="SourceAlpha"
            operator="dilate"
            :radius="sungStrokeSpec.svgDilateRadius"
            result="dilated"
          />
          <feGaussianBlur
            in="dilated"
            :stdDeviation="sungStrokeSpec.svgBlurStdDev"
            result="rounded"
          />
          <feComponentTransfer in="rounded" result="cleanEdge">
            <feFuncA type="linear" slope="2.2" intercept="-0.28" />
          </feComponentTransfer>
          <feFlood
            :flood-color="sungStrokeSpec.strokeSolidHex"
            :flood-opacity="sungStrokeSpec.strokeAlpha"
            result="flood"
          />
          <feComposite in="flood" in2="cleanEdge" operator="in" />
        </filter>

        <filter
          id="dl-outline-unsung"
          x="-20%"
          y="-28%"
          width="140%"
          height="156%"
          color-interpolation-filters="sRGB"
        >
          <feMorphology
            in="SourceAlpha"
            operator="dilate"
            :radius="unsungStrokeSpec.svgDilateRadius"
            result="dilated"
          />
          <feGaussianBlur
            in="dilated"
            :stdDeviation="unsungStrokeSpec.svgBlurStdDev"
            result="rounded"
          />
          <feComponentTransfer in="rounded" result="cleanEdge">
            <feFuncA type="linear" slope="2.2" intercept="-0.28" />
          </feComponentTransfer>
          <feFlood
            :flood-color="unsungStrokeSpec.strokeSolidHex"
            :flood-opacity="unsungStrokeSpec.strokeAlpha"
            result="flood"
          />
          <feComposite in="flood" in2="cleanEdge" operator="in" />
        </filter>

        <filter
          id="dl-outline-sub"
          x="-20%"
          y="-28%"
          width="140%"
          height="156%"
          color-interpolation-filters="sRGB"
        >
          <feMorphology
            in="SourceAlpha"
            operator="dilate"
            :radius="subStrokeSpec.svgDilateRadius"
            result="dilated"
          />
          <feGaussianBlur
            in="dilated"
            :stdDeviation="subStrokeSpec.svgBlurStdDev"
            result="rounded"
          />
          <feComponentTransfer in="rounded" result="cleanEdge">
            <feFuncA type="linear" slope="2.2" intercept="-0.28" />
          </feComponentTransfer>
          <feFlood
            :flood-color="subStrokeSpec.strokeSolidHex"
            :flood-opacity="subStrokeSpec.strokeAlpha"
            result="flood"
          />
          <feComposite in="flood" in2="cleanEdge" operator="in" />
        </filter>
      </defs>
    </svg>

    <!-- 主无框悬停卡片：常态 100% 纯透明无阴影浮空文字，悬停丝滑浮现四主题柔和毛玻璃框 -->
    <div
      class="dl-glass-card"
      @pointerenter="handlePointerEnter"
      @pointermove="handlePointerMove"
      @pointerleave="handlePointerLeave"
      @mouseenter="handlePointerEnter"
      @mousemove="handlePointerMove"
      @mouseleave="handlePointerLeave"
      @mousedown="handleStartDrag"
      @wheel="handleWheelZoom"
    >
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
            :aria-label="isPlaying ? '暂停播放' : '继续播放'"
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
            aria-label="桌面歌词工作台"
            @mouseenter="
              setHoverTip(
                isSettingsOpen
                  ? '收起桌面歌词工作台'
                  : `预设、动效与配色设置（${activeScenePresetLabel}）`,
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

      <!-- 切歌轻透提示（无实体方盒，贴合纯净浮空质感） -->
      <transition name="dl-tip-fade">
        <div
          v-if="
            !glassVisible &&
            !isLocked &&
            introVisible &&
            showIntro &&
            currentSong?.title
          "
          class="dl-intro-caption"
          aria-hidden="true"
        >
          <span class="dl-intro-dot"></span>
          <span class="dl-intro-text">
            {{ currentSong.title
            }}{{ currentSong.artist ? ` · ${currentSong.artist}` : "" }}
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

      <!-- 锁定状态下悬停解锁胶囊（微延迟/顶部热区温和触发，拒绝扫过猛闪） -->
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

      <!-- 歌词主舞台（100% 纯净无底浮空 · 纯 GPU 差量引擎 · 锁态柔和避让） -->
      <div
        ref="viewportRef"
        class="dl-lyric-viewport"
        :class="[
          isMaskSweepMotionStyle ? 'dl-motion-sweep' : 'dl-motion-bloom',
          {
            'is-idle-faded': idleFade && !isPlaying && !glassVisible,
            'is-locked-dodged': isLockedDodged,
            'has-svg-outline': hasSvgOutline,
            'has-air-shadow': shadowEnabled && !hasSvgOutline,
            'has-stroke-shadow': shadowEnabled && hasSvgOutline,
          },
        ]"
        @dblclick="handleViewportDoubleClick"
      >
        <!-- 液态光珠间奏倒计时（入拍前 1s 自然聚能收拢） -->
        <transition name="dl-fade-soft">
          <div
            v-show="isInterludeVisible"
            class="dl-interlude-beads"
            :class="`align-${interludeAlign}`"
            aria-hidden="true"
          >
            <span
              v-for="i in 3"
              :key="i"
              :ref="
                (el) => {
                  if (el) interludeBeadRefs[i - 1] = el as HTMLElement;
                }
              "
              class="dl-interlude-bead"
            ></span>
          </div>
        </transition>

        <TransitionGroup name="dl-row-glide" tag="div" class="dl-rows-stage">
          <div
            v-for="row in displayPlan.rows"
            :key="row.key"
            :data-row-key="row.key"
            :data-leaving-row="row.isLeavingKaraoke ? '1' : undefined"
            class="dl-row-slot"
            :class="[
              `align-${row.alignOverride}`,
              {
                'is-secondary': row.isSecondary,
                'is-ktv-peer': row.isKtvPeer,
                'is-preview-next': row.isPreviewNext,
                'is-active': row.isActiveKaraoke,
                'is-leaving': row.isLeavingKaraoke,
                'is-harmony-line': row.isHarmonyLine,
                'is-pan-overflow': row.overflowPx > 1,
              },
            ]"
            :style="{ '--row-y-px': computeRowOffsetPx(row.verticalPct) }"
          >
            <div class="dl-row-pan-viewport">
              <div class="dl-row-pan-track">
                <div
                  class="dl-row-scale-inner"
                  :style="{
                    transform:
                      row.fitScale < 0.995
                        ? `scale(${row.fitScale.toFixed(4)})`
                        : 'none',
                    transformOrigin:
                      row.alignOverride === 'left'
                        ? 'left center'
                        : row.alignOverride === 'right'
                        ? 'right center'
                        : 'center center',
                  }"
                >
                  <!-- 1. 逐字 KRC 纯 GPU 三层字粒（整行连贯 OKLab 流光 + 词簇防折断 + 零斑马纹） -->
                  <div
                    v-if="
                      row.presentation &&
                      row.presentation.clusters.length > 0 &&
                      (row.isActiveKaraoke || row.isLeavingKaraoke)
                    "
                    class="dl-line-text dl-krc-row"
                  >
                    <template
                      v-for="(cluster, cIdx) in row.presentation.clusters"
                      :key="cIdx"
                    ><span
                        class="dl-word-cluster"
                        :class="{
                          'is-harmony-word':
                            harmonyStyleEnabled &&
                            cluster.isHarmony &&
                            !row.isHarmonyLine,
                        }"
                      ><span
                          v-for="w in cluster.words"
                          :key="w.wordIdx"
                          class="dl-kw-span"
                          :data-text="w.text"
                        ><span
                            v-if="hasSvgOutline || shadowEnabled"
                            class="dl-kw-backing"
                            aria-hidden="true"
                            >{{ w.text }}</span
                          ><span class="dl-kw-base">{{ w.text }}</span
                          ><span
                            class="dl-kw-lit"
                            aria-hidden="true"
                            :style="getWordLitStyle(w)"
                            >{{ w.text }}</span
                          ></span
                        ></span
                      ><template v-if="cluster.trailingSpace">{{ " " }}</template></template
                    >
                  </div>

                  <!-- 2. 整行 LRC 活动行（300% 静态羽化遮罩扫光） -->
                  <div
                    v-else-if="row.isActiveKaraoke"
                    class="dl-line-text dl-plain-stack is-active-plain"
                  >
                    <span
                      v-if="hasSvgOutline || shadowEnabled"
                      class="dl-kw-backing"
                      aria-hidden="true"
                      >{{ row.text }}</span
                    >
                    <span class="dl-plain-base">{{ row.text }}</span>
                    <span
                      class="dl-plain-lit"
                      aria-hidden="true"
                      :style="getPlainLitStyle()"
                      >{{ row.text }}</span
                    >
                  </div>

                  <!-- 3. 译文 / 注音 / KTV 预告副行 -->
                  <div
                    v-else
                    class="dl-line-text dl-plain-stack"
                    :class="{
                      'is-sub': row.isSecondary,
                      'is-ktv-peer-text': row.isKtvPeer,
                    }"
                  >
                    <span
                      v-if="hasSvgOutline || shadowEnabled"
                      class="dl-kw-backing"
                      :class="{ 'is-sub-backing': row.isSecondary }"
                      aria-hidden="true"
                      >{{ row.text }}</span
                    >
                    <span class="dl-plain-passive">{{ row.text }}</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </TransitionGroup>
      </div>

      <!-- 底部 2px 实时播放进度流光细线（仅悬停显露，直写 GPU scaleX） -->
      <div class="dl-progress-track" :class="{ 'is-visible': glassVisible }">
        <div ref="progressFillRef" class="dl-progress-fill"></div>
      </div>

      <!-- 向下展开式 3-Tab 高定工作台抽屉（预设与动效 / 配色与轮廓 / 排版与窗体） -->
      <div
        class="dl-settings-collapse"
        :class="{ 'is-open': isSettingsOpen && !isLocked }"
        data-no-drag
      >
        <div class="dl-settings-clip">
          <div ref="settingsInnerRef" class="dl-settings-inner">
            <!-- 顶部分栏导航条 + 同步播放页 + 恢复默认 -->
            <div class="dl-pod-nav">
              <div
                class="dl-pod-tabs"
                role="tablist"
                aria-label="桌面歌词工作台分栏"
              >
                <button
                  id="dl-pod-tab-preset"
                  type="button"
                  role="tab"
                  class="dl-pod-tab"
                  :aria-selected="settingsTab === 'preset'"
                  :tabindex="settingsTab === 'preset' ? 0 : -1"
                  @click.stop="settingsTab = 'preset'"
                  @keydown="handlePodTabKeydown($event, 0)"
                >
                  预设与动效
                </button>
                <button
                  id="dl-pod-tab-style"
                  type="button"
                  role="tab"
                  class="dl-pod-tab"
                  :aria-selected="settingsTab === 'style'"
                  :tabindex="settingsTab === 'style' ? 0 : -1"
                  @click.stop="settingsTab = 'style'"
                  @keydown="handlePodTabKeydown($event, 1)"
                >
                  配色与轮廓
                </button>
                <button
                  id="dl-pod-tab-layout"
                  type="button"
                  role="tab"
                  class="dl-pod-tab"
                  :aria-selected="settingsTab === 'layout'"
                  :tabindex="settingsTab === 'layout' ? 0 : -1"
                  @click.stop="settingsTab = 'layout'"
                  @keydown="handlePodTabKeydown($event, 2)"
                >
                  排版与窗体
                </button>
              </div>

              <div class="dl-pod-actions">
                <button
                  type="button"
                  class="dl-reset-btn"
                  title="一键同步播放页的配色、动效引擎与字体风格"
                  @click.stop="handleSyncFromNowPlaying"
                >
                  同步播放页
                </button>
                <button
                  type="button"
                  class="dl-reset-btn"
                  title="恢复苹果清透默认风格"
                  @click.stop="handleResetDefaults"
                >
                  恢复默认
                </button>
              </div>
            </div>

            <!-- ================= 分栏 1：5 套桌面高定预设 + 4 套 GPU 动效引擎 ================= -->
            <div
              v-if="settingsTab === 'preset'"
              role="tabpanel"
              aria-labelledby="dl-pod-tab-preset"
              class="dl-pod-pane"
            >
              <!-- 5 套桌面场景预设 -->
              <div class="dl-set-card">
                <div class="dl-set-head">
                  <span class="dl-set-label">桌面场景预设</span>
                  <span class="dl-value-pill">{{ activeScenePresetLabel }}</span>
                </div>
                <div
                  class="dl-preset-grid"
                  role="radiogroup"
                  aria-label="桌面歌词高定预设"
                >
                  <button
                    v-for="p in DESKTOP_LYRIC_SCENE_PRESETS"
                    :key="p.id"
                    type="button"
                    role="radio"
                    class="dl-preset-tile"
                    :class="{ 'is-active': activeScenePreset === p.id }"
                    :aria-checked="activeScenePreset === p.id"
                    @click.stop="applyScenePresetVal(p.id)"
                  >
                    <span
                      class="dl-preset-bar"
                      :style="{ background: p.accentSwatch }"
                      aria-hidden="true"
                    ></span>
                    <span class="dl-preset-name">{{ p.label }}</span>
                    <span class="dl-preset-tag">{{ p.tagline }}</span>
                  </button>
                </div>
              </div>

              <!-- 4 套纯 GPU 逐字动效引擎 + 细腻开关 -->
              <div class="dl-set-card">
                <div class="dl-set-head">
                  <span class="dl-set-label">逐字动效引擎</span>
                  <div class="dl-head-right">
                    <button
                      type="button"
                      class="dl-toggle-chip"
                      :class="{
                        'is-active':
                          wordBounceEnabled && motionStyle !== 'apple_minimal',
                      }"
                      :disabled="
                        !wordByWordEnabled || motionStyle === 'apple_minimal'
                      "
                      @click.stop="toggleWordBounce"
                    >
                      逐字微浮
                    </button>
                    <button
                      type="button"
                      class="dl-toggle-chip"
                      :class="{ 'is-active': harmonyStyleEnabled }"
                      @click.stop="toggleHarmonyStyle"
                    >
                      和声轻量化
                    </button>
                  </div>
                </div>

                <div
                  class="dl-motion-grid"
                  role="radiogroup"
                  aria-label="桌面歌词逐字动效风格"
                >
                  <button
                    v-for="ms in LYRIC_MOTION_STYLE_OPTIONS"
                    :key="ms.id"
                    type="button"
                    role="radio"
                    class="dl-motion-tile"
                    :class="{ 'is-active': motionStyle === ms.id }"
                    :aria-checked="motionStyle === ms.id"
                    @click.stop="setMotionStyleVal(ms.id)"
                  >
                    <span class="dl-motion-name">{{ ms.label }}</span>
                    <span class="dl-motion-desc">{{ ms.desc }}</span>
                  </button>
                </div>
              </div>
            </div>

            <!-- ================= 分栏 2：双通道配色 + 圆角抗锯齿轮廓与空气柔影 ================= -->
            <div
              v-else-if="settingsTab === 'style'"
              role="tabpanel"
              aria-labelledby="dl-pod-tab-style"
              class="dl-pod-pane"
            >
              <!-- 已唱高亮 + 未唱配色 -->
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

                <div class="dl-set-row dl-split-row dl-sub-divider">
                  <div class="dl-unsung-row">
                    <span class="dl-set-sublabel">未唱</span>
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

              <!-- 圆角外扩轮廓 + 空气柔影 + 未唱透明与辉光 -->
              <div class="dl-set-card">
                <div class="dl-set-row dl-split-row">
                  <div class="dl-inline-ctrl">
                    <span class="dl-set-label">圆角轮廓</span>
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
                    空气柔影
                  </button>
                </div>

                <div v-if="strokeWidth !== 'none'" class="dl-set-row">
                  <span class="dl-set-sublabel">轮廓色</span>
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
                      title="自定义轮廓色"
                    >
                      <input
                        type="color"
                        class="dl-native-color"
                        aria-label="自定义轮廓颜色"
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

                <div class="dl-set-row dl-split-row">
                  <div class="dl-slider-box">
                    <span class="dl-set-sublabel">辉光</span>
                    <input
                      type="range"
                      class="dl-range-slider"
                      min="0"
                      max="150"
                      step="5"
                      aria-label="桌面歌词逐字辉光"
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

                  <div class="dl-slider-box dl-compact-slider">
                    <span class="dl-set-sublabel">未唱透明</span>
                    <input
                      type="range"
                      class="dl-range-slider"
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
              </div>
            </div>

            <!-- ================= 分栏 3：排版布局、字形尺寸、宽幅与生态开关 ================= -->
            <div
              v-else
              role="tabpanel"
              aria-labelledby="dl-pod-tab-layout"
              class="dl-pod-pane"
            >
              <!-- 形态、字体、字重与校准 -->
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

                <div class="dl-set-row dl-split-row">
                  <div class="dl-slider-box">
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

                  <div class="dl-slider-box dl-compact-slider">
                    <span class="dl-set-sublabel">透明</span>
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
                </div>
              </div>

              <!-- 显示与交互开关组 -->
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
                      逐字演绎
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
                    <button
                      type="button"
                      class="dl-toggle-chip"
                      :class="{ 'is-active': hoverDodgeEnabled }"
                      title="锁定时鼠标移至歌词上方自动柔和淡隐，避免遮挡底下内容"
                      @click.stop="toggleHoverDodge"
                    >
                      锁态避让
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
.dl-svg-defs {
  position: absolute;
  width: 0;
  height: 0;
  pointer-events: none;
  overflow: hidden;
}

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
  pointer-events: none;
  color: var(--dl-ui-text-1, #f5f7fa);
}

.dl-shell.is-locked {
  background: transparent;
}

/* ================= 无框主容器（常态 100% 纯透明，悬停丝滑浮现四主题毛玻璃框） ================= */
.dl-glass-card {
  position: relative;
  pointer-events: auto;
  cursor: grab;
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

.dl-shell.is-locked .dl-glass-card {
  cursor: default;
}

.dl-glass-card:active {
  cursor: grabbing;
}

.dl-shell.is-locked .dl-glass-card:active {
  cursor: default;
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
  transform: translate3d(0, -4px, 0);
  transition:
    opacity 0.15s ease,
    transform 0.18s cubic-bezier(0.22, 1, 0.36, 1);
}

.dl-toolbar.is-visible {
  opacity: 1;
  pointer-events: auto;
  transform: translate3d(0, 0, 0);
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
.dl-unlock-pill:focus-visible,
.dl-preset-tile:focus-visible,
.dl-motion-tile:focus-visible {
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
  filter: brightness(1.07);
}

.dl-close-btn:hover {
  background: rgba(239, 68, 68, 0.24);
}

/* 切歌轻透浮空小标（无实体重框，空气柔影承托） */
.dl-intro-caption {
  position: absolute;
  top: 10px;
  left: 50%;
  transform: translate3d(-50%, 0, 0);
  max-width: calc(100% - 72px);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 2px 10px;
  border-radius: 999px;
  color: rgba(248, 250, 252, 0.94);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.02em;
  text-shadow:
    0 1px 2px rgba(4, 7, 14, 0.85),
    0 2px 8px rgba(4, 7, 14, 0.6);
  white-space: nowrap;
  overflow: hidden;
  pointer-events: none;
  z-index: 8;
}

.dl-intro-text {
  overflow: hidden;
  text-overflow: ellipsis;
}

.dl-intro-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--dl-accent, #5eead4);
  box-shadow: 0 0 8px var(--dl-accent, #5eead4);
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
  transform: translate3d(-50%, 0, 0);
}

.dl-hover-tip.tip-right {
  right: 16px;
}

/* 锁定状态悬停解锁胶囊 */
.dl-locked-unlock-dock {
  position: absolute;
  top: 6px;
  left: 50%;
  transform: translate3d(-50%, 0, 0);
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
  font-weight: 700;
  cursor: pointer;
  transition: transform 0.18s cubic-bezier(0.22, 1.4, 0.36, 1);
}

.dl-locked-unlock-dock.is-zone-hot .dl-unlock-pill,
.dl-unlock-pill:hover {
  transform: scale(1.05);
}

/* ================= 歌词主舞台（100% 纯净无底浮空 · GPU 合成管线） ================= */
.dl-lyric-viewport {
  position: relative;
  width: 100%;
  height: var(--dl-viewport-h, 130px);
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
  text-rendering: geometricPrecision;
  opacity: var(--dl-lyric-opacity, 1);
  transition:
    opacity 0.24s cubic-bezier(0.22, 1, 0.36, 1),
    height 0.22s ease;
}

.dl-lyric-viewport.is-idle-faded {
  opacity: calc(var(--dl-lyric-opacity, 1) * 0.42);
}

/* 锁定穿透态鼠标靠近柔和避让：自动淡隐让底下代码/网页清晰可见 */
.dl-lyric-viewport.is-locked-dodged {
  opacity: calc(var(--dl-lyric-opacity, 1) * 0.18);
}

/* 液态光珠间奏倒计时 */
.dl-interlude-beads {
  position: absolute;
  top: 4px;
  left: 24px;
  right: 24px;
  height: 12px;
  display: flex;
  align-items: center;
  gap: 8px;
  z-index: 6;
  pointer-events: none;
}

.dl-interlude-beads.align-left {
  justify-content: flex-start;
}

.dl-interlude-beads.align-center {
  justify-content: center;
}

.dl-interlude-beads.align-right {
  justify-content: flex-end;
}

.dl-interlude-bead {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.38);
  transform-origin: center center;
}

.dl-rows-stage {
  position: relative;
  width: 100%;
  height: 100%;
}

/* 纯合成器线程定位与行切换（使用 translate3d 替代 top 动画） */
.dl-row-slot {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  transform: translate3d(0, calc(var(--row-y-px, 64px) - 50%), 0);
  display: flex;
  align-items: center;
  transition:
    transform 0.34s cubic-bezier(0.22, 1, 0.36, 1),
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

.dl-row-slot.align-right {
  justify-content: flex-end;
  text-align: right;
}

.dl-row-pan-viewport {
  max-width: 100%;
  overflow: visible;
  display: inline-flex;
  align-items: center;
}

.dl-row-slot.is-pan-overflow .dl-row-pan-viewport {
  width: 100%;
  overflow: hidden;
  justify-content: flex-start;
  padding: 6px 0;
  margin: -6px 0;
  -webkit-mask-image: linear-gradient(
    90deg,
    transparent 0%,
    #000 26px,
    #000 calc(100% - 26px),
    transparent 100%
  );
  mask-image: linear-gradient(
    90deg,
    transparent 0%,
    #000 26px,
    #000 calc(100% - 26px),
    transparent 100%
  );
}

.dl-row-pan-track {
  display: inline-flex;
  align-items: center;
  transform: translate3d(var(--dl-pan-x, 0px), 0, 0);
}

.dl-row-scale-inner {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  transition: transform 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.dl-line-text {
  font-size: var(--dl-font-size, 32px);
  font-weight: var(--dl-font-weight, 700);
  line-height: 1.32;
  letter-spacing: 0.015em;
  white-space: nowrap;
  transition:
    font-size 0.22s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.24s ease;
}

.dl-row-slot.is-harmony-line .dl-line-text {
  font-size: calc(var(--dl-font-size, 32px) * 0.84);
  font-style: italic;
  opacity: 0.88;
}

.dl-krc-row {
  display: inline-block;
  white-space: nowrap;
}

.dl-word-cluster {
  display: inline-block;
  white-space: nowrap;
  vertical-align: baseline;
}

.dl-word-cluster.is-harmony-word {
  font-size: 0.8em;
  font-style: italic;
  opacity: 0.84;
  margin: 0 0.08em;
}

/* ==================== 纯 GPU 三层合成字粒结构 ==================== */
.dl-kw-span,
.dl-plain-stack {
  position: relative;
  display: inline-block;
  transform-origin: center 72%;
}

.dl-row-slot.is-active .dl-kw-span,
.dl-row-slot.is-leaving .dl-kw-span {
  transform: translate3d(0, var(--kw-y, 0px), 0) scale(var(--kw-s, 1));
}

/* 静态预光栅化辉光伪层：每帧仅合成 opacity，零发毛零主线程重绘 */
.dl-row-slot.is-active .dl-kw-span::before,
.dl-row-slot.is-leaving .dl-kw-span::before {
  content: attr(data-text);
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  user-select: none;
  color: transparent;
  -webkit-text-fill-color: transparent;
  text-shadow:
    0 0 var(--dl-kw-glow-r1, 10px) var(--dl-kw-glow-c1, rgba(94, 234, 212, 0.35)),
    0 0 var(--dl-kw-glow-r2, 22px) var(--dl-kw-glow-c2, rgba(96, 165, 250, 0.18));
  opacity: var(--kw-glow, 0);
}

/* 底层轮廓 / 多层光学空气柔影层（静止纹理 GPU 缓存，零每帧重算） */
.dl-kw-backing {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  user-select: none;
  white-space: inherit;
}

/* 模式 A（默认苹果清透）：无硬描边，纯三层紧致光学空气柔影 */
.dl-lyric-viewport.has-air-shadow .dl-kw-backing {
  color: transparent;
  -webkit-text-fill-color: transparent;
  text-shadow: var(--dl-sung-air-shadow, none);
}

.dl-lyric-viewport.has-air-shadow .dl-row-slot:not(.is-active) .dl-kw-backing {
  text-shadow: var(--dl-unsung-air-shadow, none);
}

.dl-lyric-viewport.has-air-shadow .dl-kw-backing.is-sub-backing {
  text-shadow: var(--dl-sub-air-shadow, none);
}

/* 模式 B（开启圆角描边）：SVG feMorphology + 柔角陡化纯外扩轮廓，彻底零尖刺零吃字 */
.dl-lyric-viewport.has-svg-outline .dl-kw-backing {
  color: #000000;
  -webkit-text-fill-color: #000000;
  filter: url(#dl-outline-sung);
}

.dl-lyric-viewport.has-svg-outline .dl-row-slot:not(.is-active) .dl-kw-backing {
  filter: url(#dl-outline-unsung);
}

.dl-lyric-viewport.has-svg-outline .dl-kw-backing.is-sub-backing {
  filter: url(#dl-outline-sub);
}

.dl-lyric-viewport.has-stroke-shadow .dl-row-slot.is-active .dl-line-text {
  filter: drop-shadow(0 2px 7px rgba(4, 7, 14, 0.44));
}

.dl-lyric-viewport.has-stroke-shadow .dl-row-slot:not(.is-active) .dl-line-text {
  filter: drop-shadow(0 2px 6px rgba(4, 7, 14, 0.36));
}

/* 中层：未唱基字 */
.dl-kw-base,
.dl-plain-base {
  position: relative;
  z-index: 1;
  display: inline-block;
  color: var(--dl-unsung-color, #eef3fa);
  -webkit-text-stroke: var(--dl-heavy-unsung-stroke, 0px transparent);
  paint-order: stroke fill;
  opacity: calc(var(--dl-unsung-opacity, 0.78) * (1 - var(--kw-base-fade, 0)));
}

/* 顶层：整行连贯 OKLab 流光点亮层（静态 300% 宽羽化遮罩纯 GPU 推进） */
.dl-kw-lit,
.dl-plain-lit {
  position: absolute;
  inset: 0;
  z-index: 2;
  display: inline-block;
  pointer-events: none;
  user-select: none;
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
  -webkit-text-fill-color: transparent;
  -webkit-text-stroke: var(--dl-heavy-sung-stroke, 0px transparent);
  paint-order: stroke fill;
}

.dl-kw-lit {
  opacity: var(--kw-lit, 0);
}

.dl-plain-lit {
  opacity: var(--dl-plain-lit, 1);
}

.dl-lyric-viewport.dl-motion-sweep .dl-kw-lit,
.dl-lyric-viewport.dl-motion-sweep .dl-plain-lit {
  -webkit-mask-image: linear-gradient(
    90deg,
    #000 0%,
    #000 34%,
    rgba(0, 0, 0, 0.68) 41%,
    rgba(0, 0, 0, 0.22) 48%,
    transparent 55%,
    transparent 100%
  );
  mask-image: linear-gradient(
    90deg,
    #000 0%,
    #000 34%,
    rgba(0, 0, 0, 0.68) 41%,
    rgba(0, 0, 0, 0.22) 48%,
    transparent 55%,
    transparent 100%
  );
  -webkit-mask-size: 300% 100%;
  mask-size: 300% 100%;
  -webkit-mask-repeat: no-repeat;
  mask-repeat: no-repeat;
}

.dl-lyric-viewport.dl-motion-sweep .dl-kw-lit {
  -webkit-mask-position: var(--kw-mask-x, 100%) 0;
  mask-position: var(--kw-mask-x, 100%) 0;
}

.dl-lyric-viewport.dl-motion-sweep .dl-plain-lit {
  -webkit-mask-position: var(--dl-plain-mask-x, 100%) 0;
  mask-position: var(--dl-plain-mask-x, 100%) 0;
}

.dl-lyric-viewport.dl-motion-bloom .dl-kw-lit,
.dl-lyric-viewport.dl-motion-bloom .dl-plain-lit {
  -webkit-mask-image: none;
  mask-image: none;
}

/* 副行 / KTV 预告同级行 */
.dl-plain-stack.is-sub {
  font-size: var(--dl-sub-font-size, 20px);
  font-weight: 600;
}

.dl-plain-stack.is-ktv-peer-text {
  font-size: var(--dl-ktv-peer-size, 29px);
  font-weight: var(--dl-font-weight, 700);
}

.dl-plain-passive {
  position: relative;
  z-index: 1;
  display: inline-block;
  color: var(--dl-unsung-color, #eef3fa);
  opacity: var(--dl-unsung-opacity, 0.78);
}

.dl-row-slot.is-preview-next .dl-plain-passive {
  opacity: calc(var(--dl-unsung-opacity, 0.78) * 0.86);
}

/* 切行阻尼滑动动画（纯 transform + opacity 合成器驱动） */
.dl-row-glide-enter-active,
.dl-row-glide-leave-active {
  transition:
    opacity 0.28s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.34s cubic-bezier(0.22, 1, 0.36, 1);
}

.dl-row-glide-enter-from {
  opacity: 0;
  transform: translate3d(0, calc(var(--row-y-px, 64px) - 50% + 10px), 0)
    scale(0.975);
}

.dl-row-glide-leave-to {
  opacity: 0;
  transform: translate3d(0, calc(var(--row-y-px, 64px) - 50% - 10px), 0)
    scale(0.975);
}

/* 底部 2px 实时播放进度流光细线（仅悬停显露） */
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
  transform: scaleX(0);
  transform-origin: left center;
  background: linear-gradient(
    90deg,
    var(--dl-ui-primary, #5e6ad2) 0%,
    var(--dl-ui-secondary, #38bdf8) 100%
  );
}

/* ================= 向下展开式 3-Tab 高定工作台抽屉 ================= */
.dl-settings-collapse {
  display: grid;
  grid-template-rows: 0fr;
  opacity: 0;
  transform: translate3d(0, -4px, 0);
  pointer-events: none;
  transition:
    grid-template-rows 0.24s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.18s ease,
    transform 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.dl-settings-collapse.is-open {
  grid-template-rows: 1fr;
  opacity: 1;
  transform: translate3d(0, 0, 0);
  pointer-events: auto;
}

.dl-settings-clip {
  overflow: hidden;
  min-height: 0;
}

.dl-settings-inner {
  width: min(100%, 640px);
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
  flex-wrap: wrap;
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
  position: relative;
  height: 27px;
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

/* 按 aria-selected 同步态着色，保持恒定字重零尺寸抖动，并附非纯色形状底标满足 WCAG 1.4.1 */
.dl-pod-tab[aria-selected="true"] {
  background: linear-gradient(
    135deg,
    var(--dl-ui-primary, #5e6ad2) 0%,
    var(--dl-ui-secondary, #38bdf8) 100%
  );
  color: var(--dl-ui-on-primary, #ffffff);
  box-shadow: 0 2px 8px var(--dl-accent-glow);
}

.dl-pod-tab[aria-selected="true"]::after {
  content: "";
  position: absolute;
  left: 50%;
  bottom: 2px;
  width: 14px;
  height: 2px;
  border-radius: 999px;
  transform: translateX(-50%);
  background: currentColor;
  opacity: 0.72;
}

.dl-pod-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
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
  min-height: 24px;
}

.dl-head-right {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.dl-set-row {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 26px;
}

.dl-sub-divider {
  padding-top: 6px;
  background-image: linear-gradient(
    90deg,
    transparent 0%,
    var(--dl-ui-divider) 12%,
    var(--dl-ui-divider) 88%,
    transparent 100%
  );
  background-size: 100% 1px;
  background-repeat: no-repeat;
  background-position: top center;
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
  font-size: 12px;
  font-weight: 700;
  color: var(--dl-ui-text-2);
  white-space: nowrap;
  flex-shrink: 0;
}

/* 5 套桌面场景预设瓷砖 */
.dl-preset-grid {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 6px;
}

.dl-shell.is-compact .dl-preset-grid {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.dl-preset-tile {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 3px;
  padding: 7px 8px;
  border-radius: 11px;
  border: none;
  background: var(--dl-ui-tile);
  color: var(--dl-ui-text-1);
  text-align: left;
  cursor: pointer;
  transition:
    background 0.16s ease,
    transform 0.18s cubic-bezier(0.22, 1.35, 0.36, 1),
    box-shadow 0.18s ease;
}

.dl-preset-tile:hover {
  background: var(--dl-ui-chip-hover);
  transform: translateY(-1px);
}

.dl-preset-tile.is-active {
  background: var(--dl-ui-primary-soft);
  box-shadow: 0 3px 10px var(--dl-accent-glow);
}

.dl-preset-bar {
  width: 16px;
  height: 4px;
  border-radius: 999px;
  margin-bottom: 1px;
}

.dl-preset-name {
  font-size: 12px;
  font-weight: 800;
  color: var(--dl-ui-text-1);
  white-space: nowrap;
}

.dl-preset-tag {
  font-size: 10.5px;
  font-weight: 500;
  color: var(--dl-ui-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

/* 4 套 GPU 逐字动效风格选择卡 */
.dl-motion-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.dl-motion-tile {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 7px 9px;
  border-radius: 11px;
  border: none;
  background: var(--dl-ui-tile);
  color: var(--dl-ui-text-1);
  text-align: left;
  cursor: pointer;
  transition:
    background 0.16s ease,
    box-shadow 0.16s ease;
}

.dl-motion-tile:hover {
  background: var(--dl-ui-chip-hover);
}

.dl-motion-tile.is-active {
  background: var(--dl-ui-primary-soft);
  box-shadow: 0 3px 10px var(--dl-accent-glow);
}

.dl-motion-name {
  font-size: 12px;
  font-weight: 800;
  color: var(--dl-ui-text-1);
}

.dl-motion-desc {
  font-size: 10.5px;
  font-weight: 500;
  color: var(--dl-ui-text-3);
  line-height: 1.3;
  display: -webkit-box;
  -webkit-line-clamp: 1;
  -webkit-box-orient: vertical;
  overflow: hidden;
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
  color: var(--dl-ui-text-1);
}

.dl-native-color {
  width: 15px;
  height: 15px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  cursor: pointer;
}

.dl-native-color::-webkit-color-swatch-wrapper {
  padding: 0;
}

.dl-native-color::-webkit-color-swatch {
  border: none;
  border-radius: 50%;
}

.dl-value-pill {
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--dl-ui-primary-soft);
  color: var(--dl-ui-primary-text);
  font-size: 11px;
  font-weight: 700;
  white-space: nowrap;
}

.dl-inline-ctrl {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  flex-wrap: wrap;
}

.dl-pill-group {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px;
  border-radius: 999px;
  background: var(--dl-ui-chip);
}

.dl-pill-group.compact {
  padding: 2px;
}

.dl-pill-btn {
  height: 23px;
  padding: 0 9px;
  border-radius: 999px;
  border: none;
  background: transparent;
  color: var(--dl-ui-text-2);
  font-size: 11.5px;
  font-weight: 700;
  cursor: pointer;
  white-space: nowrap;
  transition:
    background 0.16s ease,
    color 0.16s ease;
}

.dl-pill-btn:hover {
  color: var(--dl-ui-text-1);
}

.dl-pill-btn.is-active {
  background: var(--dl-ui-primary, #5e6ad2);
  color: var(--dl-ui-on-primary, #ffffff);
}

.dl-unsung-row,
.dl-toggle-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 5px;
  flex: 1;
}

.dl-unsung-chip,
.dl-toggle-chip {
  height: 24px;
  padding: 0 9px;
  border-radius: 999px;
  border: none;
  background: var(--dl-ui-chip);
  color: var(--dl-ui-text-2);
  font-size: 11.5px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  white-space: nowrap;
  transition:
    background 0.16s ease,
    color 0.16s ease;
}

.dl-unsung-chip:hover,
.dl-toggle-chip:hover {
  background: var(--dl-ui-chip-hover);
  color: var(--dl-ui-text-1);
}

.dl-unsung-chip.is-active,
.dl-toggle-chip.is-active {
  background: var(--dl-ui-primary-soft);
  color: var(--dl-ui-primary-text);
}

.dl-toggle-chip:disabled {
  opacity: 0.42;
  cursor: not-allowed;
}

.dl-mini-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dl-slider-box {
  flex: 1;
  min-width: 185px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.dl-compact-slider {
  max-width: 240px;
}

.dl-range-slider {
  flex: 1;
  min-width: 72px;
  height: 5px;
  border-radius: 999px;
  appearance: none;
  -webkit-appearance: none;
  outline: none;
  cursor: pointer;
  background: linear-gradient(
    90deg,
    var(--dl-ui-primary, #5e6ad2) 0%,
    var(--dl-ui-secondary, #38bdf8) var(--dl-range-pct, 50%),
    var(--dl-ui-chip) var(--dl-range-pct, 50%),
    var(--dl-ui-chip) 100%
  );
}

.dl-range-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  border: none;
  background: var(--dl-ui-primary, #5e6ad2);
  box-shadow: 0 2px 6px var(--dl-accent-glow);
  cursor: pointer;
  transition: transform 0.15s cubic-bezier(0.22, 1.4, 0.36, 1);
}

.dl-range-slider::-webkit-slider-thumb:hover {
  transform: scale(1.16);
}

.dl-num-val {
  min-width: 36px;
  text-align: right;
  font-size: 11.5px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--dl-ui-text-2);
}

.dl-num-val.wide {
  min-width: 44px;
}

/* 提示淡入淡出 */
.dl-tip-fade-enter-active,
.dl-tip-fade-leave-active {
  transition:
    opacity 0.18s ease,
    transform 0.18s cubic-bezier(0.22, 1, 0.36, 1);
}

.dl-tip-fade-enter-from,
.dl-tip-fade-leave-to {
  opacity: 0;
  transform: translate3d(-50%, -4px, 0);
}

.dl-hover-tip.tip-left.dl-tip-fade-enter-from,
.dl-hover-tip.tip-left.dl-tip-fade-leave-to,
.dl-hover-tip.tip-right.dl-tip-fade-enter-from,
.dl-hover-tip.tip-right.dl-tip-fade-leave-to {
  transform: translate3d(0, -4px, 0);
}

.dl-fade-soft-enter-active,
.dl-fade-soft-leave-active {
  transition: opacity 0.22s ease;
}

.dl-fade-soft-enter-from,
.dl-fade-soft-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .dl-row-slot,
  .dl-row-glide-enter-active,
  .dl-row-glide-leave-active,
  .dl-row-scale-inner,
  .dl-toolbar,
  .dl-settings-collapse,
  .dl-preset-tile,
  .dl-icon-btn {
    transition-property: opacity, background, color;
    transition-duration: 0.14s;
  }

  .dl-row-glide-enter-from,
  .dl-row-glide-leave-to {
    transform: translate3d(0, calc(var(--row-y-px, 64px) - 50%), 0);
  }
}
</style>

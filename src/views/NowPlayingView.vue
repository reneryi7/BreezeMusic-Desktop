<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useAppStore } from "../stores/appStore";
import type {
  LyricLine,
  LyricWord,
  PlayerBackgroundStyle,
  PlayerStageStyle,
  SongAuthor,
} from "../types";
import {
  buildBorderlessCssVars,
  hexWithAlpha,
  lerpHex,
  lerpOklabHex,
  normalizeCoverUrl,
  normalizeForDarkMeshHex,
  resolveSemanticColorRoles,
  tuneAccentForDarkStage,
} from "../utils/coverPalette";
import { NowPlayingBgEngine } from "../utils/nowPlayingBgEngine";
import {
  resolveLyricTypographyStyle,
  resolveSungLyricPalette,
  resolveUnsungLyricHex,
} from "../utils/lyricTypography";
import {
  formatCommentCount,
  formatDuration,
  splitSongAuthors,
} from "../api/kugou";
import {
  detectPhoneticLanguage,
  resolveLinePhoneticText,
} from "../utils/lyricPhonetic";
import { getMotionRuntimeProfile } from "../utils/motionRuntime";
import {
  isAppForegroundActive,
  subscribeAppPowerVisibility,
} from "../utils/windowPowerState";
import {
  computeLineTargetRest,
  LyricLineCascadeBank,
  stepSpringScalar,
  WordPhysicsBank,
  type LineCascadeOptions,
  type WordPhysicsOptions,
} from "../utils/lyricMotionEngine";
import NowPlayingCoverStage from "../components/NowPlayingCoverStage.vue";
import NowPlayingDialogs from "../components/NowPlayingDialogs.vue";
import PlumpIcon from "../components/PlumpIcon.vue";

const store = useAppStore();

// ================= 1. 弹窗与浮层开关状态 & 顶部导航10秒自动隐藏 =================
const showBgStyleMenu = ref(false);
const showLyricSettings = ref(false);
const showMoreSheet = ref(false);
const showQualityDialog = ref(false);
const showAudioEffectDialog = ref(false);

const isTopNavAutoHidden = ref(false);
const isHoveringTopNav = ref(false);
let topNavIdleTimer: number | null = null;
const TOP_NAV_IDLE_HIDE_MS = 10000;

function clearTopNavIdleTimer() {
  if (topNavIdleTimer !== null) {
    window.clearTimeout(topNavIdleTimer);
    topNavIdleTimer = null;
  }
}

function scheduleTopNavAutoHide() {
  clearTopNavIdleTimer();
  topNavIdleTimer = window.setTimeout(() => {
    topNavIdleTimer = null;
    if (!isHoveringTopNav.value && !showBgStyleMenu.value) {
      isTopNavAutoHidden.value = true;
    }
  }, TOP_NAV_IDLE_HIDE_MS);
}

function wakeUpTopNav() {
  if (isTopNavAutoHidden.value) {
    isTopNavAutoHidden.value = false;
  }
  scheduleTopNavAutoHide();
}

function handleRootMouseMove(e: MouseEvent) {
  if (e.clientY <= 88) {
    wakeUpTopNav();
  }
}

function handleTopNavMouseEnter() {
  isHoveringTopNav.value = true;
  wakeUpTopNav();
}

function handleTopNavMouseLeave() {
  isHoveringTopNav.value = false;
  scheduleTopNavAutoHide();
}

// ================= 2. 封面动态色阶与高对比度声学色彩计算（智能取色 + 四主题 L1/L2 全面接管） =================
const semanticRoles = computed(() =>
  resolveSemanticColorRoles(
    store.effectiveCoverPalette,
    !store.isNowPlayingLightStage
  )
);

const palette = computed(() => {
  const raw = store.effectiveCoverPalette;
  const roles = semanticRoles.value;
  const isLight = store.isNowPlayingLightStage;
  const dom = raw.dominantAccent || roles.dominantAccent || "#4A9EFF";
  const sec = raw.secondaryAccent || roles.secondaryAccent || "#38D2C0";
  const ter =
    raw.tertiaryAccent ||
    roles.tertiaryAccent ||
    raw.secondaryAccent ||
    "#8B80F9";
  const quat =
    raw.quaternaryAccent ||
    roles.quaternaryAccent ||
    lerpOklabHex(sec, ter, 0.48);
  const rawMesh1 = roles.nebulaPrimary || raw.meshPrimary || "#141D2B";
  const rawMesh2 = roles.nebulaSecondary || raw.meshSecondary || "#0E1722";
  const rawMesh3 =
    roles.nebulaTertiary ||
    raw.meshTertiary ||
    raw.meshSecondary ||
    "#1B1828";
  const mesh1 = isLight
    ? rawMesh1
    : normalizeForDarkMeshHex(rawMesh1, 0.26, 0.66, 0.15, 0.27, 0);
  const mesh2 = isLight
    ? rawMesh2
    : normalizeForDarkMeshHex(rawMesh2, 0.24, 0.64, 0.13, 0.25, 0);
  const mesh3 = isLight
    ? rawMesh3
    : normalizeForDarkMeshHex(rawMesh3, 0.24, 0.62, 0.14, 0.26, 0);
  const abyss =
    raw.bgAbyss ||
    roles.bgAbyss ||
    roles.nebulaAbyss ||
    (isLight ? "#E4EBF7" : "#05070C");

  return {
    dominantAccent: dom,
    secondaryAccent: sec,
    tertiaryAccent: ter,
    quaternaryAccent: quat,
    rawDominantAccent: raw.rawDominantAccent || dom,
    rawSecondaryAccent: raw.rawSecondaryAccent || sec,
    onDominantAccent: raw.onDominantAccent || roles.onDominantAccent || "#FFFFFF",
    onSecondaryAccent: raw.onSecondaryAccent || roles.onSecondaryAccent || "#080B11",
    isMonochrome: Boolean(raw.isMonochrome),
    accentGlowRgba: raw.accentGlowRgba || hexWithAlpha(dom, 0.38),
    accentSubtleRgba: raw.accentSubtleRgba || hexWithAlpha(dom, 0.16),
    accentSecondaryGlowRgba: raw.accentSecondaryGlowRgba || hexWithAlpha(sec, 0.36),
    accentSecondarySubtleRgba: raw.accentSecondarySubtleRgba || hexWithAlpha(sec, 0.16),
    meshPrimary: mesh1,
    meshSecondary: mesh2,
    meshTertiary: mesh3,
    bgPrimary: raw.bgPrimary || roles.bgPrimary || lerpOklabHex(mesh1, abyss, 0.58),
    bgSecondary: raw.bgSecondary || roles.bgSecondary || lerpOklabHex(mesh2, abyss, 0.54),
    bgTertiary: raw.bgTertiary || roles.bgTertiary || lerpOklabHex(mesh3, abyss, 0.62),
    bgAbyss: abyss,
    nebulaAbyss: roles.nebulaAbyss || abyss,
    // (a) 朦胧画境 (cover_blur)
    canvasPrimary: raw.canvasPrimary || mesh1,
    canvasSecondary: raw.canvasSecondary || mesh2,
    canvasAccentGlowRgba: raw.canvasAccentGlowRgba || hexWithAlpha(dom, 0.24),
    canvasVignetteRgba: raw.canvasVignetteRgba || hexWithAlpha(abyss, 0.68),
    // (b) 流体极光 (fluid_aurora)
    fluidOrb1: raw.fluidOrb1 || roles.fluidOrb1 || dom,
    fluidOrb2: raw.fluidOrb2 || roles.fluidOrb2 || sec,
    fluidOrb3: raw.fluidOrb3 || roles.fluidOrb3 || ter,
    fluidOrb4: raw.fluidOrb4 || roles.fluidOrb4 || quat,
    fluidBlendGlowRgba: raw.fluidBlendGlowRgba || roles.fluidBlendGlowRgba || hexWithAlpha(sec, 0.36),
    // (c) 声域浮光 (rhythm_halo)
    soundwavePrimary: raw.soundwavePrimary || roles.soundwavePrimary || dom,
    soundwaveSecondary: raw.soundwaveSecondary || roles.soundwaveSecondary || sec,
    soundwaveTertiary: raw.soundwaveTertiary || roles.soundwaveTertiary || ter,
    soundwaveCoreGlowRgba: raw.soundwaveCoreGlowRgba || roles.soundwaveCoreGlowRgba || hexWithAlpha(dom, 0.32),
    soundwaveRingStrokeRgba: raw.soundwaveRingStrokeRgba || roles.soundwaveRingStrokeRgba || hexWithAlpha(sec, 0.52),
    // (d) 暗夜星尘 (starlight_nebula)
    starlightPrimary: raw.starlightPrimary || roles.starlightPrimary || dom,
    starlightSecondary: raw.starlightSecondary || roles.starlightSecondary || sec,
    starlightTertiary: raw.starlightTertiary || roles.starlightTertiary || ter,
    starlightWarmGold: raw.isMonochrome
      ? raw.quaternaryAccent || "#C8CDD6"
      : raw.starlightWarmGold || roles.starlightWarmGold || "#FFD56B",
    nebulaGlow: raw.nebulaGlow || roles.nebulaGlow || dom,
    nebulaGlowRgba: raw.nebulaGlowRgba || roles.nebulaGlowRgba || hexWithAlpha(dom, 0.28),
    nebulaSecondaryGlowRgba: raw.nebulaSecondaryGlowRgba || roles.nebulaSecondaryGlowRgba || hexWithAlpha(sec, 0.24),
    // 歌词与卡片
    lyricActiveHex: raw.lyricActiveHex || roles.lyricActiveHex || roles.lyricPrimary || dom,
    lyricSecondaryHex: raw.lyricSecondaryHex || roles.lyricSecondary || sec,
    lyricGlowRgba: raw.lyricGlowRgba || roles.lyricGlowRgba || hexWithAlpha(dom, 0.36),
    lyricInactiveRgba: raw.lyricInactiveRgba || "rgba(255, 255, 255, 0.36)",
    lyricTranslationRgba: raw.lyricTranslationRgba || "rgba(255, 255, 255, 0.64)",
    cardTintRgba: raw.cardSurfaceGlassRgba || roles.cardSurfaceGlassRgba || hexWithAlpha(mesh1, 0.44),
    cardHoverRgba: raw.cardHoverBgRgba || roles.cardHoverBgRgba || hexWithAlpha(mesh1, 0.56),
    controlActiveHex: dom,
    controlTrackRgba: raw.accentSubtleRgba || hexWithAlpha(dom, 0.24),
    prismCyan: raw.prismCyan || sec,
    prismMagenta: raw.prismMagenta || ter,
    prismViolet: raw.prismViolet || ter,
    prismGold: raw.prismGold || quat,
  };
});

const normalizedCoverUrl = computed(
  () =>
    store.activeHighResCoverUrl ||
    normalizeCoverUrl(store.currentSong?.coverUrl, 1080)
);

const readableAccentPrimary = computed(() => {
  if (store.isNowPlayingLightStage) {
    return palette.value.dominantAccent || "#4F46E5";
  }
  return tuneAccentForDarkStage(palette.value.dominantAccent || "#4A9EFF", {
    surfaceHex: palette.value.bgPrimary || "#080C14",
    minWcagContrast: 3.85,
    maxLuminance: 0.85,
    preserveChroma: true,
  });
});
const readableAccentSecondary = computed(() => {
  if (store.isNowPlayingLightStage) {
    return palette.value.secondaryAccent || "#0284C7";
  }
  return tuneAccentForDarkStage(palette.value.secondaryAccent || "#38D2C0", {
    surfaceHex: palette.value.bgSecondary || "#080C14",
    minWcagContrast: 3.65,
    maxLuminance: 0.85,
    preserveChroma: true,
  });
});
const readableAccentTertiary = computed(() => {
  if (store.isNowPlayingLightStage) {
    return palette.value.tertiaryAccent || "#EC4899";
  }
  return tuneAccentForDarkStage(palette.value.tertiaryAccent || "#8B80F9", {
    surfaceHex: palette.value.bgTertiary || "#080C14",
    minWcagContrast: 3.45,
    maxLuminance: 0.85,
    preserveChroma: true,
  });
});
const readableAccentQuaternary = computed(() => {
  if (store.isNowPlayingLightStage) {
    return palette.value.quaternaryAccent || "#0D9488";
  }
  return tuneAccentForDarkStage(palette.value.quaternaryAccent || "#38D2C0", {
    surfaceHex: palette.value.bgPrimary || "#080C14",
    minWcagContrast: 3.45,
    maxLuminance: 0.85,
    preserveChroma: true,
  });
});
const vipGold = "#FFD166";

const isLiked = computed(() =>
  store.currentSong ? store.isSongLiked(store.currentSong.hash) : false
);
const likeBtnBurst = ref(false);
function handleToggleFavorite() {
  if (!store.currentSong) return;
  store.toggleFavoriteSong(store.currentSong);
  likeBtnBurst.value = true;
  window.setTimeout(() => {
    likeBtnBurst.value = false;
  }, 420);
}

// 歌词已唱部分主副色（双通道配色工作台：8 款全新高定预设 + 纯色/渐变切换 + 自定义双色）
const lyricHighlightColors = computed(() => {
  const isLight = store.isNowPlayingLightStage;
  const darkSurface = palette.value.bgPrimary || "#080C14";
  const dynamicPrimary = isLight
    ? readableAccentPrimary.value
    : tuneAccentForDarkStage(
        palette.value.lyricActiveHex || palette.value.dominantAccent || "#7C8CF8",
        {
          surfaceHex: darkSurface,
          minWcagContrast: 4.35,
          maxLuminance: 0.88,
          preserveChroma: true,
        }
      );
  const dynamicSecondary = isLight
    ? readableAccentSecondary.value
    : tuneAccentForDarkStage(
        palette.value.lyricSecondaryHex || palette.value.secondaryAccent || "#38BDF8",
        {
          surfaceHex: darkSurface,
          minWcagContrast: 4.15,
          maxLuminance: 0.88,
          preserveChroma: true,
        }
      );
  return resolveSungLyricPalette({
    themeId: store.lyricTheme,
    sungMode: store.lyricSungMode,
    customPrimary: store.lyricCustomPrimaryHex,
    customSecondary: store.lyricCustomSecondaryHex,
    isLightStage: isLight,
    dynamicPrimary,
    dynamicSecondary,
    dynamicTertiary: readableAccentTertiary.value,
  });
});

// 歌词未唱部分独立主色（6 款精选预设 + 自定义取色）
const lyricUnsungBaseHex = computed(() =>
  resolveUnsungLyricHex({
    preset: store.lyricUnsungPreset,
    customHex: store.lyricUnsungCustomHex,
    isLightStage: store.isNowPlayingLightStage,
    sungPrimaryHex: lyricHighlightColors.value.primary,
    stageInkHex: store.effectiveCoverPalette.inkPrimary,
  })
);

// 四档真实 Windows 字重 + 字形风格解析（优先响应「字体与排版」中的播放页歌词自定义/联动字体）
const lyricTypographyActive = computed(() =>
  resolveLyricTypographyStyle({
    family: store.lyricFontFamily,
    weight: store.lyricFontWeight || 760,
    customFontFamily: store.stageLyricFontOverride.familyStack,
    customFontWeight: store.stageLyricFontOverride.weightOverride,
    isGradientText: true,
    strokeTintHex: lyricHighlightColors.value.primary,
  })
);

const lyricTypographyInactive = computed(() =>
  resolveLyricTypographyStyle({
    family: store.lyricFontFamily,
    weight: store.lyricFontWeight || 760,
    customFontFamily: store.stageLyricFontOverride.familyStack,
    customFontWeight: store.stageLyricFontOverride.weightOverride,
    isGradientText: true,
    strokeTintHex: lyricUnsungBaseHex.value,
  })
);

// ================= 3. 智能识别外语翻译与音译按钮显隐 =================
const shouldShowTranslationButton = computed(() => {
  const lrc = store.currentLyric;
  if (!lrc || !lrc.lines || lrc.lines.length === 0) return false;
  const hasAnyTrans =
    lrc.hasTranslation ||
    lrc.lines.some((line) => Boolean(line.translation?.trim()));
  if (!hasAnyTrans) return false;

  let cjkCount = 0;
  let kanaCount = 0;
  let hangulCount = 0;
  let latinCount = 0;
  let otherForeignCount = 0;

  for (const line of lrc.lines) {
    const txt = line.text || "";
    for (let i = 0; i < txt.length; i++) {
      const code = txt.charCodeAt(i);
      if (
        (code >= 0x3040 && code <= 0x309f) ||
        (code >= 0x30a0 && code <= 0x30ff) ||
        (code >= 0x31f0 && code <= 0x31ff)
      ) {
        kanaCount++;
      } else if (
        (code >= 0xac00 && code <= 0xd7af) ||
        (code >= 0x1100 && code <= 0x11ff) ||
        (code >= 0x3130 && code <= 0x318f)
      ) {
        hangulCount++;
      } else if (
        (code >= 0x4e00 && code <= 0x9fff) ||
        (code >= 0x3400 && code <= 0x4dbf)
      ) {
        cjkCount++;
      } else if (
        (code >= 0x0041 && code <= 0x005a) ||
        (code >= 0x0061 && code <= 0x007a) ||
        (code >= 0x00c0 && code <= 0x024f)
      ) {
        latinCount++;
      } else if (
        (code >= 0x0400 && code <= 0x04ff) ||
        (code >= 0x0e00 && code <= 0x0e7f)
      ) {
        otherForeignCount++;
      }
    }
  }
  const totalLetters =
    cjkCount + kanaCount + hangulCount + latinCount + otherForeignCount;
  if (totalLetters === 0) return false;
  if (kanaCount >= 3 || hangulCount >= 3 || otherForeignCount >= 4) return true;
  const foreignRatio =
    (latinCount + kanaCount + hangulCount + otherForeignCount) / totalLetters;
  return foreignRatio >= 0.2;
});

const detectedPhoneticLang = computed(() =>
  detectPhoneticLanguage(store.currentLyric, store.currentSong)
);

const shouldShowPhoneticButton = computed(
  () => detectedPhoneticLang.value !== null
);

function getLinePhoneticText(line: LyricLine): string {
  return resolveLinePhoneticText(line, detectedPhoneticLang.value);
}

// ================= 4. 舞台模式与 4 大高定光学背景切换菜单 =================
const stageModes: { id: PlayerStageStyle; label: string }[] = [
  { id: "gallery_poster", label: "画廊" },
  { id: "vinyl_turntable", label: "黑胶" },
  { id: "pure_lyric", label: "歌词" },
];

const bgQuickList: {
  id: PlayerBackgroundStyle;
  label: string;
  subtitle: string;
}[] = [
  { id: "fluid_aurora", label: "液态丝绸", subtitle: "三核封面流变" },
  { id: "rhythm_fluted", label: "长虹声幕", subtitle: "长虹玻璃光墙" },
  { id: "rhythm_cymatic", label: "声致驻波", subtitle: "封面丝绸声波" },
  { id: "starlight_nebula", label: "星尘梦境", subtitle: "星云景深散景" },
];

const currentBgLabel = computed(() => {
  const found = bgQuickList.find((item) => item.id === store.playerBgStyle);
  return found ? found.label : "液态丝绸";
});

const coverColorModeLabel = computed(() => {
  if (store.coverColorStageMode === "off") return "主题配色";
  if (store.coverColorStageMode === "dark_lock") return "封面 · 锁暗室";
  return "封面 · 随明暗";
});

const coverColorModeTooltip = computed(() => {
  if (store.coverColorStageMode === "off") {
    return "当前：跟随软件四套主题（点击切换为：封面取色 · 随明暗）";
  }
  if (store.coverColorStageMode === "follow") {
    return "当前：封面取色 · 随明暗（浅色入封面漫染，深色入暗室剧场；点击切换为：锁暗室）";
  }
  return "当前：封面取色 · 锁暗室（始终深色封面剧场；点击关闭并回到软件主题配色）";
});

// 音质胶囊文案与流金态（严格对齐概念版枚举："high"=Hi-Res / "flac"=SQ 无损 / "320"=320K HQ / "128"=128K 标准）
const currentQualityLabel = computed(() => {
  if (store.currentPlaybackInfo?.qualityLabel) {
    return store.currentPlaybackInfo.qualityLabel;
  }
  const map: Record<string, string> = {
    high: "Hi-Res",
    hires: "Hi-Res",
    flac: "SQ 无损",
    "320": "高品 320K",
    "128": "标准 128K",
  };
  return map[store.preferredQuality] || "高品 320K";
});

const isGoldQuality = computed(() => {
  const q = store.currentPlaybackInfo?.actualQuality || store.preferredQuality;
  return (
    q === "high" ||
    q === "hires" ||
    currentQualityLabel.value.includes("Hi-Res")
  );
});

// 整合音质标识与音频文件采样规格（从进度条下方迁移至歌手行内）
const compactAudioSpecLabel = computed(() => {
  const info = store.currentPlaybackInfo;
  const qLabel = currentQualityLabel.value;
  if (info?.techSpecText) {
    const clean = info.techSpecText
      .replace(/64-Bit Floating DSP\s*[·|]\s*/i, "")
      .replace(/[·|]\s*KRC 逐字音画同步/i, "")
      .replace(/KRC 逐字音画同步/i, "")
      .trim();
    if (clean && clean !== qLabel) {
      return `${qLabel} · ${clean}`;
    }
  }
  const q = info?.actualQuality || store.preferredQuality;
  if (q === "high" || q === "hires") return `${qLabel} · 96kHz/24bit FLAC`;
  if (q === "flac") return `${qLabel} · 44.1kHz/16bit FLAC`;
  if (q === "320") return `${qLabel} · 320kbps MP3`;
  return `${qLabel} · 128kbps MP3`;
});

const isAudioEffectActive = computed(
  () =>
    store.masterEffectEnabled ||
    store.activeViperEffectId !== "off" ||
    store.activeEqPreset !== "flat" ||
    store.acousticField.reverbPreset !== "none" ||
    store.acousticField.surroundWidth > 0 ||
    store.acousticField.subBassDepth > 0 ||
    store.acousticField.stageDepth > 0
);

// 评论计数角标格式化（独立读取当前播放曲目评论数，与评论弹窗浏览解耦）
const formattedCommentCount = computed(() => {
  const count = store.currentSongCommentTotal || 0;
  if (count <= 0) return "";
  return formatCommentCount(count);
});

// ================= 4.5 歌名 / 多位歌手 / 完整音质参数防截断与平滑展开测量 =================
const songArtistsList = computed<SongAuthor[]>(() => {
  if (!store.currentSong) return [];
  return splitSongAuthors(store.currentSong);
});

const titleViewportRef = ref<HTMLElement | null>(null);
const titleTrackRef = ref<HTMLElement | null>(null);
const isTitleOverflowing = ref(false);
const titleMarqueeDistPx = ref(0);
const titleMarqueeDurationSec = ref(12);

const qualityViewportRef = ref<HTMLElement | null>(null);
const qualityTrackRef = ref<HTMLElement | null>(null);
const isQualityOverflowing = ref(false);
const qualityMarqueeDistPx = ref(0);
const qualityMarqueeDurationSec = ref(10);

let metaResizeObserver: ResizeObserver | null = null;

function measureMetaOverflow() {
  const tBox = titleViewportRef.value;
  const tInner = titleTrackRef.value;
  if (tBox && tInner) {
    const diff = tInner.scrollWidth - tBox.clientWidth;
    if (diff > 3) {
      isTitleOverflowing.value = true;
      titleMarqueeDistPx.value = diff + 28;
      titleMarqueeDurationSec.value = Math.max(8, Math.min(24, (diff + 28) / 22));
    } else {
      isTitleOverflowing.value = false;
      titleMarqueeDistPx.value = 0;
    }
  }

  const qBox = qualityViewportRef.value;
  const qInner = qualityTrackRef.value;
  if (qBox && qInner) {
    const diff = qInner.scrollWidth - qBox.clientWidth;
    if (diff > 3) {
      isQualityOverflowing.value = true;
      qualityMarqueeDistPx.value = diff + 20;
      qualityMarqueeDurationSec.value = Math.max(7, Math.min(18, (diff + 20) / 20));
    } else {
      isQualityOverflowing.value = false;
      qualityMarqueeDistPx.value = 0;
    }
  }
}

watch(
  [
    () => store.currentSong?.hash,
    () => store.currentSong?.title,
    () => compactAudioSpecLabel.value,
    () => store.isNowPlayingOpen,
    () => store.playerStageStyle,
  ],
  () => {
    nextTick(() => {
      measureMetaOverflow();
      window.setTimeout(measureMetaOverflow, 120);
    });
  }
);

// ================= 5. 4 大高定光学背景统一引擎（WebGL2 + Canvas 2D） =================
function parseRgbaToken(
  input: string | undefined,
  fallbackHex: string,
  fallbackAlpha: number
): { hex: string; alpha: number } {
  if (!input) return { hex: fallbackHex, alpha: fallbackAlpha };
  const trimmed = input.trim();
  if (trimmed.startsWith("#")) {
    return { hex: trimmed, alpha: fallbackAlpha };
  }
  const m = trimmed.match(
    /rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)(?:\s*,\s*([\d.]+))?\s*\)/i
  );
  if (!m) return { hex: fallbackHex, alpha: fallbackAlpha };
  const toHex = (v: string) =>
    Math.max(0, Math.min(255, Math.round(Number(v) || 0)))
      .toString(16)
      .padStart(2, "0");
  const hex = `#${toHex(m[1])}${toHex(m[2])}${toHex(m[3])}`;
  const alpha =
    m[4] !== undefined
      ? Math.max(0, Math.min(1, Number(m[4])))
      : fallbackAlpha;
  return { hex, alpha };
}

const backdropRef = ref<HTMLElement | null>(null);
const bgGlCanvasRef = ref<HTMLCanvasElement | null>(null);
const bg2dCanvasRef = ref<HTMLCanvasElement | null>(null);
const bgEngine = new NowPlayingBgEngine();
let bgEngineAttached = false;

function ensureBgEngineAttached() {
  if (bgEngineAttached || !bgGlCanvasRef.value || !bg2dCanvasRef.value) return;
  bgEngineAttached = true;
  bgEngine.attach(bgGlCanvasRef.value, bg2dCanvasRef.value);
  void bgEngine.setCoverUrl(normalizedCoverUrl.value);
}

let coverFocusX = 0.26;
let coverFocusY = 0.54;

function measureCoverFocusPoint() {
  if (store.playerStageStyle === "pure_lyric") {
    coverFocusX = 0.5;
    coverFocusY = 0.5;
    return;
  }
  const stageEl = backdropRef.value;
  const anchorEl =
    typeof document !== "undefined"
      ? document.getElementById("np-flip-target-anchor")
      : null;
  if (!stageEl || !anchorEl) {
    coverFocusX = 0.26;
    coverFocusY = 0.54;
    return;
  }
  const sRect = stageEl.getBoundingClientRect();
  const aRect = anchorEl.getBoundingClientRect();
  if (sRect.width > 20 && sRect.height > 20 && aRect.width > 20) {
    coverFocusX = Math.max(
      0.12,
      Math.min(
        0.88,
        (aRect.left + aRect.width * 0.5 - sRect.left) / sRect.width
      )
    );
    // WebGL UV 原点在左下角，需要 1 - normY
    coverFocusY = Math.max(
      0.15,
      Math.min(
        0.85,
        1 - (aRect.top + aRect.height * 0.5 - sRect.top) / sRect.height
      )
    );
  }
}

function drawDynamicBackground(_nowMs: number, dtSec: number) {
  if (!bgEngineAttached) {
    ensureBgEngineAttached();
  }
  if (!bgEngineAttached) return;
  const pal = palette.value;
  const isLight = store.isNowPlayingLightStage;
  const useCover = Boolean(store.dynamicCoverAccentEnabled);
  const isMono = Boolean(pal.isMonochrome);

  let c1: string;
  let c2: string;
  let c3: string;
  let bg: string;

  if (isLight) {
    if (useCover) {
      // 浅色封面漫染舞台：采用 OKLab 提纯后的饱满水彩颜料 + 动态染宣画布底色
      c1 = pal.fluidOrb1 || pal.dominantAccent;
      c2 = pal.fluidOrb2 || pal.secondaryAccent;
      c3 = pal.fluidOrb3 || pal.tertiaryAccent;
      bg = pal.bgPrimary || "#F6F5F2";
    } else {
      // L1 水彩 / L2 燕麦纸固定浅色主题：采用原生高辨识主题主辅色 + 宣纸/燕麦底色
      c1 = pal.dominantAccent;
      c2 = pal.secondaryAccent;
      c3 = pal.tertiaryAccent;
      bg = pal.bgPrimary || "#F6F8FC";
    }
  } else if (useCover) {
    // 深色封面暗室剧场：原始鲜活封面双主色 + 第三伴色（交由 WebGL2 softKneeDark 选择性高光柔膝护眼）
    if (isMono) {
      c1 = "#D4D8E0";
      c2 = "#8E95A4";
      c3 = "#B0B6C2";
      bg = "#0C0E13";
    } else {
      c1 = pal.rawDominantAccent || pal.dominantAccent;
      c2 = pal.rawSecondaryAccent || pal.secondaryAccent;
      c3 = pal.tertiaryAccent || pal.quaternaryAccent;
      bg = pal.bgPrimary || pal.bgAbyss || "#0C0E14";
    }
  } else {
    // D1 极夜 / D2 丝绒固定深色主题
    c1 = pal.fluidOrb1 || pal.dominantAccent;
    c2 = pal.fluidOrb2 || pal.secondaryAccent;
    c3 = pal.fluidOrb3 || pal.tertiaryAccent;
    bg = pal.bgPrimary || "#0C0E14";
  }

  const tele = store.rhythmTelemetry;
  bgEngine.renderFrame({
    dtSec,
    mode: store.playerBgStyle,
    isLightStage: isLight,
    useCoverArtwork: useCover,
    isMonochrome: isMono,
    isPlaying: store.isPlaying,
    playMomentum: store.playMomentum,
    dim: store.lyricBackgroundDim,
    c1,
    c2,
    c3,
    bg,
    starGold: isMono ? c3 : pal.starlightWarmGold || "#FFE09A",
    bass: tele.bassEnergy || tele.lowEnergy || 0,
    mid: tele.midVocalEnergy || tele.midEnergy || 0,
    high: tele.highAirEnergy || tele.highEnergy || 0,
    beat: tele.beatPulse || 0,
    bands: store.spectrumBars,
    focusX: coverFocusX,
    focusY: coverFocusY,
  });
}

// ================= 6. 自然彗星拖尾进度时间轴与有色副歌区间引擎 =================
const timelineTrackRef = ref<HTMLElement | null>(null);
const timelineFillRef = ref<HTMLElement | null>(null);
const cometHeadCarrierRef = ref<HTMLElement | null>(null);
const isScrubbingProgress = ref(false);
const scrubFraction = ref(0);
const hoverTimelineFraction = ref<number | null>(null);
const settledSeekTargetMs = ref(-1);
const settledSeekWallMs = ref(0);
let liveSampledPosMs = 0;
let liveInterpolatedPosMs = 0;
const displayElapsedSec = ref(0);
let lastWrittenProgressFrac = -1;

function computeProgressFrac(posMs = liveInterpolatedPosMs): number {
  if (isScrubbingProgress.value) return scrubFraction.value;
  if (store.durationMs <= 0) return 0;
  return Math.max(0, Math.min(1, posMs / store.durationMs));
}

function syncDisplayElapsedSec(posMs: number) {
  const nextSec = Math.max(0, Math.floor((posMs || 0) / 1000));
  if (displayElapsedSec.value !== nextSec) {
    displayElapsedSec.value = nextSec;
  }
}

function syncTimelineProgressDom(frac: number, force = false) {
  const clamped = Math.max(0, Math.min(1, frac));
  if (!force && Math.abs(clamped - lastWrittenProgressFrac) < 0.0002) return;
  lastWrittenProgressFrac = clamped;
  const shiftCss = `${((clamped - 1) * 100).toFixed(4)}%`;
  const headOp =
    clamped <= 0.002 ? "0" : clamped < 0.015 ? (clamped / 0.015).toFixed(3) : "1";
  if (timelineFillRef.value) {
    timelineFillRef.value.style.setProperty("--prog-shift", shiftCss);
  }
  if (cometHeadCarrierRef.value) {
    cometHeadCarrierRef.value.style.setProperty("--prog-shift", shiftCss);
    cometHeadCarrierRef.value.style.setProperty("--comet-head-opacity", headOp);
  }
}

const shownElapsedSec = computed(() => {
  if (isScrubbingProgress.value && store.durationMs > 0) {
    return Math.max(0, Math.floor((scrubFraction.value * store.durationMs) / 1000));
  }
  return displayElapsedSec.value;
});

// 副歌高潮有色线段归一化范围（若曲目缺少接口高潮时间点则根据曲目特征生成稳定的黄金副歌区间）
const climaxRange = computed<[number, number] | null>(() => {
  const s = store.currentSong;
  const dur = store.durationMs;
  if (!s) return null;
  const start = s.climaxStartMs || 0;
  const end = s.climaxEndMs || 0;
  if (dur > 0 && start > 0 && end > start && start < dur) {
    return [
      Math.max(0.06, Math.min(0.88, start / dur)),
      Math.max(0.14, Math.min(0.95, end / dur)),
    ];
  }
  // 基于 hash 生成自然稳定的副歌高潮有色区间 (约在 33% ~ 62% 之间)
  const hashStr = s.hash || s.title || "breeze";
  let seed = 0;
  for (let i = 0; i < hashStr.length; i++) {
    seed = (seed * 31 + hashStr.charCodeAt(i)) >>> 0;
  }
  const offset = (seed % 11) * 0.008; // 0 ~ 0.08
  const span = 0.22 + ((seed >> 4) % 7) * 0.012; // 0.22 ~ 0.292
  const st = 0.32 + offset;
  return [st, Math.min(0.76, st + span)];
});

function updateScrubFromClientX(clientX: number) {
  const el = timelineTrackRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  if (rect.width <= 0) return;
  scrubFraction.value = Math.max(
    0,
    Math.min(1, (clientX - rect.left) / rect.width)
  );
  syncTimelineProgressDom(scrubFraction.value, true);
  syncLyricRealtimeDom(getEffectiveLyricTimeMs(), 0.016, true);
}

function handleTimelineMouseMove(e: MouseEvent) {
  const el = timelineTrackRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  if (rect.width <= 0) return;
  hoverTimelineFraction.value = Math.max(
    0,
    Math.min(1, (e.clientX - rect.left) / rect.width)
  );
}

function handleTimelineMouseLeave() {
  hoverTimelineFraction.value = null;
}

function handleProgressPointerDown(e: MouseEvent) {
  if (e.button !== 0 || store.durationMs <= 0) return;
  isScrubbingProgress.value = true;
  updateScrubFromClientX(e.clientX);

  const onMove = (ev: MouseEvent) => {
    updateScrubFromClientX(ev.clientX);
  };
  const onUp = (ev: MouseEvent) => {
    updateScrubFromClientX(ev.clientX);
    const targetMs = Math.round(scrubFraction.value * store.durationMs);
    settledSeekTargetMs.value = targetMs;
    settledSeekWallMs.value = performance.now();
    liveSampledPosMs = targetMs;
    liveInterpolatedPosMs = targetMs;
    syncDisplayElapsedSec(targetMs);
    isScrubbingProgress.value = false;
    syncTimelineProgressDom(computeProgressFrac(targetMs), true);
    syncLyricRealtimeDom(getEffectiveLyricTimeMs(), 0.016, true);
    store.seekToMs(targetMs);
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

// 传输控制微交互与滚轮音量控制
const playModeSpinCount = ref(0);
const prevNudgeX = ref(0);
const nextNudgeX = ref(0);

function triggerSkipNudge(isNext: boolean) {
  if (isNext) {
    nextNudgeX.value = 5;
    store.playNext(false);
    window.setTimeout(() => {
      nextNudgeX.value = 0;
    }, 200);
  } else {
    prevNudgeX.value = -5;
    store.playPrev();
    window.setTimeout(() => {
      prevNudgeX.value = 0;
    }, 200);
  }
}

// ================= 7. 「光晕浮升 · 声浪呼吸」逐字弹簧 & 120Hz 行级瀑布错峰滚动引擎 =================
const lyricViewportRef = ref<HTMLElement | null>(null);
const hoveredLyricIndex = ref<number | null>(null);
const isUserScrollingLyrics = ref(false);
let lyricScrubResetTimer: number | null = null;
let breathingDotPhaseVal = 0;
const activeLineIndex = ref<number>(-1);
const leavingLineIndex = ref<number>(-1);
const activeInterludeVisible = ref<boolean>(false);

// 零每帧 VDOM Diff 与零每帧 querySelector 的直接 DOM 引用缓存
let cachedLineEls: HTMLElement[] = [];
let cachedActiveLineIdx = -1;
let cachedActiveWordEls: HTMLElement[] = [];
let cachedLeavingLineIdx = -1;
let cachedLeavingWordEls: HTMLElement[] = [];
let cachedInterludeDotEls: HTMLElement[] = [];

// 每字独立物理弹簧池（活跃行 + 380ms 退场余韵行）与行级瀑布错峰物理池
const activeWordBank = new WordPhysicsBank();
const leavingWordBank = new WordPhysicsBank();
const lineCascadeBank = new LyricLineCascadeBank();
let leavingStartedWallMs = 0;
let prevCascadeWindowIndices = new Set<number>();

// 120Hz 临界阻尼弹簧滚动状态（配子像素残差补偿消除 1px 收尾微抖）
let lyricTargetScrollTop = 0;
let lyricCurrentScrollTop = 0;
let lyricScrollVelocity = 0;
let lyricScrollInitialized = false;

function getEffectiveLyricTimeMs(): number {
  const baseMs =
    isScrubbingProgress.value && store.durationMs > 0
      ? scrubFraction.value * store.durationMs
      : liveInterpolatedPosMs;
  return Math.max(0, baseMs + store.lyricOffsetMs - store.bluetoothDelayMs);
}

/** O(log N) 二分查找当前时间对应的歌词行号 */
function findLyricLineIndexAt(posMs: number): number {
  const lines = store.currentLyric?.lines;
  if (!lines || lines.length === 0) return -1;
  let lo = 0;
  let hi = lines.length - 1;
  let ans = 0;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (posMs >= lines[mid].startTimeMs) {
      ans = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return ans;
}

function getWordPhysicsOpts(): WordPhysicsOptions {
  const motion = getMotionRuntimeProfile();
  const tok = karaokeRenderTokens.value;
  return {
    wordByWord: tok.wordByWord,
    bounceEnabled: store.lyricWordBounceEnabled !== false,
    motionLevel: motion.effectiveLevel,
    glowRatio: tok.glowRatio,
    isLightStage: store.isNowPlayingLightStage,
  };
}

function getLineCascadeOpts(): LineCascadeOptions {
  const motion = getMotionRuntimeProfile();
  return {
    focusScaleEnabled: store.lyricFocusScaleEnabled !== false,
    focusScaleRatio: store.lyricFocusScaleRatio || 1.08,
    motionLevel: motion.effectiveLevel,
  };
}

function clearWordElementVars(els: HTMLElement[]) {
  for (let i = 0; i < els.length; i++) {
    const el = els[i];
    if (!el) continue;
    el.style.removeProperty("--kw-y");
    el.style.removeProperty("--kw-s");
    el.style.removeProperty("--kw-lit");
    el.style.removeProperty("--kw-glow");
  }
}

function flushWordBankToDom(bank: WordPhysicsBank, els: HTMLElement[], forceAll = false) {
  const n = Math.min(bank.count, els.length);
  for (let i = 0; i < n; i++) {
    const el = els[i];
    if (!el) continue;

    const y = bank.y[i];
    if (forceAll || Number.isNaN(bank.writtenY[i]) || Math.abs(y - bank.writtenY[i]) >= 0.025) {
      bank.writtenY[i] = y;
      el.style.setProperty("--kw-y", `${y.toFixed(2)}px`);
    }

    const s = bank.scale[i];
    if (forceAll || Number.isNaN(bank.writtenScale[i]) || Math.abs(s - bank.writtenScale[i]) >= 0.0008) {
      bank.writtenScale[i] = s;
      el.style.setProperty("--kw-s", s.toFixed(4));
    }

    const lit = bank.lit[i];
    if (forceAll || Number.isNaN(bank.writtenLit[i]) || Math.abs(lit - bank.writtenLit[i]) >= 0.004) {
      bank.writtenLit[i] = lit;
      el.style.setProperty("--kw-lit", lit.toFixed(3));
    }

    const glow = bank.glow[i];
    if (forceAll || Number.isNaN(bank.writtenGlow[i]) || Math.abs(glow - bank.writtenGlow[i]) >= 0.004) {
      bank.writtenGlow[i] = glow;
      el.style.setProperty("--kw-glow", glow.toFixed(3));
    }
  }
}

function refreshCachedLineElements() {
  const viewport = lyricViewportRef.value;
  if (!viewport) {
    cachedLineEls = [];
    return;
  }
  cachedLineEls = Array.from(
    viewport.querySelectorAll<HTMLElement>(".np-lyric-line-item")
  );
}

function getCachedLineEl(idx: number): HTMLElement | null {
  if (idx < 0) return null;
  const cached = cachedLineEls[idx];
  if (cached && cached.isConnected) return cached;
  refreshCachedLineElements();
  const retried = cachedLineEls[idx];
  return retried && retried.isConnected ? retried : null;
}

function invalidateActiveLyricDomCache() {
  cachedActiveLineIdx = -1;
  cachedActiveWordEls = [];
  cachedInterludeDotEls = [];
}

function clearLeavingLineState() {
  if (cachedLeavingWordEls.length > 0) {
    clearWordElementVars(cachedLeavingWordEls);
  }
  cachedLeavingLineIdx = -1;
  cachedLeavingWordEls = [];
  leavingWordBank.ensureSize(0);
  if (leavingLineIndex.value !== -1) {
    leavingLineIndex.value = -1;
  }
}

function ensureActiveLyricDomCache(lineIdx: number) {
  if (lineIdx < 0) {
    invalidateActiveLyricDomCache();
    return;
  }
  if (
    cachedActiveLineIdx === lineIdx &&
    cachedActiveWordEls.length > 0 &&
    cachedActiveWordEls[0]?.isConnected
  ) {
    return;
  }
  const lineEl = getCachedLineEl(lineIdx);
  if (!lineEl) {
    invalidateActiveLyricDomCache();
    return;
  }
  const spans = Array.from(
    lineEl.querySelectorAll<HTMLElement>(".np-karaoke-word-span")
  );
  cachedActiveLineIdx = lineIdx;
  cachedActiveWordEls = spans;
  activeWordBank.invalidateDomCache();
  cachedInterludeDotEls = Array.from(
    lineEl.querySelectorAll<HTMLElement>(".np-interlude-dot")
  );
}

function computeLineTargetScrollTop(lineIdx: number): number | null {
  const viewport = lyricViewportRef.value;
  if (!viewport) return null;
  if (lineIdx < 0) return 0;
  const lineEl = getCachedLineEl(lineIdx);
  if (!lineEl) return null;
  const viewportH = viewport.clientHeight;
  return Math.max(
    0,
    lineEl.offsetTop - viewportH * 0.41 + lineEl.offsetHeight * 0.5
  );
}

function clearCascadeInlineTransforms() {
  for (const idx of prevCascadeWindowIndices) {
    const el = cachedLineEls[idx];
    if (el && el.isConnected) {
      el.style.removeProperty("transform");
    }
  }
  prevCascadeWindowIndices.clear();
}

function scrollToActiveLyricLine(smooth = true) {
  if (isUserScrollingLyrics.value) return;
  const viewport = lyricViewportRef.value;
  if (!viewport) return;
  const activeIdx = activeLineIndex.value;
  const targetTop = computeLineTargetScrollTop(activeIdx);
  if (targetTop === null) return;
  lyricTargetScrollTop = targetTop;

  if (!smooth || !lyricScrollInitialized) {
    lyricCurrentScrollTop = targetTop;
    lyricScrollVelocity = 0;
    viewport.scrollTop = Math.round(targetTop);
    lyricScrollInitialized = true;
    lineCascadeBank.clear();
    clearCascadeInlineTransforms();
  }
}

/** 换行核心编排：无缝移交 380ms 退场余韵 + 触发行级弹性瀑布错峰滚动 */
function transitionActiveLyricLine(
  prevIdx: number,
  nextIdx: number,
  posMs: number,
  isSeekOrJump = false,
) {
  const lines = store.currentLyric?.lines || [];
  const motion = getMotionRuntimeProfile();

  // 1. 若有正在退场的更早一行，先干净清理其残留变量
  if (cachedLeavingWordEls.length > 0 && cachedLeavingLineIdx !== prevIdx) {
    clearWordElementVars(cachedLeavingWordEls);
    cachedLeavingWordEls = [];
    cachedLeavingLineIdx = -1;
  }

  // 2. 正常顺序播进时，把旧活跃行的逐字弹簧状态无缝移交给退场余韵池（DOM 节点不断裂零闪烁）
  if (
    !isSeekOrJump &&
    prevIdx >= 0 &&
    prevIdx !== nextIdx &&
    motion.effectiveLevel !== "none" &&
    cachedActiveLineIdx === prevIdx &&
    cachedActiveWordEls.length > 0
  ) {
    leavingWordBank.copyFrom(activeWordBank);
    cachedLeavingLineIdx = prevIdx;
    cachedLeavingWordEls = cachedActiveWordEls;
    leavingStartedWallMs = performance.now();
    leavingLineIndex.value = prevIdx;
  } else {
    clearLeavingLineState();
  }

  // 3. 切换活跃行并同步准备新行的逐字弹簧状态
  activeLineIndex.value = nextIdx;
  invalidateActiveLyricDomCache();

  const nextLine = nextIdx >= 0 ? lines[nextIdx] : undefined;
  const nextWords = nextIdx >= 0 ? resolvedLyricWordsList.value[nextIdx] : undefined;
  if (nextLine && nextWords && nextWords.length > 0) {
    if (isSeekOrJump || motion.effectiveLevel === "none") {
      activeWordBank.snapToTime(nextLine.startTimeMs, nextWords, posMs, getWordPhysicsOpts());
    } else {
      // 顺序换行从 0 平滑起跳，再由后续帧弹簧托举，丝滑无突兀闪现
      activeWordBank.ensureSize(nextWords.length);
      activeWordBank.y.fill(0);
      activeWordBank.vy.fill(0);
      activeWordBank.scale.fill(1);
      activeWordBank.vScale.fill(0);
      activeWordBank.lit.fill(0);
      activeWordBank.glow.fill(0);
      activeWordBank.invalidateDomCache();
    }
  } else {
    activeWordBank.ensureSize(0);
  }

  // 4. 触发 AMLL 式行级弹性瀑布错峰脉冲并推进滚动目标
  const nextTargetTop = computeLineTargetScrollTop(nextIdx);
  if (nextTargetTop !== null && !isUserScrollingLyrics.value) {
    const scrollDelta = nextTargetTop - lyricCurrentScrollTop;
    lyricTargetScrollTop = nextTargetTop;
    if (!lyricScrollInitialized) {
      lyricCurrentScrollTop = nextTargetTop;
      lyricScrollVelocity = 0;
      if (lyricViewportRef.value) {
        lyricViewportRef.value.scrollTop = Math.round(nextTargetTop);
      }
      lyricScrollInitialized = true;
    } else {
      lineCascadeBank.triggerLineAdvance(
        prevIdx,
        nextIdx,
        scrollDelta,
        lines.length,
        getLineCascadeOpts(),
      );
    }
  }

  nextTick(() => {
    refreshCachedLineElements();
    ensureActiveLyricDomCache(nextIdx);
    flushWordBankToDom(activeWordBank, cachedActiveWordEls, true);
    if (cachedLeavingLineIdx >= 0 && cachedLeavingWordEls.length > 0) {
      flushWordBankToDom(leavingWordBank, cachedLeavingWordEls, false);
    }
    if (!isUserScrollingLyrics.value) {
      const accurateTop = computeLineTargetScrollTop(nextIdx);
      if (accurateTop !== null) {
        lyricTargetScrollTop = accurateTop;
      }
    }
  });
}

watch(
  () => store.currentLyric,
  () => {
    clearLeavingLineState();
    invalidateActiveLyricDomCache();
    lineCascadeBank.clear();
    clearCascadeInlineTransforms();
    cachedLineEls = [];
    const posMs = getEffectiveLyricTimeMs();
    const initialIdx = findLyricLineIndexAt(posMs);
    activeLineIndex.value = initialIdx;
    nextTick(() => {
      refreshCachedLineElements();
      scrollToActiveLyricLine(false);
      syncLyricRealtimeDom(posMs, 0.016, true);
    });
  }
);

watch(
  [
    () => store.lyricFontSize,
    () => store.lyricLineSpacing,
    () => store.showTranslation,
    () => store.showPhonetic,
    () => store.playerStageStyle,
  ],
  () => {
    nextTick(() => {
      refreshCachedLineElements();
      scrollToActiveLyricLine(true);
      syncLyricRealtimeDom(getEffectiveLyricTimeMs(), 0.016, true);
    });
  }
);

watch(
  [
    () => store.lyricTheme,
    () => store.lyricSungMode,
    () => store.lyricWordByWord,
    () => store.lyricWordBounceEnabled,
    () => store.lyricFocusScaleEnabled,
    () => store.lyricFocusScaleRatio,
    () => store.lyricGlowIntensity,
    () => store.lyricInactiveOpacity,
    () => store.lyricUnsungPreset,
    () => store.lyricUnsungCustomHex,
    () => store.lyricCustomPrimaryHex,
    () => store.lyricCustomSecondaryHex,
    () => store.lyricFontWeight,
    () => store.isNowPlayingLightStage,
    () => store.effectiveCoverPalette.dominantAccent,
    () => store.effectiveCoverPalette.secondaryAccent,
  ],
  () => {
    nextTick(() => {
      syncLyricRealtimeDom(getEffectiveLyricTimeMs(), 0.016, true);
    });
  }
);

watch(
  () => store.currentSong?.hash,
  () => {
    isUserScrollingLyrics.value = false;
    lyricScrollInitialized = false;
    lyricScrollVelocity = 0;
    settledSeekTargetMs.value = -1;
    lastAudioSampleMs = 0;
    audioAnchorPosMs = 0;
    audioAnchorWallMs = performance.now();
    liveSampledPosMs = store.currentPositionMs;
    liveInterpolatedPosMs = store.currentPositionMs;
    clearLeavingLineState();
    invalidateActiveLyricDomCache();
    lineCascadeBank.clear();
    clearCascadeInlineTransforms();
    cachedLineEls = [];
    const posMs = getEffectiveLyricTimeMs();
    activeLineIndex.value = findLyricLineIndexAt(posMs);
    syncDisplayElapsedSec(store.currentPositionMs);
    syncTimelineProgressDom(computeProgressFrac(store.currentPositionMs), true);
    nextTick(() => {
      refreshCachedLineElements();
      scrollToActiveLyricLine(false);
      syncLyricRealtimeDom(posMs, 0.016, true);
    });
  }
);

function handleUserLyricWheel() {
  const viewport = lyricViewportRef.value;
  if (viewport) {
    lyricCurrentScrollTop = viewport.scrollTop;
    lyricTargetScrollTop = viewport.scrollTop;
    lyricScrollVelocity = 0;
  }
  lineCascadeBank.dampenForUserScroll();
  isUserScrollingLyrics.value = true;
  if (lyricScrubResetTimer) window.clearTimeout(lyricScrubResetTimer);
  const autoReturnMs = Math.max(1500, (store.lyricAutoReturnSec || 3) * 1000);
  lyricScrubResetTimer = window.setTimeout(() => {
    if (lyricViewportRef.value) {
      lyricCurrentScrollTop = lyricViewportRef.value.scrollTop;
      lyricScrollVelocity = 0;
    }
    isUserScrollingLyrics.value = false;
    scrollToActiveLyricLine(true);
  }, autoReturnMs);
}

function handleClickLyricLine(line: LyricLine) {
  if (!store.lyricClickSeekEnabled) return;
  const targetMs = Math.max(
    0,
    line.startTimeMs - store.lyricOffsetMs + store.bluetoothDelayMs
  );
  settledSeekTargetMs.value = targetMs;
  settledSeekWallMs.value = performance.now();
  liveSampledPosMs = targetMs;
  liveInterpolatedPosMs = targetMs;
  syncDisplayElapsedSec(targetMs);
  syncTimelineProgressDom(computeProgressFrac(targetMs), true);
  if (lyricViewportRef.value) {
    lyricCurrentScrollTop = lyricViewportRef.value.scrollTop;
    lyricScrollVelocity = 0;
  }
  isUserScrollingLyrics.value = false;
  const effMs = getEffectiveLyricTimeMs();
  const prevIdx = activeLineIndex.value;
  const nextIdx = findLyricLineIndexAt(effMs);
  transitionActiveLyricLine(prevIdx, nextIdx, effMs, true);
  store.seekToMs(targetMs);
  if (!store.isPlaying) {
    store.togglePlayPause();
  }
  nextTick(() => scrollToActiveLyricLine(true));
}

function getLyricLineContainerStyle(idx: number) {
  const activeIdx = activeLineIndex.value;
  const signedDist = idx - activeIdx;
  const dist = Math.abs(signedDist);
  const isCurrent = idx === activeIdx;
  const isPast = signedDist < 0;
  const baseInactive = Math.max(0.15, Math.min(0.85, store.lyricInactiveOpacity || 0.36));
  const scaleFactor = baseInactive / 0.36;

  let alpha = Math.min(0.82, 0.22 * scaleFactor);
  if (isCurrent) {
    alpha = 1.0;
  } else if (isPast) {
    alpha = dist === 1
      ? Math.min(0.76, 0.42 * scaleFactor)
      : Math.min(0.58, 0.26 * scaleFactor);
  } else if (dist === 1) {
    alpha = Math.min(0.92, 0.56 * scaleFactor);
  } else if (dist === 2) {
    alpha = Math.min(0.84, 0.38 * scaleFactor);
  } else if (dist === 3) {
    alpha = Math.min(0.74, 0.28 * scaleFactor);
  }

  // 行级景深与静息缩放：可视化窗口内由 120Hz 物理弹簧直接驱动 transform，窗口外由 CSS 变量承接零冲突
  const focusScaleOn = store.lyricFocusScaleEnabled !== false;
  const rest = computeLineTargetRest(
    idx,
    activeIdx,
    focusScaleOn,
    store.lyricFocusScaleRatio || 1.08,
  );

  // 仅歌词非当前行可启用景深模糊（关闭时全屏零模糊）
  const blurPx =
    !isCurrent && store.lyricBlurEnabled && hoveredLyricIndex.value !== idx
      ? Math.min(dist * 1.05 * store.lyricBlurIntensity, 4.5)
      : 0;

  const typo = isCurrent ? lyricTypographyActive.value : lyricTypographyInactive.value;

  return {
    opacity: hoveredLyricIndex.value === idx ? 0.96 : alpha,
    "--np-line-rest-y": `${rest.floatY.toFixed(2)}px`,
    "--np-line-rest-s": rest.scale.toFixed(4),
    transformOrigin:
      store.lyricAlignment === "center"
        ? "center center"
        : store.lyricAlignment === "right"
        ? "right center"
        : "left center",
    filter: blurPx > 0.15 ? `blur(${blurPx.toFixed(2)}px)` : "none",
    textAlign: store.lyricAlignment,
    fontFamily: typo.fontFamily,
    letterSpacing: `${Number(store.lyricLetterSpacing || 0).toFixed(1)}px`,
    paddingTop: `${Math.round(store.lyricLineSpacing * 0.5)}px`,
    paddingBottom: `${Math.round(store.lyricLineSpacing * 0.5)}px`,
  } as Record<string, string | number>;
}

function buildInterpolatedLineWords(
  line: LyricLine,
  nextLine?: LyricLine
): LyricWord[] {
  if (line.words && line.words.length > 0) return line.words;
  const rawText = line.text || "";
  if (!rawText.trim()) return [];

  const nextStart = nextLine?.startTimeMs;
  const gap =
    nextStart && nextStart > line.startTimeMs
      ? nextStart - line.startTimeMs
      : line.durationMs && line.durationMs > 350
      ? line.durationMs
      : 0;
  const naturalEstimate = Math.max(2400, rawText.trim().length * 300);

  let lineSpanMs: number;
  if (gap > 0) {
    lineSpanMs =
      gap > 6200
        ? Math.min(gap * 0.78, naturalEstimate)
        : Math.max(420, gap * 0.94);
  } else {
    lineSpanMs = naturalEstimate;
  }

  const tokenRegex =
    /([A-Za-z0-9''\-]+[\s,.!?;:'"’”]*|[\u4e00-\u9fff\u3040-\u30ff\uac00-\ud7af][^\S\r\n]*[，。！？、；：…,.!?;:’”」』）】]*|\S+\s*)/g;
  const tokens = rawText.match(tokenRegex) || [rawText];

  const weights = tokens.map((tok) => {
    const trimmed = tok.trim();
    if (!trimmed) return 0.18;
    const hasLatin = /[A-Za-z0-9]/.test(trimmed);
    if (hasLatin) {
      const alphaCount = trimmed.replace(/[^A-Za-z0-9]/g, "").length;
      return Math.max(1.0, alphaCount * 0.42);
    }
    return 1.0;
  });

  const totalWeight = Math.max(0.1, weights.reduce((acc, w) => acc + w, 0));
  let cursorMs = 0;
  return tokens.map((text, idx) => {
    const wDur = Math.max(40, (weights[idx] / totalWeight) * lineSpanMs);
    const item: LyricWord = {
      startTimeMs: line.startTimeMs + cursorMs,
      durationMs: wDur,
      text,
    };
    cursorMs += wDur;
    return item;
  });
}

const resolvedLyricWordsList = computed<LyricWord[][]>(() => {
  const lines = store.currentLyric?.lines || [];
  return lines.map((line, i) => buildInterpolatedLineWords(line, lines[i + 1]));
});

function getTranslationLineStyle(idx: number) {
  const isCurrent = idx === activeLineIndex.value;
  const isLight = store.isNowPlayingLightStage;
  const primary = lyricHighlightColors.value.primary;
  const secondary = lyricHighlightColors.value.secondary;
  const glowRatio = Math.max(0, Math.min(1.6, (store.lyricGlowIntensity ?? 100) / 100));
  const unifiedTint = lerpHex(primary, secondary, 0.3);
  const inkTarget = isLight ? store.effectiveCoverPalette.inkPrimary || "#18181B" : "#FFFFFF";
  const tintEnabled = store.lyricTranslationTintEnabled !== false;
  const activeColor = tintEnabled
    ? isLight
      ? hexWithAlpha(lerpHex(unifiedTint, inkTarget, 0.25), 0.9)
      : hexWithAlpha(lerpHex(unifiedTint, "#FFFFFF", 0.32), 0.88)
    : isLight
    ? hexWithAlpha(inkTarget, 0.78)
    : "rgba(255, 255, 255, 0.82)";
  const inactiveColor = !tintEnabled
    ? isLight
      ? hexWithAlpha(inkTarget, 0.56)
      : "rgba(255, 255, 255, 0.52)"
    : store.lyricTheme === "dynamic_cover"
    ? palette.value.lyricTranslationRgba
    : isLight
    ? hexWithAlpha(lerpHex(unifiedTint, inkTarget, 0.65), 0.68)
    : hexWithAlpha(lerpHex(unifiedTint, "#FFFFFF", 0.56), 0.58);
  const glowBase =
    store.lyricTheme === "dynamic_cover"
      ? parseRgbaToken(palette.value.lyricGlowRgba, primary, 0.24).hex
      : primary;
  return {
    fontSize: `${Math.round(store.lyricFontSize * store.lyricTranslationScale)}px`,
    fontWeight: isCurrent ? 580 : 500,
    color: isCurrent ? activeColor : inactiveColor,
    textShadow:
      isCurrent && !isLight && tintEnabled
        ? `0 2px ${Math.round(12 * glowRatio)}px ${hexWithAlpha(glowBase, 0.16 * glowRatio)}`
        : "none",
  };
}

const karaokeRenderTokens = computed(() => {
  const isLight = store.isNowPlayingLightStage;
  const isDynamicCover = store.lyricTheme === "dynamic_cover";
  const isSolid = store.lyricSungMode === "solid";
  const primary = lyricHighlightColors.value.primary;
  const secondary = isSolid ? primary : lyricHighlightColors.value.secondary;
  const pearlPrimary = isLight
    ? primary
    : lerpHex(primary, "#FFFFFF", isDynamicCover ? 0.08 : 0.12);
  const pearlSecondary = isSolid
    ? pearlPrimary
    : isLight
    ? secondary
    : lerpHex(secondary, "#FFFFFF", isDynamicCover ? 0.12 : 0.18);
  const crestColor = isSolid
    ? pearlPrimary
    : isLight
    ? lerpHex(pearlSecondary, pearlPrimary, 0.36)
    : lerpHex(pearlSecondary, "#FFFFFF", 0.38);

  const unsungHex = lyricUnsungBaseHex.value;
  const baseUnsungOp = Math.max(0.18, Math.min(0.85, store.lyricInactiveOpacity || 0.36));
  const unsungColor = hexWithAlpha(unsungHex, Math.min(0.92, baseUnsungOp + 0.16));
  const pastColor = hexWithAlpha(unsungHex, Math.min(0.86, baseUnsungOp + 0.34));
  const nextColor = hexWithAlpha(unsungHex, Math.min(0.96, baseUnsungOp + 0.52));
  const farColor = hexWithAlpha(unsungHex, Math.min(0.9, baseUnsungOp + 0.44));

  const glowRatio = Math.max(0, Math.min(1.6, (store.lyricGlowIntensity ?? 100) / 100));
  const glowTint = isDynamicCover
    ? parseRgbaToken(palette.value.lyricGlowRgba, primary, 0.26)
    : { hex: primary, alpha: isLight ? 0.14 : 0.24 };

  const activeTypo = lyricTypographyActive.value;
  const inactiveTypo = lyricTypographyInactive.value;
  const inactiveStroke = inactiveTypo.isHeavyOptical
    ? `0.55px ${hexWithAlpha(unsungHex, 0.48)}`
    : "0px transparent";
  const heavyStrokeSung = activeTypo.isHeavyOptical
    ? `0.55px ${hexWithAlpha(pearlPrimary, 0.56)}`
    : "0px transparent";
  const heavyStrokeUnsung = activeTypo.isHeavyOptical
    ? `0.55px ${hexWithAlpha(unsungHex, 0.56)}`
    : "0px transparent";

  const glowR1 = Math.max(6, Math.round(14 * Math.max(0.45, glowRatio)));
  const glowR2 = Math.max(12, Math.round(28 * Math.max(0.45, glowRatio)));
  const glowAlpha1 = isLight
    ? Math.min(0.24, glowTint.alpha * 0.88 * glowRatio)
    : Math.min(0.5, glowTint.alpha * 1.38 * glowRatio);
  const glowAlpha2 = isLight
    ? Math.min(0.14, glowTint.alpha * 0.48 * glowRatio)
    : Math.min(0.28, glowTint.alpha * 0.78 * glowRatio);
  const glowColor1 = hexWithAlpha(glowTint.hex, glowAlpha1);
  const glowColor2 = hexWithAlpha(lerpHex(glowTint.hex, secondary, 0.45), glowAlpha2);

  const litBg = `linear-gradient(108deg, ${pearlPrimary} 0%, ${pearlSecondary} 100%)`;
  const midDotColor = lerpOklabHex(primary, secondary, 0.5);

  const stageCssVars: Record<string, string> = {
    "--np-kw-lit-bg": litBg,
    "--np-kw-unsung-color": unsungColor,
    "--np-kw-past-color": pastColor,
    "--np-kw-next-color": nextColor,
    "--np-kw-far-color": farColor,
    "--np-kw-stroke-inactive": inactiveStroke,
    "--np-kw-stroke-sung": heavyStrokeSung,
    "--np-kw-stroke-unsung": heavyStrokeUnsung,
    "--np-kw-glow-r1": `${glowR1}px`,
    "--np-kw-glow-r2": `${glowR2}px`,
    "--np-kw-glow-c1": glowColor1,
    "--np-kw-glow-c2": glowColor2,
  };

  return {
    primary,
    secondary,
    midDotColor,
    dimDotColor: palette.value.lyricInactiveRgba || "rgba(255, 255, 255, 0.28)",
    pearlPrimary,
    pearlSecondary,
    crestColor,
    unsungColor,
    glowRatio,
    glowTint,
    bounceOn: store.lyricWordByWord && store.lyricWordBounceEnabled !== false,
    wordByWord: Boolean(store.lyricWordByWord),
    stageCssVars,
  };
});

function getResolvedLineEndMs(idx: number): number {
  const lines = store.currentLyric?.lines || [];
  const curr = lines[idx];
  if (!curr) return 0;
  if (curr.words && curr.words.length > 0) {
    return curr.startTimeMs + Math.max(1200, curr.durationMs || 2500);
  }
  const interpWords = resolvedLyricWordsList.value[idx];
  if (interpWords && interpWords.length > 0) {
    const last = interpWords[interpWords.length - 1];
    return Math.max(curr.startTimeMs + 1200, last.startTimeMs + last.durationMs);
  }
  return curr.startTimeMs + Math.max(1200, curr.durationMs || 2500);
}

function isInInterludeGapAt(idx: number, posMs: number): boolean {
  if (store.lyricInterludeDotsEnabled === false || idx < 0) return false;
  const lines = store.currentLyric?.lines || [];
  if (idx + 1 >= lines.length) return false;
  const next = lines[idx + 1];
  const currEnd = getResolvedLineEndMs(idx);
  const gapMs = next.startTimeMs - currEnd;
  return gapMs >= 4500 && posMs >= currEnd && posMs < next.startTimeMs;
}

function shouldShowInterludeDots(idx: number): boolean {
  return idx === activeLineIndex.value && activeInterludeVisible.value;
}

function getInterludeDotStyle(idx: number, dotIndex: number) {
  const lines = store.currentLyric?.lines || [];
  const curr = lines[idx];
  const next = lines[idx + 1];
  if (!curr || !next) return {};
  const currEnd = getResolvedLineEndMs(idx);
  const gapMs = Math.max(1, next.startTimeMs - currEnd);
  const totalProg = Math.max(
    0,
    Math.min(1, (getEffectiveLyricTimeMs() - currEnd) / gapMs)
  );
  const segStart = dotIndex / 3;
  const segEnd = (dotIndex + 1) / 3;
  const dotProg = Math.max(
    0,
    Math.min(1, (totalProg - segStart) / (segEnd - segStart))
  );
  const tok = karaokeRenderTokens.value;
  const dotColor =
    dotIndex === 0 ? tok.primary : dotIndex === 1 ? tok.midDotColor : tok.secondary;
  const wave = Math.sin(breathingDotPhaseVal - dotIndex * 0.75);
  const ty = -2.2 * Math.max(0, wave) * (0.35 + 0.65 * dotProg);
  const s = 1 + 0.14 * Math.max(0, wave) * dotProg;

  return {
    color: dotProg > 0.35 ? dotColor : tok.dimDotColor,
    transform: `translateY(${ty.toFixed(2)}px) scale(${s.toFixed(3)})`,
    opacity: 0.45 + 0.55 * dotProg,
  };
}

/**
 * 120Hz 直接 GPU 合成歌词驱动器：
 * 1. 二分查找行号，换行时无缝移交 380ms 退场余韵池并触发行级瀑布错峰波浪；
 * 2. 以独立二阶弹簧更新活跃行逐字浮升/微缩/广域浸润/长音呼吸微光，仅写入变化 CSS 变量，零每帧重绘；
 * 3. 返回逐字与退场池是否完全收敛，供暂停静止零功耗停帧判断。
 */
function syncLyricRealtimeDom(
  posMs: number,
  dtSec = 0.016,
  forceSnap = false,
): boolean {
  const lines = store.currentLyric?.lines;
  if (!lines || lines.length === 0) {
    if (activeLineIndex.value !== -1) activeLineIndex.value = -1;
    if (activeInterludeVisible.value) activeInterludeVisible.value = false;
    clearLeavingLineState();
    return true;
  }

  const prevIdx = activeLineIndex.value;
  const nextIdx = findLyricLineIndexAt(posMs);
  if (nextIdx !== prevIdx) {
    transitionActiveLyricLine(
      prevIdx,
      nextIdx,
      posMs,
      forceSnap || isScrubbingProgress.value || Math.abs(nextIdx - prevIdx) > 2,
    );
  }

  const inInterlude = isInInterludeGapAt(nextIdx, posMs);
  if (inInterlude !== activeInterludeVisible.value) {
    activeInterludeVisible.value = inInterlude;
    cachedInterludeDotEls = [];
  }

  let activeSettled = true;
  if (nextIdx >= 0) {
    ensureActiveLyricDomCache(nextIdx);
    const words = resolvedLyricWordsList.value[nextIdx];
    const line = lines[nextIdx];
    if (
      line &&
      words &&
      words.length > 0 &&
      cachedActiveWordEls.length === words.length
    ) {
      const wordOpts = getWordPhysicsOpts();
      if (forceSnap || isScrubbingProgress.value) {
        activeWordBank.snapToTime(line.startTimeMs, words, posMs, wordOpts);
        flushWordBankToDom(activeWordBank, cachedActiveWordEls, true);
      } else {
        activeSettled = activeWordBank.stepActive(
          line.startTimeMs,
          words,
          posMs,
          dtSec,
          wordOpts,
        );
        flushWordBankToDom(activeWordBank, cachedActiveWordEls, false);
      }
    }
  }

  // 推进 380ms 退场余韵行：平滑回落并柔和消融辉光
  let leavingSettled = true;
  if (leavingLineIndex.value >= 0 && cachedLeavingWordEls.length > 0) {
    const motion = getMotionRuntimeProfile();
    const elapsedLeaveMs = performance.now() - leavingStartedWallMs;
    leavingSettled =
      leavingWordBank.stepLeaving(dtSec, motion.effectiveLevel) ||
      elapsedLeaveMs >= 460;
    flushWordBankToDom(leavingWordBank, cachedLeavingWordEls, false);
    if (leavingSettled) {
      clearLeavingLineState();
    }
  }

  const tok = karaokeRenderTokens.value;
  if (inInterlude) {
    if (cachedInterludeDotEls.length !== 3 || !cachedInterludeDotEls[0]?.isConnected) {
      const lineEl = getCachedLineEl(nextIdx);
      cachedInterludeDotEls = lineEl
        ? Array.from(lineEl.querySelectorAll<HTMLElement>(".np-interlude-dot"))
        : [];
    }
    if (cachedInterludeDotEls.length === 3 && nextIdx + 1 < lines.length) {
      const currEnd = getResolvedLineEndMs(nextIdx);
      const gapMs = Math.max(1, lines[nextIdx + 1].startTimeMs - currEnd);
      const totalProg = Math.max(0, Math.min(1, (posMs - currEnd) / gapMs));
      const dotColors = [tok.primary, tok.midDotColor, tok.secondary];
      for (let d = 0; d < 3; d++) {
        const dotEl = cachedInterludeDotEls[d];
        if (!dotEl) continue;
        const dotProg = Math.max(0, Math.min(1, (totalProg - d / 3) * 3));
        const wave = Math.sin(breathingDotPhaseVal - d * 0.75);
        const ty = -2.2 * Math.max(0, wave) * (0.35 + 0.65 * dotProg);
        const s = 1 + 0.14 * Math.max(0, wave) * dotProg;
        dotEl.style.color = dotProg > 0.35 ? dotColors[d] : tok.dimDotColor;
        dotEl.style.transform = `translateY(${ty.toFixed(2)}px) scale(${s.toFixed(3)})`;
        dotEl.style.opacity = (0.45 + 0.55 * dotProg).toFixed(3);
      }
    }
  }

  return activeSettled && leavingSettled;
}

function stepAndApplyLineCascadeDom(dtSec: number, subpixelResidualPx: number): boolean {
  const lines = store.currentLyric?.lines;
  if (!lines || lines.length === 0) {
    clearCascadeInlineTransforms();
    return true;
  }
  const cascadeOpts = getLineCascadeOpts();
  const { settled, indices } = lineCascadeBank.stepWindow(
    activeLineIndex.value,
    lines.length,
    dtSec,
    isUserScrollingLyrics.value,
    cascadeOpts,
  );

  const nextWindowSet = new Set<number>(indices);
  for (const oldIdx of prevCascadeWindowIndices) {
    if (!nextWindowSet.has(oldIdx)) {
      const oldEl = cachedLineEls[oldIdx];
      if (oldEl && oldEl.isConnected) {
        oldEl.style.removeProperty("transform");
      }
    }
  }
  prevCascadeWindowIndices = nextWindowSet;

  for (let i = 0; i < indices.length; i++) {
    const idx = indices[i];
    const node = lineCascadeBank.getNode(idx);
    const el = getCachedLineEl(idx);
    if (!node || !el) continue;
    const totalY = node.floatY + node.cascadeY + subpixelResidualPx;
    const transformStr = `translate3d(0, ${totalY.toFixed(2)}px, 0) scale(${node.scale.toFixed(4)})`;
    if (transformStr !== node.lastTransform) {
      node.lastTransform = transformStr;
      el.style.transform = transformStr;
    }
  }
  return settled;
}

// ================= 8. 主循环：驱动高刷时间采样、弹簧歌词滚动与光学背景 =================
let mainLoopRafId: number | null = null;
let loopPauseTimer: number | null = null;
let prevLoopWallMs = performance.now();
let lastAudioSampleMs = -1;
let audioAnchorPosMs = 0;
let audioAnchorWallMs = performance.now();
let unsubscribePowerVis: (() => void) | null = null;

function startNowPlayingMasterLoop() {
  if (!isAppForegroundActive()) return;
  if (loopPauseTimer !== null) {
    window.clearTimeout(loopPauseTimer);
    loopPauseTimer = null;
  }
  ensureBgEngineAttached();
  prevLoopWallMs = performance.now();
  const initialPos = store.getLivePlaybackPositionMs();
  lastAudioSampleMs = initialPos;
  audioAnchorPosMs = initialPos;
  audioAnchorWallMs = prevLoopWallMs;
  liveSampledPosMs = initialPos;
  liveInterpolatedPosMs = initialPos;
  syncDisplayElapsedSec(initialPos);
  syncTimelineProgressDom(computeProgressFrac(initialPos), true);
  syncLyricRealtimeDom(getEffectiveLyricTimeMs(), 0.016, true);
  stepAndApplyLineCascadeDom(0.016, 0);
  drawDynamicBackground(prevLoopWallMs, 0.016);
  if (mainLoopRafId === null) {
    mainLoopRafId = requestAnimationFrame(tickNowPlayingMasterLoop);
  }
}

function stopNowPlayingMasterLoop() {
  if (loopPauseTimer !== null) {
    window.clearTimeout(loopPauseTimer);
    loopPauseTimer = null;
  }
  if (mainLoopRafId !== null) {
    cancelAnimationFrame(mainLoopRafId);
    mainLoopRafId = null;
  }
}

function tickNowPlayingMasterLoop(now: number) {
  if (!isAppForegroundActive() || !store.isNowPlayingOpen) {
    mainLoopRafId = null;
    return;
  }
  const dtSec = Math.min(0.064, Math.max(0.001, (now - prevLoopWallMs) / 1000));
  prevLoopWallMs = now;

  let posSettled = true;
  if (!isScrubbingProgress.value) {
    const rawPos = store.getLivePlaybackPositionMs();
    const effRate = store.getEffectivePlaybackRate();

    if (
      settledSeekTargetMs.value >= 0 &&
      now - settledSeekWallMs.value <= 520
    ) {
      if (Math.abs(rawPos - settledSeekTargetMs.value) <= 380) {
        settledSeekTargetMs.value = -1;
        lastAudioSampleMs = rawPos;
        audioAnchorPosMs = rawPos;
        audioAnchorWallMs = now;
        liveSampledPosMs = rawPos;
      } else {
        liveSampledPosMs = settledSeekTargetMs.value;
      }
    } else {
      settledSeekTargetMs.value = -1;
      liveSampledPosMs = rawPos;
      if (Math.abs(rawPos - lastAudioSampleMs) > 0.4 || rawPos < lastAudioSampleMs - 20) {
        lastAudioSampleMs = rawPos;
        audioAnchorPosMs = rawPos;
        audioAnchorWallMs = now;
      }
    }

    // 60fps/120fps 高刷硬件级采样 + 连续时间差锁相环 (PLL)：保持 KRC/LRC 逐字流光顺滑度
    const targetPos = liveSampledPosMs;
    if (store.isPlaying && store.durationMs > 0) {
      posSettled = false;
      const elapsedSinceSample =
        settledSeekTargetMs.value >= 0
          ? 0
          : Math.min(350, Math.max(0, now - audioAnchorWallMs));
      const baseAnchor =
        settledSeekTargetMs.value >= 0 ? targetPos : audioAnchorPosMs;
      const continuousTargetPos = Math.min(
        store.durationMs,
        baseAnchor + elapsedSinceSample * effRate
      );
      const rawAdvance = dtSec * 1000 * effRate;
      const nextUnclamped = liveInterpolatedPosMs + rawAdvance;
      const drift = continuousTargetPos - nextUnclamped;

      if (Math.abs(drift) > 220) {
        liveInterpolatedPosMs = continuousTargetPos;
      } else {
        const correctedStep = rawAdvance + drift * Math.min(1, dtSec * 8.5);
        const monotonicStep = Math.max(
          0,
          Math.min(rawAdvance * 1.25, correctedStep)
        );
        liveInterpolatedPosMs = Math.min(
          store.durationMs,
          liveInterpolatedPosMs + monotonicStep
        );
      }
    } else if (Math.abs(targetPos - liveInterpolatedPosMs) >= 0.5) {
      posSettled = false;
      liveInterpolatedPosMs +=
        (targetPos - liveInterpolatedPosMs) * Math.min(1, dtSec * 16);
    } else {
      liveInterpolatedPosMs = targetPos;
    }

    syncDisplayElapsedSec(liveSampledPosMs);
    syncTimelineProgressDom(computeProgressFrac());
  } else {
    posSettled = false;
  }

  // 直接 GPU 合成驱动活跃行逐字弹簧、380ms 退场余韵与间奏呼吸点
  const effLyricMs = getEffectiveLyricTimeMs();
  if (store.isPlaying && activeInterludeVisible.value) {
    breathingDotPhaseVal =
      (breathingDotPhaseVal + dtSec * ((2 * Math.PI) / 1.4)) %
      (Math.PI * 2);
  }
  const wordsSettled = syncLyricRealtimeDom(effLyricMs, dtSec, false);

  // 120Hz 子步进临界阻尼弹簧驱动歌词视口平滑滚动 + 子像素残差无抖补偿
  const motion = getMotionRuntimeProfile();
  let scrollSettled = true;
  let subpixelResidual = 0;
  const viewport = lyricViewportRef.value;
  if (viewport && !isUserScrollingLyrics.value) {
    if (!lyricScrollInitialized) {
      scrollToActiveLyricLine(false);
    } else if (motion.effectiveLevel === "none") {
      if (Math.abs(lyricTargetScrollTop - lyricCurrentScrollTop) > 0.5) {
        lyricCurrentScrollTop = lyricTargetScrollTop;
        lyricScrollVelocity = 0;
        viewport.scrollTop = Math.round(lyricTargetScrollTop);
      }
    } else {
      const disp = lyricTargetScrollTop - lyricCurrentScrollTop;
      if (Math.abs(disp) > 0.12 || Math.abs(lyricScrollVelocity) > 0.35) {
        scrollSettled = false;
        const [nextScroll, nextVel] = stepSpringScalar(
          lyricCurrentScrollTop,
          lyricScrollVelocity,
          lyricTargetScrollTop,
          motion.effectiveLevel === "standard" ? 136 : 114,
          motion.effectiveLevel === "standard" ? 23.4 : 21.4,
          dtSec,
        );
        lyricCurrentScrollTop = nextScroll;
        lyricScrollVelocity = nextVel;
        if (Math.abs(lyricTargetScrollTop - lyricCurrentScrollTop) < 0.18 && Math.abs(lyricScrollVelocity) < 0.35) {
          lyricCurrentScrollTop = Math.round(lyricTargetScrollTop);
          lyricScrollVelocity = 0;
          viewport.scrollTop = lyricCurrentScrollTop;
          subpixelResidual = 0;
          scrollSettled = true;
        } else {
          const roundedScroll = Math.round(lyricCurrentScrollTop);
          viewport.scrollTop = roundedScroll;
          // 将 scrollTop 取整产生的 ±0.5px 误差吸收到行级 GPU translate3d，彻底消除慢速收尾 1px 阶梯抖动
          subpixelResidual = -(lyricCurrentScrollTop - roundedScroll);
        }
      }
    }
  }

  // 推进 AMLL 式行级弹性瀑布错峰与景深缩放
  const cascadeSettled = stepAndApplyLineCascadeDom(dtSec, subpixelResidual);

  drawDynamicBackground(now, dtSec);

  // 无动效模式且暂停、歌词弹簧与进度收敛时自动停帧，做到 0% 静止 CPU/GPU 占用
  if (
    !store.isPlaying &&
    posSettled &&
    scrollSettled &&
    wordsSettled &&
    cascadeSettled &&
    motion.effectiveLevel === "none"
  ) {
    mainLoopRafId = null;
    return;
  }

  mainLoopRafId = requestAnimationFrame(tickNowPlayingMasterLoop);
}

const dynamicCssVars = computed(() => {
  const isLight = store.isNowPlayingLightStage;
  const borderlessVars = buildBorderlessCssVars(
    store.effectiveCoverPalette,
    !isLight
  );
  const pal = palette.value;
  const activeLyricColors = lyricHighlightColors.value;
  const activeLyricGlow =
    store.lyricTheme === "dynamic_cover"
      ? pal.lyricGlowRgba
      : hexWithAlpha(activeLyricColors.primary, isLight ? 0.18 : 0.42);
  const accentPrimary = readableAccentPrimary.value;
  const accentSecondary = readableAccentSecondary.value;
  const accentMidpoint = lerpOklabHex(accentPrimary, accentSecondary, 0.5);
  const ink1 = isLight ? store.effectiveCoverPalette.inkPrimary || "#18181B" : "#FFFFFF";
  const ink2 = isLight
    ? store.effectiveCoverPalette.inkSecondary || "rgba(24, 24, 27, 0.72)"
    : "rgba(255, 255, 255, 0.68)";
  const ink3 = isLight
    ? store.effectiveCoverPalette.inkMuted || "rgba(24, 24, 27, 0.46)"
    : "rgba(255, 255, 255, 0.44)";
  return {
    ...borderlessVars,
    colorScheme: isLight ? "light" : "dark",
    "--np-ink-1": ink1,
    "--np-ink-2": ink2,
    "--np-ink-3": ink3,
    "--np-track-bg": isLight ? "rgba(24, 24, 27, 0.13)" : "rgba(255, 255, 255, 0.13)",
    "--np-mesh-primary": pal.meshPrimary,
    "--np-mesh-secondary": pal.meshSecondary,
    "--np-mesh-tertiary": pal.meshTertiary,
    "--np-accent-primary": accentPrimary,
    "--np-accent-secondary": accentSecondary,
    "--np-accent-tertiary": readableAccentTertiary.value,
    "--np-accent-quaternary": readableAccentQuaternary.value,
    "--np-accent-primary-raw": pal.rawDominantAccent,
    "--np-accent-secondary-raw": pal.rawSecondaryAccent,
    "--np-on-accent-primary": pal.onDominantAccent,
    "--np-on-accent-secondary": pal.onSecondaryAccent,
    "--np-accent-midpoint": accentMidpoint,
    "--np-dual-gradient": `linear-gradient(135deg, ${accentPrimary} 0%, ${accentSecondary} 100%)`,
    "--np-accent-glow": pal.accentGlowRgba,
    "--np-secondary-glow": pal.accentSecondaryGlowRgba,
    "--np-secondary-subtle": pal.accentSecondarySubtleRgba,
    "--np-surface-tint": isLight
      ? hexWithAlpha(pal.bgSecondary || "#F2ECE1", 0.86)
      : hexWithAlpha(pal.bgPrimary || pal.meshPrimary || "#141D2B", 0.48),
    "--np-card-tint": pal.cardTintRgba,
    "--np-card-hover": pal.cardHoverRgba,
    "--np-control-active": pal.controlActiveHex,
    "--np-control-track": pal.controlTrackRgba,
    "--np-lyric-primary": activeLyricColors.primary,
    "--np-lyric-secondary": activeLyricColors.secondary,
    "--np-lyric-glow": activeLyricGlow,
    "--np-lyric-inactive": pal.lyricInactiveRgba,
    "--np-lyric-translation": pal.lyricTranslationRgba,
  };
});

watch(
  () => store.isNowPlayingOpen,
  (open) => {
    if (open) {
      wakeUpTopNav();
      startNowPlayingMasterLoop();
      nextTick(() => {
        lyricScrollInitialized = false;
        refreshCachedLineElements();
        scrollToActiveLyricLine(false);
        syncLyricRealtimeDom(getEffectiveLyricTimeMs(), 0.016, true);
        stepAndApplyLineCascadeDom(0.016, 0);
        measureMetaOverflow();
      });
    } else {
      showBgStyleMenu.value = false;
      showLyricSettings.value = false;
      showMoreSheet.value = false;
      showQualityDialog.value = false;
      showAudioEffectDialog.value = false;
      store.isNowPlayingQueueOpen = false;
      clearTopNavIdleTimer();
      if (loopPauseTimer !== null) {
        window.clearTimeout(loopPauseTimer);
      }
      loopPauseTimer = window.setTimeout(() => {
        loopPauseTimer = null;
        if (!store.isNowPlayingOpen) {
          stopNowPlayingMasterLoop();
        }
      }, 812);
    }
  }
);

watch(
  normalizedCoverUrl,
  (url) => {
    if (bgEngineAttached) {
      void bgEngine.setCoverUrl(url);
    }
  }
);

watch(
  [() => store.playerStageStyle, () => store.isNowPlayingOpen],
  () => {
    nextTick(() => measureCoverFocusPoint());
  }
);

watch([titleViewportRef, qualityViewportRef], ([tEl, qEl]) => {
  if (!metaResizeObserver) return;
  metaResizeObserver.disconnect();
  if (tEl) metaResizeObserver.observe(tEl);
  if (qEl) metaResizeObserver.observe(qEl);
  if (backdropRef.value) metaResizeObserver.observe(backdropRef.value);
  nextTick(() => {
    measureMetaOverflow();
    measureCoverFocusPoint();
  });
});

watch(
  [
    () => store.isPlaying,
    () => store.playerBgStyle,
    () => store.motionLevel,
    () => store.isNowPlayingLightStage,
    () => store.coverColorStageMode,
    () => store.effectiveCoverPalette.dominantAccent,
    () => store.effectiveCoverPalette.secondaryAccent,
    () => store.lyricBackgroundDim,
  ],
  () => {
    if (store.isNowPlayingOpen && isAppForegroundActive() && mainLoopRafId === null) {
      startNowPlayingMasterLoop();
    }
  }
);

onMounted(() => {
  prevLoopWallMs = performance.now();
  const initPos = store.getLivePlaybackPositionMs();
  liveSampledPosMs = initPos;
  liveInterpolatedPosMs = initPos;
  activeLineIndex.value = findLyricLineIndexAt(getEffectiveLyricTimeMs());
  syncDisplayElapsedSec(initPos);
  syncTimelineProgressDom(computeProgressFrac(initPos), true);
  measureCoverFocusPoint();
  unsubscribePowerVis = subscribeAppPowerVisibility((active) => {
    if (!active) {
      stopNowPlayingMasterLoop();
    } else if (store.isNowPlayingOpen) {
      startNowPlayingMasterLoop();
    }
  });
  if (typeof ResizeObserver !== "undefined") {
    metaResizeObserver = new ResizeObserver(() => {
      measureMetaOverflow();
      measureCoverFocusPoint();
    });
    if (titleViewportRef.value) metaResizeObserver.observe(titleViewportRef.value);
    if (qualityViewportRef.value) metaResizeObserver.observe(qualityViewportRef.value);
    if (backdropRef.value) metaResizeObserver.observe(backdropRef.value);
  }
  nextTick(() => {
    measureMetaOverflow();
    measureCoverFocusPoint();
    refreshCachedLineElements();
    syncLyricRealtimeDom(getEffectiveLyricTimeMs(), 0.016, true);
    stepAndApplyLineCascadeDom(0.016, 0);
  });
  if (store.isNowPlayingOpen) {
    ensureBgEngineAttached();
    scheduleTopNavAutoHide();
    startNowPlayingMasterLoop();
    nextTick(() => scrollToActiveLyricLine(false));
  }
});

onBeforeUnmount(() => {
  if (unsubscribePowerVis) {
    unsubscribePowerVis();
    unsubscribePowerVis = null;
  }
  invalidateActiveLyricDomCache();
  stopNowPlayingMasterLoop();
  bgEngineAttached = false;
  bgEngine.dispose();
  if (lyricScrubResetTimer) window.clearTimeout(lyricScrubResetTimer);
  if (metaResizeObserver) {
    metaResizeObserver.disconnect();
    metaResizeObserver = null;
  }
  clearTopNavIdleTimer();
});
</script>

<template>
  <div
    class="np-root"
    :class="[
      store.isNowPlayingLightStage
        ? 'np-stage-light is-light-stage'
        : 'np-stage-dark is-dark-surface',
      !store.dynamicCoverAccentEnabled &&
      store.isNowPlayingLightStage &&
      store.lightThemeVariant === 'nordic_oat'
        ? 'is-oat-stage'
        : '',
      store.dynamicCoverAccentEnabled && store.isNowPlayingLightStage
        ? 'is-dynamic-light-stage'
        : '',
    ]"
    :style="dynamicCssVars"
    @mousemove="handleRootMouseMove"
    @click="showBgStyleMenu = false"
  >
    <!-- =========================================================================
         1. 全屏博物馆级环境背景层（统一 WebGL2 + Canvas 2D 双管线 · 4 大高定光学预设）
         ========================================================================= -->
    <div
      ref="backdropRef"
      class="np-ambient-backdrop"
      :class="`np-bg-mode-${store.playerBgStyle}`"
      :data-bg-style="store.playerBgStyle"
    >
      <canvas
        ref="bgGlCanvasRef"
        class="np-bg-canvas"
        :class="`np-bg-canvas-${store.playerBgStyle}`"
      ></canvas>
      <canvas
        ref="bg2dCanvasRef"
        class="np-bg-canvas"
        :class="`np-bg-canvas-${store.playerBgStyle}`"
      ></canvas>
    </div>

    <!-- =========================================================================
         2. 左上角极简下箭头返回按钮 + 顶部居中舞台胶囊栏 (10秒自动隐藏 · 鼠标移至顶部自然浮现)
         ========================================================================= -->
    <button
      type="button"
      class="np-top-left-back-btn"
      aria-label="收起播放页"
      @click.stop="store.isNowPlayingOpen = false"
    >
      <PlumpIcon name="np-chevron-down" :size="22" />
    </button>

    <header
      class="np-skyline-bar"
      :class="{
        'is-auto-hidden':
          isTopNavAutoHidden && !showBgStyleMenu && !isHoveringTopNav,
      }"
      @mouseenter="handleTopNavMouseEnter"
      @mouseleave="handleTopNavMouseLeave"
      @click.stop="wakeUpTopNav"
    >
      <div class="np-center-mode-capsule">
        <!-- 舞台形态切换：画廊 / 黑胶 / 歌词 -->
        <div class="np-stage-mode-group">
          <button
            v-for="mode in stageModes"
            :key="mode.id"
            type="button"
            class="np-stage-mode-pill"
            :class="{ active: store.playerStageStyle === mode.id }"
            @click="
              store.playerStageStyle = mode.id;
              wakeUpTopNav();
            "
          >
            {{ mode.label }}
          </button>
        </div>

        <!-- 光学背景氛围选择器（无框色阶分层，移除分割线） -->
        <div class="np-ambience-dropdown-wrap">
          <button
            type="button"
            class="np-ambience-trigger"
            :class="{ active: showBgStyleMenu }"
            @click="
              showBgStyleMenu = !showBgStyleMenu;
              wakeUpTopNav();
            "
          >
            <span class="np-ambience-twin-orbs" aria-hidden="true">
              <span class="np-orb-primary"></span>
              <span class="np-orb-secondary"></span>
            </span>
            <span>{{ currentBgLabel }}</span>
          </button>

          <transition name="np-popover-fade">
            <div v-if="showBgStyleMenu" class="np-ambience-popover">
              <button
                v-for="item in bgQuickList"
                :key="item.id"
                type="button"
                class="np-ambience-option"
                :class="{ active: store.playerBgStyle === item.id }"
                @click="
                  store.playerBgStyle = item.id;
                  showBgStyleMenu = false;
                  wakeUpTopNav();
                "
              >
                <div class="np-ambience-opt-text">
                  <span class="np-ambience-opt-label">{{ item.label }}</span>
                  <span class="np-ambience-opt-sub">{{ item.subtitle }}</span>
                </div>
                <span
                  v-if="store.playerBgStyle === item.id"
                  class="np-ambience-opt-check"
                >
                  ✓
                </span>
              </button>
            </div>
          </transition>
        </div>

        <!-- 智能封面取色三态循环开关（关闭 · 主题配色 → 封面 · 随明暗 → 封面 · 锁暗室） -->
        <button
          type="button"
          class="np-color-toggle-pill"
          :class="{
            active: store.coverColorStageMode !== 'off',
            'is-dark-lock': store.coverColorStageMode === 'dark_lock',
          }"
          :aria-pressed="store.coverColorStageMode !== 'off'"
          :title="coverColorModeTooltip"
          @click="
            store.cycleCoverColorStageMode();
            wakeUpTopNav();
          "
        >
          <span class="np-color-toggle-dot" aria-hidden="true"></span>
          <span>{{ coverColorModeLabel }}</span>
        </button>
      </div>
    </header>

    <!-- =========================================================================
         3. 主舞台双栏工作区 (全宽舒展 50%/50% 对称架构，左栏居中于左半屏)
         ========================================================================= -->
    <div
      class="np-main-workspace"
      :class="{
        'is-pure-lyric': store.playerStageStyle === 'pure_lyric',
      }"
    >
      <!-- ==================== 左栏：放大对称封面/黑胶舞台 + 曲目信息头 + 一体化悬浮无框播控台 ==================== -->
      <section
        v-if="store.playerStageStyle !== 'pure_lyric'"
        class="np-left-stage"
      >
        <!-- 上部：统一尺寸居中封面舞台 + 右下角竖向无框三联捷径 -->
        <div class="np-cover-hero-box">
          <div class="np-cover-stage-bundle">
            <NowPlayingCoverStage />

            <div class="np-cover-side-actions" aria-label="曲目快捷操作">
              <button
                type="button"
                class="np-side-action-btn"
                :class="{ liked: isLiked, 'burst-pop': likeBtnBurst }"
                data-tip="喜欢 / 收藏"
                @click.stop="handleToggleFavorite"
              >
                <PlumpIcon
                  :name="isLiked ? 'np-heart-filled' : 'np-heart'"
                  :size="20"
                  :color="isLiked ? '#FF4D6D' : 'currentColor'"
                />
              </button>

              <button
                type="button"
                class="np-side-action-btn"
                data-tip="添加到我的歌单"
                @click.stop="
                  store.currentSong &&
                    (store.addToPlaylistModalSong = store.currentSong)
                "
              >
                <PlumpIcon
                  name="playlist-add"
                  :size="20"
                  color="currentColor"
                  accent="var(--np-accent-secondary, #38D2C0)"
                />
              </button>

              <button
                type="button"
                class="np-side-action-btn"
                data-tip="生成歌词海报"
                @click.stop="store.isLyricPosterOpen = true"
              >
                <PlumpIcon
                  name="poster"
                  :size="20"
                  color="currentColor"
                  accent="var(--np-accent-secondary, #38D2C0)"
                />
              </button>
            </div>
          </div>
        </div>

        <!-- 下部：曲目元信息 + 无卡片全沉浸双行播控台 -->
        <div class="np-console-stack">
          <!-- (1) 曲目分层信息（纯净无右侧按钮拥挤） -->
          <div class="np-meta-header">
            <div class="np-meta-texts">
              <div
                ref="titleViewportRef"
                class="np-title-marquee-viewport"
                :class="{ 'is-overflowing': isTitleOverflowing }"
                :style="{
                  '--marquee-dist': `${titleMarqueeDistPx}px`,
                  '--marquee-dur': `${titleMarqueeDurationSec}s`,
                }"
              >
                <h1 ref="titleTrackRef" class="np-song-title-text">
                  {{ store.currentSong?.title || "微风音乐 · 概念声学" }}
                </h1>
              </div>

              <div class="np-artists-flow-row">
                <template v-if="songArtistsList.length > 0">
                  <template
                    v-for="(author, aIdx) in songArtistsList"
                    :key="`${author.id}_${author.name}_${aIdx}`"
                  >
                    <button
                      type="button"
                      class="np-meta-link artist np-artist-chip-btn"
                      @click="
                        store.currentSong &&
                          store.navigateToSongArtist(store.currentSong, author)
                      "
                    >
                      {{ author.name }}
                    </button>
                    <span
                      v-if="aIdx < songArtistsList.length - 1"
                      class="np-artist-sep"
                    >
                      /
                    </span>
                  </template>
                </template>
                <span v-else class="np-meta-link artist">未知艺术家</span>
              </div>

              <div class="np-quality-row">
                <button
                  type="button"
                  class="np-quality-spec-pill"
                  :class="{
                    'is-gold': isGoldQuality,
                    'is-overflowing': isQualityOverflowing,
                  }"
                  :style="{
                    '--marquee-dist': `${qualityMarqueeDistPx}px`,
                    '--marquee-dur': `${qualityMarqueeDurationSec}s`,
                  }"
                  @click.stop="showQualityDialog = true"
                >
                  <span class="np-quality-dot"></span>
                  <span ref="qualityViewportRef" class="np-quality-marquee-viewport">
                    <span ref="qualityTrackRef" class="np-quality-marquee-track">
                      {{ compactAudioSpecLabel }}
                    </span>
                  </span>
                </button>
              </div>
            </div>
          </div>

          <!-- (2) 一体化无框悬浮播控台 (彗星时间轴 + 双色主传输中枢 + 4 键双色功能坞) -->
          <div class="np-studio-console-deck">
            <div class="np-timeline-box">
              <div
                ref="timelineTrackRef"
                class="np-timeline-track-hit"
                :class="{ 'is-scrubbing': isScrubbingProgress }"
                @mousedown="handleProgressPointerDown"
                @mousemove="handleTimelineMouseMove"
                @mouseleave="handleTimelineMouseLeave"
              >
                <!-- 拖拽或悬停时间气泡 -->
                <transition name="fade">
                  <div
                    v-if="
                      (isScrubbingProgress || hoverTimelineFraction !== null) &&
                      store.durationMs > 0
                    "
                    class="np-timeline-scrub-pill"
                    :style="{
                      left: `${
                        (isScrubbingProgress
                          ? scrubFraction
                          : hoverTimelineFraction || 0) * 100
                      }%`,
                    }"
                  >
                    {{
                      formatDuration(
                        ((isScrubbingProgress
                          ? scrubFraction
                          : hoverTimelineFraction || 0) *
                          store.durationMs) /
                          1000
                      )
                    }}
                  </div>
                </transition>

                <div class="np-timeline-rail" :class="{ 'is-playing': store.isPlaying }">
                  <div class="np-timeline-rail-mask">
                    <!-- 高显副歌高潮辅助色区段 -->
                    <div
                      v-if="climaxRange"
                      class="np-timeline-climax-zone"
                      :style="{
                        left: `${climaxRange[0] * 100}%`,
                        width: `${(climaxRange[1] - climaxRange[0]) * 100}%`,
                      }"
                    ></div>

                    <!-- GPU 亚像素无极滑动双色彗星拖尾填充条（直接 DOM CSS 变量更新，零 Vue 重渲染） -->
                    <div
                      ref="timelineFillRef"
                      class="np-timeline-fill"
                    >
                      <span class="np-comet-tail-stream"></span>
                    </div>
                  </div>

                  <!-- 同步无极滑动的双色彗星羽化光晕与星核 -->
                  <div ref="cometHeadCarrierRef" class="np-comet-head-carrier">
                    <span class="np-comet-plume"></span>
                    <span class="np-comet-head"></span>
                  </div>
                </div>
              </div>

              <div class="np-timeline-labels">
                <span
                  class="np-time-num"
                  :class="{ active: isScrubbingProgress }"
                >
                  {{ formatDuration(shownElapsedSec) }}
                </span>

                <span class="np-time-num">
                  {{ formatDuration(store.durationMs / 1000) }}
                </span>
              </div>
            </div>

            <!-- 5 键主传输控制舱：模式 / 上一曲 / 封面双色主播放圆钮 / 下一曲 / 双色队列 -->
            <div class="np-transport-bar">
              <button
                type="button"
                class="np-transport-aux-btn"
                :class="{ 'is-active': store.playMode !== 'repeat_all' }"
                :style="{
                  transform:
                    store.playMode === 'repeat_all'
                      ? `rotate(${playModeSpinCount * 180}deg)`
                      : 'none',
                }"
                @click="
                  playModeSpinCount++;
                  store.cyclePlayMode();
                "
              >
                <PlumpIcon
                  :name="
                    store.playMode === 'repeat_one'
                      ? 'np-repeat-one'
                      : store.playMode === 'shuffle'
                      ? 'np-shuffle'
                      : store.playMode === 'sequence'
                      ? 'np-sequence'
                      : 'np-repeat'
                  "
                  :size="22"
                />
              </button>

              <button
                type="button"
                class="np-transport-skip-btn"
                :style="{ transform: `translateX(${prevNudgeX}px)` }"
                @click="triggerSkipNudge(false)"
              >
                <PlumpIcon name="np-prev" :size="27" />
              </button>

              <button
                type="button"
                class="np-hero-play-btn"
                :class="{ 'is-playing': store.isPlaying }"
                @click="store.togglePlayPause()"
              >
                <transition name="bm-icon-morph" mode="out-in">
                  <PlumpIcon
                    :key="store.isPlaying ? 'np-pause' : 'np-play'"
                    :name="store.isPlaying ? 'np-pause' : 'np-play'"
                    :size="28"
                    color="var(--np-on-accent-primary, #FFFFFF)"
                  />
                </transition>
              </button>

              <button
                type="button"
                class="np-transport-skip-btn"
                :style="{ transform: `translateX(${nextNudgeX}px)` }"
                @click="triggerSkipNudge(true)"
              >
                <PlumpIcon name="np-next" :size="27" />
              </button>

              <button
                type="button"
                class="np-transport-aux-btn np-queue-trigger-btn"
                :class="{ 'is-active': store.isNowPlayingQueueOpen }"
                aria-label="播放队列"
                @click.stop="store.toggleNowPlayingQueue($event.currentTarget as HTMLElement)"
              >
                <PlumpIcon
                  name="queue"
                  :size="21"
                  :color="store.isNowPlayingQueueOpen ? 'var(--np-accent-primary, #4da3ff)' : 'currentColor'"
                  accent="var(--np-accent-secondary, #38d2c0)"
                />
                <span v-if="store.climaxModeEnabled" class="np-queue-skim-dot"></span>
              </button>
            </div>

            <!-- 4 键双色功能坞 -->
            <div class="np-utility-dock">
              <button
                type="button"
                class="np-utility-icon-btn"
                :class="{ active: isAudioEffectActive }"
                @click="showAudioEffectDialog = true"
              >
                <PlumpIcon
                  name="wave"
                  :size="20"
                  :color="isAudioEffectActive ? 'var(--np-accent-primary, #4da3ff)' : 'currentColor'"
                  accent="var(--np-accent-secondary, #38d2c0)"
                />
                <span v-if="isAudioEffectActive" class="np-utility-dot"></span>
              </button>

              <button
                type="button"
                class="np-utility-icon-btn"
                @click="store.openCommentsModal(store.currentSong)"
              >
                <PlumpIcon
                  name="comment"
                  :size="20"
                  color="currentColor"
                  accent="var(--np-accent-secondary, #38d2c0)"
                />
                <span v-if="formattedCommentCount" class="np-utility-badge">
                  {{ formattedCommentCount }}
                </span>
              </button>

              <button
                type="button"
                class="np-utility-icon-btn"
                :class="{ active: store.desktopLyricEnabled }"
                @click="
                  store.desktopLyricEnabled = !store.desktopLyricEnabled;
                  store.notify(
                    store.desktopLyricEnabled
                      ? '已开启系统级桌面悬浮歌词'
                      : '已关闭桌面悬浮歌词'
                  );
                "
              >
                <PlumpIcon
                  name="lyric"
                  :size="20"
                  :color="store.desktopLyricEnabled ? 'var(--np-accent-primary, #4da3ff)' : 'currentColor'"
                  accent="var(--np-accent-secondary, #38d2c0)"
                />
              </button>

              <button
                type="button"
                class="np-utility-icon-btn"
                @click.stop="showMoreSheet = true"
              >
                <PlumpIcon
                  name="more"
                  :size="20"
                  color="currentColor"
                  accent="var(--np-accent-secondary, #38d2c0)"
                />
              </button>
            </div>
          </div>
        </div>
      </section>

      <!-- ==================== 右栏：画廊级全高 KRC 逐字流光歌词舞台 ==================== -->
      <section class="np-right-stage">
        <!-- 纯享歌词模式顶部微缩曲目抬头 -->
        <div
          v-if="store.playerStageStyle === 'pure_lyric'"
          class="np-pure-lyric-header"
        >
          <span class="np-pl-title">{{ store.currentSong?.title }}</span>
          <span class="np-pl-sep">—</span>
          <span class="np-pl-artist">{{ store.currentSong?.artist }}</span>
        </div>

        <!-- 空歌词或检索中状态 -->
        <div
          v-if="!store.currentLyric?.lines || store.currentLyric.lines.length === 0"
          class="np-lyric-empty-state"
        >
          <div class="np-lyric-empty-orb">♪</div>
          <p class="np-lyric-empty-title">
            {{
              store.isLoadingLyric
                ? "正在检索 KRC 逐字音画歌词..."
                : "纯音乐，请静享旋律流淌"
            }}
          </p>
          <span class="np-lyric-empty-sub">
            {{ store.currentSong?.title || "BreezeMusic Studio" }}
          </span>
        </div>

        <!-- 歌词滚动主视口 (采用 vh 内边距锚定中心，大字号 + 对称宽松左右边距 + 行级瀑布弹簧滚动) -->
        <div
          v-else
          ref="lyricViewportRef"
          class="np-lyric-scroll-viewport"
          :style="karaokeRenderTokens.stageCssVars"
          @wheel.passive="handleUserLyricWheel"
          @touchmove.passive="handleUserLyricWheel"
        >
          <div
            v-for="(line, idx) in store.currentLyric.lines"
            :key="idx"
            :data-lyric-idx="idx"
            class="np-lyric-line-item"
            :class="{
              'is-active': idx === activeLineIndex,
              'is-leaving': idx === leavingLineIndex,
              'is-past': idx < activeLineIndex && idx !== leavingLineIndex,
              'is-next': idx === activeLineIndex + 1,
              'is-far': idx > activeLineIndex + 1,
              'is-hovered': hoveredLyricIndex === idx,
            }"
            :style="getLyricLineContainerStyle(idx)"
            @mouseenter="hoveredLyricIndex = idx"
            @mouseleave="hoveredLyricIndex = null"
            @click="handleClickLyricLine(line)"
          >
            <!-- 1. 音译行 (粤语粤拼 / 日语罗马音 / 韩语罗马音 / 原生音译) -->
            <div
              v-if="
                shouldShowPhoneticButton &&
                store.showPhonetic &&
                getLinePhoneticText(line)
              "
              class="np-lyric-phonetic-line"
              :style="{
                fontSize: `${Math.max(13, Math.round(store.lyricFontSize * 0.56))}px`,
                color:
                  idx === activeLineIndex
                    ? hexWithAlpha(lyricHighlightColors.secondary, 0.92)
                    : store.lyricTheme === 'dynamic_cover'
                    ? palette.lyricTranslationRgba
                    : store.isNowPlayingLightStage
                    ? 'var(--np-ink-2)'
                    : hexWithAlpha(lerpHex(lyricHighlightColors.secondary, '#FFFFFF', 0.56), 0.62),
              }"
            >
              {{ getLinePhoneticText(line) }}
            </div>

            <!-- 2. 主歌词行：「光晕浮升 · 声浪呼吸」双层纯 GPU 合成逐字引擎 -->
            <div
              v-if="resolvedLyricWordsList[idx]?.length"
              class="np-karaoke-words-flow"
              :class="`align-${store.lyricAlignment}`"
              :style="{
                fontSize: `${store.lyricFontSize}px`,
                fontWeight: lyricTypographyActive.fontWeight,
                lineHeight: 1.44,
              }"
            >
              <span
                v-for="(w, wIdx) in resolvedLyricWordsList[idx]"
                :key="wIdx"
                class="np-karaoke-word-span"
                :data-text="w.text"
              ><span class="np-kw-base">{{ w.text }}</span><span
                  v-if="idx === activeLineIndex || idx === leavingLineIndex"
                  class="np-kw-lit"
                  aria-hidden="true"
                >{{ w.text }}</span></span>
            </div>

            <!-- 兜底空文本/纯文本行 -->
            <div
              v-else
              class="np-lyric-plain-main"
              :style="{
                fontSize: `${store.lyricFontSize}px`,
                fontWeight: lyricTypographyActive.fontWeight,
                WebkitTextStroke:
                  idx === activeLineIndex
                    ? lyricTypographyActive.opticalStrokeCss
                    : lyricTypographyInactive.opticalStrokeCss,
                lineHeight: 1.44,
                color:
                  idx === activeLineIndex
                    ? lyricHighlightColors.primary
                    : hexWithAlpha(lyricUnsungBaseHex, Math.min(0.92, store.lyricInactiveOpacity + 0.44)),
                textShadow:
                  idx === activeLineIndex
                    ? `0 0 24px ${
                        store.lyricTheme === 'dynamic_cover'
                          ? palette.lyricGlowRgba
                          : hexWithAlpha(lyricHighlightColors.primary, 0.38)
                      }`
                    : 'none',
              }"
            >
              {{ line.text }}
            </div>

            <!-- 3. 翻译行（与唱词主色系和谐统一） -->
            <div
              v-if="
                shouldShowTranslationButton &&
                store.showTranslation &&
                line.translation
              "
              class="np-lyric-trans-line"
              :style="getTranslationLineStyle(idx)"
            >
              {{ line.translation }}
            </div>

            <!-- 4. 间奏呼吸三圆点倒计时 -->
            <div
              v-if="shouldShowInterludeDots(idx)"
              class="np-interlude-dots-row"
              :class="`align-${store.lyricAlignment}`"
            >
              <span
                v-for="dotIdx in [0, 1, 2]"
                :key="dotIdx"
                class="np-interlude-dot"
                :style="getInterludeDotStyle(idx, dotIdx)"
              >
                ●
              </span>
            </div>

            <!-- 悬停行右侧即点即播时间胶囊 -->
            <span
              v-if="store.lyricClickSeekEnabled && hoveredLyricIndex === idx"
              class="np-lyric-hover-seek-badge"
            >
              ▶ {{ formatDuration(line.startTimeMs / 1000) }}
            </span>
          </div>
        </div>

        <!-- 右下角纯图标悬浮歌词控制条 (翻译 / 音译 / 歌词设置) -->
        <div class="np-lyric-floating-toolbar">
          <button
            v-if="shouldShowTranslationButton"
            type="button"
            class="np-lyric-tool-btn"
            :class="{ active: store.showTranslation }"
            @click="store.showTranslation = !store.showTranslation"
          >
            <PlumpIcon name="np-translate" :size="18" />
          </button>

          <button
            v-if="shouldShowPhoneticButton"
            type="button"
            class="np-lyric-tool-btn"
            :class="{ active: store.showPhonetic }"
            @click="store.showPhonetic = !store.showPhonetic"
          >
            <PlumpIcon name="np-phonetic" :size="18" />
          </button>

          <button
            type="button"
            class="np-lyric-tool-btn"
            :class="{ active: showLyricSettings }"
            @click.stop="showLyricSettings = true"
          >
            <PlumpIcon name="np-lyric-sliders" :size="18" />
          </button>
        </div>
      </section>
    </div>

    <!-- 纯享歌词模式底部悬浮迷你播控胶囊 -->
    <transition name="np-popover-fade">
      <div
        v-if="store.playerStageStyle === 'pure_lyric'"
        class="np-pure-lyric-bottom-dock"
        @click.stop
      >
        <img
          v-if="store.currentSong?.coverUrl"
          :src="store.currentSong.coverUrl"
          class="np-pl-mini-cover"
          alt=""
          @click="store.playerStageStyle = 'gallery_poster'"
        />
        <div class="np-pl-mini-meta">
          <div class="np-pl-mini-title">{{ store.currentSong?.title }}</div>
          <div class="np-pl-mini-artist">{{ store.currentSong?.artist }}</div>
        </div>
        <div class="np-pl-mini-controls">
          <button
            type="button"
            class="np-transport-aux-btn"
            @click="triggerSkipNudge(false)"
          >
            <PlumpIcon name="np-prev" :size="20" />
          </button>
          <button
            type="button"
            class="np-pl-mini-play"
            @click="store.togglePlayPause()"
          >
            <PlumpIcon
              :name="store.isPlaying ? 'np-pause' : 'np-play'"
              :size="18"
              color="var(--np-on-accent-primary, #FFFFFF)"
            />
          </button>
          <button
            type="button"
            class="np-transport-aux-btn"
            @click="triggerSkipNudge(true)"
          >
            <PlumpIcon name="np-next" :size="20" />
          </button>
        </div>
      </div>
    </transition>

    <!-- =========================================================================
         4. 全部弹窗与声学抽屉组
         ========================================================================= -->
    <NowPlayingDialogs
      v-model:show-lyric-settings="showLyricSettings"
      v-model:show-more-sheet="showMoreSheet"
      v-model:show-quality-dialog="showQualityDialog"
      v-model:show-audio-effect-dialog="showAudioEffectDialog"
    />
  </div>
</template>

<style scoped>
/* ============================================================================
   1. 根容器与全屏锁定架构 (100vh 零溢出，彻底根治 Grid 高度撑裂问题)
   ============================================================================ */
.np-root {
  position: relative;
  width: 100%;
  height: 100vh;
  max-height: 100vh;
  min-height: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  user-select: none;
  color: var(--np-on-surface, #ffffff);
  background: var(--np-bg-primary, var(--np-nebula-abyss, #05070c));
}

/* 1.1 博物馆级深邃环境背景层（四模式严格边界隔离，互不串扰） */
.np-ambient-backdrop {
  position: absolute;
  inset: 0;
  z-index: 0;
  overflow: hidden;
  pointer-events: none;
}

.np-bg-canvas {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  display: block;
}

.np-bg-canvas,
.np-bg-canvas-fluid_aurora,
.np-bg-canvas-rhythm_fluted,
.np-bg-canvas-rhythm_cymatic,
.np-bg-canvas-starlight_nebula,
.np-ambient-backdrop[data-bg-style="rhythm_fluted"],
.np-ambient-backdrop[data-bg-style="rhythm_fluted"] *,
.np-ambient-backdrop[data-bg-style="rhythm_cymatic"],
.np-ambient-backdrop[data-bg-style="rhythm_cymatic"] * {
  filter: none !important;
  backdrop-filter: none !important;
}

/* ============================================================================
   2. 左上角极简下箭头返回钮 + 顶部极简居中舞台与氛围胶囊栏 (10秒自动隐藏 · 无框色阶分层)
   ============================================================================ */
.np-top-left-back-btn {
  position: absolute;
  top: 15px;
  left: 22px;
  z-index: 40;
  width: 38px;
  height: 38px;
  border-radius: 50%;
  background: transparent;
  color: var(--np-ink-2, rgba(255, 255, 255, 0.72));
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    color 0.22s ease,
    background 0.22s ease,
    transform 0.24s cubic-bezier(0.22, 1.35, 0.36, 1);
}

.np-top-left-back-btn:hover {
  color: var(--np-ink-1, #ffffff);
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #5e6ad2) 14%,
    transparent
  );
  transform: translateY(1px) scale(1.06);
}

.np-skyline-bar {
  position: absolute;
  top: 14px;
  left: 0;
  right: 0;
  z-index: 35;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  opacity: 1;
  transform: translateY(0);
  transition:
    opacity 0.42s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.42s cubic-bezier(0.22, 1, 0.36, 1);
}

.np-skyline-bar.is-auto-hidden {
  opacity: 0;
  transform: translateY(-16px);
  pointer-events: none;
}

.np-center-mode-capsule {
  pointer-events: auto;
  height: 40px;
  padding: 4px 6px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(
    in oklab,
    var(--np-surface-container-low, #141923) 90%,
    transparent
  );
  backdrop-filter: blur(18px) saturate(165%);
  -webkit-backdrop-filter: blur(18px) saturate(165%);
  box-shadow: 0 12px 30px var(--np-ambient-shadow, rgba(0, 0, 0, 0.44));
  display: inline-flex;
  align-items: center;
  gap: 6px;
  -webkit-font-smoothing: antialiased;
}

.np-stage-mode-group {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 2px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(
    in oklab,
    var(--np-surface-container-lowest, #0d1118) 68%,
    transparent
  );
}

.np-stage-mode-pill {
  height: 28px;
  padding: 0 13px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 700;
  color: var(--np-ink-1, rgba(255, 255, 255, 0.88));
  opacity: 0.78;
  background: transparent;
  cursor: pointer;
  transition:
    color 0.18s ease,
    opacity 0.18s ease,
    background 0.2s ease;
}

.np-stage-mode-pill:hover:not(.active) {
  opacity: 1;
  color: var(--np-ink-1, #ffffff);
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #5e6ad2) 14%,
    transparent
  );
}

.np-stage-mode-pill.active {
  opacity: 1;
  color: var(--np-on-accent-primary, #ffffff);
  background: linear-gradient(
    135deg in oklab,
    var(--np-accent-primary, #4a9eff) 0%,
    color-mix(
      in oklab,
      var(--np-accent-primary, #4a9eff) 72%,
      var(--np-accent-secondary, #38d2c0)
    ) 100%
  );
  box-shadow: 0 4px 14px var(--np-accent-glow, rgba(0, 0, 0, 0.28));
}

.np-ambience-dropdown-wrap {
  position: relative;
}

.np-ambience-trigger,
.np-color-toggle-pill {
  height: 32px;
  padding: 0 13px;
  border-radius: var(--bm-r-pill, 999px);
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--np-ink-1, rgba(255, 255, 255, 0.94));
  background: color-mix(
    in oklab,
    var(--np-surface-container, #1a212e) 76%,
    transparent
  );
  cursor: pointer;
  transition: background 0.18s ease, color 0.18s ease;
}

.np-ambience-trigger:hover,
.np-ambience-trigger.active,
.np-color-toggle-pill:hover {
  color: var(--np-ink-1, #ffffff);
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #5e6ad2) 18%,
    var(--np-surface-container-highest, rgba(44, 53, 69, 0.9))
  );
}

.np-color-toggle-pill.active {
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #5e6ad2) 22%,
    var(--np-surface-container, #1a212e)
  );
  color: var(--np-ink-1, #ffffff);
}

.np-color-toggle-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  background: var(--np-ink-3, rgba(255, 255, 255, 0.45));
  transition: background 0.2s ease, box-shadow 0.2s ease;
}

.np-color-toggle-pill.active .np-color-toggle-dot {
  background: linear-gradient(
    135deg,
    var(--np-accent-primary, #5e6ad2),
    var(--np-accent-secondary, #38bdf8)
  );
  box-shadow: 0 0 8px var(--np-accent-glow, rgba(94, 106, 210, 0.55));
}

.np-color-toggle-pill.is-dark-lock .np-color-toggle-dot {
  background: radial-gradient(
    circle at 35% 35%,
    #fde68a 0%,
    var(--np-accent-primary, #5e6ad2) 68%,
    #111827 100%
  );
  box-shadow: 0 0 10px var(--np-accent-glow, rgba(94, 106, 210, 0.65));
}

.np-ambience-twin-orbs {
  display: inline-flex;
  align-items: center;
  position: relative;
  width: 14px;
  height: 9px;
  flex-shrink: 0;
}

.np-orb-primary,
.np-orb-secondary {
  width: 7.5px;
  height: 7.5px;
  border-radius: 50%;
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
}

.np-orb-primary {
  left: 0;
  z-index: 2;
  background: var(--np-accent-primary, #4a9eff);
  box-shadow: 0 0 8px var(--np-accent-primary, #4a9eff);
}

.np-orb-secondary {
  right: 0;
  z-index: 1;
  background: var(--np-accent-secondary, #38d2c0);
  box-shadow: 0 0 8px var(--np-accent-secondary, #38d2c0);
}

.np-ambience-popover {
  position: absolute;
  top: calc(100% + 10px);
  right: 0;
  width: 176px;
  padding: 7px;
  border-radius: var(--bm-r-lg, 18px);
  background: color-mix(
    in oklab,
    var(--np-surface-container-low, #141923) 92%,
    transparent
  );
  backdrop-filter: blur(30px) saturate(180%);
  -webkit-backdrop-filter: blur(30px) saturate(180%);
  box-shadow: 0 22px 52px var(--np-ambient-shadow, rgba(0, 0, 0, 0.58));
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.np-ambience-option {
  min-height: 44px;
  padding: 6px 12px;
  border-radius: var(--bm-r-md, 14px);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  color: rgba(255, 255, 255, 0.76);
  background: transparent;
  cursor: pointer;
  transition: background 0.18s ease, color 0.18s ease;
}

.np-ambience-opt-text {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1px;
  text-align: left;
}

.np-ambience-opt-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  line-height: 1.25;
}

.np-ambience-opt-sub {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 500;
  color: rgba(255, 255, 255, 0.52);
  line-height: 1.25;
}

.np-ambience-option:hover {
  color: #ffffff;
  background: color-mix(
    in oklab,
    var(--np-surface-container-high, #222a38) 76%,
    rgba(255, 255, 255, 0.05)
  );
}

.np-ambience-option.active {
  color: #ffffff;
  font-weight: 800;
  background: linear-gradient(
    135deg in oklab,
    color-mix(in oklab, var(--np-accent-primary, #4a9eff) 20%, rgba(28, 36, 52, 0.9)) 0%,
    color-mix(in oklab, var(--np-accent-secondary, #38d2c0) 14%, rgba(22, 28, 42, 0.9)) 100%
  );
}

.np-ambience-option.active .np-ambience-opt-sub {
  color: rgba(255, 255, 255, 0.68);
}

.np-ambience-opt-check {
  color: var(--np-accent-secondary, #38d2c0);
  text-shadow: 0 0 10px var(--np-secondary-glow, rgba(56, 210, 192, 0.4));
  font-weight: 900;
}

/* ============================================================================
   3. 双栏全宽舒展主舞台 (扩大封面舞台与歌词流之间的呼吸留白)
   ============================================================================ */
.np-main-workspace {
  position: relative;
  z-index: 5;
  flex: 1 1 0%;
  min-height: 0;
  width: 100%;
  max-width: min(94vw, 2160px);
  margin: 0 auto;
  height: 100%;
  padding: clamp(54px, 5.8vh, 72px) clamp(36px, 4vw, 80px) clamp(20px, 2.6vh, 34px);
  display: grid;
  grid-template-columns: minmax(0, 46%) minmax(0, 54%);
  grid-template-rows: minmax(0, 1fr);
  align-items: center;
  gap: clamp(28px, 3.8vw, 68px);
}

.np-main-workspace.is-pure-lyric {
  grid-template-columns: minmax(0, 1fr);
  max-width: min(86vw, 1360px);
  padding-bottom: 96px;
}

/* ==================== 左栏：统一对称封面舞台 + 右下竖排三键 + 沉浸无卡片播控台 ==================== */
.np-left-stage {
  width: clamp(410px, 34vw, 540px);
  max-width: 96%;
  height: 100%;
  min-height: 0;
  margin: 0 auto;
  padding-top: clamp(4px, 0.9vh, 14px);
  padding-bottom: clamp(10px, 1.8vh, 22px);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: clamp(18px, 2.7vh, 34px);
}

/* 封面宿主容器：完全无边界溢流展示，封面严格居中，三联按钮安全避让旋转流光框外缘 */
.np-cover-hero-box {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: visible;
}

.np-cover-stage-bundle {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  overflow: visible;
}

.np-cover-side-actions {
  position: absolute;
  left: calc(100% + 26px);
  bottom: 10px;
  z-index: 6;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}

.np-side-action-btn {
  width: 38px;
  height: 38px;
  border-radius: 50%;
  border: none;
  background: transparent;
  box-shadow: none;
  color: var(--np-ink-2, rgba(255, 255, 255, 0.78));
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    transform 0.24s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.22s ease,
    color 0.22s ease,
    box-shadow 0.22s ease;
}

.np-side-action-btn:hover {
  color: var(--np-ink-1, #ffffff);
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #5e6ad2) 16%,
    transparent
  );
  transform: translateY(-1px) scale(1.12);
  box-shadow: 0 6px 18px var(--np-accent-glow, rgba(94, 106, 210, 0.24));
}

.np-side-action-btn.liked {
  color: #ff4d6d;
}

.np-side-action-btn.liked:hover {
  background: rgba(255, 77, 109, 0.14);
  box-shadow: 0 6px 18px rgba(255, 77, 109, 0.24);
}

.np-side-action-btn.burst-pop {
  animation: favHeartPulse 0.42s cubic-bezier(0.22, 1.4, 0.36, 1);
}

.np-console-stack {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: clamp(14px, 1.9vh, 22px);
}

/* 完全沉浸无框无卡片播控区：直接落于剧场背景之上 */
.np-studio-console-deck {
  width: 100%;
  border-radius: 0;
  border: none;
  padding: 6px 4px 2px;
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow: none;
  display: flex;
  flex-direction: column;
  gap: clamp(14px, 1.85vh, 20px);
}

/* 3.1 曲目分层信息：歌名 / 多位歌手流式列表 / 极简音质参数胶囊 */
.np-meta-header {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 12px;
}

.np-meta-texts {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.np-title-marquee-viewport {
  width: 100%;
  overflow: hidden;
  position: relative;
}

.np-title-marquee-viewport.is-overflowing {
  -webkit-mask-image: linear-gradient(
    90deg,
    transparent 0%,
    #000 12px,
    #000 calc(100% - 18px),
    transparent 100%
  );
  mask-image: linear-gradient(
    90deg,
    transparent 0%,
    #000 12px,
    #000 calc(100% - 18px),
    transparent 100%
  );
}

.np-song-title-text {
  margin: 0;
  display: inline-block;
  min-width: 100%;
  font-size: clamp(23px, 2vw, 29px);
  font-weight: 780;
  color: var(--np-ink-1, rgba(255, 255, 255, 0.96));
  letter-spacing: -0.015em;
  line-height: 1.24;
  white-space: nowrap;
}

.np-title-marquee-viewport.is-overflowing .np-song-title-text {
  animation: npMetaPingPong var(--marquee-dur, 12s) ease-in-out infinite alternate;
  will-change: transform;
}

.np-title-marquee-viewport.is-overflowing:hover .np-song-title-text {
  animation-play-state: paused;
}

.np-artists-flow-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 3px;
  width: 100%;
  min-width: 0;
  max-height: 72px;
  overflow-y: auto;
  scrollbar-width: none;
}

.np-artists-flow-row::-webkit-scrollbar {
  display: none;
}

.np-meta-link.artist,
.np-artist-chip-btn {
  display: inline-flex;
  align-items: center;
  padding: 1px 4px;
  margin: -1px -2px;
  border-radius: var(--bm-r-sm, 10px);
  background: transparent;
  font-size: clamp(14px, 1.12vw, 16px);
  font-weight: 560;
  line-height: 1.38;
  color: var(--np-ink-2, rgba(255, 255, 255, 0.64));
  white-space: normal;
  word-break: keep-all;
  text-align: left;
  cursor: pointer;
  transition:
    color 0.2s ease,
    background 0.2s ease,
    transform 0.2s cubic-bezier(0.22, 1, 0.36, 1);
}

.np-artist-chip-btn:hover {
  color: var(--np-ink-1, #ffffff);
  background: color-mix(in oklab, var(--np-accent-primary, #5e6ad2) 12%, transparent);
}

.np-artist-sep {
  color: var(--np-ink-3, rgba(255, 255, 255, 0.3));
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 500;
  margin: 0 3px;
  user-select: none;
}

.np-quality-row {
  display: flex;
  align-items: center;
  width: 100%;
  min-width: 0;
  margin-top: 2px;
}

.np-quality-spec-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3.5px 10px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 640;
  letter-spacing: 0.025em;
  line-height: 1.35;
  color: var(--np-ink-2, rgba(255, 255, 255, 0.86));
  background: color-mix(
    in oklab,
    var(--np-accent-secondary, #38d2c0) 14%,
    transparent
  );
  border: none;
  cursor: pointer;
  max-width: 100%;
  min-width: 0;
  transition:
    transform 0.22s cubic-bezier(0.22, 1, 0.36, 1),
    background 0.22s ease,
    box-shadow 0.22s ease,
    border-radius 0.22s ease,
    color 0.22s ease;
}

.np-quality-dot {
  width: 5.5px;
  height: 5.5px;
  border-radius: 50%;
  flex-shrink: 0;
  background: var(--np-dual-gradient, linear-gradient(135deg, var(--np-accent-primary, #4da3ff), var(--np-accent-secondary, #38d2c0)));
  box-shadow: 0 0 7px var(--np-accent-secondary, #38d2c0);
}

.np-quality-marquee-viewport {
  display: inline-block;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  white-space: nowrap;
}

.np-quality-marquee-track {
  display: inline-block;
  white-space: nowrap;
}

.np-quality-spec-pill.is-overflowing .np-quality-marquee-viewport {
  -webkit-mask-image: linear-gradient(
    90deg,
    transparent 0%,
    #000 10px,
    #000 calc(100% - 14px),
    transparent 100%
  );
  mask-image: linear-gradient(
    90deg,
    transparent 0%,
    #000 10px,
    #000 calc(100% - 14px),
    transparent 100%
  );
}

.np-quality-spec-pill.is-overflowing .np-quality-marquee-track {
  animation: npMetaPingPong var(--marquee-dur, 10s) ease-in-out infinite alternate;
  will-change: transform;
}

.np-quality-spec-pill.is-overflowing:hover {
  border-radius: var(--bm-r-md, 14px);
}

.np-quality-spec-pill.is-overflowing:hover .np-quality-marquee-viewport {
  white-space: normal;
  overflow: visible;
  -webkit-mask-image: none;
  mask-image: none;
}

.np-quality-spec-pill.is-overflowing:hover .np-quality-marquee-track {
  animation: none;
  transform: none;
  white-space: normal;
  word-break: break-word;
  text-align: left;
}

.np-quality-spec-pill.is-gold {
  color: color-mix(in oklab, var(--np-prism-gold, #ffd166) 84%, #ffffff);
  background: color-mix(
    in oklab,
    var(--np-prism-gold, #ffd166) 10%,
    rgba(255, 255, 255, 0.04)
  );
  box-shadow: 0 2px 10px color-mix(in oklab, var(--np-prism-gold, #ffd166) 16%, transparent);
}

.np-quality-spec-pill:hover {
  transform: translateY(-1px);
  color: #ffffff;
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #4da3ff) 16%,
    rgba(255, 255, 255, 0.08)
  );
  box-shadow: 0 4px 12px color-mix(in oklab, var(--np-accent-primary, #7c6af6) 20%, transparent);
}

@keyframes npMetaPingPong {
  0%,
  16% {
    transform: translate3d(0, 0, 0);
  }
  84%,
  100% {
    transform: translate3d(calc(-1 * var(--marquee-dist, 0px)), 0, 0);
  }
}

@keyframes favHeartPulse {
  0% {
    transform: scale(0.8);
  }
  55% {
    transform: scale(1.22);
  }
  100% {
    transform: scale(1);
  }
}

/* 3.2 克制流光进度时间轴 + 柔和副歌线段 */
.np-timeline-box {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.np-timeline-track-hit {
  position: relative;
  width: 100%;
  height: 22px;
  display: flex;
  align-items: center;
  cursor: pointer;
}

.np-timeline-rail {
  position: relative;
  width: 100%;
  height: 5px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--np-track-bg, rgba(255, 255, 255, 0.13));
  overflow: visible;
  transition:
    height 0.24s cubic-bezier(0.22, 1, 0.36, 1),
    background 0.2s ease;
}

.np-timeline-rail-mask {
  position: absolute;
  inset: 0;
  border-radius: var(--bm-r-pill, 999px);
  overflow: hidden;
  pointer-events: none;
}

.np-timeline-track-hit:hover .np-timeline-rail,
.np-timeline-track-hit.is-scrubbing .np-timeline-rail {
  height: 7.5px;
  background: color-mix(in oklab, var(--np-ink-1, #ffffff) 16%, transparent);
}

/* 高辨识度辅助色副歌高潮区间线段 */
.np-timeline-climax-zone {
  position: absolute;
  top: 0;
  bottom: 0;
  z-index: 1;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(in oklab, var(--np-accent-secondary, #38d2c0) 74%, rgba(255, 255, 255, 0.18));
  box-shadow: 0 0 8px var(--np-secondary-glow, rgba(56, 210, 192, 0.34));
  pointer-events: none;
}

/* 双色流光进度填充：纯 GPU translate3d 无极滑动，零布局重排 */
.np-timeline-fill {
  position: relative;
  z-index: 2;
  width: 100%;
  height: 100%;
  border-radius: var(--bm-r-pill, 999px);
  background: linear-gradient(
    90deg in oklab,
    color-mix(in oklab, var(--np-accent-primary, #3898ff) 72%, transparent) 0%,
    var(--np-accent-primary, #3898ff) 64%,
    color-mix(in oklab, var(--np-accent-primary, #3898ff) 42%, var(--np-accent-secondary, #38d2c0)) 90%,
    var(--np-accent-secondary, #38d2c0) 100%
  );
  box-shadow:
    0 0 12px var(--np-accent-glow, rgba(77, 163, 255, 0.28)),
    0 0 16px var(--np-secondary-subtle, rgba(56, 210, 192, 0.2));
  transform: translate3d(var(--prog-shift, -100%), 0, 0);
  will-change: transform;
  transition: none !important;
}

.np-comet-head-carrier {
  position: absolute;
  inset: 0;
  z-index: 3;
  pointer-events: none;
  transform: translate3d(var(--prog-shift, -100%), 0, 0);
  opacity: var(--comet-head-opacity, 1);
  will-change: transform, opacity;
  transition: none !important;
}

/* 彗星尾迹柔和微光渐隐带 (主强调 -> 副辅助光晕过度，固定宽度 + 纯透明度脉冲，零重排) */
.np-comet-tail-stream {
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  width: min(100%, 64px);
  border-radius: var(--bm-r-pill, 999px);
  background: linear-gradient(
    90deg,
    rgba(255, 255, 255, 0) 0%,
    color-mix(in oklab, var(--np-accent-primary, #3898ff) 44%, transparent) 44%,
    color-mix(in oklab, var(--np-accent-secondary, #38d2c0) 76%, #ffffff) 100%
  );
  opacity: 0.78;
  pointer-events: none;
}

/* 彗星颈部羽化微光 */
.np-comet-plume {
  position: absolute;
  right: -5px;
  top: 50%;
  width: 24px;
  height: 11px;
  border-radius: var(--bm-r-pill, 999px);
  background: radial-gradient(
    ellipse at 74% 50%,
    rgba(255, 255, 255, 0.78) 0%,
    color-mix(in oklab, var(--np-accent-secondary, #38d2c0) 56%, var(--np-accent-primary, #3898ff)) 48%,
    transparent 100%
  );
  transform: translateY(-50%) scale(1, 1);
  filter: blur(3px);
  opacity: 0.76;
  pointer-events: none;
}

@keyframes npCometPlumeBreathe {
  0%, 100% {
    opacity: 0.72;
    transform: translateY(-50%) scale(0.92, 0.90);
  }
  50% {
    opacity: 0.96;
    transform: translateY(-50%) scale(1.14, 1.08);
  }
}

@keyframes npCometTailBreathe {
  0%, 100% {
    opacity: 0.72;
  }
  50% {
    opacity: 0.96;
  }
}

.np-timeline-rail.is-playing .np-comet-plume {
  animation: npCometPlumeBreathe 3.2s ease-in-out infinite;
}

.np-timeline-rail.is-playing .np-comet-tail-stream {
  animation: npCometTailBreathe 3.2s ease-in-out infinite;
}

/* 进度星核（平时小巧克制，悬浮时放大呼应双色光晕） */
.np-comet-head {
  position: absolute;
  right: -4.5px;
  top: 50%;
  width: 9.5px;
  height: 9.5px;
  border-radius: 50%;
  background: #ffffff;
  transform: translateY(-50%) scale(0.84);
  box-shadow:
    0 0 6px rgba(255, 255, 255, 0.9),
    0 0 11px var(--np-accent-secondary, #38d2c0),
    0 0 16px var(--np-accent-primary, #3898ff);
  transition: transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1);
  pointer-events: none;
}

.np-timeline-track-hit:hover .np-comet-head,
.np-timeline-track-hit.is-scrubbing .np-comet-head {
  transform: translateY(-50%) scale(1.22);
}

.np-timeline-scrub-pill {
  position: absolute;
  bottom: 24px;
  transform: translateX(-50%);
  padding: 4px 10px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(in oklab, var(--np-surface-container-low, #0e121c) 90%, var(--np-accent-primary, #4da3ff) 8%);
  backdrop-filter: blur(14px);
  border: none;
  color: #ffffff;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 680;
  font-variant-numeric: tabular-nums;
  box-shadow:
    0 8px 20px rgba(0, 0, 0, 0.44),
    0 0 14px var(--np-secondary-subtle, rgba(56, 210, 192, 0.22));
  pointer-events: none;
  white-space: nowrap;
}

.np-timeline-labels {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--bm-fs-meta, 13px);
  color: var(--np-ink-3, rgba(255, 255, 255, 0.48));
}

.np-time-num {
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  min-width: 46px;
}

.np-time-num:last-child {
  text-align: right;
}

.np-time-num.active {
  color: var(--np-ink-1, rgba(255, 255, 255, 0.94));
  font-weight: 700;
}

/* 3.3 圆润主传输控制舱 */
.np-transport-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 2px 6px;
}

.np-transport-aux-btn {
  position: relative;
  width: 46px;
  height: 46px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--np-ink-2, rgba(255, 255, 255, 0.62));
  background: transparent;
  cursor: pointer;
  transition:
    color 0.22s ease,
    background 0.22s ease,
    box-shadow 0.22s ease,
    transform 0.26s cubic-bezier(0.22, 1.35, 0.36, 1);
}

.np-transport-aux-btn:hover {
  color: var(--np-ink-1, #ffffff);
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #4da3ff) 16%,
    transparent
  );
  transform: scale(1.08);
}

.np-transport-aux-btn.is-active {
  color: var(--np-accent-secondary, #38d2c0);
  background: color-mix(
    in oklab,
    var(--np-accent-secondary, #38d2c0) 16%,
    rgba(255, 255, 255, 0.05)
  );
  box-shadow: 0 0 18px var(--np-secondary-glow, rgba(56, 210, 192, 0.24));
}

.np-queue-skim-dot {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 5.5px;
  height: 5.5px;
  border-radius: 50%;
  background: var(--np-accent-secondary, #38d2c0);
  box-shadow: 0 0 7px var(--np-accent-secondary, #38d2c0);
  pointer-events: none;
}

.np-transport-skip-btn {
  width: 54px;
  height: 54px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--np-ink-1, rgba(255, 255, 255, 0.9));
  background: transparent;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    color 0.2s ease,
    background 0.2s ease;
}

.np-transport-skip-btn:hover {
  color: var(--np-ink-1, #ffffff);
  background: color-mix(in oklab, var(--np-accent-primary, #4da3ff) 14%, transparent);
  transform: scale(1.08);
}

.np-hero-play-btn {
  width: 66px;
  height: 66px;
  border-radius: 50%;
  background: linear-gradient(
    135deg in oklab,
    var(--np-accent-primary, #4da3ff) 0%,
    color-mix(
      in oklab,
      var(--np-accent-primary, #4da3ff) 64%,
      var(--np-accent-secondary, #38d2c0)
    ) 100%
  );
  color: var(--np-on-accent-primary, #ffffff);
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow:
    0 12px 30px rgba(0, 0, 0, 0.34),
    0 0 26px var(--np-accent-glow, rgba(77, 163, 255, 0.32)),
    0 0 40px var(--np-secondary-subtle, rgba(56, 210, 192, 0.18));
  cursor: pointer;
  transition:
    transform 0.26s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.28s ease,
    box-shadow 0.28s ease;
}

.np-hero-play-btn.is-playing {
  box-shadow:
    0 14px 32px rgba(0, 0, 0, 0.46),
    0 0 28px var(--np-accent-glow, rgba(77, 163, 255, 0.38)),
    0 0 42px var(--np-secondary-glow, rgba(56, 210, 192, 0.26));
}

.np-hero-play-btn:hover {
  transform: scale(1.07);
  filter: brightness(1.06);
  box-shadow:
    0 16px 38px rgba(0, 0, 0, 0.5),
    0 0 32px var(--np-accent-glow, rgba(77, 163, 255, 0.42)),
    0 0 46px var(--np-secondary-glow, rgba(56, 210, 192, 0.32));
}

/* 3.4 底部控制坞：安静柔和排在主控五键下方，无框悬停泛彩光 */
.np-utility-dock {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: clamp(22px, 2.5vw, 32px);
  padding-top: 2px;
}

.np-utility-icon-btn {
  position: relative;
  width: 42px;
  height: 42px;
  border-radius: 50%;
  background: transparent;
  color: var(--np-ink-2, rgba(255, 255, 255, 0.56));
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    background 0.22s ease,
    color 0.22s ease,
    box-shadow 0.24s ease,
    transform 0.24s cubic-bezier(0.22, 1.35, 0.36, 1);
}

.np-utility-icon-btn:hover {
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #4da3ff) 16%,
    transparent
  );
  color: var(--np-ink-1, #ffffff);
  box-shadow: 0 6px 18px var(--np-accent-glow, rgba(77, 163, 255, 0.22));
  transform: translateY(-1.5px) scale(1.08);
}

.np-utility-icon-btn.active {
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #4da3ff) 14%,
    rgba(255, 255, 255, 0.04)
  );
  color: var(--np-accent-primary, #4da3ff);
}

.np-utility-dot {
  position: absolute;
  top: 7px;
  right: 7px;
  width: 5.5px;
  height: 5.5px;
  border-radius: 50%;
  background: var(--np-accent-secondary, #38d2c0);
  box-shadow: 0 0 7px var(--np-accent-secondary, #38d2c0);
}

.np-utility-badge {
  position: absolute;
  top: -2px;
  right: -6px;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 760;
  color: var(--np-on-accent-secondary, #080b11);
  background: var(--np-accent-secondary, #38d2c0);
  padding: 1px 5.5px;
  border-radius: var(--bm-r-pill, 999px);
  line-height: 1.3;
  box-shadow: 0 2px 8px var(--np-secondary-glow, rgba(56, 210, 192, 0.32));
  pointer-events: none;
}

/* ============================================================================
   4. 右栏：画廊级全高 KRC 逐字流光歌词舞台 (宽松呼吸感 + 柔和珠光焦点)
   ============================================================================ */
.np-right-stage {
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.np-pure-lyric-header {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding-bottom: 10px;
  flex-shrink: 0;
}

.np-pl-title {
  font-size: 20px;
  font-weight: 780;
  color: #ffffff;
}

.np-pl-sep {
  color: rgba(255, 255, 255, 0.32);
}

.np-pl-artist {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: 560;
  color: rgba(255, 255, 255, 0.64);
}

.np-lyric-empty-state {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: rgba(255, 255, 255, 0.52);
}

.np-lyric-empty-orb {
  width: 60px;
  height: 60px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 26px;
  color: rgba(255, 255, 255, 0.42);
}

.np-lyric-empty-title {
  margin: 0;
  font-size: 17px;
  font-weight: 680;
  color: rgba(255, 255, 255, 0.72);
}

.np-lyric-empty-sub {
  font-size: var(--bm-fs-meta, 13.5px);
  color: rgba(255, 255, 255, 0.4);
}

/* 歌词滚动视口：扩大左右呼吸留白与上下柔和渐隐 */
.np-lyric-scroll-viewport {
  flex: 1 1 0%;
  min-height: 0;
  width: 100%;
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 36vh clamp(52px, 4.8vw, 92px) 42vh clamp(52px, 4.8vw, 92px);
  -webkit-mask-image: linear-gradient(
    to bottom,
    transparent 0%,
    black 14%,
    black 86%,
    transparent 100%
  );
  mask-image: linear-gradient(
    to bottom,
    transparent 0%,
    black 14%,
    black 86%,
    transparent 100%
  );
}

.np-lyric-line-item {
  position: relative;
  width: 100%;
  border-radius: var(--bm-r-lg, 18px);
  padding-left: 20px;
  padding-right: 20px;
  cursor: pointer;
  transform: translate3d(0, var(--np-line-rest-y, 0px), 0)
    scale(var(--np-line-rest-s, 1));
  transition:
    opacity 0.44s cubic-bezier(0.22, 1, 0.36, 1),
    filter 0.44s cubic-bezier(0.22, 1, 0.36, 1),
    background 0.22s ease;
}

.np-main-workspace.is-pure-lyric .np-lyric-line-item {
  padding-left: 28px;
  padding-right: 28px;
}

.np-lyric-line-item:hover {
  background: rgba(255, 255, 255, 0.045);
}

.np-lyric-phonetic-line {
  font-weight: 560;
  letter-spacing: 0.03em;
  margin-bottom: 5px;
  transition: color 0.38s ease;
}

.np-karaoke-words-flow {
  display: flex;
  flex-wrap: wrap;
  letter-spacing: inherit;
}

.np-karaoke-words-flow.align-left {
  justify-content: flex-start;
}
.np-karaoke-words-flow.align-center {
  justify-content: center;
}
.np-karaoke-words-flow.align-right {
  justify-content: flex-end;
}

/* 「光晕浮升 · 声浪呼吸」双层纯 GPU 合成字粒结构（零每帧 gradient/drop-shadow 重绘） */
.np-karaoke-word-span {
  position: relative;
  display: inline-block;
  white-space: pre-wrap;
  transform-origin: center 72%;
}

.np-lyric-line-item.is-active .np-karaoke-word-span,
.np-lyric-line-item.is-leaving .np-karaoke-word-span {
  transform: translate3d(0, var(--kw-y, 0px), 0) scale(var(--kw-s, 1));
  will-change: transform;
}

/* 静态预光栅化辉光伪层：仅在活跃行与 380ms 退场余韵行启用，每帧仅合成 opacity，字缘零发毛 */
.np-lyric-line-item.is-active .np-karaoke-word-span::before,
.np-lyric-line-item.is-leaving .np-karaoke-word-span::before {
  content: attr(data-text);
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  user-select: none;
  white-space: pre-wrap;
  color: transparent;
  -webkit-text-fill-color: transparent;
  text-shadow:
    0 0 var(--np-kw-glow-r1, 14px) var(--np-kw-glow-c1, rgba(255, 255, 255, 0.34)),
    0 0 var(--np-kw-glow-r2, 28px) var(--np-kw-glow-c2, rgba(255, 255, 255, 0.16));
  opacity: var(--kw-glow, 0);
}

.np-kw-base {
  position: relative;
  z-index: 1;
  display: inline-block;
  white-space: pre-wrap;
  color: var(--np-kw-far-color, rgba(255, 255, 255, 0.62));
  -webkit-text-stroke: var(--np-kw-stroke-inactive, 0px transparent);
  paint-order: stroke fill;
  transition: color 0.38s ease;
}

.np-lyric-line-item.is-past .np-kw-base,
.np-lyric-line-item.is-leaving .np-kw-base {
  color: var(--np-kw-past-color, rgba(255, 255, 255, 0.54));
}

.np-lyric-line-item.is-next .np-kw-base {
  color: var(--np-kw-next-color, rgba(255, 255, 255, 0.72));
}

.np-lyric-line-item.is-active .np-kw-base {
  color: var(--np-kw-unsung-color, rgba(255, 255, 255, 0.52));
  -webkit-text-stroke: var(--np-kw-stroke-unsung, 0px transparent);
  transition: none;
}

.np-lyric-line-item.is-active .np-kw-base,
.np-lyric-line-item.is-leaving .np-kw-base {
  opacity: calc(1 - var(--kw-lit, 0) * 0.92);
}

.np-kw-lit {
  position: absolute;
  inset: 0;
  z-index: 2;
  display: inline-block;
  white-space: pre-wrap;
  pointer-events: none;
  user-select: none;
  background-image: var(--np-kw-lit-bg);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
  -webkit-text-fill-color: transparent;
  -webkit-text-stroke: var(--np-kw-stroke-sung, 0px transparent);
  paint-order: stroke fill;
  opacity: var(--kw-lit, 0);
}

.np-lyric-plain-main {
  word-break: break-word;
  letter-spacing: -0.01em;
  transition:
    color 0.38s ease,
    text-shadow 0.38s ease;
}

.np-lyric-trans-line {
  margin-top: 7px;
  line-height: 1.44;
  transition:
    color 0.38s ease,
    text-shadow 0.38s ease;
}

.np-interlude-dots-row {
  display: flex;
  gap: 8px;
  margin-top: 10px;
  font-size: var(--bm-fs-body, 14.5px);
}
.np-interlude-dots-row.align-left {
  justify-content: flex-start;
}
.np-interlude-dots-row.align-center {
  justify-content: center;
}
.np-interlude-dots-row.align-right {
  justify-content: flex-end;
}

.np-interlude-dot {
  display: inline-block;
  transition: transform 0.14s ease, color 0.18s ease;
}

.np-lyric-hover-seek-badge {
  position: absolute;
  right: 16px;
  top: 50%;
  transform: translateY(-50%);
  padding: 4px 10px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(
    in oklab,
    var(--np-surface-container-low, #121824) 84%,
    var(--np-accent-primary, #4da3ff) 14%
  );
  border: none;
  color: #ffffff;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  box-shadow: 0 4px 14px var(--np-secondary-subtle, rgba(56, 210, 192, 0.22));
  pointer-events: none;
}

/* 右下角纯图标竖向轻盈歌词控制胶囊 (平时安静透明，悬浮时浮现磨砂胶囊与纯色点缀光效) */
.np-lyric-floating-toolbar {
  position: absolute;
  right: 20px;
  bottom: 18px;
  z-index: 12;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 6px 5px;
  border-radius: var(--bm-r-pill, 999px);
  background: transparent;
  opacity: 0.42;
  transition:
    opacity 0.26s ease,
    background 0.26s ease,
    transform 0.26s cubic-bezier(0.22, 1, 0.36, 1),
    box-shadow 0.26s ease;
}

.np-lyric-floating-toolbar:hover {
  opacity: 1;
  background: color-mix(
    in oklab,
    var(--np-surface-container, #161c28) 82%,
    transparent
  );
  backdrop-filter: blur(20px) saturate(160%);
  -webkit-backdrop-filter: blur(20px) saturate(160%);
  transform: translateY(-2px);
  box-shadow: 0 16px 36px rgba(0, 0, 0, 0.42);
}

.np-lyric-tool-btn {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  color: rgba(255, 255, 255, 0.68);
  background: transparent;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    color 0.2s ease,
    background 0.2s ease,
    box-shadow 0.22s ease,
    transform 0.24s cubic-bezier(0.22, 1.35, 0.36, 1);
}

.np-lyric-tool-btn:hover {
  color: #ffffff;
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #4da3ff) 16%,
    rgba(255, 255, 255, 0.08)
  );
  transform: translateY(-1px) scale(1.06);
}

.np-lyric-tool-btn.active {
  color: var(--np-accent-primary, #4da3ff);
  background: color-mix(
    in oklab,
    var(--np-accent-primary, #4da3ff) 14%,
    rgba(255, 255, 255, 0.05)
  );
}

/* 纯享歌词模式底部迷你悬浮播控条 (100% 无框色阶面板) */
.np-pure-lyric-bottom-dock {
  position: absolute;
  bottom: 22px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 30;
  height: 60px;
  padding: 6px 18px 6px 8px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(
    in oklab,
    var(--np-surface-container-low, #141923) 86%,
    transparent
  );
  backdrop-filter: blur(28px) saturate(180%);
  -webkit-backdrop-filter: blur(28px) saturate(180%);
  box-shadow: 0 22px 52px var(--np-ambient-shadow, rgba(0, 0, 0, 0.56));
  display: flex;
  align-items: center;
  gap: 14px;
}

.np-pl-mini-cover {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  object-fit: cover;
  cursor: pointer;
}

.np-pl-mini-meta {
  max-width: 220px;
  min-width: 110px;
}

.np-pl-mini-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: 800;
  color: var(--np-on-surface, #ffffff);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.np-pl-mini-artist {
  font-size: var(--bm-fs-caption, 12.5px);
  color: rgba(255, 255, 255, 0.64);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.np-pl-mini-controls {
  display: flex;
  align-items: center;
  gap: 6px;
}

.np-pl-mini-play {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: var(
    --np-dual-gradient,
    linear-gradient(135deg, var(--np-accent-primary, #4da3ff), var(--np-accent-secondary, #38d2c0))
  );
  color: var(--np-on-accent-primary, #ffffff);
  box-shadow: 0 6px 18px var(--np-accent-glow, rgba(77, 163, 255, 0.3));
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

/* 动效与响应式适配 */
.np-popover-fade-enter-active,
.np-popover-fade-leave-active {
  transition: opacity 0.2s ease, transform 0.22s cubic-bezier(0.22, 1, 0.36, 1);
}
.np-popover-fade-enter-from,
.np-popover-fade-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.96);
}

/* ============================================================================
   浅色舞台专属清晰化精修（覆盖：L1/L2 四主题浅色舞台 + 封面取色 · 浅色漫染舞台）
   ============================================================================ */
.np-root.is-light-stage .np-center-mode-capsule,
.np-root.is-light-stage .np-ambience-popover,
.np-root.is-light-stage .np-pure-lyric-bottom-dock,
.np-root.is-light-stage .np-lyric-floating-toolbar:hover {
  background: rgba(255, 255, 255, 0.90);
  box-shadow: 0 12px 32px rgba(24, 24, 27, 0.12);
}

.np-root.is-light-stage.is-dynamic-light-stage .np-center-mode-capsule,
.np-root.is-light-stage.is-dynamic-light-stage .np-ambience-popover,
.np-root.is-light-stage.is-dynamic-light-stage .np-pure-lyric-bottom-dock,
.np-root.is-light-stage.is-dynamic-light-stage .np-lyric-floating-toolbar:hover {
  background: color-mix(in oklab, var(--np-mesh-primary, #faf8f5) 86%, #ffffff);
  backdrop-filter: blur(26px) saturate(1.3);
  -webkit-backdrop-filter: blur(26px) saturate(1.3);
  box-shadow: 0 12px 32px color-mix(in oklab, var(--np-accent-primary, #5e6ad2) 14%, rgba(24, 24, 27, 0.10));
}

.np-root.is-light-stage.is-oat-stage .np-center-mode-capsule,
.np-root.is-light-stage.is-oat-stage .np-ambience-popover,
.np-root.is-light-stage.is-oat-stage .np-pure-lyric-bottom-dock,
.np-root.is-light-stage.is-oat-stage .np-lyric-floating-toolbar:hover {
  background: #faf6ee;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow: 0 12px 30px rgba(42, 33, 22, 0.16);
}

.np-root.is-light-stage .np-stage-mode-group {
  background: rgba(24, 24, 27, 0.065);
}

.np-root.is-light-stage .np-ambience-trigger,
.np-root.is-light-stage .np-color-toggle-pill {
  background: rgba(24, 24, 27, 0.06);
  color: var(--np-ink-1);
}

.np-root.is-light-stage .np-color-toggle-pill.active {
  background: color-mix(in oklab, var(--np-accent-primary) 16%, transparent);
  color: var(--np-accent-primary);
}

.np-root.is-light-stage .np-ambience-option {
  color: var(--np-ink-2);
}

.np-root.is-light-stage .np-ambience-opt-sub {
  color: var(--np-ink-3);
}

.np-root.is-light-stage .np-ambience-option.active,
.np-root.is-light-stage .np-ambience-option:hover {
  color: var(--np-ink-1);
  background: color-mix(in oklab, var(--np-accent-primary) 13%, transparent);
}

.np-root.is-light-stage .np-lyric-line-item:hover {
  background: color-mix(in oklab, var(--np-accent-primary) 6%, transparent);
}

.np-root.is-light-stage .np-lyric-tool-btn {
  color: var(--np-ink-2);
}

.np-root.is-light-stage .np-lyric-tool-btn:hover {
  color: var(--np-ink-1);
}

.np-root.is-light-stage .np-quality-spec-pill.is-gold {
  color: #92400e;
  background: rgba(180, 83, 9, 0.12);
}

.np-root.is-light-stage .np-quality-spec-pill:hover {
  color: var(--np-ink-1);
}

.np-root.is-light-stage .np-timeline-scrub-pill,
.np-root.is-light-stage .np-lyric-hover-seek-badge {
  background: var(--np-surface-container-highest, #ffffff);
  color: var(--np-ink-1);
  box-shadow: 0 8px 20px rgba(24, 24, 27, 0.14);
}

.np-root.is-light-stage .np-pl-mini-artist {
  color: var(--np-ink-2);
}

.np-root.is-light-stage .np-pl-title {
  color: var(--np-ink-1);
}

.np-root.is-light-stage .np-pl-sep {
  color: var(--np-ink-3);
}

.np-root.is-light-stage .np-pl-artist {
  color: var(--np-ink-2);
}

.np-root.is-light-stage .np-lyric-empty-state {
  color: var(--np-ink-2);
}

.np-root.is-light-stage .np-lyric-empty-orb {
  background: color-mix(in oklab, var(--np-accent-primary) 12%, transparent);
  color: var(--np-accent-primary);
}

.np-root.is-light-stage .np-lyric-empty-title {
  color: var(--np-ink-1);
}

.np-root.is-light-stage .np-lyric-empty-sub {
  color: var(--np-ink-3);
}

@media (max-width: 1120px) {
  .np-cover-side-actions {
    left: calc(100% + 14px);
  }
}

@media (max-width: 980px) {
  .np-main-workspace {
    grid-template-columns: 48% 52%;
    padding: 58px 20px 20px;
  }
  .np-left-stage {
    width: clamp(310px, 42vw, 440px);
  }
  .np-lyric-scroll-viewport {
    padding: 34vh 28px 40vh 28px;
  }
}

@media (max-height: 800px) {
  .np-left-stage {
    gap: 14px;
    padding-top: 4px;
    padding-bottom: 6px;
  }
  .np-cover-hero-box {
    margin-top: 0;
  }
  .np-console-stack {
    margin-top: 0;
    gap: 12px;
  }
  .np-studio-console-deck {
    padding: 4px 2px 2px;
    gap: 10px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .np-lyric-line-item,
  .np-hero-play-btn,
  .np-transport-aux-btn,
  .np-transport-skip-btn {
    transition: none !important;
  }
  .np-timeline-rail.is-playing .np-comet-plume,
  .np-timeline-rail.is-playing .np-comet-tail-stream {
    animation: none !important;
  }
}
</style>

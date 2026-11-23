<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import type {
  AudioQualityValue,
  ConceptAudioQualityValue,
  LyricAlignment,
  LyricFontFamily,
  LyricFontWeightLevel,
  LyricHighlightThemeId,
  LyricUnsungPresetId,
  ReverbPresetId,
  Song,
  SongAvailableQualityItem,
  ViperEffectItem,
} from "../types";
import {
  FALLBACK_VIPER_PRESETS,
  REVERB_ROOM_MODELS,
} from "../utils/audioDsp";
import {
  hexWithAlpha,
  lerpHex,
  lerpOklabHex,
  tuneAccentForDarkStage,
} from "../utils/coverPalette";
import {
  LYRIC_FONT_FAMILY_OPTIONS,
  LYRIC_FONT_WEIGHT_OPTIONS,
  LYRIC_SUNG_PRESETS,
  LYRIC_UNSUNG_PRESETS,
  resolveLyricTypographyStyle,
  resolveSungLyricPalette,
  resolveUnsungLyricHex,
} from "../utils/lyricTypography";
import { useFloatingModalTheme } from "../composables/useFloatingModalTheme";
import {
  getSongAvailableQualities,
  normalizeConceptQuality,
  splitSongAuthors,
} from "../api/kugou";
import { buildAudioDetailModel } from "../utils/audioDetailModel";
import PlumpIcon from "./PlumpIcon.vue";
import SongDetailGrid from "./SongDetailGrid.vue";

const props = defineProps<{
  showLyricSettings: boolean;
  showMoreSheet: boolean;
  showQualityDialog: boolean;
  showAudioEffectDialog?: boolean;
}>();

const emit = defineEmits<{
  (e: "update:showLyricSettings", val: boolean): void;
  (e: "update:showMoreSheet", val: boolean): void;
  (e: "update:showQualityDialog", val: boolean): void;
  (e: "update:showAudioEffectDialog", val: boolean): void;
}>();

const store = useAppStore();

// 四套主题 + 动态封面双主色与可读前景自适应引擎（同步支持浅/深主题与播放页开关取色）
const {
  isLightSurface,
  palette,
  semanticRoles,
  accentPrimary,
  accentSecondary,
  onAccentPrimary,
  onAccentSecondary,
  vipGold,
  modalSurfaceStyle,
  activeTileStyle,
  rangeProgressStyle,
} = useFloatingModalTheme("auto");

const readablePrimary = accentPrimary;
const readableSecondary = accentSecondary;
const readableLiveAccent = computed(() =>
  isLightSurface.value
    ? accentPrimary.value
    : tuneAccentForDarkStage(
        palette.value.lyricActiveHex ||
          palette.value.secondaryAccent ||
          accentPrimary.value,
        {
          surfaceHex: palette.value.bgSecondary || "#0A0E17",
          minWcagContrast: 4.15,
          maxLuminance: 0.88,
          preserveChroma: true,
        }
      )
);

const floatingWindowStyle = computed(() => {
  return {
    ...modalSurfaceStyle.value,
    "--np-primary": readablePrimary.value,
    "--np-secondary": readableSecondary.value,
    "--np-tertiary": palette.value.tertiaryAccent || readableSecondary.value,
    "--np-accent-midpoint": lerpOklabHex(
      readablePrimary.value,
      readableSecondary.value,
      0.5
    ),
    "--np-live-accent": readableLiveAccent.value,
    "--np-live-soft": hexWithAlpha(
      readableLiveAccent.value,
      isLightSurface.value ? 0.14 : 0.16
    ),
    "--np-live-glow": hexWithAlpha(readableLiveAccent.value, 0.24),
    "--np-on-primary": onAccentPrimary.value,
    "--np-on-secondary": onAccentSecondary.value,
    "--np-primary-soft": hexWithAlpha(
      readablePrimary.value,
      isLightSurface.value ? 0.14 : 0.16
    ),
    "--np-primary-glow": hexWithAlpha(readablePrimary.value, 0.24),
    "--np-secondary-soft": hexWithAlpha(
      readableSecondary.value,
      isLightSurface.value ? 0.14 : 0.15
    ),
    "--np-secondary-glow": hexWithAlpha(readableSecondary.value, 0.24),
    "--np-vip-gold": vipGold.value,
    "--np-vip-soft": hexWithAlpha(
      vipGold.value,
      isLightSurface.value ? 0.16 : 0.22
    ),
    "--np-danger": isLightSurface.value ? "#C81E45" : "#FF6B8B",
    "--np-danger-soft": isLightSurface.value
      ? "rgba(200, 30, 69, 0.13)"
      : "rgba(255, 77, 109, 0.2)",
  };
});

// ================= 1. 播放页歌词设置中心 (LyricSettingsWindow · 纯粹播放页专属 · 3 页签 + 顶部固定实时预览) =================
const lyricSubTab = ref<"style" | "layout" | "sync">("style");

const sungPresetThemes = computed<
  Array<{
    id: Exclude<LyricHighlightThemeId, "custom">;
    label: string;
    primaryHex: string;
    secondaryHex: string;
  }>
>(() => {
  const isLightStage = store.isNowPlayingLightStage;
  const dynPrimary = isLightStage
    ? palette.value.lyricActiveHex || accentPrimary.value
    : tuneAccentForDarkStage(
        palette.value.lyricActiveHex ||
          palette.value.dominantAccent ||
          accentPrimary.value,
        {
          surfaceHex: palette.value.bgPrimary || "#0A0E17",
          minWcagContrast: 4.35,
          maxLuminance: 0.88,
          preserveChroma: true,
        }
      );
  const dynSecondary = isLightStage
    ? palette.value.lyricSecondaryHex || accentSecondary.value
    : tuneAccentForDarkStage(
        palette.value.lyricSecondaryHex ||
          palette.value.secondaryAccent ||
          "#38BDF8",
        {
          surfaceHex: palette.value.bgSecondary || "#0A0E17",
          minWcagContrast: 4.15,
          maxLuminance: 0.88,
          preserveChroma: true,
        }
      );

  return LYRIC_SUNG_PRESETS.map((p) => {
    if (p.isDynamic) {
      return {
        id: p.id,
        label: store.dynamicCoverAccentEnabled ? "封面流光" : "主题流光",
        primaryHex: dynPrimary,
        secondaryHex: dynSecondary,
      };
    }
    return {
      id: p.id,
      label: p.label,
      primaryHex: isLightStage ? p.lightPrimary : p.darkPrimary,
      secondaryHex: isLightStage ? p.lightSecondary : p.darkSecondary,
    };
  });
});

const unsungPresetItems = computed(() =>
  LYRIC_UNSUNG_PRESETS.map((item) => ({
    id: item.id,
    label: item.label,
    swatch:
      item.id === "tint_soft"
        ? resolvedPreviewSung.value.primary
        : store.isNowPlayingLightStage
        ? item.swatchLight
        : item.swatchDark,
  }))
);

const alignmentOptions: Array<{ id: LyricAlignment; label: string }> = [
  { id: "left", label: "居左" },
  { id: "center", label: "居中" },
  { id: "right", label: "居右" },
];

const fontFamilyOptions = LYRIC_FONT_FAMILY_OPTIONS;
const fontWeightOptions = LYRIC_FONT_WEIGHT_OPTIONS;

const offsetSecText = computed(() => {
  const sec = store.lyricOffsetMs / 1000;
  if (store.lyricOffsetMs === 0) return "0.0s 准时";
  const sign = sec > 0 ? "+" : "";
  return `${sign}${sec.toFixed(1)}s`;
});

function adjustLyricOffset(deltaMs: number) {
  store.lyricOffsetMs = Math.max(
    -5000,
    Math.min(5000, store.lyricOffsetMs + deltaMs)
  );
}

const autoReturnOptions: Array<{ sec: 2 | 3 | 5; label: string }> = [
  { sec: 2, label: "2 秒" },
  { sec: 3, label: "3 秒" },
  { sec: 5, label: "5 秒" },
];

const lyricHeaderSubtitle = computed(() => {
  if (lyricSubTab.value === "style") {
    return "双通道独立配色、字形与四档字重";
  }
  if (lyricSubTab.value === "layout") {
    return "字号字距、焦点倍率与景深动效";
  }
  return "双档时间校准与在线词源检索";
});

function handleResetActiveLyricTab() {
  if (lyricSubTab.value === "sync") {
    store.lyricOffsetMs = 0;
    store.bluetoothDelayMs = 0;
    store.notify("已归零歌词时间偏移与蓝牙延迟补偿");
  } else {
    store.resetLyricSettingsToDefault();
    store.notify("已恢复播放页默认歌词配色、排版与动效");
  }
}

function handlePickCustomSungPrimary(hex: string) {
  store.lyricCustomPrimaryHex = hex;
  store.lyricTheme = "custom";
}

function handlePickCustomSungSecondary(hex: string) {
  store.lyricCustomSecondaryHex = hex;
  store.lyricSungMode = "gradient";
  store.lyricTheme = "custom";
}

function handlePickCustomUnsung(hex: string) {
  store.lyricUnsungCustomHex = hex;
  store.lyricUnsungPreset = "custom";
}

const resolvedPreviewSung = computed(() => {
  const dyn = sungPresetThemes.value[0];
  return resolveSungLyricPalette({
    themeId: store.lyricTheme,
    sungMode: store.lyricSungMode,
    customPrimary: store.lyricCustomPrimaryHex,
    customSecondary: store.lyricCustomSecondaryHex,
    isLightStage: store.isNowPlayingLightStage,
    dynamicPrimary: dyn.primaryHex,
    dynamicSecondary: dyn.secondaryHex,
  });
});

const activeSungLabel = computed(() => {
  if (store.lyricTheme === "custom") {
    return store.lyricSungMode === "solid" ? "自定义纯色" : "自定义渐变";
  }
  const found = sungPresetThemes.value.find((t) => t.id === store.lyricTheme);
  const modeTag = store.lyricSungMode === "solid" ? " · 纯色" : "";
  return `${found?.label || "封面流光"}${modeTag}`;
});

const activeUnsungLabel = computed(() => {
  if (store.lyricUnsungPreset === "custom") return "自定义未唱色";
  return (
    LYRIC_UNSUNG_PRESETS.find((u) => u.id === store.lyricUnsungPreset)?.label ||
    "智能随境"
  );
});

// 实时预览样本：优先读取当前正在播放歌曲的真实歌词，无歌词时回落经典示例
const livePreviewSample = computed(() => {
  const lines = store.currentLyric?.lines || [];
  if (lines.length > 0) {
    const pos =
      store.currentPositionMs +
      (store.lyricOffsetMs || 0) -
      (store.bluetoothDelayMs || 0);
    let idx = 0;
    for (let i = lines.length - 1; i >= 0; i--) {
      if (pos >= lines[i].startTimeMs) {
        idx = i;
        break;
      }
    }
    const cur = lines[idx] || lines[0];
    const next = lines[idx + 1] || lines[Math.max(0, idx - 1)];
    return {
      main: (cur.text || "").trim() || "夜空中最亮的星 能否听清",
      trans:
        (cur.translation || "").trim() ||
        "Can you hear the brightest star in the night sky",
      next: (next?.text || "").trim() || "那仰望的人 心底的孤独和叹息",
      isRealSong: true,
    };
  }
  return {
    main: "夜空中最亮的星 能否听清",
    trans: "Can you hear the brightest star in the night sky",
    next: "那仰望的人 心底的孤独和叹息",
    isRealSong: false,
  };
});

const lyricPreviewColors = computed(() => {
  const sung = resolvedPreviewSung.value;
  const primary = sung.primary;
  const secondary = store.lyricSungMode === "solid" ? primary : sung.secondary;
  const glowRatio = Math.max(0, Math.min(2, (store.lyricGlowIntensity ?? 100) / 100));
  const isLightStage = store.isNowPlayingLightStage;
  const baseInk = isLightStage ? palette.value.inkPrimary || "#182230" : "#FFFFFF";
  const stageBgHex = isLightStage
    ? palette.value.bgPrimary || "#F2F4F8"
    : palette.value.bgAbyss || "#060A14";
  const unsungHex = resolveUnsungLyricHex({
    preset: store.lyricUnsungPreset,
    customHex: store.lyricUnsungCustomHex,
    isLightStage,
    sungPrimaryHex: primary,
    stageInkHex: palette.value.inkPrimary,
  });
  const baseOp = Math.max(0.18, Math.min(0.85, store.lyricInactiveOpacity || 0.36));
  const tintOn = store.lyricTranslationTintEnabled !== false;
  const typo = resolveLyricTypographyStyle({
    family: store.lyricFontFamily,
    weight: store.lyricFontWeight || 760,
    customFontFamily: store.stageLyricFontOverride.familyStack,
    customFontWeight: store.stageLyricFontOverride.weightOverride,
    isGradientText: true,
    strokeTintHex: primary,
  });

  return {
    primary,
    secondary,
    unsungHex,
    translationColor: tintOn
      ? hexWithAlpha(lerpHex(primary, baseInk, 0.34), 0.9)
      : hexWithAlpha(unsungHex, 0.78),
    inactiveRgba: hexWithAlpha(unsungHex, Math.min(0.92, baseOp + 0.16)),
    nextColor: hexWithAlpha(unsungHex, Math.min(0.95, baseOp + 0.42)),
    stageBgHex,
    stageBg: isLightStage
      ? hexWithAlpha(stageBgHex, 0.96)
      : hexWithAlpha(stageBgHex, 0.78),
    glowShadow:
      glowRatio > 0.02
        ? isLightStage
          ? `0 2px ${Math.round(8 * glowRatio)}px ${hexWithAlpha(primary, Math.min(0.32, 0.16 * glowRatio))}`
          : `0 2px ${Math.round(12 * glowRatio)}px ${hexWithAlpha(primary, Math.min(0.68, 0.36 * glowRatio))}`
        : "none",
    fontFamily: typo.fontFamily,
    fontWeight: typo.fontWeight,
    opticalStrokeCss: typo.opticalStrokeCss,
  };
});

const activeDownloadTaskInModal = computed(() =>
  store.downloadQueue.find(
    (t) =>
      t.status === "resolving" ||
      t.status === "downloading" ||
      t.status === "tagging" ||
      t.status === "queued"
  ) || null
);

const customLyricSearchKw = ref("");
function triggerSearchLyricCandidates() {
  store.searchLyricCandidatesForCurrentSong(
    customLyricSearchKw.value.trim() || undefined
  );
}

// ================= 2. 播放更多设置中心 (支持内联二级子视图无缝切换) =================
type MoreSubView = "home" | "speed_pitch" | "sleep" | "detail";
const moreSubView = ref<MoreSubView>("home");

watch(
  () => props.showMoreSheet,
  (open) => {
    if (open) moreSubView.value = "home";
  }
);

const currentAuthors = computed(() => {
  if (!store.currentSong) return [];
  return splitSongAuthors(store.currentSong);
});

const artistEntrySubtitle = computed(() => {
  const count = currentAuthors.value.length;
  if (count > 1) {
    return `${store.currentSong?.artist || "群星"} (${count}位)`;
  }
  return store.currentSong?.artist || "查看歌手代表作";
});

const albumEntrySubtitle = computed(() => {
  const alb = (store.currentSong?.album || "").trim();
  return alb ? `《${alb}》` : "查看所属专辑详情";
});

const currentQualityLabel = computed(() => {
  const pb = store.currentPlaybackInfo;
  if (pb?.qualityBadge) return pb.qualityBadge;
  const map: Record<ConceptAudioQualityValue, string> = {
    high: "Hi-Res",
    flac: "SQ 无损",
    "320": "HQ 高品",
    "128": "标准",
  };
  return map[normalizeConceptQuality(store.preferredQuality)] || "Hi-Res";
});

const speedPitchSummary = computed(() => {
  const spd = `${store.playbackRate.toFixed(2)}x`;
  if (store.pitchSemitones === 0) return `${spd} 原调`;
  const sign = store.pitchSemitones > 0 ? "+" : "";
  return `${spd} · ${sign}${store.pitchSemitones}半音`;
});

const sleepTimerSummary = computed(() => {
  if (store.sleepTimerMinutes === -1) return "播完本首停";
  if (store.sleepRemainingSeconds > 0) {
    const m = Math.ceil(store.sleepRemainingSeconds / 60);
    return `剩余 ${m} 分钟`;
  }
  return "未开启";
});

// 更多控制中枢：随当前视图与内容条数平滑自适应宽高（杜绝二级页底部空洞与尺寸突跳）
const moreHubWindowStyle = computed(() => {
  const base = floatingWindowStyle.value;
  if (moreSubView.value === "speed_pitch") {
    return {
      ...base,
      width: "min(536px, 92vw)",
      height: "min(454px, 84vh)",
    };
  }
  if (moreSubView.value === "sleep") {
    const hasActiveSleep =
      store.sleepTimerMinutes !== 0 || store.sleepRemainingSeconds > 0;
    return {
      ...base,
      width: "min(520px, 92vw)",
      height: hasActiveSleep ? "min(378px, 84vh)" : "min(330px, 84vh)",
    };
  }
  if (moreSubView.value === "detail") {
    return {
      ...base,
      width: "min(600px, 94vw)",
      height: "min(576px, 86vh)",
    };
  }
  return {
    ...base,
    width: "min(560px, 94vw)",
    height: store.downloadFeatureUnlocked ? "min(452px, 84vh)" : "min(388px, 84vh)",
  };
});

function handleJumpArtist() {
  if (!store.currentSong) return;
  emit("update:showMoreSheet", false);
  store.navigateToSongArtist(store.currentSong);
}

function handleJumpAlbum() {
  if (!store.currentSong) return;
  emit("update:showMoreSheet", false);
  store.navigateToSongAlbum(store.currentSong);
}

function handleToggleClimax() {
  store.climaxModeEnabled = !store.climaxModeEnabled;
  if (
    store.climaxModeEnabled &&
    store.currentPlaybackInfo &&
    store.currentPlaybackInfo.climaxStartMs > 0
  ) {
    store.seekToMs(store.currentPlaybackInfo.climaxStartMs);
  }
  store.notify(
    store.climaxModeEnabled
      ? "已开启「副歌速刷」模式：自动直达高潮精华段落"
      : "已关闭副歌速刷，恢复整曲完整播放"
  );
}

// ================= 3. 音质选择与规格（概念版最高 Hi-Res · 按歌曲真实可用音质动态过滤） =================
const qualityCatalog = computed<SongAvailableQualityItem[]>(
  () => store.currentSongAvailableQualities || []
);

const downloadQualityCatalog = computed<SongAvailableQualityItem[]>(() =>
  getSongAvailableQualities(store.downloadModalSong)
);

const normalizedPreferredQuality = computed<ConceptAudioQualityValue>(() =>
  normalizeConceptQuality(store.preferredQuality)
);

const activeActualQuality = computed<ConceptAudioQualityValue>(() => {
  const pb = store.currentPlaybackInfo;
  if (pb?.actualQuality || pb?.matchedQuality) {
    return normalizeConceptQuality(pb.actualQuality || pb.matchedQuality);
  }
  return normalizedPreferredQuality.value;
});

const normalizedDownloadQuality = computed<ConceptAudioQualityValue>(() =>
  normalizeConceptQuality(store.downloadQuality)
);

function selectQuality(q: AudioQualityValue) {
  store.switchAudioQuality(normalizeConceptQuality(q));
  emit("update:showQualityDialog", false);
}

// ================= 4. 倍速与升降调 =================
const speedPresets = [0.75, 1.0, 1.25, 1.5, 2.0];

const pitchMultiplier = computed({
  get: () => {
    if (store.linkPitchWithSpeed) {
      return Number(store.playbackRate.toFixed(2));
    }
    return Number(Math.pow(2, store.pitchSemitones / 12).toFixed(2));
  },
  set: (mult: number) => {
    const clamped = Math.max(0.5, Math.min(2.0, mult));
    const semi = Math.round(12 * Math.log2(clamped));
    store.setPitchSemitones(semi);
  },
});

function handleSpeedInput(val: number) {
  const clamped = Math.max(0.5, Math.min(2.0, Number(val.toFixed(2))));
  store.setPlaybackSpeed(clamped);
}

function resetSpeedAndPitch() {
  store.linkPitchWithSpeed = false;
  store.setPlaybackSpeed(1.0);
  store.setPitchSemitones(0);
}

// ================= 5. 睡眠定时 =================
const sleepPresets = [
  { mins: 15, label: "15 分钟" },
  { mins: 30, label: "30 分钟" },
  { mins: 45, label: "45 分钟" },
  { mins: 60, label: "60 分钟" },
  { mins: 90, label: "90 分钟" },
  { mins: -1, label: "播完本首" },
];

function chooseSleepPreset(mins: number) {
  store.startSleepTimer(mins);
  if (moreSubView.value === "sleep") {
    moreSubView.value = "home";
  }
}

// ================= 6. 详情模型映射 (添加到歌单已统一抽离至 AddToPlaylistModal.vue) =================

const moreAudioDetail = computed(() => {
  const cur = store.currentSong;
  const modalTarget = store.songDetailModalSong;
  const isModalSameAsCurrent =
    Boolean(cur && modalTarget) &&
    ((cur?.hash && cur.hash === modalTarget?.hash) ||
      (cur?.id && cur.id === modalTarget?.id));
  const detail =
    store.currentSongDetailInfo ||
    (isModalSameAsCurrent ? store.activeSongDetailInfo : null);
  return buildAudioDetailModel(
    cur,
    detail,
    store.currentPlaybackInfo,
    currentQualityLabel.value
  );
});

const modalAudioDetail = computed(() => {
  const target = store.songDetailModalSong;
  const isCurrent =
    Boolean(target && store.currentSong) &&
    ((target?.hash && target.hash === store.currentSong?.hash) ||
      (target?.id && target.id === store.currentSong?.id));
  const detail =
    store.activeSongDetailInfo ||
    (isCurrent ? store.currentSongDetailInfo : null);
  return buildAudioDetailModel(
    target,
    detail,
    isCurrent ? store.currentPlaybackInfo : null,
    currentQualityLabel.value
  );
});

// ================= 7. 蝰蛇声学中心 (Viper Acoustic Studio · 极简双视图重构) =================
type AudioStudioTab = "presets" | "tuning";
const audioStudioTab = ref<AudioStudioTab>("presets");
const effectCategoryTab = ref<string>("全部");
const effectSearchKeyword = ref<string>("");

const effectCategories = computed(() => {
  const base = ["全部", "官方旗舰", "3D空间", "曲风人声", "复古介质", "设备车载"];
  const hasCloudLab = (store.viperEffects || []).some(
    (item) => item.category === "云端实验室"
  );
  return hasCloudLab ? [...base, "云端实验室"] : base;
});

const categoryCounts = computed<Record<string, number>>(() => {
  const list = store.viperEffects || [];
  const counts: Record<string, number> = { 全部: list.length };
  for (const item of list) {
    const cat = item.category || "官方旗舰";
    counts[cat] = (counts[cat] || 0) + 1;
  }
  return counts;
});

const filteredViperEffects = computed(() => {
  const list = store.viperEffects || [];
  const cat = effectCategoryTab.value;
  const kw = effectSearchKeyword.value.trim().toLowerCase();
  return list.filter((item) => {
    const matchCat =
      cat === "全部" ||
      (item.category && item.category.includes(cat)) ||
      (cat === "官方旗舰" && !item.category);
    if (!matchCat) return false;
    if (!kw) return true;
    const title = (item.label || item.name || "").toLowerCase();
    return (
      title.includes(kw) ||
      (item.desc && item.desc.toLowerCase().includes(kw)) ||
      (item.authorName && item.authorName.toLowerCase().includes(kw)) ||
      (item.acousticTags &&
        item.acousticTags.some((t) => t.toLowerCase().includes(kw)))
    );
  });
});

const activeViperObj = computed(() =>
  (store.viperEffects || []).find((i) => i.id === store.activeViperEffectId)
);

const isAnyAcousticEffectModified = computed(() => {
  return (
    Boolean(activeViperObj.value) ||
    store.acousticField.enabled ||
    (store.eqGains || []).some((g) => Math.abs(g) > 0.05) ||
    Boolean(store.activeCloudStreamEffect)
  );
});

const activeAcousticSummaryText = computed(() => {
  if (!store.masterEffectEnabled) {
    return "原声直通 · DSP 已旁路";
  }
  if (activeViperObj.value) {
    const name = activeViperObj.value.label || activeViperObj.value.name || "";
    const room = REVERB_ROOM_MODELS.find(
      (r) => r.id === store.acousticField.reverbPreset
    );
    const roomText =
      room && room.id !== "none" && store.acousticField.reverbWetPercent > 0
        ? ` · ${room.label}`
        : "";
    return `${name}${roomText}`;
  }
  if (store.acousticField.enabled) {
    return "自定义听感微调";
  }
  if (store.activeEqPreset && store.activeEqPreset !== "flat") {
    return `调音台 · ${store.activeEqPreset.toUpperCase()}`;
  }
  return "原声直通 · 0dB";
});

// 迷你频响样条轮廓 SVG 路径生成器（供音效卡片精致呈现频响气质）
function buildMiniCurvePath(gains?: number[]): string {
  if (!gains || gains.length < 2) return "M 2 14 L 70 14";
  const w = 72;
  const h = 24;
  const padX = 3;
  const usableW = w - padX * 2;
  const pts = gains.map((g, idx) => {
    const x = padX + (idx / (gains.length - 1)) * usableW;
    const clamped = Math.max(-9, Math.min(9, Number(g) || 0));
    const y = h / 2 - (clamped / 9) * (h * 0.38);
    return { x: Number(x.toFixed(1)), y: Number(y.toFixed(1)) };
  });
  let d = `M ${pts[0].x} ${pts[0].y}`;
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[i];
    const p1 = pts[i + 1];
    const cx = Number(((p0.x + p1.x) / 2).toFixed(1));
    d += ` C ${cx} ${p0.y}, ${cx} ${p1.y}, ${p1.x} ${p1.y}`;
  }
  return d;
}

function selectViperPresetFromDialog(effectId: string) {
  store.applyViperEffect(effectId || "off");
}

function resetAllAcousticToBitPerfect() {
  store.resetAcousticAndEqToDefault();
  if (store.activeCloudStreamEffect) {
    store.switchCloudStreamEffect(store.activeCloudStreamEffect);
  }
}

function restoreCurrentPresetDefaults() {
  const cur =
    activeViperObj.value ||
    FALLBACK_VIPER_PRESETS.find((v) => v.id === store.activeViperEffectId);
  if (!cur) {
    store.resetAcousticAndEqToDefault();
    return;
  }
  // 先置空再重新挂载，确保完整恢复出厂声学参数并激活
  store.activeViperEffectId = "off";
  store.applyViperEffect(cur);
  store.notify(`已恢复「${cur.label || cur.name}」默认声学参数`);
}

// 听感微调延迟持久化防抖（拖拽期间 persist=false 保证 WebAudio 实时 0 延迟响应且不掉帧）
let dspPersistDebounceTimer: number | null = null;
function scheduleDspPersist() {
  if (dspPersistDebounceTimer !== null) {
    window.clearTimeout(dspPersistDebounceTimer);
  }
  dspPersistDebounceTimer = window.setTimeout(() => {
    store.syncDspChain(true);
    dspPersistDebounceTimer = null;
  }, 220);
}

type SpatialNumericKey =
  | "subBassDepth"
  | "bassPunch"
  | "surroundWidth"
  | "stageDepth"
  | "vocalClarity"
  | "deEsserLevel"
  | "airHarmonics"
  | "vinylTubeWarmth"
  | "transientPunch"
  | "crossfeedLevel"
  | "reverbWetPercent"
  | "surroundRotationSpeed"
  | "loudnessBoost";

function updateSpatialFieldParam(
  key: SpatialNumericKey,
  val: number,
  persistNow = false
) {
  store.masterEffectEnabled = true;
  store.peqBypass = false;
  store.acousticField.enabled = true;

  if (key === "loudnessBoost") {
    store.acousticField.loudnessBoost = Math.max(
      0,
      Math.min(10, Number(val.toFixed(1)))
    );
  } else if (key === "reverbWetPercent") {
    const clampedWet = Math.max(0, Math.min(80, Math.round(val)));
    store.acousticField.reverbWetPercent = clampedWet;
    if (clampedWet > 0 && store.acousticField.reverbPreset === "none") {
      store.acousticField.reverbPreset = "small_room";
    }
  } else {
    store.acousticField[key] = Math.max(0, Math.min(100, Math.round(val)));
    if (key === "surroundRotationSpeed") {
      store.acousticField.surroundRotation = val > 0;
    }
    if (key === "crossfeedLevel") {
      store.acousticField.bauerCrossfeed = val >= 15;
    }
  }

  store.syncDspChain(persistNow);
  if (!persistNow) {
    scheduleDspPersist();
  }
}

function selectReverbRoomModel(presetId: ReverbPresetId) {
  store.masterEffectEnabled = true;
  store.peqBypass = false;
  store.acousticField.enabled = true;
  store.acousticField.reverbPreset = presetId;
  if (presetId === "none") {
    store.acousticField.reverbWetPercent = 0;
  } else if (store.acousticField.reverbWetPercent < 12) {
    store.acousticField.reverbWetPercent = 24;
  }
  const room = REVERB_ROOM_MODELS.find((r) => r.id === presetId);
  if (room) {
    store.acousticField.reverbDamping = room.defaultDamping;
  }
  store.syncDspChain(true);
}

// ---------- 极简直观四维听感微调滑块（复杂参量 EQ 交由专业调音台） ----------
const coreTuningSliders = computed<
  Array<{
    key: SpatialNumericKey;
    label: string;
    enLabel: string;
    desc: string;
    min: number;
    max: number;
    step: number;
    unit: string;
    value: number;
  }>
>(() => [
  {
    key: "subBassDepth",
    label: "低频下潜",
    enLabel: "SUB-BASS",
    desc: "增强超重低频深度与氛围弹性",
    min: 0,
    max: 100,
    step: 1,
    unit: "%",
    value: Number(store.acousticField.subBassDepth ?? 0),
  },
  {
    key: "vocalClarity",
    label: "人声清透",
    enLabel: "VOCAL",
    desc: "提升人声前置感与唇齿清晰度",
    min: 0,
    max: 100,
    step: 1,
    unit: "%",
    value: Number(store.acousticField.vocalClarity ?? 0),
  },
  {
    key: "surroundWidth",
    label: "空间宽广",
    enLabel: "SPATIAL",
    desc: "展宽立体声横向包裹感与临场宽度",
    min: 0,
    max: 100,
    step: 1,
    unit: "%",
    value: Number(store.acousticField.surroundWidth ?? 0),
  },
  {
    key: "reverbWetPercent",
    label: "空间混响度",
    enLabel: "REVERB",
    desc: "柔和拓展演出厅与厅堂自然残响",
    min: 0,
    max: 80,
    step: 1,
    unit: "%",
    value: Number(store.acousticField.reverbWetPercent ?? 0),
  },
]);

onUnmounted(() => {
  if (dspPersistDebounceTimer !== null) {
    window.clearTimeout(dspPersistDebounceTimer);
  }
});

const isViperStudioVisible = computed(
  () => Boolean(props.showAudioEffectDialog || store.isViperStudioModalOpen)
);

function closeViperStudio() {
  emit("update:showAudioEffectDialog", false);
  store.isViperStudioModalOpen = false;
}

function handleJumpToTuner() {
  closeViperStudio();
  store.openSoundMixerFromPlayer();
}
</script>

<template>
  <!-- =========================================================================
       1. 播放页歌词设置中心 (LyricSettingsWindow · 顶部固定预览 + 3 页签纯粹分区)
       ========================================================================= -->
  <transition name="np-float-spring">
    <div
      v-if="props.showLyricSettings"
      class="np-floating-layer anchor-bottom-right"
      @click.self="emit('update:showLyricSettings', false)"
    >
      <div
        class="np-floating-window w-lyric-studio"
        :class="{ 'is-dark-surface': !isLightSurface }"
        :style="floatingWindowStyle"
        @click.stop
      >
        <!-- 顶部栏 -->
        <div class="nfw-header">
          <div class="nfw-title-row">
            <div class="nfw-icon-orb">
              <PlumpIcon
                name="np-lyric-sliders"
                :size="18"
                :color="readablePrimary"
              />
            </div>
            <div>
              <h3 class="nfw-title">播放页歌词设置</h3>
              <p class="nfw-sub">{{ lyricHeaderSubtitle }}</p>
            </div>
          </div>

          <div class="nfw-header-actions">
            <div class="nfw-pill-switcher" aria-label="歌词设置分页">
              <button
                type="button"
                class="nfw-pill-tab"
                :class="{ active: lyricSubTab === 'style' }"
                :aria-pressed="lyricSubTab === 'style'"
                @click="lyricSubTab = 'style'"
              >
                配色与字形
              </button>
              <button
                type="button"
                class="nfw-pill-tab"
                :class="{ active: lyricSubTab === 'layout' }"
                :aria-pressed="lyricSubTab === 'layout'"
                @click="lyricSubTab = 'layout'"
              >
                排版与动效
              </button>
              <button
                type="button"
                class="nfw-pill-tab"
                :class="{ active: lyricSubTab === 'sync' }"
                :aria-pressed="lyricSubTab === 'sync'"
                @click="lyricSubTab = 'sync'"
              >
                校准与词源
              </button>
            </div>

            <button
              type="button"
              class="nfw-icon-btn"
              :title="
                lyricSubTab === 'sync'
                  ? '归零歌词偏移与延迟'
                  : '恢复播放页默认歌词设置'
              "
              @click="handleResetActiveLyricTab"
            >
              <PlumpIcon name="refresh" :size="14" :color="readablePrimary" />
            </button>
            <button
              type="button"
              class="nfw-icon-btn"
              aria-label="关闭歌词设置"
              @click="emit('update:showLyricSettings', false)"
            >
              <PlumpIcon name="close" :size="14" />
            </button>
          </div>
        </div>

        <!-- 顶部常驻实时双行预览台（切页时始终保持所见即所得） -->
        <div
          class="nfw-lyric-preview-card"
          :style="{
            textAlign: store.lyricAlignment,
            fontFamily: lyricPreviewColors.fontFamily,
            background: lyricPreviewColors.stageBg,
          }"
        >
          <span class="nfw-preview-corner-tag">
            {{ livePreviewSample.isRealSong ? "当前曲目实时预览" : "效果实时预览" }}
          </span>
          <div
            class="nfw-lp-main"
            :style="{
              fontSize: `${Math.max(15, Math.min(25, Math.round(store.lyricFontSize * 0.66 * (store.lyricFocusScaleEnabled ? store.lyricFocusScaleRatio : 1))))}px`,
              fontWeight: lyricPreviewColors.fontWeight,
              letterSpacing: `${Number(store.lyricLetterSpacing || 0).toFixed(1)}px`,
              textShadow: lyricPreviewColors.glowShadow,
            }"
          >
            <span
              class="nfw-lp-sung"
              :style="{
                WebkitTextStroke: lyricPreviewColors.opticalStrokeCss,
                paintOrder: 'stroke fill',
                backgroundImage: store.lyricWordByWord
                  ? `linear-gradient(90deg, ${lyricPreviewColors.primary} 0%, ${lyricPreviewColors.secondary} 58%, ${lyricPreviewColors.inactiveRgba} 68%, ${lyricPreviewColors.inactiveRgba} 100%)`
                  : `linear-gradient(90deg, ${lyricPreviewColors.primary} 0%, ${lyricPreviewColors.secondary} 100%)`,
              }"
            >
              {{ livePreviewSample.main }}
            </span>
          </div>
          <div
            v-if="store.showTranslation"
            class="nfw-lp-trans"
            :style="{
              fontSize: `${Math.max(11, Math.round(Math.min(22, store.lyricFontSize * 0.62) * store.lyricTranslationScale))}px`,
              color: lyricPreviewColors.translationColor,
            }"
          >
            {{ livePreviewSample.trans }}
          </div>
          <div
            class="nfw-lp-next"
            :style="{
              color: lyricPreviewColors.nextColor,
              fontWeight: lyricPreviewColors.fontWeight,
              letterSpacing: `${Number(store.lyricLetterSpacing || 0).toFixed(1)}px`,
              filter: store.lyricBlurEnabled
                ? `blur(${Math.min(2.2, 0.9 * store.lyricBlurIntensity).toFixed(2)}px)`
                : 'none',
            }"
          >
            {{ livePreviewSample.next }}
          </div>
        </div>

        <!-- ======================= 子页 A：配色与字形（已唱/未唱双通道独立工作台） ======================= -->
        <div v-if="lyricSubTab === 'style'" class="nfw-lyric-body">
          <!-- 1. 已唱部分配色（8 款高定新预设 + 纯色/渐变切换 + 自定义双色取色器） -->
          <div class="nfw-section-block">
            <div class="nfw-block-head">
              <span>已唱部分配色</span>
              <div class="nfw-inline-ctrl-bar">
                <div class="nfw-mini-pills">
                  <button
                    type="button"
                    class="nfw-mini-pill"
                    :class="{ active: store.lyricSungMode === 'gradient' }"
                    @click="store.lyricSungMode = 'gradient'"
                  >
                    流光渐变
                  </button>
                  <button
                    type="button"
                    class="nfw-mini-pill"
                    :class="{ active: store.lyricSungMode === 'solid' }"
                    @click="store.lyricSungMode = 'solid'"
                  >
                    纯色极简
                  </button>
                </div>
                <span class="nfw-block-tag">{{ activeSungLabel }}</span>
              </div>
            </div>

            <div class="nfw-theme-grid">
              <button
                v-for="theme in sungPresetThemes"
                :key="theme.id"
                type="button"
                class="nfw-theme-swatch"
                :class="{ active: store.lyricTheme === theme.id }"
                :style="store.lyricTheme === theme.id ? activeTileStyle(true) : {}"
                @click="store.lyricTheme = theme.id"
              >
                <span
                  class="nfw-swatch-orb"
                  :style="{
                    background:
                      store.lyricSungMode === 'solid'
                        ? theme.primaryHex
                        : `linear-gradient(135deg, ${theme.primaryHex}, ${theme.secondaryHex})`,
                    boxShadow: `0 0 8px ${hexWithAlpha(theme.primaryHex, 0.38)}`,
                  }"
                ></span>
                <span class="nfw-swatch-name">{{ theme.label }}</span>
              </button>
            </div>

            <div class="nfw-color-custom-row">
              <div class="nfw-color-picker-group">
                <label
                  class="nfw-color-picker-chip"
                  :class="{ active: store.lyricTheme === 'custom' }"
                >
                  <input
                    type="color"
                    class="nfw-color-native-input"
                    aria-label="自定义已唱主色"
                    :value="store.lyricCustomPrimaryHex"
                    @input="
                      handlePickCustomSungPrimary(
                        ($event.target as HTMLInputElement).value
                      )
                    "
                  />
                  <span>{{ store.lyricSungMode === 'solid' ? '自定义纯色' : '自定义起色' }}</span>
                </label>

                <label
                  v-if="store.lyricSungMode === 'gradient'"
                  class="nfw-color-picker-chip"
                  :class="{ active: store.lyricTheme === 'custom' }"
                >
                  <input
                    type="color"
                    class="nfw-color-native-input"
                    aria-label="自定义已唱副色"
                    :value="store.lyricCustomSecondaryHex"
                    @input="
                      handlePickCustomSungSecondary(
                        ($event.target as HTMLInputElement).value
                      )
                    "
                  />
                  <span>自定义终色</span>
                </label>
              </div>
              <span class="nfw-slider-badge">点击圆点自由调色</span>
            </div>
          </div>

          <!-- 2. 未唱部分独立配色（6 款精选未唱色 + 自定义色 + 未唱透明度） -->
          <div class="nfw-section-block">
            <div class="nfw-block-head">
              <span>未唱部分配色</span>
              <span class="nfw-block-tag">
                {{ activeUnsungLabel }} · {{ Math.round(store.lyricInactiveOpacity * 100) }}%
              </span>
            </div>

            <div class="nfw-unsung-grid">
              <button
                v-for="u in unsungPresetItems"
                :key="u.id"
                type="button"
                class="nfw-theme-swatch"
                :class="{ active: store.lyricUnsungPreset === u.id }"
                :style="store.lyricUnsungPreset === u.id ? activeTileStyle(true) : {}"
                @click="store.lyricUnsungPreset = u.id as LyricUnsungPresetId"
              >
                <span
                  class="nfw-swatch-orb"
                  :style="{
                    background: u.swatch,
                    boxShadow: `0 0 6px ${hexWithAlpha(u.swatch, 0.35)}`,
                  }"
                ></span>
                <span class="nfw-swatch-name">{{ u.label }}</span>
              </button>
            </div>

            <div class="nfw-color-custom-row">
              <label
                class="nfw-color-picker-chip"
                :class="{ active: store.lyricUnsungPreset === 'custom' }"
              >
                <input
                  type="color"
                  class="nfw-color-native-input"
                  aria-label="自定义未唱颜色"
                  :value="store.lyricUnsungCustomHex"
                  @input="
                    handlePickCustomUnsung(
                      ($event.target as HTMLInputElement).value
                    )
                  "
                />
                <span>自定义未唱色</span>
              </label>

              <div class="nfw-slider-item" style="flex: 1; min-width: 190px">
                <span class="nfw-slider-name">未唱浓度</span>
                <input
                  v-model.number="store.lyricInactiveOpacity"
                  type="range"
                  min="0.18"
                  max="0.82"
                  step="0.02"
                  aria-label="未唱歌词透明度"
                  class="nfw-range nfw-range-fill"
                  :style="rangeProgressStyle(store.lyricInactiveOpacity, 0.18, 0.82)"
                />
                <span class="nfw-slider-badge">
                  {{ Math.round(store.lyricInactiveOpacity * 100) }}%
                </span>
              </div>
            </div>
          </div>

          <!-- 3. 字形、四档真实字重与对齐 -->
          <div class="nfw-section-block">
            <div class="nfw-block-head">
              <span>字体风格与四档字重</span>
              <span class="nfw-block-tag">Windows 实装阶梯</span>
            </div>

            <div class="nfw-font-grid-4col">
              <button
                v-for="font in fontFamilyOptions"
                :key="font.id"
                type="button"
                class="nfw-font-pill"
                :style="activeTileStyle(store.lyricFontFamily === font.id)"
                @click="store.lyricFontFamily = font.id"
              >
                <span class="nfw-font-label">{{ font.label }}</span>
                <span class="nfw-font-desc">{{ font.desc }}</span>
              </button>
            </div>

            <div class="nfw-inline-pair-row">
              <div class="nfw-inline-group">
                <span class="nfw-inline-label">字重</span>
                <div class="nfw-mini-pills">
                  <button
                    v-for="fw in fontWeightOptions"
                    :key="fw.id"
                    type="button"
                    class="nfw-mini-pill"
                    :class="{ active: store.lyricFontWeight === fw.id }"
                    @click="store.lyricFontWeight = fw.id"
                  >
                    {{ fw.label }}
                  </button>
                </div>
              </div>

              <div class="nfw-inline-group">
                <span class="nfw-inline-label">对齐</span>
                <div class="nfw-mini-pills">
                  <button
                    v-for="align in alignmentOptions"
                    :key="align.id"
                    type="button"
                    class="nfw-mini-pill"
                    :class="{ active: store.lyricAlignment === align.id }"
                    @click="store.lyricAlignment = align.id"
                  >
                    {{ align.label }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- ======================= 子页 B：排版、光学与动效开关 ======================= -->
        <div v-else-if="lyricSubTab === 'layout'" class="nfw-lyric-body">
          <!-- 1. 排版尺寸与光学质感 -->
          <div class="nfw-section-block">
            <div class="nfw-block-head">
              <span>排版尺寸与光学质感</span>
              <span class="nfw-block-tag">{{ Math.round(store.lyricFontSize) }}px</span>
            </div>

            <div class="nfw-sliders-2col">
              <div class="nfw-slider-item">
                <span class="nfw-slider-name">主词字号</span>
                <input
                  v-model.number="store.lyricFontSize"
                  type="range"
                  min="20"
                  max="48"
                  step="1"
                  aria-label="主词字号"
                  class="nfw-range nfw-range-fill"
                  :style="rangeProgressStyle(store.lyricFontSize, 20, 48)"
                />
                <span class="nfw-slider-badge">{{ Math.round(store.lyricFontSize) }}px</span>
              </div>

              <div class="nfw-slider-item">
                <span class="nfw-slider-name">译文比例</span>
                <input
                  v-model.number="store.lyricTranslationScale"
                  type="range"
                  min="0.65"
                  max="0.95"
                  step="0.01"
                  aria-label="译文比例"
                  class="nfw-range nfw-range-fill"
                  :style="rangeProgressStyle(store.lyricTranslationScale, 0.65, 0.95)"
                />
                <span class="nfw-slider-badge">
                  {{ Math.round(store.lyricTranslationScale * 100) }}%
                </span>
              </div>

              <div class="nfw-slider-item">
                <span class="nfw-slider-name">行间呼吸</span>
                <input
                  v-model.number="store.lyricLineSpacing"
                  type="range"
                  min="8"
                  max="28"
                  step="1"
                  aria-label="行间呼吸"
                  class="nfw-range nfw-range-fill"
                  :style="rangeProgressStyle(store.lyricLineSpacing, 8, 28)"
                />
                <span class="nfw-slider-badge">{{ Math.round(store.lyricLineSpacing) }}px</span>
              </div>

              <div class="nfw-slider-item">
                <span class="nfw-slider-name">字间微距</span>
                <input
                  v-model.number="store.lyricLetterSpacing"
                  type="range"
                  min="-1"
                  max="6"
                  step="0.5"
                  aria-label="字间距"
                  class="nfw-range nfw-range-fill"
                  :style="rangeProgressStyle(store.lyricLetterSpacing, -1, 6)"
                />
                <span class="nfw-slider-badge">
                  {{ Number(store.lyricLetterSpacing).toFixed(1) }}px
                </span>
              </div>

              <div
                class="nfw-slider-item"
                :class="{ 'is-disabled': !store.lyricFocusScaleEnabled }"
              >
                <span class="nfw-slider-name">焦点倍率</span>
                <input
                  v-model.number="store.lyricFocusScaleRatio"
                  type="range"
                  min="1.0"
                  max="1.24"
                  step="0.02"
                  :disabled="!store.lyricFocusScaleEnabled"
                  aria-label="焦点放大倍率"
                  class="nfw-range nfw-range-fill"
                  :style="rangeProgressStyle(store.lyricFocusScaleRatio, 1.0, 1.24)"
                />
                <span class="nfw-slider-badge">
                  {{ Number(store.lyricFocusScaleRatio).toFixed(2) }}x
                </span>
              </div>

              <div class="nfw-slider-item">
                <span class="nfw-slider-name">逐字辉光</span>
                <input
                  v-model.number="store.lyricGlowIntensity"
                  type="range"
                  min="0"
                  max="200"
                  step="5"
                  aria-label="逐字辉光"
                  class="nfw-range nfw-range-fill"
                  :style="rangeProgressStyle(store.lyricGlowIntensity, 0, 200)"
                />
                <span class="nfw-slider-badge">{{ Math.round(store.lyricGlowIntensity) }}%</span>
              </div>

              <div
                class="nfw-slider-item"
                :class="{ 'is-disabled': !store.lyricBlurEnabled }"
              >
                <span class="nfw-slider-name">景深模糊</span>
                <input
                  v-model.number="store.lyricBlurIntensity"
                  type="range"
                  min="0.4"
                  max="1.8"
                  step="0.05"
                  :disabled="!store.lyricBlurEnabled"
                  aria-label="景深强度"
                  class="nfw-range nfw-range-fill"
                  :style="rangeProgressStyle(store.lyricBlurIntensity, 0.4, 1.8)"
                />
                <span class="nfw-slider-badge">
                  {{ Number(store.lyricBlurIntensity).toFixed(1) }}x
                </span>
              </div>
            </div>
          </div>

          <!-- 2. 动效与交互开关矩阵 -->
          <div class="nfw-section-block">
            <div class="nfw-block-head">
              <span>视觉动效与交互开关</span>
              <span class="nfw-block-tag">实时联动</span>
            </div>

            <div class="nfw-switch-matrix">
              <button
                type="button"
                class="nfw-switch-tile"
                :class="{ on: store.lyricWordByWord }"
                @click="store.lyricWordByWord = !store.lyricWordByWord"
              >
                <div class="nfw-st-copy">
                  <span class="nfw-st-title">逐字染色</span>
                  <span class="nfw-st-desc">KRC 逐字流光推进</span>
                </div>
                <span class="nfw-switch-pill" :class="{ on: store.lyricWordByWord }">
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>

              <button
                type="button"
                class="nfw-switch-tile"
                :class="{
                  on: store.lyricWordByWord && store.lyricWordBounceEnabled,
                  'is-disabled': !store.lyricWordByWord,
                }"
                :disabled="!store.lyricWordByWord"
                @click="store.lyricWordBounceEnabled = !store.lyricWordBounceEnabled"
              >
                <div class="nfw-st-copy">
                  <span class="nfw-st-title">逐字弹跳</span>
                  <span class="nfw-st-desc">
                    {{ store.lyricWordByWord ? "在唱文字轻盈上浮" : "需先开启逐字染色" }}
                  </span>
                </div>
                <span
                  class="nfw-switch-pill"
                  :class="{ on: store.lyricWordByWord && store.lyricWordBounceEnabled }"
                >
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>

              <button
                type="button"
                class="nfw-switch-tile"
                :class="{ on: store.lyricFocusScaleEnabled }"
                @click="store.lyricFocusScaleEnabled = !store.lyricFocusScaleEnabled"
              >
                <div class="nfw-st-copy">
                  <span class="nfw-st-title">焦点放大</span>
                  <span class="nfw-st-desc">当前行突出缩放聚焦</span>
                </div>
                <span class="nfw-switch-pill" :class="{ on: store.lyricFocusScaleEnabled }">
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>

              <button
                type="button"
                class="nfw-switch-tile"
                :class="{ on: store.lyricBlurEnabled }"
                @click="store.lyricBlurEnabled = !store.lyricBlurEnabled"
              >
                <div class="nfw-st-copy">
                  <span class="nfw-st-title">歌词景深模糊</span>
                  <span class="nfw-st-desc">关闭则全部歌词锐利清晰</span>
                </div>
                <span class="nfw-switch-pill" :class="{ on: store.lyricBlurEnabled }">
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>

              <button
                type="button"
                class="nfw-switch-tile"
                :class="{ on: store.lyricTranslationTintEnabled }"
                @click="store.lyricTranslationTintEnabled = !store.lyricTranslationTintEnabled"
              >
                <div class="nfw-st-copy">
                  <span class="nfw-st-title">译文随色</span>
                  <span class="nfw-st-desc">关闭则跟随未唱墨色</span>
                </div>
                <span class="nfw-switch-pill" :class="{ on: store.lyricTranslationTintEnabled }">
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>

              <button
                type="button"
                class="nfw-switch-tile"
                :class="{ on: store.showTranslation }"
                @click="store.showTranslation = !store.showTranslation"
              >
                <div class="nfw-st-copy">
                  <span class="nfw-st-title">双语译文</span>
                  <span class="nfw-st-desc">外语歌曲展示中文翻译</span>
                </div>
                <span class="nfw-switch-pill" :class="{ on: store.showTranslation }">
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>

              <button
                type="button"
                class="nfw-switch-tile"
                :class="{ on: store.showPhonetic }"
                @click="store.showPhonetic = !store.showPhonetic"
              >
                <div class="nfw-st-copy">
                  <span class="nfw-st-title">音译注音</span>
                  <span class="nfw-st-desc">粤拼 / 日韩罗马音</span>
                </div>
                <span class="nfw-switch-pill" :class="{ on: store.showPhonetic }">
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>

              <button
                type="button"
                class="nfw-switch-tile"
                :class="{ on: store.lyricInterludeDotsEnabled }"
                @click="store.lyricInterludeDotsEnabled = !store.lyricInterludeDotsEnabled"
              >
                <div class="nfw-st-copy">
                  <span class="nfw-st-title">间奏呼吸</span>
                  <span class="nfw-st-desc">长间奏三点倒计时</span>
                </div>
                <span class="nfw-switch-pill" :class="{ on: store.lyricInterludeDotsEnabled }">
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>

              <button
                type="button"
                class="nfw-switch-tile"
                :class="{ on: store.lyricClickSeekEnabled }"
                @click="store.lyricClickSeekEnabled = !store.lyricClickSeekEnabled"
              >
                <div class="nfw-st-copy">
                  <span class="nfw-st-title">点击跳转</span>
                  <span class="nfw-st-desc">关闭可防止误触切进度</span>
                </div>
                <span class="nfw-switch-pill" :class="{ on: store.lyricClickSeekEnabled }">
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>

              <div class="nfw-return-delay-tile">
                <div class="nfw-st-copy">
                  <span class="nfw-st-title">滚轮自动回中</span>
                  <span class="nfw-st-desc">翻看歌词后回弹等待</span>
                </div>
                <div class="nfw-mini-pills">
                  <button
                    v-for="opt in autoReturnOptions"
                    :key="opt.sec"
                    type="button"
                    class="nfw-mini-pill"
                    :class="{ active: store.lyricAutoReturnSec === opt.sec }"
                    @click="store.lyricAutoReturnSec = opt.sec"
                  >
                    {{ opt.label }}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- ======================= 子页 C：双档时间校准、LRC 导出与在线词源 ======================= -->
        <div v-else class="nfw-lyric-body">
          <div class="nfw-section-block">
            <div class="nfw-block-head">
              <span>歌词时间轴双档校准（全局同步生效）</span>
              <span class="nfw-block-tag">{{ offsetSecText }}</span>
            </div>
            <div class="nfw-offset-stepper-5">
              <button
                type="button"
                class="nfw-step-btn"
                @click="adjustLyricOffset(-500)"
              >
                -0.5s
              </button>
              <button
                type="button"
                class="nfw-step-btn"
                @click="adjustLyricOffset(-100)"
              >
                -0.1s 延后
              </button>
              <button
                type="button"
                class="nfw-step-reset"
                :class="{ modified: store.lyricOffsetMs !== 0 }"
                @click="store.lyricOffsetMs = 0"
              >
                归零
              </button>
              <button
                type="button"
                class="nfw-step-btn"
                @click="adjustLyricOffset(100)"
              >
                +0.1s 提前
              </button>
              <button
                type="button"
                class="nfw-step-btn"
                @click="adjustLyricOffset(500)"
              >
                +0.5s
              </button>
            </div>

            <div class="nfw-slider-item" style="margin-top: 4px">
              <span class="nfw-slider-name" style="width: 72px">蓝牙补偿</span>
              <input
                v-model.number="store.bluetoothDelayMs"
                type="range"
                min="0"
                max="600"
                step="10"
                aria-label="蓝牙延迟补偿"
                class="nfw-range nfw-range-fill"
                :style="rangeProgressStyle(store.bluetoothDelayMs, 0, 600)"
              />
              <span class="nfw-slider-badge">
                {{ Math.round(store.bluetoothDelayMs) }}ms
              </span>
            </div>
          </div>

          <div class="nfw-search-bar-row">
            <input
              v-model="customLyricSearchKw"
              type="text"
              class="nfw-text-input"
              :placeholder="
                store.currentSong
                  ? `${store.currentSong.artist} - ${store.currentSong.title}`
                  : '输入歌名或歌手检索其他版本歌词...'
              "
              @keydown.enter.prevent="triggerSearchLyricCandidates"
            />
            <button
              type="button"
              class="nfw-primary-pill"
              :disabled="store.isSearchingLyricCandidates"
              @click="triggerSearchLyricCandidates"
            >
              {{ store.isSearchingLyricCandidates ? "检索中..." : "搜索词源" }}
            </button>
          </div>

          <div v-if="store.lyricCandidates.length > 0" class="nfw-cand-list">
            <button
              v-for="cand in store.lyricCandidates"
              :key="cand.id"
              type="button"
              class="nfw-cand-item"
              :style="
                activeTileStyle(store.activeLyricCandidateId === cand.id)
              "
              @click="store.selectLyricCandidate(cand)"
            >
              <div class="nfw-cand-main">
                <div class="nfw-cand-title">
                  {{ cand.songName }} - {{ cand.singerName }}
                </div>
                <div class="nfw-cand-sub">
                  {{ cand.fmt.toUpperCase() }} 逐字词源 · 匹配评分 {{ cand.score }}
                </div>
              </div>
              <span
                v-if="store.activeLyricCandidateId === cand.id"
                class="nfw-cand-check"
              >
                ✓ 已应用
              </span>
            </button>
          </div>
          <div v-else class="nfw-empty-hint">
            点击上方「搜索词源」可手动检索并切换不同版本的 KRC/LRC 双语歌词
          </div>
        </div>
      </div>
    </div>
  </transition>

  <!-- =========================================================================
       2. 播放更多设置中心悬浮窗 (MoreControlHub · 支持内联切换音质/倍速/睡眠/详情)
       ========================================================================= -->
  <transition name="np-float-spring">
    <div
      v-if="props.showMoreSheet"
      class="np-floating-layer"
      @click.self="emit('update:showMoreSheet', false)"
    >
      <div
        class="np-floating-window w-more-hub"
        :class="{ 'is-dark-surface': !isLightSurface }"
        :style="moreHubWindowStyle"
        @click.stop
      >
        <!-- 顶部导航栏（在二级子视图时显示返回按钮） -->
        <div class="nfw-header">
          <div class="nfw-title-row">
            <button
              v-if="moreSubView !== 'home'"
              type="button"
              class="nfw-back-orb"
              @click="moreSubView = 'home'"
            >
              <PlumpIcon
                name="arrow-left"
                :size="15"
                :color="readablePrimary"
              />
            </button>
            <div v-else class="nfw-icon-orb">
              <PlumpIcon name="sparkles" :size="17" :color="readablePrimary" />
            </div>

            <div>
              <h3 class="nfw-title">
                {{
                  moreSubView === "home"
                    ? "播放控制中枢"
                    : moreSubView === "speed_pitch"
                    ? "倍速与音高变调"
                    : moreSubView === "sleep"
                    ? "睡眠定时关闭"
                    : "音频流与母带元数据"
                }}
              </h3>
              <p class="nfw-sub">
                {{
                  moreSubView === "home"
                    ? "曲目档案与声学播控工具"
                    : moreSubView === "speed_pitch"
                    ? "独立调节播放倍速与半音升降调"
                    : moreSubView === "sleep"
                    ? sleepTimerSummary
                    : "无损音质规格与声学参数实时解析"
                }}
              </p>
            </div>
          </div>

          <button
            type="button"
            class="nfw-icon-btn"
            @click="emit('update:showMoreSheet', false)"
          >
            <PlumpIcon name="close" :size="14" />
          </button>
        </div>

        <!-- 内联子页面切换器 -->
        <transition name="nfw-subview-slide" mode="out-in">
          <!-- A. 主控制台视图 -->
          <div v-if="moreSubView === 'home'" key="home" class="nfw-more-body">
            <!-- 1. 歌曲名片 + 声学规格徽标 + 歌手/专辑/详情三联入口 -->
            <div class="nfw-song-hero-card">
              <div class="nfw-song-hero-top">
                <img
                  v-if="store.currentSong?.coverUrl"
                  :src="store.currentSong.coverUrl"
                  class="nfw-hero-cover"
                  alt=""
                />
                <div v-else class="nfw-hero-cover fallback">
                  <PlumpIcon name="disc" :size="24" />
                </div>

                <div class="nfw-hero-texts">
                  <div class="nfw-hero-title">
                    {{ store.currentSong?.title || "未在播放" }}
                  </div>
                  <div class="nfw-hero-sub">
                    {{ store.currentSong?.artist || "未知歌手" }}
                    <template v-if="store.currentSong?.album">
                      · 《{{ store.currentSong.album }}》
                    </template>
                  </div>
                  <div class="nfw-hero-badges">
                    <span class="nfw-hero-spec-badge">
                      {{
                        store.currentPlaybackInfo?.techSpecText ||
                        `${currentQualityLabel} · 无损音源`
                      }}
                    </span>
                    <span
                      v-if="activeViperObj || store.acousticField.enabled"
                      class="nfw-hero-dsp-badge"
                    >
                      ●
                      {{
                        activeViperObj?.label ||
                        activeViperObj?.name ||
                        "母带声场"
                      }}
                    </span>
                  </div>
                </div>

                <button
                  type="button"
                  class="nfw-detail-pill"
                  @click="moreSubView = 'detail'"
                >
                  <span>音频档案</span>
                </button>
              </div>

              <div class="nfw-hero-jumps">
                <button
                  type="button"
                  class="nfw-jump-card"
                  @click="handleJumpArtist"
                >
                  <div class="nfw-jump-orb">
                    <PlumpIcon
                      name="user"
                      :size="15"
                      :color="readablePrimary"
                    />
                  </div>
                  <div class="nfw-jump-meta">
                    <span class="nfw-jump-label">歌手主页</span>
                    <span class="nfw-jump-val">{{ artistEntrySubtitle }}</span>
                  </div>
                </button>

                <button
                  type="button"
                  class="nfw-jump-card"
                  @click="handleJumpAlbum"
                >
                  <div class="nfw-jump-orb">
                    <PlumpIcon
                      name="disc"
                      :size="15"
                      :color="readablePrimary"
                    />
                  </div>
                  <div class="nfw-jump-meta">
                    <span class="nfw-jump-label">所属专辑</span>
                    <span class="nfw-jump-val">{{ albumEntrySubtitle }}</span>
                  </div>
                </button>
              </div>
            </div>

            <!-- 2. 声学与播控 2x2 矩阵 -->
            <div class="nfw-bento-grid">
              <button
                type="button"
                class="nfw-bento-card"
                :style="activeTileStyle(store.climaxModeEnabled, true)"
                @click="handleToggleClimax"
              >
                <div class="nfw-bento-icon-orb">
                  <PlumpIcon
                    name="sparkles"
                    :size="17"
                    :color="store.climaxModeEnabled ? vipGold : readablePrimary"
                  />
                </div>
                <div class="nfw-bento-meta">
                  <span class="nfw-bento-name">副歌速刷</span>
                  <span class="nfw-bento-desc">
                    {{ store.climaxModeEnabled ? "高潮连播中" : "30秒精华直达" }}
                  </span>
                </div>
              </button>

              <button
                type="button"
                class="nfw-bento-card"
                :style="activeTileStyle(store.ktvAccompanimentMode)"
                @click="store.toggleKtvAccompaniment()"
              >
                <div class="nfw-bento-icon-orb">
                  <PlumpIcon name="music-note" :size="17" :color="readablePrimary" />
                </div>
                <div class="nfw-bento-meta">
                  <span class="nfw-bento-name">KTV 伴奏</span>
                  <span class="nfw-bento-desc">
                    {{
                      store.ktvAccompanimentMode ? "伴奏已开启" : "人声伴奏切换"
                    }}
                  </span>
                </div>
              </button>

              <button
                type="button"
                class="nfw-bento-card"
                :style="
                  activeTileStyle(
                    Math.abs(store.playbackRate - 1.0) > 0.01 ||
                      store.pitchSemitones !== 0
                  )
                "
                @click="moreSubView = 'speed_pitch'"
              >
                <div class="nfw-bento-icon-orb">
                  <PlumpIcon name="tuning" :size="17" :color="readablePrimary" />
                </div>
                <div class="nfw-bento-meta">
                  <span class="nfw-bento-name">倍速升降调</span>
                  <span class="nfw-bento-desc">{{ speedPitchSummary }}</span>
                </div>
              </button>

              <button
                type="button"
                class="nfw-bento-card"
                :style="
                  activeTileStyle(
                    store.sleepTimerMinutes !== 0 ||
                      store.sleepRemainingSeconds > 0
                  )
                "
                @click="moreSubView = 'sleep'"
              >
                <div class="nfw-bento-icon-orb">
                  <PlumpIcon name="timer" :size="17" :color="readablePrimary" />
                </div>
                <div class="nfw-bento-meta">
                  <span class="nfw-bento-name">睡眠定时</span>
                  <span class="nfw-bento-desc">{{ sleepTimerSummary }}</span>
                </div>
              </button>

              <button
                type="button"
                class="nfw-bento-card"
                :class="{ 'is-span-full': !store.downloadFeatureUnlocked }"
                :style="activeTileStyle(false)"
                @click="
                  emit('update:showMoreSheet', false);
                  if (store.currentSong)
                    store.openShareSongModal(store.currentSong);
                "
              >
                <div class="nfw-bento-icon-orb">
                  <PlumpIcon
                    name="share"
                    :size="17"
                    :color="readablePrimary"
                  />
                </div>
                <div class="nfw-bento-meta">
                  <span class="nfw-bento-name">分享单曲与唤醒链</span>
                  <span class="nfw-bento-desc">酷狗双端 · 歌词海报</span>
                </div>
              </button>

              <button
                v-if="store.downloadFeatureUnlocked"
                type="button"
                class="nfw-bento-card"
                :style="activeTileStyle(false)"
                @click="
                  emit('update:showMoreSheet', false);
                  if (store.currentSong)
                    store.downloadModalSong = store.currentSong;
                "
              >
                <div class="nfw-bento-icon-orb">
                  <PlumpIcon
                    name="download"
                    :size="17"
                    :color="readablePrimary"
                  />
                </div>
                <div class="nfw-bento-meta">
                  <span class="nfw-bento-name">下载歌曲 / 歌词</span>
                  <span class="nfw-bento-desc">音质 · 内嵌 · LRC</span>
                </div>
              </button>
            </div>
          </div>

          <!-- B. 内联子视图：倍速与升降调 -->
          <div
            v-else-if="moreSubView === 'speed_pitch'"
            key="speed_pitch"
            class="nfw-more-body"
          >
            <div class="nfw-section-block">
              <div class="nfw-block-head">
                <span>播放倍速 (Speed)</span>
                <span class="nfw-block-tag">
                  {{ store.playbackRate.toFixed(2) }}x
                </span>
              </div>
              <input
                :value="store.playbackRate"
                type="range"
                min="0.5"
                max="2.0"
                step="0.05"
                class="nfw-range nfw-range-fill"
                :style="rangeProgressStyle(store.playbackRate, 0.5, 2.0)"
                @input="
                  handleSpeedInput(
                    Number(($event.target as HTMLInputElement).value)
                  )
                "
              />
              <div class="nfw-preset-row">
                <button
                  v-for="sp in speedPresets"
                  :key="sp"
                  type="button"
                  class="nfw-preset-chip"
                  :style="
                    activeTileStyle(Math.abs(store.playbackRate - sp) < 0.02)
                  "
                  @click="handleSpeedInput(sp)"
                >
                  {{ sp.toFixed(2).replace(/\.00$/, ".0") }}x
                </button>
              </div>
            </div>

            <div class="nfw-section-block">
              <div class="nfw-block-head">
                <span>音高变调 (Pitch)</span>
                <span class="nfw-block-tag">
                  {{ pitchMultiplier.toFixed(2) }}x ({{
                    store.pitchSemitones > 0
                      ? `+${store.pitchSemitones}`
                      : store.pitchSemitones
                  }}半音)
                </span>
              </div>
              <input
                v-model.number="store.pitchSemitones"
                type="range"
                min="-12"
                max="12"
                step="1"
                :disabled="store.linkPitchWithSpeed"
                class="nfw-range nfw-range-fill"
                :style="rangeProgressStyle(store.pitchSemitones, -12, 12)"
                @input="store.setPitchSemitones(store.pitchSemitones)"
                @change="store.setPitchSemitones(store.pitchSemitones)"
              />
              <button
                type="button"
                class="nfw-switch-row"
                @click="
                  store.linkPitchWithSpeed = !store.linkPitchWithSpeed;
                  store.applySpeedAndPitch();
                "
              >
                <div>
                  <div class="nfw-switch-title">
                    黑胶物理转速联动（变速同步变调）
                  </div>
                  <div class="nfw-switch-sub">
                    开启后改变播放速度将自然改变歌曲音高
                  </div>
                </div>
                <span
                  class="nfw-switch-pill"
                  :class="{ on: store.linkPitchWithSpeed }"
                >
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>
            </div>

            <div class="nfw-footer-actions">
              <button
                type="button"
                class="nfw-ghost-btn"
                @click="resetSpeedAndPitch"
              >
                重置 1.0x 原速原调
              </button>
              <button
                type="button"
                class="nfw-primary-pill"
                @click="moreSubView = 'home'"
              >
                完成
              </button>
            </div>
          </div>

          <!-- D. 内联子视图：睡眠定时 -->
          <div
            v-else-if="moreSubView === 'sleep'"
            key="sleep"
            class="nfw-more-body"
          >
            <div class="nfw-sleep-grid">
              <button
                v-for="item in sleepPresets"
                :key="item.mins"
                type="button"
                class="nfw-sleep-card"
                :style="activeTileStyle(store.sleepTimerMinutes === item.mins)"
                @click="chooseSleepPreset(item.mins)"
              >
                {{ item.label }}
              </button>
            </div>

            <div class="nfw-section-block">
              <button
                type="button"
                class="nfw-switch-row"
                @click="
                  store.sleepWaitCurrentSong = !store.sleepWaitCurrentSong
                "
              >
                <div>
                  <div class="nfw-switch-title">
                    倒计时结束后播完当前歌曲再暂停
                  </div>
                  <div class="nfw-switch-sub">
                    避免在音乐播放中途突兀截断乐句
                  </div>
                </div>
                <span
                  class="nfw-switch-pill"
                  :class="{ on: store.sleepWaitCurrentSong }"
                >
                  <span class="nfw-switch-thumb"></span>
                </span>
              </button>
            </div>

            <div
              v-if="
                store.sleepTimerMinutes !== 0 ||
                store.sleepRemainingSeconds > 0
              "
              class="nfw-footer-actions"
            >
              <button
                type="button"
                class="nfw-danger-pill"
                @click="chooseSleepPreset(0)"
              >
                关闭当前睡眠定时
              </button>
            </div>
          </div>

          <!-- E. 内联子视图：音频详情 -->
          <div v-else key="detail" class="nfw-detail-scroll">
            <SongDetailGrid
              :model="moreAudioDetail"
              :loading="store.isLoadingCurrentSongDetail"
              variant="immersive"
            />
          </div>
        </transition>
      </div>
    </div>
  </transition>

  <!-- =========================================================================
       3. 独立音质切换悬浮窗 (从音质胶囊直接唤起时展示)
       ========================================================================= -->
  <transition name="np-float-spring">
    <div
      v-if="props.showQualityDialog"
      class="np-floating-layer"
      @click.self="emit('update:showQualityDialog', false)"
    >
      <div
        class="np-floating-window w-compact"
        :class="{ 'is-dark-surface': !isLightSurface }"
        :style="floatingWindowStyle"
        @click.stop
      >
        <div class="nfw-header">
          <div class="nfw-title-row">
            <div class="nfw-icon-orb">
              <PlumpIcon name="crown" :size="18" :color="readablePrimary" />
            </div>
            <div>
              <h3 class="nfw-title">切换在线音频规格</h3>
              <p class="nfw-sub">无损直连 · 仅展示当前曲目真实可用音质</p>
            </div>
          </div>
          <button
            type="button"
            class="nfw-icon-btn"
            @click="emit('update:showQualityDialog', false)"
          >
            <PlumpIcon name="close" :size="14" />
          </button>
        </div>

        <div class="nfw-list-scroll">
          <button
            v-for="q in qualityCatalog"
            :key="q.id"
            type="button"
            class="nfw-quality-card"
            :style="activeTileStyle(activeActualQuality === q.id, q.isGold)"
            @click="selectQuality(q.id)"
          >
            <div class="nfw-q-left">
              <div class="nfw-q-head">
                <span class="nfw-q-title">{{ q.title }}</span>
                <span
                  class="nfw-q-badge"
                  :style="{
                    background: q.isGold
                      ? hexWithAlpha(vipGold, 0.22)
                      : hexWithAlpha(readablePrimary, 0.2),
                    color: q.isGold ? vipGold : readablePrimary,
                  }"
                >
                  {{ q.badge }}
                </span>
                <span
                  v-if="
                    normalizedPreferredQuality === q.id &&
                    activeActualQuality !== q.id
                  "
                  class="nfw-q-pref-tag"
                >
                  默认偏好
                </span>
              </div>
              <div class="nfw-q-desc">{{ q.spec }}</div>
            </div>
            <div class="nfw-q-right">
              <span class="nfw-q-size">{{ q.sizeText }}</span>
              <span v-if="activeActualQuality === q.id" class="nfw-q-check">
                ✓
              </span>
            </div>
          </button>

          <div v-if="qualityCatalog.length === 0" class="nfw-empty-hint">
            当前暂无在播曲目或暂无可切换的音频规格
          </div>
        </div>
      </div>
    </div>
  </transition>

  <!-- =========================================================================
       4. 独立音频详情悬浮窗
       ========================================================================= -->
  <transition name="np-float-spring">
    <div
      v-if="store.songDetailModalSong"
      class="np-floating-layer"
      @click.self="store.songDetailModalSong = null"
    >
      <div
        class="np-floating-window w-detail"
        :class="{ 'is-dark-surface': !isLightSurface }"
        :style="floatingWindowStyle"
        @click.stop
      >
        <div class="nfw-header">
          <div class="nfw-title-row">
            <div class="nfw-icon-orb">
              <PlumpIcon name="wave" :size="18" :color="readablePrimary" />
            </div>
            <div>
              <h3 class="nfw-title">音频流与母带元数据</h3>
              <p class="nfw-sub">发行档案与硬件级声学流实时解析</p>
            </div>
          </div>
          <button
            type="button"
            class="nfw-icon-btn"
            @click="store.songDetailModalSong = null"
          >
            <PlumpIcon name="close" :size="14" />
          </button>
        </div>

        <div class="nfw-detail-scroll">
          <SongDetailGrid
            :model="modalAudioDetail"
            :loading="store.isLoadingSongDetail"
            variant="immersive"
          />
        </div>
      </div>
    </div>
  </transition>

  <!-- =========================================================================
       7. 添加到歌单悬浮窗已统一至全局 AddToPlaylistModal.vue
       ========================================================================= -->

  <!-- =========================================================================
       8. 下载歌曲悬浮窗
       ========================================================================= -->
  <transition name="np-float-spring">
    <div
      v-if="store.downloadModalSong"
      class="np-floating-layer"
      @click.self="store.downloadModalSong = null"
    >
      <div
        class="np-floating-window w-compact"
        :class="{ 'is-dark-surface': !isLightSurface }"
        :style="floatingWindowStyle"
        @click.stop
      >
        <div class="nfw-header">
          <div class="nfw-title-row">
            <div class="nfw-icon-orb">
              <PlumpIcon name="download" :size="18" :color="readablePrimary" />
            </div>
            <div>
              <h3 class="nfw-title">下载歌曲与内嵌标签</h3>
              <p class="nfw-sub">
                《{{ store.downloadModalSong.title }}》 · 自动内嵌封面与歌词
              </p>
            </div>
          </div>
          <button
            type="button"
            class="nfw-icon-btn"
            @click="store.downloadModalSong = null"
          >
            <PlumpIcon name="close" :size="14" />
          </button>
        </div>

        <div class="nfw-dl-opts-grid">
          <button
            type="button"
            class="nfw-dl-opt-chip"
            :class="{ 'is-on': store.embedCoverAndTags }"
            :aria-pressed="store.embedCoverAndTags"
            @click="store.embedCoverAndTags = !store.embedCoverAndTags"
          >
            <PlumpIcon name="image" :size="13" />
            <span>内嵌高清封面与标签</span>
          </button>
          <button
            type="button"
            class="nfw-dl-opt-chip"
            :class="{ 'is-on': store.embedLyricsInAudio }"
            :aria-pressed="store.embedLyricsInAudio"
            @click="store.embedLyricsInAudio = !store.embedLyricsInAudio"
          >
            <PlumpIcon name="lyric" :size="13" />
            <span>内嵌歌词到音频</span>
          </button>
          <button
            type="button"
            class="nfw-dl-opt-chip"
            :class="{ 'is-on': store.exportStandaloneLrc }"
            :aria-pressed="store.exportStandaloneLrc"
            @click="store.exportStandaloneLrc = !store.exportStandaloneLrc"
          >
            <PlumpIcon name="download" :size="13" />
            <span>同时保存 .lrc 歌词</span>
          </button>
          <button
            type="button"
            class="nfw-dl-opt-chip"
            :class="{ 'is-on': store.skipExistingFiles }"
            :aria-pressed="store.skipExistingFiles"
            @click="store.skipExistingFiles = !store.skipExistingFiles"
          >
            <PlumpIcon name="check" :size="13" />
            <span>跳过同名已存在</span>
          </button>
        </div>

        <div class="nfw-list-scroll">
          <button
            v-for="q in downloadQualityCatalog"
            :key="q.id"
            type="button"
            class="nfw-quality-card"
            :style="activeTileStyle(normalizedDownloadQuality === q.id, q.isGold)"
            @click="
              if (store.downloadModalSong) {
                store.triggerDownloadSong(store.downloadModalSong, q.id);
              }
            "
          >
            <div class="nfw-q-left">
              <div class="nfw-q-head">
                <span class="nfw-q-title">下载 {{ q.title }}</span>
                <span
                  class="nfw-q-badge"
                  :style="{
                    background: q.isGold
                      ? hexWithAlpha(vipGold, 0.22)
                      : hexWithAlpha(readablePrimary, 0.2),
                    color: q.isGold ? vipGold : readablePrimary,
                  }"
                >
                  {{ q.badge }}
                </span>
              </div>
              <div class="nfw-q-desc">{{ q.spec }}</div>
            </div>
            <div class="nfw-q-right">
              <span class="nfw-q-size">{{ q.sizeText }}</span>
            </div>
          </button>

          <div v-if="downloadQualityCatalog.length === 0" class="nfw-empty-hint">
            该曲目暂无可下载的在线音频规格
          </div>
        </div>

        <div class="nfw-dl-lyric-footer">
          <div class="nfw-dl-lyric-actions">
            <button
              type="button"
              class="nfw-primary-pill sm"
              :disabled="store.isSavingLyricOnly"
              @click="
                store.downloadModalSong &&
                  store.saveSongLyricOnly(store.downloadModalSong)
              "
            >
              <PlumpIcon name="lyric" :size="13" />
              <span>{{ store.isSavingLyricOnly ? "正在保存 .lrc..." : "仅保存 .lrc 歌词文件" }}</span>
            </button>
            <button
              type="button"
              class="nfw-ghost-btn sm"
              @click="
                store.downloadModalSong &&
                  store.copySongLrcToClipboard(store.downloadModalSong)
              "
            >
              <span>复制 .lrc</span>
            </button>
          </div>
          <div
            v-if="activeDownloadTaskInModal"
            class="nfw-dl-live-progress"
            role="status"
            aria-live="polite"
          >
            <div class="nfw-dl-live-meta">
              <span class="nfw-dl-live-title">
                正在下载：{{ activeDownloadTaskInModal.song.title }}
              </span>
              <span class="nfw-dl-live-pct">
                {{ Math.round(activeDownloadTaskInModal.percent) }}%
              </span>
            </div>
            <progress
              class="bms-native-progress"
              :max="100"
              :value="
                activeDownloadTaskInModal.status === 'downloading'
                  ? Math.round(activeDownloadTaskInModal.percent)
                  : undefined
              "
              :aria-label="`下载进度：${activeDownloadTaskInModal.song.title}`"
            ></progress>
          </div>
        </div>
      </div>
    </div>
  </transition>

  <!-- =========================================================================
       9. 蝰蛇声学中心 (Viper Acoustic Studio · 极简现代重构：精选音效 + 听感微调，支持全局 Teleport 唤起)
       ========================================================================= -->
  <Teleport to="body">
    <transition name="np-float-spring">
      <div
        v-if="isViperStudioVisible"
        class="np-floating-layer"
        @click.self="closeViperStudio"
      >
        <div
          class="np-floating-window w-viper-studio"
          :class="{ 'is-dark-surface': !isLightSurface }"
          :style="floatingWindowStyle"
          @click.stop
        >
        <!-- 顶部主控栏：品牌标识 + 极简双视图切换 + DSP 总控 -->
        <div class="nfw-header vas-header">
          <div class="nfw-title-row">
            <div class="nfw-icon-orb vas-orb">
              <PlumpIcon
                name="headphones"
                :size="18"
                :color="readablePrimary"
              />
            </div>
            <div class="vas-title-block">
              <div class="vas-title-line">
                <h3 class="nfw-title">蝰蛇声学中心</h3>
                <span
                  v-if="store.isPlaying && store.masterEffectEnabled"
                  class="vas-acoustic-eq"
                  aria-hidden="true"
                >
                  <i></i><i></i><i></i>
                </span>
              </div>
              <p class="nfw-sub">{{ activeAcousticSummaryText }}</p>
            </div>
          </div>

          <!-- 极简双态模式胶囊：精选音效 / 听感微调 -->
          <div class="vas-mode-tabs" role="tablist" aria-label="音效模式">
            <button
              type="button"
              role="tab"
              class="vas-mode-pill"
              :class="{ active: audioStudioTab === 'presets' }"
              :aria-selected="audioStudioTab === 'presets'"
              @click="audioStudioTab = 'presets'"
            >
              <span>精选音效</span>
              <span class="vas-mode-count">
                {{ (store.viperEffects || []).length }}
              </span>
            </button>
            <button
              type="button"
              role="tab"
              class="vas-mode-pill"
              :class="{ active: audioStudioTab === 'tuning' }"
              :aria-selected="audioStudioTab === 'tuning'"
              @click="audioStudioTab = 'tuning'"
            >
              <span>听感微调</span>
              <span
                v-if="store.acousticField.enabled"
                class="vas-mode-live-dot"
              ></span>
            </button>
          </div>

          <div class="nfw-header-actions">
            <button
              type="button"
              class="nfw-dsp-pill"
              :class="{ on: store.masterEffectEnabled }"
              @click="store.toggleMasterEffect()"
            >
              <span class="nfw-dsp-dot"></span>
              <span>
                {{ store.masterEffectEnabled ? "DSP 开启" : "DSP 旁路" }}
              </span>
            </button>

            <button
              type="button"
              class="nfw-ghost-btn sm"
              :class="{ muted: !isAnyAcousticEffectModified }"
              title="一键复位为 0dB 原声直通"
              @click="resetAllAcousticToBitPerfect"
            >
              原声复位
            </button>

            <button
              type="button"
              class="nfw-icon-btn"
              @click="closeViperStudio"
            >
              <PlumpIcon name="close" :size="14" />
            </button>
          </div>
        </div>

        <!-- ======================= 视图切换舞台（L1 分段器方向感知滑动过渡） ======================= -->
        <transition
          :name="audioStudioTab === 'tuning' ? 'vas-pane-left' : 'vas-pane-right'"
          mode="out-in"
        >
        <!-- ======================= 视图 A：精选音效矩阵 ======================= -->
        <div v-if="audioStudioTab === 'presets'" key="vas-presets" class="vas-tab-body">
          <!-- 分类胶囊 + 极简搜索框 -->
          <div class="vas-toolbar">
            <div class="vas-cat-pills" role="tablist" aria-label="音效分类">
              <button
                v-for="cat in effectCategories"
                :key="cat"
                type="button"
                role="tab"
                class="vas-cat-btn"
                :class="{ active: effectCategoryTab === cat }"
                :aria-selected="effectCategoryTab === cat"
                @click="effectCategoryTab = cat"
              >
                <span>{{ cat }}</span>
                <span class="vas-cat-num">{{ categoryCounts[cat] || 0 }}</span>
              </button>
            </div>

            <div class="vas-search-box">
              <PlumpIcon
                name="search"
                :size="13"
                color="var(--fm-text-3)"
              />
              <input
                v-model="effectSearchKeyword"
                type="text"
                placeholder="检索音效或场景..."
                class="vas-search-input"
              />
              <button
                v-if="effectSearchKeyword"
                type="button"
                class="vas-search-clear"
                @click="effectSearchKeyword = ''"
              >
                ×
              </button>
            </div>
          </div>

          <!-- 2列精致音效卡片矩阵（随分类 Tab 平滑切换） -->
          <transition name="tag-content-morph" mode="out-in">
          <div :key="effectCategoryTab" class="vas-preset-grid">
            <button
              v-for="effect in filteredViperEffects"
              :key="effect.id"
              type="button"
              class="vas-preset-card"
              :class="{
                mounted: store.activeViperEffectId === effect.id,
              }"
              :style="
                activeTileStyle(
                  store.activeViperEffectId === effect.id,
                  effect.id === 'viper_black_diamond' ||
                    effect.id === 'viper_clear_master' ||
                    effect.id === 'viper_panorama_51'
                )
              "
              @click="selectViperPresetFromDialog(effect.id)"
            >
              <div class="vas-card-top">
                <div class="vas-card-title-row">
                  <span class="vas-card-name">
                    {{ effect.label || effect.name }}
                  </span>
                </div>
                <span
                  class="vas-card-badge"
                  :class="{ mounted: store.activeViperEffectId === effect.id }"
                >
                  <template v-if="store.activeViperEffectId === effect.id">
                    <span class="vas-live-pulse"></span>
                    <span>生效中</span>
                  </template>
                  <template v-else>
                    {{ effect.category || "官方旗舰" }}
                  </template>
                </span>
              </div>

              <p class="vas-card-desc">
                {{ effect.desc || "高精度多维空间声场调节" }}
              </p>

              <div class="vas-card-bottom">
                <svg
                  class="vas-curve-spark"
                  viewBox="0 0 72 24"
                  preserveAspectRatio="none"
                  aria-hidden="true"
                >
                  <line
                    x1="2"
                    y1="12"
                    x2="70"
                    y2="12"
                    :stroke="isLightSurface ? 'rgba(24,34,48,0.12)' : 'rgba(255,255,255,0.14)'"
                    stroke-width="1"
                    stroke-dasharray="2 2"
                  />
                  <path
                    :d="buildMiniCurvePath(effect.gains)"
                    fill="none"
                    :stroke="
                      store.activeViperEffectId === effect.id
                        ? effect.id === 'viper_black_diamond' ||
                          effect.id === 'viper_clear_master' ||
                          effect.id === 'viper_panorama_51'
                          ? vipGold
                          : readablePrimary
                        : isLightSurface
                        ? 'rgba(24,34,48,0.44)'
                        : 'rgba(255,255,255,0.52)'
                    "
                    stroke-width="1.8"
                    stroke-linecap="round"
                  />
                </svg>

                <div class="vas-card-tags">
                  <span
                    v-for="tag in (effect.acousticTags || []).slice(0, 2)"
                    :key="tag"
                    class="vas-card-tag"
                  >
                    {{ tag }}
                  </span>
                </div>
              </div>
            </button>
          </div>
          </transition>

          <div
            v-if="filteredViperEffects.length === 0"
            class="nfw-empty-hint"
          >
            未找到匹配「{{ effectSearchKeyword }}」的声学预设，请尝试更换关键词
          </div>
        </div>

        <!-- ======================= 视图 B：听感微调（极简四维滑块 + 空间模型 + 调音台直达） ======================= -->
        <div v-else key="vas-tuning" class="vas-tab-body vas-tuning-body">
          <!-- 顶部当前调节上下文提示条 -->
          <div class="vas-tuning-context-bar">
            <div class="vas-tc-left">
              <span class="vas-tc-dot" :class="{ on: store.acousticField.enabled }"></span>
              <span class="vas-tc-text">
                {{
                  activeViperObj
                    ? `正在微调「${activeViperObj.label || activeViperObj.name}」的听感参数`
                    : store.acousticField.enabled
                    ? "当前处于自定义听感微调模式"
                    : "轻推滑块即可实时调校低频、人声与空间临场感"
                }}
              </span>
            </div>
            <div class="vas-tc-actions">
              <button
                v-if="activeViperObj"
                type="button"
                class="vas-tc-btn"
                @click="restoreCurrentPresetDefaults"
              >
                恢复预设默认
              </button>
            </div>
          </div>

          <!-- 1. 四维核心听感直观滑块卡片（2×2 大、简、雅布局） -->
          <div class="vas-core-grid">
            <div
              v-for="item in coreTuningSliders"
              :key="item.key"
              class="vas-core-card"
            >
              <div class="vas-core-head">
                <div>
                  <div class="vas-core-title">{{ item.label }}</div>
                  <div class="vas-core-desc">{{ item.desc }}</div>
                </div>
                <span class="vas-core-val">
                  {{ Math.round(item.value) }}{{ item.unit }}
                </span>
              </div>
              <input
                :value="item.value"
                type="range"
                :min="item.min"
                :max="item.max"
                :step="item.step"
                class="nfw-range nfw-range-fill vas-core-range"
                :style="rangeProgressStyle(item.value, item.min, item.max)"
                @input="
                  updateSpatialFieldParam(
                    item.key,
                    Number(($event.target as HTMLInputElement).value),
                    false
                  )
                "
                @change="
                  updateSpatialFieldParam(
                    item.key,
                    Number(($event.target as HTMLInputElement).value),
                    true
                  )
                "
              />
            </div>
          </div>

          <!-- 2. 空间氛围模型选择 -->
          <div class="nfw-section-block vas-env-card">
            <div class="nfw-block-head">
              <span>空间氛围模型</span>
              <span class="nfw-block-tag">
                {{
                  REVERB_ROOM_MODELS.find(
                    (r) => r.id === store.acousticField.reverbPreset
                  )?.label || "无混响"
                }}
              </span>
            </div>
            <div class="vas-reverb-grid">
              <button
                v-for="room in REVERB_ROOM_MODELS"
                :key="room.id"
                type="button"
                class="vas-room-card"
                :style="activeTileStyle(store.acousticField.reverbPreset === room.id)"
                @click="selectReverbRoomModel(room.id)"
              >
                <span class="vas-room-title">{{ room.label }}</span>
              </button>
            </div>
          </div>

          <!-- 3. 专业调音台页面跳转引导卡片 -->
          <div class="vas-tuner-callout">
            <div class="vas-tuner-callout-copy">
              <div class="vas-tuner-callout-title">
                需要 10 / 15 / 31 段参量均衡与频谱图精度调校？
              </div>
              <div class="vas-tuner-callout-sub">
                多段 EQ 曲线、双耳互馈及高阶声学机架已统一收敛至独立调音台
              </div>
            </div>
            <button
              type="button"
              class="nfw-primary-pill vas-tuner-jump-btn"
              @click="handleJumpToTuner"
            >
              <PlumpIcon name="sliders" :size="14" :color="onAccentPrimary" />
              <span>跳去专业调音台页面 ›</span>
            </button>
          </div>
        </div>
        </transition>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
/* ============================================================================
   1. 透明非覆盖式弹出层：底层播放页内容 100% 保持原貌，仅弹出上层悬浮窗口
   ============================================================================ */
.np-floating-layer {
  position: fixed;
  inset: 0;
  z-index: 2300;
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.np-floating-layer.anchor-bottom-right {
  justify-content: flex-end;
  align-items: flex-end;
  padding: 24px 36px 76px 24px;
}

@media (max-width: 820px) {
  .np-floating-layer.anchor-bottom-right {
    justify-content: center;
    align-items: center;
    padding: 20px;
  }
}

.np-floating-window {
  border-radius: var(--bm-r-xl, 24px);
  padding: 20px 24px;
  color: var(--fm-text-1);
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow: hidden;
  position: relative;
}

.np-floating-window.w-lyric-studio {
  width: min(720px, 92vw);
  height: min(660px, 88vh);
}

.np-floating-window.w-more-hub {
  width: min(560px, 94vw);
  height: min(452px, 84vh);
  transition:
    width 0.34s cubic-bezier(0.22, 1, 0.36, 1),
    height 0.34s cubic-bezier(0.22, 1, 0.36, 1);
}

.np-floating-window.w-compact {
  width: min(496px, 92vw);
  height: auto;
  max-height: min(560px, 84vh);
}

.np-floating-window.w-compact .nfw-list-scroll,
.np-floating-window.w-compact .nfw-more-body {
  flex: 0 1 auto;
}

.np-floating-window.w-detail {
  width: min(580px, 94vw);
  height: min(580px, 84vh);
}

.np-floating-window.w-viper-studio {
  width: min(780px, 90vw);
  height: min(620px, 84vh);
  padding: 22px 24px;
}

/* ============================================================================
   2. 通用悬浮窗头部与微交互按钮
   ============================================================================ */
.nfw-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-shrink: 0;
}

.nfw-title-row {
  display: flex;
  align-items: center;
  gap: 11px;
  min-width: 0;
  flex: 1;
}

.nfw-title-row > div:not(.nfw-icon-orb) {
  min-width: 0;
  flex: 1;
}

.nfw-icon-orb,
.nfw-back-orb {
  width: 36px;
  height: 36px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--np-primary-soft, rgba(56, 152, 255, 0.2));
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.nfw-back-orb {
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease;
}

.nfw-back-orb:hover {
  transform: translateX(-2px) scale(1.05);
  background: var(--fm-card-hover);
}

.nfw-title {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: 800;
  color: var(--fm-text-1);
  margin: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nfw-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
  margin: 2px 0 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nfw-header-actions {
  display: flex;
  align-items: center;
  gap: 7px;
  flex-shrink: 0;
}

.nfw-icon-btn {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  background: var(
    --np-card-elevated,
    var(--fm-card-elevated)
  );
  color: var(--fm-text-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    color 0.2s ease;
}

.nfw-icon-btn:hover {
  transform: scale(1.08);
  background: var(--fm-card-hover);
  color: var(--fm-text-1);
}

.nfw-segmented-mini {
  display: inline-flex;
  align-items: center;
  gap: 14px;
}

.nfw-seg-pill {
  padding: 4px 2px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--fm-text-3);
  background: transparent;
  cursor: pointer;
  transition: color 0.2s ease;
}

.nfw-seg-pill:hover {
  color: var(--fm-text-1);
}

.nfw-seg-pill.active {
  background: transparent;
  color: var(--fm-text-1);
}

/* 歌词设置三段胶囊切换（主题化无框，不被全局 tab 下划线劫持） */
.nfw-pill-switcher {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 3px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-chip-bg);
}

.nfw-pill-tab {
  height: 28px;
  padding: 0 12px;
  border-radius: var(--bm-r-pill, 999px);
  background: transparent;
  color: var(--fm-text-2);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background 0.22s ease,
    color 0.2s ease,
    box-shadow 0.22s ease;
}

.nfw-pill-tab:hover {
  color: var(--fm-text-1);
}

.nfw-pill-tab.active {
  background: linear-gradient(
    135deg,
    var(--np-primary, var(--fm-primary)),
    var(--np-secondary, var(--fm-secondary))
  );
  color: var(--np-on-primary, var(--fm-on-primary));
  font-weight: 800;
  box-shadow: 0 4px 12px var(--np-primary-glow, rgba(0, 0, 0, 0.2));
}

/* ============================================================================
   3. 歌词视觉与排版悬浮窗内部组件
   ============================================================================ */
.nfw-lyric-body,
.nfw-more-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-right: 4px;
}

.nfw-dl-control-bar {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 9px;
}

.nfw-dl-mode-pair {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.nfw-dl-bg-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.nfw-dl-swatch-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
}

.nfw-dl-hero-bar {
  gap: 10px;
}

.nfw-dl-hero-actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.nfw-dl-sync-strip {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding-top: 2px;
}

.nfw-dl-scope-note {
  font-size: var(--bm-fs-micro, 11.5px);
  font-weight: 600;
  color: var(--fm-text-3);
}

.nfw-dl-toggle-strip {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 7px;
  padding-top: 2px;
}

.nfw-inline-pair-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 10px 14px;
}

.nfw-inline-group {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  flex-wrap: wrap;
}

.nfw-inline-label {
  font-size: var(--bm-fs-micro, 11.5px);
  font-weight: 800;
  color: var(--fm-text-3);
  white-space: nowrap;
}

.nfw-mini-pills {
  display: inline-flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px;
  padding: 2px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-card-elevated);
}

.nfw-mini-pill {
  height: 26px;
  padding: 0 10px;
  border-radius: var(--bm-r-pill, 999px);
  background: transparent;
  color: var(--fm-text-2);
  font-size: var(--bm-fs-micro, 11.5px);
  font-weight: 700;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background 0.18s ease,
    color 0.18s ease,
    transform 0.18s ease;
}

.nfw-mini-pill:hover {
  color: var(--fm-text-1);
  background: var(--fm-chip-hover);
}

.nfw-mini-pill.active {
  background: var(--np-primary-soft, var(--fm-primary-soft));
  color: var(--np-primary, var(--fm-primary));
  font-weight: 800;
}

.nfw-mini-pill.lg {
  height: 30px;
  padding: 0 12px;
  background: var(--fm-card-elevated);
  font-size: var(--bm-fs-caption, 12.5px);
}

.nfw-mini-pill.lg:hover {
  background: var(--fm-card-hover);
}

.nfw-mini-pill.lg.active {
  background: var(--np-primary-soft, var(--fm-primary-soft));
  color: var(--np-primary, var(--fm-primary));
}

/* 清晰分组的动效与交互开关矩阵 */
.nfw-switch-matrix {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

@media (max-width: 560px) {
  .nfw-switch-matrix,
  .nfw-dl-hero-actions {
    grid-template-columns: 1fr;
  }
}

.nfw-switch-tile,
.nfw-return-delay-tile {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 9px 12px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--fm-card-elevated);
  text-align: left;
  transition:
    background 0.2s ease,
    transform 0.2s ease;
}

.nfw-switch-tile {
  cursor: pointer;
}

.nfw-switch-tile:hover {
  background: var(--fm-card-hover);
  transform: translateY(-1px);
}

.nfw-switch-tile.on {
  background: var(--fm-card-active);
}

.nfw-st-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nfw-st-title {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.nfw-st-desc {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--fm-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nfw-section-block.nfw-copy-bar {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.nfw-copy-meta {
  min-width: 0;
}

.nfw-copy-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.nfw-lyric-preview-card {
  position: relative;
  border-radius: var(--bm-r-lg, 18px);
  padding: 14px 16px 12px;
  background: var(--fm-input-bg);
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex-shrink: 0;
  transition: background 0.25s ease;
}

.nfw-preview-corner-tag {
  position: absolute;
  top: 8px;
  right: 12px;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  padding: 2px 8px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-chip-bg);
  color: var(--fm-text-3);
  pointer-events: none;
}

.nfw-lp-main {
  font-weight: 800;
  line-height: 1.35;
}

.nfw-lp-sung {
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
  -webkit-text-fill-color: transparent;
  transition: background-image 0.25s ease;
}

.nfw-lp-trans {
  font-weight: 600;
  line-height: 1.3;
  transition: color 0.25s ease;
}

.nfw-lp-next {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  color: var(--fm-text-2);
  transition:
    opacity 0.2s ease,
    filter 0.2s ease;
}

.nfw-section-block {
  background: var(--fm-card);
  border-radius: var(--bm-r-lg, 18px);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 9px;
}

.nfw-block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--fm-text-2);
}

.nfw-inline-ctrl-bar {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.nfw-block-tag {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--np-primary, #3898ff);
}

.nfw-theme-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 7px;
}

.nfw-unsung-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 7px;
}

.nfw-color-custom-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 10px;
  padding-top: 2px;
}

.nfw-color-picker-group {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.nfw-color-picker-chip {
  height: 30px;
  padding: 0 10px 0 7px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-card-elevated);
  color: var(--fm-text-2);
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  font-weight: 700;
  cursor: pointer;
  transition: background 0.16s ease, color 0.16s ease;
}

.nfw-color-picker-chip:hover,
.nfw-color-picker-chip.active {
  background: var(--fm-primary-soft);
  color: var(--fm-text-1);
}

.nfw-color-native-input {
  width: 16px;
  height: 16px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  cursor: pointer;
  overflow: hidden;
  -webkit-appearance: none;
  appearance: none;
}

.nfw-color-native-input::-webkit-color-swatch-wrapper {
  padding: 0;
}

.nfw-color-native-input::-webkit-color-swatch {
  border: none;
  border-radius: 50%;
}

.nfw-switch-tile.is-disabled {
  opacity: 0.46;
  cursor: not-allowed;
}

.nfw-theme-swatch {
  height: 34px;
  border-radius: var(--bm-r-sm, 10px);
  padding: 0 9px;
  background: var(--fm-card-elevated);
  color: var(--fm-text-2);
  display: flex;
  align-items: center;
  gap: 7px;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    box-shadow 0.2s ease;
}

.nfw-theme-swatch:hover {
  transform: translateY(-1px);
  background: var(--fm-card-hover);
}

.nfw-theme-swatch.active {
  color: var(--fm-text-1);
}

.nfw-swatch-orb {
  width: 11px;
  height: 11px;
  border-radius: 50%;
  flex-shrink: 0;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
}

.nfw-swatch-name {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  white-space: nowrap;
}

.nfw-dual-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

@media (max-width: 520px) {
  .nfw-dual-row {
    grid-template-columns: 1fr;
  }
}

.nfw-segmented-bar {
  display: inline-flex;
  align-items: center;
  gap: 18px;
}

.nfw-seg-option {
  height: 28px;
  padding: 0 2px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--fm-text-3);
  background: transparent;
  cursor: pointer;
  transition: color 0.2s cubic-bezier(0.22, 1, 0.36, 1);
}

.nfw-seg-option:hover {
  color: var(--fm-text-1);
}

.nfw-seg-option.active {
  background: transparent;
  color: var(--fm-text-1);
  font-weight: 800;
}

.nfw-offset-stepper {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
  gap: 5px;
}

.nfw-offset-stepper-5 {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 7px;
}

.nfw-step-btn,
.nfw-step-reset {
  height: 30px;
  padding: 0 8px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--fm-card-elevated);
  color: var(--fm-text-1);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  cursor: pointer;
  transition:
    transform 0.18s ease,
    background 0.18s ease;
}

.nfw-step-btn:hover,
.nfw-step-reset:hover {
  background: var(--fm-chip-hover);
}

.nfw-step-reset.modified {
  background: var(--np-vip-soft, rgba(255, 200, 87, 0.22));
  color: var(--np-vip-gold, #b45309);
  font-weight: 800;
}

.nfw-font-grid-4col {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
}

.nfw-font-pill {
  background: var(--fm-card-elevated);
  color: var(--fm-text-1);
  padding: 9px 10px;
  border-radius: var(--bm-r-md, 14px);
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  text-align: left;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    box-shadow 0.2s ease;
}

.nfw-font-pill:hover {
  transform: translateY(-1px);
}

.nfw-font-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.nfw-font-desc {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  color: var(--fm-text-3);
}

.nfw-sliders-group {
  gap: 10px;
}

.nfw-sliders-2col {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px 18px;
}

.nfw-slider-span2 {
  grid-column: span 2;
}

@media (max-width: 580px) {
  .nfw-font-grid-4col {
    grid-template-columns: repeat(2, 1fr);
  }
  .nfw-sliders-2col {
    grid-template-columns: 1fr;
  }
  .nfw-slider-span2 {
    grid-column: span 1;
  }
}

.nfw-slider-item {
  display: flex;
  align-items: center;
  gap: 10px;
  transition: opacity 0.2s ease;
}

.nfw-slider-item.is-disabled {
  opacity: 0.42;
}

.nfw-slider-name {
  width: 58px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--fm-text-2);
  flex-shrink: 0;
}

.nfw-range {
  flex: 1;
  -webkit-appearance: none;
  appearance: none;
  height: 5px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-chip-hover);
  accent-color: var(--np-primary, var(--fm-primary));
  cursor: pointer;
}

.nfw-range:focus-visible {
  outline: 2px solid var(--np-primary, var(--fm-primary));
  outline-offset: 4px;
}

.nfw-range.nfw-range-fill {
  background: linear-gradient(
    90deg,
    var(--np-primary, var(--fm-primary)) 0%,
    var(--np-secondary, var(--fm-secondary)) var(--range-pct, 0%),
    var(--fm-chip-bg) var(--range-pct, 0%), var(--fm-chip-bg) 100%
  );
}

.nfw-range::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 15px;
  height: 15px;
  border-radius: 50%;
  background: #ffffff;
  box-shadow:
    0 2px 8px rgba(0, 0, 0, 0.36),
    0 0 10px var(--np-primary-glow, rgba(56, 152, 255, 0.45));
  cursor: pointer;
  transition: transform 0.18s cubic-bezier(0.22, 1.35, 0.36, 1);
}

.nfw-range::-moz-range-thumb {
  width: 15px;
  height: 15px;
  border: none;
  border-radius: 50%;
  background: #ffffff;
  box-shadow:
    0 2px 8px rgba(0, 0, 0, 0.36),
    0 0 10px var(--np-primary-glow, rgba(56, 152, 255, 0.45));
  cursor: pointer;
}

.nfw-range::-webkit-slider-thumb:hover {
  transform: scale(1.16);
}

.nfw-slider-badge {
  min-width: 46px;
  text-align: right;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--np-primary, #3898ff);
  font-variant-numeric: tabular-nums;
}

.nfw-toggles-3col {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
}

.nfw-toggles-4col {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 9px;
}

@media (max-width: 580px) {
  .nfw-toggles-4col {
    grid-template-columns: repeat(2, 1fr);
  }
}

.nfw-toggle-chip {
  background: var(--fm-card);
  color: var(--fm-text-1);
  padding: 10px 12px;
  border-radius: var(--bm-r-md, 14px);
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 3px;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    box-shadow 0.2s ease;
}

.nfw-toggle-chip:hover {
  transform: translateY(-1px);
}

.nfw-chip-title {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.nfw-chip-state {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  color: var(--np-primary, #3898ff);
}

/* 搜索词源与通用输入栏 */
.nfw-search-bar-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.nfw-text-input {
  flex: 1;
  height: 36px;
  padding: 0 13px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--fm-input-bg);
  color: var(--fm-text-1);
  font-size: var(--bm-fs-caption, 12.5px);
}

.nfw-text-input::placeholder {
  color: var(--fm-text-3);
}

.nfw-primary-pill {
  height: 36px;
  padding: 0 15px;
  border-radius: var(--bm-r-sm, 10px);
  background: linear-gradient(
    135deg,
    var(--np-primary, #3898ff),
    var(--np-secondary, #00e5ff)
  );
  color: var(--np-on-primary, var(--fm-on-primary));
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  white-space: nowrap;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    filter 0.2s ease;
}

.nfw-primary-pill.sm {
  height: 30px;
  padding: 0 12px;
  border-radius: var(--bm-r-pill, 999px);
}

.nfw-primary-pill:hover {
  transform: translateY(-1px);
  filter: brightness(1.06);
}

.nfw-cand-list,
.nfw-list-scroll {
  display: flex;
  flex-direction: column;
  gap: 7px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-right: 4px;
}

.nfw-cand-item {
  background: var(--fm-card);
  color: var(--fm-text-1);
  padding: 10px 13px;
  border-radius: var(--bm-r-md, 14px);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  text-align: left;
  cursor: pointer;
  transition: transform 0.2s ease;
}

.nfw-cand-item:hover {
  transform: translateX(2px);
}

.nfw-cand-title {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 700;
  color: var(--fm-text-1);
}

.nfw-cand-sub {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--fm-text-3);
  margin-top: 2px;
}

.nfw-cand-check {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--np-primary, #3898ff);
  white-space: nowrap;
}

.nfw-empty-hint {
  padding: 36px 16px;
  text-align: center;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
}

/* ============================================================================
   4. 更多设置中心 (MoreControlHub) 与内联子视图
   ============================================================================ */
.nfw-song-hero-card {
  background: var(--fm-card);
  box-shadow: 0 10px 28px var(--np-card-ambient-glow, rgba(0, 0, 0, 0.14));
  border-radius: var(--bm-r-lg, 18px);
  padding: 13px 15px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.nfw-song-hero-top {
  display: flex;
  align-items: center;
  gap: 14px;
}

.nfw-hero-cover {
  width: 62px;
  height: 62px;
  border-radius: var(--bm-r-md, 14px);
  object-fit: cover;
  flex-shrink: 0;
  box-shadow: 0 8px 22px rgba(0, 0, 0, 0.4);
}

.nfw-hero-cover.fallback {
  background: var(--fm-chip-bg);
  display: flex;
  align-items: center;
  justify-content: center;
}

.nfw-hero-texts {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.nfw-hero-title {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: 800;
  color: var(--fm-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nfw-hero-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nfw-hero-badges {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-top: 3px;
  flex-wrap: wrap;
}

.nfw-hero-spec-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 2px 8px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
}

.nfw-hero-dsp-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 2px 8px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--np-live-soft, rgba(46, 229, 157, 0.16));
  color: var(--np-live-accent, #2ee59d);
}

.nfw-detail-pill {
  padding: 7px 14px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--np-primary-soft, rgba(56, 152, 255, 0.2));
  color: var(--np-primary, #3898ff);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  cursor: pointer;
  flex-shrink: 0;
  transition: transform 0.2s ease;
}

.nfw-detail-pill:hover {
  transform: scale(1.04);
}

.nfw-hero-jumps {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.nfw-jump-card {
  padding: 10px 13px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--fm-card-elevated);
  display: flex;
  align-items: center;
  gap: 11px;
  text-align: left;
  cursor: pointer;
  transition:
    transform 0.2s ease,
    background 0.2s ease;
}

.nfw-jump-card:hover {
  transform: translateY(-1px);
  background: var(--fm-card-hover);
}

.nfw-jump-orb {
  width: 32px;
  height: 32px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--fm-chip-bg);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.nfw-jump-meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.nfw-jump-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.nfw-jump-val {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--fm-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nfw-bento-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 9px;
}

.nfw-bento-card {
  background: var(--fm-card);
  color: var(--fm-text-1);
  padding: 11px 13px;
  border-radius: var(--bm-r-lg, 18px);
  display: flex;
  align-items: center;
  gap: 11px;
  text-align: left;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    box-shadow 0.2s ease;
}

.nfw-bento-card.is-span-full {
  grid-column: 1 / -1;
}

.nfw-bento-card:hover {
  transform: translateY(-2px);
}

.nfw-bento-icon-orb {
  width: 34px;
  height: 34px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--fm-chip-bg);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.nfw-bento-meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nfw-bento-name {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 800;
  color: var(--fm-text-1);
  white-space: nowrap;
}

.nfw-bento-desc {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  color: var(--fm-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 音质 / 倍速 / 睡眠 / 歌单 / 详情 通用卡片 */
.nfw-quality-card {
  background: var(--fm-card);
  color: var(--fm-text-1);
  padding: 11px 14px;
  border-radius: var(--bm-r-md, 14px);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  text-align: left;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    box-shadow 0.2s ease;
}

.nfw-quality-card:hover {
  transform: translateX(2px);
}

.nfw-q-head {
  display: flex;
  align-items: center;
  gap: 7px;
}

.nfw-q-title {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.nfw-q-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 2px 6px;
  border-radius: var(--bm-r-xs, 6px);
}

.nfw-q-pref-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  padding: 2px 6px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--fm-chip-bg, rgba(128, 138, 158, 0.14));
  color: var(--fm-text-2);
}

.nfw-q-desc {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--fm-text-3);
  margin-top: 2px;
}

.nfw-q-right {
  display: flex;
  align-items: center;
  gap: 9px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  flex-shrink: 0;
}

.nfw-q-check {
  font-size: var(--bm-fs-song-title, 15px);
  font-weight: 900;
  color: var(--np-primary, #3898ff);
}

.nfw-preset-row {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 6px;
}

.nfw-preset-chip {
  background: var(--fm-card-elevated);
  color: var(--fm-text-1);
  height: 32px;
  border-radius: var(--bm-r-sm, 10px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  cursor: pointer;
  transition: transform 0.2s ease, background 0.18s ease;
}

.nfw-preset-chip:hover {
  background: var(--fm-card-hover);
  transform: translateY(-1px);
}

.nfw-switch-row {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  text-align: left;
  cursor: pointer;
  padding-top: 4px;
}

.nfw-switch-title {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.nfw-switch-sub {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--fm-text-3);
  margin-top: 2px;
}

.nfw-switch-pill {
  width: 36px;
  height: 20px;
  border-radius: var(--bm-r-pill, 999px);
  padding: 2px;
  background: var(--fm-chip-hover);
  display: inline-flex;
  align-items: center;
  flex-shrink: 0;
  transition: background 0.2s ease;
}

.nfw-switch-pill.on {
  background: var(--np-primary, #3898ff);
}

.nfw-switch-thumb {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #ffffff;
  transition: transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1);
}

.nfw-switch-pill.on .nfw-switch-thumb {
  transform: translateX(16px);
}

.nfw-footer-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.nfw-ghost-btn {
  height: 34px;
  padding: 0 13px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  cursor: pointer;
  transition: background 0.2s ease;
}

.nfw-ghost-btn.sm {
  height: 30px;
  padding: 0 11px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-caption, 12.5px);
}

.nfw-section-block .nfw-ghost-btn {
  background: var(--fm-card-elevated);
}

.nfw-ghost-btn:hover {
  background: var(--fm-chip-hover);
}

.nfw-danger-pill {
  height: 34px;
  padding: 0 14px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--np-danger-soft, rgba(255, 77, 109, 0.2));
  color: var(--np-danger, #ff6b8b);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  cursor: pointer;
}

.nfw-sleep-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.nfw-sleep-card {
  background: var(--fm-card);
  color: var(--fm-text-1);
  height: 42px;
  border-radius: var(--bm-r-md, 14px);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 800;
  cursor: pointer;
  transition:
    transform 0.2s cubic-bezier(0.22, 1.35, 0.36, 1);
}

.nfw-sleep-card:hover {
  transform: translateY(-1px);
}

.nfw-detail-scroll {
  display: flex;
  flex-direction: column;
  gap: 9px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-gutter: stable;
  padding-right: 4px;
}

.nfw-detail-loading {
  padding: 7px 12px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(
    --np-card-elevated,
    var(--fm-card-elevated)
  );
  color: var(--np-primary, #3898ff);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  text-align: center;
}

.nfw-detail-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

@media (max-width: 520px) {
  .nfw-detail-grid {
    grid-template-columns: 1fr;
  }
}

.nfw-detail-list {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.nfw-detail-item {
  padding: 10px 13px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--fm-card);
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  gap: 4px;
  font-size: var(--bm-fs-caption, 12.5px);
  transition:
    background 0.18s ease,
    transform 0.18s ease;
}

.nfw-detail-item.full {
  grid-column: 1 / -1;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.nfw-detail-item.clickable {
  cursor: pointer;
}

.nfw-detail-item.clickable:hover {
  background: var(
    --np-card-hover-bg,
    var(--fm-card-elevated)
  );
}

.nfw-dk {
  color: var(--fm-text-3);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  flex-shrink: 0;
}

.nfw-dv {
  color: var(--fm-text-1);
  font-weight: 700;
  font-size: var(--bm-fs-caption, 12.5px);
  word-break: break-word;
}

.nfw-detail-item.full .nfw-dv {
  text-align: right;
  word-break: break-all;
}

.nfw-dv.accent {
  color: var(--np-primary, #3898ff);
}

.nfw-dv.mono {
  font-family: Consolas, monospace;
  font-size: var(--bm-fs-micro, 11px);
}

.nfw-credits-chips {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
}

.nfw-credit-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 9px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(
    --np-card-elevated,
    var(--fm-card-elevated)
  );
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-1);
  font-weight: 700;
}

.nfw-credit-chip em {
  font-style: normal;
  font-size: var(--bm-fs-micro, 11px);
  color: var(--np-primary, #3898ff);
  font-weight: 800;
}

/* 添加到歌单行 */
.nfw-playlist-item {
  padding: 10px 12px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--fm-card);
  display: flex;
  align-items: center;
  gap: 11px;
  text-align: left;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease;
}

.nfw-playlist-item:hover {
  transform: translateX(3px);
  background: var(
    --np-card-active-bg,
    var(--np-primary-soft, rgba(56, 152, 255, 0.18))
  );
}

.nfw-pl-orb {
  width: 38px;
  height: 38px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--fm-chip-bg);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.nfw-pl-cover {
  width: 38px;
  height: 38px;
  border-radius: var(--bm-r-sm, 10px);
  object-fit: cover;
  flex-shrink: 0;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.32);
}

.nfw-pl-meta {
  flex: 1;
  min-width: 0;
}

.nfw-pl-name {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 800;
  color: var(--fm-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nfw-pl-count {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--fm-text-3);
  margin-top: 2px;
}

.nfw-pl-add-tag {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--np-primary, #3898ff);
  padding: 4px 10px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-chip-bg);
}

/* ============================================================================
   5. 蝰蛇声学中心 (Viper Acoustic Studio · 极简现代无框化重构)
   ============================================================================ */
.vas-header {
  flex-wrap: wrap;
  padding-bottom: 6px;
}

.vas-title-block {
  min-width: 0;
}

.vas-title-line {
  display: flex;
  align-items: center;
  gap: 8px;
}

.vas-acoustic-eq {
  display: inline-flex;
  align-items: flex-end;
  gap: 2.5px;
  height: 12px;
  padding: 0 2px;
  flex-shrink: 0;
}

.vas-acoustic-eq i {
  display: block;
  width: 2.5px;
  border-radius: 999px;
  background: var(--np-primary, var(--fm-primary, var(--primary)));
  animation: vasEqWave 0.9s ease-in-out infinite alternate;
}

.vas-acoustic-eq i:nth-child(1) {
  height: 5px;
}

.vas-acoustic-eq i:nth-child(2) {
  height: 11px;
  animation-delay: 0.22s;
}

.vas-acoustic-eq i:nth-child(3) {
  height: 7px;
  animation-delay: 0.44s;
}

.vas-acoustic-eq.paused i {
  animation-play-state: paused;
  height: 4px;
  opacity: 0.55;
}

@keyframes vasEqWave {
  0% {
    transform: scaleY(0.45);
  }
  100% {
    transform: scaleY(1);
  }
}

.vas-mode-tabs {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 3px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-chip-bg);
}

.vas-mode-pill {
  height: 30px;
  padding: 0 14px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--fm-text-2);
  background: transparent;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  transition: color 0.22s cubic-bezier(0.22, 1, 0.36, 1);
}

.vas-mode-pill:hover:not(.active) {
  color: var(--fm-text-1);
}

.vas-mode-pill.active {
  color: var(--np-on-accent-primary, var(--fm-on-primary, #ffffff));
}

.vas-mode-count {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  font-variant-numeric: tabular-nums;
  padding: 1px 6px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-chip-hover);
}

.vas-mode-live-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--np-live-accent, #2ee59d);
  box-shadow: 0 0 8px var(--np-live-glow, rgba(46, 229, 157, 0.72));
  flex-shrink: 0;
}

/* 蝰蛇声学中心 L1 方向感知视图切换动效 */
.vas-pane-left-enter-active,
.vas-pane-left-leave-active,
.vas-pane-right-enter-active,
.vas-pane-right-leave-active {
  transition:
    opacity 0.22s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.24s cubic-bezier(0.22, 1, 0.36, 1);
  will-change: opacity, transform;
}

.vas-pane-left-enter-from {
  opacity: 0;
  transform: translate3d(18px, 0, 0);
}

.vas-pane-left-leave-to {
  opacity: 0;
  transform: translate3d(-12px, 0, 0);
}

.vas-pane-right-enter-from {
  opacity: 0;
  transform: translate3d(-18px, 0, 0);
}

.vas-pane-right-leave-to {
  opacity: 0;
  transform: translate3d(12px, 0, 0);
}

@media (prefers-reduced-motion: reduce) {
  .vas-pane-left-enter-active,
  .vas-pane-left-leave-active,
  .vas-pane-right-enter-active,
  .vas-pane-right-leave-active,
  .vas-acoustic-eq i {
    transition: none !important;
    animation: none !important;
  }
}

.nfw-dsp-pill {
  height: 30px;
  padding: 0 12px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  transition: all 0.22s ease;
}

.nfw-dsp-pill.on {
  background: var(--np-live-soft, rgba(46, 229, 157, 0.18));
  color: var(--np-live-accent, #2ee59d);
  box-shadow: 0 4px 14px var(--np-live-glow, rgba(46, 229, 157, 0.18));
}

.nfw-dsp-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: currentColor;
}

.nfw-ghost-btn.sm.muted {
  opacity: 0.45;
}

/* 主视图容器 */
.vas-tab-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-right: 2px;
}

/* 视图 A：单行分类筛选 + 紧凑搜索条 */
.vas-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: nowrap;
  flex-shrink: 0;
}

.vas-cat-pills {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 14px;
  flex-wrap: nowrap;
  overflow-x: auto;
  scrollbar-width: none;
}

.vas-cat-pills::-webkit-scrollbar {
  display: none;
}

.vas-cat-btn {
  height: 28px;
  padding: 0 2px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--fm-text-3);
  background: transparent;
  border: none;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  transition: color 0.2s ease;
}

.vas-cat-btn:hover {
  background: transparent;
  color: var(--fm-text-1);
}

.vas-cat-btn.active {
  background: transparent;
  color: var(--fm-text-1);
  font-weight: 800;
}

.vas-cat-num {
  font-size: var(--bm-fs-micro, 11px);
  opacity: 0.72;
  font-weight: 800;
  font-variant-numeric: tabular-nums;
}

.vas-search-box {
  height: 30px;
  padding: 0 11px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-input-bg);
  display: flex;
  align-items: center;
  gap: 6px;
  width: 184px;
  flex-shrink: 0;
}

@media (max-width: 620px) {
  .vas-toolbar {
    flex-wrap: wrap;
  }
  .vas-search-box {
    width: 100%;
  }
}

.vas-search-input {
  flex: 1;
  min-width: 0;
  background: transparent;
  color: var(--fm-text-1);
  font-size: var(--bm-fs-caption, 12.5px);
}

.vas-search-input::placeholder {
  color: var(--fm-text-3);
}

.vas-search-clear {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--fm-chip-hover);
  color: var(--fm-text-1);
  font-size: var(--bm-fs-micro, 11px);
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

/* 2列宽松大简雅预设矩阵 */
.vas-preset-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 11px;
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 2px 4px 4px 0;
}

@media (max-width: 560px) {
  .vas-preset-grid {
    grid-template-columns: 1fr;
  }
}

.vas-preset-card {
  padding: 14px 16px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--fm-card);
  color: var(--fm-text-1);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  align-items: stretch;
  gap: 9px;
  text-align: left;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    box-shadow 0.2s ease;
}

.vas-preset-card:hover {
  transform: translateY(-2px);
  background: var(--fm-card-hover);
}

.vas-card-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.vas-card-title-row {
  min-width: 0;
  flex: 1;
}

.vas-card-name {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 800;
  color: var(--fm-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: block;
}

.vas-card-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  padding: 2px 7px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-chip-bg);
  color: var(--fm-text-3);
  display: inline-flex;
  align-items: center;
  gap: 4px;
  white-space: nowrap;
  flex-shrink: 0;
}

.vas-card-badge.mounted {
  background: var(--np-live-soft, rgba(46, 229, 157, 0.16));
  color: var(--np-live-accent, #2ee59d);
  font-weight: 800;
}

.vas-live-pulse {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--np-live-accent, #2ee59d);
  box-shadow: 0 0 6px var(--np-live-glow, rgba(46, 229, 157, 0.85));
}

.vas-card-desc {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--fm-text-3);
  line-height: 1.44;
  margin: 0;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  min-height: 31px;
}

.vas-card-bottom {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 2px;
}

.vas-curve-spark {
  width: 64px;
  height: 20px;
  flex-shrink: 0;
  opacity: 0.92;
}

.vas-card-tags {
  display: flex;
  align-items: center;
  gap: 4px;
  justify-content: flex-end;
  min-width: 0;
  overflow: hidden;
}

.vas-card-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  padding: 2px 6px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
  white-space: nowrap;
}

/* ============================================================================
   视图 B：听感微调（旋钮 + 质感滑块卡片 + 空间模型）
   ============================================================================ */
.vas-tuning-body {
  gap: 9px;
}

.vas-tuning-context-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 14px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--fm-card);
  flex-shrink: 0;
}

.vas-tc-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.vas-tc-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--fm-scroll-thumb);
  flex-shrink: 0;
}

.vas-tc-dot.on {
  background: var(--np-primary, #3898ff);
  box-shadow: 0 0 8px var(--np-primary-glow, rgba(56, 152, 255, 0.6));
}

.vas-tc-text {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--fm-text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.vas-tc-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.vas-tc-btn {
  height: 26px;
  padding: 0 10px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--np-primary-soft, rgba(56, 152, 255, 0.2));
  color: var(--np-primary, #3898ff);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  cursor: pointer;
  transition: transform 0.18s ease, background 0.18s ease;
}

.vas-tc-btn:hover {
  transform: translateY(-1px);
}

.vas-tc-btn.subtle {
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
}

/* 四维核心听感直观卡片 (2x2 大、简、雅) */
.vas-core-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

@media (max-width: 600px) {
  .vas-core-grid {
    grid-template-columns: 1fr;
  }
}

.vas-core-card {
  background: var(--fm-card);
  border-radius: var(--bm-r-lg, 18px);
  padding: 11px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.vas-core-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 10px;
}

.vas-core-title {
  font-size: var(--bm-fs-body, 14px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.vas-core-desc {
  margin-top: 2px;
  font-size: var(--bm-fs-micro, 11.5px);
  color: var(--fm-text-3);
}

.vas-core-val {
  font-size: var(--bm-fs-song-title, 15px);
  font-weight: 800;
  color: var(--np-primary, var(--fm-primary));
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.vas-core-range {
  width: 100%;
}

.vas-env-card {
  gap: 8px;
}

.vas-reverb-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 7px;
}

.vas-room-card {
  height: 32px;
  padding: 0 8px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--fm-card-elevated);
  color: var(--fm-text-1);
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  cursor: pointer;
  transition:
    transform 0.2s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease;
}

.vas-room-card:hover {
  transform: translateY(-1px);
  background: var(--fm-card-hover);
}

.vas-room-title {
  font-size: var(--bm-fs-micro, 11.5px);
  font-weight: 800;
  color: inherit;
  white-space: nowrap;
}

.vas-tuner-callout {
  margin-top: auto;
  padding: 11px 14px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--fm-card-elevated);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
}

.vas-tuner-callout-copy {
  min-width: 0;
}

.vas-tuner-callout-title {
  font-size: var(--bm-fs-meta, 13px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.vas-tuner-callout-sub {
  margin-top: 2px;
  font-size: var(--bm-fs-micro, 11.5px);
  color: var(--fm-text-3);
}

.vas-tuner-jump-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 16px;
  flex-shrink: 0;
}

/* ============================================================================
   6. 灵动柔和弹簧弹出与收起动画 + 内联子视图过渡
   ============================================================================ */
.np-float-spring-enter-active {
  transition:
    opacity 0.34s cubic-bezier(0.16, 1, 0.3, 1),
    transform 0.42s cubic-bezier(0.22, 1.18, 0.36, 1);
}

.np-float-spring-leave-active {
  transition:
    opacity 0.22s cubic-bezier(0.4, 0, 1, 1),
    transform 0.24s cubic-bezier(0.4, 0, 0.6, 1);
}

.np-float-spring-enter-from {
  opacity: 0;
  transform: translate3d(0, 18px, 0) scale(0.92);
}

.np-float-spring-leave-to {
  opacity: 0;
  transform: translate3d(0, 10px, 0) scale(0.95);
}

.nfw-subview-slide-enter-active,
.nfw-subview-slide-leave-active {
  transition:
    opacity 0.2s ease,
    transform 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.nfw-subview-slide-enter-from {
  opacity: 0;
  transform: translateX(12px);
}

.nfw-subview-slide-leave-to {
  opacity: 0;
  transform: translateX(-12px);
}

/* 统一下载弹窗：4 项无框内嵌开关 + 仅保存 .lrc 底部条 + 原生 <progress> */
.nfw-dl-opts-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
  margin: 0 2px 10px;
}

.nfw-dl-opt-chip {
  min-block-size: 34px;
  padding: 6px 11px;
  border: none;
  border-radius: 12px;
  background: var(--fm-chip-bg, rgba(255, 255, 255, 0.08));
  color: var(--fm-text-2);
  font-size: 12px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  transition: background 0.18s ease, color 0.18s ease, transform 0.18s ease;
}

.nfw-dl-opt-chip:hover {
  background: var(--fm-chip-hover, rgba(255, 255, 255, 0.14));
  color: var(--fm-text-1);
}

.nfw-dl-opt-chip.is-on {
  background: color-mix(in srgb, var(--fm-accent, var(--primary)) 22%, transparent);
  color: var(--fm-text-1);
}

.nfw-dl-lyric-footer {
  margin-top: 10px;
  padding-top: 10px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.nfw-dl-lyric-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.nfw-dl-lyric-actions .nfw-primary-pill,
.nfw-dl-lyric-actions .nfw-ghost-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-block-size: 34px;
  padding: 6px 14px;
}

.nfw-dl-live-progress {
  padding: 10px 12px;
  border-radius: 14px;
  background: var(--fm-chip-bg, rgba(255, 255, 255, 0.07));
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.nfw-dl-live-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 12px;
  font-weight: 700;
  color: var(--fm-text-2);
}

.nfw-dl-live-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nfw-dl-live-pct {
  color: var(--fm-accent, var(--primary));
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}
</style>

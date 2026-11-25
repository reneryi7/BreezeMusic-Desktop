import { defineStore } from "pinia";
import { ref, computed, watch, reactive } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  formatAudioSizeBytes,
  getSongAvailableQualities,
  kugouClient,
  normalizeConceptQuality,
  normSong,
  splitSongAuthors,
} from "../api/kugou";
import type {
  AccountProfile,
  AcousticFieldSettings,
  NavigationHistoryEntry,
  SongAuthor,
  AlbumItem,
  AppColorScheme,
  AppTypographyConfig,
  AudioOutputDeviceItem,
  AudioQualityValue,
  CustomFontRecord,
  DarkThemeVariant,
  DesktopLyricClarityMode,
  DesktopLyricLayoutMode,
  DesktopLyricSnapAnchor,
  DesktopLyricStrokeColorMode,
  DesktopLyricStrokeWidth,
  DesktopLyricUnsungTone,
  DiscoverSubView,
  DownloadFileNamingMode,
  DownloadFolderOrganizeMode,
  DownloadHistoryRecord,
  DownloadProgressPayload,
  DownloadQueueItem,
  FmChannel,
  FollowedArtist,
  GatewayDiagResult,
  GlobalShortcutActionId,
  HdArtworkPreviewTarget,
  InAppShortcutActionId,
  KugouSession,
  LibraryAssetTab,
  LibraryDetailMode,
  LightThemeVariant,
  LyricAlignment,
  LyricCandidateItem,
  LyricData,
  LyricFontFamily,
  LyricFontWeightLevel,
  LyricHighlightThemeId,
  LyricSungColorMode,
  LyricThemeId,
  LyricUnsungPresetId,
  MainNavTab,
  PeqFilterType,
  PeqMemorySlot,
  PlaybackResult,
  PlayerBackgroundStyle,
  PlayerStageStyle,
  PlayMode,
  Playlist,
  PurchasedAlbumItem,
  PurchasedSongItem,
  RankItem,
  SearchCategoryTab,
  SearchDiscoveryData,
  SharedMusicResolved,
  Song,
  SongComment,
  SongDetailInfo,
  SystemFontFamilyInfo,
  VipStatusInfo,
  ViperEffectItem,
  AudiophileTargetCurve,
  ReverbPresetId,
  CoverColorRoles,
  CoverColorStageMode,
  CoverExtractedPalette,
  SettingsSectionId,
  MotionLevel,
  UiDensityMode,
  StartupHomePage,
} from "../types";
import {
  triggerFlyToQueue,
  triggerHeartBurst,
} from "../utils/microMotionFx";
import {
  containsKugouShareSignal,
  isClipboardShareTextHandled,
  markClipboardShareTextHandled,
} from "../utils/kugouShare";
import {
  ensureAppPowerVisibilityTracking,
  isAppForegroundActive,
  setNativeMainWindowVisible,
  subscribeAppPowerVisibility,
} from "../utils/windowPowerState";
import {
  createDefaultGlobalHotkeys,
  createDefaultInAppHotkeys,
  toGlobalHotkeyBindingsList,
} from "../utils/shortcuts";
import {
  AUDIOPHILE_TARGET_CURVES,
  CLASSIC_EQ_PRESETS,
  DEFAULT_TEN_BAND_HZ,
  DEFAULT_TEN_BAND_Q,
  DEFAULT_TEN_BAND_TYPES,
  FALLBACK_VIPER_PRESETS,
  buildBandPurifiedReverbImpulse,
  perceptualGainToSlider,
  sliderToPerceptualGain,
} from "../utils/audioDsp";
import {
  DEFAULT_COVER_EXTRACTED_PALETTE,
  buildLightTintedCoverPalette,
  extractCoverMeshColors,
  getThemeFallbackCoverPalette,
  lerpPalette,
  readableOnDarkSurface,
  resolveBestCoverWithMetrics,
  toCoverColorRoles,
  tuneAccentForDarkStage,
} from "../utils/coverPalette";
import {
  createBrowserDevShowcase,
  isBrowserDevPreviewEnv,
} from "../utils/devPreviewShowcase";
import {
  normalizeLyricThemeId,
  normalizeUnsungPresetId,
} from "../utils/lyricTypography";

export type MainTab = MainNavTab;

export interface RecentPlayQueueSnapshot {
  id: string;
  title: string;
  source: string;
  savedAt: number;
  savedAtText: string;
  lastPlayedAt: number;
  lastPlayedAtText: string;
  covers: string[];
  songs: Song[];
  trackCount: number;
  activeHash?: string;
}

const RECENT_QUEUES_STORAGE_KEY = "bm_recent_play_queues_v2";
const CURRENT_QUEUE_TITLE_STORAGE_KEY = "bm_current_queue_title";
export const DESKTOP_LYRIC_SNAPSHOT_STORAGE_KEY = "bm_desktop_lyric_snapshot_v2";
export const DESKTOP_LYRIC_POS_STORAGE_KEY = "bm_desktop_lyric_pos_v2";
export const MINI_PLAYER_SNAPSHOT_STORAGE_KEY = "bm_mini_player_snapshot_v1";

export function formatQueueSavedTimeText(ts: number): string {
  if (!Number.isFinite(ts) || ts <= 0) return "近期记忆";
  const diffSec = Math.floor((Date.now() - ts) / 1000);
  if (diffSec < 45) return "刚刚";
  if (diffSec < 3600) return `${Math.max(1, Math.floor(diffSec / 60))}分钟前`;
  const d = new Date(ts);
  const now = new Date();
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  if (
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate()
  ) {
    return `今天 ${hh}:${mm}`;
  }
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  if (
    d.getFullYear() === yesterday.getFullYear() &&
    d.getMonth() === yesterday.getMonth() &&
    d.getDate() === yesterday.getDate()
  ) {
    return `昨天 ${hh}:${mm}`;
  }
  const mon = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${mon}-${day} ${hh}:${mm}`;
}

import {
  NEW_SONG_AREAS,
  type NewSongAreaOption,
  createDiscoverAndFmModule,
} from "./modules/useDiscoverAndFmModule";
import { createWebAudioDspModule } from "./modules/useWebAudioDspModule";
import {
  BROWSER_PREVIEW_FALLBACK_FONTS,
  DEFAULT_TYPOGRAPHY_CONFIG,
  applyTypographyToDocument,
  broadcastTypographyConfig,
  ensureAllCustomFontsLoaded,
  ensureCustomFontLoaded,
  hydrateAndApplyTypography,
  loadPersistedTypographyConfig,
  normalizeTypographyConfig,
  persistTypographyConfig,
  resolveDesktopLyricFontOverride,
  resolveStageLyricFontOverride,
  unloadCustomFontFace,
} from "../utils/fontRuntime";
import {
  applyMotionLevelToDocument,
  applyUiDensityToDocument,
  broadcastMotionAndDensity,
  loadPersistedMotionLevel,
  loadPersistedUiDensity,
  normalizeMotionLevel,
  normalizeUiDensity,
  resolveEffectiveMotionLevel,
} from "../utils/motionRuntime";
import { createLibraryAndDetailModule } from "./modules/useLibraryAndDetailModule";
import { createSearchModule } from "./modules/useSearchModule";
import { createAutoRefreshScheduler } from "./modules/useAutoRefreshScheduler";
import {
  FOCUS_SCENE_PRESETS,
  createFocusPodModule,
} from "./modules/useFocusPodModule";
import {
  AMBIENT_CHANNELS,
  ambientSoundEngine,
} from "../utils/ambientSoundEngine";
export {
  NEW_SONG_AREAS,
  type NewSongAreaOption,
  FOCUS_SCENE_PRESETS,
  AMBIENT_CHANNELS,
};

function clampVal(n: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, n));
}

export interface QueueAnchorRect {
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
}

export interface AccountProfileOriginRect {
  left: number;
  top: number;
  width: number;
  height: number;
  src: string;
  anchorMode?: "top-right" | "card-overlay";
  cardRect?: {
    left: number;
    top: number;
    width: number;
    height: number;
  };
}

export const useAppStore = defineStore("breezeApp", () => {
  // ================= 1. 顶部天际线中枢导航与全局浮层状态 =================
  const activeTab = ref<MainNavTab>("discover");
  const settingsActiveSection = ref<SettingsSectionId>("appearance");
  function openSettingsSection(section: SettingsSectionId = "appearance") {
    const safeSection: SettingsSectionId =
      section === "downloads" && !downloadFeatureUnlocked.value
        ? "appearance"
        : section;
    settingsActiveSection.value = safeSection;
    isNowPlayingOpen.value = false;
    activeTab.value = "settings";
  }
  const isSearchPaletteOpen = ref(false);
  const isNowPlayingOpen = ref(false);
  const isQueueOpen = ref(false);
  const queueAnchorRect = ref<QueueAnchorRect | null>(null);
  const isNowPlayingQueueOpen = computed<boolean>({
    get: () => isQueueOpen.value,
    set: (v) => {
      if (!v) {
        queueAnchorRect.value = null;
      }
      isQueueOpen.value = v;
    },
  });

  function toggleNowPlayingQueue(anchorEl?: HTMLElement | null) {
    if (isQueueOpen.value) {
      isQueueOpen.value = false;
      queueAnchorRect.value = null;
      return;
    }
    if (anchorEl) {
      const rect = anchorEl.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        queueAnchorRect.value = {
          left: rect.left,
          top: rect.top,
          right: rect.right,
          bottom: rect.bottom,
          width: rect.width,
          height: rect.height,
        };
      } else {
        queueAnchorRect.value = null;
      }
    } else {
      queueAnchorRect.value = null;
    }
    isQueueOpen.value = true;
  }
  const isViperStudioModalOpen = ref(false);
  const isMiniPlayerMode = ref(false);
  const isLoginModalOpen = ref(false);
  const isVipCenterOpen = ref(false);
  const isAccountProfileOpen = ref(false);
  const accountProfileOrigin = ref<AccountProfileOriginRect | null>(null);
  const isAccountManagerOpen = ref(false);
  const isFocusPodOpen = ref(false);
  const isLyricPosterOpen = ref(false);
  const lyricPosterTargetSong = ref<Song | null>(null);
  const isCommentsModalOpen = ref(false);
  const commentsModalSong = ref<Song | null>(null);
  const isMultiArtistModalOpen = ref(false);
  const multiArtistCandidates = ref<SongAuthor[]>([]);
  const multiArtistSongTitle = ref<string>("");
  const statusBanner = ref<string>("");

  // 独立页面返回栈与历史记录
  const navHistoryStack = ref<NavigationHistoryEntry[]>([]);

  const isStandaloneDesktopLyricWindow =
    typeof window !== "undefined" &&
    (new URLSearchParams(window.location.search).get("window") === "desktop_lyric" ||
      window.location.hash.includes("desktop_lyric"));

  const isStandaloneMiniPlayerWindow =
    typeof window !== "undefined" &&
    (new URLSearchParams(window.location.search).get("window") === "mini_player" ||
      window.location.hash.includes("mini_player"));

  const isStandaloneSubWindow =
    isStandaloneDesktopLyricWindow || isStandaloneMiniPlayerWindow;

  const hasTauriBridge =
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

  // 桌面悬浮穿透歌词栏（默认纯透明无框，默认宽度 ~820px，默认字号 32px，全配置独立作用于桌面歌词）
  const defaultCompactLyricWidth = clampVal(
    Math.round((typeof window !== "undefined" && window.screen?.availWidth ? window.screen.availWidth : 1920) * 0.44),
    760,
    880
  );
  const rawSavedLyricWidth = Number(localStorage.getItem("bm_desktop_lyric_width_v2"));
  const initialDesktopLyricWidth =
    rawSavedLyricWidth >= 420 && rawSavedLyricWidth <= 1280
      ? rawSavedLyricWidth
      : defaultCompactLyricWidth;

  const desktopLyricEnabled = ref<boolean>(
    localStorage.getItem("bm_desktop_lyric") === "true"
  );
  const desktopLyricLocked = ref<boolean>(
    localStorage.getItem("bm_desktop_lyric_locked") === "true"
  );
  const desktopLyricColor = ref<string>(localStorage.getItem("bm_desktop_lyric_color") || "#5EEAD4");
  const desktopLyricColorScheme = ref<string>(
    normalizeLyricThemeId(localStorage.getItem("bm_desktop_lyric_scheme") || "dynamic_cover")
  );
  const desktopLyricSungMode = ref<LyricSungColorMode>(
    localStorage.getItem("bm_dl_sung_mode_v2") === "solid" ? "solid" : "gradient"
  );
  const desktopLyricCustomPrimary = ref<string>(
    localStorage.getItem("bm_dl_custom_primary_v2") || "#5EEAD4"
  );
  const desktopLyricCustomSecondary = ref<string>(
    localStorage.getItem("bm_dl_custom_secondary_v2") || "#60A5FA"
  );
  const desktopLyricUnsungPreset = ref<LyricUnsungPresetId>(
    normalizeUnsungPresetId(
      localStorage.getItem("bm_dl_unsung_preset_v2") ||
        localStorage.getItem("bm_desktop_lyric_unsung_tone") ||
        "auto"
    )
  );
  const desktopLyricUnsungCustomHex = ref<string>(
    localStorage.getItem("bm_dl_unsung_custom_v2") || "#E6ECF5"
  );
  const rawSavedDlUnsungOp = Number(localStorage.getItem("bm_dl_unsung_opacity_v2"));
  const desktopLyricUnsungOpacity = ref<number>(
    Number.isFinite(rawSavedDlUnsungOp) && rawSavedDlUnsungOp >= 0.35 && rawSavedDlUnsungOp <= 0.98
      ? Number(rawSavedDlUnsungOp.toFixed(2))
      : 0.78
  );
  const rawSavedDlStrokeW = localStorage.getItem("bm_dl_stroke_width_v2");
  const desktopLyricStrokeWidth = ref<DesktopLyricStrokeWidth>(
    rawSavedDlStrokeW === "none" ||
      rawSavedDlStrokeW === "thin" ||
      rawSavedDlStrokeW === "medium" ||
      rawSavedDlStrokeW === "bold"
      ? rawSavedDlStrokeW
      : "medium"
  );
  const rawSavedDlStrokeC = localStorage.getItem("bm_dl_stroke_color_mode_v2");
  const desktopLyricStrokeColorMode = ref<DesktopLyricStrokeColorMode>(
    rawSavedDlStrokeC === "auto" ||
      rawSavedDlStrokeC === "black" ||
      rawSavedDlStrokeC === "white" ||
      rawSavedDlStrokeC === "custom"
      ? rawSavedDlStrokeC
      : "auto"
  );
  const desktopLyricStrokeCustomHex = ref<string>(
    localStorage.getItem("bm_dl_stroke_custom_v2") || "#090D16"
  );
  const desktopLyricShadowEnabled = ref<boolean>(
    localStorage.getItem("bm_dl_shadow_enabled_v2") !== "false"
  );
  const rawSavedDlSize = Number(localStorage.getItem("bm_desktop_lyric_size_v2"));
  const desktopLyricFontSize = ref<number>(
    Number.isFinite(rawSavedDlSize) && rawSavedDlSize >= 18 && rawSavedDlSize <= 56
      ? rawSavedDlSize
      : 32
  );
  const desktopLyricWidth = ref<number>(
    clampVal(initialDesktopLyricWidth, 440, 1280)
  );
  const desktopLyricX = ref<number | null>(
    localStorage.getItem("bm_desktop_lyric_x") !== null
      ? Number(localStorage.getItem("bm_desktop_lyric_x"))
      : null
  );
  const desktopLyricY = ref<number | null>(
    localStorage.getItem("bm_desktop_lyric_y") !== null
      ? Number(localStorage.getItem("bm_desktop_lyric_y"))
      : null
  );
  const desktopLyricShowTranslation = ref<boolean>(
    localStorage.getItem("bm_desktop_lyric_trans") !== "false"
  );
  const rawSavedDlLayout = localStorage.getItem("bm_desktop_lyric_layout_v2");
  const desktopLyricLayoutMode = ref<DesktopLyricLayoutMode>(
    rawSavedDlLayout === "center" ||
      rawSavedDlLayout === "ktv" ||
      rawSavedDlLayout === "single" ||
      rawSavedDlLayout === "left"
      ? rawSavedDlLayout
      : localStorage.getItem("bm_desktop_lyric_line_mode") === "single"
      ? "single"
      : "center"
  );
  const desktopLyricLineMode = computed<"double" | "single">({
    get: () => (desktopLyricLayoutMode.value === "single" ? "single" : "double"),
    set: (v) => {
      desktopLyricLayoutMode.value = v === "single" ? "single" : "center";
    },
  });
  // 桌面歌词常态永久透明浮空（已移除底纹选项）
  const desktopLyricBgStyle = ref<"transparent" | "frost" | "dim" | "theme">("transparent");
  const rawSavedDlClarity = localStorage.getItem("bm_desktop_lyric_clarity_v2");
  const desktopLyricClarity = ref<DesktopLyricClarityMode>(
    rawSavedDlClarity === "soft" ||
      rawSavedDlClarity === "crisp" ||
      rawSavedDlClarity === "bold" ||
      rawSavedDlClarity === "ink_light"
      ? rawSavedDlClarity
      : "crisp"
  );
  const rawSavedDlFont = localStorage.getItem("bm_desktop_lyric_font_family");
  const desktopLyricFontFamily = ref<LyricFontFamily>(
    rawSavedDlFont === "sans" ||
      rawSavedDlFont === "serif" ||
      rawSavedDlFont === "rounded" ||
      rawSavedDlFont === "editorial"
      ? rawSavedDlFont
      : "sans"
  );
  const rawSavedDlWeight = Number(localStorage.getItem("bm_desktop_lyric_font_weight"));
  const desktopLyricFontWeight = ref<LyricFontWeightLevel>(
    rawSavedDlWeight === 500 ||
      rawSavedDlWeight === 650 ||
      rawSavedDlWeight === 760 ||
      rawSavedDlWeight === 880
      ? (rawSavedDlWeight as LyricFontWeightLevel)
      : 760
  );
  const desktopLyricWordByWord = ref<boolean>(
    localStorage.getItem("bm_desktop_lyric_krc") !== "false"
  );
  const rawSavedDlUnsung = localStorage.getItem("bm_desktop_lyric_unsung_tone");
  const desktopLyricUnsungTone = ref<DesktopLyricUnsungTone>(
    rawSavedDlUnsung === "snow" ||
      rawSavedDlUnsung === "warm" ||
      rawSavedDlUnsung === "ink"
      ? rawSavedDlUnsung
      : "snow"
  );
  const rawSavedDlSpacing = Number(localStorage.getItem("bm_desktop_lyric_line_spacing"));
  const desktopLyricLineSpacing = ref<number>(
    Number.isFinite(rawSavedDlSpacing) && rawSavedDlSpacing >= 0 && rawSavedDlSpacing <= 16
      ? Math.round(rawSavedDlSpacing)
      : 6
  );
  const rawSavedDlGlow = Number(localStorage.getItem("bm_desktop_lyric_glow_v2"));
  const desktopLyricGlowIntensity = ref<number>(
    Number.isFinite(rawSavedDlGlow) && rawSavedDlGlow >= 0 && rawSavedDlGlow <= 150
      ? Math.round(rawSavedDlGlow)
      : 90
  );
  const rawSavedDlOpacity = Number(
    localStorage.getItem("bm_desktop_lyric_opacity")
  );
  const desktopLyricOpacity = ref<number>(
    Number.isFinite(rawSavedDlOpacity) &&
      rawSavedDlOpacity >= 0.35 &&
      rawSavedDlOpacity <= 1.0
      ? Number(rawSavedDlOpacity.toFixed(2))
      : 1.0
  );
  const desktopLyricIdleFade = ref<boolean>(
    localStorage.getItem("bm_desktop_lyric_idle_fade") === "true"
  );
  const desktopLyricShowIntro = ref<boolean>(
    localStorage.getItem("bm_desktop_lyric_intro") !== "false"
  );
  const desktopLyricShowPhonetic = ref<boolean>(
    localStorage.getItem("bm_desktop_lyric_phonetic") === "true"
  );
  const activeHighResCoverUrl = ref<string>("");
  const activeHighResCoverMetrics = ref<{
    width: number;
    height: number;
  } | null>(null);

  // 全局右键上下文菜单与歌曲详情/加歌单弹窗
  const contextMenu = reactive<{
    visible: boolean;
    x: number;
    y: number;
    song: Song | null;
  }>({
    visible: false,
    x: 0,
    y: 0,
    song: null,
  });
  const songDetailModalSong = ref<Song | null>(null);
  const activeSongDetailInfo = ref<SongDetailInfo | null>(null);
  const currentSongDetailInfo = ref<SongDetailInfo | null>(null);
  const isLoadingSongDetail = ref<boolean>(false);
  const isLoadingCurrentSongDetail = ref<boolean>(false);
  const addToPlaylistModalSong = ref<Song | null>(null);
  const downloadModalSong = ref<Song | null>(null);
  const shareModalSong = ref<Song | null>(null);
  const shareModalPlaylist = ref<Playlist | null>(null);
  const detectedSharedTarget = ref<SharedMusicResolved | null>(null);
  const hdArtworkPreviewUrl = ref<string | null>(null);
  const hdArtworkPreviewTarget = ref<HdArtworkPreviewTarget | null>(null);

  function openShareSongModal(song?: Song | null) {
    if (!song) return;
    shareModalPlaylist.value = null;
    shareModalSong.value = { ...song };
  }

  function openSharePlaylistModal(playlist?: Playlist | null) {
    if (!playlist) return;
    shareModalSong.value = null;
    shareModalPlaylist.value = { ...playlist };
  }

  function closeShareModal() {
    shareModalSong.value = null;
    shareModalPlaylist.value = null;
  }

  function dismissDetectedSharedTarget() {
    detectedSharedTarget.value = null;
  }

  function openHdArtworkPreview(
    input: Song | HdArtworkPreviewTarget | string | null | undefined,
    overrideUrl?: string | null,
    initialMetrics?: { width: number; height: number } | null
  ) {
    if (!input) return;
    if (typeof input === "string") {
      const url = input.trim();
      if (!url) return;
      hdArtworkPreviewTarget.value = {
        url,
        initialMetrics: initialMetrics ? { ...initialMetrics } : null,
      };
      hdArtworkPreviewUrl.value = url;
      return;
    }
    if ("url" in input && typeof input.url === "string") {
      const url = (overrideUrl || input.url || "").trim();
      if (!url) return;
      hdArtworkPreviewTarget.value = {
        ...input,
        url,
        song: input.song ? { ...input.song } : null,
        initialMetrics:
          initialMetrics !== undefined
            ? initialMetrics
              ? { ...initialMetrics }
              : null
            : input.initialMetrics || null,
      };
      hdArtworkPreviewUrl.value = url;
      return;
    }
    const song = input as Song;
    const rawUrl = (
      overrideUrl ||
      song.coverUrl ||
      song.rawCoverTemplate ||
      ""
    ).trim();
    if (!rawUrl) return;
    const upgradedUrl = rawUrl.includes("/400/")
      ? rawUrl.replace("/400/", "/1000/")
      : rawUrl;
    hdArtworkPreviewTarget.value = {
      url: upgradedUrl,
      title: song.title || "专辑封面预览",
      artist: song.artist || "高清封面画廊",
      album: song.album || "",
      rawCoverTemplate: song.rawCoverTemplate || song.coverUrl || upgradedUrl,
      hdCoverCandidates: Array.isArray(song.hdCoverCandidates)
        ? [...song.hdCoverCandidates]
        : [],
      song: { ...song },
      initialMetrics: initialMetrics ? { ...initialMetrics } : null,
    };
    hdArtworkPreviewUrl.value = upgradedUrl;
  }

  function closeHdArtworkPreview() {
    hdArtworkPreviewTarget.value = null;
    hdArtworkPreviewUrl.value = null;
  }

  function openLyricPosterModal(song?: Song | null) {
    const target = song || currentSong.value;
    lyricPosterTargetSong.value = target ? { ...target } : null;
    isLyricPosterOpen.value = true;
  }

  function closeLyricPosterModal() {
    isLyricPosterOpen.value = false;
  }

  // ================= 5. 全局设置与播放/歌词/DSP 配置 =================
  // 明暗双主题（D1/D2 深色外观 abyssal_dark / L1/L2 浅色外观 watercolor_light / 跟随系统 system）
  const rawSavedScheme = localStorage.getItem("bm_color_scheme_kugou_v2") || "watercolor_light";
  const initialColorScheme: AppColorScheme =
    rawSavedScheme === "watercolor_light" || rawSavedScheme === "light"
      ? "watercolor_light"
      : rawSavedScheme === "system"
      ? "system"
      : "abyssal_dark";
  const colorScheme = ref<AppColorScheme>(initialColorScheme);

  const rawSavedLightVariant =
    localStorage.getItem("bm_light_theme_variant_v1") || "watercolor";
  const initialLightVariant: LightThemeVariant =
    rawSavedLightVariant === "nordic_oat" ? "nordic_oat" : "watercolor";
  const lightThemeVariant = ref<LightThemeVariant>(initialLightVariant);

  const rawSavedDarkVariant =
    localStorage.getItem("bm_dark_theme_variant_v1") || "obsidian_aurora";
  const initialDarkVariant: DarkThemeVariant =
    rawSavedDarkVariant === "velvet_nebula" ? "velvet_nebula" : "obsidian_aurora";
  const darkThemeVariant = ref<DarkThemeVariant>(initialDarkVariant);

  const LIGHT_THEME_VARIANT_OPTIONS: ReadonlyArray<{
    id: LightThemeVariant;
    shortBadge: string;
    name: string;
    subtitle: string;
  }> = [
    {
      id: "watercolor",
      shortBadge: "L1 水彩",
      name: "L1 棉纸水彩浸润",
      subtitle: "清透云宣白 #F6F8FC · 樱粉天青湿水彩 + 晴空鸢尾冰霜玻璃",
    },
    {
      id: "nordic_oat",
      shortBadge: "L2 燕麦",
      name: "L2 晨雾燕麦与矿物纸感",
      subtitle: "烘焙燕麦暖纸 #E8E0D0 · 北欧矿物青玉 (#0F766E) + 三阶手抄纸层",
    },
  ];

  const DARK_THEME_VARIANT_OPTIONS: ReadonlyArray<{
    id: DarkThemeVariant;
    shortBadge: string;
    name: string;
    subtitle: string;
  }> = [
    {
      id: "obsidian_aurora",
      shortBadge: "D1 曜石",
      name: "D1 黑曜石与暗夜极光",
      subtitle: "极夜冷曜 #0C0E14 · 极光蓝青靛主调 + 纯冷冰川青绿散景",
    },
    {
      id: "velvet_nebula",
      shortBadge: "D2 星云",
      name: "D2 丝绒暗紫与星云流光",
      subtitle: "微醺苦巧天鹅绒 #1A1116 · 落日玫瑰×琥珀流金主调 + 香槟暖字",
    },
  ];

  const lightThemeVariantMeta = computed(
    () =>
      LIGHT_THEME_VARIANT_OPTIONS.find((item) => item.id === lightThemeVariant.value) ||
      LIGHT_THEME_VARIANT_OPTIONS[0]
  );

  const darkThemeVariantMeta = computed(
    () =>
      DARK_THEME_VARIANT_OPTIONS.find((item) => item.id === darkThemeVariant.value) ||
      DARK_THEME_VARIANT_OPTIONS[0]
  );

  const systemPrefersDark = ref<boolean>(
    typeof window !== "undefined" && window.matchMedia
      ? window.matchMedia("(prefers-color-scheme: dark)").matches
      : true
  );
  if (typeof window !== "undefined" && window.matchMedia) {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    mq.addEventListener?.("change", (ev) => {
      systemPrefersDark.value = ev.matches;
    });
  }

  const resolvedColorScheme = computed<"abyssal_dark" | "watercolor_light">(() => {
    if (colorScheme.value === "system") {
      return systemPrefersDark.value ? "abyssal_dark" : "watercolor_light";
    }
    return colorScheme.value === "watercolor_light" ? "watercolor_light" : "abyssal_dark";
  });

  const isDarkTheme = computed(() => resolvedColorScheme.value === "abyssal_dark");

  function setLightThemeVariant(variant: LightThemeVariant, switchToMode = true) {
    lightThemeVariant.value = variant;
    if (switchToMode && resolvedColorScheme.value !== "watercolor_light") {
      colorScheme.value = "watercolor_light";
    }
  }

  function setDarkThemeVariant(variant: DarkThemeVariant, switchToMode = true) {
    darkThemeVariant.value = variant;
    if (switchToMode && resolvedColorScheme.value !== "abyssal_dark") {
      colorScheme.value = "abyssal_dark";
    }
  }

  function cycleLightThemeVariant() {
    const next: LightThemeVariant =
      lightThemeVariant.value === "watercolor" ? "nordic_oat" : "watercolor";
    setLightThemeVariant(next);
    return lightThemeVariantMeta.value;
  }

  function cycleDarkThemeVariant() {
    const next: DarkThemeVariant =
      darkThemeVariant.value === "obsidian_aurora" ? "velvet_nebula" : "obsidian_aurora";
    setDarkThemeVariant(next);
    return darkThemeVariantMeta.value;
  }

  function cycleCurrentModeVariant() {
    if (resolvedColorScheme.value === "watercolor_light") {
      return cycleLightThemeVariant();
    }
    return cycleDarkThemeVariant();
  }

  function syncDocumentThemeAttr() {
    if (typeof document === "undefined") return;
    const isLight = resolvedColorScheme.value === "watercolor_light";
    const isSubOverlayWindow =
      typeof window !== "undefined" &&
      (window.location.search.includes("window=desktop_lyric") ||
        window.location.search.includes("window=mini_player"));
    const bgHex = isLight
      ? lightThemeVariant.value === "nordic_oat"
        ? "#E8E0D0"
        : "#F6F8FC"
      : darkThemeVariant.value === "velvet_nebula"
      ? "#1A1116"
      : "#0C0E14";
    const bgRgb: [number, number, number] = isLight
      ? lightThemeVariant.value === "nordic_oat"
        ? [232, 224, 208]
        : [246, 248, 252]
      : darkThemeVariant.value === "velvet_nebula"
      ? [26, 17, 22]
      : [12, 14, 20];

    document.documentElement.dataset.theme = isLight ? "watercolor_light" : "abyssal_dark";
    document.documentElement.dataset.lightVariant = lightThemeVariant.value;
    document.documentElement.dataset.darkVariant = darkThemeVariant.value;
    document.documentElement.style.colorScheme = isLight ? "light" : "dark";
    document.documentElement.style.backgroundColor = isSubOverlayWindow ? "transparent" : bgHex;

    const hasTauriRuntime =
      typeof window !== "undefined" &&
      ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);
    if (!isSubOverlayWindow && hasTauriRuntime) {
      void invoke("sync_main_window_bg_color_cmd", {
        r: bgRgb[0],
        g: bgRgb[1],
        b: bgRgb[2],
      }).catch(() => {});
    }
  }
  syncDocumentThemeAttr();

  const motionLevel = ref<MotionLevel>(loadPersistedMotionLevel());
  const uiDensity = ref<UiDensityMode>(loadPersistedUiDensity());
  applyMotionLevelToDocument(motionLevel.value);
  applyUiDensityToDocument(uiDensity.value);
  if (
    !isStandaloneSubWindow &&
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  ) {
    void kugouClient
      .syncMotionLevel(resolveEffectiveMotionLevel(motionLevel.value))
      .catch(() => {});
  }

  function setMotionLevel(level: MotionLevel) {
    const next = normalizeMotionLevel(level);
    motionLevel.value = next;
    applyMotionLevelToDocument(next);
    broadcastMotionAndDensity(next, uiDensity.value);
    if (
      !isStandaloneSubWindow &&
      typeof window !== "undefined" &&
      ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
    ) {
      void kugouClient
        .syncMotionLevel(resolveEffectiveMotionLevel(next))
        .catch(() => {});
    }
    if (isMiniPlayerMode.value) {
      broadcastMiniPlayerState();
    }
  }

  function setUiDensity(density: UiDensityMode) {
    const next = normalizeUiDensity(density);
    uiDensity.value = next;
    applyUiDensityToDocument(next);
    broadcastMotionAndDensity(motionLevel.value, next);
  }

  const rawStartupHome = localStorage.getItem("bm_startup_home_v1");
  const startupHomePage = ref<StartupHomePage>(
    rawStartupHome === "library" ||
      rawStartupHome === "local_music" ||
      rawStartupHome === "last"
      ? rawStartupHome
      : "discover"
  );
  const resetPlaybackRateOnTrackChange = ref<boolean>(
    localStorage.getItem("bm_reset_speed_on_track_v1") === "true"
  );
  const startupSafeVolumeEnabled = ref<boolean>(
    localStorage.getItem("bm_startup_safe_vol_enabled_v1") === "true"
  );
  const rawSafeVolLimit = Number(
    localStorage.getItem("bm_startup_safe_vol_limit_v1") || "0.5"
  );
  const startupSafeVolumeLimit = ref<number>(
    Number.isFinite(rawSafeVolLimit)
      ? clampVal(Number(rawSafeVolLimit.toFixed(2)), 0.1, 0.9)
      : 0.5
  );

  const autoClaimVipOnStartup = ref<boolean>(
    localStorage.getItem("bm_auto_claim_vip") !== "false"
  );
  const autoPlayOnStartup = ref<boolean>(
    localStorage.getItem("bm_auto_play_on_startup_v1") === "true"
  );
  const closeToTray = ref<boolean>(
    localStorage.getItem("bm_close_to_tray") !== "false"
  );
  const miniYieldFullscreenEnabled = ref<boolean>(
    localStorage.getItem("bm_mini_yield_fullscreen_v1") !== "false"
  );
  const clipboardShareDetectEnabled = ref<boolean>(
    localStorage.getItem("bm_clipboard_share_detect_v1") !== "false"
  );
  const autostartEnabled = ref<boolean>(false);
  const isUpdatingAutostart = ref<boolean>(false);
  const osGlobalHotkeysEnabled = ref<boolean>(
    localStorage.getItem("bm_os_global_hotkeys") !== "false"
  );
  const isRecordingShortcut = ref<boolean>(false);
  const globalHotkeyFailedActions = ref<string[]>([]);

  function loadSavedGlobalHotkeys(): Record<GlobalShortcutActionId, string> {
    const defaults = createDefaultGlobalHotkeys();
    try {
      const raw = localStorage.getItem("bm_global_hotkeys_v1");
      if (!raw) return defaults;
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object") return defaults;
      for (const k of Object.keys(defaults) as GlobalShortcutActionId[]) {
        if (typeof parsed[k] === "string") {
          defaults[k] = parsed[k].trim();
        }
      }
      return defaults;
    } catch {
      return defaults;
    }
  }

  function loadSavedInAppHotkeys(): Record<InAppShortcutActionId, string> {
    const defaults = createDefaultInAppHotkeys();
    try {
      const raw = localStorage.getItem("bm_in_app_hotkeys_v1");
      if (!raw) return defaults;
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object") return defaults;
      for (const k of Object.keys(defaults) as InAppShortcutActionId[]) {
        if (typeof parsed[k] === "string") {
          defaults[k] = parsed[k].trim();
        }
      }
      return defaults;
    } catch {
      return defaults;
    }
  }

  const globalHotkeys = ref<Record<GlobalShortcutActionId, string>>(
    loadSavedGlobalHotkeys()
  );
  const inAppHotkeys = ref<Record<InAppShortcutActionId, string>>(
    loadSavedInAppHotkeys()
  );

  async function syncGlobalHotkeysToOs(showReportToast = false) {
    try {
      const report = await kugouClient.updateGlobalHotkeys(
        toGlobalHotkeyBindingsList(globalHotkeys.value)
      );
      globalHotkeyFailedActions.value = Array.isArray(report?.failedActions)
        ? report.failedActions
        : [];
      if (showReportToast && globalHotkeyFailedActions.value.length > 0 && osGlobalHotkeysEnabled.value) {
        notify("部分全局热键可能被系统或其他软件占用，已在快捷键列表标出");
      }
      return report;
    } catch {
      return null;
    }
  }

  async function setGlobalHotkey(
    action: GlobalShortcutActionId,
    accelerator: string
  ) {
    const clean = (accelerator || "").trim();
    const next = { ...globalHotkeys.value };
    if (clean) {
      const upper = clean.toUpperCase();
      for (const k of Object.keys(next) as GlobalShortcutActionId[]) {
        if (k !== action && (next[k] || "").trim().toUpperCase() === upper) {
          next[k] = "";
        }
      }
    }
    next[action] = clean;
    globalHotkeys.value = next;
    try {
      localStorage.setItem("bm_global_hotkeys_v1", JSON.stringify(next));
    } catch {
      // ignore storage quota error
    }
    const report = await syncGlobalHotkeysToOs(false);
    if (clean && report?.failedActions?.includes(action)) {
      notify(`热键「${clean}」注册失败，可能已被其他程序占用，请尝试其他组合`);
    } else if (clean) {
      notify(`已更新全局热键：${clean}`);
    } else {
      notify("已清空该项全局热键绑定");
    }
  }

  function setInAppHotkey(action: InAppShortcutActionId, accelerator: string) {
    const clean = (accelerator || "").trim();
    const next = { ...inAppHotkeys.value };
    if (clean) {
      const upper = clean.toUpperCase();
      for (const k of Object.keys(next) as InAppShortcutActionId[]) {
        if (k !== action && (next[k] || "").trim().toUpperCase() === upper) {
          next[k] = "";
        }
      }
    }
    next[action] = clean;
    inAppHotkeys.value = next;
    try {
      localStorage.setItem("bm_in_app_hotkeys_v1", JSON.stringify(next));
    } catch {
      // ignore storage quota error
    }
    notify(clean ? `已更新应用内快捷键：${clean}` : "已清空该项应用内快捷键");
  }

  async function resetGlobalHotkeys() {
    globalHotkeys.value = createDefaultGlobalHotkeys();
    try {
      localStorage.setItem(
        "bm_global_hotkeys_v1",
        JSON.stringify(globalHotkeys.value)
      );
    } catch {
      // ignore
    }
    await syncGlobalHotkeysToOs(true);
    notify("已恢复默认系统级全局热键");
  }

  function resetInAppHotkeys() {
    inAppHotkeys.value = createDefaultInAppHotkeys();
    try {
      localStorage.setItem(
        "bm_in_app_hotkeys_v1",
        JSON.stringify(inAppHotkeys.value)
      );
    } catch {
      // ignore
    }
    notify("已恢复默认应用内快捷键");
  }

  async function resetAllHotkeys() {
    globalHotkeys.value = createDefaultGlobalHotkeys();
    inAppHotkeys.value = createDefaultInAppHotkeys();
    try {
      localStorage.setItem(
        "bm_global_hotkeys_v1",
        JSON.stringify(globalHotkeys.value)
      );
      localStorage.setItem(
        "bm_in_app_hotkeys_v1",
        JSON.stringify(inAppHotkeys.value)
      );
    } catch {
      // ignore
    }
    await syncGlobalHotkeysToOs(true);
    notify("已恢复全部默认快捷键");
  }

  async function refreshAutostartState() {
    try {
      autostartEnabled.value = await kugouClient.getAutostartEnabled();
    } catch {
      autostartEnabled.value = false;
    }
  }

  async function setAutostartEnabled(enabled: boolean) {
    if (isUpdatingAutostart.value) return;
    isUpdatingAutostart.value = true;
    try {
      const actual = await kugouClient.setAutostartEnabled(enabled);
      autostartEnabled.value = actual;
      notify(
        actual
          ? "已开启 Windows 开机自启动"
          : "已关闭 Windows 开机自启动"
      );
    } catch (e: any) {
      notify(`设置开机自启动失败：${String(e)}`);
      await refreshAutostartState();
    } finally {
      isUpdatingAutostart.value = false;
    }
  }

  // 播放页与歌词页内部专属设置（1:1 对齐 Android LandscapePlayerPage & NowPlayingDialogs）
  const rawSavedStage = localStorage.getItem("bm_stage_style") || "vinyl_turntable";
  const playerStageStyle = ref<PlayerStageStyle>(
    rawSavedStage === "gallery_poster" ? "gallery_poster" : "vinyl_turntable"
  );
  const rawSavedBgStyle = localStorage.getItem("bm_bg_style") || "fluid_aurora";
  const normalizeSavedBgStyle = (raw: string): PlayerBackgroundStyle => {
    if (
      raw === "fluid_aurora" ||
      raw === "rhythm_fluted" ||
      raw === "rhythm_cymatic" ||
      raw === "starlight_nebula"
    ) {
      return raw;
    }
    if (raw === "rhythm_halo") return "rhythm_fluted";
    return "fluid_aurora";
  };
  const playerBgStyle = ref<PlayerBackgroundStyle>(
    normalizeSavedBgStyle(rawSavedBgStyle)
  );
  const rawSavedColorStageMode = localStorage.getItem(
    "bm_cover_color_stage_mode_v1"
  );
  const initialColorStageMode: CoverColorStageMode =
    rawSavedColorStageMode === "off" ||
    rawSavedColorStageMode === "follow" ||
    rawSavedColorStageMode === "dark_lock"
      ? rawSavedColorStageMode
      : localStorage.getItem("bm_dynamic_cover_accent") === "false"
      ? "off"
      : "follow";
  const coverColorStageMode = ref<CoverColorStageMode>(initialColorStageMode);
  const dynamicCoverAccentEnabled = computed<boolean>({
    get: () => coverColorStageMode.value !== "off",
    set: (enabled: boolean) => {
      if (!enabled) {
        coverColorStageMode.value = "off";
      } else if (coverColorStageMode.value === "off") {
        coverColorStageMode.value = "follow";
      }
    },
  });
  function cycleCoverColorStageMode() {
    const cur = coverColorStageMode.value;
    coverColorStageMode.value =
      cur === "off" ? "follow" : cur === "follow" ? "dark_lock" : "off";
  }
  const rawSavedLyricTheme =
    localStorage.getItem("bm_lyric_theme") || "dynamic_cover";
  const lyricTheme = ref<LyricHighlightThemeId>(
    normalizeLyricThemeId(rawSavedLyricTheme)
  );
  const lyricSungMode = ref<LyricSungColorMode>(
    localStorage.getItem("bm_lyric_sung_mode_v2") === "solid" ? "solid" : "gradient"
  );
  const lyricCustomPrimaryHex = ref<string>(
    localStorage.getItem("bm_lyric_custom_primary_v2") || "#5EEAD4"
  );
  const lyricCustomSecondaryHex = ref<string>(
    localStorage.getItem("bm_lyric_custom_secondary_v2") || "#60A5FA"
  );
  const lyricUnsungPreset = ref<LyricUnsungPresetId>(
    normalizeUnsungPresetId(localStorage.getItem("bm_lyric_unsung_preset_v2") || "auto")
  );
  const lyricUnsungCustomHex = ref<string>(
    localStorage.getItem("bm_lyric_unsung_custom_v2") || "#CBD5E1"
  );
  const rawSavedLetterSpacing = Number(localStorage.getItem("bm_lyric_letter_spacing_v2"));
  const lyricLetterSpacing = ref<number>(
    Number.isFinite(rawSavedLetterSpacing) && rawSavedLetterSpacing >= -1 && rawSavedLetterSpacing <= 6
      ? Number(rawSavedLetterSpacing.toFixed(1))
      : 0
  );
  const rawSavedFocusRatio = Number(localStorage.getItem("bm_lyric_focus_scale_ratio_v2"));
  const lyricFocusScaleRatio = ref<number>(
    Number.isFinite(rawSavedFocusRatio) && rawSavedFocusRatio >= 1.0 && rawSavedFocusRatio <= 1.24
      ? Number(rawSavedFocusRatio.toFixed(2))
      : 1.08
  );
  const lyricFontSize = ref<number>(
    clampVal(Number(localStorage.getItem("bm_lyric_font_size_v2")) || 28, 20, 48)
  );
  watch(lyricFontSize, (val) => {
    localStorage.setItem("bm_lyric_font_size_v2", String(val));
  });
  const rawSavedLyricWeight = Number(localStorage.getItem("bm_lyric_font_weight"));
  const lyricFontWeight = ref<LyricFontWeightLevel>(
    rawSavedLyricWeight === 500 ||
      rawSavedLyricWeight === 650 ||
      rawSavedLyricWeight === 760 ||
      rawSavedLyricWeight === 880
      ? (rawSavedLyricWeight as LyricFontWeightLevel)
      : 760
  );
  const lyricTranslationScale = ref<number>(
    clampVal(
      Number(localStorage.getItem("bm_lyric_trans_scale")) || 0.84,
      0.65,
      0.95
    )
  );
  const rawSavedSpacing = Number(localStorage.getItem("bm_lyric_line_spacing"));
  const lyricLineSpacing = ref<number>(
    Number.isFinite(rawSavedSpacing) && rawSavedSpacing >= 6
      ? clampVal(rawSavedSpacing, 8, 28)
      : 16
  );
  const lyricAlignment = ref<LyricAlignment>(
    (localStorage.getItem("bm_lyric_align") as LyricAlignment) || "left"
  );
  const lyricBlurEnabled = ref<boolean>(
    localStorage.getItem("bm_lyric_blur") !== "false"
  );
  const rawSavedBlurInt = Number(localStorage.getItem("bm_lyric_blur_int"));
  const lyricBlurIntensity = ref<number>(
    Number.isFinite(rawSavedBlurInt) &&
      rawSavedBlurInt >= 0.4 &&
      rawSavedBlurInt <= 1.8
      ? Number(rawSavedBlurInt.toFixed(2))
      : 1.0
  );
  const rawSavedBgDim = Number(localStorage.getItem("bm_lyric_bg_dim"));
  const lyricBackgroundDim = ref<number>(
    Number.isFinite(rawSavedBgDim) && rawSavedBgDim >= 0.4 && rawSavedBgDim <= 0.92
      ? Number(rawSavedBgDim.toFixed(2))
      : 0.72
  );
  const lyricWordByWord = ref<boolean>(
    localStorage.getItem("bm_lyric_krc") !== "false"
  );
  const lyricWordBounceEnabled = ref<boolean>(
    localStorage.getItem("bm_lyric_word_bounce") !== "false"
  );
  const lyricFocusScaleEnabled = ref<boolean>(
    localStorage.getItem("bm_lyric_focus_scale") !== "false"
  );
  const lyricTranslationTintEnabled = ref<boolean>(
    localStorage.getItem("bm_lyric_trans_tint") !== "false"
  );
  const lyricInterludeDotsEnabled = ref<boolean>(
    localStorage.getItem("bm_lyric_interlude_dots") !== "false"
  );
  const lyricClickSeekEnabled = ref<boolean>(
    localStorage.getItem("bm_lyric_click_seek") !== "false"
  );
  const rawSavedAutoReturn = Number(localStorage.getItem("bm_lyric_auto_return_sec"));
  const lyricAutoReturnSec = ref<2 | 3 | 5>(
    rawSavedAutoReturn === 2 || rawSavedAutoReturn === 3 || rawSavedAutoReturn === 5
      ? (rawSavedAutoReturn as 2 | 3 | 5)
      : 3
  );
  const lyricOffsetMs = ref<number>(
    Number(localStorage.getItem("bm_lyric_offset_ms")) || 0
  );
  const bluetoothDelayMs = ref<number>(
    Number(localStorage.getItem("bm_bt_delay_ms")) || 0
  );
  const showTranslation = ref<boolean>(true);
  const showPhonetic = ref<boolean>(false);
  const lyricFontFamily = ref<LyricFontFamily>(
    ((localStorage.getItem("bm_lyric_font_family") as LyricFontFamily) || "sans")
  );
  const rawSavedGlow = Number(localStorage.getItem("bm_lyric_glow_int"));
  const lyricGlowIntensity = ref<number>(
    Number.isFinite(rawSavedGlow) && rawSavedGlow >= 0 && rawSavedGlow <= 200
      ? Math.round(rawSavedGlow)
      : 100
  );
  const rawSavedInactiveOp = Number(localStorage.getItem("bm_lyric_inactive_op"));
  const lyricInactiveOpacity = ref<number>(
    Number.isFinite(rawSavedInactiveOp) && rawSavedInactiveOp >= 0.15 && rawSavedInactiveOp <= 0.85
      ? Number(rawSavedInactiveOp.toFixed(2))
      : 0.36
  );

  function resetLyricSettingsToDefault() {
    lyricTheme.value = "dynamic_cover";
    lyricSungMode.value = "gradient";
    lyricCustomPrimaryHex.value = "#5EEAD4";
    lyricCustomSecondaryHex.value = "#60A5FA";
    lyricUnsungPreset.value = "auto";
    lyricUnsungCustomHex.value = "#CBD5E1";
    lyricFontFamily.value = "sans";
    lyricFontWeight.value = 760;
    lyricFontSize.value = 28;
    lyricTranslationScale.value = 0.84;
    lyricLineSpacing.value = 16;
    lyricLetterSpacing.value = 0;
    lyricFocusScaleRatio.value = 1.08;
    lyricAlignment.value = "left";
    lyricBlurEnabled.value = true;
    lyricBlurIntensity.value = 1.0;
    lyricBackgroundDim.value = 0.72;
    lyricGlowIntensity.value = 100;
    lyricInactiveOpacity.value = 0.36;
    lyricWordByWord.value = true;
    lyricWordBounceEnabled.value = true;
    lyricFocusScaleEnabled.value = true;
    lyricTranslationTintEnabled.value = true;
    lyricInterludeDotsEnabled.value = true;
    lyricClickSeekEnabled.value = true;
    lyricAutoReturnSec.value = 3;
    lyricOffsetMs.value = 0;
  }

  // 进阶播放控制（副歌速刷 Climax 30s、伴奏模式、音量平衡、跨曲淡入淡出、倍速、升降调、睡眠定时）
  const climaxModeEnabled = ref<boolean>(
    localStorage.getItem("bm_climax_mode") === "true"
  );
  const ktvAccompanimentMode = ref<boolean>(false);
  const replayGainEnabled = ref<boolean>(
    localStorage.getItem("bm_replay_gain") !== "false"
  );
  const nightLoudnessEnabled = ref<boolean>(
    localStorage.getItem("bm_night_loudness") === "true"
  );
  const crossfadeSeconds = ref<number>(
    Number(localStorage.getItem("bm_crossfade_sec")) || 0
  );
  const playPauseFadeEnabled = ref<boolean>(
    localStorage.getItem("bm_play_fade") !== "false"
  );
  const playbackRate = ref<number>(1.0);
  const pitchSemitones = ref<number>(0);
  const linkPitchWithSpeed = ref<boolean>(false);

  const sleepTimerMinutes = ref<number>(0);
  const sleepWaitCurrentSong = ref<boolean>(true);
  const sleepRemainingSeconds = ref<number>(0);
  let sleepIntervalId: number | null = null;

  // 合规隐藏式「音乐下载与歌词保存」开关（默认关闭隐藏，在设置->功能与隐私管控中手动开启后解锁）
  const downloadFeatureUnlocked = ref<boolean>(
    localStorage.getItem("bm_download_unlocked") === "true"
  );
  const rawSavedDownloadQuality = localStorage.getItem("bm_dl_quality");
  const initialDownloadQuality = normalizeConceptQuality(rawSavedDownloadQuality);
  if (rawSavedDownloadQuality && rawSavedDownloadQuality !== initialDownloadQuality) {
    localStorage.setItem("bm_dl_quality", initialDownloadQuality);
  }
  const downloadQuality = ref<AudioQualityValue>(initialDownloadQuality);
  const downloadMusicDir = ref<string>(
    localStorage.getItem("bm_dl_music_dir") || "./downloads"
  );
  const downloadLrcDir = ref<string>(
    localStorage.getItem("bm_dl_lrc_dir") || "./downloads/Lyrics"
  );
  const embedCoverAndTags = ref<boolean>(
    localStorage.getItem("bm_dl_embed_cover") !== "false"
  );
  const embedLyricsInAudio = ref<boolean>(
    localStorage.getItem("bm_dl_embed_lyric") !== "false"
  );
  const exportStandaloneLrc = ref<boolean>(
    localStorage.getItem("bm_dl_export_lrc") !== "false"
  );
  const bilingualLrcExport = ref<boolean>(
    localStorage.getItem("bm_dl_bilingual_lrc") !== "false"
  );
  const skipExistingFiles = ref<boolean>(
    localStorage.getItem("bm_dl_skip_exist") !== "false"
  );
  const rawSavedNaming = localStorage.getItem("bm_dl_file_naming_v1");
  const downloadFileNaming = ref<DownloadFileNamingMode>(
    rawSavedNaming === "title_artist" || rawSavedNaming === "title_only"
      ? rawSavedNaming
      : "artist_title"
  );
  const rawSavedOrg = localStorage.getItem("bm_dl_folder_org_v1");
  const downloadFolderOrganize = ref<DownloadFolderOrganizeMode>(
    rawSavedOrg === "artist" || rawSavedOrg === "artist_album"
      ? rawSavedOrg
      : "flat"
  );
  const rawSavedConcurrency = Number(localStorage.getItem("bm_dl_concurrency_v1") || "2");
  const downloadConcurrency = ref<1 | 2 | 3>(
    rawSavedConcurrency === 1 || rawSavedConcurrency === 3 ? rawSavedConcurrency : 2
  );
  const autoSyncLocalAfterDownload = ref<boolean>(
    localStorage.getItem("bm_dl_auto_sync_local_v1") !== "false"
  );
  const downloadQueue = ref<DownloadQueueItem[]>([]);
  const isSavingLyricOnly = ref<boolean>(false);
  const activeDownloadTaskIds = new Set<string>();
  const canceledDownloadTaskIds = new Set<string>();
  const downloadSpeedTrackers = new Map<
    string,
    { lastBytes: number; lastTsMs: number; emaSpeed: number }
  >();

  const downloadHistory = ref<DownloadHistoryRecord[]>(
    (() => {
      try {
        const parsed = JSON.parse(localStorage.getItem("bm_dl_history") || "[]");
        return Array.isArray(parsed) ? parsed : [];
      } catch {
        return [];
      }
    })()
  );
  const isDownloadingSong = ref<boolean>(false);

  // ================= 全局与多窗口字体排版中心状态 =================
  const typographyConfig = reactive<AppTypographyConfig>(
    loadPersistedTypographyConfig()
  );
  const systemFonts = ref<SystemFontFamilyInfo[]>([]);
  const customFonts = ref<CustomFontRecord[]>([]);
  const customFontsDir = ref<string>("./fonts");
  const isScanningFonts = ref<boolean>(false);
  const isImportingFonts = ref<boolean>(false);

  const stageLyricFontOverride = computed(() =>
    resolveStageLyricFontOverride(typographyConfig)
  );
  const desktopLyricFontOverride = computed(() =>
    resolveDesktopLyricFontOverride(typographyConfig)
  );

  // 立即同步初始化 :root CSS 变量（主窗口与独立子窗口零延迟）
  applyTypographyToDocument(typographyConfig);

  async function refreshInstalledFonts(silent = false) {
    if (isScanningFonts.value) return;
    isScanningFonts.value = true;
    try {
      const payload = await kugouClient.listInstalledFonts();
      systemFonts.value = Array.isArray(payload.systemFonts)
        ? payload.systemFonts
        : [];
      customFonts.value = Array.isArray(payload.customFonts)
        ? payload.customFonts
        : [];
      if (payload.fontsDir) {
        customFontsDir.value = payload.fontsDir;
      }
      await ensureAllCustomFontsLoaded(customFonts.value);
      await hydrateAndApplyTypography(typographyConfig);
    } catch {
      // 纯浏览器预览环境优雅回退内置 Windows 常见字体目录
      if (systemFonts.value.length === 0) {
        systemFonts.value = [...BROWSER_PREVIEW_FALLBACK_FONTS];
      }
      if (!silent) {
        notify("浏览器预览模式：已展示 Windows 常见字体列表（桌面客户端将实时扫描本机全部字体）");
      }
    } finally {
      isScanningFonts.value = false;
    }
  }

  async function updateTypographyConfig(patch: Partial<AppTypographyConfig>) {
    const next = normalizeTypographyConfig({ ...typographyConfig, ...patch });
    Object.assign(typographyConfig, next);
    persistTypographyConfig(next);
    await hydrateAndApplyTypography(next);
    broadcastTypographyConfig(next);
    broadcastDesktopLyricState();
  }

  async function importCustomFontsFromPicker() {
    if (isImportingFonts.value) return;
    isImportingFonts.value = true;
    try {
      const picked = await kugouClient.pickFontFilesDialog();
      if (!picked || picked.length === 0) return;
      const added = await kugouClient.importCustomFontFiles(picked);
      if (added.length > 0) {
        for (const rec of added) {
          await ensureCustomFontLoaded(rec.id);
        }
        await refreshInstalledFonts(true);
        const ttcCount = added.filter((a) => a.extractedFromTtc).length;
        notify(
          ttcCount > 0
            ? `已成功导入 ${added.length} 个字体文件（含 ${ttcCount} 个 .ttc 自动拆包）`
            : `已成功导入 ${added.length} 个自定义字体文件`
        );
      }
    } catch (e: any) {
      notify(`导入字体失败：${String(e)}`);
    } finally {
      isImportingFonts.value = false;
    }
  }

  async function removeCustomFont(fontId: string) {
    try {
      const remaining = await kugouClient.deleteCustomFont(fontId);
      unloadCustomFontFace(fontId);
      customFonts.value = remaining;
      // 若当前作用域正在使用被删除字体，自动安全回退到系统默认
      const patch: Partial<AppTypographyConfig> = {};
      if (typographyConfig.uiCustomFontId === fontId) {
        patch.uiFontFamily = "";
        patch.uiCustomFontId = "";
        patch.uiFontDisplayName = "系统默认";
      }
      if (typographyConfig.stageLyricCustomFontId === fontId) {
        patch.stageLyricMode = "preset";
        patch.stageLyricFontFamily = "";
        patch.stageLyricCustomFontId = "";
        patch.stageLyricFontDisplayName = "";
      }
      if (typographyConfig.desktopLyricCustomFontId === fontId) {
        patch.desktopLyricMode = "preset";
        patch.desktopLyricFontFamily = "";
        patch.desktopLyricCustomFontId = "";
        patch.desktopLyricFontDisplayName = "";
      }
      if (Object.keys(patch).length > 0) {
        await updateTypographyConfig(patch);
      }
      notify("已卸载并移除该自定义字体文件");
    } catch (e: any) {
      notify(`卸载字体失败：${String(e)}`);
    }
  }

  async function resetTypographyConfig() {
    await updateTypographyConfig({ ...DEFAULT_TYPOGRAPHY_CONFIG });
    notify("已恢复系统默认字体与 100% 标准排版比例");
  }

  // 四维存储缓存明细统计（对齐 SettingsDownloadAndPermissions.kt，从 Rust 后端实时读取真实磁盘字节）
  const rawSavedCacheQuota = localStorage.getItem("bm_cache_max_quota_gb_v1");
  const initialCacheQuotaGb = (() => {
    if (rawSavedCacheQuota === null) return 4;
    const n = Number(rawSavedCacheQuota);
    if (!Number.isFinite(n) || n < 0) return 4;
    return n === 0 ? 0 : Math.min(64, Math.max(0.5, n));
  })();
  const cacheStats = reactive({
    audioStreamMb: 0,
    artworkImageMb: 0,
    playlistSnapshotMb: 0,
    lyricAndUrlMb: 0,
    maxQuotaGb: initialCacheQuotaGb,
    cacheDirPath: "./cache",
  });

  async function setCacheQuotaGb(gb: number) {
    const safeGb = !Number.isFinite(gb) || gb <= 0 ? 0 : Math.min(64, Math.max(0.5, gb));
    cacheStats.maxQuotaGb = safeGb;
    try {
      localStorage.setItem("bm_cache_max_quota_gb_v1", String(safeGb));
    } catch {
      // ignore
    }
    try {
      const updated = await kugouClient.setAudioCacheQuota(safeGb);
      cacheStats.audioStreamMb = Number((updated.audioStreamMb || 0).toFixed(2));
      cacheStats.artworkImageMb = Number((updated.artworkImageMb || 0).toFixed(2));
      cacheStats.playlistSnapshotMb = Number((updated.playlistSnapshotMb || 0).toFixed(2));
      cacheStats.lyricAndUrlMb = Number((updated.lyricAndUrlMb || 0).toFixed(2));
    } catch {
      // ignore in browser preview
    }
    notify(
      safeGb === 0
        ? "音频流磁盘缓存上限已设为「无限制」"
        : `音频流磁盘缓存上限已设为 ${safeGb} GB，超限旧缓存将自动 LRU 裁剪`
    );
  }

  async function openDirectoryInExplorer(pathOrAlias: string) {
    try {
      await kugouClient.openPathInExplorer(pathOrAlias);
    } catch (e: any) {
      notify(`打开目录失败：${String(e)}`);
    }
  }

  async function revealFileInExplorer(filePath: string) {
    if (!filePath || !filePath.trim()) {
      notify("无可定位的文件路径");
      return;
    }
    try {
      await kugouClient.revealPathInExplorer(filePath);
    } catch (e: any) {
      notify(`定位文件失败：${String(e)}`);
    }
  }

  function clearDownloadHistory() {
    downloadHistory.value = [];
    try {
      localStorage.removeItem("bm_dl_history");
    } catch {
      // ignore
    }
    notify("已清空本地下载记录（磁盘音频文件不受影响）");
  }

  async function refreshCacheStats() {
    try {
      const real = await kugouClient.getCacheStats();
      cacheStats.audioStreamMb = Number((real.audioStreamMb || 0).toFixed(2));
      cacheStats.artworkImageMb = Number((real.artworkImageMb || 0).toFixed(2));
      cacheStats.playlistSnapshotMb = Number((real.playlistSnapshotMb || 0).toFixed(2));
      cacheStats.lyricAndUrlMb = Number((real.lyricAndUrlMb || 0).toFixed(2));
      if (real.cacheDirPath) {
        cacheStats.cacheDirPath = real.cacheDirPath;
        const installRoot = real.cacheDirPath.replace(/[\\/]+cache$/i, "");
        if (
          installRoot &&
          (!downloadMusicDir.value ||
            downloadMusicDir.value === "./downloads" ||
            downloadMusicDir.value.toLowerCase() === "c:\\users\\public\\music\\breezemusic")
        ) {
          downloadMusicDir.value = `${installRoot}\\downloads`;
        }
        if (
          installRoot &&
          (!downloadLrcDir.value ||
            downloadLrcDir.value === "./downloads/Lyrics" ||
            downloadLrcDir.value.toLowerCase() === "c:\\users\\public\\music\\breezemusic\\lyrics")
        ) {
          downloadLrcDir.value = `${installRoot}\\downloads\\Lyrics`;
        }
      }
    } catch {
      // ignore in browser preview
    }
  }

  watch(colorScheme, (v) => {
    localStorage.setItem("bm_color_scheme_kugou_v2", v);
    syncDocumentThemeAttr();
    broadcastDesktopLyricState();
    broadcastMiniPlayerState();
  });
  watch(lightThemeVariant, (v) => {
    localStorage.setItem("bm_light_theme_variant_v1", v);
    syncDocumentThemeAttr();
    broadcastDesktopLyricState();
    broadcastMiniPlayerState();
  });
  watch(darkThemeVariant, (v) => {
    localStorage.setItem("bm_dark_theme_variant_v1", v);
    syncDocumentThemeAttr();
    broadcastDesktopLyricState();
    broadcastMiniPlayerState();
  });
  watch(resolvedColorScheme, () => {
    syncDocumentThemeAttr();
    broadcastDesktopLyricState();
    broadcastMiniPlayerState();
  });
  watch(startupHomePage, (v) =>
    localStorage.setItem("bm_startup_home_v1", v)
  );
  watch(activeTab, (tab) => {
    if (tab === "discover" || tab === "library" || tab === "local_music") {
      try {
        localStorage.setItem("bm_last_home_tab_v1", tab);
      } catch {
        // ignore
      }
    }
  });
  watch(resetPlaybackRateOnTrackChange, (v) =>
    localStorage.setItem("bm_reset_speed_on_track_v1", String(v))
  );
  watch(startupSafeVolumeEnabled, (v) =>
    localStorage.setItem("bm_startup_safe_vol_enabled_v1", String(v))
  );
  watch(startupSafeVolumeLimit, (v) => {
    const safe = clampVal(Number(Number(v).toFixed(2)) || 0.5, 0.1, 0.9);
    if (safe !== v) {
      startupSafeVolumeLimit.value = safe;
    }
    localStorage.setItem("bm_startup_safe_vol_limit_v1", String(safe));
  });
  watch(autoClaimVipOnStartup, (v) =>
    localStorage.setItem("bm_auto_claim_vip", String(v))
  );
  watch(autoPlayOnStartup, (v) =>
    localStorage.setItem("bm_auto_play_on_startup_v1", String(v))
  );
  watch(closeToTray, (v) => {
    localStorage.setItem("bm_close_to_tray", String(v));
    kugouClient.setCloseToTray(v).catch(() => {});
  });
  watch(miniYieldFullscreenEnabled, (v) => {
    localStorage.setItem("bm_mini_yield_fullscreen_v1", String(v));
    kugouClient.setMiniPlayerYieldFullscreen(v).catch(() => {});
  });
  watch(clipboardShareDetectEnabled, (v) => {
    localStorage.setItem("bm_clipboard_share_detect_v1", String(v));
  });
  watch(osGlobalHotkeysEnabled, (v) => {
    localStorage.setItem("bm_os_global_hotkeys", String(v));
    kugouClient.setGlobalHotkeysEnabled(v).catch(() => {});
    if (v) {
      syncGlobalHotkeysToOs(false);
    } else {
      globalHotkeyFailedActions.value = [];
    }
    notify(
      v
        ? "已启用系统级后台全局热键（自定义组合键 + 硬件媒体键）"
        : "已暂停系统级后台全局热键，仅保留应用内快捷键"
    );
  });

  async function browseDownloadMusicDir() {
    try {
      const picked = await kugouClient.pickFolderDialog(
        downloadMusicDir.value,
        "选择音乐保存目录"
      );
      if (picked && picked.trim()) {
        downloadMusicDir.value = picked.trim();
        notify(`已更新音乐保存目录：${picked.trim()}`);
      }
    } catch (e: any) {
      notify(`打开目录选择器失败：${String(e)}`);
    }
  }

  async function browseDownloadLrcDir() {
    try {
      const picked = await kugouClient.pickFolderDialog(
        downloadLrcDir.value,
        "选择独立歌词 (.lrc) 保存目录"
      );
      if (picked && picked.trim()) {
        downloadLrcDir.value = picked.trim();
        notify(`已更新独立歌词保存目录：${picked.trim()}`);
      }
    } catch (e: any) {
      notify(`打开目录选择器失败：${String(e)}`);
    }
  }
  watch(playerStageStyle, (v) => localStorage.setItem("bm_stage_style", v));
  watch(playerBgStyle, (v) => localStorage.setItem("bm_bg_style", v));
  watch(coverColorStageMode, (v) => {
    localStorage.setItem("bm_cover_color_stage_mode_v1", v);
    localStorage.setItem("bm_dynamic_cover_accent", String(v !== "off"));
    broadcastDesktopLyricState();
  });
  watch(lyricTheme, (v) => {
    localStorage.setItem("bm_lyric_theme", v);
  });
  watch(lyricSungMode, (v) => {
    localStorage.setItem("bm_lyric_sung_mode_v2", v);
  });
  watch(lyricCustomPrimaryHex, (v) => {
    localStorage.setItem("bm_lyric_custom_primary_v2", v);
  });
  watch(lyricCustomSecondaryHex, (v) => {
    localStorage.setItem("bm_lyric_custom_secondary_v2", v);
  });
  watch(lyricUnsungPreset, (v) => {
    localStorage.setItem("bm_lyric_unsung_preset_v2", v);
  });
  watch(lyricUnsungCustomHex, (v) => {
    localStorage.setItem("bm_lyric_unsung_custom_v2", v);
  });
  watch(lyricLetterSpacing, (v) => {
    localStorage.setItem("bm_lyric_letter_spacing_v2", String(v));
  });
  watch(lyricFocusScaleRatio, (v) => {
    localStorage.setItem("bm_lyric_focus_scale_ratio_v2", String(v));
  });
  watch(desktopLyricSungMode, (v) => {
    localStorage.setItem("bm_dl_sung_mode_v2", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricCustomPrimary, (v) => {
    localStorage.setItem("bm_dl_custom_primary_v2", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricCustomSecondary, (v) => {
    localStorage.setItem("bm_dl_custom_secondary_v2", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricUnsungPreset, (v) => {
    localStorage.setItem("bm_dl_unsung_preset_v2", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricUnsungCustomHex, (v) => {
    localStorage.setItem("bm_dl_unsung_custom_v2", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricUnsungOpacity, (v) => {
    localStorage.setItem("bm_dl_unsung_opacity_v2", String(v));
    broadcastDesktopLyricState();
  });
  watch(desktopLyricStrokeWidth, (v) => {
    localStorage.setItem("bm_dl_stroke_width_v2", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricStrokeColorMode, (v) => {
    localStorage.setItem("bm_dl_stroke_color_mode_v2", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricStrokeCustomHex, (v) => {
    localStorage.setItem("bm_dl_stroke_custom_v2", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricShadowEnabled, (v) => {
    localStorage.setItem("bm_dl_shadow_enabled_v2", String(v));
    broadcastDesktopLyricState();
  });
  watch(lyricFontSize, (v) =>
    localStorage.setItem("bm_lyric_font_size", String(v))
  );
  watch(lyricFontWeight, (v) =>
    localStorage.setItem("bm_lyric_font_weight", String(v))
  );
  watch(lyricTranslationScale, (v) =>
    localStorage.setItem("bm_lyric_trans_scale", String(v))
  );
  watch(lyricLineSpacing, (v) =>
    localStorage.setItem("bm_lyric_line_spacing", String(v))
  );
  watch(lyricAlignment, (v) => localStorage.setItem("bm_lyric_align", v));
  watch(lyricBlurEnabled, (v) =>
    localStorage.setItem("bm_lyric_blur", String(v))
  );
  watch(lyricBlurIntensity, (v) =>
    localStorage.setItem("bm_lyric_blur_int", String(v))
  );
  watch(lyricBackgroundDim, (v) =>
    localStorage.setItem("bm_lyric_bg_dim", String(v))
  );
  watch(lyricWordByWord, (v) => {
    localStorage.setItem("bm_lyric_krc", String(v));
  });
  watch(lyricWordBounceEnabled, (v) =>
    localStorage.setItem("bm_lyric_word_bounce", String(v))
  );
  watch(lyricFocusScaleEnabled, (v) =>
    localStorage.setItem("bm_lyric_focus_scale", String(v))
  );
  watch(lyricTranslationTintEnabled, (v) =>
    localStorage.setItem("bm_lyric_trans_tint", String(v))
  );
  watch(lyricInterludeDotsEnabled, (v) =>
    localStorage.setItem("bm_lyric_interlude_dots", String(v))
  );
  watch(lyricClickSeekEnabled, (v) =>
    localStorage.setItem("bm_lyric_click_seek", String(v))
  );
  watch(lyricAutoReturnSec, (v) =>
    localStorage.setItem("bm_lyric_auto_return_sec", String(v))
  );
  watch(lyricFontFamily, (v) => {
    localStorage.setItem("bm_lyric_font_family", v);
  });
  watch(desktopLyricLayoutMode, (v) => {
    localStorage.setItem("bm_desktop_lyric_layout_v2", v);
    localStorage.setItem("bm_desktop_lyric_line_mode", v === "single" ? "single" : "double");
    broadcastDesktopLyricState();
  });
  watch(desktopLyricBgStyle, (v) => {
    localStorage.setItem("bm_desktop_lyric_bg_style_v2", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricClarity, (v) => {
    localStorage.setItem("bm_desktop_lyric_clarity_v2", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricFontFamily, (v) => {
    localStorage.setItem("bm_desktop_lyric_font_family", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricFontWeight, (v) => {
    localStorage.setItem("bm_desktop_lyric_font_weight", String(v));
    broadcastDesktopLyricState();
  });
  watch(desktopLyricWordByWord, (v) => {
    localStorage.setItem("bm_desktop_lyric_krc", String(v));
    broadcastDesktopLyricState();
  });
  watch(desktopLyricUnsungTone, (v) => {
    localStorage.setItem("bm_desktop_lyric_unsung_tone", v);
    broadcastDesktopLyricState();
  });
  watch(desktopLyricLineSpacing, (v) => {
    localStorage.setItem("bm_desktop_lyric_line_spacing", String(v));
    broadcastDesktopLyricState();
  });
  watch(desktopLyricGlowIntensity, (v) => {
    localStorage.setItem("bm_desktop_lyric_glow_v2", String(v));
    broadcastDesktopLyricState();
  });
  watch(desktopLyricOpacity, (v) => {
    localStorage.setItem("bm_desktop_lyric_opacity", String(v));
    broadcastDesktopLyricState();
  });
  watch(desktopLyricIdleFade, (v) => {
    localStorage.setItem("bm_desktop_lyric_idle_fade", String(v));
    broadcastDesktopLyricState();
  });
  watch(desktopLyricShowIntro, (v) => {
    localStorage.setItem("bm_desktop_lyric_intro", String(v));
    broadcastDesktopLyricState();
  });
  watch(desktopLyricShowPhonetic, (v) => {
    localStorage.setItem("bm_desktop_lyric_phonetic", String(v));
    broadcastDesktopLyricState();
  });
  watch(lyricGlowIntensity, (v) =>
    localStorage.setItem("bm_lyric_glow_int", String(v))
  );
  watch(lyricInactiveOpacity, (v) =>
    localStorage.setItem("bm_lyric_inactive_op", String(v))
  );
  watch(lyricOffsetMs, (v) => {
    localStorage.setItem("bm_lyric_offset_ms", String(v));
    broadcastDesktopLyricState();
    broadcastDesktopLyricPosition();
  });
  watch(bluetoothDelayMs, (v) => {
    localStorage.setItem("bm_bt_delay_ms", String(v));
    broadcastDesktopLyricState();
    broadcastDesktopLyricPosition();
  });
  // 跨进程/跨窗口 双向握手 + 实时高频双通道 (BroadcastChannel + Tauri Event + Storage 快照兜底) 桌面歌词同步架构
  const desktopLyricChannel =
    typeof BroadcastChannel !== "undefined"
      ? new BroadcastChannel("bm_desktop_lyric_channel")
      : null;

  function serializeLyricForCrossWindow(lyricData: LyricData | null | undefined) {
    if (!lyricData || !Array.isArray(lyricData.lines)) return null;
    return {
      isKrc: Boolean(lyricData.isKrc),
      hasTranslation: Boolean(lyricData.hasTranslation),
      hasPhonetic: Boolean(lyricData.hasPhonetic),
      lines: lyricData.lines.map((line) => ({
        startTimeMs: Number(line.startTimeMs) || 0,
        durationMs: Number(line.durationMs) || 0,
        text: String(line.text || ""),
        translation: line.translation ? String(line.translation) : undefined,
        phonetic: line.phonetic ? String(line.phonetic) : undefined,
        words: Array.isArray(line.words)
          ? line.words.map((w) => ({
              text: String(w.text || ""),
              startTimeMs: Number(w.startTimeMs) || 0,
              durationMs: Number(w.durationMs) || 0,
            }))
          : undefined,
      })),
    };
  }

  let desktopLyricStateSeq = 0;
  let desktopLyricLyricSeq = 0;
  let suppressDesktopLyricSchemeOverride = false;
  let skipNextOsLockSync = false;
  let skipNextOsWidthSync = false;

  function resolveSchemeFromHex(hex: string): string {
    const lower = String(hex || "").trim().toLowerCase();
    const map: Record<string, string> = {
      "#5eead4": "glacier_aurora",
      "#60a5fa": "glacier_aurora",
      "#0f766e": "glacier_aurora",
      "#fb7185": "sunset_amber",
      "#fbbf24": "sunset_amber",
      "#e11d48": "sunset_amber",
      "#f8fafc": "moonlight_frost",
      "#ffffff": "moonlight_frost",
      "#34d399": "jade_mineral",
      "#047857": "jade_mineral",
      "#f9a8d4": "rose_champagne",
      "#be185d": "rose_champagne",
      "#c084fc": "amethyst_mist",
      "#7e22ce": "amethyst_mist",
      "#ff785a": "ink_vermilion",
      "#b91c1c": "ink_vermilion",
    };
    return map[lower] || "custom";
  }

  function broadcastDesktopLyricPosition() {
    if (isStandaloneSubWindow || !desktopLyricEnabled.value) return;
    try {
      const livePosMs =
        typeof getLivePlaybackPositionMs === "function"
          ? Math.round(getLivePlaybackPositionMs())
          : Math.round(currentPositionMs.value || 0);
      const effRate =
        typeof getEffectivePlaybackRate === "function"
          ? getEffectivePlaybackRate()
          : Number(playbackRate.value) || 1;
      const payload = {
        type: "POSITION_SYNC",
        songHash: String(currentSong.value?.hash || ""),
        lyricSeq: desktopLyricLyricSeq,
        isPlaying: Boolean(isPlaying.value),
        positionMs: livePosMs,
        durationMs: Number(durationMs.value || currentSong.value?.durationMs || 0),
        playbackRate: effRate,
        lyricOffsetMs: Number(lyricOffsetMs.value) || 0,
        bluetoothDelayMs: Number(bluetoothDelayMs.value) || 0,
        sentAtMs: Date.now(),
      };
      if (desktopLyricChannel && !hasTauriBridge) {
        desktopLyricChannel.postMessage(payload);
      }
      invoke("push_desktop_lyric_sync_cmd", { payload }).catch(() => {});
    } catch {
      // ignore serialization error
    }
  }

  function broadcastDesktopLyricState(force = false) {
    if (isStandaloneSubWindow || (!force && !desktopLyricEnabled.value)) return;
    try {
      const dynamicPrimary = tuneAccentForDarkStage(
        coverPalette.value.dominantAccent || "#3898FF",
        {
          surfaceHex: "#0B1018",
          minWcagContrast: 4.2,
          maxLuminance: 0.88,
          preserveChroma: true,
        }
      );
      const dynamicSecondary = tuneAccentForDarkStage(
        coverPalette.value.secondaryAccent || "#2DD4BF",
        {
          surfaceHex: "#0B1018",
          minWcagContrast: 4.0,
          maxLuminance: 0.88,
          preserveChroma: true,
        }
      );
      const livePosMs =
        typeof getLivePlaybackPositionMs === "function"
          ? Math.round(getLivePlaybackPositionMs())
          : Math.round(currentPositionMs.value || 0);
      const effRate =
        typeof getEffectivePlaybackRate === "function"
          ? getEffectivePlaybackRate()
          : Number(playbackRate.value) || 1;
      const payload = {
        type: "STATE_SYNC",
        stateSeq: ++desktopLyricStateSeq,
        lyricSeq: desktopLyricLyricSeq,
        enabled: Boolean(desktopLyricEnabled.value),
        locked: Boolean(desktopLyricLocked.value),
        color: String(desktopLyricColor.value || "#5EEAD4"),
        colorSchemeId: String(desktopLyricColorScheme.value || "dynamic_cover"),
        sungMode: String(desktopLyricSungMode.value || "gradient"),
        customPrimary: String(desktopLyricCustomPrimary.value || "#5EEAD4"),
        customSecondary: String(desktopLyricCustomSecondary.value || "#60A5FA"),
        unsungPreset: String(desktopLyricUnsungPreset.value || "auto"),
        unsungCustomHex: String(desktopLyricUnsungCustomHex.value || "#E6ECF5"),
        unsungOpacity: Number(desktopLyricUnsungOpacity.value) || 0.78,
        strokeWidth: String(desktopLyricStrokeWidth.value || "medium"),
        strokeColorMode: String(desktopLyricStrokeColorMode.value || "auto"),
        strokeCustomHex: String(desktopLyricStrokeCustomHex.value || "#090D16"),
        shadowEnabled: Boolean(desktopLyricShadowEnabled.value),
        dynamicPrimary: String(dynamicPrimary || "#5EEAD4"),
        dynamicSecondary: String(dynamicSecondary || "#60A5FA"),
        themePrimary: String(
          effectiveCoverPalette.value.dominantAccent || "#5E6AD2"
        ),
        themeSecondary: String(
          effectiveCoverPalette.value.secondaryAccent || "#38BDF8"
        ),
        dynamicCoverAccentEnabled: Boolean(dynamicCoverAccentEnabled.value),
        colorScheme: String(resolvedColorScheme.value || "abyssal_dark"),
        globalColorScheme: String(resolvedColorScheme.value || "abyssal_dark"),
        lightThemeVariant: String(lightThemeVariant.value || "watercolor"),
        darkThemeVariant: String(darkThemeVariant.value || "obsidian_aurora"),
        isLightStage: Boolean(isNowPlayingLightStage.value),
        playMode: String(playMode.value || "repeat_all"),
        fontSize: Number(desktopLyricFontSize.value) || 32,
        fontFamily: String(desktopLyricFontFamily.value || "sans"),
        fontWeight: Number(desktopLyricFontWeight.value) || 760,
        customFontFamilyStack: desktopLyricFontOverride.value.familyStack || "",
        customFontId: desktopLyricFontOverride.value.customFontId || "",
        customFontWeight: desktopLyricFontOverride.value.weightOverride,
        width: Number(desktopLyricWidth.value) || 820,
        layoutMode: String(desktopLyricLayoutMode.value || "center"),
        lineMode: String(desktopLyricLineMode.value || "double"),
        bgStyle: String(desktopLyricBgStyle.value || "transparent"),
        clarityMode: String(desktopLyricClarity.value || "crisp"),
        unsungTone: String(desktopLyricUnsungTone.value || "snow"),
        lineSpacing: Number(desktopLyricLineSpacing.value) || 6,
        glowIntensity: Number(desktopLyricGlowIntensity.value) ?? 90,
        opacity: Number(desktopLyricOpacity.value) || 1.0,
        idleFade: Boolean(desktopLyricIdleFade.value),
        showIntro: Boolean(desktopLyricShowIntro.value),
        showPhonetic: Boolean(desktopLyricShowPhonetic.value),
        showTranslation: Boolean(desktopLyricShowTranslation.value),
        wordByWord: Boolean(desktopLyricWordByWord.value),
        isPlaying: Boolean(isPlaying.value),
        positionMs: livePosMs,
        durationMs: Number(durationMs.value || currentSong.value?.durationMs || 0),
        playbackRate: effRate,
        lyricOffsetMs: Number(lyricOffsetMs.value) || 0,
        bluetoothDelayMs: Number(bluetoothDelayMs.value) || 0,
        sentAtMs: Date.now(),
        song: currentSong.value
          ? {
              hash: String(currentSong.value.hash || ""),
              title: String(currentSong.value.title || ""),
              artist: String(currentSong.value.artist || ""),
              coverUrl: String(currentSong.value.coverUrl || ""),
            }
          : null,
        lyric: desktopLyricEnabled.value
          ? serializeLyricForCrossWindow(currentLyric.value)
          : null,
      };
      try {
        localStorage.setItem(DESKTOP_LYRIC_SNAPSHOT_STORAGE_KEY, JSON.stringify(payload));
      } catch {
        // ignore storage quota error
      }
      if (desktopLyricChannel && !hasTauriBridge) {
        desktopLyricChannel.postMessage(payload);
      }
      invoke("push_desktop_lyric_sync_cmd", { payload }).catch(() => {});
    } catch {
      // ignore serialization error
    }
  }

  let lastDesktopLyricCmdDedupKey = "";
  let lastDesktopLyricCmdAt = 0;

  function handleDesktopLyricCommand(msg: any) {
    if (isStandaloneDesktopLyricWindow || !msg || typeof msg !== "object") return;
    if (msg.type === "CMD_REQUEST_SYNC" || msg.type === "CMD_DESKTOP_LYRIC_READY") {
      broadcastDesktopLyricState();
      broadcastDesktopLyricPosition();
      return;
    }
    const now = performance.now();
    const dedupKey = JSON.stringify(msg);
    if (dedupKey === lastDesktopLyricCmdDedupKey && now - lastDesktopLyricCmdAt < 45) {
      return;
    }
    lastDesktopLyricCmdDedupKey = dedupKey;
    lastDesktopLyricCmdAt = now;

    if (msg.type === "CMD_TOGGLE_PLAY") togglePlayPause();
    else if (msg.type === "CMD_PREV") playPrev();
    else if (msg.type === "CMD_NEXT") playNext(false);
    else if (msg.type === "CMD_UNLOCK") {
      if (desktopLyricLocked.value !== false) {
        skipNextOsLockSync = true;
        desktopLyricLocked.value = false;
      }
    } else if (msg.type === "CMD_LOCK") {
      if (desktopLyricLocked.value !== true) {
        skipNextOsLockSync = true;
        desktopLyricLocked.value = true;
      }
    } else if (msg.type === "CMD_CLOSE") {
      desktopLyricEnabled.value = false;
    } else if (msg.type === "CMD_SET_COLOR") {
      suppressDesktopLyricSchemeOverride = true;
      let colorChanged = false;
      let schemeChanged = false;
      try {
        if (msg.schemeId) {
          const nextScheme = String(msg.schemeId);
          if (desktopLyricColorScheme.value !== nextScheme) {
            desktopLyricColorScheme.value = nextScheme;
            localStorage.setItem("bm_desktop_lyric_scheme", nextScheme);
            schemeChanged = true;
          }
        }
        if (msg.color) {
          const nextColor = String(msg.color);
          if (desktopLyricColor.value !== nextColor) {
            desktopLyricColor.value = nextColor;
            colorChanged = true;
          }
        }
      } finally {
        suppressDesktopLyricSchemeOverride = false;
      }
      if (!colorChanged && (schemeChanged || msg.schemeId)) {
        broadcastDesktopLyricState();
      }
    } else if (msg.type === "CMD_SET_SUNG_COLOR") {
      suppressDesktopLyricSchemeOverride = true;
      try {
        if (msg.schemeId) {
          const nextScheme = normalizeLyricThemeId(String(msg.schemeId));
          desktopLyricColorScheme.value = nextScheme;
          localStorage.setItem("bm_desktop_lyric_scheme", nextScheme);
        }
        if (msg.sungMode === "gradient" || msg.sungMode === "solid") {
          desktopLyricSungMode.value = msg.sungMode;
        }
        if (msg.customPrimary) {
          desktopLyricCustomPrimary.value = String(msg.customPrimary);
          desktopLyricColor.value = String(msg.customPrimary);
        }
        if (msg.customSecondary) {
          desktopLyricCustomSecondary.value = String(msg.customSecondary);
        }
      } finally {
        suppressDesktopLyricSchemeOverride = false;
      }
      broadcastDesktopLyricState();
    } else if (msg.type === "CMD_SET_UNSUNG_COLOR") {
      if (msg.unsungPreset) {
        desktopLyricUnsungPreset.value = normalizeUnsungPresetId(String(msg.unsungPreset));
      }
      if (msg.unsungCustomHex) {
        desktopLyricUnsungCustomHex.value = String(msg.unsungCustomHex);
      }
      if (msg.unsungOpacity !== undefined) {
        desktopLyricUnsungOpacity.value = clampVal(Number(msg.unsungOpacity), 0.35, 0.98);
      }
    } else if (msg.type === "CMD_SET_STROKE") {
      if (
        msg.strokeWidth === "none" ||
        msg.strokeWidth === "thin" ||
        msg.strokeWidth === "medium" ||
        msg.strokeWidth === "bold"
      ) {
        desktopLyricStrokeWidth.value = msg.strokeWidth;
      }
      if (
        msg.strokeColorMode === "auto" ||
        msg.strokeColorMode === "black" ||
        msg.strokeColorMode === "white" ||
        msg.strokeColorMode === "custom"
      ) {
        desktopLyricStrokeColorMode.value = msg.strokeColorMode;
      }
      if (msg.strokeCustomHex) {
        desktopLyricStrokeCustomHex.value = String(msg.strokeCustomHex);
      }
      if (msg.shadowEnabled !== undefined) {
        desktopLyricShadowEnabled.value = Boolean(msg.shadowEnabled);
      }
    } else if (msg.type === "CMD_RESET_DEFAULTS") {
      resetDesktopLyricSettingsToDefault();
    } else if (msg.type === "CMD_SET_WIDTH" && msg.width) {
      skipNextOsWidthSync = true;
      desktopLyricWidth.value = clampVal(Number(msg.width), 440, 1280);
    } else if (msg.type === "CMD_SET_FONT_SIZE" && msg.fontSize !== undefined) {
      desktopLyricFontSize.value = clampVal(Number(msg.fontSize), 18, 56);
    } else if (
      msg.type === "CMD_SET_SHOW_TRANSLATION" &&
      msg.showTranslation !== undefined
    ) {
      desktopLyricShowTranslation.value = Boolean(msg.showTranslation);
    } else if (
      msg.type === "CMD_SET_LAYOUT_MODE" &&
      ["center", "ktv", "single", "left"].includes(String(msg.layoutMode))
    ) {
      desktopLyricLayoutMode.value = msg.layoutMode as DesktopLyricLayoutMode;
    } else if (
      msg.type === "CMD_SET_LINE_MODE" &&
      (msg.lineMode === "single" || msg.lineMode === "double")
    ) {
      desktopLyricLayoutMode.value = msg.lineMode === "single" ? "single" : "center";
    } else if (msg.type === "CMD_SET_BG_STYLE" && msg.bgStyle) {
      desktopLyricBgStyle.value = msg.bgStyle;
    } else if (
      msg.type === "CMD_SET_CLARITY" &&
      ["soft", "crisp", "bold", "ink_light"].includes(String(msg.clarityMode))
    ) {
      desktopLyricClarity.value = msg.clarityMode as DesktopLyricClarityMode;
    } else if (
      msg.type === "CMD_SET_UNSUNG_TONE" &&
      ["snow", "warm", "ink"].includes(String(msg.unsungTone))
    ) {
      desktopLyricUnsungTone.value = msg.unsungTone as DesktopLyricUnsungTone;
    } else if (msg.type === "CMD_SET_OPACITY" && msg.opacity !== undefined) {
      desktopLyricOpacity.value = clampVal(Number(msg.opacity), 0.35, 1.0);
    } else if (msg.type === "CMD_SET_LINE_SPACING" && msg.lineSpacing !== undefined) {
      desktopLyricLineSpacing.value = clampVal(Math.round(Number(msg.lineSpacing)), 0, 16);
    } else if (msg.type === "CMD_SET_GLOW" && msg.glowIntensity !== undefined) {
      desktopLyricGlowIntensity.value = clampVal(Math.round(Number(msg.glowIntensity)), 0, 150);
    } else if (msg.type === "CMD_SET_IDLE_FADE" && msg.idleFade !== undefined) {
      desktopLyricIdleFade.value = Boolean(msg.idleFade);
    } else if (msg.type === "CMD_SET_SHOW_INTRO" && msg.showIntro !== undefined) {
      desktopLyricShowIntro.value = Boolean(msg.showIntro);
    } else if (msg.type === "CMD_SET_SHOW_PHONETIC" && msg.showPhonetic !== undefined) {
      desktopLyricShowPhonetic.value = Boolean(msg.showPhonetic);
    } else if (msg.type === "CMD_SET_WORD_BY_WORD" && msg.wordByWord !== undefined) {
      // 独立作用于桌面歌词自身，不影响播放页歌词
      desktopLyricWordByWord.value = Boolean(msg.wordByWord);
    } else if (msg.type === "CMD_SET_FONT_FAMILY" && msg.fontFamily) {
      desktopLyricFontFamily.value = msg.fontFamily as LyricFontFamily;
    } else if (msg.type === "CMD_SET_FONT_WEIGHT" && msg.fontWeight) {
      const w = Number(msg.fontWeight);
      if (w === 500 || w === 650 || w === 760 || w === 880) {
        desktopLyricFontWeight.value = w as LyricFontWeightLevel;
      }
    } else if (msg.type === "CMD_ADJUST_OFFSET" && msg.deltaMs !== undefined) {
      adjustLyricOffsetMs(Number(msg.deltaMs) || 0);
    } else if (msg.type === "CMD_RESET_OFFSET") {
      lyricOffsetMs.value = 0;
    } else if (msg.type === "CMD_RESET_POSITION") {
      snapDesktopLyricPosition("bottom");
    } else if (msg.type === "CMD_SNAP_POSITION") {
      const anchor =
        msg.anchor === "top" || msg.anchor === "center" || msg.anchor === "bottom"
          ? (msg.anchor as DesktopLyricSnapAnchor)
          : "bottom";
      const baseH =
        typeof msg.baseHeight === "number" && Number.isFinite(msg.baseHeight)
          ? msg.baseHeight
          : undefined;
      snapDesktopLyricPosition(anchor, baseH);
    } else if (msg.type === "CMD_FOCUS_MAIN_WINDOW") {
      invoke("focus_main_window_cmd").catch(() => {});
    } else if (msg.type === "CMD_CYCLE_PLAY_MODE") {
      cyclePlayMode();
    }
  }

  if (desktopLyricChannel && !isStandaloneSubWindow) {
    desktopLyricChannel.onmessage = (ev) => {
      handleDesktopLyricCommand(ev.data);
    };
  }

  // ==================== Mini 灵动岛独立透明窗体跨进程双向同步通道 ====================
  const MINI_PLAYER_POS_STORAGE_KEY = "bm_mini_player_pos_v1";
  const miniPlayerChannel =
    typeof BroadcastChannel !== "undefined"
      ? new BroadcastChannel("bm_mini_player_channel")
      : null;
  let miniPlayerStateSeq = 0;

  function resolveMiniQualityBadge(): string {
    if (currentPlaybackInfo.value?.qualityBadge) {
      return currentPlaybackInfo.value.qualityBadge;
    }
    const q =
      currentPlaybackInfo.value?.actualQuality ||
      currentPlaybackInfo.value?.matchedQuality ||
      preferredQuality.value;
    if (q === "hires" || q === "high") return "Hi-Res";
    if (q === "flac") return "SQ";
    if (q === "320") return "HQ";
    return "128K";
  }

  function resolveMiniEffectBadge(): string {
    try {
      if (!masterEffectEnabled?.value) return "音效关闭";
      const vId = activeViperEffectId?.value;
      if (vId && vId !== "off") {
        const matched = (viperEffects?.value || []).find((item) => item.id === vId);
        if (matched) return matched.label || matched.name || "蝰蛇音效";
      }
      if (acousticField?.enabled) return "听感微调";
      const eqId = activeEqPreset?.value;
      if (eqId && eqId !== "flat") {
        const eqLabels: Record<string, string> = {
          pop: "现代流行",
          rock: "摇滚动感",
          classical: "古典交响",
          vocal: "清澈人声",
          jazz: "蓝调爵士",
          electronic: "电音脉冲",
          sub_bass: "深潜超低",
          acoustic: "原木民谣",
          acg: "次元律动",
        };
        return eqLabels[eqId] || "均衡调音";
      }
    } catch {
      // ignore if DSP module not yet initialized
    }
    return "原声直通";
  }

  function broadcastMiniPlayerPosition() {
    if (isStandaloneSubWindow || !isMiniPlayerMode.value) return;
    try {
      const livePosMs =
        typeof getLivePlaybackPositionMs === "function"
          ? Math.round(getLivePlaybackPositionMs())
          : Math.round(currentPositionMs.value || 0);
      const effRate =
        typeof getEffectivePlaybackRate === "function"
          ? getEffectivePlaybackRate()
          : Number(playbackRate.value) || 1;
      const payload = {
        type: "POSITION_SYNC",
        songHash: String(currentSong.value?.hash || ""),
        isPlaying: Boolean(isPlaying.value),
        positionMs: livePosMs,
        durationMs: Number(durationMs.value || currentSong.value?.durationMs || 0),
        playbackRate: effRate,
        lyricOffsetMs: Number(lyricOffsetMs.value) || 0,
        bluetoothDelayMs: Number(bluetoothDelayMs.value) || 0,
        volume: Number(volume.value ?? 0.8),
        sentAtMs: Date.now(),
      };
      if (miniPlayerChannel && !hasTauriBridge) {
        miniPlayerChannel.postMessage(payload);
      }
      invoke("push_mini_player_sync_cmd", { payload }).catch(() => {});
    } catch {
      // ignore serialization error
    }
  }

  function broadcastMiniPlayerState(force = false) {
    if (isStandaloneSubWindow || (!force && !isMiniPlayerMode.value)) return;
    try {
      const livePosMs =
        typeof getLivePlaybackPositionMs === "function"
          ? Math.round(getLivePlaybackPositionMs())
          : Math.round(currentPositionMs.value || 0);
      const effRate =
        typeof getEffectivePlaybackRate === "function"
          ? getEffectivePlaybackRate()
          : Number(playbackRate.value) || 1;
      const pal = effectiveCoverPalette.value;
      const payload = {
        type: "STATE_SYNC",
        stateSeq: ++miniPlayerStateSeq,
        visible: Boolean(isMiniPlayerMode.value),
        isPlaying: Boolean(isPlaying.value),
        positionMs: livePosMs,
        durationMs: Number(durationMs.value || currentSong.value?.durationMs || 0),
        playbackRate: effRate,
        lyricOffsetMs: Number(lyricOffsetMs.value) || 0,
        bluetoothDelayMs: Number(bluetoothDelayMs.value) || 0,
        volume: Number(volume.value ?? 0.8),
        playMode: String(playMode.value || "repeat_all"),
        isLiked:
          currentSong.value && typeof isSongLiked === "function"
            ? isSongLiked(currentSong.value)
            : false,
        qualityBadge: resolveMiniQualityBadge(),
        effectBadge: resolveMiniEffectBadge(),
        desktopLyricEnabled: Boolean(desktopLyricEnabled.value),
        resolvedColorScheme: String(resolvedColorScheme.value || "abyssal_dark"),
        lightThemeVariant: String(lightThemeVariant.value || "watercolor"),
        darkThemeVariant: String(darkThemeVariant.value || "obsidian_aurora"),
        motionLevel: resolveEffectiveMotionLevel(motionLevel.value),
        coverAccentPrimary: String(pal.dominantAccent || "#7c6af6"),
        coverAccentSecondary: String(pal.secondaryAccent || "#2dd4bf"),
        song: currentSong.value
          ? {
              hash: String(currentSong.value.hash || ""),
              title: String(currentSong.value.title || ""),
              artist: String(currentSong.value.artist || ""),
              coverUrl: String(currentSong.value.coverUrl || ""),
              isVip: Boolean(currentSong.value.isVip),
            }
          : null,
        lyric: isMiniPlayerMode.value
          ? serializeLyricForCrossWindow(currentLyric.value)
          : null,
        sentAtMs: Date.now(),
      };
      try {
        localStorage.setItem(MINI_PLAYER_SNAPSHOT_STORAGE_KEY, JSON.stringify(payload));
      } catch {
        // ignore storage quota error
      }
      if (miniPlayerChannel && !hasTauriBridge) {
        miniPlayerChannel.postMessage(payload);
      }
      invoke("push_mini_player_sync_cmd", { payload }).catch(() => {});
    } catch {
      // ignore serialization error
    }
  }

  let lastMiniCmdDedupKey = "";
  let lastMiniCmdAt = 0;
  let lastMiniToggleAt = 0;
  const seenMiniCmdIds = new Map<string, number>();

  function handleMiniPlayerCommand(msg: any) {
    if (isStandaloneSubWindow || !msg || typeof msg !== "object") return;
    if (msg.type === "CMD_REQUEST_SYNC" || msg.type === "CMD_MINI_PLAYER_READY") {
      broadcastMiniPlayerState();
      broadcastMiniPlayerPosition();
      return;
    }
    const now = performance.now();
    const cmdId = typeof msg.cmdId === "string" ? msg.cmdId.trim() : "";
    if (cmdId) {
      if (seenMiniCmdIds.has(cmdId)) return;
      seenMiniCmdIds.set(cmdId, now);
      if (seenMiniCmdIds.size > 32) {
        for (const [id, ts] of seenMiniCmdIds) {
          if (now - ts > 4000) seenMiniCmdIds.delete(id);
        }
      }
    } else {
      const dedupKey = `${msg.type}_${msg.positionMs ?? ""}_${msg.volume ?? ""}`;
      if (
        dedupKey === lastMiniCmdDedupKey &&
        now - lastMiniCmdAt < 220 &&
        msg.type !== "CMD_SET_VOLUME" &&
        msg.type !== "CMD_SEEK_MS"
      ) {
        return;
      }
      lastMiniCmdDedupKey = dedupKey;
      lastMiniCmdAt = now;
    }

    if (msg.type === "CMD_TOGGLE_PLAY") {
      // 覆盖 155ms 暂停淡出窗口，防止快速连点或双通道残余回调反转播放态
      if (now - lastMiniToggleAt < 180) return;
      lastMiniToggleAt = now;
      togglePlayPause();
    } else if (msg.type === "CMD_PREV") playPrev();
    else if (msg.type === "CMD_NEXT") playNext(false);
    else if (msg.type === "CMD_SEEK_MS") {
      seekToMs(Number(msg.positionMs) || 0);
      broadcastMiniPlayerPosition();
    } else if (msg.type === "CMD_SET_VOLUME" && Number.isFinite(Number(msg.volume))) {
      setVolume(Number(msg.volume));
    } else if (msg.type === "CMD_CYCLE_PLAY_MODE") {
      cyclePlayMode();
    } else if (msg.type === "CMD_TOGGLE_LIKE") {
      if (currentSong.value) {
        toggleFavoriteSong(currentSong.value);
        broadcastMiniPlayerState();
      }
    } else if (msg.type === "CMD_TOGGLE_DESKTOP_LYRIC") {
      desktopLyricEnabled.value = !desktopLyricEnabled.value;
    } else if (msg.type === "CMD_RESTORE_MAIN") {
      isMiniPlayerMode.value = false;
    } else if (msg.type === "CMD_OPEN_NOW_PLAYING") {
      isMiniPlayerMode.value = false;
      isNowPlayingOpen.value = true;
    } else if (msg.type === "CMD_HIDE_TO_TRAY") {
      isMiniPlayerMode.value = false;
    }
  }

  if (miniPlayerChannel && !isStandaloneSubWindow) {
    miniPlayerChannel.onmessage = (ev) => {
      handleMiniPlayerCommand(ev.data);
    };
  }

  if (!isStandaloneSubWindow) {
    listen<any>("bm-mini-player-cmd", (event) => {
      handleMiniPlayerCommand(event.payload);
    }).catch(() => {});

    listen<{ visible?: boolean }>("bm-mini-player-visibility", (event) => {
      const p = event.payload;
      if (p && typeof p.visible === "boolean" && isMiniPlayerMode.value !== p.visible) {
        isMiniPlayerMode.value = p.visible;
      }
    }).catch(() => {});

    listen<{ visible?: boolean }>("bm-main-window-visibility", (event) => {
      const vis = Boolean(event.payload?.visible);
      setNativeMainWindowVisible(vis);
    }).catch(() => {});

    listen<{ maximized?: boolean }>("bm-main-window-restored", () => {
      setNativeMainWindowVisible(true);
      syncDocumentThemeAttr();
      if (typeof window !== "undefined") {
        window.requestAnimationFrame(() => {
          window.dispatchEvent(new Event("resize"));
          window.requestAnimationFrame(() => {
            window.dispatchEvent(new Event("resize"));
          });
        });
      }
    }).catch(() => {});
  }

  if (!isStandaloneSubWindow) {
    listen<any>("bm-desktop-lyric-cmd", (event) => {
      handleDesktopLyricCommand(event.payload);
    }).catch(() => {});

    listen<any>("bm-desktop-lyric-visibility", (event) => {
      const p = event.payload;
      if (p && typeof p === "object") {
        if (typeof p.visible === "boolean" && desktopLyricEnabled.value !== p.visible) {
          desktopLyricEnabled.value = p.visible;
        }
        if (typeof p.locked === "boolean" && desktopLyricLocked.value !== p.locked) {
          skipNextOsLockSync = true;
          desktopLyricLocked.value = p.locked;
        }
        if (p.visible) {
          broadcastDesktopLyricState();
        }
      }
    }).catch(() => {});

    listen<{ locked?: boolean; notify?: boolean }>(
      "bm-desktop-lyric-lock-sync",
      (event) => {
        const p = event.payload;
        if (!p || typeof p !== "object" || typeof p.locked !== "boolean") return;
        const nextLocked = Boolean(p.locked);
        if (desktopLyricLocked.value !== nextLocked) {
          skipNextOsLockSync = true;
          desktopLyricLocked.value = nextLocked;
        }
        if (p.notify) {
          notify(
            nextLocked
              ? "已锁定桌面歌词（鼠标穿透生效，可通过托盘或 Ctrl+Shift+L 解锁）"
              : "已解锁桌面歌词，现在可自由拖动与调节"
          );
        }
      }
    ).catch(() => {});

    listen<{ x?: number; y?: number }>("bm-desktop-lyric-moved", (event) => {
      const p = event.payload;
      if (p && Number.isFinite(Number(p.x)) && Number.isFinite(Number(p.y))) {
        desktopLyricX.value = Math.round(Number(p.x));
        desktopLyricY.value = Math.round(Number(p.y));
      }
    }).catch(() => {});
  }

  watch(desktopLyricEnabled, (v) => {
    if (isStandaloneSubWindow) return;
    localStorage.setItem("bm_desktop_lyric", String(v));
    invoke("set_desktop_lyric_window_state_cmd", {
      visible: v,
      locked: desktopLyricLocked.value,
      width: desktopLyricWidth.value,
      height: null,
      x: desktopLyricX.value,
      y: desktopLyricY.value,
      resetPosition: desktopLyricX.value === null && desktopLyricY.value === null ? true : null,
      applyVisibility: true,
    })
      .then(() => {
        if (v) {
          broadcastDesktopLyricState();
          setTimeout(() => broadcastDesktopLyricState(), 90);
          setTimeout(() => broadcastDesktopLyricState(), 260);
        }
      })
      .catch(() => {});
    broadcastDesktopLyricState(!v && !hasTauriBridge);
    broadcastMiniPlayerState();
  });
  watch(
    desktopLyricLocked,
    (v) => {
      if (isStandaloneSubWindow) return;
      localStorage.setItem("bm_desktop_lyric_locked", String(v));
      const shouldSkipOs = skipNextOsLockSync;
      skipNextOsLockSync = false;
      if (desktopLyricEnabled.value && !shouldSkipOs) {
        kugouClient
          .setDesktopLyricWindowState(true, v, desktopLyricWidth.value)
          .catch(() => {});
      }
      broadcastDesktopLyricState();
    },
    { flush: "sync" }
  );
  function setDesktopLyricColorScheme(schemeId: string, colorHex?: string) {
    suppressDesktopLyricSchemeOverride = true;
    try {
      desktopLyricColorScheme.value = schemeId;
      localStorage.setItem("bm_desktop_lyric_scheme", schemeId);
      if (colorHex) {
        desktopLyricColor.value = colorHex;
      }
    } finally {
      suppressDesktopLyricSchemeOverride = false;
    }
    broadcastDesktopLyricState();
  }

  function snapDesktopLyricPosition(
    anchor: DesktopLyricSnapAnchor = "bottom",
    baseHeight?: number
  ) {
    invoke<[number, number] | null>("snap_desktop_lyric_position_cmd", {
      anchor,
      baseHeight: baseHeight ?? null,
    })
      .then((coords) => {
        if (
          Array.isArray(coords) &&
          coords.length === 2 &&
          Number.isFinite(coords[0]) &&
          Number.isFinite(coords[1])
        ) {
          desktopLyricX.value = Math.round(coords[0]);
          desktopLyricY.value = Math.round(coords[1]);
        }
      })
      .catch(() => {});
  }

  function resetDesktopLyricPosition() {
    snapDesktopLyricPosition("bottom");
  }

  function syncDesktopLyricFromNowPlaying() {
    const nextScheme = normalizeLyricThemeId(lyricTheme.value);
    setDesktopLyricColorScheme(nextScheme);
    desktopLyricSungMode.value = lyricSungMode.value;
    desktopLyricCustomPrimary.value = lyricCustomPrimaryHex.value;
    desktopLyricCustomSecondary.value = lyricCustomSecondaryHex.value;
    desktopLyricUnsungPreset.value = lyricUnsungPreset.value;
    desktopLyricUnsungCustomHex.value = lyricUnsungCustomHex.value;
    desktopLyricUnsungOpacity.value = clampVal(
      Number((lyricInactiveOpacity.value + 0.16).toFixed(2)),
      0.35,
      0.98
    );
    desktopLyricFontFamily.value = lyricFontFamily.value;
    desktopLyricFontWeight.value = lyricFontWeight.value;
    desktopLyricWordByWord.value = lyricWordByWord.value;
    desktopLyricShowTranslation.value = showTranslation.value;
    desktopLyricShowPhonetic.value = showPhonetic.value;
    desktopLyricGlowIntensity.value = clampVal(Math.round(lyricGlowIntensity.value * 0.85), 0, 150);
    broadcastDesktopLyricState();
  }

  function resetDesktopLyricSettingsToDefault() {
    setDesktopLyricColorScheme("dynamic_cover");
    desktopLyricSungMode.value = "gradient";
    desktopLyricCustomPrimary.value = "#5EEAD4";
    desktopLyricCustomSecondary.value = "#60A5FA";
    desktopLyricUnsungPreset.value = "auto";
    desktopLyricUnsungCustomHex.value = "#E6ECF5";
    desktopLyricUnsungOpacity.value = 0.78;
    desktopLyricStrokeWidth.value = "medium";
    desktopLyricStrokeColorMode.value = "auto";
    desktopLyricStrokeCustomHex.value = "#090D16";
    desktopLyricShadowEnabled.value = true;
    desktopLyricFontSize.value = 32;
    desktopLyricWidth.value = defaultCompactLyricWidth;
    desktopLyricLayoutMode.value = "center";
    desktopLyricBgStyle.value = "transparent";
    desktopLyricClarity.value = "crisp";
    desktopLyricFontFamily.value = "sans";
    desktopLyricFontWeight.value = 760;
    desktopLyricWordByWord.value = true;
    desktopLyricUnsungTone.value = "snow";
    desktopLyricLineSpacing.value = 6;
    desktopLyricGlowIntensity.value = 90;
    desktopLyricOpacity.value = 1.0;
    desktopLyricIdleFade.value = false;
    desktopLyricShowIntro.value = true;
    desktopLyricShowTranslation.value = true;
    desktopLyricShowPhonetic.value = false;
    broadcastDesktopLyricState();
  }

  watch(
    desktopLyricColor,
    (v) => {
      if (isStandaloneSubWindow) return;
      localStorage.setItem("bm_desktop_lyric_color", v);
      if (!suppressDesktopLyricSchemeOverride) {
        const resolved = resolveSchemeFromHex(v);
        desktopLyricColorScheme.value = resolved;
        localStorage.setItem("bm_desktop_lyric_scheme", resolved);
      }
      broadcastDesktopLyricState();
    },
    { flush: "sync" }
  );
  watch(desktopLyricFontSize, (v) => {
    if (isStandaloneSubWindow) return;
    const clamped = clampVal(Number(v) || 32, 18, 56);
    localStorage.setItem("bm_desktop_lyric_size_v2", String(clamped));
    broadcastDesktopLyricState();
  });
  watch(desktopLyricWidth, (v) => {
    if (isStandaloneSubWindow) return;
    const clamped = clampVal(v, 440, 1280);
    localStorage.setItem("bm_desktop_lyric_width_v2", String(clamped));
    const shouldSkipOs = skipNextOsWidthSync;
    skipNextOsWidthSync = false;
    if (desktopLyricEnabled.value && !shouldSkipOs) {
      invoke("resize_desktop_lyric_window_cmd", {
        width: clamped,
        height: 156,
        anchorCenter: true,
      }).catch(() => {});
    }
    if (!shouldSkipOs) {
      broadcastDesktopLyricState();
    }
  });
  watch(desktopLyricX, (v) => {
    if (v === null) localStorage.removeItem("bm_desktop_lyric_x");
    else localStorage.setItem("bm_desktop_lyric_x", String(Math.round(v)));
  });
  watch(desktopLyricY, (v) => {
    if (v === null) localStorage.removeItem("bm_desktop_lyric_y");
    else localStorage.setItem("bm_desktop_lyric_y", String(Math.round(v)));
  });
  watch(desktopLyricShowTranslation, (v) => {
    localStorage.setItem("bm_desktop_lyric_trans", String(v));
    broadcastDesktopLyricState();
  });
  watch(climaxModeEnabled, (v) =>
    localStorage.setItem("bm_climax_mode", String(v))
  );
  watch(replayGainEnabled, (v) => {
    localStorage.setItem("bm_replay_gain", String(v));
    if (v && !getAudioContext() && audio.crossOrigin) {
      ensureWebAudioGraph();
    }
    syncDspChain();
  });
  watch(nightLoudnessEnabled, (v) => {
    localStorage.setItem("bm_night_loudness", String(v));
    if (v && !getAudioContext() && audio.crossOrigin) {
      ensureWebAudioGraph();
    }
    syncDspChain();
  });
  watch(crossfadeSeconds, (v) => {
    localStorage.setItem("bm_crossfade_sec", String(v));
    if (v <= 0) {
      setCrossfadeDeckGains(1.0, 0.0);
      applyMasterOutputVolume(volume.value, 0.015);
    }
  });
  watch(playPauseFadeEnabled, (v) =>
    localStorage.setItem("bm_play_fade", String(v))
  );
  watch(downloadFeatureUnlocked, (v) => {
    localStorage.setItem("bm_download_unlocked", String(v));
    if (!v && settingsActiveSection.value === "downloads") {
      settingsActiveSection.value = "about";
    }
  });
  watch(downloadConcurrency, (v) => {
    const safe: 1 | 2 | 3 = v === 1 ? 1 : v === 3 ? 3 : 2;
    if (safe !== v) downloadConcurrency.value = safe;
    localStorage.setItem("bm_dl_concurrency_v1", String(safe));
    void processDownloadQueueWorker();
  });
  watch(autoSyncLocalAfterDownload, (v) =>
    localStorage.setItem("bm_dl_auto_sync_local_v1", String(v))
  );
  watch(downloadQuality, (v) => {
    const safe = normalizeConceptQuality(v);
    if (safe !== v) {
      downloadQuality.value = safe;
    }
    localStorage.setItem("bm_dl_quality", safe);
  });
  watch(downloadMusicDir, (v) => localStorage.setItem("bm_dl_music_dir", v));
  watch(downloadLrcDir, (v) => localStorage.setItem("bm_dl_lrc_dir", v));
  watch(embedCoverAndTags, (v) =>
    localStorage.setItem("bm_dl_embed_cover", String(v))
  );
  watch(embedLyricsInAudio, (v) =>
    localStorage.setItem("bm_dl_embed_lyric", String(v))
  );
  watch(exportStandaloneLrc, (v) =>
    localStorage.setItem("bm_dl_export_lrc", String(v))
  );
  watch(bilingualLrcExport, (v) =>
    localStorage.setItem("bm_dl_bilingual_lrc", String(v))
  );
  watch(skipExistingFiles, (v) =>
    localStorage.setItem("bm_dl_skip_exist", String(v))
  );
  watch(downloadFileNaming, (v) =>
    localStorage.setItem("bm_dl_file_naming_v1", v)
  );
  watch(downloadFolderOrganize, (v) =>
    localStorage.setItem("bm_dl_folder_org_v1", v)
  );

  // ================= 6. 账号、多账号切换、等级时长与概念版 VIP 状态 =================
  const session = ref<KugouSession | null>(null);
  const savedAccounts = ref<KugouSession[]>([]);
  const profile = ref<AccountProfile | null>(null);
  const vipStatus = ref<VipStatusInfo | null>(null);
  const isClaimingVip = ref(false);
  const gatewayDiag = ref<GatewayDiagResult | null>(null);
  const isDiagnosingGateway = ref(false);

  const isLoggedIn = computed(() => Boolean(session.value?.token));
  const isSyncingListeningGrade = ref(false);

  // ================= 8. 64-bit 风格母带级 WebAudio DSP 引擎 + 播放连续动量 =================
  const audio = new Audio();
  audio.preload = "auto";
  audio.crossOrigin = "anonymous";

  // Deck B 副轨实例：用于实现真正的时间轴双轨重叠交叉淡入淡出 (True Dual-Deck Overlapping Crossfade)
  const crossfadeAudio = new Audio();
  crossfadeAudio.preload = "auto";
  crossfadeAudio.crossOrigin = "anonymous";

  function compactQueueSong(s: Song): Song {
    const clean = normSong(s);
    return {
      id: clean.id,
      hash: clean.hash,
      hash128: clean.hash128,
      hash320: clean.hash320,
      hashFlac: clean.hashFlac,
      hashHiRes: clean.hashHiRes,
      accompanyHash: clean.accompanyHash || "",
      size128: clean.size128,
      size320: clean.size320,
      sizeFlac: clean.sizeFlac,
      sizeHiRes: clean.sizeHiRes,
      title: clean.title,
      artist: clean.artist,
      artistId: clean.artistId,
      authors: Array.isArray(clean.authors) ? clean.authors.slice(0, 6) : undefined,
      album: clean.album,
      albumId: clean.albumId,
      coverUrl: clean.coverUrl,
      durationMs: clean.durationMs,
      durationSeconds: clean.durationSeconds,
      albumAudioId: clean.albumAudioId,
      audioId: clean.audioId,
      mixSongId: clean.mixSongId,
      isVip: clean.isVip,
      privilege: clean.privilege,
      payType: clean.payType,
      failProcess: clean.failProcess,
      isCloud: clean.isCloud,
      fileId: clean.fileId,
      rawCoverTemplate: clean.rawCoverTemplate,
      hdCoverCandidates: clean.hdCoverCandidates,
      isLossless: clean.isLossless,
      isHiRes: clean.isHiRes,
      hasHiRes: clean.hasHiRes,
      hasHighQuality: clean.hasHighQuality,
      hasSq: clean.hasSq,
      isSq: clean.isSq,
      hasFlac: clean.hasFlac,
      has320: clean.has320,
      has128: clean.has128,
      availableQualities: clean.availableQualities,
      highestQuality: clean.highestQuality,
      publishDate: clean.publishDate,
      publishCompany: clean.publishCompany,
      language: clean.language,
      genre: clean.genre,
      lyricist: clean.lyricist,
      composer: clean.composer,
      arranger: clean.arranger,
      climaxStartMs: clean.climaxStartMs || 0,
      climaxEndMs: clean.climaxEndMs || 0,
    };
  }

  function extractQueueCovers(songs: Song[]): string[] {
    const covers: string[] = [];
    for (const s of songs) {
      if (s?.coverUrl && !covers.includes(s.coverUrl)) {
        covers.push(s.coverUrl);
        if (covers.length >= 4) break;
      }
    }
    return covers;
  }

  function buildQueueSignature(songs: Song[]): string {
    if (!songs.length) return "";
    const head = songs
      .slice(0, 6)
      .map((s) => s.hash || s.id || s.title)
      .join("|");
    const tail = songs[songs.length - 1]?.hash || songs[songs.length - 1]?.id || "";
    return `${songs.length}:${head}:${tail}`;
  }

  function areSongListsEquivalent(a: Song[], b: Song[]): boolean {
    if (a === b) return true;
    if (a.length !== b.length) return false;
    if (a.length === 0) return true;
    const sampleIndices = [
      0,
      Math.floor(a.length / 2),
      a.length - 1,
    ];
    for (const idx of sampleIndices) {
      const sa = a[idx];
      const sb = b[idx];
      const keyA = sa?.hash || sa?.id || sa?.title || "";
      const keyB = sb?.hash || sb?.id || sb?.title || "";
      if (keyA !== keyB) return false;
    }
    return buildQueueSignature(a) === buildQueueSignature(b);
  }

  function loadInitialRecentPlayQueues(): RecentPlayQueueSnapshot[] {
    try {
      const raw = localStorage.getItem(RECENT_QUEUES_STORAGE_KEY);
      if (!raw) return [];
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .filter((item) => item && Array.isArray(item.songs) && item.songs.length > 0)
        .slice(0, 5)
        .map((item) => {
          const ts = Number(item.lastPlayedAt || item.savedAt) || Date.now();
          const timeText = formatQueueSavedTimeText(ts);
          const songs = item.songs.slice(0, 120).map(compactQueueSong);
          return {
            id: String(item.id || `${ts}_${Math.random().toString(36).slice(2, 6)}`),
            title: String(item.title || "历史播放队列"),
            source: String(item.source || item.title || "播放记忆"),
            savedAt: ts,
            savedAtText: timeText,
            lastPlayedAt: ts,
            lastPlayedAtText: timeText,
            covers:
              Array.isArray(item.covers) && item.covers.length > 0
                ? item.covers.filter(Boolean).slice(0, 4)
                : extractQueueCovers(songs),
            songs,
            trackCount: Number(item.trackCount) || songs.length,
            activeHash: item.activeHash ? String(item.activeHash) : undefined,
          };
        });
    } catch {
      return [];
    }
  }

  const playQueue = ref<Song[]>([]);
  const currentQueueTitle = ref<string>(
    localStorage.getItem(CURRENT_QUEUE_TITLE_STORAGE_KEY) || "当前播放队列"
  );
  const recentPlayQueues = ref<RecentPlayQueueSnapshot[]>(loadInitialRecentPlayQueues());
  let originalQueueSnapshot: Song[] = [];

  function persistRecentPlayQueues() {
    try {
      const payload = recentPlayQueues.value.slice(0, 5).map((q) => ({
        ...q,
        songs: q.songs.slice(0, 100).map(compactQueueSong),
      }));
      localStorage.setItem(RECENT_QUEUES_STORAGE_KEY, JSON.stringify(payload));
    } catch {
      try {
        const slim = recentPlayQueues.value.slice(0, 5).map((q) => ({
          ...q,
          songs: q.songs.slice(0, 35).map(compactQueueSong),
        }));
        localStorage.setItem(RECENT_QUEUES_STORAGE_KEY, JSON.stringify(slim));
      } catch {
        // ignore storage quota error
      }
    }
  }

  let activeQueueSnapshotId: string | null = null;

  function saveQueueSnapshot(
    songs: Song[],
    title = currentQueueTitle.value,
    source?: string,
    activeHash?: string
  ) {
    if (!Array.isArray(songs) || songs.length === 0) return;
    const cleanTitle = (title || "").trim() || "当前播放队列";
    const cleanSource = (source || cleanTitle).trim();
    const compacted = songs.slice(0, 120).map(compactQueueSong);
    const sig = buildQueueSignature(compacted);
    const now = Date.now();
    const nowText = formatQueueSavedTimeText(now);
    const snapshot: RecentPlayQueueSnapshot = {
      id: `${now}_${Math.random().toString(36).slice(2, 6)}`,
      title: cleanTitle,
      source: cleanSource,
      savedAt: now,
      savedAtText: nowText,
      lastPlayedAt: now,
      lastPlayedAtText: nowText,
      covers: extractQueueCovers(compacted),
      songs: compacted,
      trackCount: songs.length,
      activeHash: activeHash || compacted[0]?.hash,
    };
    activeQueueSnapshotId = snapshot.id;

    const filtered = recentPlayQueues.value
      .filter((existing) => {
        const existingSig = buildQueueSignature(existing.songs);
        if (sig && existingSig === sig) return false;
        if (
          cleanTitle !== "当前播放队列" &&
          cleanTitle !== "当前播放列表" &&
          existing.title === cleanTitle &&
          existing.songs.length === compacted.length
        ) {
          return false;
        }
        return true;
      })
      .map((existing) => {
        const ts = Number(existing.lastPlayedAt || existing.savedAt) || now;
        const text = formatQueueSavedTimeText(ts);
        return {
          ...existing,
          savedAt: ts,
          savedAtText: text,
          lastPlayedAt: ts,
          lastPlayedAtText: text,
        };
      });

    recentPlayQueues.value = [snapshot, ...filtered].slice(0, 5);
    persistRecentPlayQueues();
  }

  function touchActiveQueueLastPlayed(activeSong?: Song | null) {
    if (playQueue.value.length === 0) return;
    const now = Date.now();
    const nowText = formatQueueSavedTimeText(now);
    const targetHash =
      activeSong?.hash || currentSong.value?.hash || playQueue.value[0]?.hash;
    const activeSig = buildQueueSignature(playQueue.value);
    const cleanTitle = (currentQueueTitle.value || "").trim() || "当前播放队列";

    let matchIdx = activeQueueSnapshotId
      ? recentPlayQueues.value.findIndex((existing) => existing.id === activeQueueSnapshotId)
      : -1;
    if (matchIdx < 0) {
      matchIdx = recentPlayQueues.value.findIndex((existing) => {
        const existingSig = buildQueueSignature(existing.songs);
        if (activeSig && existingSig === activeSig) return true;
        if (
          cleanTitle !== "当前播放队列" &&
          cleanTitle !== "当前播放列表" &&
          existing.title === cleanTitle
        ) {
          return true;
        }
        return false;
      });
    }

    if (matchIdx >= 0) {
      const target = recentPlayQueues.value[matchIdx];
      activeQueueSnapshotId = target.id;
      const compacted = playQueue.value.slice(0, 120).map(compactQueueSong);
      const updatedTarget: RecentPlayQueueSnapshot = {
        ...target,
        title: cleanTitle || target.title,
        savedAt: now,
        savedAtText: nowText,
        lastPlayedAt: now,
        lastPlayedAtText: nowText,
        covers: compacted.length > 0 ? extractQueueCovers(compacted) : target.covers,
        songs: compacted.length > 0 ? compacted : target.songs,
        activeHash: targetHash || target.activeHash,
        trackCount: playQueue.value.length || target.trackCount,
      };
      const others = recentPlayQueues.value
        .filter((_, idx) => idx !== matchIdx)
        .map((q) => {
          const ts = Number(q.lastPlayedAt || q.savedAt) || now;
          const text = formatQueueSavedTimeText(ts);
          return {
            ...q,
            savedAt: ts,
            savedAtText: text,
            lastPlayedAt: ts,
            lastPlayedAtText: text,
          };
        });
      recentPlayQueues.value = [updatedTarget, ...others].slice(0, 5);
      persistRecentPlayQueues();
    } else {
      saveQueueSnapshot(playQueue.value, cleanTitle, cleanTitle, targetHash);
    }
  }

  watch(currentQueueTitle, (val) => {
    localStorage.setItem(CURRENT_QUEUE_TITLE_STORAGE_KEY, val || "当前播放队列");
  });

  watch(isNowPlayingOpen, () => {
    isQueueOpen.value = false;
  });
  const currentIndex = ref<number>(-1);
  const savedPlayMode = localStorage.getItem("bm_play_mode") as PlayMode | null;
  const playMode = ref<PlayMode>(
    savedPlayMode && ["repeat_all", "repeat_one", "shuffle", "sequence"].includes(savedPlayMode)
      ? savedPlayMode
      : "repeat_all"
  );

  // Fisher-Yates 真随机洗牌无放回抽样池 + 随机播放回退历史栈（彻底解决随机重复抽歌与上一首无法回退问题）
  let shuffleForwardDeck: number[] = [];
  let shuffleHistoryStack: number[] = [];

  function rebuildShuffleDeck(excludeIdx = -1) {
    const len = playQueue.value.length;
    if (len <= 0) {
      shuffleForwardDeck = [];
      return;
    }
    const indices: number[] = [];
    for (let i = 0; i < len; i++) {
      if (i !== excludeIdx || len === 1) {
        indices.push(i);
      }
    }
    // 标准 Fisher-Yates 原地洗牌
    for (let i = indices.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      const tmp = indices[i];
      indices[i] = indices[j];
      indices[j] = tmp;
    }
    shuffleForwardDeck = indices;
  }

  // 音频输出声卡/耳机设备枚举与实时切换 (HTMLMediaElement.setSinkId / AudioContext.setSinkId)
  const audioOutputDevices = ref<AudioOutputDeviceItem[]>([
    { deviceId: "default", label: "系统默认音频输出设备", isDefault: true },
  ]);
  const selectedAudioDeviceId = ref<string>(
    localStorage.getItem("bm_audio_sink_id") || "default"
  );

  async function refreshAudioOutputDevices() {
    if (
      typeof navigator === "undefined" ||
      !navigator.mediaDevices ||
      typeof navigator.mediaDevices.enumerateDevices !== "function"
    ) {
      return;
    }
    try {
      const allDevs = await navigator.mediaDevices.enumerateDevices();
      const outputs = allDevs.filter((d) => d.kind === "audiooutput");
      const mapped: AudioOutputDeviceItem[] = [
        { deviceId: "default", label: "系统默认音频输出设备", isDefault: true },
      ];
      const seenIds = new Set<string>(["default"]);
      outputs.forEach((d, idx) => {
        const id = d.deviceId || "";
        if (!id || id === "communications" || seenIds.has(id)) return;
        seenIds.add(id);
        mapped.push({
          deviceId: id,
          label: d.label ? d.label.trim() : `外置声卡 / 耳机通道 #${idx + 1}`,
          isDefault: id === "default",
        });
      });
      audioOutputDevices.value = mapped;
      if (!mapped.some((d) => d.deviceId === selectedAudioDeviceId.value)) {
        selectedAudioDeviceId.value = "default";
      }
    } catch {
      // ignore permission/enumeration fallback
    }
  }

  async function selectAudioOutputDevice(deviceId: string, silent = false) {
    const targetId = deviceId || "default";
    selectedAudioDeviceId.value = targetId;
    localStorage.setItem("bm_audio_sink_id", targetId);
    const sinkArg = targetId === "default" ? "" : targetId;
    try {
      if (typeof (audio as any).setSinkId === "function") {
        await (audio as any).setSinkId(sinkArg);
      }
      if (typeof (crossfadeAudio as any).setSinkId === "function") {
        await (crossfadeAudio as any).setSinkId(sinkArg);
      }
      const ctx = getAudioContext();
      if (ctx && typeof (ctx as any).setSinkId === "function") {
        await (ctx as any).setSinkId(sinkArg);
      }
      if (!silent) {
        const matched = audioOutputDevices.value.find((d) => d.deviceId === targetId);
        notify(`已切换音频输出设备至：「${matched?.label || "系统默认设备"}」`);
      }
    } catch (e: any) {
      if (!silent) {
        notify(`切换音频输出声卡失败：${String(e?.message || e)}`);
      }
    }
  }

  let miniOverlayCleanupTimer: ReturnType<typeof setTimeout> | null = null;

  async function toggleMiniPlayerMode(forceState?: boolean) {
    const next = forceState !== undefined ? forceState : !isMiniPlayerMode.value;
    const effectiveMotion = resolveEffectiveMotionLevel(motionLevel.value);
    if (miniOverlayCleanupTimer !== null) {
      clearTimeout(miniOverlayCleanupTimer);
      miniOverlayCleanupTimer = null;
    }
    isMiniPlayerMode.value = next;
    if (next) {
      // 先广播最新播放态/主题/封面双色，再交由原生 GPU 空间舞台接管当前画面；
      // 绝不在截帧前关闭浮窗，避免切换瞬间先闪跳再收束。
      broadcastMiniPlayerState();
      broadcastMiniPlayerPosition();
    }
    await kugouClient.setMiniPlayerMode(next, effectiveMotion).catch(() => {});
    if (next) {
      broadcastMiniPlayerState();
      miniOverlayCleanupTimer = setTimeout(() => {
        miniOverlayCleanupTimer = null;
        if (isMiniPlayerMode.value) {
          isQueueOpen.value = false;
          isSearchPaletteOpen.value = false;
        }
      }, effectiveMotion === "none" ? 0 : 150);
      setTimeout(() => broadcastMiniPlayerState(), 110);
      setTimeout(() => broadcastMiniPlayerState(), 320);
    } else if (!hasTauriBridge) {
      broadcastMiniPlayerState(true);
    }
    if (!next) {
      notify("已还原标准主窗口视图");
    }
  }
  const currentSong = computed<Song | null>(() =>
    currentIndex.value >= 0 && currentIndex.value < playQueue.value.length
      ? playQueue.value[currentIndex.value]
      : null
  );

  // ================= 封面智能取色与 13 级 CIE L* 语义色阶系统 (对齐 Android NowPlayingSheet & Theme.kt) =================
  const coverPalette = ref<CoverExtractedPalette>({
    ...DEFAULT_COVER_EXTRACTED_PALETTE,
  });
  let paletteAnimRafId: number | null = null;
  let activeCoverExtractReq = 0;

  function animateCoverPaletteTo(target: CoverExtractedPalette) {
    if (paletteAnimRafId !== null) {
      cancelAnimationFrame(paletteAnimRafId);
      paletteAnimRafId = null;
    }
    if (
      isMiniPlayerMode.value ||
      (typeof document !== "undefined" && document.hidden)
    ) {
      coverPalette.value = { ...target };
      broadcastDesktopLyricState();
      broadcastMiniPlayerState();
      return;
    }
    const startSnap: CoverExtractedPalette = { ...coverPalette.value };
    const startWall = performance.now();
    const duration = 650;
    const step = (now: number) => {
      const t = Math.min(1, Math.max(0, (now - startWall) / duration));
      const eased = 1 - Math.pow(1 - t, 3);
      coverPalette.value = lerpPalette(startSnap, target, eased);
      if (t < 1) {
        paletteAnimRafId = requestAnimationFrame(step);
      } else {
        paletteAnimRafId = null;
        broadcastDesktopLyricState();
        broadcastMiniPlayerState();
      }
    };
    paletteAnimRafId = requestAnimationFrame(step);
  }

  watch(
    () => [
      currentSong.value?.hash,
      currentSong.value?.coverUrl,
      currentSong.value?.rawCoverTemplate,
    ],
    () => {
      const song = currentSong.value;
      const coverUrl = song?.coverUrl || "";
      const reqId = ++activeCoverExtractReq;
      if (!coverUrl && !song?.rawCoverTemplate) {
        activeHighResCoverUrl.value = "";
        activeHighResCoverMetrics.value = null;
        animateCoverPaletteTo({ ...DEFAULT_COVER_EXTRACTED_PALETTE });
        return;
      }

      activeHighResCoverUrl.value = "";
      activeHighResCoverMetrics.value = null;

      extractCoverMeshColors(coverUrl || song?.rawCoverTemplate || "")
        .then((extracted) => {
          if (reqId !== activeCoverExtractReq) return;
          animateCoverPaletteTo(extracted);
        })
        .catch(() => {
          if (reqId !== activeCoverExtractReq) return;
          animateCoverPaletteTo({ ...DEFAULT_COVER_EXTRACTED_PALETTE });
        });

      // 异步探测并缓存该曲目最高分辨率原图，播放页本体主图与封面灯箱无缝升级
      resolveBestCoverWithMetrics(song?.rawCoverTemplate || coverUrl, [
        ...(song?.hdCoverCandidates || []),
        coverUrl,
      ])
        .then((best) => {
          if (reqId !== activeCoverExtractReq || !best) return;
          activeHighResCoverUrl.value = best.url;
          activeHighResCoverMetrics.value = {
            width: best.width,
            height: best.height,
          };
          if (
            hdArtworkPreviewTarget.value?.song &&
            song &&
            isSameHistoryTrack(hdArtworkPreviewTarget.value.song, song)
          ) {
            hdArtworkPreviewTarget.value = {
              ...hdArtworkPreviewTarget.value,
              url: best.url,
              initialMetrics: { width: best.width, height: best.height },
            };
            hdArtworkPreviewUrl.value = best.url;
          }
        })
        .catch(() => {});
    },
    { immediate: true }
  );

  const isNowPlayingLightStage = computed<boolean>(() => {
    if (coverColorStageMode.value === "dark_lock") return false;
    return resolvedColorScheme.value === "watercolor_light";
  });

  const effectiveCoverPalette = computed<CoverExtractedPalette>(() => {
    if (coverColorStageMode.value === "off") {
      return getThemeFallbackCoverPalette(
        resolvedColorScheme.value,
        darkThemeVariant.value,
        lightThemeVariant.value
      );
    }
    if (isNowPlayingLightStage.value) {
      return buildLightTintedCoverPalette(coverPalette.value);
    }
    return coverPalette.value;
  });

  const coverColorRoles = computed<CoverColorRoles>(() => {
    const pal = effectiveCoverPalette.value;
    const seed = pal.dominantAccent || DEFAULT_COVER_EXTRACTED_PALETTE.dominantAccent;
    return toCoverColorRoles(seed, !isNowPlayingLightStage.value, pal);
  });

  const isPlaying = ref(false);
  const isBuffering = ref(false);
  const lastTrackNavDir = ref<"next" | "prev">("next");
  const currentPositionMs = ref(0);
  const durationMs = ref(0);
  // 全局原生无框确认弹窗状态（取代 window.confirm，适配四主题与顶层 <dialog>）
  const confirmDialog = reactive<{
    open: boolean;
    title: string;
    message: string;
    confirmText: string;
    cancelText: string;
    tone: "default" | "danger";
  }>({
    open: false,
    title: "确认操作",
    message: "",
    confirmText: "确定",
    cancelText: "取消",
    tone: "default",
  });
  let confirmDialogResolver: ((confirmed: boolean) => void) | null = null;

  function askConfirm(options: {
    title?: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    tone?: "default" | "danger";
  }): Promise<boolean> {
    if (confirmDialogResolver) {
      confirmDialogResolver(false);
      confirmDialogResolver = null;
    }
    confirmDialog.title = options.title || "确认操作";
    confirmDialog.message = options.message;
    confirmDialog.confirmText = options.confirmText || "确定";
    confirmDialog.cancelText = options.cancelText || "取消";
    confirmDialog.tone = options.tone || "default";
    confirmDialog.open = true;
    return new Promise<boolean>((resolve) => {
      confirmDialogResolver = resolve;
    });
  }

  function resolveConfirmDialog(confirmed: boolean) {
    confirmDialog.open = false;
    const resolver = confirmDialogResolver;
    confirmDialogResolver = null;
    resolver?.(confirmed);
  }

  const savedVolRaw = localStorage.getItem("bm_volume");
  const hasMigratedVolCurve = localStorage.getItem("bm_volume_curve_v2") === "1";
  let initialVolume = 0.62; // 新版 50dB 对数曲线默认 62%（约 -19dB，耳机即插即用不刺耳）
  if (savedVolRaw !== null && Number.isFinite(Number(savedVolRaw))) {
    const parsedVol = Math.max(0, Math.min(1, Number(savedVolRaw)));
    if (hasMigratedVolCurve) {
      initialVolume = parsedVol;
    } else {
      // 一次性无损迁移旧版线性音量：将历史线性响度映射至新版对数滑块位置，保持主观听感不突变
      initialVolume =
        parsedVol <= 0
          ? 0
          : Math.round(Math.max(0.01, Math.min(0.95, perceptualGainToSlider(parsedVol))) * 100) / 100;
      try {
        localStorage.setItem("bm_volume", String(initialVolume));
        localStorage.setItem("bm_volume_curve_v2", "1");
      } catch {
        // ignore storage errors
      }
    }
  } else if (!hasMigratedVolCurve) {
    try {
      localStorage.setItem("bm_volume_curve_v2", "1");
    } catch {
      // ignore storage errors
    }
  }
  const volume = ref(initialVolume);
  audio.volume = sliderToPerceptualGain(volume.value);

  // 节流 / 防抖持久化队列与播放进度，并在暂停/切歌/隐藏/退出时立即冲刷落盘
  let queuePersistTimerId: number | null = null;
  let queuePersistIdleId: number | null = null;
  let queuePersistDirty = false;
  let lastSavedPosWallMs = 0;

  function persistPlayQueueNow() {
    try {
      const compactQueue = playQueue.value.slice(0, 2000).map((s) => compactQueueSong(s));
      localStorage.setItem("bm_saved_queue", JSON.stringify(compactQueue));
      localStorage.setItem("bm_saved_index", String(currentIndex.value));
    } catch {
      try {
        localStorage.setItem(
          "bm_saved_queue",
          JSON.stringify(playQueue.value.slice(0, 400))
        );
      } catch {
        // ignore quota error
      }
    } finally {
      queuePersistDirty = false;
    }
  }

  function schedulePersistPlayQueue() {
    queuePersistDirty = true;
    if (queuePersistTimerId !== null) {
      window.clearTimeout(queuePersistTimerId);
      queuePersistTimerId = null;
    }
    if (queuePersistIdleId !== null && typeof (window as any).cancelIdleCallback === "function") {
      (window as any).cancelIdleCallback(queuePersistIdleId);
      queuePersistIdleId = null;
    }
    queuePersistTimerId = window.setTimeout(() => {
      queuePersistTimerId = null;
      if (typeof (window as any).requestIdleCallback === "function") {
        queuePersistIdleId = (window as any).requestIdleCallback(
          () => {
            queuePersistIdleId = null;
            if (queuePersistDirty) persistPlayQueueNow();
          },
          { timeout: 1500 }
        );
      } else {
        if (queuePersistDirty) persistPlayQueueNow();
      }
    }, 1200);
  }

  function flushPlaybackPersistenceNow() {
    if (queuePersistTimerId !== null) {
      window.clearTimeout(queuePersistTimerId);
      queuePersistTimerId = null;
    }
    if (queuePersistIdleId !== null && typeof (window as any).cancelIdleCallback === "function") {
      (window as any).cancelIdleCallback(queuePersistIdleId);
      queuePersistIdleId = null;
    }
    if (queuePersistDirty) {
      persistPlayQueueNow();
    } else {
      try {
        localStorage.setItem("bm_saved_index", String(currentIndex.value));
      } catch {
        // ignore
      }
    }
    try {
      const pos = Math.max(0, Math.round(currentPositionMs.value || 0));
      localStorage.setItem("bm_saved_pos_ms", String(pos));
      lastSavedPosWallMs = Date.now();
    } catch {
      // ignore
    }
  }

  // 跨曲交叉淡入淡出、播放/暂停淡入淡出定时器竞态保护与 HTML5 Audio 包络平滑控制器
  let isCrossfadeTransitioning = false;
  let volumeRampRafId: number | null = null;
  let pauseFadeTimerId: number | null = null;
  let isPauseFadingOut = false;
  let playPauseToggleSeq = 0;

  function cancelVolumeRamp() {
    if (volumeRampRafId !== null) {
      cancelAnimationFrame(volumeRampRafId);
      volumeRampRafId = null;
    }
  }
  function rampHtmlAudioVolume(
    fromVol: number,
    toVol: number,
    durationMsVal: number,
    onDone?: () => void
  ) {
    cancelVolumeRamp();
    const startVal = Math.max(0, Math.min(1, fromVol));
    const endVal = Math.max(0, Math.min(1, toVol));
    const totalMs = Math.max(30, durationMsVal);
    const startWall = performance.now();
    audio.volume = startVal;
    const tick = (now: number) => {
      const progress = Math.min(1, (now - startWall) / totalMs);
      audio.volume = startVal + (endVal - startVal) * progress;
      if (progress < 1) {
        volumeRampRafId = requestAnimationFrame(tick);
      } else {
        volumeRampRafId = null;
        onDone?.();
      }
    };
    volumeRampRafId = requestAnimationFrame(tick);
  }

  // 记录单曲播放开始时间戳，供 CSCC 三段式真实听歌时长上报使用
  let currentSongPlayStartWallMs = 0;
  // 播放请求单调递增令牌与多 CDN 备用直链容灾队列（对齐 Android KGM 播放器自动切源机制）
  let activePlayRequestId = 0;
  let currentCandidateUrls: string[] = [];
  let currentCandidateIdx = 0;
  let hasRetriedWithoutCors = false;

  // 播放 <-> 暂停 880ms 连续动量相位因子 (0.0 静止 ~ 1.0 全速)，消除暂停时背景与唱盘跳变
  const playMomentum = ref<number>(0);
  let momentumRafId: number | null = null;
  function animatePlayMomentum(target: number) {
    if (momentumRafId) cancelAnimationFrame(momentumRafId);
    const startVal = playMomentum.value;
    const startTime = performance.now();
    const duration = 880;
    const step = (now: number) => {
      const t = Math.min(1, (now - startTime) / duration);
      const eased = t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
      playMomentum.value = startVal + (target - startVal) * eased;
      if (t < 1) {
        momentumRafId = requestAnimationFrame(step);
      }
    };
    momentumRafId = requestAnimationFrame(step);
  }
  watch(isPlaying, (playing) => {
    animatePlayMomentum(playing ? 1 : 0);
  });

  const rawSavedPrefQuality = localStorage.getItem("bm_pref_quality");
  const initialPrefQuality = normalizeConceptQuality(rawSavedPrefQuality);
  if (rawSavedPrefQuality && rawSavedPrefQuality !== initialPrefQuality) {
    localStorage.setItem("bm_pref_quality", initialPrefQuality);
  }
  const preferredQuality = ref<AudioQualityValue>(initialPrefQuality);

  // 酷狗云端特种合成音效流 (仅保留钢琴/清唱/重低音/古风/唢呐/DJ，屏蔽超纲蝰蛇音质)
  const rawSavedCloudEffect = (localStorage.getItem("bm_cloud_stream_effect") || "")
    .trim()
    .toLowerCase();
  const blockedOverTierEffects = new Set([
    "viper_atmos",
    "viper_tape",
    "viper_clear",
    "master",
    "atmos",
    "tape",
    "clear",
    "super",
    "ultra",
  ]);
  const initialCloudEffect = blockedOverTierEffects.has(rawSavedCloudEffect)
    ? ""
    : rawSavedCloudEffect;
  if (rawSavedCloudEffect && !initialCloudEffect) {
    localStorage.removeItem("bm_cloud_stream_effect");
  }
  const activeCloudStreamEffect = ref<string>(initialCloudEffect);
  const currentPlaybackInfo = ref<PlaybackResult | null>(null);
  const currentSongAvailableQualities = computed(() =>
    getSongAvailableQualities(currentSong.value)
  );

  function syncEnrichedSongMetadata(target: Song | null | undefined, enriched: Song) {
    if (!target || !enriched) return;
    const clean = normSong(enriched);
    target.hash128 = clean.hash128;
    if (clean.mp3Hash128) target.mp3Hash128 = clean.mp3Hash128;
    if (clean.canonicalHash) target.canonicalHash = clean.canonicalHash;
    target.hash320 = clean.hash320;
    target.hashFlac = clean.hashFlac;
    target.hashHiRes = clean.hashHiRes;
    target.size128 = clean.size128;
    target.size320 = clean.size320;
    target.sizeFlac = clean.sizeFlac;
    target.sizeHiRes = clean.sizeHiRes;
    target.isHiRes = clean.isHiRes;
    target.isLossless = clean.isLossless;
    target.hasHiRes = clean.hasHiRes;
    target.hasFlac = clean.hasFlac;
    target.hasSq = clean.hasSq;
    target.isSq = clean.isSq;
    target.has320 = clean.has320;
    target.has128 = clean.has128;
    target.hasHighQuality = clean.hasHighQuality;
    target.availableQualities = clean.availableQualities;
    target.highestQuality = clean.highestQuality;
    if (clean.privilegeVerified) {
      target.privilegeVerified = true;
      target.privilegeSchemaVer = clean.privilegeSchemaVer;
      target.isVip = clean.isVip;
      if (typeof clean.isFreePrivilege === "boolean") {
        target.isFreePrivilege = clean.isFreePrivilege;
      }
    }
    if (
      typeof clean.privilege === "number" ||
      typeof clean.payType === "number" ||
      typeof clean.failProcess === "number"
    ) {
      target.isVip = clean.isVip;
      if (typeof clean.privilege === "number") target.privilege = clean.privilege;
      if (typeof clean.payType === "number") target.payType = clean.payType;
      if (typeof clean.failProcess === "number") target.failProcess = clean.failProcess;
    } else if (clean.isVip) {
      target.isVip = true;
    }
    if (clean.artist && (!target.artist || target.artist === "未知歌手")) {
      target.artist = clean.artist;
    }
    if (clean.artistId && (!target.artistId || target.artistId === "0")) {
      target.artistId = clean.artistId;
    }
    if (Array.isArray(clean.authors) && clean.authors.length > 0 && (!target.authors || target.authors.length <= 1)) {
      target.authors = clean.authors;
    }
    if (!target.coverUrl && clean.coverUrl) target.coverUrl = clean.coverUrl;
    if (clean.rawCoverTemplate && !target.rawCoverTemplate) {
      target.rawCoverTemplate = clean.rawCoverTemplate;
    }
    if (Array.isArray(clean.hdCoverCandidates) && clean.hdCoverCandidates.length > 0 && (!target.hdCoverCandidates || target.hdCoverCandidates.length === 0)) {
      target.hdCoverCandidates = clean.hdCoverCandidates;
    }
    if (!target.album && clean.album) target.album = clean.album;
    if ((!target.albumId || target.albumId === "0") && clean.albumId && clean.albumId !== "0") {
      target.albumId = clean.albumId;
    }
    if (!target.albumAudioId && clean.albumAudioId) {
      target.albumAudioId = clean.albumAudioId;
    }
    if (!target.durationMs && clean.durationMs) {
      target.durationMs = clean.durationMs;
      target.durationSeconds = clean.durationSeconds;
    }
    if (clean.publishDate) target.publishDate = clean.publishDate;
    if (clean.publishCompany) target.publishCompany = clean.publishCompany;
    if (clean.language) target.language = clean.language;
    if (clean.genre) target.genre = clean.genre;
    if (clean.lyricist) target.lyricist = clean.lyricist;
    if (clean.composer) target.composer = clean.composer;
    if (clean.arranger) target.arranger = clean.arranger;
    if (clean.accompanyHash && !target.accompanyHash) {
      target.accompanyHash = clean.accompanyHash;
    }
    if (clean.climaxEndMs && clean.climaxEndMs > (clean.climaxStartMs || 0)) {
      target.climaxStartMs = clean.climaxStartMs || 0;
      target.climaxEndMs = clean.climaxEndMs;
    }
  }

  let activeCurrentDetailSeq = 0;
  async function refreshCurrentSongDetailInfo(
    song: Song,
    pb?: PlaybackResult | null,
    reqIdSnapshot?: number
  ) {
    const seq = ++activeCurrentDetailSeq;
    isLoadingCurrentSongDetail.value = true;
    try {
      const targetQuality =
        pb?.actualQuality || preferredQuality.value || "high";
      const detail = await kugouClient.getSongDetailInfo(song, targetQuality, pb ?? null);
      if (seq !== activeCurrentDetailSeq) return;
      if (
        reqIdSnapshot !== undefined &&
        reqIdSnapshot !== activePlayRequestId
      ) {
        return;
      }
      if (
        !currentSong.value ||
        !isSameHistoryTrack(currentSong.value, song)
      ) {
        return;
      }
      // 将当前正在播放流的实时声学参数同步合并进详情对象，确保 100% 一致
      if (pb) {
        const effCodec =
          pb.codec || (pb.extName ? pb.extName.toUpperCase() : detail.codec || "MP3");
        const effBytes =
          pb.fileSizeBytes > 0 ? pb.fileSizeBytes : detail.fileSizeBytes || 0;
        const effDur =
          (pb.durationMs && pb.durationMs > 0 ? pb.durationMs : 0) ||
          detail.durationMs ||
          currentSong.value.durationMs ||
          0;
        detail.codec = effCodec;
        detail.actualQuality = pb.actualQuality || detail.actualQuality;
        detail.qualityLabel = pb.qualityLabel || detail.qualityLabel;
        detail.sampleRateHz = pb.sampleRateHz || detail.sampleRateHz;
        detail.bitDepth = pb.bitDepth || detail.bitDepth;
        detail.bitrateKbps = pb.bitrateKbps || detail.bitrateKbps;
        detail.channels = pb.channels || detail.channels || 2;
        detail.fileSizeBytes = effBytes;
        detail.durationMs = effDur;
        detail.albumAudioId =
          pb.albumAudioId || detail.albumAudioId || Number(currentSong.value.albumAudioId) || 0;
        detail.audioQuality = `${pb.qualityLabel} (${effCodec})`;
        detail.sampleRate = `${((pb.sampleRateHz || 44100) / 1000).toFixed(1)} kHz / ${
          pb.bitDepth || 16
        } bit (${pb.channels || 2}ch · ${effCodec})`;
        detail.bitRate = `${pb.bitrateKbps || 320} kbps`;
        if (effBytes > 0) {
          detail.fileSize = formatAudioSizeBytes(effBytes) || detail.fileSize;
        }
        if (pb.resolvedHash) {
          detail.fileHash = pb.resolvedHash.toUpperCase();
        }
      }
      if (detail.song) {
        syncEnrichedSongMetadata(currentSong.value, detail.song);
        recordSongToRecentHistory(currentSong.value);
        syncWindowsMediaSession(currentSong.value);
      }
      currentSongDetailInfo.value = detail;
      if (
        songDetailModalSong.value &&
        isSameHistoryTrack(songDetailModalSong.value, song)
      ) {
        syncEnrichedSongMetadata(songDetailModalSong.value, detail.song);
        activeSongDetailInfo.value = detail;
      }
    } catch {
      // 详情异步富化失败不影响当前播放链路
    } finally {
      if (seq === activeCurrentDetailSeq) {
        isLoadingCurrentSongDetail.value = false;
      }
    }
  }
  const currentLyric = ref<LyricData>({
    lines: [],
    rawText: "",
    isKrc: false,
    hasTranslation: false,
    hasPhonetic: false,
  });
  const isLoadingLyric = ref(false);

  // 歌曲评论区状态（评论弹窗独立会话，绝不受后台播放切歌污染）
  const activeSongComments = ref<SongComment[]>([]);
  const activeSongCommentTotal = ref<number>(0);
  // 当前正在播放歌曲的独立评论总数（仅用于播放页角标，切歌时轻量拉取计数，不触碰弹窗会话）
  const currentSongCommentTotal = ref<number>(0);
  const commentSortType = ref<"hot" | "new">("hot");
  const isLoadingComments = ref(false);
  const expandedFloorCommentId = ref<string | null>(null);
  const floorRepliesMap = reactive<Record<string, SongComment[]>>({});
  const isLoadingFloorReplies = ref<boolean>(false);

  let currentSongCommentCountSeq = 0;

  function getCommentSongKey(song: Song | null | undefined): string {
    if (!song) return "";
    return `${String(song.hash || "").trim().toLowerCase()}::${String(
      song.albumAudioId || song.mixSongId || song.id || ""
    ).trim()}`;
  }

  function setCurrentSongCommentTotal(total: number) {
    if (Number.isFinite(total) && total >= 0) {
      currentSongCommentTotal.value = Math.round(total);
    }
  }

  function setActiveSongCommentTotal(total: number) {
    if (Number.isFinite(total) && total >= 0) {
      const rounded = Math.round(total);
      activeSongCommentTotal.value = rounded;
      if (
        commentsModalSong.value &&
        currentSong.value &&
        isSameHistoryTrack(commentsModalSong.value, currentSong.value)
      ) {
        currentSongCommentTotal.value = Math.max(
          currentSongCommentTotal.value,
          rounded
        );
      }
    }
  }

  async function refreshCurrentSongCommentCount(
    song: Song | null = currentSong.value
  ) {
    const reqSeq = ++currentSongCommentCountSeq;
    if (!song) {
      currentSongCommentTotal.value = 0;
      return;
    }
    const targetKey = getCommentSongKey(song);
    currentSongCommentTotal.value = 0;
    try {
      const count = await kugouClient.getSongCommentCount(song);
      if (
        reqSeq !== currentSongCommentCountSeq ||
        getCommentSongKey(currentSong.value) !== targetKey
      ) {
        return;
      }
      currentSongCommentTotal.value =
        Number.isFinite(count) && count > 0 ? Math.round(count) : 0;
    } catch {
      if (
        reqSeq !== currentSongCommentCountSeq ||
        getCommentSongKey(currentSong.value) !== targetKey
      ) {
        return;
      }
      if (isBrowserDevPreviewEnv() && playQueue.value[0]?.hash === song.hash) {
        currentSongCommentTotal.value = 136;
      } else {
        currentSongCommentTotal.value = 0;
      }
    }
  }

  function getLivePlaybackPositionMs(): number {
    if (audio.src && !audio.paused && !audio.ended) {
      const t = audio.currentTime;
      if (Number.isFinite(t) && t >= 0) {
        return t * 1000;
      }
    }
    return currentPositionMs.value;
  }

  function getEffectivePlaybackRate(): number {
    if (audio.src && Number.isFinite(audio.playbackRate) && audio.playbackRate > 0) {
      return audio.playbackRate;
    }
    return playbackRate.value || 1;
  }

  let subwindowPositionSyncTimerId: number | null = null;
  function syncSubwindowPositionTimer() {
    if (isStandaloneSubWindow) return;
    const needSync =
      isPlaying.value &&
      !audio.paused &&
      (desktopLyricEnabled.value || isMiniPlayerMode.value);
    if (!needSync) {
      if (subwindowPositionSyncTimerId !== null) {
        window.clearInterval(subwindowPositionSyncTimerId);
        subwindowPositionSyncTimerId = null;
      }
      return;
    }
    if (subwindowPositionSyncTimerId === null) {
      subwindowPositionSyncTimerId = window.setInterval(() => {
        if (!isPlaying.value || audio.paused) {
          syncSubwindowPositionTimer();
          return;
        }
        if (desktopLyricEnabled.value) {
          broadcastDesktopLyricPosition();
        }
        if (isMiniPlayerMode.value) {
          broadcastMiniPlayerPosition();
        }
      }, 1200);
    }
  }

  if (!isStandaloneSubWindow) {
    watch(
      [currentSong, currentLyric, isPlaying, playMode, currentPlaybackInfo],
      () => {
        broadcastDesktopLyricState();
        broadcastMiniPlayerState();
      },
      { deep: false }
    );
    watch([isPlaying, desktopLyricEnabled, isMiniPlayerMode], () => {
      syncSubwindowPositionTimer();
    });
  }

  function getSongChorusWindowMs(
    song: Song | null = currentSong.value,
    pb: PlaybackResult | null = currentPlaybackInfo.value
  ): { startMs: number; endMs: number; isDetected: boolean } {
    const dur = durationMs.value || song?.durationMs || 0;
    const rawStart = pb?.climaxStartMs || song?.climaxStartMs || 0;
    const rawEnd = pb?.climaxEndMs || song?.climaxEndMs || 0;
    if (rawStart > 0 && rawEnd > rawStart + 6000) {
      return { startMs: rawStart, endMs: rawEnd, isDetected: true };
    }
    if (dur > 22000 && dur < 92000) {
      const startMs = Math.round(dur * 0.3);
      const endMs = Math.min(dur - 3000, startMs + 35000);
      return { startMs, endMs, isDetected: false };
    }
    const startMs = 45000;
    const endMs = dur > 0 ? Math.min(85000, Math.max(startMs + 15000, dur - 3000)) : 85000;
    return { startMs, endMs, isDetected: false };
  }

  async function toggleChorusQuickSkim(forceState?: boolean) {
    const next = forceState !== undefined ? forceState : !climaxModeEnabled.value;
    climaxModeEnabled.value = next;
    if (next && currentSong.value) {
      const { startMs, endMs } = getSongChorusWindowMs(
        currentSong.value,
        currentPlaybackInfo.value
      );
      if (audio.src && (currentPositionMs.value < startMs || currentPositionMs.value >= endMs)) {
        seekToMs(startMs);
      }
      if (audio.paused) {
        await togglePlayPause();
      }
      notify("已开启「副歌速刷」：直达每首高潮黄金段落试听");
    } else {
      notify(next ? "已开启「副歌速刷」模式" : "已关闭副歌速刷，恢复完整曲目播放");
    }
  }

  // ================= 模块化领域 Composable 注入 (DSP / 发现电台 / 音乐库详情 / 搜索) =================
  function notify(msg: string) {
    statusBanner.value = msg;
    setTimeout(() => {
      if (statusBanner.value === msg) {
        statusBanner.value = "";
      }
    }, 3400);
  }

  const {
    masterEffectEnabled,
    eqBandCount,
    eqFrequencies,
    eqBandCenterHz,
    eqBandQFactors,
    eqGains,
    eqBandTypes,
    eqQFactor,
    eqHighPassHz,
    eqLowPassHz,
    spectralTiltDb,
    masterPreampDb,
    estimatedPeakHeadroomDb,
    channelBalance,
    speakerDelaySplitMs,
    channelWiringMode,
    phaseInvertL,
    phaseInvertR,
    antiClippingEnabled,
    activeEqPreset,
    activeTargetCurveId,
    peqBypass,
    viperEffects,
    activeViperEffectId,
    peqMemorySlots,
    acousticField,
    spectrumBars,
    rhythmTelemetry,
    getAudioContext,
    getMasterPreampGain,
    getMasterOutputGain,
    applyMasterOutputVolume,
    getRafId,
    isCurrentlyPauseFadingOut,
    setPauseFadingOutState,
    getSharedAudioContext,
    ensureWebAudioGraph,
    computeTargetMasterPreampLinearGain,
    cancelPendingPauseFade,
    syncDspChain,
    applyViperEffect,
    toggleMasterEffect,
    applyAudiophileTargetCurve,
    setBandQFactor,
    setBandCenterHz,
    resetAcousticAndEqToDefault,
    trackReplayGainDb,
    measuredLufs,
    dynamicReplayGainDb,
    effectiveReplayGainDb,
    setTrackReplayGainDb,
    setCrossfadeDeckGains,
    syncIndependentPitchShifter,
    startSpectrumLoop,
    syncSpectrumLoopDemand,
    scheduleAudioContextSuspend,
    cancelAudioContextSuspend,
    setEqPreset,
    updateSingleEqBand,
    setEqBandGainsBatch,
    setEqBandCount,
    smoothCurrentEqCurve,
    scaleCurrentEqGains,
    applySpectralTilt,
    savePeqToSlot,
    loadPeqFromSlot,
    resetSinglePeqSlot,
    resetAllPeqSlots,
    setBandFilterTopology,
    exportTunerProfileJson,
    importTunerProfileJson,
  } = createWebAudioDspModule({
    audio,
    crossfadeAudio,
    isPlaying,
    playMomentum,
    volume,
    replayGainEnabled,
    nightLoudnessEnabled,
    pitchSemitones,
    linkPitchWithSpeed,
    notify,
    cancelVolumeRamp,
    hasVisualSpectrumConsumer: () =>
      isAppForegroundActive() &&
      (isNowPlayingOpen.value || activeTab.value === "tuner" || activeTab.value === "radio"),
  });

  let clipboardDetectTimerId: number | null = null;
  let isCheckingClipboardShare = false;

  async function checkClipboardForKugouShare(force = false) {
    if (isStandaloneSubWindow || isCheckingClipboardShare) return;
    if (!force && !clipboardShareDetectEnabled.value) return;
    if (!force && !isAppForegroundActive()) return;
    if (detectedSharedTarget.value || shareModalSong.value || shareModalPlaylist.value) return;

    isCheckingClipboardShare = true;
    try {
      let clipText = "";
      try {
        clipText = await kugouClient.readClipboardText();
      } catch {
        // 非 Tauri 浏览器预览环境下退回 Web Clipboard API
        if (
          typeof navigator !== "undefined" &&
          navigator.clipboard?.readText &&
          typeof document !== "undefined" &&
          document.hasFocus()
        ) {
          try {
            clipText = await navigator.clipboard.readText();
          } catch {
            clipText = "";
          }
        }
      }
      const trimmed = (clipText || "").trim();
      if (!trimmed || !containsKugouShareSignal(trimmed)) return;
      if (!force && isClipboardShareTextHandled(trimmed)) return;
      markClipboardShareTextHandled(trimmed);

      const resolved = await kugouClient.parseSharedMusic(trimmed);
      if (
        (resolved.targetType === "song" && resolved.song) ||
        (resolved.targetType === "playlist" &&
          (resolved.playlist || (resolved.songs && resolved.songs.length > 0)))
      ) {
        detectedSharedTarget.value = resolved;
      }
    } catch {
      // ignore invalid or non-kugou clipboard contents
    } finally {
      isCheckingClipboardShare = false;
    }
  }

  function scheduleClipboardShareCheck(delayMs = 420) {
    if (isStandaloneSubWindow || !clipboardShareDetectEnabled.value) return;
    if (clipboardDetectTimerId !== null) {
      window.clearTimeout(clipboardDetectTimerId);
    }
    clipboardDetectTimerId = window.setTimeout(() => {
      clipboardDetectTimerId = null;
      void checkClipboardForKugouShare(false);
    }, delayMs);
  }

  watch([isNowPlayingOpen, activeTab, isPlaying], () => {
    syncSpectrumLoopDemand();
  });

  if (typeof document !== "undefined") {
    ensureAppPowerVisibilityTracking();
    subscribeAppPowerVisibility((active) => {
      syncSpectrumLoopDemand();
      if (!active) {
        if (!isStandaloneSubWindow) {
          flushPlaybackPersistenceNow();
        }
        return;
      }
      // 从最小化或托盘恢复前台：立刻同步音频进度（补偿后台降频跳过），并延迟检测剪贴板分享口令
      if (!isStandaloneSubWindow) {
        const liveMs = getLivePlaybackPositionMs();
        if (Math.abs(liveMs - currentPositionMs.value) >= 120) {
          currentPositionMs.value = liveMs;
        }
        scheduleClipboardShareCheck(450);
      }
    });
  }
  if (typeof window !== "undefined" && !isStandaloneSubWindow) {
    window.addEventListener("focus", () => {
      scheduleClipboardShareCheck(420);
    });
    window.addEventListener("beforeunload", flushPlaybackPersistenceNow);
  }

  let pushNavHistoryBridge: () => void = () => {};

  const {
    discoverSubView,
    discoverHeroExpanding,
    isGlobalQueueOpen: _moduleGlobalQueueOpen,
    recommendSongs,
    isLoadingRecommend,
    personalFmSongs,
    personalFmIndex,
    isLoadingFm,
    isAppendingFmQueue,
    isPersonalFmDrawerOpen,
    currentFmSong,
    nextFmSong,
    newSongPeriod,
    newSongs,
    newSongsByPeriod,
    isLoadingNewSongs,
    isRefreshingAllNewSongs,
    rankList,
    isLoadingRanks,
    isRankGridModalOpen,
    selectedRankCategory,
    activeRank,
    activeRankSongs,
    isLoadingRankSongs,
    isLoadingMoreRankSongs,
    fmChannels,
    selectedFmCategory,
    activeFmChannel,
    fmStationSongs,
    isLoadingFmStation,
    isStageScrolledDown,
    scrollStageToTop,
    loadDailyRecommend,
    loadPersonalFm,
    fetchMorePersonalFmForQueue,
    nextPersonalFmTrack,
    loadNewSongExpress,
    refreshAllNewSongTags,
    loadRankList,
    openRankDetail,
    refreshActiveRank,
    resolveFmChannelCovers,
    refreshDiscoverAll,
    openDailyRecommendPage,
    openGuessYouLikePage,
    openNewSongsPage,
    openRadioPlazaPage,
    playRankDirectly,
    loadFmChannels,
    loadFmStationSongs,
    clearDiscoverCaches,
  } = createDiscoverAndFmModule({
    activeTab,
    isNowPlayingOpen,
    playQueue,
    currentIndex,
    durationMs,
    notify,
    playSongFromList: (s, l, t) => playSongFromList(s, l, t),
    pushNavHistory: () => pushNavHistoryBridge(),
  });

  const isGlobalQueueOpen = computed<boolean>({
    get: () => isQueueOpen.value,
    set: (v) => {
      isQueueOpen.value = v;
    },
  });

  const canNavigateBack = computed(() =>
    isNowPlayingOpen.value ||
    navHistoryStack.value.length > 0 ||
    (activeTab.value === "discover" && discoverSubView.value !== "home") ||
    (activeTab.value === "search" && searchViewMode.value === "results") ||
    activeTab.value === "cloud_drive" ||
    activeTab.value === "local_music" ||
    activeTab.value === "artist_detail" ||
    activeTab.value === "album_detail" ||
    activeTab.value === "playlist_detail"
  );

  let isArtistFollowedBridge: (artistId?: string, name?: string) => boolean = () => false;

  const {
    searchKeyword,
    searchViewMode,
    searchSuggestions,
    searchCategoryTab,
    hotSearchKeywords,
    searchDiscovery,
    isLoadingSearchDiscovery,
    hotArtistCategory,
    isChangingHotArtistCat,
    searchHistory,
    searchResultsSongs,
    searchResultsPlaylists,
    searchResultsAlbums,
    searchResultsArtists,
    searchTotals,
    searchPages,
    searchHasMore,
    isLoadingMoreSearch,
    isSearching,
    loadSearchDiscovery,
    switchHotArtistCategory,
    openSearchLanding,
    executeSearch,
    loadMoreSearchCategory,
    clearSearchHistory,
  } = createSearchModule({
    activeTab,
    isSearchPaletteOpen,
    isNowPlayingOpen,
    notify,
    scrollStageToTop,
    isArtistFollowed: (id, name) => isArtistFollowedBridge(id, name),
    pushNavHistory: () => pushNavHistoryBridge(),
  });

  const {
    localMusicSongs,
    localMusicDir,
    localMusicSavedDirs,
    localMusicSettings,
    isScanningLocalMusic,
    addLocalMusicSavedDir,
    removeLocalMusicSavedDir,
    updateLocalMusicSettings,
    scanLocalMusicLibrary,
    rebuildLocalMusicCache,
    pickAndScanLocalMusicDir,
    cloudDiskSongs,
    cloudDiskTotal,
    cloudDiskUsedBytes,
    cloudDiskMaxBytes,
    cloudDiskHasMore,
    isLoadingCloudDisk,
    loadUserCloudDisk,
    loadMoreCloudDisk,
    deleteCloudDiskSongs,
    userPlaylists,
    selectedPlaylist,
    playlistSongs,
    likedSongs,
    recentHistorySongs,
    followedArtists,
    purchasedAlbums,
    purchasedSongs,
    libraryAssetTab,
    libraryDetailMode,
    isLibraryGridView,
    isLoadingLibrary,
    activeArtistDetail,
    activeArtistSongs,
    activeArtistAlbums,
    artistDetailSubTab,
    artistSongsSort,
    isLoadingArtist,
    activeAlbumDetail,
    activeAlbumSongs,
    isLoadingAlbum,
    createdOrderIds,
    collectedOrderIds,
    artistsOrderIds,
    purchasedOrderIds,
    createdPlaylists,
    collectedPlaylists,
    likedPlaylistMeta,
    artistSongsPage,
    artistSongsHasMore,
    isLoadingMoreArtistSongs,
    artistAlbumsPage,
    artistAlbumsSort,
    artistAlbumsHasMore,
    isLoadingMoreArtistAlbums,
    isSameHistoryTrack,
    recordSongToRecentHistory,
    loadRecentHistory,
    loadUserPlaylists,
    pushNavHistory,
    clearNavHistory,
    navigateBack,
    openSoundMixerFromPlayer,
    selectPlaylist,
    openPlaylistDetail,
    openResolvedSharedPlaylist,
    openLikedSongsDetail,
    openRecentHistoryDetail,
    openPurchasedSongsDetail,
    openCloudDrivePage,
    openLocalMusicPage,
    refreshCurrentPlaylistOnly,
    updatePlaylistMeta,
    batchAddSongsToPlaylist,
    batchRemoveSongsFromPlaylist,
    createNewPlaylist,
    deletePlaylist,
    removeSongFromCurrentPlaylist,
    importSongsAsNewPlaylist,
    importExternalPlaylist,
    isPlaylistCollected,
    toggleCollectPlaylist,
    addSongToUserPlaylist,
    isSongLiked,
    toggleFavoriteSong,
    isArtistFollowed,
    loadFollowedArtists,
    toggleFollowArtist,
    loadPurchasedAssets,
    reorderLibraryAssetList,
    openArtistDetail,
    loadMoreArtistSongs,
    loadMoreArtistAlbums,
    switchArtistAlbumSort,
    switchArtistSongSort,
    openAlbumDetail,
    navigateToSongArtist,
    navigateToSongAlbum,
    clearSessionPlayedSongs,
    clearAccountSwrCaches,
  } = createLibraryAndDetailModule({
    activeTab,
    discoverSubView,
    isNowPlayingOpen,
    isSearchPaletteOpen,
    isLoginModalOpen,
    isRankGridModalOpen,
    isMultiArtistModalOpen,
    multiArtistCandidates,
    multiArtistSongTitle,
    addToPlaylistModalSong,
    navHistoryStack,
    isLoggedIn,
    session,
    profile,
    searchKeyword,
    searchViewMode,
    searchSuggestions,
    searchResultsArtists,
    searchDiscovery,
    notify,
    scrollStageToTop,
    closeSongContextMenu: () => closeSongContextMenu(),
  });

  watch([activeTab, discoverSubView], () => {
    if (isRankGridModalOpen.value) {
      isRankGridModalOpen.value = false;
    }
    if (contextMenu.visible) {
      closeSongContextMenu();
    }
  });

  isArtistFollowedBridge = isArtistFollowed;
  pushNavHistoryBridge = pushNavHistory;

  if (!isStandaloneSubWindow) {
    watch(
      () => likedSongs.value.length,
      () => {
        broadcastMiniPlayerState();
      }
    );
    watch(
      [
        masterEffectEnabled,
        activeViperEffectId,
        activeEqPreset,
        () => acousticField.enabled,
      ],
      () => {
        broadcastMiniPlayerState();
      }
    );
  }

  const {
    focusTodayCompletedCount,
    focusTodayTotalMinutes,
    focusAllTimeCompletedCount,
    focusAllTimeTotalMinutes,
    ambientSyncWithMusic,
    recordCompletedFocusSession,
    activePresetId: focusPresetId,
    activePreset: activeFocusPreset,
    focusMode,
    focusRunState,
    focusTargetMinutes,
    focusRemainingSec,
    focusElapsedSec,
    pomodoroFocusMinutes,
    pomodoroShortBreakMinutes,
    pomodoroLongBreakMinutes,
    pomodoroPhase,
    pomodoroCompletedRounds,
    autoPlaySceneMusic,
    autoMixAmbient,
    isFetchingSceneMusic,
    formattedClock: focusFormattedClock,
    progressRatio: focusProgressRatio,
    applyScenePreset: applyFocusScenePreset,
    switchFocusMode,
    setCustomCountdownMinutes,
    startFocusSession,
    pauseFocusSession,
    resumeFocusSession,
    toggleFocusRunPause,
    stopAndResetFocusSession,
    skipPomodoroPhase,
    setAutoPlaySceneMusic,
    setAutoMixAmbient,
    triggerSceneMusicPlayback,
  } = createFocusPodModule({
    fmChannels,
    isPlaying,
    notify,
    playSongFromList: (s, l, t) => playSongFromList(s, l, t),
  });

  watch(
    isPlaying,
    (playing) => {
      ambientSoundEngine.onMusicPlayStateChanged(playing);
    },
    { immediate: true }
  );

  watch(isLibraryGridView, (v) =>
    localStorage.setItem("bm_lib_grid", String(v))
  );
  // 真实播放秒数累加器与播放卡死看门狗（对齐 MakcRe/KuGouMusicApi user_listen_report.js & user_grade_info.js）
  let activeTelemetrySong: Song | null = null;
  let currentTrackPlayedAccumMs = 0;
  let unPersistedListenSec = 0;
  let unSyncedGradeSec = 0;
  let pendingProfileListenSec = 0;
  let telemetryLoopId: number | null = null;
  let lastWatchdogPosSec = -1;
  let watchdogStallTicks = 0;
  let lastWatchdogRecoverWallMs = 0;

  function flushProfileListenSec(force = false) {
    if (!profile.value || pendingProfileListenSec <= 0) return;
    const prevSec = profile.value.listenDurationSec || 0;
    const nextSec = prevSec + pendingProfileListenSec;
    const prevMin = Math.floor(prevSec / 60);
    const nextMin = Math.floor(nextSec / 60);
    if (!force && nextMin === prevMin && pendingProfileListenSec < 60) {
      return;
    }
    pendingProfileListenSec = 0;
    profile.value.listenDurationSec = nextSec;
    profile.value.listenDurationMin = nextMin;
    const h = Math.floor(nextMin / 60);
    const m = nextMin % 60;
    profile.value.listenDurationFormatted = h > 0 ? `${h} 小时 ${m} 分` : `${m} 分钟`;
  }

  function settleActiveTrackTelemetry() {
    flushProfileListenSec(true);
    const prevSong = activeTelemetrySong;
    const playedMs = currentTrackPlayedAccumMs;
    const remSec = unPersistedListenSec;
    activeTelemetrySong = null;
    currentTrackPlayedAccumMs = 0;
    unPersistedListenSec = 0;
    unSyncedGradeSec = 0;

    if (!isLoggedIn.value) return;

    const flushPromise =
      remSec > 0
        ? kugouClient.recordListeningSeconds(remSec).catch(() => {})
        : Promise.resolve();

    if (prevSong && playedMs >= 10000) {
      flushPromise.finally(() => {
        kugouClient
          .reportPlaybackTelemetry(prevSong, false, playedMs)
          .then(() => {
            setTimeout(() => {
              kugouClient
                .syncListeningGrade(false)
                .then((res) => {
                  if (res?.profile) {
                    pendingProfileListenSec = 0;
                    profile.value = res.profile;
                  }
                })
                .catch(() => {});
            }, 2000);
          })
          .catch(() => {});
      });
    }
  }

  function runPlaybackTelemetryTick() {
    if (!isPlaying.value || !currentSong.value || audio.paused || audio.ended) {
      watchdogStallTicks = 0;
      return;
    }
    currentTrackPlayedAccumMs += 1000;
    unPersistedListenSec += 1;
    unSyncedGradeSec += 1;
    pendingProfileListenSec += 1;
    flushProfileListenSec(false);
    // 每满 10 秒持久化增量秒数至本地缓冲，杜绝重复叠加与关闭丢失
    if (unPersistedListenSec >= 10 && isLoggedIn.value) {
      const secToSave = unPersistedListenSec;
      unPersistedListenSec = 0;
      kugouClient.recordListeningSeconds(secToSave).catch(() => {});
    }
    // 每满 60 秒真实播放仅触发一次 /v2/get_grade_info 增量时长同步（绝不在曲中伪造 CSCC 完播事件）
    if (unSyncedGradeSec >= 60 && isLoggedIn.value) {
      unSyncedGradeSec = 0;
      kugouClient
        .syncListeningGrade(false)
        .then((res) => {
          if (res?.profile) {
            pendingProfileListenSec = 0;
            profile.value = res.profile;
          }
        })
        .catch(() => {});
    }

    // 播放中途停滞看门狗 (PlaybackStallWatchdog)：检测 isPlaying=true 但 currentTime 连续 4 秒停滞
    const curSec = audio.currentTime || 0;
    const realDurMs =
      audio.duration && isFinite(audio.duration) && audio.duration > 5
        ? audio.duration * 1000
        : durationMs.value;
    if (curSec > 1 && realDurMs > 10000 && curSec * 1000 < realDurMs - 2500) {
      if (Math.abs(curSec - lastWatchdogPosSec) < 0.04) {
        watchdogStallTicks += 1;
        if (
          watchdogStallTicks >= 4 &&
          Date.now() - lastWatchdogRecoverWallMs > 8000
        ) {
          watchdogStallTicks = 0;
          lastWatchdogRecoverWallMs = Date.now();
          const resumeMs = Math.floor(curSec * 1000);
          console.warn(
            `[PlaybackStallWatchdog] 检测到播放中途停滞于 ${curSec.toFixed(1)}s，正在无缝重连恢复...`
          );
          if (currentCandidateIdx + 1 < currentCandidateUrls.length) {
            currentCandidateIdx += 1;
            audio.src = currentCandidateUrls[currentCandidateIdx];
            audio.currentTime = curSec;
            audio.play().catch(() => {});
          } else {
            loadAndPlayCurrentSong(resumeMs);
          }
        }
      } else {
        lastWatchdogPosSec = curSec;
        watchdogStallTicks = 0;
      }
    }
  }

  function startPlaybackTelemetryLoop() {
    if (isStandaloneSubWindow || typeof window === "undefined") return;
    if (telemetryLoopId !== null) return;
    telemetryLoopId = window.setInterval(runPlaybackTelemetryTick, 1000);
  }

  function stopPlaybackTelemetryLoop() {
    if (telemetryLoopId !== null) {
      window.clearInterval(telemetryLoopId);
      telemetryLoopId = null;
    }
    watchdogStallTicks = 0;
    flushProfileListenSec(true);
  }

  if (!isStandaloneSubWindow) {
    watch(
      isPlaying,
      (playing) => {
        if (playing) {
          startPlaybackTelemetryLoop();
        } else {
          stopPlaybackTelemetryLoop();
        }
      },
      { immediate: true }
    );
  }

  // HTML5 Audio 事件绑定
  audio.addEventListener("timeupdate", () => {
    const posMs = Math.floor(audio.currentTime * 1000);
    // 后台托盘/最小化状态跳过高频写 Vue 响应式进度，彻底消除隐藏窗口期间的无效依赖通知；
    // 下方的副歌速刷、双轨交叉淡入和切歌仍直接基于 audio.currentTime 100% 精准运行。
    if (isAppForegroundActive()) {
      currentPositionMs.value = posMs;
    }

    // 副歌速刷模式 (Chorus Quick-Skim 精华连播)：到达副歌结束点自动循环或切下一首
    if (climaxModeEnabled.value && currentSong.value) {
      const { startMs: cStart, endMs: cEnd } = getSongChorusWindowMs(
        currentSong.value,
        currentPlaybackInfo.value
      );
      if (cEnd > cStart + 5000 && posMs >= cEnd) {
        if (playQueue.value.length <= 1 || playMode.value === "repeat_one") {
          seekToMs(cStart);
        } else {
          playNext(true);
        }
        return;
      }
    }

    // 双轨重叠跨曲交叉淡入淡出 (True Dual-Deck Overlapping Crossfade: Deck A 淡出同时 Deck B 同步淡入)
    if (
      crossfadeSeconds.value > 0 &&
      !climaxModeEnabled.value &&
      isPlaying.value &&
      audio.duration &&
      isFinite(audio.duration) &&
      audio.duration > 30 &&
      playQueue.value.length > 0
    ) {
      const realDurMs = Math.floor(audio.duration * 1000);
      const fadeWinMs = crossfadeSeconds.value * 1000;
      const prefetchWinMs = fadeWinMs + 3800;
      const remainMs = realDurMs - posMs;

      if (remainMs > 0 && remainMs <= prefetchWinMs) {
        prefetchCrossfadeDeckIfNeeded();
      }

      if (remainMs > 0 && remainMs <= fadeWinMs && !isCrossfadeTransitioning) {
        const progress = clampVal(1 - remainMs / fadeWinMs, 0, 1);
        const gainOut = Math.cos(progress * Math.PI * 0.5);
        const gainIn = Math.sin(progress * Math.PI * 0.5);

        const targetLinear = sliderToPerceptualGain(volume.value);
        if (crossfadePrefetchPb && crossfadeAudio.src && !isCrossfadeOverlapActive) {
          isCrossfadeOverlapActive = true;
          crossfadeAudio.currentTime = 0;
          crossfadeAudio.volume = getAudioContext()
            ? 1.0
            : Math.max(0, Math.min(1, targetLinear * gainIn));
          applySpeedAndPitch();
          crossfadeAudio.play().catch(() => {});
        }

        if (isCrossfadeOverlapActive && !crossfadeAudio.paused) {
          if (getAudioContext()) {
            if (audio.volume !== 1.0) audio.volume = 1.0;
            if (crossfadeAudio.volume !== 1.0) crossfadeAudio.volume = 1.0;
            setCrossfadeDeckGains(gainOut, gainIn);
          } else {
            audio.volume = Math.max(0, Math.min(1, targetLinear * gainOut));
            crossfadeAudio.volume = Math.max(0, Math.min(1, targetLinear * gainIn));
          }
        } else if (getAudioContext()) {
          if (audio.volume !== 1.0) audio.volume = 1.0;
          setCrossfadeDeckGains(Math.max(0.02, gainOut), 0);
        } else {
          audio.volume = Math.max(targetLinear * 0.02, Math.min(1, targetLinear * gainOut));
        }

        if (remainMs <= 180) {
          isCrossfadeTransitioning = true;
          if (isCrossfadeOverlapActive && crossfadePrefetchPb && crossfadePrefetchSong) {
            completeCrossfadeDeckHandoff();
          } else {
            playNext(true);
          }
        }
      }
    }
  });
  audio.addEventListener("loadedmetadata", () => {
    if (audio.duration && isFinite(audio.duration)) {
      durationMs.value = Math.floor(audio.duration * 1000);
    }
    applySpeedAndPitch();
    broadcastDesktopLyricState();
    broadcastMiniPlayerState();
  });
  audio.addEventListener("play", () => {
    cancelAudioContextSuspend();
    isPlaying.value = true;
    isBuffering.value = false;
    if (audio.crossOrigin) {
      ensureWebAudioGraph();
      syncDspChain(false);
    }
    touchActiveQueueLastPlayed(currentSong.value);
    broadcastDesktopLyricState();
    broadcastDesktopLyricPosition();
    broadcastMiniPlayerState();
    broadcastMiniPlayerPosition();
  });
  audio.addEventListener("pause", () => {
    isPlaying.value = false;
    broadcastDesktopLyricState();
    broadcastDesktopLyricPosition();
    broadcastMiniPlayerState();
    broadcastMiniPlayerPosition();
    flushPlaybackPersistenceNow();
    scheduleAudioContextSuspend(800);
  });
  audio.addEventListener("waiting", () => {
    isBuffering.value = true;
  });
  audio.addEventListener("canplay", () => {
    isBuffering.value = false;
  });
  let hasRetriedFallbackQuality = false;
  audio.addEventListener("error", () => {
    const reqId = activePlayRequestId;
    if (!currentSong.value || currentCandidateUrls.length === 0) {
      isBuffering.value = false;
      return;
    }
    const resumePosSec = audio.currentTime > 0 ? audio.currentTime : 0;
    if (currentCandidateIdx + 1 < currentCandidateUrls.length) {
      currentCandidateIdx += 1;
      const nextUrl = currentCandidateUrls[currentCandidateIdx];
      console.warn(
        `[Audio Failover] 主节点加载失败，自动切换备用节点 (${currentCandidateIdx + 1}/${currentCandidateUrls.length}):`,
        nextUrl
      );
      audio.src = nextUrl;
      if (resumePosSec > 0) {
        audio.currentTime = resumePosSec;
      }
      audio.play().catch((err) => {
        if (reqId === activePlayRequestId && err?.name !== "AbortError") {
          console.warn("[Audio Failover] 备用节点 play() 异常:", err);
        }
      });
      return;
    }
    // 仅在浏览器预览模式（无本地 Rust 代理 proxyUrl）且所有节点因 CORS 失败时才降级移除 crossOrigin；
    // 在 Tauri 桌面端本地代理下始终保持 crossOrigin="anonymous"，确保 WebAudio 64-Bit DSP 音效永久在线
    const hasLocalProxyCandidate = currentCandidateUrls.some((u) =>
      u.startsWith("http://127.0.0.1:")
    );
    if (!hasRetriedWithoutCors && !hasLocalProxyCandidate) {
      hasRetriedWithoutCors = true;
      currentCandidateIdx = 0;
      audio.removeAttribute("crossorigin");
      audio.src = currentCandidateUrls[0];
      if (resumePosSec > 0) {
        audio.currentTime = resumePosSec;
      }
      audio.play().catch(() => {});
      return;
    }
    // 若当前高码率音源流损坏，自动平滑降级一档音质并在原进度续播
    if (!hasRetriedFallbackQuality && currentSong.value) {
      hasRetriedFallbackQuality = true;
      const fallbackOrder: AudioQualityValue[] = ["high", "flac", "320", "128"];
      const effectiveCurQ = normalizeConceptQuality(
        currentPlaybackInfo.value?.actualQuality || preferredQuality.value
      );
      const curIdx = fallbackOrder.indexOf(effectiveCurQ);
      const fallbackQ = fallbackOrder[Math.min(fallbackOrder.length - 1, Math.max(0, curIdx) + 1)] || "128";
      const songSnapshot = currentSong.value;
      const resumeMs = Math.floor(resumePosSec * 1000);
      kugouClient
        .resolveSongUrl(songSnapshot, fallbackQ)
        .then((pb) => {
          if (reqId !== activePlayRequestId) return;
          currentPlaybackInfo.value = pb;
          refreshCurrentSongDetailInfo(songSnapshot, pb, reqId);
          const nextCandidates = pb.proxyUrl
            ? [pb.proxyUrl]
            : [pb.url, ...(pb.backupUrls || [])].filter(
                (u, idx, arr): u is string => Boolean(u) && arr.indexOf(u) === idx
              );
          if (nextCandidates.length > 0) {
            currentCandidateUrls = nextCandidates;
            currentCandidateIdx = 0;
            if (!audio.crossOrigin) {
              audio.crossOrigin = "anonymous";
            }
            audio.src = nextCandidates[0];
            if (resumeMs > 0) {
              audio.currentTime = resumeMs / 1000;
            }
            audio.play().catch(() => {});
          }
        })
        .catch(() => {
          isBuffering.value = false;
          notify("当前音源直链节点连接失败，请尝试切换其他曲目");
        });
      return;
    }
    isBuffering.value = false;
    notify("当前音源直链节点连接失败，正在尝试重新加载或切换音质");
  });
  let crossfadePrefetchIdx = -1;
  let crossfadePrefetchSong: Song | null = null;
  let crossfadePrefetchPb: PlaybackResult | null = null;
  let crossfadePrefetchCandidates: string[] = [];
  let isCrossfadePrefetching = false;
  let isCrossfadeOverlapActive = false;

  function resetCrossfadeDeck() {
    isCrossfadePrefetching = false;
    isCrossfadeOverlapActive = false;
    crossfadePrefetchIdx = -1;
    crossfadePrefetchSong = null;
    crossfadePrefetchPb = null;
    crossfadePrefetchCandidates = [];
    if (!crossfadeAudio.paused) {
      crossfadeAudio.pause();
    }
    crossfadeAudio.removeAttribute("src");
    setCrossfadeDeckGains(1.0, 0.0);
  }

  function peekNextQueueIndex(): number {
    const len = playQueue.value.length;
    if (len === 0) return -1;
    if (playMode.value === "repeat_one") return currentIndex.value;
    if (playMode.value === "sequence" && currentIndex.value >= len - 1) {
      return -1;
    }
    if (playMode.value === "shuffle" && len > 1) {
      shuffleForwardDeck = shuffleForwardDeck.filter(
        (idx) => idx >= 0 && idx < len && idx !== currentIndex.value
      );
      if (shuffleForwardDeck.length === 0) {
        rebuildShuffleDeck(currentIndex.value);
      }
      return shuffleForwardDeck[0] ?? (currentIndex.value + 1) % len;
    }
    return (currentIndex.value + 1) % len;
  }

  function prefetchCrossfadeDeckIfNeeded() {
    if (
      isCrossfadePrefetching ||
      crossfadePrefetchPb !== null ||
      playQueue.value.length === 0
    ) {
      return;
    }
    const nextIdx = peekNextQueueIndex();
    if (nextIdx < 0 || nextIdx >= playQueue.value.length) return;
    const nextSong = playQueue.value[nextIdx];
    if (!nextSong) return;

    isCrossfadePrefetching = true;
    crossfadePrefetchIdx = nextIdx;
    crossfadePrefetchSong = nextSong;
    const reqSnapshot = activePlayRequestId;

    const targetPrefetchQuality = (activeCloudStreamEffect.value || preferredQuality.value) as AudioQualityValue;
    const resolvePromise = ktvAccompanimentMode.value
      ? kugouClient
          .resolveSongAccompaniment(nextSong)
          .catch(() => kugouClient.resolveSongUrl(nextSong, targetPrefetchQuality))
      : kugouClient.resolveSongUrl(nextSong, targetPrefetchQuality);

    resolvePromise
      .then((pb) => {
        if (reqSnapshot !== activePlayRequestId) return;
        const cands = [pb.proxyUrl, pb.url, ...(pb.backupUrls || [])].filter(
          (u, idx, arr): u is string => Boolean(u) && arr.indexOf(u) === idx
        );
        if (cands.length === 0) return;
        crossfadePrefetchPb = pb;
        crossfadePrefetchCandidates = cands;
        if (!crossfadeAudio.crossOrigin) {
          crossfadeAudio.crossOrigin = "anonymous";
        }
        crossfadeAudio.src = cands[0];
        crossfadeAudio.load();
      })
      .catch(() => {
        crossfadePrefetchIdx = -1;
        crossfadePrefetchSong = null;
        crossfadePrefetchPb = null;
      })
      .finally(() => {
        isCrossfadePrefetching = false;
      });
  }

  async function completeCrossfadeDeckHandoff() {
    if (!crossfadePrefetchSong || !crossfadePrefetchPb || crossfadePrefetchIdx < 0) {
      resetCrossfadeDeck();
      await playNext(true);
      return;
    }
    settleActiveTrackTelemetry();
    const nextIdx = crossfadePrefetchIdx;
    const nextSong = crossfadePrefetchSong;
    const nextPb = crossfadePrefetchPb;
    const nextCands = [...crossfadePrefetchCandidates];
    const handoffTimeSec = Math.max(0, crossfadeAudio.currentTime || 0);

    const reqId = ++activePlayRequestId;
    currentIndex.value = nextIdx;
    currentPlaybackInfo.value = nextPb;
    refreshCurrentSongDetailInfo(nextSong, nextPb, reqId);
    setTrackReplayGainDb(nextPb.replayGainDb);
    currentCandidateUrls = nextCands;
    currentCandidateIdx = 0;
    hasRetriedWithoutCors = false;
    hasRetriedFallbackQuality = false;
    durationMs.value = nextSong.durationMs || 0;
    currentPositionMs.value = Math.floor(handoffTimeSec * 1000);

    recordSongToRecentHistory(nextSong);
    touchActiveQueueLastPlayed(nextSong);
    syncWindowsMediaSession(nextSong);
    loadLyricForSong(nextSong);
    refreshCurrentSongCommentCount(nextSong);
    broadcastDesktopLyricState();

    activeTelemetrySong = nextSong;
    currentTrackPlayedAccumMs = Math.floor(handoffTimeSec * 1000);
    kugouClient.reportPlaybackTelemetry(nextSong, true, 0).catch(() => {});

    if (!audio.crossOrigin) {
      audio.crossOrigin = "anonymous";
    }
    audio.src = nextCands[0] || nextPb.url;
    if (handoffTimeSec > 0.05) {
      audio.currentTime = handoffTimeSec;
    }
    applyMasterOutputVolume(volume.value, 0);
    applySpeedAndPitch();
    resetCrossfadeDeck();
    isCrossfadeTransitioning = false;

    try {
      await audio.play();
      if (nextPb.isFreePart) {
        notify("当前为试听片段（登录并签到概念版 VIP 即可解锁整首无损）");
      }
    } catch {
      if (reqId === activePlayRequestId) {
        await loadAndPlayCurrentSong(Math.floor(handoffTimeSec * 1000));
      }
    }
  }

  audio.addEventListener("ended", () => {
    if (isCrossfadeOverlapActive && crossfadePrefetchPb && crossfadePrefetchSong) {
      cancelAudioContextSuspend();
      completeCrossfadeDeckHandoff();
      return;
    }
    settleActiveTrackTelemetry();
    scheduleAudioContextSuspend(800);

    if (
      sleepTimerMinutes.value === -1 ||
      (sleepRemainingSeconds.value <= 0 && sleepTimerMinutes.value > 0)
    ) {
      sleepTimerMinutes.value = 0;
      resetCrossfadeDeck();
      audio.pause();
      notify("睡眠定时已到点，已为您自然播完本曲并暂停");
      return;
    }
    playNext(true);
  });


  function applySpeedAndPitch() {
    const pitchFactor = Math.pow(2, pitchSemitones.value / 12);
    if (linkPitchWithSpeed.value) {
      const combinedRate = Number((playbackRate.value * pitchFactor).toFixed(3));
      audio.playbackRate = combinedRate;
      (audio as any).preservesPitch = false;
      crossfadeAudio.playbackRate = combinedRate;
      (crossfadeAudio as any).preservesPitch = false;
      syncIndependentPitchShifter(0, false);
    } else {
      audio.playbackRate = playbackRate.value;
      (audio as any).preservesPitch = true;
      crossfadeAudio.playbackRate = playbackRate.value;
      (crossfadeAudio as any).preservesPitch = true;
      if (Math.abs(pitchSemitones.value) >= 0.02) {
        ensureWebAudioGraph();
      }
      syncIndependentPitchShifter(pitchSemitones.value, true);
    }
  }

  watch(linkPitchWithSpeed, () => {
    applySpeedAndPitch();
  });

  function setPlaybackSpeed(rate: number) {
    playbackRate.value = Number(rate.toFixed(2));
    applySpeedAndPitch();
    broadcastDesktopLyricPosition();
    broadcastMiniPlayerPosition();
  }

  function setPitchSemitones(semitones: number) {
    pitchSemitones.value = Math.max(-12, Math.min(12, semitones));
    applySpeedAndPitch();
  }

  function startSleepTimer(mins: number) {
    if (sleepIntervalId) {
      clearInterval(sleepIntervalId);
      sleepIntervalId = null;
    }
    sleepTimerMinutes.value = mins;
    if (mins <= 0) {
      sleepRemainingSeconds.value = 0;
      if (mins === -1) {
        notify("将在当前曲目播放完毕后自动停止");
      } else {
        notify("已关闭睡眠定时");
      }
      return;
    }
    sleepRemainingSeconds.value = mins * 60;
    notify(`已开启睡眠定时：${mins} 分钟后停止播放`);
    sleepIntervalId = window.setInterval(() => {
      if (sleepRemainingSeconds.value > 0) {
        sleepRemainingSeconds.value -= 1;
        if (sleepRemainingSeconds.value <= 0) {
          if (sleepIntervalId) clearInterval(sleepIntervalId);
          sleepIntervalId = null;
          if (!sleepWaitCurrentSong.value) {
            audio.pause();
            sleepTimerMinutes.value = 0;
            notify("睡眠定时已结束，音乐已暂停");
          } else {
            notify("睡眠定时已到点，将在当前歌曲播完后停止");
          }
        }
      }
    }, 1000);
  }

  // ================= 9. 核心启动与全模块数据加载 =================
  async function bootstrap() {
    try {
      const [savedSession, accounts, vipers] = await Promise.all([
        kugouClient.initAppState().catch(() => null),
        kugouClient.getSavedAccounts().catch(() => []),
        kugouClient.getViperEffects().catch(() => []),
      ]);
      savedAccounts.value = accounts;
      if (Array.isArray(vipers) && vipers.length > 0) {
        viperEffects.value = vipers;
      }
      if (savedSession && savedSession.token) {
        session.value = savedSession;
      }
    } catch (e) {
      console.warn("initAppState skipped in browser preview:", e);
    }

    // 仅在非 Tauri 桌面容器的开发浏览器预览中注入演示曲目/歌词/分支乐评/歌单，便于直接打开网页调试 UI
    if (isBrowserDevPreviewEnv() && !isStandaloneSubWindow && playQueue.value.length === 0) {
      const demo = createBrowserDevShowcase();
      playQueue.value = demo.songs;
      currentIndex.value = 0;
      durationMs.value = demo.songs[0].durationMs;
      currentPositionMs.value = 11200;
      currentLyric.value = demo.lyric;
      activeSongComments.value = demo.comments;
      activeSongCommentTotal.value = 136;
      currentSongCommentTotal.value = 136;
      Object.assign(floorRepliesMap, demo.floorRepliesMap);
      userPlaylists.value = demo.playlists;
      recentHistorySongs.value = [...demo.songs];
      activeHighResCoverUrl.value = demo.songs[0].coverUrl;
      activeHighResCoverMetrics.value = { width: 1080, height: 1080 };
    }

    // 同步系统托盘关闭策略、全屏灵动岛礼让策略、开机自启动、自定义全局热键、缓存配额与独立桌面歌词窗口状态
    kugouClient.setCloseToTray(closeToTray.value).catch(() => {});
    kugouClient
      .setMiniPlayerYieldFullscreen(miniYieldFullscreenEnabled.value)
      .catch(() => {});
    kugouClient.setGlobalHotkeysEnabled(osGlobalHotkeysEnabled.value).catch(() => {});
    syncGlobalHotkeysToOs(false);
    refreshAutostartState();
    kugouClient.setAudioCacheQuota(cacheStats.maxQuotaGb).catch(() => {});
    refreshCacheStats();
    listen<{ enabled?: boolean; failedActions?: string[] }>(
      "bm-global-hotkeys-report",
      (event) => {
        const p = event.payload;
        if (p && Array.isArray(p.failedActions)) {
          globalHotkeyFailedActions.value = p.failedActions;
        }
      }
    ).catch(() => {});
    listen<DownloadProgressPayload>("bm-download-progress", (event) => {
      const p = event.payload;
      if (!p || !p.taskId) return;
      const item = downloadQueue.value.find((t) => t.taskId === p.taskId);
      if (!item || item.status === "canceled") return;
      const nextBytes = Number(p.downloadedBytes) || 0;
      const nextTotal = Number(p.totalBytes) || 0;
      const nowMs = Date.now();
      const tracker = downloadSpeedTrackers.get(p.taskId);
      if (p.stage === "downloading" && nextBytes > 0) {
        if (!tracker) {
          downloadSpeedTrackers.set(p.taskId, {
            lastBytes: nextBytes,
            lastTsMs: nowMs,
            emaSpeed: 0,
          });
        } else {
          const dtSec = (nowMs - tracker.lastTsMs) / 1000;
          if (dtSec >= 0.12) {
            const inst = Math.max(0, (nextBytes - tracker.lastBytes) / dtSec);
            const ema =
              tracker.emaSpeed > 0 ? tracker.emaSpeed * 0.65 + inst * 0.35 : inst;
            tracker.lastBytes = nextBytes;
            tracker.lastTsMs = nowMs;
            tracker.emaSpeed = ema;
            item.speedBytesPerSec = Math.round(ema);
            if (nextTotal > nextBytes && ema > 1024) {
              item.etaSeconds = Math.ceil((nextTotal - nextBytes) / ema);
            } else {
              item.etaSeconds = 0;
            }
          }
        }
      } else if (p.stage === "done" || p.stage === "error") {
        downloadSpeedTrackers.delete(p.taskId);
        item.speedBytesPerSec = 0;
        item.etaSeconds = 0;
      }
      item.status = p.stage;
      item.downloadedBytes = nextBytes;
      item.totalBytes = nextTotal;
      item.percent = Math.max(0, Math.min(100, Number(p.percent) || 0));
      if (p.message) {
        item.message = p.message;
      }
    }).catch(() => {});
    hydrateAndApplyTypography(typographyConfig).catch(() => {});

    // 启动保护：若开启开机安全音量上限且当前音量超限，温和回落防炸耳
    if (
      !isStandaloneSubWindow &&
      startupSafeVolumeEnabled.value &&
      volume.value > startupSafeVolumeLimit.value
    ) {
      setVolume(startupSafeVolumeLimit.value);
    }

    // 启动默认首页恢复 + 本地音乐静默扫描
    if (!isStandaloneSubWindow) {
      const prefHome = startupHomePage.value;
      if (prefHome === "library") {
        activeTab.value = "library";
      } else if (prefHome === "local_music") {
        activeTab.value = "local_music";
        void scanLocalMusicLibrary(undefined, true);
      } else if (prefHome === "last") {
        const lastSaved = localStorage.getItem("bm_last_home_tab_v1");
        if (lastSaved === "library" || lastSaved === "local_music") {
          activeTab.value = lastSaved;
          if (lastSaved === "local_music") {
            void scanLocalMusicLibrary(undefined, true);
          }
        }
      }
    }
    if (desktopLyricEnabled.value) {
      invoke("set_desktop_lyric_window_state_cmd", {
        visible: true,
        locked: desktopLyricLocked.value,
        width: desktopLyricWidth.value,
        height: null,
        x: desktopLyricX.value,
        y: desktopLyricY.value,
        resetPosition: desktopLyricX.value === null && desktopLyricY.value === null ? true : null,
        applyVisibility: true,
      })
        .then(() => {
          broadcastDesktopLyricState();
          setTimeout(() => broadcastDesktopLyricState(), 140);
          setTimeout(() => broadcastDesktopLyricState(), 460);
        })
        .catch(() => {});
    } else {
      broadcastDesktopLyricState();
    }
    listen<string>("tray-action", (event) => {
      const cmd = event.payload;
      if (cmd === "toggle_play" || cmd === "toggle") {
        togglePlayPause();
      } else if (cmd === "prev_song" || cmd === "prev") {
        playPrev();
      } else if (cmd === "next_song" || cmd === "next") {
        playNext(false);
      } else if (cmd === "toggle_desktop_lyric" || cmd === "desktop_lyric") {
        desktopLyricEnabled.value = !desktopLyricEnabled.value;
        notify(
          desktopLyricEnabled.value
            ? "已开启系统级桌面悬浮歌词"
            : "已关闭桌面悬浮歌词"
        );
      } else if (cmd === "toggle_lyric_lock") {
        desktopLyricLocked.value = !desktopLyricLocked.value;
        notify(
          desktopLyricLocked.value
            ? "已锁定桌面歌词（鼠标穿透生效，可通过托盘或 Ctrl+Shift+L 解锁）"
            : "已解锁桌面歌词，现在可自由拖动与调节"
        );
      } else if (cmd === "toggle_mini_mode") {
        toggleMiniPlayerMode();
      } else if (cmd === "volume_up") {
        setVolume(Math.min(1, Number((volume.value + 0.05).toFixed(2))));
      } else if (cmd === "volume_down") {
        setVolume(Math.max(0, Number((volume.value - 0.05).toFixed(2))));
      } else if (cmd === "toggle_like") {
        if (currentSong.value) {
          toggleFavoriteSong(currentSong.value);
        }
      }
    }).catch(() => {});

    refreshAudioOutputDevices().then(() => {
      if (selectedAudioDeviceId.value && selectedAudioDeviceId.value !== "default") {
        selectAudioOutputDevice(selectedAudioDeviceId.value, true);
      }
    });
    if (
      typeof navigator !== "undefined" &&
      navigator.mediaDevices &&
      typeof navigator.mediaDevices.addEventListener === "function"
    ) {
      navigator.mediaDevices.addEventListener("devicechange", () => {
        refreshAudioOutputDevices();
      });
    }

    // 启动时静默记忆并恢复上次播放队列与进度；仅当用户显式开启「启动时自动继续播放」时才出声续播
    try {
      const savedQueueRaw = localStorage.getItem("bm_saved_queue");
      const savedIdx = Number(localStorage.getItem("bm_saved_index") ?? -1);
      const savedPosMs = Number(localStorage.getItem("bm_saved_pos_ms") ?? 0);
      if (savedQueueRaw) {
        const parsedQueueRaw = JSON.parse(savedQueueRaw) as Song[];
        if (Array.isArray(parsedQueueRaw) && parsedQueueRaw.length > 0) {
          const parsedQueue = parsedQueueRaw.map(normSong);
          playQueue.value = parsedQueue;
          originalQueueSnapshot = [...parsedQueue];
          currentIndex.value = clampVal(savedIdx >= 0 ? savedIdx : 0, 0, parsedQueue.length - 1);
          if (recentPlayQueues.value.length === 0) {
            saveQueueSnapshot(
              parsedQueue,
              currentQueueTitle.value || "上次播放队列",
              currentQueueTitle.value || "上次播放队列",
              parsedQueue[currentIndex.value]?.hash
            );
          }
          const restoredSong = playQueue.value[currentIndex.value];
          if (restoredSong) {
            durationMs.value = restoredSong.durationMs || 210000;
            currentPositionMs.value = Math.max(0, savedPosMs);
            loadLyricForSong(restoredSong);
            if (autoPlayOnStartup.value) {
              setTimeout(() => {
                loadAndPlayCurrentSong(Math.max(0, savedPosMs));
              }, 600);
            } else {
              refreshCurrentSongCommentCount(restoredSong);
              refreshCurrentSongDetailInfo(restoredSong);
            }
          }
        }
      }
    } catch {
      // ignore corrupted queue
    }

    // 启动时执行三波错峰全量刷新并开启每小时静默自动刷新周期（含休眠唤醒补偿与分钟时钟）
    startAutoRefreshScheduler();
    runStartupStaggeredRefresh();
    kugouClient
      .getSearchHot()
      .then((kws) => {
        if (kws.length > 0 && searchDiscovery.value.hotBoards.length === 0) {
          hotSearchKeywords.value = kws;
        }
      })
      .catch(() => {});
  }

  async function refreshAccountProfileAndVipOnly(isStartup = false) {
    if (!isLoggedIn.value) return;
    try {
      const [prof, vip, accounts] = await Promise.all([
        kugouClient.getAccountProfile().catch(() => null),
        kugouClient.getVipStatus().catch(() => null),
        kugouClient.getSavedAccounts().catch(() => []),
      ]);
      if (accounts.length > 0) savedAccounts.value = accounts;
      if (prof) profile.value = prof;
      if (vip) {
        vipStatus.value = vip;
        if (isStartup && autoClaimVipOnStartup.value && !vip.todayClaimed) {
          claimVipDaily(true);
        }
      }
    } catch (e) {
      console.warn("refreshAccountProfileAndVipOnly error:", e);
    }
  }

  async function refreshAccountAndVip(isStartup = false) {
    if (!isLoggedIn.value) return;
    await refreshAccountProfileAndVipOnly(isStartup);
    loadUserPlaylists();
    loadRecentHistory();
    loadFollowedArtists();
    loadPurchasedAssets();
    loadUserCloudDisk(true, true);
  }

  const {
    nowClockTick,
    lastFullRefreshAt,
    isBackgroundRefreshing,
    startScheduler: startAutoRefreshScheduler,
    runStartupStaggeredRefresh,
    runHourlySilentRefresh,
    resetCycleForAuthChange,
  } = createAutoRefreshScheduler({
    isLoggedIn,
    isStandaloneSubWindow,
    refreshAccountProfileAndVipSilent: refreshAccountProfileAndVipOnly,
    loadDailyRecommend,
    loadPersonalFm,
    loadNewSongExpress,
    refreshAllNewSongTags,
    loadRankList,
    loadFmChannels,
    loadUserPlaylists,
    loadRecentHistory,
    loadFollowedArtists,
    loadPurchasedAssets,
    loadUserCloudDisk,
    loadSearchDiscovery,
    scanLocalMusicLibrary,
    shouldAutoScanLocalMusic: () => Boolean(localMusicSettings.value.autoScanOnStartup),
    activeNewSongPeriod: newSongPeriod,
  });

  async function syncListeningGradeCloud(forceReport = true) {
    if (!isLoggedIn.value) {
      isLoginModalOpen.value = true;
      return;
    }
    if (isSyncingListeningGrade.value) return;
    isSyncingListeningGrade.value = true;
    flushProfileListenSec(true);
    try {
      if (unPersistedListenSec > 0) {
        const remSec = unPersistedListenSec;
        unPersistedListenSec = 0;
        unSyncedGradeSec = 0;
        await kugouClient.recordListeningSeconds(remSec).catch(() => {});
      }
      const [syncRes, vip] = await Promise.all([
        kugouClient.syncListeningGrade(forceReport),
        kugouClient.getVipStatus().catch(() => null),
      ]);
      if (syncRes?.profile) {
        pendingProfileListenSec = 0;
        profile.value = syncRes.profile;
      }
      if (vip) {
        vipStatus.value = vip;
      }
      notify(syncRes?.message || "已校准云端听歌等级与累计时长");
    } catch (e: any) {
      notify(`同步听歌等级失败：${String(e)}`);
    } finally {
      isSyncingListeningGrade.value = false;
    }
  }

  async function switchAccount(userId: string) {
    try {
      const switched = await kugouClient.switchSavedAccount(userId);
      if (switched) {
        clearSessionPlayedSongs();
        clearAccountSwrCaches();
        clearDiscoverCaches();
        resetCycleForAuthChange();
        recentHistorySongs.value = [];
        session.value = switched;
        isLoginModalOpen.value = false;
        notify(`已切换至账号「${switched.nickname}」`);
        await refreshAccountAndVip(false);
        await loadDailyRecommend();
        loadSearchDiscovery(true);
      }
    } catch (e: any) {
      notify(`切换账号失败：${String(e)}`);
    }
  }

  async function removeSavedAccount(userId: string) {
    try {
      savedAccounts.value = await kugouClient.removeSavedAccount(userId);
      if (session.value?.userId === userId) {
        session.value = savedAccounts.value[0] || null;
        if (session.value) {
          await refreshAccountAndVip(false);
        } else {
          await logout();
        }
      }
    } catch (e: any) {
      notify(`移除账号失败：${String(e)}`);
    }
  }

  async function claimVipDaily(silent = false) {
    if (!isLoggedIn.value) {
      isLoginModalOpen.value = true;
      return;
    }
    isClaimingVip.value = true;
    try {
      const updated = await kugouClient.claimDailyVip();
      vipStatus.value = updated;
      const prof = await kugouClient.getAccountProfile().catch(() => null);
      if (prof) profile.value = prof;
      // 签到解锁 VIP 后，若当前正在试听或实际音质低于偏好档位，自动在原进度无缝重载升级为完整/无损流
      const activePb = currentPlaybackInfo.value;
      const qualityRank = (q?: string) => {
        const n = normalizeConceptQuality(q);
        return n === "high" ? 4 : n === "flac" ? 3 : n === "320" ? 2 : 1;
      };
      const needsUpgrade =
        isPlaying.value &&
        Boolean(currentSong.value) &&
        Boolean(activePb) &&
        (Boolean(activePb?.isFreePart) ||
          qualityRank(activePb?.actualQuality) < qualityRank(preferredQuality.value));
      if (needsUpgrade) {
        const resumeMs = Math.max(0, Math.floor(audio.currentTime * 1000) || currentPositionMs.value || 0);
        loadAndPlayCurrentSong(resumeMs).catch(() => {});
      }
      if (!silent) {
        notify(
          needsUpgrade
            ? "今日概念版 VIP 权益已生效，正在为当前歌曲升级完整音源"
            : "概念版今日 VIP 签到与升级已完成！"
        );
      }
    } catch (e: any) {
      if (!silent) {
        notify(`VIP 签到提示：${String(e)}`);
      }
    } finally {
      isClaimingVip.value = false;
    }
  }

  async function resignDeviceAndRefresh() {
    isDiagnosingGateway.value = true;
    try {
      const res = await kugouClient.resignDeviceIdentity();
      gatewayDiag.value = res;
      if (vipStatus.value) {
        vipStatus.value.dfid = res.dfid;
        vipStatus.value.guid = res.guid;
      }
      notify(res.statusText);
    } catch (e: any) {
      notify(`重签设备标识失败：${String(e)}`);
    } finally {
      isDiagnosingGateway.value = false;
    }
  }

  async function runGatewayDiagnostic() {
    isDiagnosingGateway.value = true;
    try {
      const res = await kugouClient.diagnoseGatewayLatency();
      gatewayDiag.value = res;
      notify(res.statusText);
    } catch (e: any) {
      notify(`网关诊断异常：${String(e)}`);
    } finally {
      isDiagnosingGateway.value = false;
    }
  }

  async function openCommentsModal(song?: Song | null) {
    const target = song || currentSong.value;
    if (!target) {
      notify("请先选择或播放一首歌曲以查看评论");
      return;
    }
    const frozenTarget: Song = { ...target };
    const nextKey = getCommentSongKey(frozenTarget);
    const prevKey = getCommentSongKey(commentsModalSong.value);
    const isDifferentSong = nextKey !== prevKey || nextKey !== lastCommentsSongKey;
    commentsModalSong.value = frozenTarget;
    if (isDifferentSong) {
      activeSongComments.value = [];
      activeSongCommentTotal.value = 0;
      expandedFloorCommentId.value = null;
      for (const k of Object.keys(floorRepliesMap)) {
        delete floorRepliesMap[k];
      }
    }
    isCommentsModalOpen.value = true;
    await loadCommentsForSong(frozenTarget, commentSortType.value);
  }

  function closeCommentsModal() {
    isCommentsModalOpen.value = false;
  }

  // ================= 11. 播放调度、伴奏切换、评论楼中楼与本地下载 =================
  async function playSongFromList(
    song: Song,
    list?: Song[],
    queueTitle = "当前播放列表"
  ) {
    const ctx = getSharedAudioContext();
    if (ctx.state === "suspended") {
      await ctx.resume();
    }

    // 切歌前结算上一首曲目的真实播放时长与 CSCC 完播/切歌事件
    settleActiveTrackTelemetry();

    if (list && list.length > 0) {
      const isPersonalFmSource =
        personalFmSongs.value.length > 0 &&
        (list === personalFmSongs.value ||
          areSongListsEquivalent(list, personalFmSongs.value));
      const effectiveQueueTitle =
        (!queueTitle || queueTitle === "当前播放列表") && isPersonalFmSource
          ? "猜你喜欢 · 私人漫游"
          : queueTitle;

      const isSameActiveQueue =
        list === playQueue.value ||
        (areSongListsEquivalent(list, playQueue.value) &&
          (effectiveQueueTitle === "当前播放列表" ||
            effectiveQueueTitle === currentQueueTitle.value));

      if (!isSameActiveQueue) {
        if (playQueue.value.length > 0) {
          saveQueueSnapshot(
            playQueue.value,
            currentQueueTitle.value || "历史播放队列",
            currentQueueTitle.value || "历史播放队列",
            currentSong.value?.hash
          );
        }
        const resolvedTitle =
          effectiveQueueTitle && effectiveQueueTitle !== "当前播放列表"
            ? effectiveQueueTitle
            : song.album
            ? `专辑《${song.album}》`
            : `${song.title} · 播放队列`;
        currentQueueTitle.value = resolvedTitle;
        playQueue.value = [...list];
        originalQueueSnapshot = [...list];
        saveQueueSnapshot(list, resolvedTitle, resolvedTitle, song.hash);
      } else if (
        isPersonalFmSource &&
        !String(currentQueueTitle.value || "").includes("猜你喜欢")
      ) {
        currentQueueTitle.value = "猜你喜欢 · 私人漫游";
      }

      const idx = playQueue.value.findIndex(
        (s) =>
          (s.hash && s.hash === song.hash) || (s.id && s.id === song.id)
      );
      currentIndex.value = idx >= 0 ? idx : 0;
      shuffleHistoryStack = [];
      rebuildShuffleDeck(currentIndex.value);
    } else {
      const existingIdx = playQueue.value.findIndex(
        (s) =>
          (s.hash && s.hash === song.hash) || (s.id && s.id === song.id)
      );
      if (existingIdx >= 0) {
        currentIndex.value = existingIdx;
      } else {
        const wasEmpty = playQueue.value.length === 0;
        playQueue.value.push(song);
        currentIndex.value = playQueue.value.length - 1;
        if (wasEmpty) {
          const singleTitle = song.album ? `专辑《${song.album}》` : `${song.title} · 单曲队列`;
          currentQueueTitle.value = singleTitle;
          originalQueueSnapshot = [...playQueue.value];
          saveQueueSnapshot(playQueue.value, singleTitle, singleTitle, song.hash);
        }
      }
    }

    recordSongToRecentHistory(song);
    touchActiveQueueLastPlayed(song);

    await loadAndPlayCurrentSong(0);
  }

  async function loadAndPlayCurrentSong(seekMs = 0) {
    const song = currentSong.value;
    if (!song) return;

    cancelAudioContextSuspend();
    flushPlaybackPersistenceNow();
    const reqId = ++activePlayRequestId;
    resetCrossfadeDeck();
    if (
      resetPlaybackRateOnTrackChange.value &&
      Math.abs(playbackRate.value - 1) > 0.001
    ) {
      setPlaybackSpeed(1.0);
    }
    recordSongToRecentHistory(song);
    touchActiveQueueLastPlayed(song);
    isBuffering.value = true;
    currentPlaybackInfo.value = null;
    currentSongDetailInfo.value = null;
    currentTrackPlayedAccumMs = 0;
    durationMs.value = song.durationMs || 0;
    syncWindowsMediaSession(song);
    broadcastDesktopLyricState();

    loadLyricForSong(song);
    refreshCurrentSongCommentCount(song);

    try {
      const targetStreamQuality = (activeCloudStreamEffect.value ||
        preferredQuality.value) as AudioQualityValue;
      const pb = ktvAccompanimentMode.value
        ? await kugouClient
            .resolveSongAccompaniment(song)
            .catch(() => kugouClient.resolveSongUrl(song, targetStreamQuality))
        : await kugouClient.resolveSongUrl(song, targetStreamQuality);

      // 若在异步解析直链期间用户已切到其他歌曲，直接丢弃过期结果，杜绝切歌竞态覆盖
      if (reqId !== activePlayRequestId) return;

      currentPlaybackInfo.value = pb;
      setTrackReplayGainDb(pb.replayGainDb);
      recordSongToRecentHistory(song);
      // 播放启动后异步富化完整音频元数据与可用音质列表
      refreshCurrentSongDetailInfo(song, pb, reqId);
      // 优先使用 Rust 本地 HTTP Range 边听边存流媒体代理 (proxyUrl)
      // proxyUrl 内部已托管主/备 CDN 节点自动容灾与 CORS 响应头，避免前端回退至无 CORS 裸连导致 WebAudio DSP 断链
      const candidates = pb.proxyUrl
        ? [pb.proxyUrl]
        : [pb.url, ...(pb.backupUrls || [])].filter(
            (u, idx, arr): u is string => Boolean(u) && arr.indexOf(u) === idx
          );
      currentCandidateUrls = candidates;
      currentCandidateIdx = 0;
      hasRetriedWithoutCors = false;
      hasRetriedFallbackQuality = false;
      lastWatchdogPosSec = -1;
      watchdogStallTicks = 0;
      if (!audio.crossOrigin) {
        audio.crossOrigin = "anonymous";
      }
      audio.src = candidates[0] || pb.url;
      currentSongPlayStartWallMs = Date.now();

      // 触发 CSCC start 与听歌足迹上报
      activeTelemetrySong = song;
      currentTrackPlayedAccumMs = 0;
      kugouClient.reportPlaybackTelemetry(song, true, 0).catch(() => {});

      const chorusWindow = climaxModeEnabled.value
        ? getSongChorusWindowMs(song, pb)
        : null;
      const startAtMs =
        seekMs > 0
          ? seekMs
          : chorusWindow && chorusWindow.startMs > 0
          ? chorusWindow.startMs
          : 0;

      if (startAtMs > 0) {
        audio.currentTime = startAtMs / 1000;
      }
      cancelPendingPauseFade(true);
      const shouldFadeIn = isCrossfadeTransitioning && crossfadeSeconds.value > 0;
      isCrossfadeTransitioning = false;
      const targetLinear = sliderToPerceptualGain(volume.value);

      if (audio.crossOrigin) {
        ensureWebAudioGraph();
        syncDspChain(false);
      }
      setCrossfadeDeckGains(1.0, 0.0);
      const outGainNode = getMasterOutputGain();
      const ctxForStart = getAudioContext();
      if (ctxForStart && outGainNode) {
        audio.volume = 1.0;
        const startGain = shouldFadeIn ? targetLinear * 0.05 : targetLinear;
        outGainNode.gain.cancelScheduledValues(ctxForStart.currentTime);
        outGainNode.gain.setValueAtTime(startGain, ctxForStart.currentTime);
      } else {
        audio.volume = shouldFadeIn ? targetLinear * 0.05 : targetLinear;
      }

      try {
        await audio.play();
      } catch (playErr: any) {
        if (reqId !== activePlayRequestId) return;
        if (playErr?.name === "AbortError") {
          return;
        }
        // 主直链 play() 失败时立即尝试备用 CDN 节点
        let recovered = false;
        while (currentCandidateIdx + 1 < currentCandidateUrls.length) {
          if (reqId !== activePlayRequestId) return;
          currentCandidateIdx += 1;
          audio.src = currentCandidateUrls[currentCandidateIdx];
          if (startAtMs > 0) {
            audio.currentTime = startAtMs / 1000;
          }
          try {
            await audio.play();
            recovered = true;
            break;
          } catch (retryErr: any) {
            if (retryErr?.name === "AbortError") {
              return;
            }
          }
        }
        if (!recovered) {
          applyMasterOutputVolume(volume.value, 0);
          throw playErr;
        }
      }

      if (shouldFadeIn && targetLinear > 0.0001) {
        const fadeSec = Math.min(2.6, crossfadeSeconds.value * 0.45);
        const ctxAfterPlay = getAudioContext();
        const outAfterPlay = getMasterOutputGain();
        if (ctxAfterPlay && outAfterPlay) {
          const now = ctxAfterPlay.currentTime;
          outAfterPlay.gain.cancelScheduledValues(now);
          outAfterPlay.gain.setValueAtTime(targetLinear * 0.05, now);
          outAfterPlay.gain.linearRampToValueAtTime(targetLinear, now + fadeSec);
        } else {
          rampHtmlAudioVolume(targetLinear * 0.05, targetLinear, fadeSec * 1000);
        }
      }
      if (pb.isFreePart) {
        notify("当前为试听片段（登录并签到概念版 VIP 即可解锁整首无损）");
      }
      refreshCacheStats();
    } catch (e: any) {
      if (reqId !== activePlayRequestId || e?.name === "AbortError") return;
      isBuffering.value = false;
      notify(String(e));
    }
  }

  async function toggleKtvAccompaniment() {
    ktvAccompanimentMode.value = !ktvAccompanimentMode.value;
    if (currentSong.value) {
      const pos = currentPositionMs.value;
      await loadAndPlayCurrentSong(pos);
      notify(
        ktvAccompanimentMode.value
          ? "已切换至 KTV 伴奏音轨模式"
          : "已恢复原唱人声模式"
      );
    }
  }

  let activeSongDetailSeq = 0;

  async function openSongDetailModal(song: Song) {
    const reqSeq = ++activeSongDetailSeq;
    const frozenSong: Song = { ...song };
    songDetailModalSong.value = frozenSong;
    const isCurrentAtOpen =
      Boolean(currentSong.value) && isSameHistoryTrack(currentSong.value!, frozenSong);
    if (isCurrentAtOpen && currentSongDetailInfo.value) {
      activeSongDetailInfo.value = currentSongDetailInfo.value;
      isLoadingSongDetail.value = false;
      return;
    }
    activeSongDetailInfo.value = null;
    isLoadingSongDetail.value = true;
    try {
      const targetQuality =
        isCurrentAtOpen && currentPlaybackInfo.value?.actualQuality
          ? currentPlaybackInfo.value.actualQuality
          : preferredQuality.value;
      const detail = await kugouClient.getSongDetailInfo(
        frozenSong,
        targetQuality,
        isCurrentAtOpen ? currentPlaybackInfo.value : null
      );
      if (
        reqSeq !== activeSongDetailSeq ||
        !songDetailModalSong.value ||
        !isSameHistoryTrack(songDetailModalSong.value, frozenSong)
      ) {
        return;
      }
      activeSongDetailInfo.value = detail;
      if (detail?.song) {
        syncEnrichedSongMetadata(song, detail.song);
        syncEnrichedSongMetadata(frozenSong, detail.song);
        if (songDetailModalSong.value) {
          syncEnrichedSongMetadata(songDetailModalSong.value, detail.song);
        }
      }
      if (
        currentSong.value &&
        isSameHistoryTrack(currentSong.value, frozenSong)
      ) {
        if (detail?.song) {
          syncEnrichedSongMetadata(currentSong.value, detail.song);
        }
        currentSongDetailInfo.value = detail;
      }
    } catch {
      if (reqSeq === activeSongDetailSeq) {
        activeSongDetailInfo.value = null;
      }
    } finally {
      if (reqSeq === activeSongDetailSeq) {
        isLoadingSongDetail.value = false;
      }
    }
  }

  const lyricCandidates = ref<LyricCandidateItem[]>([]);
  const isSearchingLyricCandidates = ref<boolean>(false);
  const activeLyricCandidateId = ref<string>("");

  let lyricLoadRequestSeq = 0;
  let lyricCandidateSearchSeq = 0;

  async function loadLyricForSong(song: Song) {
    const reqSeq = ++lyricLoadRequestSeq;
    lyricCandidateSearchSeq += 1;
    const targetKey = song.hash || song.id || "";
    isLoadingLyric.value = true;
    isSearchingLyricCandidates.value = false;
    lyricCandidates.value = [];
    activeLyricCandidateId.value = "";
    // 切歌瞬间先清空旧歌残留歌词并立即同步桌面歌词窗口，防止新歌名配旧歌词错位闪烁
    currentLyric.value = {
      lines: [],
      rawText: "",
      isKrc: false,
      hasTranslation: false,
      hasPhonetic: false,
    };
    desktopLyricLyricSeq += 1;
    broadcastDesktopLyricState();
    try {
      const loaded = await kugouClient.getSongLyric(song);
      const currentKey = currentSong.value ? currentSong.value.hash || currentSong.value.id || "" : "";
      if (reqSeq !== lyricLoadRequestSeq || (currentKey && targetKey && currentKey !== targetKey)) {
        return;
      }
      currentLyric.value = loaded;
      desktopLyricLyricSeq += 1;
      broadcastDesktopLyricState();
      broadcastDesktopLyricPosition();
    } catch {
      if (reqSeq !== lyricLoadRequestSeq) return;
      currentLyric.value = {
        lines: [],
        rawText: "",
        isKrc: false,
        hasTranslation: false,
        hasPhonetic: false,
      };
      desktopLyricLyricSeq += 1;
      broadcastDesktopLyricState();
    } finally {
      if (reqSeq === lyricLoadRequestSeq) {
        isLoadingLyric.value = false;
      }
    }
  }

  async function searchLyricCandidatesForCurrentSong(customKeyword?: string) {
    const song = currentSong.value;
    if (!song) {
      notify("请先播放一首歌曲再搜索候选歌词");
      return;
    }
    const reqSeq = ++lyricCandidateSearchSeq;
    isSearchingLyricCandidates.value = true;
    try {
      const list = await kugouClient.searchLyricCandidates(song, customKeyword);
      if (
        reqSeq !== lyricCandidateSearchSeq ||
        !currentSong.value ||
        !isSameHistoryTrack(currentSong.value, song)
      ) {
        return;
      }
      lyricCandidates.value = list;
      if (list.length > 0 && !activeLyricCandidateId.value) {
        activeLyricCandidateId.value = list[0].id;
      }
      if (list.length === 0) {
        notify("未检索到其他匹配的候选歌词");
      }
    } catch (e: any) {
      if (reqSeq === lyricCandidateSearchSeq) {
        notify(`检索候选歌词失败：${String(e)}`);
      }
    } finally {
      if (reqSeq === lyricCandidateSearchSeq) {
        isSearchingLyricCandidates.value = false;
      }
    }
  }

  async function selectLyricCandidate(cand: LyricCandidateItem) {
    const song = currentSong.value;
    if (!song) return;
    const reqSeq = ++lyricLoadRequestSeq;
    isLoadingLyric.value = true;
    try {
      const updated = await kugouClient.applyLyricCandidate(
        song.hash,
        cand.id,
        cand.accesskey,
        cand.fmt
      );
      if (reqSeq !== lyricLoadRequestSeq) return;
      currentLyric.value = updated;
      activeLyricCandidateId.value = cand.id;
      lyricOffsetMs.value = 0;
      desktopLyricLyricSeq += 1;
      broadcastDesktopLyricState();
      broadcastDesktopLyricPosition();
      notify(`已切换歌词版本：「${cand.songName} - ${cand.singerName}」(${cand.fmt.toUpperCase()})`);
    } catch (e: any) {
      notify(`切换候选歌词失败：${String(e)}`);
    } finally {
      if (reqSeq === lyricLoadRequestSeq) {
        isLoadingLyric.value = false;
      }
    }
  }

  function adjustLyricOffsetMs(deltaMs: number) {
    if (deltaMs === 0) {
      lyricOffsetMs.value = 0;
      notify("已重置歌词时间轴偏移为 0.0s");
      return;
    }
    lyricOffsetMs.value = clampVal(lyricOffsetMs.value + deltaMs, -15000, 15000);
    const secStr = (lyricOffsetMs.value / 1000).toFixed(1);
    notify(
      `歌词时间轴偏移已校准：${lyricOffsetMs.value >= 0 ? "+" : ""}${secStr}s`
    );
  }

  let activeCommentsReqId = 0;
  let lastCommentsSongKey = "";

  async function loadCommentsForSong(
    song: Song | null = commentsModalSong.value || currentSong.value,
    sort: "hot" | "new" = "hot"
  ) {
    if (!song) return;
    const reqId = ++activeCommentsReqId;
    const songKey = getCommentSongKey(song);
    if (songKey !== lastCommentsSongKey) {
      lastCommentsSongKey = songKey;
      for (const k of Object.keys(floorRepliesMap)) {
        delete floorRepliesMap[k];
      }
    }
    commentSortType.value = sort;
    expandedFloorCommentId.value = null;
    isLoadingComments.value = true;
    try {
      const list = await kugouClient.getSongComments(song, sort, 1);
      if (
        reqId !== activeCommentsReqId ||
        (commentsModalSong.value &&
          getCommentSongKey(commentsModalSong.value) !== songKey)
      ) {
        return;
      }
      activeSongComments.value = Array.isArray(list) ? list : [];
      const firstTotal = Number(activeSongComments.value[0]?.totalCount || 0);
      let remoteCount = 0;
      if (!Number.isFinite(firstTotal) || firstTotal <= 0) {
        remoteCount = await kugouClient.getSongCommentCount(song).catch(() => 0);
        if (
          reqId !== activeCommentsReqId ||
          (commentsModalSong.value &&
            getCommentSongKey(commentsModalSong.value) !== songKey)
        ) {
          return;
        }
      }
      const bestTotal = Math.max(
        Number(remoteCount) || 0,
        Number.isFinite(firstTotal) ? firstTotal : 0,
        activeSongComments.value.length
      );
      activeSongCommentTotal.value = bestTotal;
      if (currentSong.value && isSameHistoryTrack(currentSong.value, song)) {
        currentSongCommentTotal.value = Math.max(
          currentSongCommentTotal.value,
          bestTotal
        );
      }
    } catch {
      if (
        reqId !== activeCommentsReqId ||
        (commentsModalSong.value &&
          getCommentSongKey(commentsModalSong.value) !== songKey)
      ) {
        return;
      }
      if (
        isBrowserDevPreviewEnv() &&
        playQueue.value[0]?.hash === song.hash
      ) {
        const demo = createBrowserDevShowcase();
        activeSongComments.value = demo.comments;
        activeSongCommentTotal.value = 136;
        Object.assign(floorRepliesMap, demo.floorRepliesMap);
      } else {
        activeSongComments.value = [];
        activeSongCommentTotal.value = 0;
      }
    } finally {
      if (reqId === activeCommentsReqId) {
        isLoadingComments.value = false;
      }
    }
  }

  async function toggleCommentFloorReplies(cmt: SongComment) {
    if (expandedFloorCommentId.value === cmt.id) {
      expandedFloorCommentId.value = null;
      return;
    }
    expandedFloorCommentId.value = cmt.id;
    const targetSong = commentsModalSong.value;
    if (floorRepliesMap[cmt.id]?.length || !targetSong) return;
    const targetKey = getCommentSongKey(targetSong);
    const reqId = activeCommentsReqId;
    isLoadingFloorReplies.value = true;
    try {
      const replies = await kugouClient.getCommentFloorReplies(
        targetSong,
        cmt.id,
        cmt.specialChildId,
        1
      );
      if (
        reqId !== activeCommentsReqId ||
        getCommentSongKey(commentsModalSong.value) !== targetKey
      ) {
        return;
      }
      floorRepliesMap[cmt.id] = replies;
    } catch {
      if (
        reqId === activeCommentsReqId &&
        getCommentSongKey(commentsModalSong.value) === targetKey
      ) {
        floorRepliesMap[cmt.id] = [];
      }
    } finally {
      if (reqId === activeCommentsReqId) {
        isLoadingFloorReplies.value = false;
      }
    }
  }

  async function toggleLikeComment(cmt: SongComment) {
    const targetSong = commentsModalSong.value;
    if (!targetSong) return;
    const prevLiked = Boolean(cmt.isLiked);
    const prevCount = Number(cmt.likeCount) || 0;
    const nextLike = !prevLiked;
    cmt.isLiked = nextLike;
    cmt.likeCount = Math.max(0, prevCount + (nextLike ? 1 : -1));
    if (nextLike) {
      triggerHeartBurst(undefined, { compact: true });
    }
    try {
      await kugouClient.likeSongComment(targetSong, cmt.id, nextLike);
    } catch {
      cmt.isLiked = prevLiked;
      cmt.likeCount = prevCount;
      notify("点赞操作失败，请检查登录状态或网络连接");
    }
  }

  async function submitSongComment(content: string, replyToId?: string) {
    const targetSong = commentsModalSong.value;
    if (!targetSong) return false;
    const targetKey = getCommentSongKey(targetSong);
    const reqId = activeCommentsReqId;
    if (!isLoggedIn.value) {
      isLoginModalOpen.value = true;
      return false;
    }
    try {
      const created = await kugouClient.postSongComment(
        targetSong,
        content,
        replyToId
      );
      const stillSameModalSong =
        reqId === activeCommentsReqId &&
        getCommentSongKey(commentsModalSong.value) === targetKey;
      if (stillSameModalSong) {
        if (replyToId) {
          if (!floorRepliesMap[replyToId]) {
            floorRepliesMap[replyToId] = [];
          }
          floorRepliesMap[replyToId].unshift(created);
          const parent = activeSongComments.value.find((c) => c.id === replyToId);
          if (parent) {
            parent.replyCount = (Number(parent.replyCount) || 0) + 1;
          }
          expandedFloorCommentId.value = replyToId;
        } else {
          activeSongComments.value.unshift(created);
          activeSongCommentTotal.value = Math.max(
            activeSongCommentTotal.value + 1,
            activeSongComments.value.length
          );
        }
      }
      if (
        !replyToId &&
        currentSong.value &&
        isSameHistoryTrack(currentSong.value, targetSong)
      ) {
        currentSongCommentTotal.value = Math.max(
          currentSongCommentTotal.value + 1,
          stillSameModalSong ? activeSongCommentTotal.value : 0
        );
      }
      notify("评论已发布");
      return true;
    } catch (e: any) {
      notify(`发表评论失败：${String(e)}`);
      return false;
    }
  }

  async function togglePlayPause() {
    const toggleSeq = ++playPauseToggleSeq;
    const ctx = getSharedAudioContext();
    if (ctx.state === "suspended") {
      await ctx.resume();
    }
    if (!currentSong.value) {
      if (recommendSongs.value.length > 0) {
        await playSongFromList(recommendSongs.value[0], recommendSongs.value);
      }
      return;
    }
    if (!audio.src) {
      await loadAndPlayCurrentSong(currentPositionMs.value);
      return;
    }
    const activeCtx = getAudioContext();
    const preampNode = getMasterPreampGain();
    const targetLinear = sliderToPerceptualGain(volume.value);
    // 若当前处于已暂停状态，或正处于 155ms 暂停淡出窗口内被用户再次点击恢复播放：立即取消暂停定时器并执行平滑淡入
    if (audio.paused || isCurrentlyPauseFadingOut()) {
      cancelAudioContextSuspend();
      cancelPendingPauseFade(false);
      const targetGain = computeTargetMasterPreampLinearGain();
      if (playPauseFadeEnabled.value && activeCtx && preampNode) {
        applyMasterOutputVolume(volume.value, 0);
        const now = activeCtx.currentTime;
        preampNode.gain.cancelScheduledValues(now);
        preampNode.gain.setValueAtTime(Math.min(0.05, targetGain * 0.05), now);
        if (audio.paused) {
          await audio.play();
        }
        if (toggleSeq !== playPauseToggleSeq) return;
        const rampNow = activeCtx.currentTime;
        preampNode.gain.cancelScheduledValues(rampNow);
        preampNode.gain.setValueAtTime(Math.min(0.05, targetGain * 0.05), rampNow);
        preampNode.gain.linearRampToValueAtTime(targetGain, rampNow + 0.16);
      } else if (playPauseFadeEnabled.value && targetLinear > 0.0002) {
        const startFloor = targetLinear * 0.05;
        audio.volume = startFloor;
        if (audio.paused) {
          await audio.play();
        }
        if (toggleSeq !== playPauseToggleSeq) return;
        rampHtmlAudioVolume(startFloor, targetLinear, 160);
      } else {
        applyMasterOutputVolume(volume.value, 0);
        if (activeCtx && preampNode) {
          preampNode.gain.cancelScheduledValues(activeCtx.currentTime);
          preampNode.gain.setValueAtTime(targetGain, activeCtx.currentTime);
        }
        if (audio.paused) {
          await audio.play();
        }
      }
    } else {
      cancelPendingPauseFade(false);
      const targetGain = computeTargetMasterPreampLinearGain();
      if (playPauseFadeEnabled.value && activeCtx && preampNode) {
        const now = activeCtx.currentTime;
        preampNode.gain.cancelScheduledValues(now);
        preampNode.gain.setValueAtTime(targetGain, now);
        preampNode.gain.linearRampToValueAtTime(Math.min(0.02, targetGain * 0.02), now + 0.15);
        const timerId = window.setTimeout(() => {
          if (!isCurrentlyPauseFadingOut() || toggleSeq !== playPauseToggleSeq) return;
          setPauseFadingOutState(false, null);
          audio.pause();
          const ctxAfter = getAudioContext();
          const gainAfter = getMasterPreampGain();
          if (gainAfter && ctxAfter) {
            const restoredGain = computeTargetMasterPreampLinearGain();
            gainAfter.gain.cancelScheduledValues(ctxAfter.currentTime);
            gainAfter.gain.setValueAtTime(restoredGain, ctxAfter.currentTime);
          }
        }, 155);
        setPauseFadingOutState(true, timerId);
      } else if (playPauseFadeEnabled.value && audio.volume > 0.0002) {
        setPauseFadingOutState(true, null);
        const startVol = audio.volume;
        rampHtmlAudioVolume(startVol, startVol * 0.02, 150, () => {
          if (!isCurrentlyPauseFadingOut() || toggleSeq !== playPauseToggleSeq) return;
          setPauseFadingOutState(false, null);
          audio.pause();
          applyMasterOutputVolume(volume.value, 0);
        });
      } else {
        audio.pause();
      }
    }
  }

  async function playNext(fromAutoEnded = false) {
    lastTrackNavDir.value = "next";
    const len = playQueue.value.length;
    if (len === 0) return;

    if (!fromAutoEnded) {
      settleActiveTrackTelemetry();
    }

    if (fromAutoEnded && playMode.value === "repeat_one") {
      if (currentSong.value) {
        recordSongToRecentHistory(currentSong.value);
        touchActiveQueueLastPlayed(currentSong.value);
        activeTelemetrySong = currentSong.value;
        currentTrackPlayedAccumMs = 0;
        kugouClient.reportPlaybackTelemetry(currentSong.value, true, 0).catch(() => {});
      }
      const chorusStartSec = climaxModeEnabled.value
        ? getSongChorusWindowMs(currentSong.value, currentPlaybackInfo.value).startMs / 1000
        : 0;
      audio.currentTime = chorusStartSec;
      currentPositionMs.value = Math.round(chorusStartSec * 1000);
      await audio.play();
      broadcastDesktopLyricState();
      broadcastDesktopLyricPosition();
      return;
    }

    // 顺序播放模式 (sequence)：当自动播完队列最后一首曲目时自然停止播放，不循环回列表开头
    if (
      fromAutoEnded &&
      playMode.value === "sequence" &&
      currentIndex.value >= len - 1
    ) {
      resetCrossfadeDeck();
      audio.pause();
      isPlaying.value = false;
      currentPositionMs.value = 0;
      notify("已按顺序播放完当前队列全部曲目");
      return;
    }

    if (playMode.value === "shuffle" && len > 1) {
      if (currentIndex.value >= 0 && currentIndex.value < len) {
        shuffleHistoryStack.push(currentIndex.value);
        if (shuffleHistoryStack.length > 300) {
          shuffleHistoryStack.shift();
        }
      }
      shuffleForwardDeck = shuffleForwardDeck.filter(
        (idx) => idx >= 0 && idx < len && idx !== currentIndex.value
      );
      if (shuffleForwardDeck.length === 0) {
        rebuildShuffleDeck(currentIndex.value);
      }
      const nextIdx = shuffleForwardDeck.shift() ?? (currentIndex.value + 1) % len;
      currentIndex.value = nextIdx;
    } else {
      currentIndex.value = (currentIndex.value + 1) % len;
    }
    await loadAndPlayCurrentSong(0);
  }

  async function playPrev() {
    lastTrackNavDir.value = "prev";
    const len = playQueue.value.length;
    if (len === 0) return;
    settleActiveTrackTelemetry();
    if (playMode.value === "shuffle" && len > 1 && shuffleHistoryStack.length > 0) {
      while (shuffleHistoryStack.length > 0) {
        const prevIdx = shuffleHistoryStack.pop()!;
        if (prevIdx >= 0 && prevIdx < len && prevIdx !== currentIndex.value) {
          if (currentIndex.value >= 0 && currentIndex.value < len) {
            shuffleForwardDeck.unshift(currentIndex.value);
          }
          currentIndex.value = prevIdx;
          await loadAndPlayCurrentSong(0);
          return;
        }
      }
    }
    currentIndex.value = (currentIndex.value - 1 + len) % len;
    await loadAndPlayCurrentSong(0);
  }

  function insertPlayNext(song: Song) {
    if (playQueue.value.length === 0) {
      playSongFromList(song, [song], "当前播放队列");
      return;
    }
    const existingIdx = playQueue.value.findIndex(
      (s) => (s.hash && s.hash === song.hash) || (s.id && s.id === song.id)
    );
    if (existingIdx >= 0 && existingIdx === currentIndex.value) {
      notify(`「${song.title}」正在播放中`);
      return;
    }
    if (existingIdx >= 0 && existingIdx !== currentIndex.value) {
      playQueue.value.splice(existingIdx, 1);
      if (existingIdx < currentIndex.value) {
        currentIndex.value -= 1;
      }
    }
    const insertIdx = Math.min(playQueue.value.length, currentIndex.value + 1);
    playQueue.value.splice(insertIdx, 0, song);
    resetCrossfadeDeck();
    if (playMode.value === "shuffle") {
      // 在随机模式下也将插队曲目置为洗牌池首项，确保下一首必播该曲
      shuffleForwardDeck = [
        insertIdx,
        ...shuffleForwardDeck.filter((i) => i !== insertIdx),
      ];
    }
    notify(`已安排下一首播放：「${song.title}」`);
    triggerFlyToQueue(undefined, song.coverUrl);
  }

  function appendSongToQueue(song: Song) {
    if (playQueue.value.length === 0) {
      playSongFromList(song, [song], "当前播放队列");
      return;
    }
    const existingIdx = playQueue.value.findIndex(
      (s) => (s.hash && s.hash === song.hash) || (s.id && s.id === song.id)
    );
    if (existingIdx >= 0) {
      notify(`「${song.title}」已在播放队列中 (第 ${existingIdx + 1} 首)`);
      return;
    }
    playQueue.value.push(song);
    if (playMode.value === "shuffle") {
      shuffleForwardDeck.push(playQueue.value.length - 1);
    }
    notify(`已加入播放队列末尾：「${song.title}」`);
    triggerFlyToQueue(undefined, song.coverUrl);
  }

  function removeSongFromQueue(index: number) {
    if (index < 0 || index >= playQueue.value.length) return;
    const removed = playQueue.value[index];
    playQueue.value.splice(index, 1);
    if (playQueue.value.length === 0) {
      settleActiveTrackTelemetry();
      audio.pause();
      currentIndex.value = -1;
      return;
    }
    if (index < currentIndex.value) {
      currentIndex.value -= 1;
    } else if (index === currentIndex.value) {
      settleActiveTrackTelemetry();
      currentIndex.value = currentIndex.value % playQueue.value.length;
      loadAndPlayCurrentSong(0);
    }
    if (removed) {
      notify(`已从播放队列移除「${removed.title}」`);
    }
  }

  function clearPlayQueue() {
    if (playQueue.value.length > 0) {
      saveQueueSnapshot(
        playQueue.value,
        currentQueueTitle.value || "清空前队列快照",
        currentQueueTitle.value || "清空前队列快照",
        currentSong.value?.hash
      );
    }
    settleActiveTrackTelemetry();
    audio.pause();
    playQueue.value = [];
    originalQueueSnapshot = [];
    currentIndex.value = -1;
    notify("已清空当前播放队列（已自动保存至最近队列记忆）");
  }

  function moveQueueItem(fromIndex: number, toIndex: number) {
    if (
      fromIndex === toIndex ||
      fromIndex < 0 ||
      toIndex < 0 ||
      fromIndex >= playQueue.value.length ||
      toIndex >= playQueue.value.length
    ) {
      return;
    }
    const currentHash = currentSong.value?.hash;
    const currentId = currentSong.value?.id;
    const list = [...playQueue.value];
    const [moved] = list.splice(fromIndex, 1);
    list.splice(toIndex, 0, moved);
    playQueue.value = list;
    if (currentHash || currentId) {
      const newIdx = list.findIndex(
        (s) => (currentHash && s.hash === currentHash) || (currentId && s.id === currentId)
      );
      if (newIdx >= 0) currentIndex.value = newIdx;
    }
    if (playMode.value === "shuffle") {
      rebuildShuffleDeck(currentIndex.value);
    }
  }

  function sortPlayQueue(
    mode: "artist" | "quality" | "duration" | "title" | "restore",
    direction: "asc" | "desc" = "asc"
  ) {
    if (playQueue.value.length <= 1) return;
    const activeSongRef = currentSong.value;
    const activeHash = activeSongRef?.hash;
    const activeId = activeSongRef?.id;

    let nextList: Song[];
    if (mode === "restore" && originalQueueSnapshot.length > 0) {
      const presentHashes = new Set(playQueue.value.map((s) => s.hash || s.id));
      nextList = originalQueueSnapshot.filter((s) => presentHashes.has(s.hash || s.id));
      const added = playQueue.value.filter(
        (s) => !originalQueueSnapshot.some((o) => (o.hash || o.id) === (s.hash || s.id))
      );
      nextList = [...nextList, ...added];
    } else {
      const qualityScore = (s: Song) => {
        if (s.isHiRes || s.hasHiRes || (s.sizeHiRes || 0) > 0 || Boolean(s.hashHiRes)) return 4;
        if (s.isLossless || s.isSq || s.hasSq || (s.sizeFlac || 0) > 0 || Boolean(s.hashFlac)) return 3;
        if (s.hasHighQuality || (s.size320 || 0) > 0 || Boolean(s.hash320)) return 2;
        return 1;
      };
      nextList = [...playQueue.value].sort((a, b) => {
        let cmp = 0;
        if (mode === "artist") {
          cmp = (a.artist || "").localeCompare(b.artist || "", "zh-CN");
          if (cmp === 0) cmp = (a.title || "").localeCompare(b.title || "", "zh-CN");
        } else if (mode === "quality") {
          cmp = qualityScore(a) - qualityScore(b);
          if (cmp === 0) {
            cmp =
              (a.sizeHiRes || a.sizeFlac || a.size320 || a.size128 || 0) -
              (b.sizeHiRes || b.sizeFlac || b.size320 || b.size128 || 0);
          }
        } else if (mode === "duration") {
          cmp = (a.durationMs || 0) - (b.durationMs || 0);
        } else {
          cmp = (a.title || "").localeCompare(b.title || "", "zh-CN");
        }
        return direction === "asc" ? cmp : -cmp;
      });
    }

    playQueue.value = nextList;
    if (activeSongRef) {
      let newIdx = nextList.indexOf(activeSongRef);
      if (newIdx < 0 && (activeHash || activeId)) {
        newIdx = nextList.findIndex(
          (s) => (activeHash && s.hash === activeHash) || (activeId && s.id === activeId)
        );
      }
      if (newIdx >= 0) currentIndex.value = newIdx;
    }
    if (playMode.value === "shuffle") {
      rebuildShuffleDeck(currentIndex.value);
    }
  }

  function reorderQueueByMood(mood: "energetic" | "chill" | "vocal") {
    if (playQueue.value.length <= 2) return;
    const current = currentSong.value;
    const rest = playQueue.value.filter((_, idx) => idx !== currentIndex.value);
    rest.sort((a, b) => {
      if (mood === "energetic") {
        return (a.durationMs || 200000) - (b.durationMs || 200000);
      } else if (mood === "chill") {
        return (b.durationMs || 200000) - (a.durationMs || 200000);
      } else {
        return a.artist.localeCompare(b.artist, "zh-CN");
      }
    });
    playQueue.value = current ? [current, ...rest] : rest;
    currentIndex.value = 0;
    const moodLabels = {
      energetic: "律动高能",
      chill: "舒缓松弛",
      vocal: "人声叙事",
    };
    notify(`AI 智能编排已按「${moodLabels[mood]}」重塑队列顺序`);
  }

  function switchToRecentQueue(idOrIdx: string | number) {
    const target =
      typeof idOrIdx === "number"
        ? recentPlayQueues.value[idOrIdx]
        : recentPlayQueues.value.find((q) => q.id === idOrIdx);
    if (!target || target.songs.length === 0) return;
    activeQueueSnapshotId = target.id;
    const startSong =
      (target.activeHash &&
        target.songs.find((s) => s.hash === target.activeHash)) ||
      target.songs[0];
    playSongFromList(startSong, target.songs, target.title);
    touchActiveQueueLastPlayed(startSong);
    notify(`已切换至队列「${target.title}」`);
  }

  function removeRecentQueue(idOrIdx: string | number) {
    const idx =
      typeof idOrIdx === "number"
        ? idOrIdx
        : recentPlayQueues.value.findIndex((q) => q.id === idOrIdx);
    if (idx >= 0 && idx < recentPlayQueues.value.length) {
      recentPlayQueues.value.splice(idx, 1);
      persistRecentPlayQueues();
    }
  }

  function syncWindowsMediaSession(song: Song) {
    if (typeof navigator === "undefined" || !("mediaSession" in navigator)) return;
    try {
      navigator.mediaSession.metadata = new MediaMetadata({
        title: song.title,
        artist: song.artist,
        album: song.album || "微风音乐 · 酷狗概念版",
        artwork: song.coverUrl
          ? [{ src: song.coverUrl, sizes: "400x400", type: "image/jpeg" }]
          : [],
      });
      navigator.mediaSession.setActionHandler("play", () => togglePlayPause());
      navigator.mediaSession.setActionHandler("pause", () => togglePlayPause());
      navigator.mediaSession.setActionHandler("previoustrack", () => playPrev());
      navigator.mediaSession.setActionHandler("nexttrack", () => playNext(false));
    } catch {
      // ignore unsupported mediaSession environment
    }
  }

  function seekToMs(ms: number) {
    if (!audio.src) return;
    const clamped = Math.max(0, Math.min(ms, durationMs.value || ms));
    isCrossfadeTransitioning = false;
    cancelPendingPauseFade(true);
    audio.currentTime = clamped / 1000;
    currentPositionMs.value = clamped;
    if (desktopLyricEnabled.value) {
      broadcastDesktopLyricPosition();
    }
    if (isMiniPlayerMode.value) {
      broadcastMiniPlayerPosition();
    }
  }

  function setVolume(val: number) {
    const v = Math.max(0, Math.min(1, val));
    cancelVolumeRamp();
    volume.value = v;
    applyMasterOutputVolume(v, 0.015);
    localStorage.setItem("bm_volume", String(v));
    if (
      (nightLoudnessEnabled.value || acousticField.equalLoudnessContour) &&
      getAudioContext()
    ) {
      syncDspChain(false);
    }
    if (isMiniPlayerMode.value) {
      broadcastMiniPlayerPosition();
    }
  }

  async function switchAudioQuality(q: AudioQualityValue) {
    const normalizedQ = normalizeConceptQuality(q);
    preferredQuality.value = normalizedQ;
    activeCloudStreamEffect.value = "";
    localStorage.setItem("bm_pref_quality", normalizedQ);
    localStorage.removeItem("bm_cloud_stream_effect");
    if (currentSong.value) {
      const resumeMs = currentPositionMs.value;
      await loadAndPlayCurrentSong(resumeMs);
      if (currentPlaybackInfo.value) {
        notify(
          `已切换至 ${currentPlaybackInfo.value.qualityLabel} (${currentPlaybackInfo.value.techSpecText})`
        );
      }
    }
  }

  async function switchCloudStreamEffect(effectQuality: string) {
    const cleanEffect = String(effectQuality || "").trim().toLowerCase();
    if (blockedOverTierEffects.has(cleanEffect)) {
      await switchAudioQuality("high");
      return;
    }
    const nextVal =
      activeCloudStreamEffect.value === cleanEffect ? "" : cleanEffect;
    activeCloudStreamEffect.value = nextVal;
    if (nextVal) {
      localStorage.setItem("bm_cloud_stream_effect", nextVal);
    } else {
      localStorage.removeItem("bm_cloud_stream_effect");
    }
    if (currentSong.value) {
      const resumeMs = currentPositionMs.value;
      await loadAndPlayCurrentSong(resumeMs);
      if (nextVal && currentPlaybackInfo.value) {
        notify(
          `已切换酷狗云端合成音效流：${currentPlaybackInfo.value.qualityLabel} (${currentPlaybackInfo.value.techSpecText})`
        );
      } else {
        notify("已退出云端合成音效流，恢复标准母带音源");
      }
    } else {
      notify(
        nextVal
          ? `已预设酷狗云端合成音效流 (${nextVal})，将在播放时生效`
          : "已恢复标准母带音源流"
      );
    }
  }

  function setPlayMode(mode: PlayMode, silent = false) {
    const normalized: PlayMode =
      mode === "loop_all"
        ? "repeat_all"
        : mode === "loop_one"
        ? "repeat_one"
        : mode;
    playMode.value = normalized;
    localStorage.setItem("bm_play_mode", normalized);
    resetCrossfadeDeck();
    if (normalized === "shuffle") {
      shuffleHistoryStack = [];
      rebuildShuffleDeck(currentIndex.value);
    }
    if (!silent) {
      const labelMap: Record<PlayMode, string> = {
        repeat_all: "已切换为列表循环",
        repeat_one: "已切换为单曲循环",
        shuffle: "已切换为真随机洗牌播放 (Fisher-Yates)",
        sequence: "已切换为顺序播放（播完列表末尾自动停止）",
        loop_all: "已切换为列表循环",
        loop_one: "已切换为单曲循环",
      };
      notify(labelMap[normalized]);
    }
  }

  function cyclePlayMode() {
    const order: PlayMode[] = ["repeat_all", "repeat_one", "shuffle", "sequence"];
    const currIdx = order.indexOf(playMode.value);
    const next = order[(currIdx >= 0 ? currIdx + 1 : 0) % order.length];
    setPlayMode(next, false);
  }

  function openSongContextMenu(e: MouseEvent, song: Song, _sourcePlaylist?: any) {
    e.preventDefault();
    e.stopPropagation();
    contextMenu.x = Math.max(12, Math.min(e.clientX, window.innerWidth - 260));
    contextMenu.y = Math.max(12, Math.min(e.clientY, window.innerHeight - 440));
    contextMenu.song = song;
    contextMenu.visible = true;
  }

  function closeSongContextMenu() {
    contextMenu.visible = false;
  }

  function formatDownloadTimestamp(tsMs: number): string {
    const d = new Date(tsMs);
    const yyyy = d.getFullYear();
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    const hh = String(d.getHours()).padStart(2, "0");
    const mi = String(d.getMinutes()).padStart(2, "0");
    return `${yyyy}-${mm}-${dd} ${hh}:${mi}`;
  }

  function persistDownloadHistoryList() {
    try {
      const compact = downloadHistory.value.slice(0, 500).map((rec) => ({
        ...rec,
        songSnapshot: rec.songSnapshot ? compactQueueSong(rec.songSnapshot) : undefined,
      }));
      localStorage.setItem("bm_dl_history", JSON.stringify(compact));
    } catch {
      try {
        const slim = downloadHistory.value.slice(0, 200).map((rec) => ({
          ...rec,
          songSnapshot: undefined,
        }));
        localStorage.setItem("bm_dl_history", JSON.stringify(slim));
      } catch {
        // ignore storage quota errors
      }
    }
  }

  function syncDownloadingActiveFlag() {
    isDownloadingSong.value =
      activeDownloadTaskIds.size > 0 ||
      downloadQueue.value.some(
        (t) =>
          t.status === "resolving" ||
          t.status === "downloading" ||
          t.status === "tagging"
      );
  }

  async function runSingleDownloadTask(task: DownloadQueueItem) {
    activeDownloadTaskIds.add(task.taskId);
    canceledDownloadTaskIds.delete(task.taskId);
    syncDownloadingActiveFlag();
    task.status = "resolving";
    task.percent = 4;
    task.startedAtMs = Date.now();
    task.speedBytesPerSec = 0;
    task.etaSeconds = 0;
    task.message = "正在解析高清音源直链...";
    try {
      const res = await kugouClient.downloadSong(
        task.song,
        task.quality,
        downloadMusicDir.value,
        downloadLrcDir.value,
        exportStandaloneLrc.value,
        embedCoverAndTags.value,
        embedLyricsInAudio.value,
        bilingualLrcExport.value,
        skipExistingFiles.value,
        task.taskId,
        downloadFileNaming.value,
        downloadFolderOrganize.value
      );
      if (canceledDownloadTaskIds.has(task.taskId)) {
        task.status = "canceled";
        task.speedBytesPerSec = 0;
        task.etaSeconds = 0;
        task.message = "已取消下载";
        return;
      }
      task.status = "done";
      task.percent = 100;
      task.speedBytesPerSec = 0;
      task.etaSeconds = 0;
      task.savedAudioPath = res.savedAudioPath;
      task.savedLrcPath = res.savedLrcPath || "";
      task.qualityLabel = res.qualityLabel;
      task.downloadedBytes = res.fileSizeBytes;
      task.totalBytes = res.fileSizeBytes;
      task.message = res.message;

      const nowMs = Date.now();
      const sizeMb =
        res.fileSizeBytes >= 1024 * 1024
          ? `${(res.fileSizeBytes / (1024 * 1024)).toFixed(1)} MB`
          : `${Math.max(1, Math.round(res.fileSizeBytes / 1024))} KB`;
      const record: DownloadHistoryRecord = {
        id: `${task.song.hash || task.song.id}_${nowMs}`,
        songHash: task.song.hash || task.song.id || "",
        title: task.song.title,
        artist: task.song.artist,
        album: task.song.album || "",
        coverUrl: task.song.coverUrl || "",
        qualityLabel: res.qualityLabel,
        savedPath: res.savedAudioPath,
        savedLrcPath: res.savedLrcPath || undefined,
        fileSizeBytes: res.fileSizeBytes,
        fileSizeText: sizeMb,
        timestampText: formatDownloadTimestamp(nowMs),
        completedAtMs: nowMs,
        songSnapshot: compactQueueSong(task.song),
      };
      downloadHistory.value.unshift(record);
      if (downloadHistory.value.length > 500) {
        downloadHistory.value = downloadHistory.value.slice(0, 500);
      }
      persistDownloadHistoryList();
      notify(`下载完成：「${task.song.title}」(${res.qualityLabel} · ${sizeMb})`);
      if (
        autoSyncLocalAfterDownload.value &&
        (localMusicSongs.value.length > 0 || activeTab.value === "local_music")
      ) {
        void scanLocalMusicLibrary(undefined, true);
      }
    } catch (e: any) {
      const errText = String(e || "下载失败");
      if (
        canceledDownloadTaskIds.has(task.taskId) ||
        errText.includes("下载任务已取消")
      ) {
        task.status = "canceled";
        task.speedBytesPerSec = 0;
        task.etaSeconds = 0;
        task.message = "已取消下载并清理临时文件";
      } else {
        task.status = "error";
        task.speedBytesPerSec = 0;
        task.etaSeconds = 0;
        task.message = errText;
        notify(`「${task.song.title}」下载失败：${task.message}`);
      }
    } finally {
      activeDownloadTaskIds.delete(task.taskId);
      downloadSpeedTrackers.delete(task.taskId);
      canceledDownloadTaskIds.delete(task.taskId);
      syncDownloadingActiveFlag();
      void processDownloadQueueWorker();
    }
  }

  async function processDownloadQueueWorker() {
    const limit = Math.max(1, Math.min(3, Number(downloadConcurrency.value) || 2));
    while (activeDownloadTaskIds.size < limit) {
      const nextTask = downloadQueue.value.find(
        (t) => t.status === "queued" && !activeDownloadTaskIds.has(t.taskId)
      );
      if (!nextTask) break;
      void runSingleDownloadTask(nextTask);
    }
    syncDownloadingActiveFlag();
  }

  function pauseDownloadTask(taskId: string) {
    const task = downloadQueue.value.find((t) => t.taskId === taskId);
    if (!task) return;
    if (task.status === "queued") {
      task.status = "paused";
      task.message = "已暂停等待";
      return;
    }
    if (
      task.status === "resolving" ||
      task.status === "downloading" ||
      task.status === "tagging"
    ) {
      canceledDownloadTaskIds.add(taskId);
      task.status = "paused";
      task.speedBytesPerSec = 0;
      task.etaSeconds = 0;
      task.message = "已暂停，恢复后将重新续传";
      void kugouClient.cancelDownloadTask(taskId).catch(() => {});
    }
  }

  function resumeDownloadTask(taskId: string) {
    const task = downloadQueue.value.find((t) => t.taskId === taskId);
    if (!task) return;
    if (
      task.status === "paused" ||
      task.status === "error" ||
      task.status === "canceled"
    ) {
      canceledDownloadTaskIds.delete(taskId);
      task.status = "queued";
      task.percent = 0;
      task.downloadedBytes = 0;
      task.speedBytesPerSec = 0;
      task.etaSeconds = 0;
      task.message = "等待队列调度...";
      void processDownloadQueueWorker();
    }
  }

  function pauseAllDownloads() {
    let pausedCount = 0;
    for (const task of downloadQueue.value) {
      if (
        task.status === "queued" ||
        task.status === "resolving" ||
        task.status === "downloading"
      ) {
        pauseDownloadTask(task.taskId);
        pausedCount++;
      }
    }
    if (pausedCount > 0) {
      notify(`已暂停 ${pausedCount} 个下载任务`);
    }
  }

  function resumeAllDownloads() {
    let resumedCount = 0;
    for (const task of downloadQueue.value) {
      if (task.status === "paused") {
        task.status = "queued";
        task.message = "等待队列调度...";
        resumedCount++;
      }
    }
    if (resumedCount > 0) {
      notify(`已恢复 ${resumedCount} 个已暂停任务`);
      void processDownloadQueueWorker();
    }
  }

  function prioritizeDownloadTask(taskId: string) {
    const idx = downloadQueue.value.findIndex((t) => t.taskId === taskId);
    if (idx <= 0) return;
    const [task] = downloadQueue.value.splice(idx, 1);
    downloadQueue.value.unshift(task);
    notify(`已将「${task.song.title}」置顶为优先下载`);
  }

  async function cancelDownloadTask(taskId: string) {
    const task = downloadQueue.value.find((t) => t.taskId === taskId);
    if (!task) return;
    canceledDownloadTaskIds.add(taskId);
    const isRunning =
      task.status === "resolving" ||
      task.status === "downloading" ||
      task.status === "tagging";
    task.status = "canceled";
    task.speedBytesPerSec = 0;
    task.etaSeconds = 0;
    task.message = "已取消下载并清理临时文件";
    if (isRunning) {
      await kugouClient.cancelDownloadTask(taskId).catch(() => {});
    }
    syncDownloadingActiveFlag();
    void processDownloadQueueWorker();
  }

  function retryDownloadTask(taskId: string) {
    resumeDownloadTask(taskId);
  }

  function retryAllFailedDownloads() {
    let count = 0;
    for (const task of downloadQueue.value) {
      if (task.status === "error" || task.status === "canceled") {
        task.status = "queued";
        task.percent = 0;
        task.downloadedBytes = 0;
        task.speedBytesPerSec = 0;
        task.etaSeconds = 0;
        task.message = "重新排队中...";
        count++;
      }
    }
    if (count > 0) {
      notify(`已重新加入 ${count} 个失败/取消任务`);
      void processDownloadQueueWorker();
    }
  }

  async function playDownloadedLocalFile(
    savedPath: string,
    meta?: { title?: string; artist?: string; album?: string; coverUrl?: string }
  ) {
    const cleanPath = String(savedPath || "").trim();
    if (!cleanPath) {
      notify("该记录未包含有效的本地文件路径");
      return;
    }
    const dirSep = Math.max(cleanPath.lastIndexOf("\\"), cleanPath.lastIndexOf("/"));
    const parentDir = dirSep > 0 ? cleanPath.slice(0, dirSep) : downloadMusicDir.value;
    const normTarget = cleanPath.replace(/\//g, "\\").toLowerCase();
    const findMatch = (list: Song[]) =>
      list.find((s) => {
        const mix = String(s.mixSongId || "");
        const p = mix.toLowerCase().startsWith("local://") ? mix.slice(8) : mix;
        return p.replace(/\//g, "\\").toLowerCase() === normTarget;
      });
    let matched = findMatch(localMusicSongs.value);
    if (!matched && parentDir) {
      try {
        const scanned = await kugouClient.scanLocalMusicDir(parentDir, {
          scanSubfolders: false,
          minDurationSec: 0,
          minFileSizeKb: 0,
          preferEmbeddedTags: localMusicSettings.value.preferEmbeddedTags,
          deduplicate: false,
        });
        matched = findMatch(scanned);
      } catch {
        // ignore
      }
    }
    if (matched) {
      await playSongFromList(matched, [matched], "本地已下载");
      return;
    }
    notify(`未能定位本地音频文件（可能已移动或删除）：${meta?.title || cleanPath}`);
  }

  async function redownloadHistoryItem(record: DownloadHistoryRecord) {
    if (!record) return;
    if (record.songSnapshot) {
      await triggerDownloadSong(record.songSnapshot);
      return;
    }
    notify("该历史条目为早期简略记录，缺少在线曲目快照，请从搜索或歌单重新下载");
  }

  function removeDownloadHistoryItem(recordId: string) {
    downloadHistory.value = downloadHistory.value.filter((r) => r.id !== recordId);
    persistDownloadHistoryList();
  }

  async function triggerDownloadSong(
    song: Song,
    customQuality?: AudioQualityValue
  ) {
    if (!downloadFeatureUnlocked.value) {
      notify("当前未启用离线保存功能");
      return;
    }
    if (!song || String(song.mixSongId || "").startsWith("local://")) {
      notify("该曲目已经是本地磁盘音频，无需重复下载");
      return;
    }
    const targetQuality = normalizeConceptQuality(
      customQuality || downloadQuality.value
    );
    downloadModalSong.value = null;

    // 若同曲目同音质已在排队、暂停或下载中则不重复添加
    const duplicate = downloadQueue.value.find(
      (t) =>
        (t.status === "queued" ||
          t.status === "paused" ||
          t.status === "resolving" ||
          t.status === "downloading" ||
          t.status === "tagging") &&
        t.quality === targetQuality &&
        ((song.hash && t.song.hash === song.hash) ||
          (song.id && t.song.id === song.id))
    );
    if (duplicate) {
      notify(`「${song.title}」已在当前下载队列中`);
      return;
    }

    const task: DownloadQueueItem = {
      taskId: `dl_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      song: normSong(song),
      quality: targetQuality,
      status: "queued",
      downloadedBytes: 0,
      totalBytes: 0,
      percent: 0,
      speedBytesPerSec: 0,
      etaSeconds: 0,
      message: "等待队列调度...",
      createdAtMs: Date.now(),
    };
    downloadQueue.value.unshift(task);
    if (downloadQueue.value.length > 100) {
      downloadQueue.value = downloadQueue.value.slice(0, 100);
    }

    const pendingCount = downloadQueue.value.filter(
      (t) =>
        t.status === "queued" ||
        t.status === "resolving" ||
        t.status === "downloading" ||
        t.status === "tagging"
    ).length;
    notify(
      pendingCount > 1
        ? `已加入下载队列：「${song.title}」（当前队列 ${pendingCount} 首）`
        : `开始下载「${song.title}」（可在设置 → 下载管理查看实时进度）`
    );

    void processDownloadQueueWorker();
  }

  async function saveSongLyricOnly(song: Song) {
    if (!downloadFeatureUnlocked.value) {
      notify("当前未启用离线保存功能");
      return;
    }
    if (!song) return;
    isSavingLyricOnly.value = true;
    try {
      const res = await kugouClient.saveSongLyricOnly(
        song,
        downloadLrcDir.value,
        downloadMusicDir.value,
        bilingualLrcExport.value,
        downloadFileNaming.value,
        downloadFolderOrganize.value
      );
      notify(
        `已保存歌词文件（${res.lineCount} 句${res.hasTranslation ? " · 含双语译文" : ""}）：${res.savedLrcPath}`
      );
      return res;
    } catch (e: any) {
      notify(`保存歌词失败：${String(e)}`);
      return null;
    } finally {
      isSavingLyricOnly.value = false;
    }
  }

  async function copySongLrcToClipboard(song: Song) {
    if (!song) return false;
    try {
      let lyricData =
        currentSong.value &&
        ((song.hash && currentSong.value.hash === song.hash) ||
          (song.id && currentSong.value.id === song.id)) &&
        currentLyric.value.lines.length > 0
          ? currentLyric.value
          : await kugouClient.getSongLyric(song);

      const lines = lyricData?.lines || [];
      if (lines.length === 0) {
        notify("该歌曲暂无可复制的时间轴歌词");
        return false;
      }
      const formatTag = (ms: number) => {
        const safe = Math.max(0, Math.round(ms));
        const min = Math.floor(safe / 60000);
        const sec = Math.floor((safe % 60000) / 1000);
        const cs = Math.floor((safe % 1000) / 10);
        return `[${String(min).padStart(2, "0")}:${String(sec).padStart(2, "0")}.${String(cs).padStart(2, "0")}]`;
      };
      const out: string[] = [];
      for (const ln of lines) {
        const tag = formatTag(ln.startTimeMs);
        out.push(`${tag}${ln.text}`);
        if (bilingualLrcExport.value && ln.translation?.trim()) {
          out.push(`${tag}${ln.translation.trim()}`);
        }
      }
      await navigator.clipboard.writeText(out.join("\n"));
      notify(`已复制标准带时间戳 .lrc 歌词（${lines.length} 句）`);
      return true;
    } catch (e: any) {
      notify(`复制歌词失败：${String(e)}`);
      return false;
    }
  }

  function clearCompletedDownloads() {
    downloadQueue.value = downloadQueue.value.filter(
      (t) =>
        t.status === "queued" ||
        t.status === "paused" ||
        t.status === "resolving" ||
        t.status === "downloading" ||
        t.status === "tagging"
    );
  }

  function clearAllDownloads() {
    for (const t of downloadQueue.value) {
      if (
        t.status === "resolving" ||
        t.status === "downloading" ||
        t.status === "tagging"
      ) {
        canceledDownloadTaskIds.add(t.taskId);
        void kugouClient.cancelDownloadTask(t.taskId).catch(() => {});
      }
    }
    downloadQueue.value = [];
    syncDownloadingActiveFlag();
  }

  function removeDownloadQueueItem(taskId: string) {
    const target = downloadQueue.value.find((t) => t.taskId === taskId);
    if (
      target &&
      (target.status === "resolving" ||
        target.status === "downloading" ||
        target.status === "tagging")
    ) {
      canceledDownloadTaskIds.add(taskId);
      void kugouClient.cancelDownloadTask(taskId).catch(() => {});
    }
    downloadQueue.value = downloadQueue.value.filter((t) => t.taskId !== taskId);
    syncDownloadingActiveFlag();
  }

  function batchAppendSongsToQueue(songs: Song[]) {
    if (!songs || songs.length === 0) return;
    let added = 0;
    for (const song of songs) {
      if (!song) continue;
      const exists = playQueue.value.some(
        (s) =>
          (s.hash && song.hash && s.hash.toLowerCase() === song.hash.toLowerCase()) ||
          (s.id && song.id && s.id === song.id)
      );
      if (!exists) {
        playQueue.value.push(song);
        added++;
      }
    }
    if (added > 0) {
      triggerFlyToQueue(undefined, songs[0]?.coverUrl);
    }
    notify(
      added > 0
        ? `已将 ${added} 首曲目批量追加至播放队列`
        : "所选曲目已在当前播放队列中"
    );
  }

  async function batchDownloadSongs(
    songs: Song[],
    customQuality?: AudioQualityValue
  ) {
    if (!downloadFeatureUnlocked.value) {
      notify("当前未启用离线保存功能");
      return;
    }
    const valid = (songs || []).filter(
      (s) => s && !String(s.mixSongId || "").startsWith("local://")
    );
    if (valid.length === 0) {
      notify("所选曲目均为本地磁盘音频，无需重复下载");
      return;
    }
    const targetQuality = normalizeConceptQuality(
      customQuality || downloadQuality.value
    );
    let enqueued = 0;
    for (const song of valid) {
      const exists = downloadQueue.value.some(
        (t) =>
          (t.status === "queued" ||
            t.status === "paused" ||
            t.status === "resolving" ||
            t.status === "downloading" ||
            t.status === "tagging") &&
          t.quality === targetQuality &&
          ((song.hash && t.song.hash === song.hash) ||
            (song.id && t.song.id === song.id))
      );
      if (!exists) {
        downloadQueue.value.push({
          taskId: `dl_${Date.now()}_${enqueued}_${Math.random().toString(36).slice(2, 6)}`,
          song: normSong(song),
          quality: targetQuality,
          status: "queued",
          downloadedBytes: 0,
          totalBytes: 0,
          percent: 0,
          speedBytesPerSec: 0,
          etaSeconds: 0,
          message: "等待队列调度...",
          createdAtMs: Date.now(),
        });
        enqueued++;
      }
    }
    if (enqueued === 0) {
      notify("所选曲目已在当前下载队列中");
      return;
    }
    notify(`已将 ${enqueued} 首曲目加入后台下载队列，可在设置 → 下载管理查看进度`);
    void processDownloadQueueWorker();
  }

  async function clearCacheCategory(
    category: "audio" | "image" | "snapshot" | "lyric" | "all"
  ) {
    try {
      const updated = await kugouClient.clearCacheCategory(category);
      cacheStats.audioStreamMb = Number((updated.audioStreamMb || 0).toFixed(2));
      cacheStats.artworkImageMb = Number((updated.artworkImageMb || 0).toFixed(2));
      cacheStats.playlistSnapshotMb = Number((updated.playlistSnapshotMb || 0).toFixed(2));
      cacheStats.lyricAndUrlMb = Number((updated.lyricAndUrlMb || 0).toFixed(2));
    } catch {
      if (category === "audio" || category === "all") cacheStats.audioStreamMb = 0;
      if (category === "image" || category === "all") cacheStats.artworkImageMb = 0;
      if (category === "snapshot" || category === "all") cacheStats.playlistSnapshotMb = 0;
      if (category === "lyric" || category === "all") cacheStats.lyricAndUrlMb = 0;
    }
    notify("已完成指定磁盘缓存分项清理");
  }

  // 自动持久化播放队列与进度（索引即时写、队列 1200ms+Idle 防抖、进度 10s 节流，暂停/切歌/隐藏/退出时立即冲刷）
  watch(currentIndex, (idx) => {
    try {
      localStorage.setItem("bm_saved_index", String(idx));
    } catch {
      // ignore storage quota error
    }
  });
  watch(
    playQueue,
    () => {
      schedulePersistPlayQueue();
    },
    { deep: false }
  );
  watch(currentPositionMs, (pos) => {
    const nowWall = Date.now();
    if (pos > 0 && nowWall - lastSavedPosWallMs >= 10000) {
      lastSavedPosWallMs = nowWall;
      try {
        localStorage.setItem("bm_saved_pos_ms", String(pos));
      } catch {
        // ignore storage quota error
      }
    }
  });

  async function handleLoginSuccess(
    newSession: KugouSession,
    newProfile?: AccountProfile | null
  ) {
    clearSessionPlayedSongs();
    clearAccountSwrCaches();
    clearDiscoverCaches();
    resetCycleForAuthChange();
    session.value = newSession;
    if (newProfile) profile.value = newProfile;
    isLoginModalOpen.value = false;
    notify(`欢迎回来，${newSession.nickname}！`);
    await refreshAccountAndVip(false);
    await loadDailyRecommend();
    loadSearchDiscovery(true);
  }

  function openAccountProfile(
    originEl?: HTMLElement | null,
    options?: {
      anchorMode?: "top-right" | "card-overlay";
      cardEl?: HTMLElement | null;
    }
  ) {
    if (!isLoggedIn.value) {
      isLoginModalOpen.value = true;
      return;
    }
    const anchorMode = options?.anchorMode || "top-right";
    let cardRect: AccountProfileOriginRect["cardRect"] = undefined;
    if (anchorMode === "card-overlay") {
      const cardNode =
        options?.cardEl ||
        (originEl?.closest(".bento-identity-card") as HTMLElement | null) ||
        null;
      if (cardNode) {
        const cr = cardNode.getBoundingClientRect();
        if (cr.width > 0 && cr.height > 0) {
          cardRect = {
            left: cr.left,
            top: cr.top,
            width: cr.width,
            height: cr.height,
          };
        }
      }
    }

    if (originEl) {
      const avatarTarget =
        (originEl.querySelector("img") as HTMLElement | null) || originEl;
      const rect = avatarTarget.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        accountProfileOrigin.value = {
          left: rect.left,
          top: rect.top,
          width: rect.width,
          height: rect.height,
          src: profile.value?.avatarUrl || session.value?.avatarUrl || "",
          anchorMode,
          cardRect,
        };
      } else if (cardRect) {
        accountProfileOrigin.value = {
          left: cardRect.left + 20,
          top: cardRect.top + 20,
          width: 48,
          height: 48,
          src: profile.value?.avatarUrl || session.value?.avatarUrl || "",
          anchorMode,
          cardRect,
        };
      } else {
        accountProfileOrigin.value = null;
      }
    } else {
      accountProfileOrigin.value = null;
    }
    isAccountProfileOpen.value = true;
  }

  async function logout() {
    await kugouClient.logoutAccount().catch(() => {});
    clearSessionPlayedSongs();
    clearAccountSwrCaches();
    clearDiscoverCaches();
    resetCycleForAuthChange();
    isAccountProfileOpen.value = false;
    accountProfileOrigin.value = null;
    session.value = null;
    profile.value = null;
    vipStatus.value = null;
    userPlaylists.value = [];
    playlistSongs.value = [];
    likedSongs.value = [];
    recentHistorySongs.value = [];
    followedArtists.value = [];
    purchasedAlbums.value = [];
    purchasedSongs.value = [];
    cloudDiskSongs.value = [];
    cloudDiskTotal.value = 0;
    cloudDiskUsedBytes.value = 0;
    cloudDiskMaxBytes.value = 0;
    cloudDiskHasMore.value = false;
    loadRecentHistory();
    notify("已退出酷狗概念版账号");
  }

  return {
    isStageScrolledDown,
    desktopLyricColor,
    desktopLyricColorScheme,
    desktopLyricSungMode,
    desktopLyricCustomPrimary,
    desktopLyricCustomSecondary,
    desktopLyricUnsungPreset,
    desktopLyricUnsungCustomHex,
    desktopLyricUnsungOpacity,
    desktopLyricStrokeWidth,
    desktopLyricStrokeColorMode,
    desktopLyricStrokeCustomHex,
    desktopLyricShadowEnabled,
    setDesktopLyricColorScheme,
    resetDesktopLyricPosition,
    snapDesktopLyricPosition,
    syncDesktopLyricFromNowPlaying,
    resetDesktopLyricSettingsToDefault,
    desktopLyricFontSize,
    desktopLyricWidth,
    desktopLyricX,
    desktopLyricY,
    desktopLyricShowTranslation,
    desktopLyricLayoutMode,
    desktopLyricLineMode,
    desktopLyricBgStyle,
    desktopLyricClarity,
    desktopLyricFontFamily,
    desktopLyricFontWeight,
    desktopLyricWordByWord,
    desktopLyricUnsungTone,
    desktopLyricLineSpacing,
    desktopLyricGlowIntensity,
    desktopLyricOpacity,
    desktopLyricIdleFade,
    desktopLyricShowIntro,
    desktopLyricShowPhonetic,
    activeHighResCoverUrl,
    activeHighResCoverMetrics,
    isNowPlayingLightStage,
    navHistoryStack,
    canNavigateBack,
    pushNavHistory,
    clearNavHistory,
    navigateBack,
    openSoundMixerFromPlayer,
    isCommentsModalOpen,
    commentsModalSong,
    openCommentsModal,
    closeCommentsModal,
    isMultiArtistModalOpen,
    multiArtistCandidates,
    multiArtistSongTitle,
    navigateToSongArtist,
    navigateToSongAlbum,
    openPlaylistDetail,
    openLikedSongsDetail,
    openRecentHistoryDetail,
    openPurchasedSongsDetail,
    openCloudDrivePage,
    openLocalMusicPage,
    syncListeningGradeCloud,
    refreshCurrentPlaylistOnly,
    updatePlaylistMeta,
    batchAddSongsToPlaylist,
    batchRemoveSongsFromPlaylist,
    artistSongsPage,
    artistSongsHasMore,
    isLoadingMoreArtistSongs,
    loadMoreArtistSongs,
    artistAlbumsPage,
    artistAlbumsSort,
    artistAlbumsHasMore,
    isLoadingMoreArtistAlbums,
    loadMoreArtistAlbums,
    switchArtistAlbumSort,
    focusTodayCompletedCount,
    focusTodayTotalMinutes,
    focusAllTimeCompletedCount,
    focusAllTimeTotalMinutes,
    ambientSyncWithMusic,
    recordCompletedFocusSession,
    focusPresetId,
    activeFocusPreset,
    focusMode,
    focusRunState,
    focusTargetMinutes,
    focusRemainingSec,
    focusElapsedSec,
    pomodoroFocusMinutes,
    pomodoroShortBreakMinutes,
    pomodoroLongBreakMinutes,
    pomodoroPhase,
    pomodoroCompletedRounds,
    autoPlaySceneMusic,
    autoMixAmbient,
    isFetchingSceneMusic,
    focusFormattedClock,
    focusProgressRatio,
    applyFocusScenePreset,
    switchFocusMode,
    setCustomCountdownMinutes,
    startFocusSession,
    pauseFocusSession,
    resumeFocusSession,
    toggleFocusRunPause,
    stopAndResetFocusSession,
    skipPomodoroPhase,
    setAutoPlaySceneMusic,
    setAutoMixAmbient,
    triggerSceneMusicPlayback,
    ambientEngine: ambientSoundEngine,
    skipExistingDownloaded: skipExistingFiles,
    hotKeywords: hotSearchKeywords,
    artistSongSort: artistSongsSort,
    switchArtistSongSort,
    eqBandTopologies: eqBandTypes,
    masterPreampDb,
    setBandFilterTopology,
    activeTab,
    settingsActiveSection,
    openSettingsSection,
    isSearchPaletteOpen,
    isNowPlayingOpen,
    isQueueOpen,
    queueAnchorRect,
    toggleNowPlayingQueue,
    isNowPlayingQueueOpen,
    isViperStudioModalOpen,
    isLoginModalOpen,
    isVipCenterOpen,
    isAccountProfileOpen,
    accountProfileOrigin,
    openAccountProfile,
    isAccountManagerOpen,
    isFocusPodOpen,
    isLyricPosterOpen,
    lyricPosterTargetSong,
    openLyricPosterModal,
    closeLyricPosterModal,
    desktopLyricEnabled,
    desktopLyricLocked,
    contextMenu,
    songDetailModalSong,
    activeSongDetailInfo,
    currentSongDetailInfo,
    isLoadingSongDetail,
    isLoadingCurrentSongDetail,
    currentSongAvailableQualities,
    getSongAvailableQualities,
    addToPlaylistModalSong,
    downloadModalSong,
    shareModalSong,
    shareModalPlaylist,
    detectedSharedTarget,
    openShareSongModal,
    openSharePlaylistModal,
    closeShareModal,
    dismissDetectedSharedTarget,
    checkClipboardForKugouShare,
    clipboardShareDetectEnabled,
    hdArtworkPreviewUrl,
    hdArtworkPreviewTarget,
    openHdArtworkPreview,
    closeHdArtworkPreview,
    statusBanner,
    discoverSubView,
    discoverHeroExpanding,
    isGlobalQueueOpen,
    activeRank,
    activeRankSongs,
    isLoadingRankSongs,
    isLoadingMoreRankSongs,
    isRankGridModalOpen,
    selectedRankCategory,
    fmChannels,
    selectedFmCategory,
    activeFmChannel,
    fmStationSongs,
    isLoadingFmStation,
    colorScheme,
    lightThemeVariant,
    lightThemeVariantMeta,
    LIGHT_THEME_VARIANT_OPTIONS,
    setLightThemeVariant,
    cycleLightThemeVariant,
    darkThemeVariant,
    darkThemeVariantMeta,
    DARK_THEME_VARIANT_OPTIONS,
    setDarkThemeVariant,
    cycleDarkThemeVariant,
    cycleCurrentModeVariant,
    resolvedColorScheme,
    isDarkTheme,
    isSyncingListeningGrade,
    motionLevel,
    uiDensity,
    setMotionLevel,
    setUiDensity,
    startupHomePage,
    resetPlaybackRateOnTrackChange,
    startupSafeVolumeEnabled,
    startupSafeVolumeLimit,
    autoClaimVipOnStartup,
    autoPlayOnStartup,
    autoResumeOnStartup: autoPlayOnStartup,
    closeToTray,
    miniYieldFullscreenEnabled,
    playerStageStyle,
    playerBgStyle,
    coverColorStageMode,
    cycleCoverColorStageMode,
    dynamicCoverAccentEnabled,
    coverPalette,
    effectiveCoverPalette,
    coverColorRoles,
    lyricTheme,
    lyricSungMode,
    lyricCustomPrimaryHex,
    lyricCustomSecondaryHex,
    lyricUnsungPreset,
    lyricUnsungCustomHex,
    lyricLetterSpacing,
    lyricFocusScaleRatio,
    lyricFontSize,
    lyricFontWeight,
    lyricTranslationScale,
    lyricLineSpacing,
    lyricAlignment,
    lyricBlurEnabled,
    lyricBlurIntensity,
    lyricBackgroundDim,
    lyricWordByWord,
    lyricWordBounceEnabled,
    lyricFocusScaleEnabled,
    lyricTranslationTintEnabled,
    lyricInterludeDotsEnabled,
    lyricClickSeekEnabled,
    lyricAutoReturnSec,
    lyricFontFamily,
    lyricGlowIntensity,
    lyricInactiveOpacity,
    lyricOffsetMs,
    bluetoothDelayMs,
    showTranslation,
    showPhonetic,
    resetLyricSettingsToDefault,
    activeCloudStreamEffect,
    switchCloudStreamEffect,
    climaxModeEnabled,
    ktvAccompanimentMode,
    replayGainEnabled,
    nightLoudnessEnabled,
    crossfadeSeconds,
    playPauseFadeEnabled,
    playbackRate,
    pitchSemitones,
    linkPitchWithSpeed,
    sleepTimerMinutes,
    sleepWaitCurrentSong,
    sleepRemainingSeconds,
    downloadFeatureUnlocked,
    downloadQuality,
    downloadMusicDir,
    downloadLrcDir,
    embedCoverAndTags,
    embedLyricsInAudio,
    exportStandaloneLrc,
    bilingualLrcExport,
    skipExistingFiles,
    downloadFileNaming,
    downloadFolderOrganize,
    downloadQueue,
    isSavingLyricOnly,
    downloadHistory,
    isDownloadingSong,
    typographyConfig,
    systemFonts,
    customFonts,
    customFontsDir,
    isScanningFonts,
    isImportingFonts,
    stageLyricFontOverride,
    desktopLyricFontOverride,
    cacheStats,
    session,
    savedAccounts,
    profile,
    vipStatus,
    isClaimingVip,
    gatewayDiag,
    isDiagnosingGateway,
    isLoggedIn,
    recommendSongs,
    isLoadingRecommend,
    personalFmSongs,
    personalFmIndex,
    isLoadingFm,
    isAppendingFmQueue,
    isPersonalFmDrawerOpen,
    currentFmSong,
    nextFmSong,
    newSongPeriod,
    newSongs,
    newSongsByPeriod,
    isLoadingNewSongs,
    isRefreshingAllNewSongs,
    rankList,
    isLoadingRanks,
    userPlaylists,
    selectedPlaylist,
    playlistSongs,
    likedSongs,
    recentHistorySongs,
    followedArtists,
    purchasedAlbums,
    purchasedSongs,
    libraryAssetTab,
    libraryDetailMode,
    isLibraryGridView,
    isLoadingLibrary,
    activeArtistDetail,
    activeArtistSongs,
    activeArtistAlbums,
    artistDetailSubTab,
    artistSongsSort,
    isLoadingArtist,
    activeAlbumDetail,
    activeAlbumSongs,
    isLoadingAlbum,
    createdPlaylists,
    collectedPlaylists,
    likedPlaylistMeta,
    createdOrderIds,
    collectedOrderIds,
    artistsOrderIds,
    purchasedOrderIds,
    searchKeyword,
    searchViewMode,
    searchSuggestions,
    searchCategoryTab,
    hotSearchKeywords,
    searchDiscovery,
    isLoadingSearchDiscovery,
    hotArtistCategory,
    isChangingHotArtistCat,
    searchHistory,
    searchResultsSongs,
    searchResultsPlaylists,
    searchResultsAlbums,
    searchResultsArtists,
    searchTotals,
    searchPages,
    searchHasMore,
    isLoadingMoreSearch,
    isSearching,
    playQueue,
    currentQueueTitle,
    recentPlayQueues,
    currentIndex,
    playMode,
    currentSong,
    isPlaying,
    isBuffering,
    lastTrackNavDir,
    currentPositionMs,
    durationMs,
    getLivePlaybackPositionMs,
    getEffectivePlaybackRate,
    getSongChorusWindowMs,
    toggleChorusQuickSkim,
    volume,
    playMomentum,
    preferredQuality,
    currentPlaybackInfo,
    currentLyric,
    isLoadingLyric,
    activeSongComments,
    activeSongCommentTotal,
    currentSongCommentTotal,
    setActiveSongCommentTotal,
    setCurrentSongCommentTotal,
    refreshCurrentSongCommentCount,
    commentSortType,
    isLoadingComments,
    expandedFloorCommentId,
    floorRepliesMap,
    isLoadingFloorReplies,
    masterEffectEnabled,
    eqBandCount,
    eqFrequencies,
    eqBandCenterHz,
    eqBandQFactors,
    eqGains,
    eqBandTypes,
    eqQFactor,
    eqHighPassHz,
    eqLowPassHz,
    spectralTiltDb,
    estimatedPeakHeadroomDb,
    channelBalance,
    speakerDelaySplitMs,
    channelWiringMode,
    phaseInvertL,
    phaseInvertR,
    antiClippingEnabled,
    activeEqPreset,
    activeTargetCurveId,
    peqBypass,
    peqMemorySlots,
    viperEffects,
    activeViperEffectId,
    acousticField,
    rhythmTelemetry,
    spectrumBars,
    notify,
    bootstrap,
    refreshAccountAndVip,
    switchAccount,
    removeSavedAccount,
    claimVipDaily,
    resignDeviceAndRefresh,
    runGatewayDiagnostic,
    loadDailyRecommend,
    loadPersonalFm,
    fetchMorePersonalFmForQueue,
    nextPersonalFmTrack,
    loadNewSongExpress,
    refreshAllNewSongTags,
    loadRankList,
    openRankDetail,
    refreshActiveRank,
    resolveFmChannelCovers,
    refreshDiscoverAll,
    openDailyRecommendPage,
    openGuessYouLikePage,
    openNewSongsPage,
    openRadioPlazaPage,
    playRankDirectly,
    loadFmChannels,
    loadFmStationSongs,
    loadRecentHistory,
    loadUserPlaylists,
    selectPlaylist,
    openResolvedSharedPlaylist,
    createNewPlaylist,
    deletePlaylist,
    removeSongFromCurrentPlaylist,
    importSongsAsNewPlaylist,
    importExternalPlaylist,
    isPlaylistCollected,
    toggleCollectPlaylist,
    addSongToUserPlaylist,
    isSongLiked,
    toggleFavoriteSong,
    loadFollowedArtists,
    toggleFollowArtist,
    loadPurchasedAssets,
    reorderLibraryAssetList,
    openArtistDetail,
    openAlbumDetail,
    loadSearchDiscovery,
    switchHotArtistCategory,
    openSearchLanding,
    executeSearch,
    loadMoreSearchCategory,
    clearSearchHistory,
    recordSongToRecentHistory,
    playSongFromList,
    togglePlayPause,
    toggleKtvAccompaniment,
    openSongDetailModal,
    playNext,
    playPrev,
    insertPlayNext,
    removeSongFromQueue,
    clearPlayQueue,
    moveQueueItem,
    sortPlayQueue,
    reorderQueueByMood,
    saveQueueSnapshot,
    touchActiveQueueLastPlayed,
    formatQueueSavedTimeText,
    switchToRecentQueue,
    removeRecentQueue,
    seekToMs,
    setVolume,
    setPlaybackSpeed,
    setPitchSemitones,
    applySpeedAndPitch,
    startSleepTimer,
    switchAudioQuality,
    setPlayMode,
    cyclePlayMode,
    syncDspChain,
    applyViperEffect,
    toggleMasterEffect,
    applyAudiophileTargetCurve,
    setBandQFactor,
    setBandCenterHz,
    resetAcousticAndEqToDefault,
    setEqPreset,
    updateSingleEqBand,
    setEqBandGainsBatch,
    setEqBandCount,
    smoothCurrentEqCurve,
    scaleCurrentEqGains,
    applySpectralTilt,
    savePeqToSlot,
    loadPeqFromSlot,
    resetSinglePeqSlot,
    resetAllPeqSlots,
    exportTunerProfileJson,
    importTunerProfileJson,
    loadCommentsForSong,
    toggleCommentFloorReplies,
    toggleLikeComment,
    submitSongComment,
    openSongContextMenu,
    closeSongContextMenu,
    triggerDownloadSong,
    saveSongLyricOnly,
    copySongLrcToClipboard,
    downloadConcurrency,
    autoSyncLocalAfterDownload,
    pauseDownloadTask,
    resumeDownloadTask,
    pauseAllDownloads,
    resumeAllDownloads,
    prioritizeDownloadTask,
    cancelDownloadTask,
    retryDownloadTask,
    retryAllFailedDownloads,
    clearCompletedDownloads,
    clearAllDownloads,
    removeDownloadQueueItem,
    playDownloadedLocalFile,
    redownloadHistoryItem,
    removeDownloadHistoryItem,
    batchDownloadSongs,
    batchAppendSongsToQueue,
    localMusicSongs,
    localMusicDir,
    localMusicSavedDirs,
    localMusicSettings,
    isScanningLocalMusic,
    addLocalMusicSavedDir,
    removeLocalMusicSavedDir,
    updateLocalMusicSettings,
    scanLocalMusicLibrary,
    rebuildLocalMusicCache,
    pickAndScanLocalMusicDir,
    cloudDiskSongs,
    cloudDiskTotal,
    cloudDiskUsedBytes,
    cloudDiskMaxBytes,
    cloudDiskHasMore,
    isLoadingCloudDisk,
    loadUserCloudDisk,
    loadMoreCloudDisk,
    deleteCloudDiskSongs,
    lyricCandidates,
    isSearchingLyricCandidates,
    activeLyricCandidateId,
    searchLyricCandidatesForCurrentSong,
    selectLyricCandidate,
    adjustLyricOffsetMs,
    isMiniPlayerMode,
    toggleMiniPlayerMode,
    audioOutputDevices,
    selectedAudioDeviceId,
    refreshAudioOutputDevices,
    selectAudioOutputDevice,
    appendSongToQueue,
    osGlobalHotkeysEnabled,
    autostartEnabled,
    isUpdatingAutostart,
    refreshAutostartState,
    setAutostartEnabled,
    isRecordingShortcut,
    globalHotkeys,
    inAppHotkeys,
    globalHotkeyFailedActions,
    syncGlobalHotkeysToOs,
    setGlobalHotkey,
    setInAppHotkey,
    resetGlobalHotkeys,
    resetInAppHotkeys,
    resetAllHotkeys,
    setCacheQuotaGb,
    openDirectoryInExplorer,
    revealFileInExplorer,
    clearDownloadHistory,
    browseDownloadMusicDir,
    browseDownloadLrcDir,
    refreshInstalledFonts,
    updateTypographyConfig,
    importCustomFontsFromPicker,
    removeCustomFont,
    resetTypographyConfig,
    trackReplayGainDb,
    measuredLufs,
    dynamicReplayGainDb,
    effectiveReplayGainDb,
    refreshCacheStats,
    clearCacheCategory,
    getSharedAudioContext,
    broadcastDesktopLyricState,
    broadcastDesktopLyricPosition,
    broadcastMiniPlayerState,
    broadcastMiniPlayerPosition,
    confirmDialog,
    askConfirm,
    resolveConfirmDialog,
    nowClockTick,
    lastFullRefreshAt,
    isBackgroundRefreshing,
    runHourlySilentRefresh,
    handleLoginSuccess,
    logout,
  };
});

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import PlumpIcon from "./PlumpIcon.vue";
import {
  TYPOGRAPHY_SYNC_CHANNEL,
  hydrateAndApplyTypography,
  loadPersistedTypographyConfig,
  normalizeTypographyConfig,
} from "../utils/fontRuntime";

type DockEdge = "none" | "top" | "bottom" | "left" | "right";
type PlayMode = "repeat_all" | "repeat_one" | "shuffle" | "sequence";

interface MiniLyricWord {
  startTimeMs: number;
  durationMs: number;
  text: string;
}

interface MiniLyricLine {
  startTimeMs: number;
  durationMs: number;
  text: string;
  translation?: string;
  phonetic?: string;
  words?: MiniLyricWord[];
}

interface MiniStateSyncPayload {
  type?: string;
  stateSeq?: number;
  songHash?: string;
  visible?: boolean;
  isPlaying?: boolean;
  positionMs?: number;
  durationMs?: number;
  playbackRate?: number;
  lyricOffsetMs?: number;
  bluetoothDelayMs?: number;
  volume?: number;
  playMode?: string;
  isLiked?: boolean;
  qualityBadge?: string;
  effectBadge?: string;
  desktopLyricEnabled?: boolean;
  resolvedColorScheme?: string;
  lightThemeVariant?: string;
  darkThemeVariant?: string;
  coverAccentPrimary?: string;
  coverAccentSecondary?: string;
  song?: {
    hash: string;
    title: string;
    artist: string;
    coverUrl: string;
    isVip?: boolean;
  } | null;
  lyric?: {
    isKrc?: boolean;
    hasTranslation?: boolean;
    hasPhonetic?: boolean;
    lines: MiniLyricLine[];
  } | null;
  sentAtMs?: number;
}

const props = withDefaults(
  defineProps<{
    standalone?: boolean;
    previewStore?: Record<string, any> | null;
  }>(),
  {
    standalone: false,
    previewStore: null,
  }
);

const MINI_DOCK_EDGE_STORAGE_KEY = "bm_mini_dock_edge_v1";
const MINI_SNAPSHOT_STORAGE_KEY = "bm_mini_player_snapshot_v1";
const MINI_POS_STORAGE_KEY = "bm_mini_player_pos_v1";

const isTauriRuntime =
  typeof window !== "undefined" &&
  ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

const isStandalone = computed(() => Boolean(props.standalone));
const store = computed<Record<string, any>>(() => props.previewStore || {});

function readSavedDockEdge(): DockEdge {
  try {
    const raw = localStorage.getItem(MINI_DOCK_EDGE_STORAGE_KEY);
    if (raw === "top" || raw === "bottom" || raw === "left" || raw === "right") {
      return raw;
    }
  } catch {
    // ignore storage errors
  }
  return "none";
}

function saveDockEdge(edge: DockEdge) {
  try {
    localStorage.setItem(MINI_DOCK_EDGE_STORAGE_KEY, edge);
  } catch {
    // ignore storage errors
  }
}

const dockEdge = ref<DockEdge>(isStandalone.value ? readSavedDockEdge() : "none");
const isIslandExpanded = ref<boolean>(dockEdge.value === "none");
const snapHintEdge = ref<DockEdge>("none");
const isPointerDragging = ref(false);
const isHoveringShell = ref(false);
const isVolumeOpen = ref(false);
const isScrubbingProgress = ref(false);
const scrubPosMs = ref(0);
const isDraggingVolume = ref(false);

// Custom glassmorphic tooltip state (replaces native browser title)
const miniTip = ref<{
  visible: boolean;
  text: string;
  x: number;
  y: number;
  place: "top" | "bottom";
}>({
  visible: false,
  text: "",
  x: 0,
  y: 0,
  place: "top",
});
const shellElRef = ref<HTMLElement | null>(null);

function showMiniTip(ev: MouseEvent | FocusEvent, text: string) {
  if (visualMode.value !== "card") {
    hideMiniTip();
    return;
  }
  if (!text || isPointerDragging.value) return;
  const target = ev.currentTarget as HTMLElement | null;
  const shellEl = shellElRef.value;
  if (!target || !shellEl) return;
  const tRect = target.getBoundingClientRect();
  const sRect = shellEl.getBoundingClientRect();
  const relCenterX = tRect.left - sRect.left + tRect.width / 2;
  const relTopY = tRect.top - sRect.top;
  const relBottomY = tRect.bottom - sRect.top;
  const place: "top" | "bottom" = relTopY < 36 ? "bottom" : "top";
  const clampedX = Math.max(44, Math.min(sRect.width - 44, relCenterX));
  miniTip.value = {
    visible: true,
    text,
    x: Math.round(clampedX),
    y: Math.round(place === "top" ? relTopY - 7 : relBottomY + 7),
    place,
  };
}

function hideMiniTip() {
  if (miniTip.value.visible) {
    miniTip.value.visible = false;
  }
}

// Standalone synced reactive state
const isMiniWindowVisible = ref<boolean>(true);
const remoteSnapshot = ref<MiniStateSyncPayload | null>(null);
const livePositionMs = ref(0);
let anchorPosMs = 0;
let anchorPerfMs = typeof performance !== "undefined" ? performance.now() : 0;
let lastStateSeq = -1;
let lastStateSentAtMs = 0;
let lastHandshakeReqAt = 0;
let rafId: number | null = null;
let collapseTimer: number | null = null;
let volumeHoverCloseTimer: number | null = null;
let optimisticTimer: number | null = null;
let handshakeTimers: number[] = [];
let lastExpandAtMs = 0;
let headerActionShieldUntil = 0;
let lastKnownVolume = 0.65;
let cmdCounter = 0;
const optimisticPlaying = ref<{ value: boolean; expiresAt: number } | null>(null);

// Cross-window listeners (only active when standalone)
let miniChannel: BroadcastChannel | null = null;
let miniTypoChannel: BroadcastChannel | null = null;
const unlisteners: UnlistenFn[] = [];

// Preview-mode free viewport positioning (browser fallback)
const previewCardPos = ref<{ x: number; y: number }>({
  x: typeof window !== "undefined" ? Math.max(20, window.innerWidth - 488) : 320,
  y: typeof window !== "undefined" ? Math.max(20, window.innerHeight - 246) : 240,
});
const previewDockRatio = ref(0.5);

// Offscreen Canvas text width measurement for zero-ellipsis marquee
let measureCtx: CanvasRenderingContext2D | null = null;
function measureTextPx(text: string, font: string): number {
  if (!text) return 0;
  if (typeof document !== "undefined") {
    if (!measureCtx) {
      const c = document.createElement("canvas");
      measureCtx = c.getContext("2d");
    }
    if (measureCtx) {
      measureCtx.font = font;
      return measureCtx.measureText(text).width;
    }
  }
  let units = 0;
  for (let i = 0; i < text.length; i++) {
    units += text.charCodeAt(i) <= 0x7f ? 0.56 : 1;
  }
  return units * 13;
}

function hydrateFromStorageSnapshots() {
  if (!isStandalone.value) return;
  try {
    const rawState = localStorage.getItem(MINI_SNAPSHOT_STORAGE_KEY);
    if (rawState) {
      const parsed = JSON.parse(rawState) as MiniStateSyncPayload;
      applySyncPayload(parsed, { fromStorage: true });
    }
    const rawPos = localStorage.getItem(MINI_POS_STORAGE_KEY);
    if (rawPos) {
      const parsedPos = JSON.parse(rawPos) as MiniStateSyncPayload;
      applySyncPayload(parsedPos, { fromStorage: true });
    }
  } catch {
    // ignore malformed snapshot
  }
}

function applyDocumentThemeClasses(scheme: string, lightVar: string, darkVar: string) {
  if (!isStandalone.value || typeof document === "undefined") return;
  const root = document.documentElement;
  root.dataset.theme = scheme === "watercolor_light" ? "watercolor_light" : "abyssal_dark";
  root.dataset.lightVariant = lightVar === "nordic_oat" ? "nordic_oat" : "watercolor";
  root.dataset.darkVariant = darkVar === "velvet_nebula" ? "velvet_nebula" : "obsidian_aurora";
}

function requestFullSync() {
  if (!isStandalone.value) return;
  dispatchMiniCommand({ type: "CMD_REQUEST_SYNC" });
  if (isTauriRuntime) {
    invoke<MiniStateSyncPayload | null>("pull_mini_player_sync_cmd")
      .then((snap) => {
        if (snap) applySyncPayload(snap);
      })
      .catch(() => {});
  }
}

function scheduleHandshakeWaves() {
  for (const t of handshakeTimers) {
    window.clearTimeout(t);
  }
  handshakeTimers = [];
  requestFullSync();
  handshakeTimers.push(window.setTimeout(() => requestFullSync(), 120));
  handshakeTimers.push(window.setTimeout(() => requestFullSync(), 340));
}

function reconcileOptimisticPlaying(authoritativePlaying: boolean) {
  if (!optimisticPlaying.value) return;
  if (
    authoritativePlaying === optimisticPlaying.value.value ||
    performance.now() >= optimisticPlaying.value.expiresAt
  ) {
    optimisticPlaying.value = null;
    if (optimisticTimer !== null) {
      window.clearTimeout(optimisticTimer);
      optimisticTimer = null;
    }
  }
}

function applySyncPayload(
  data: MiniStateSyncPayload | null | undefined,
  options?: { fromStorage?: boolean }
) {
  if (!data || typeof data !== "object") return;
  const fromStorage = Boolean(options?.fromStorage);

  if (data.type === "POSITION_SYNC") {
    const incomingHash = data.songHash ? String(data.songHash) : "";
    const curHash = remoteSnapshot.value?.song?.hash
      ? String(remoteSnapshot.value.song.hash)
      : "";
    if (incomingHash && curHash && incomingHash !== curHash) {
      const now = performance.now();
      if (!fromStorage && now - lastHandshakeReqAt > 320) {
        lastHandshakeReqAt = now;
        requestFullSync();
      }
    }

    const pos = Math.max(0, Number(data.positionMs) || 0);
    const rate = Number(data.playbackRate) || 1;
    const playing = Boolean(data.isPlaying);
    reconcileOptimisticPlaying(playing);
    const nowPerf = performance.now();
    const sentAt = Number(data.sentAtMs) || Date.now();
    const ageMs = playing ? Math.max(0, Math.min(260, Date.now() - sentAt)) : 0;
    const calibrated = pos + ageMs * rate;
    const delta = calibrated - livePositionMs.value;
    if (!playing || Math.abs(delta) > 280 || calibrated === 0) {
      anchorPosMs = calibrated;
      livePositionMs.value = calibrated;
    } else {
      anchorPosMs = delta >= -35 && delta <= 0 ? livePositionMs.value : calibrated;
      if (delta > 0 && !isScrubbingProgress.value) {
        livePositionMs.value = anchorPosMs;
      }
    }
    anchorPerfMs = nowPerf;
    if (remoteSnapshot.value) {
      remoteSnapshot.value = {
        ...remoteSnapshot.value,
        isPlaying: playing,
        durationMs:
          Number(data.durationMs) > 0
            ? Number(data.durationMs)
            : remoteSnapshot.value.durationMs,
        playbackRate: rate,
        lyricOffsetMs: Number(data.lyricOffsetMs ?? remoteSnapshot.value.lyricOffsetMs ?? 0),
        bluetoothDelayMs: Number(
          data.bluetoothDelayMs ?? remoteSnapshot.value.bluetoothDelayMs ?? 0
        ),
        volume:
          data.volume !== undefined ? Number(data.volume) : remoteSnapshot.value.volume,
      };
    }
    return;
  }

  // STATE_SYNC: guard against stale out-of-order packets without blocking new app sessions
  const seq = Number(data.stateSeq) || 0;
  const sentAt = Number(data.sentAtMs) || 0;
  const incomingHash = data.song?.hash || "";
  const currentHash = remoteSnapshot.value?.song?.hash || "";
  const isNewerTimestamp = sentAt > 0 && sentAt >= lastStateSentAtMs - 150;
  const isNewSessionReset = seq > 0 && seq <= 3;
  const isDifferentSong = Boolean(incomingHash && incomingHash !== currentHash);

  if (
    !fromStorage &&
    seq > 0 &&
    lastStateSeq > 0 &&
    seq < lastStateSeq &&
    !isNewerTimestamp &&
    !isNewSessionReset &&
    !isDifferentSong
  ) {
    return;
  }
  if (!fromStorage && seq > 0) {
    lastStateSeq = seq;
  }
  if (sentAt > lastStateSentAtMs) {
    lastStateSentAtMs = sentAt;
  }

  const pos = Math.max(0, Number(data.positionMs) || 0);
  const rate = Number(data.playbackRate) || 1;
  const playing = Boolean(data.isPlaying);
  reconcileOptimisticPlaying(playing);
  const ageMs = playing && sentAt > 0 ? Math.max(0, Math.min(260, Date.now() - sentAt)) : 0;
  anchorPosMs = pos + ageMs * rate;
  anchorPerfMs = performance.now();
  if (!isScrubbingProgress.value) {
    livePositionMs.value = anchorPosMs;
  }
  if (!fromStorage && typeof data.visible === "boolean") {
    isMiniWindowVisible.value = data.visible;
  }
  remoteSnapshot.value = data;
  applyDocumentThemeClasses(
    data.resolvedColorScheme || "abyssal_dark",
    data.lightThemeVariant || "watercolor",
    data.darkThemeVariant || "obsidian_aurora"
  );
  syncClockLoopDemand();
}

// Unified accessors (standalone snapshot vs direct store in browser preview)
const activeSong = computed(() => {
  if (isStandalone.value) {
    return remoteSnapshot.value?.song ?? null;
  }
  const s = store.value.currentSong;
  return s
    ? {
        hash: s.hash,
        title: s.title,
        artist: s.artist,
        coverUrl: s.coverUrl,
        isVip: Boolean(s.isVip),
      }
    : null;
});

const isPlaying = computed(() => {
  if (optimisticPlaying.value && performance.now() < optimisticPlaying.value.expiresAt) {
    return optimisticPlaying.value.value;
  }
  return isStandalone.value ? Boolean(remoteSnapshot.value?.isPlaying) : Boolean(store.value.isPlaying);
});

const durationMs = computed(() => {
  if (isStandalone.value) {
    return Math.max(0, Number(remoteSnapshot.value?.durationMs) || 0);
  }
  return Math.max(0, Number(store.value.durationMs || store.value.currentSong?.durationMs || 0));
});

const playbackRate = computed(() =>
  isStandalone.value
    ? Number(remoteSnapshot.value?.playbackRate) || 1
    : Number(store.value.playbackRate) || 1
);

const lyricOffsetMs = computed(() =>
  isStandalone.value
    ? Number(remoteSnapshot.value?.lyricOffsetMs) || 0
    : Number(store.value.lyricOffsetMs) || 0
);

const bluetoothDelayMs = computed(() =>
  isStandalone.value
    ? Number(remoteSnapshot.value?.bluetoothDelayMs) || 0
    : Number(store.value.bluetoothDelayMs) || 0
);

const currentVolume = computed(() => {
  const raw = isStandalone.value
    ? Number(remoteSnapshot.value?.volume ?? 0.72)
    : Number(store.value.volume ?? 0.72);
  const clamped = Math.max(0, Math.min(1, raw));
  if (clamped > 0.01) lastKnownVolume = clamped;
  return clamped;
});

const volumePercent = computed(() => Math.round(currentVolume.value * 100));

const playMode = computed<PlayMode>(() => {
  const m = isStandalone.value ? remoteSnapshot.value?.playMode : store.value.playMode;
  if (m === "repeat_one" || m === "shuffle" || m === "sequence") return m;
  return "repeat_all";
});

const isLiked = computed(() => {
  if (isStandalone.value) return Boolean(remoteSnapshot.value?.isLiked);
  const st = store.value;
  return st.currentSong && typeof st.isSongLiked === "function"
    ? st.isSongLiked(st.currentSong)
    : false;
});

const qualityBadge = computed(() => {
  if (isStandalone.value) return remoteSnapshot.value?.qualityBadge || "SQ";
  const st = store.value;
  if (st.currentPlaybackInfo?.qualityBadge) return st.currentPlaybackInfo.qualityBadge;
  const q =
    st.currentPlaybackInfo?.actualQuality ||
    st.currentPlaybackInfo?.matchedQuality ||
    st.preferredQuality;
  if (q === "hires" || q === "high") return "Hi-Res";
  if (q === "flac") return "SQ";
  if (q === "320") return "HQ";
  return "128K";
});

const effectBadge = computed(() => {
  if (isStandalone.value) {
    return remoteSnapshot.value?.effectBadge || "原声直通";
  }
  const st = store.value;
  if (!st.masterEffectEnabled) return "音效关闭";
  const vId = st.activeViperEffectId;
  if (vId && vId !== "off") {
    const matched = (st.viperEffects || []).find((i: any) => i.id === vId);
    if (matched) return matched.label || matched.name || "蝰蛇音效";
  }
  if (st.acousticField?.enabled) return "听感微调";
  if (st.activeEqPreset && st.activeEqPreset !== "flat") return "均衡调音";
  return "原声直通";
});

const desktopLyricEnabled = computed(() =>
  isStandalone.value
    ? Boolean(remoteSnapshot.value?.desktopLyricEnabled)
    : Boolean(store.value.desktopLyricEnabled)
);

const resolvedColorScheme = computed(() =>
  isStandalone.value
    ? remoteSnapshot.value?.resolvedColorScheme || "abyssal_dark"
    : store.value.resolvedColorScheme || "abyssal_dark"
);

const lightThemeVariant = computed(() =>
  isStandalone.value
    ? remoteSnapshot.value?.lightThemeVariant || "watercolor"
    : store.value.lightThemeVariant || "watercolor"
);

const darkThemeVariant = computed(() =>
  isStandalone.value
    ? remoteSnapshot.value?.darkThemeVariant || "obsidian_aurora"
    : store.value.darkThemeVariant || "obsidian_aurora"
);

const accentPrimary = computed(() => {
  if (isStandalone.value) {
    return remoteSnapshot.value?.coverAccentPrimary || "#7c6af6";
  }
  return store.value.effectiveCoverPalette?.dominantAccent || "#7c6af6";
});

const accentSecondary = computed(() => {
  if (isStandalone.value) {
    return remoteSnapshot.value?.coverAccentSecondary || "#2dd4bf";
  }
  return store.value.effectiveCoverPalette?.secondaryAccent || "#2dd4bf";
});

function hexToRgb(hex: string): [number, number, number] {
  const clean = (hex || "").trim().replace("#", "");
  if (clean.length === 3) {
    const r = parseInt(clean[0] + clean[0], 16);
    const g = parseInt(clean[1] + clean[1], 16);
    const b = parseInt(clean[2] + clean[2], 16);
    if (!Number.isNaN(r) && !Number.isNaN(g) && !Number.isNaN(b)) return [r, g, b];
  } else if (clean.length >= 6) {
    const r = parseInt(clean.slice(0, 2), 16);
    const g = parseInt(clean.slice(2, 4), 16);
    const b = parseInt(clean.slice(4, 6), 16);
    if (!Number.isNaN(r) && !Number.isNaN(g) && !Number.isNaN(b)) return [r, g, b];
  }
  return [124, 106, 246];
}

function relativeLuminance([r, g, b]: [number, number, number]): number {
  const toLinear = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  };
  return 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
}

const onAccentInk = computed(() => {
  const lum1 = relativeLuminance(hexToRgb(accentPrimary.value));
  const lum2 = relativeLuminance(hexToRgb(accentSecondary.value));
  const avgLum = (lum1 + lum2) * 0.5;
  return avgLum > 0.42 ? "#16151c" : "#ffffff";
});

const displayPosMs = computed(() => {
  if (isScrubbingProgress.value) return scrubPosMs.value;
  return livePositionMs.value;
});

const progressPercent = computed(() => {
  const dur = durationMs.value;
  if (dur <= 0) return 0;
  return Math.max(0, Math.min(100, (displayPosMs.value / dur) * 100));
});

const earRingCircumference = 2 * Math.PI * 16.5;
const earRingDashOffset = computed(() => {
  return earRingCircumference * (1 - progressPercent.value / 100);
});

const activeLyricInfo = computed(() => {
  const lines = isStandalone.value
    ? remoteSnapshot.value?.lyric?.lines ?? []
    : store.value.currentLyric?.lines ?? [];
  if (!lines.length) {
    if (activeSong.value) {
      return {
        lineKey: `fallback-${activeSong.value.hash}`,
        text: `${activeSong.value.title} · ${activeSong.value.artist || "未知歌手"}`,
        isPlaceholder: true,
        durationMs: 6500,
      };
    }
    return {
      lineKey: "idle",
      text: "等待点亮一段旋律…",
      isPlaceholder: true,
      durationMs: 6000,
    };
  }
  const effMs = Math.max(0, displayPosMs.value - lyricOffsetMs.value - bluetoothDelayMs.value);
  let matchedIdx = -1;
  for (let i = 0; i < lines.length; i++) {
    if (effMs >= lines[i].startTimeMs) {
      matchedIdx = i;
    } else {
      break;
    }
  }
  if (matchedIdx < 0) matchedIdx = 0;
  const line = lines[matchedIdx];
  const nextLine = lines[matchedIdx + 1];
  const rawDur = Number(line.durationMs) || 0;
  const lineDur =
    rawDur > 400
      ? rawDur
      : nextLine && nextLine.startTimeMs > line.startTimeMs
        ? Math.min(12000, nextLine.startTimeMs - line.startTimeMs)
        : 4200;

  return {
    lineKey: `${activeSong.value?.hash || "s"}-${matchedIdx}-${line.startTimeMs}`,
    text: line.text?.trim() || "♪ 间奏韵律",
    isPlaceholder: false,
    durationMs: Math.max(1800, lineDur),
  };
});

// Marquee overflow calculations (zero-ellipsis smooth scroll when text exceeds viewport)
const FONT_FAMILY_SPEC = '"PingFang SC", "Microsoft YaHei UI", "Segoe UI", sans-serif';

const pillLyricMarquee = computed(() => {
  const text = activeLyricInfo.value.text;
  const w = measureTextPx(text, `600 12px ${FONT_FAMILY_SPEC}`);
  const viewportW = 248;
  const overflow = Math.max(0, Math.ceil(w - viewportW));
  const durSec = Math.max(4.2, Math.min(14, Math.max(activeLyricInfo.value.durationMs / 1000, overflow / 32 + 2.2)));
  return {
    isOverflow: overflow > 3,
    style:
      overflow > 3
        ? {
            "--marquee-dist": `-${overflow + 10}px`,
            "--marquee-dur": `${durSec.toFixed(2)}s`,
          }
        : {},
  };
});

const cardTitleMarquee = computed(() => {
  const text = activeSong.value?.title || "微风音乐 BreezeMusic";
  const w = measureTextPx(text, `700 15px ${FONT_FAMILY_SPEC}`);
  const viewportW = 314;
  const overflow = Math.max(0, Math.ceil(w - viewportW));
  const durSec = Math.max(5.5, Math.min(16, overflow / 28 + 3));
  return {
    text,
    isOverflow: overflow > 3,
    style:
      overflow > 3
        ? {
            "--marquee-dist": `-${overflow + 12}px`,
            "--marquee-dur": `${durSec.toFixed(2)}s`,
          }
        : {},
  };
});

const cardArtistMarquee = computed(() => {
  const text = activeSong.value?.artist || "探索每一刻好音乐";
  const w = measureTextPx(text, `500 12px ${FONT_FAMILY_SPEC}`);
  const viewportW = 314;
  const overflow = Math.max(0, Math.ceil(w - viewportW));
  const durSec = Math.max(5.5, Math.min(16, overflow / 28 + 3));
  return {
    text,
    isOverflow: overflow > 3,
    style:
      overflow > 3
        ? {
            "--marquee-dist": `-${overflow + 12}px`,
            "--marquee-dur": `${durSec.toFixed(2)}s`,
          }
        : {},
  };
});

const cardLyricMarquee = computed(() => {
  const text = activeLyricInfo.value.text;
  const w = measureTextPx(text, `600 12px ${FONT_FAMILY_SPEC}`);
  const viewportW = 314;
  const overflow = Math.max(0, Math.ceil(w - viewportW));
  const durSec = Math.max(4.0, Math.min(14, Math.max(activeLyricInfo.value.durationMs / 1000, overflow / 34 + 2.0)));
  return {
    text,
    isOverflow: overflow > 3,
    style:
      overflow > 3
        ? {
            "--marquee-dist": `-${overflow + 12}px`,
            "--marquee-dur": `${durSec.toFixed(2)}s`,
          }
        : {},
  };
});

const visualMode = computed<"card" | "pill-h" | "ear-v">(() => {
  if (dockEdge.value === "none" || isIslandExpanded.value) return "card";
  if (dockEdge.value === "left" || dockEdge.value === "right") return "ear-v";
  return "pill-h";
});

const capsuleSubMode = computed<"pill-h" | "ear-v">(() => {
  if (dockEdge.value === "left" || dockEdge.value === "right") return "ear-v";
  return "pill-h";
});

const snapHintText = computed(() => {
  if (snapHintEdge.value === "top") return "松手贴边停靠顶部灵动岛";
  if (snapHintEdge.value === "bottom") return "松手贴边停靠底部灵动岛";
  if (snapHintEdge.value === "left") return "松手贴边停靠左侧黑胶岛";
  if (snapHintEdge.value === "right") return "松手贴边停靠右侧黑胶岛";
  return "";
});

const playModeTitle = computed(() => {
  if (playMode.value === "repeat_one") return "单曲循环";
  if (playMode.value === "shuffle") return "随机漫游";
  if (playMode.value === "sequence") return "顺序播放";
  return "列表循环";
});

function formatTimeMs(ms: number): string {
  const sec = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

const rootDynamicStyle = computed(() => {
  const [r1, g1, b1] = hexToRgb(accentPrimary.value);
  const [r2, g2, b2] = hexToRgb(accentSecondary.value);
  return {
    "--mini-accent-primary": accentPrimary.value,
    "--mini-accent-secondary": accentSecondary.value,
    "--mini-on-accent": onAccentInk.value,
    "--mini-accent-rgb": `${r1}, ${g1}, ${b1}`,
    "--mini-accent2-rgb": `${r2}, ${g2}, ${b2}`,
    "--mini-progress-pct": `${progressPercent.value.toFixed(2)}%`,
    "--mini-volume-pct": `${volumePercent.value}%`,
  } as Record<string, string>;
});

const previewShellStyle = computed(() => {
  if (isStandalone.value) return {};
  const vw = typeof window !== "undefined" ? window.innerWidth : 1280;
  const vh = typeof window !== "undefined" ? window.innerHeight : 760;
  const edge = dockEdge.value;
  const w = 480;
  const h = 228;
  if (edge === "none") {
    const maxX = Math.max(12, vw - w);
    const maxY = Math.max(12, vh - h);
    return {
      left: `${Math.max(0, Math.min(maxX, previewCardPos.value.x))}px`,
      top: `${Math.max(0, Math.min(maxY, previewCardPos.value.y))}px`,
      width: `${w}px`,
      height: `${h}px`,
    };
  }
  if (edge === "top") {
    const left = Math.round(
      Math.max(12, Math.min(vw - w - 12, previewDockRatio.value * Math.max(12, vw - w)))
    );
    return { left: `${left}px`, top: "0px", width: `${w}px`, height: `${h}px` };
  }
  if (edge === "bottom") {
    const left = Math.round(
      Math.max(12, Math.min(vw - w - 12, previewDockRatio.value * Math.max(12, vw - w)))
    );
    return {
      left: `${left}px`,
      top: `${Math.max(0, vh - h)}px`,
      width: `${w}px`,
      height: `${h}px`,
    };
  }
  if (edge === "left") {
    const top = Math.round(
      Math.max(12, Math.min(vh - h - 12, previewDockRatio.value * Math.max(12, vh - h)))
    );
    return { left: "0px", top: `${top}px`, width: `${w}px`, height: `${h}px` };
  }
  const top = Math.round(
    Math.max(12, Math.min(vh - h - 12, previewDockRatio.value * Math.max(12, vh - h)))
  );
  return {
    left: `${Math.max(0, vw - w)}px`,
    top: `${top}px`,
    width: `${w}px`,
    height: `${h}px`,
  };
});

// Single primary IPC command dispatch with unique cmdId (BroadcastChannel fallback in browser)
function dispatchMiniCommand(command: {
  type: string;
  positionMs?: number;
  volume?: number;
}) {
  if (!isStandalone.value) {
    const st = store.value;
    if (command.type === "CMD_TOGGLE_PLAY") st.togglePlayPause?.();
    else if (command.type === "CMD_PREV") st.playPrev?.();
    else if (command.type === "CMD_NEXT") st.playNext?.(false);
    else if (command.type === "CMD_SEEK_MS" && Number.isFinite(command.positionMs)) {
      st.seekToMs?.(Number(command.positionMs));
    } else if (command.type === "CMD_SET_VOLUME" && Number.isFinite(command.volume)) {
      st.setVolume?.(Number(command.volume));
    } else if (command.type === "CMD_CYCLE_PLAY_MODE") st.cyclePlayMode?.();
    else if (command.type === "CMD_TOGGLE_LIKE" && st.currentSong) {
      st.toggleFavoriteSong?.(st.currentSong);
    } else if (command.type === "CMD_TOGGLE_DESKTOP_LYRIC") {
      st.desktopLyricEnabled = !st.desktopLyricEnabled;
    } else if (command.type === "CMD_RESTORE_MAIN" || command.type === "CMD_HIDE_TO_TRAY") {
      st.isMiniPlayerMode = false;
    }
    return;
  }

  const enrichedCmd = {
    ...command,
    cmdId: `mini-${Date.now()}-${++cmdCounter}`,
  };
  if (isTauriRuntime) {
    invoke("send_mini_player_command_cmd", { command: enrichedCmd }).catch(() => {});
  } else if (miniChannel) {
    try {
      miniChannel.postMessage(enrichedCmd);
    } catch {
      // ignore channel error
    }
  }
}

function sendTogglePlay() {
  const nextPlaying = !isPlaying.value;
  if (isStandalone.value) {
    optimisticPlaying.value = {
      value: nextPlaying,
      expiresAt: performance.now() + 560,
    };
    if (optimisticTimer !== null) {
      window.clearTimeout(optimisticTimer);
    }
    optimisticTimer = window.setTimeout(() => {
      optimisticPlaying.value = null;
      optimisticTimer = null;
    }, 560);
  }
  dispatchMiniCommand({ type: "CMD_TOGGLE_PLAY" });
}
function sendPrev() {
  if (visualMode.value !== "card") return;
  dispatchMiniCommand({ type: "CMD_PREV" });
}
function sendNext() {
  if (visualMode.value !== "card") return;
  dispatchMiniCommand({ type: "CMD_NEXT" });
}
function sendCyclePlayMode() {
  if (visualMode.value !== "card") return;
  dispatchMiniCommand({ type: "CMD_CYCLE_PLAY_MODE" });
}
function sendToggleLike() {
  if (visualMode.value !== "card") return;
  dispatchMiniCommand({ type: "CMD_TOGGLE_LIKE" });
}
function sendToggleDesktopLyric() {
  if (visualMode.value !== "card") return;
  dispatchMiniCommand({ type: "CMD_TOGGLE_DESKTOP_LYRIC" });
}
function sendRestoreMain() {
  if (visualMode.value !== "card") return;
  hideMiniTip();
  dispatchMiniCommand({ type: "CMD_RESTORE_MAIN" });
}
function onCardActionClick(action: () => void) {
  if (visualMode.value !== "card") return;
  if (performance.now() < headerActionShieldUntil) return;
  action();
}

function onToggleMuteQuick() {
  const cur = currentVolume.value;
  if (cur > 0.005) {
    lastKnownVolume = cur;
    dispatchMiniCommand({ type: "CMD_SET_VOLUME", volume: 0 });
  } else {
    dispatchMiniCommand({ type: "CMD_SET_VOLUME", volume: Math.max(0.15, lastKnownVolume) });
  }
}

function onVolumeInput(ev: Event) {
  const target = ev.target as HTMLInputElement;
  const nextVal = Math.max(0, Math.min(100, Number(target.value) || 0)) / 100;
  dispatchMiniCommand({ type: "CMD_SET_VOLUME", volume: nextVal });
}

function onScrubInput(ev: Event) {
  const target = ev.target as HTMLInputElement;
  isScrubbingProgress.value = true;
  scrubPosMs.value = Math.max(0, Math.min(durationMs.value, Number(target.value) || 0));
}

function onScrubCommit(ev: Event) {
  const target = ev.target as HTMLInputElement;
  const targetMs = Math.max(0, Math.min(durationMs.value, Number(target.value) || 0));
  scrubPosMs.value = targetMs;
  anchorPosMs = targetMs;
  anchorPerfMs = performance.now();
  livePositionMs.value = targetMs;
  isScrubbingProgress.value = false;
  dispatchMiniCommand({ type: "CMD_SEEK_MS", positionMs: Math.round(targetMs) });
}

function onWheelAdjustVolume(ev: WheelEvent) {
  if (visualMode.value !== "card" && !isHoveringShell.value) return;
  ev.preventDefault();
  const delta = ev.deltaY < 0 ? 0.02 : -0.02;
  const nextVol = Math.max(0, Math.min(1, currentVolume.value + delta));
  dispatchMiniCommand({ type: "CMD_SET_VOLUME", volume: Number(nextVol.toFixed(2)) });
}

function openVolumeDrawer() {
  if (visualMode.value !== "card") return;
  if (volumeHoverCloseTimer !== null) {
    window.clearTimeout(volumeHoverCloseTimer);
    volumeHoverCloseTimer = null;
  }
  isVolumeOpen.value = true;
}

function scheduleCloseVolumeDrawer() {
  if (isDraggingVolume.value) return;
  if (volumeHoverCloseTimer !== null) {
    window.clearTimeout(volumeHoverCloseTimer);
  }
  volumeHoverCloseTimer = window.setTimeout(() => {
    isVolumeOpen.value = false;
    volumeHoverCloseTimer = null;
  }, 240);
}

// Expand / Collapse Dynamic Island with zero-resize 480x228 fluid choreography
async function expandDockIsland() {
  if (dockEdge.value === "none") return;
  hideMiniTip();
  if (collapseTimer !== null) {
    window.clearTimeout(collapseTimer);
    collapseTimer = null;
  }
  const guardNow = performance.now();
  lastExpandAtMs = guardNow;
  headerActionShieldUntil = guardNow + 440;
  isIslandExpanded.value = true;
  if (isStandalone.value && isTauriRuntime) {
    await invoke("expand_mini_player_from_dock_cmd", { edge: dockEdge.value }).catch(() => {});
    lastExpandAtMs = performance.now();
  }
}

function collapseDockIsland() {
  if (dockEdge.value === "none" || isPointerDragging.value) return;
  hideMiniTip();
  isVolumeOpen.value = false;
  isIslandExpanded.value = false;
  if (collapseTimer !== null) {
    window.clearTimeout(collapseTimer);
  }
  const targetEdge = dockEdge.value;
  collapseTimer = window.setTimeout(() => {
    collapseTimer = null;
    if (!isIslandExpanded.value && dockEdge.value === targetEdge && !isPointerDragging.value) {
      if (isStandalone.value && isTauriRuntime) {
        invoke("collapse_mini_player_to_dock_cmd", { edge: targetEdge }).catch(() => {});
      }
    }
  }, 630);
}

// Drag + 4-edge magnet engine
let dragActive = false;
let dragMoved = false;
let dragStartX = 0;
let dragStartY = 0;
let dragDockEdge: DockEdge = "none";
let dragExpanded = true;
let dragGrabX = 218;
let dragGrabY = 32;
let dragScaleFactor = 1;
let dragStepInFlight = false;
let dragStepPending = false;
let previewStartBoxX = 0;
let previewStartBoxY = 0;
let previewDockAtStart: DockEdge = "none";
let previewExpandedAtStart = true;

async function runTauriDragStep() {
  if (!dragActive || !dragMoved) return;
  if (dragStepInFlight) {
    dragStepPending = true;
    return;
  }
  dragStepInFlight = true;
  dragStepPending = false;
  try {
    const res = await invoke<{
      dockEdge: string;
      nearEdge: string;
      expanded: boolean;
      scaleFactor: number;
      winX: number;
      winY: number;
      winW: number;
      winH: number;
      grabOffsetX: number;
      grabOffsetY: number;
    } | null>("drag_mini_player_step_cmd", {
      grabOffsetX: Math.round(dragGrabX),
      grabOffsetY: Math.round(dragGrabY),
      dockEdge: dragDockEdge,
      expanded: dragExpanded,
    });
    if (res && dragActive) {
      dragDockEdge = (res.dockEdge as DockEdge) || "none";
      dragExpanded = Boolean(res.expanded);
      dragGrabX = res.grabOffsetX;
      dragGrabY = res.grabOffsetY;
      dragScaleFactor = res.scaleFactor || 1;
      dockEdge.value = dragDockEdge;
      isIslandExpanded.value = dragExpanded;
      snapHintEdge.value = (res.nearEdge as DockEdge) || "none";
    }
  } catch {
    // ignore transient drag IPC error
  } finally {
    dragStepInFlight = false;
    if (dragStepPending && dragActive) {
      requestAnimationFrame(() => {
        runTauriDragStep();
      });
    }
  }
}

function handlePreviewDragMove(ev: PointerEvent) {
  const dx = ev.clientX - dragStartX;
  const dy = ev.clientY - dragStartY;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const pullThresh = 30;
  const snapThresh = 22;

  if (previewDockAtStart !== "none" && !previewExpandedAtStart) {
    const pulledAway =
      (previewDockAtStart === "top" && ev.clientY > 36 + pullThresh) ||
      (previewDockAtStart === "bottom" && vh - ev.clientY > 36 + pullThresh) ||
      (previewDockAtStart === "left" && ev.clientX > 50 + pullThresh) ||
      (previewDockAtStart === "right" && vw - ev.clientX > 50 + pullThresh);

    if (pulledAway) {
      dockEdge.value = "none";
      isIslandExpanded.value = true;
      previewDockAtStart = "none";
      previewExpandedAtStart = true;
      previewStartBoxX = ev.clientX - 240;
      previewStartBoxY = ev.clientY - 38;
      dragStartX = ev.clientX;
      dragStartY = ev.clientY;
    } else {
      if (previewDockAtStart === "top" || previewDockAtStart === "bottom") {
        const w = 480;
        const maxLeft = Math.max(12, vw - w);
        const curLeft = Math.max(0, Math.min(maxLeft, ev.clientX - w / 2));
        previewDockRatio.value = curLeft / maxLeft;
      } else {
        const h = 228;
        const maxTop = Math.max(12, vh - h);
        const curTop = Math.max(0, Math.min(maxTop, ev.clientY - h / 2));
        previewDockRatio.value = curTop / maxTop;
      }
      snapHintEdge.value = "none";
      return;
    }
  }

  // Free drag card inside viewport
  if (dockEdge.value !== "none") {
    dockEdge.value = "none";
    isIslandExpanded.value = true;
  }
  const nextX = Math.max(-16, Math.min(vw - 464, previewStartBoxX + dx));
  const nextY = Math.max(-16, Math.min(vh - 212, previewStartBoxY + dy));
  previewCardPos.value = { x: nextX, y: nextY };

  const distTop = nextY + 22;
  const distBottom = vh - (nextY + 206);
  const distLeft = nextX + 22;
  const distRight = vw - (nextX + 458);

  const candidates: Array<[DockEdge, number]> = [
    ["top", distTop],
    ["bottom", distBottom],
    ["left", distLeft],
    ["right", distRight],
  ];
  let bestEdge: DockEdge = "none";
  let bestDist = snapThresh + 1;
  for (const [edge, dist] of candidates) {
    if (dist <= snapThresh && dist < bestDist) {
      bestDist = dist;
      bestEdge = edge;
    }
  }
  snapHintEdge.value = bestEdge;
}

async function onShellPointerDown(ev: PointerEvent) {
  if (ev.button !== 0) return;
  const targetEl = ev.target as HTMLElement | null;
  if (targetEl?.closest("button, input, a, [data-no-drag]")) {
    return;
  }
  hideMiniTip();
  if (collapseTimer !== null) {
    window.clearTimeout(collapseTimer);
    collapseTimer = null;
  }
  dragActive = true;
  dragMoved = false;
  dragStartX = ev.clientX;
  dragStartY = ev.clientY;
  dragDockEdge = dockEdge.value;
  dragExpanded = isIslandExpanded.value;
  snapHintEdge.value = "none";

  if (isStandalone.value && isTauriRuntime) {
    dragScaleFactor = window.devicePixelRatio || 1;
    dragGrabX = Math.round(ev.clientX * dragScaleFactor);
    dragGrabY = Math.round(ev.clientY * dragScaleFactor);
    invoke<{
      cursorX: number;
      cursorY: number;
      winX: number;
      winY: number;
      scaleFactor: number;
    } | null>("get_mini_player_drag_anchor_cmd")
      .then((anchor) => {
        if (anchor && dragActive) {
          dragScaleFactor = anchor.scaleFactor || 1;
          dragGrabX = anchor.cursorX - anchor.winX;
          dragGrabY = anchor.cursorY - anchor.winY;
        }
      })
      .catch(() => {});
  } else {
    previewDockAtStart = dockEdge.value;
    previewExpandedAtStart = isIslandExpanded.value;
    const styleBox = previewShellStyle.value as Record<string, string>;
    previewStartBoxX = parseFloat(styleBox.left || "200") || 200;
    previewStartBoxY = parseFloat(styleBox.top || "200") || 200;
  }

  window.addEventListener("pointermove", onWindowPointerMove);
  window.addEventListener("pointerup", onWindowPointerUp);
  window.addEventListener("pointercancel", onWindowPointerUp);
}

function onWindowPointerMove(ev: PointerEvent) {
  if (!dragActive) return;
  const dist = Math.hypot(ev.clientX - dragStartX, ev.clientY - dragStartY);
  if (!dragMoved && dist < 9) return;
  if (!dragMoved) {
    dragMoved = true;
    isPointerDragging.value = true;
    isVolumeOpen.value = false;
    hideMiniTip();
  }

  if (isStandalone.value && isTauriRuntime) {
    runTauriDragStep();
  } else {
    handlePreviewDragMove(ev);
  }
}

async function onWindowPointerUp() {
  window.removeEventListener("pointermove", onWindowPointerMove);
  window.removeEventListener("pointerup", onWindowPointerUp);
  window.removeEventListener("pointercancel", onWindowPointerUp);

  if (!dragActive) return;
  const wasMoved = dragMoved;
  const targetSnapEdge = snapHintEdge.value;
  dragActive = false;
  dragMoved = false;
  isPointerDragging.value = false;
  snapHintEdge.value = "none";

  if (!wasMoved) {
    // Pure tap/click: if docked & collapsed, spring-expand Dynamic Island
    if (dockEdge.value !== "none" && !isIslandExpanded.value) {
      await expandDockIsland();
    }
    return;
  }

  if (targetSnapEdge !== "none") {
    saveDockEdge(targetSnapEdge);
    dockEdge.value = targetSnapEdge;
    isIslandExpanded.value = false;
    if (isStandalone.value && isTauriRuntime) {
      if (collapseTimer !== null) window.clearTimeout(collapseTimer);
      invoke("snap_mini_player_to_edge_cmd", { edge: targetSnapEdge }).catch(() => {});
    } else {
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      if (targetSnapEdge === "top" || targetSnapEdge === "bottom") {
        previewDockRatio.value = Math.max(
          0,
          Math.min(1, previewCardPos.value.x / Math.max(1, vw - 480))
        );
      } else {
        previewDockRatio.value = Math.max(
          0,
          Math.min(1, previewCardPos.value.y / Math.max(1, vh - 228))
        );
      }
    }
  } else {
    saveDockEdge(dockEdge.value);
  }
}

function handleWindowBlur() {
  if (performance.now() - lastExpandAtMs < 640) return;
  if (isPointerDragging.value || isDraggingVolume.value || isScrubbingProgress.value) return;
  if (dockEdge.value !== "none" && isIslandExpanded.value) {
    collapseDockIsland();
  }
}

function handlePreviewStageClick(ev: MouseEvent) {
  if (isStandalone.value) return;
  const target = ev.target as HTMLElement | null;
  if (target?.closest(".bm-mini-shell")) return;
  if (dockEdge.value !== "none" && isIslandExpanded.value) {
    collapseDockIsland();
  }
}

function handleStorageEvent(ev: StorageEvent) {
  if (!isStandalone.value || !ev.newValue) return;
  if (ev.key === MINI_SNAPSHOT_STORAGE_KEY || ev.key === MINI_POS_STORAGE_KEY) {
    try {
      const parsed = JSON.parse(ev.newValue) as MiniStateSyncPayload;
      applySyncPayload(parsed, { fromStorage: true });
    } catch {
      // ignore parse errors
    }
  }
}

// Smooth 30fps RAF timer (active ONLY while window is visible & playing)
let lastMiniTickPerfMs = 0;

function shouldRunClockLoop(): boolean {
  return (
    isMiniWindowVisible.value &&
    isPlaying.value &&
    (typeof document === "undefined" || !document.hidden)
  );
}

function stopClockLoop() {
  if (rafId !== null) {
    cancelAnimationFrame(rafId);
    rafId = null;
  }
}

function tickLoop(now: number) {
  if (!shouldRunClockLoop()) {
    rafId = null;
    return;
  }
  if (!isScrubbingProgress.value && (lastMiniTickPerfMs === 0 || now - lastMiniTickPerfMs >= 33)) {
    lastMiniTickPerfMs = now;
    if (isStandalone.value) {
      const rawElapsed = Math.max(0, performance.now() - anchorPerfMs);
      const elapsed = Math.min(rawElapsed, 2500) * Math.max(0.25, playbackRate.value);
      const dur = durationMs.value;
      const next = anchorPosMs + elapsed;
      livePositionMs.value = dur > 0 ? Math.min(dur, next) : next;
    } else {
      const dur = durationMs.value;
      const st = store.value;
      const next = st.getLivePlaybackPositionMs?.() ?? st.currentPositionMs ?? 0;
      livePositionMs.value = dur > 0 ? Math.min(dur, next) : next;
    }
  }
  rafId = requestAnimationFrame(tickLoop);
}

function startClockLoop() {
  if (!shouldRunClockLoop() || rafId !== null) return;
  lastMiniTickPerfMs = 0;
  rafId = requestAnimationFrame(tickLoop);
}

function syncClockLoopDemand() {
  if (shouldRunClockLoop()) {
    startClockLoop();
  } else {
    stopClockLoop();
  }
}

watch([isMiniWindowVisible, isPlaying], () => {
  syncClockLoopDemand();
});

watch(visualMode, (mode) => {
  hideMiniTip();
  if (mode !== "card") {
    isVolumeOpen.value = false;
    if (volumeHoverCloseTimer !== null) {
      window.clearTimeout(volumeHoverCloseTimer);
      volumeHoverCloseTimer = null;
    }
  }
});

watch(
  () => [resolvedColorScheme.value, lightThemeVariant.value, darkThemeVariant.value],
  ([scheme, lightVar, darkVar]) => {
    applyDocumentThemeClasses(scheme, lightVar, darkVar);
  },
  { immediate: true }
);

onMounted(async () => {
  if (isStandalone.value && typeof document !== "undefined") {
    document.documentElement.classList.add("is-mini-player-win");
    document.body.classList.add("is-mini-player-win");
    hydrateFromStorageSnapshots();
    window.addEventListener("storage", handleStorageEvent);

    if (typeof BroadcastChannel !== "undefined") {
      miniChannel = new BroadcastChannel("bm_mini_player_channel");
      miniChannel.onmessage = (ev) => {
        if (ev.data?.type === "STATE_SYNC" || ev.data?.type === "POSITION_SYNC") {
          applySyncPayload(ev.data);
        }
      };
      miniTypoChannel = new BroadcastChannel(TYPOGRAPHY_SYNC_CHANNEL);
      miniTypoChannel.onmessage = (ev) => {
        if (ev.data?.type === "TYPOGRAPHY_UPDATED" && ev.data?.config) {
          hydrateAndApplyTypography(normalizeTypographyConfig(ev.data.config)).catch(() => {});
        }
      };
    }
    hydrateAndApplyTypography(loadPersistedTypographyConfig()).catch(() => {});

    if (isTauriRuntime) {
      const unlistenSync = await listen<MiniStateSyncPayload>("bm-mini-player-sync", (ev) => {
        applySyncPayload(ev.payload);
      }).catch(() => null);
      if (unlistenSync) unlisteners.push(unlistenSync);

      const unlistenVis = await listen<{ visible?: boolean }>(
        "bm-mini-player-visibility",
        (ev) => {
          const vis = Boolean(ev.payload?.visible);
          isMiniWindowVisible.value = vis;
          syncClockLoopDemand();
          if (vis) {
            scheduleHandshakeWaves();
            if (dockEdge.value !== "none" && !isIslandExpanded.value) {
              invoke("snap_mini_player_to_edge_cmd", { edge: dockEdge.value }).catch(() => {});
            } else if (dockEdge.value !== "none" && isIslandExpanded.value) {
              invoke("expand_mini_player_from_dock_cmd", { edge: dockEdge.value }).catch(() => {});
            }
          } else {
            isHoveringShell.value = false;
            hideMiniTip();
          }
        }
      ).catch(() => null);
      if (unlistenVis) unlisteners.push(unlistenVis);

      const unlistenLeave = await listen("bm-mini-island-cursor-leave", () => {
        isHoveringShell.value = false;
        hideMiniTip();
      }).catch(() => null);
      if (unlistenLeave) unlisteners.push(unlistenLeave);

      if (dockEdge.value !== "none" && !isIslandExpanded.value) {
        invoke("snap_mini_player_to_edge_cmd", { edge: dockEdge.value }).catch(() => {});
      } else if (dockEdge.value !== "none" && isIslandExpanded.value) {
        invoke("expand_mini_player_from_dock_cmd", { edge: dockEdge.value }).catch(() => {});
      }
    }

    dispatchMiniCommand({ type: "CMD_MINI_PLAYER_READY" });
    scheduleHandshakeWaves();
    window.addEventListener("blur", handleWindowBlur);
  } else {
    lastKnownVolume = Math.max(0.15, Number(store.value.volume ?? 0.65));
  }

  if (typeof document !== "undefined") {
    document.addEventListener("visibilitychange", syncClockLoopDemand);
  }
  syncClockLoopDemand();
});

onBeforeUnmount(() => {
  if (typeof document !== "undefined") {
    document.removeEventListener("visibilitychange", syncClockLoopDemand);
  }
  if (miniTypoChannel) {
    miniTypoChannel.close();
    miniTypoChannel = null;
  }
  stopClockLoop();
  if (collapseTimer !== null) window.clearTimeout(collapseTimer);
  if (volumeHoverCloseTimer !== null) window.clearTimeout(volumeHoverCloseTimer);
  if (optimisticTimer !== null) window.clearTimeout(optimisticTimer);
  for (const t of handshakeTimers) window.clearTimeout(t);
  window.removeEventListener("pointermove", onWindowPointerMove);
  window.removeEventListener("pointerup", onWindowPointerUp);
  window.removeEventListener("pointercancel", onWindowPointerUp);
  window.removeEventListener("blur", handleWindowBlur);
  window.removeEventListener("storage", handleStorageEvent);
  for (const fn of unlisteners) {
    try {
      fn();
    } catch {
      // ignore
    }
  }
  if (miniChannel) {
    try {
      miniChannel.close();
    } catch {
      // ignore
    }
    miniChannel = null;
  }
  if (isStandalone.value && typeof document !== "undefined") {
    document.documentElement.classList.remove("is-mini-player-win");
    document.body.classList.remove("is-mini-player-win");
  }
});
</script>

<template>
  <div
    class="bm-mini-stage"
    :class="[
      `scheme-${resolvedColorScheme}`,
      `light-variant-${lightThemeVariant}`,
      `dark-variant-${darkThemeVariant}`,
      `dock-${dockEdge}`,
      `mode-${visualMode}`,
      {
        'is-standalone': isStandalone,
        'is-preview': !isStandalone,
        'is-expanded': isIslandExpanded,
        'is-dragging': isPointerDragging,
        'is-playing': isPlaying,
      },
    ]"
    :style="rootDynamicStyle"
    @click="handlePreviewStageClick"
  >
    <div
      class="bm-mini-shell-anchor"
      :style="previewShellStyle"
      @pointerenter="isHoveringShell = true"
      @pointerleave="
        isHoveringShell = false;
        hideMiniTip();
      "
    >
      <div
        ref="shellElRef"
        class="bm-mini-shell"
        :class="[
          `shell-${visualMode}`,
          `edge-${dockEdge}`,
          { 'is-hovered': isHoveringShell, 'has-snap-hint': snapHintEdge !== 'none' },
          snapHintEdge !== 'none' ? `snap-preview-${snapHintEdge}` : '',
        ]"
        :style="{ cursor: isPointerDragging ? 'grabbing' : undefined }"
        @pointerdown="onShellPointerDown"
        @wheel="onWheelAdjustVolume"
      >
        <!-- Cover Dual-Accent Ambient Aura -->
        <div class="bm-mini-aura" aria-hidden="true"></div>

        <!-- Edge Magnet Glow Indicator when approaching screen edge -->
        <div
          v-if="snapHintEdge !== 'none'"
          class="bm-mini-snap-edge-glow"
          :class="`glow-${snapHintEdge}`"
          aria-hidden="true"
        ></div>

        <!-- ================================================================
             LAYER A: DOCKED CAPSULE LAYER (Cross-Dissolve Morph)
             ================================================================ -->
        <div
          class="bm-morph-layer bm-morph-layer-capsule"
          :class="{ 'is-layer-active': visualMode !== 'card' }"
          :aria-hidden="visualMode === 'card'"
          :inert="visualMode === 'card'"
        >
          <!-- A1: TOP / BOTTOM FLUSH NOTCH DYNAMIC ISLAND (332 x 34px) -->
          <div v-if="capsuleSubMode === 'pill-h'" class="bm-mini-pill-h">
            <div class="bm-pill-cover-ring" :class="{ spinning: isPlaying }">
              <img
                v-if="activeSong?.coverUrl"
                :src="activeSong.coverUrl"
                alt="封面"
                class="bm-pill-cover-img"
                draggable="false"
              />
              <div v-else class="bm-pill-cover-fallback">
                <PlumpIcon name="music-note" :size="12" color="var(--mini-on-accent)" />
              </div>
            </div>

            <div
              class="bm-pill-lyric-wrap"
              :class="{ 'is-scrolling': pillLyricMarquee.isOverflow }"
            >
              <span
                :key="activeLyricInfo.lineKey"
                class="bm-pill-lyric-text"
                :class="{
                  'is-placeholder': activeLyricInfo.isPlaceholder,
                  'is-marquee': pillLyricMarquee.isOverflow,
                }"
                :style="pillLyricMarquee.style"
              >
                {{ activeLyricInfo.text }}
              </span>
            </div>

            <div class="bm-pill-right-slot" data-no-drag>
              <div
                class="bm-pill-wave"
                :class="{ paused: !isPlaying, 'hide-on-hover': true }"
                aria-hidden="true"
              >
                <span></span>
                <span></span>
                <span></span>
              </div>
              <button
                type="button"
                class="bm-pill-quick-play"
                :aria-label="isPlaying ? '暂停' : '播放'"
                @click.stop="sendTogglePlay"
              >
                <PlumpIcon
                  :name="isPlaying ? 'pause' : 'play'"
                  :size="11"
                  color="var(--mini-on-accent)"
                  accent="var(--mini-on-accent)"
                />
              </button>
            </div>
          </div>

          <!-- A2: LEFT / RIGHT MICRO-FLOATING VINYL PICK (46 x 96px) -->
          <div v-else class="bm-mini-ear-v">
            <div class="bm-ear-vinyl-stage">
              <svg class="bm-ear-ring-svg" viewBox="0 0 38 38" aria-hidden="true">
                <circle class="bm-ear-ring-track" cx="19" cy="19" r="16.5" />
                <circle
                  class="bm-ear-ring-fill"
                  cx="19"
                  cy="19"
                  r="16.5"
                  :stroke-dasharray="earRingCircumference"
                  :stroke-dashoffset="earRingDashOffset"
                />
              </svg>
              <div class="bm-ear-vinyl-disc" :class="{ spinning: isPlaying }">
                <img
                  v-if="activeSong?.coverUrl"
                  :src="activeSong.coverUrl"
                  alt="黑胶封面"
                  class="bm-ear-vinyl-img"
                  draggable="false"
                />
                <div v-else class="bm-ear-vinyl-fallback">
                  <PlumpIcon name="music-note" :size="12" color="var(--mini-on-accent)" />
                </div>
                <span class="bm-ear-vinyl-spindle" aria-hidden="true"></span>
              </div>
            </div>

            <button
              type="button"
              class="bm-ear-play-btn"
              data-no-drag
              :aria-label="isPlaying ? '暂停' : '播放'"
              @click.stop="sendTogglePlay"
            >
              <PlumpIcon
                :name="isPlaying ? 'pause' : 'play'"
                :size="13"
                color="var(--mini-on-accent)"
                accent="var(--mini-on-accent)"
              />
            </button>
          </div>
        </div>

        <!-- ================================================================
             LAYER B: EXPANDED / FREE-FLOATING CARD (436 x 184px, Cross-Dissolve)
             ================================================================ -->
        <div
          class="bm-morph-layer bm-morph-layer-card"
          :class="{ 'is-layer-active': visualMode === 'card' }"
          :aria-hidden="visualMode !== 'card'"
          :inert="visualMode !== 'card'"
        >
          <div class="bm-mini-card">
            <!-- Row 1: Stealth Auto-Hide Top Bar (Quality + FX Badge & Window Actions) -->
            <div
              class="bm-card-header"
              :class="{
                'is-revealed':
                  visualMode === 'card' && (isHoveringShell || Boolean(snapHintText)),
              }"
            >
              <div class="bm-card-status-pills">
                <template v-if="snapHintText">
                  <span class="bm-card-snap-pill">{{ snapHintText }}</span>
                </template>
                <template v-else>
                  <span
                    class="bm-card-quality-tag"
                    :class="{ hires: qualityBadge === 'Hi-Res' }"
                  >
                    {{ qualityBadge }}
                  </span>
                  <span class="bm-card-effect-tag">
                    {{ effectBadge }}
                  </span>
                </template>
              </div>

              <div class="bm-card-top-actions" data-no-drag>
                <button
                  v-if="dockEdge !== 'none'"
                  type="button"
                  class="bm-card-icon-btn is-accent"
                  aria-label="收起为灵动岛"
                  @mouseenter="showMiniTip($event, '收起为灵动岛')"
                  @mouseleave="hideMiniTip"
                  @focus="showMiniTip($event, '收起为灵动岛')"
                  @blur="hideMiniTip"
                  @click.stop="onCardActionClick(collapseDockIsland)"
                >
                  <PlumpIcon
                    name="dock-collapse"
                    :size="13"
                    color="currentColor"
                    accent="var(--mini-accent-primary)"
                  />
                </button>
                <button
                  type="button"
                  class="bm-card-icon-btn"
                  :class="{ 'is-active': desktopLyricEnabled }"
                  aria-label="桌面歌词开关"
                  @mouseenter="
                    showMiniTip($event, desktopLyricEnabled ? '关闭桌面歌词' : '开启桌面歌词')
                  "
                  @mouseleave="hideMiniTip"
                  @focus="
                    showMiniTip($event, desktopLyricEnabled ? '关闭桌面歌词' : '开启桌面歌词')
                  "
                  @blur="hideMiniTip"
                  @click.stop="onCardActionClick(sendToggleDesktopLyric)"
                >
                  <PlumpIcon
                    name="lyric"
                    :size="13"
                    color="currentColor"
                    accent="var(--mini-accent-primary)"
                  />
                </button>
                <button
                  type="button"
                  class="bm-card-icon-btn is-close"
                  aria-label="退出 Mini 小窗并返回主界面"
                  @mouseenter="showMiniTip($event, '返回主窗口')"
                  @mouseleave="hideMiniTip"
                  @focus="showMiniTip($event, '返回主窗口')"
                  @blur="hideMiniTip"
                  @click.stop="onCardActionClick(sendRestoreMain)"
                >
                  <PlumpIcon name="close" :size="13" color="currentColor" />
                </button>
              </div>
            </div>

            <!-- Row 2: Pure Showcase Cover + 3-Line Marquee Track Meta -->
            <div class="bm-card-body">
              <div class="bm-card-cover-frame">
                <img
                  v-if="activeSong?.coverUrl"
                  :src="activeSong.coverUrl"
                  alt="歌曲封面"
                  class="bm-card-cover-img"
                  draggable="false"
                />
                <div v-else class="bm-card-cover-fallback">
                  <PlumpIcon name="music-note" :size="24" color="var(--mini-on-accent)" />
                </div>
                <span v-if="isPlaying" class="bm-card-cover-vinyl-dot" aria-hidden="true"></span>
              </div>

              <div class="bm-card-meta">
                <div
                  class="bm-card-line-wrap"
                  :class="{ 'is-scrolling': cardTitleMarquee.isOverflow }"
                >
                  <span
                    :key="`title-${activeSong?.hash || 'idle'}`"
                    class="bm-card-title"
                    :class="{ 'is-marquee': cardTitleMarquee.isOverflow }"
                    :style="cardTitleMarquee.style"
                  >
                    {{ cardTitleMarquee.text }}
                  </span>
                </div>

                <div
                  class="bm-card-line-wrap"
                  :class="{ 'is-scrolling': cardArtistMarquee.isOverflow }"
                >
                  <span
                    :key="`artist-${activeSong?.hash || 'idle'}`"
                    class="bm-card-artist"
                    :class="{ 'is-marquee': cardArtistMarquee.isOverflow }"
                    :style="cardArtistMarquee.style"
                  >
                    {{ cardArtistMarquee.text }}
                  </span>
                </div>

                <div
                  class="bm-card-line-wrap"
                  :class="{ 'is-scrolling': cardLyricMarquee.isOverflow }"
                >
                  <span
                    :key="activeLyricInfo.lineKey"
                    class="bm-card-lyric"
                    :class="{
                      'is-placeholder': activeLyricInfo.isPlaceholder,
                      'is-marquee': cardLyricMarquee.isOverflow,
                    }"
                    :style="cardLyricMarquee.style"
                  >
                    {{ cardLyricMarquee.text }}
                  </span>
                </div>
              </div>
            </div>

            <!-- Row 3: Scrubbable Progress + Timestamps -->
            <div class="bm-card-progress-row" data-no-drag>
              <span class="bm-time-text">{{ formatTimeMs(displayPosMs) }}</span>
              <div class="bm-progress-slider-wrap">
                <div class="bm-progress-track-visual" aria-hidden="true">
                  <div
                    class="bm-progress-fill-gpu"
                    :style="{
                      transform: `scaleX(${(progressPercent / 100).toFixed(5)})`,
                    }"
                  ></div>
                </div>
                <input
                  type="range"
                  class="bm-progress-range"
                  aria-label="播放进度"
                  min="0"
                  :max="Math.max(1000, durationMs)"
                  step="10"
                  :value="displayPosMs"
                  @input="onScrubInput"
                  @change="onScrubCommit"
                />
              </div>
              <span class="bm-time-text">{{ formatTimeMs(durationMs) }}</span>
            </div>

            <!-- Row 4: 5 Transport Controls + Expandable Inline Volume Bar -->
            <div
              class="bm-card-controls-row"
              :class="{ 'volume-expanded': isVolumeOpen }"
              data-no-drag
            >
              <div class="bm-transport-group">
                <button
                  type="button"
                  class="bm-ctrl-btn"
                  :aria-label="playModeTitle"
                  @mouseenter="showMiniTip($event, playModeTitle)"
                  @mouseleave="hideMiniTip"
                  @click.stop="sendCyclePlayMode"
                >
                  <PlumpIcon
                    :name="playMode"
                    :size="15"
                    color="currentColor"
                    accent="var(--mini-accent-secondary)"
                  />
                </button>

                <button
                  type="button"
                  class="bm-ctrl-btn"
                  aria-label="上一首"
                  @mouseenter="showMiniTip($event, '上一首')"
                  @mouseleave="hideMiniTip"
                  @click.stop="sendPrev"
                >
                  <PlumpIcon
                    name="prev"
                    :size="16"
                    color="currentColor"
                    accent="var(--mini-accent-secondary)"
                  />
                </button>

                <button
                  type="button"
                  class="bm-hero-play-btn"
                  :aria-label="isPlaying ? '暂停' : '播放'"
                  @mouseenter="showMiniTip($event, isPlaying ? '暂停' : '播放')"
                  @mouseleave="hideMiniTip"
                  @click.stop="sendTogglePlay"
                >
                  <PlumpIcon
                    :name="isPlaying ? 'pause' : 'play'"
                    :size="17"
                    color="var(--mini-on-accent)"
                    accent="var(--mini-on-accent)"
                  />
                </button>

                <button
                  type="button"
                  class="bm-ctrl-btn"
                  aria-label="下一首"
                  @mouseenter="showMiniTip($event, '下一首')"
                  @mouseleave="hideMiniTip"
                  @click.stop="sendNext"
                >
                  <PlumpIcon
                    name="next"
                    :size="16"
                    color="currentColor"
                    accent="var(--mini-accent-secondary)"
                  />
                </button>

                <button
                  type="button"
                  class="bm-ctrl-btn"
                  :class="{ 'is-liked': isLiked }"
                  :aria-label="isLiked ? '取消喜欢' : '加入我喜欢'"
                  @mouseenter="showMiniTip($event, isLiked ? '取消喜欢' : '加入喜欢')"
                  @mouseleave="hideMiniTip"
                  @click.stop="sendToggleLike"
                >
                  <PlumpIcon
                    :name="isLiked ? 'heart-filled' : 'heart'"
                    :size="15"
                    :color="isLiked ? '#ff5b7f' : 'currentColor'"
                    accent="#ff85a1"
                  />
                </button>
              </div>

              <div
                class="bm-inline-volume-wrap"
                @mouseenter="openVolumeDrawer"
                @mouseleave="scheduleCloseVolumeDrawer"
              >
                <div
                  class="bm-inline-volume-slider-box"
                  :class="{ 'is-open': isVolumeOpen }"
                  :aria-hidden="!isVolumeOpen"
                >
                  <input
                    type="range"
                    class="bm-volume-range"
                    aria-label="Mini 小窗音量"
                    :tabindex="isVolumeOpen ? 0 : -1"
                    min="0"
                    max="100"
                    step="1"
                    :value="volumePercent"
                    @pointerdown="isDraggingVolume = true"
                    @pointerup="isDraggingVolume = false"
                    @pointercancel="isDraggingVolume = false"
                    @input="onVolumeInput"
                  />
                  <span class="bm-volume-pct-text">{{ volumePercent }}%</span>
                </div>

                <button
                  type="button"
                  class="bm-ctrl-btn bm-vol-btn"
                  :class="{ 'is-active': isVolumeOpen }"
                  aria-label="音量开关与调节"
                  @click.stop="isVolumeOpen ? onToggleMuteQuick() : openVolumeDrawer()"
                >
                  <PlumpIcon
                    :name="volumePercent === 0 ? 'volume-mute' : 'volume'"
                    :size="15"
                    color="currentColor"
                    accent="var(--mini-accent-primary)"
                  />
                </button>
              </div>
            </div>
          </div>
        </div>

        <!-- Custom Glassmorphic Smart Tooltip (synced with main app design) -->
        <transition name="bm-tip-pop">
          <div
            v-if="miniTip.visible && miniTip.text"
            class="bm-mini-smart-tip"
            :class="`place-${miniTip.place}`"
            :style="{ left: `${miniTip.x}px`, top: `${miniTip.y}px` }"
            role="tooltip"
            aria-hidden="true"
          >
            {{ miniTip.text }}
          </div>
        </transition>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 4-theme surface palette (100% Zero-Border Architecture) */
.bm-mini-stage {
  --mini-bg: rgba(16, 16, 22, 0.94);
  --mini-bg-soft: rgba(255, 255, 255, 0.085);
  --mini-bg-soft-hover: rgba(255, 255, 255, 0.15);
  --mini-text: rgba(246, 247, 252, 0.96);
  --mini-text-sub: rgba(226, 230, 244, 0.7);
  --mini-text-dim: rgba(214, 220, 236, 0.5);
  --mini-track-bg: rgba(255, 255, 255, 0.14);
  --mini-tip-bg: rgba(24, 24, 34, 0.94);
  --mini-shadow: 0 6px 16px rgba(4, 4, 8, 0.32), 0 2px 5px rgba(4, 4, 8, 0.2);
  --mini-pill-shadow: 0 4px 12px rgba(4, 4, 8, 0.28), 0 1px 4px rgba(4, 4, 8, 0.18);
  position: relative;
  width: 100%;
  height: 100%;
  background: transparent;
  overflow: visible;
  user-select: none;
  font-family: "PingFang SC", "Microsoft YaHei UI", "Segoe UI", system-ui, sans-serif;
}

.bm-mini-stage.is-preview {
  position: fixed;
  inset: 0;
  z-index: 950;
  background: radial-gradient(circle at 50% 50%, rgba(18, 18, 24, 0.24), rgba(10, 10, 14, 0.42));
  backdrop-filter: blur(4px);
}

/* D2 Velvet Nebula */
.bm-mini-stage.scheme-abyssal_dark.dark-variant-velvet_nebula {
  --mini-bg: rgba(25, 19, 32, 0.94);
  --mini-bg-soft: rgba(244, 228, 255, 0.09);
  --mini-bg-soft-hover: rgba(244, 228, 255, 0.16);
  --mini-text: rgba(248, 242, 255, 0.96);
  --mini-text-sub: rgba(225, 206, 244, 0.72);
  --mini-text-dim: rgba(208, 188, 230, 0.52);
  --mini-track-bg: rgba(244, 228, 255, 0.15);
  --mini-tip-bg: rgba(32, 23, 42, 0.95);
  --mini-shadow: 0 6px 16px rgba(10, 4, 16, 0.34), 0 2px 5px rgba(14, 6, 22, 0.22);
  --mini-pill-shadow: 0 4px 12px rgba(10, 4, 16, 0.3), 0 1px 4px rgba(14, 6, 22, 0.18);
}

/* L1 Watercolor Light */
.bm-mini-stage.scheme-watercolor_light {
  --mini-bg: rgba(251, 248, 241, 0.96);
  --mini-bg-soft: rgba(65, 58, 48, 0.075);
  --mini-bg-soft-hover: rgba(65, 58, 48, 0.13);
  --mini-text: rgba(28, 27, 34, 0.95);
  --mini-text-sub: rgba(62, 60, 74, 0.74);
  --mini-text-dim: rgba(86, 82, 98, 0.56);
  --mini-track-bg: rgba(58, 52, 46, 0.13);
  --mini-tip-bg: rgba(252, 249, 244, 0.96);
  --mini-shadow: 0 6px 16px rgba(42, 34, 24, 0.14), 0 2px 5px rgba(42, 34, 24, 0.08);
  --mini-pill-shadow: 0 4px 12px rgba(42, 34, 24, 0.15), 0 1px 4px rgba(42, 34, 24, 0.08);
}

/* L2 Nordic Oat */
.bm-mini-stage.scheme-watercolor_light.light-variant-nordic_oat {
  --mini-bg: rgba(250, 245, 236, 0.97);
  --mini-bg-soft: rgba(82, 68, 48, 0.085);
  --mini-bg-soft-hover: rgba(82, 68, 48, 0.15);
  --mini-text: rgba(34, 29, 24, 0.96);
  --mini-text-sub: rgba(78, 68, 56, 0.76);
  --mini-text-dim: rgba(102, 90, 74, 0.58);
  --mini-track-bg: rgba(78, 66, 52, 0.14);
  --mini-tip-bg: rgba(248, 243, 233, 0.98);
  --mini-shadow: 0 6px 16px rgba(56, 44, 28, 0.14), 0 2px 5px rgba(56, 44, 28, 0.08);
  --mini-pill-shadow: 0 4px 12px rgba(56, 44, 28, 0.15), 0 1px 4px rgba(56, 44, 28, 0.08);
}

.bm-mini-stage.is-preview.scheme-watercolor_light {
  background: radial-gradient(
    circle at 50% 50%,
    rgba(245, 242, 235, 0.22),
    rgba(226, 218, 204, 0.44)
  );
}

.bm-mini-shell-anchor {
  position: relative;
  display: flex;
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  align-items: center;
  justify-content: center;
  padding: 0;
}

.bm-mini-stage.is-preview .bm-mini-shell-anchor {
  position: fixed;
  transition:
    left 520ms cubic-bezier(0.28, 0.94, 0.36, 1),
    top 520ms cubic-bezier(0.28, 0.94, 0.36, 1);
}

.bm-mini-stage.is-preview.is-dragging .bm-mini-shell-anchor {
  transition: none;
}

/* Fluid Morphing Shell (620ms damped silk curve inside constant 480x228 stage) */
.bm-mini-shell {
  --mini-fluid-ease: var(--ease-spring-tactile, cubic-bezier(0.28, 0.94, 0.36, 1));
  position: relative;
  overflow: hidden;
  border: none;
  background: var(--mini-bg);
  color: var(--mini-text);
  backdrop-filter: blur(28px) saturate(1.45);
  box-shadow: var(--mini-shadow);
  cursor: grab;
  transform: translate3d(0, 0, 0) scale(1);
  will-change: transform;
  transition:
    width 620ms var(--mini-fluid-ease),
    height 620ms var(--mini-fluid-ease),
    border-radius 600ms var(--mini-fluid-ease),
    transform 620ms var(--mini-fluid-ease),
    box-shadow 520ms ease;
}

.bm-mini-shell.shell-card {
  width: 436px;
  height: 184px;
  border-radius: 22px;
  transform: translate3d(0, 0, 0) scale(1);
}

/* Docked Expanded Card offsets (8px breathing room from attached screen edge) */
.bm-mini-shell.shell-card.edge-top {
  transform: translate3d(0, -14px, 0) scale(1);
}
.bm-mini-shell.shell-card.edge-bottom {
  transform: translate3d(0, 14px, 0) scale(1);
}
.bm-mini-shell.shell-card.edge-left {
  transform: translate3d(-14px, 0, 0) scale(1);
}
.bm-mini-shell.shell-card.edge-right {
  transform: translate3d(14px, 0, 0) scale(1);
}

/* Magnetic Pre-Snap Compression when dragging near screen edges */
.bm-mini-shell.shell-card.snap-preview-top {
  transform: translate3d(0, -10px, 0) scale(0.955, 0.92);
  border-radius: 14px 14px 22px 22px;
}
.bm-mini-shell.shell-card.snap-preview-bottom {
  transform: translate3d(0, 10px, 0) scale(0.955, 0.92);
  border-radius: 22px 22px 14px 14px;
}
.bm-mini-shell.shell-card.snap-preview-left {
  transform: translate3d(-10px, 0, 0) scale(0.93, 0.96);
}
.bm-mini-shell.shell-card.snap-preview-right {
  transform: translate3d(10px, 0, 0) scale(0.93, 0.96);
}

/* Top / Bottom Collapsed Dynamic Island Pill (332x34 flush to 0px screen edge) */
.bm-mini-shell.shell-pill-h {
  width: 332px;
  height: 34px;
  box-shadow: var(--mini-pill-shadow);
  cursor: pointer;
}

.bm-mini-shell.shell-pill-h.edge-top {
  border-radius: 0 0 18px 18px;
  transform: translate3d(0, -97px, 0) scale(1);
}

.bm-mini-shell.shell-pill-h.edge-bottom {
  border-radius: 18px 18px 0 0;
  transform: translate3d(0, 97px, 0) scale(1);
}

/* Left / Right Collapsed Micro-Floating Vinyl Pick (46x96 at 4px from screen edge) */
.bm-mini-shell.shell-ear-v {
  width: 46px;
  height: 96px;
  border-radius: 999px;
  box-shadow: var(--mini-pill-shadow);
  cursor: pointer;
}

.bm-mini-shell.shell-ear-v.edge-left {
  transform: translate3d(-213px, 0, 0) scale(1);
}

.bm-mini-shell.shell-ear-v.edge-right {
  transform: translate3d(213px, 0, 0) scale(1);
}

/* Dual-Layer Fixed-Topology Morph System (zero flex squashing during width/height morph) */
.bm-morph-layer {
  position: absolute;
  box-sizing: border-box;
  z-index: 1;
  opacity: 0;
  visibility: hidden;
  pointer-events: none;
  will-change: opacity, transform;
  transition:
    opacity 230ms cubic-bezier(0.32, 0, 0.6, 1),
    transform 460ms cubic-bezier(0.28, 0.94, 0.36, 1),
    visibility 0ms linear 230ms;
}

/* Hard-seal inactive morph layer and ALL descendants so child pointer-events:auto can never hijack hits */
.bm-morph-layer:not(.is-layer-active),
.bm-morph-layer:not(.is-layer-active) * {
  pointer-events: none !important;
}

/* Card layer locked to intrinsic 436x184 at (0,0) so children never squash or jump across edges */
.bm-morph-layer-card {
  width: 436px;
  height: 184px;
  top: 0;
  left: 0;
  transform-origin: 0 0;
}

.bm-morph-layer-card:not(.is-layer-active) {
  transform: translate3d(0px, -4px, 0) scale(0.76, 0.45);
}

.bm-mini-shell.edge-bottom .bm-morph-layer-card:not(.is-layer-active) {
  transform: translate3d(0px, -45px, 0) scale(0.76, 0.45);
}

.bm-mini-shell.edge-left .bm-morph-layer-card:not(.is-layer-active) {
  transform: translate3d(-4px, 0px, 0) scale(0.28, 0.52);
}

.bm-mini-shell.edge-right .bm-morph-layer-card:not(.is-layer-active) {
  transform: translate3d(-72px, 0px, 0) scale(0.28, 0.52);
}

.bm-morph-layer-card.is-layer-active {
  z-index: 3;
  opacity: 1;
  visibility: visible;
  pointer-events: auto;
  transform: translate3d(0, 0, 0) scale(1);
  transition:
    opacity 420ms cubic-bezier(0.25, 0.94, 0.35, 1) 105ms,
    transform 580ms cubic-bezier(0.28, 0.94, 0.36, 1) 25ms,
    visibility 0ms linear 0ms;
}

/* Capsule layer locked to intrinsic 332x34 (top/bottom) or 46x96 (left/right) at (0,0) */
.bm-morph-layer-capsule {
  width: 332px;
  height: 34px;
  top: 0;
  left: 0;
  transform-origin: 0 0;
}

.bm-morph-layer-capsule:not(.is-layer-active) {
  transform: translate3d(52px, 4px, 0) scale(1.06, 1.12);
}

.bm-mini-shell.edge-bottom .bm-morph-layer-capsule:not(.is-layer-active) {
  transform: translate3d(52px, 146px, 0) scale(1.06, 1.12);
}

.bm-mini-shell.edge-left .bm-morph-layer-capsule,
.bm-mini-shell.edge-right .bm-morph-layer-capsule {
  width: 46px;
  height: 96px;
}

.bm-mini-shell.edge-left .bm-morph-layer-capsule:not(.is-layer-active) {
  transform: translate3d(4px, 44px, 0) scale(1.12, 1.06);
}

.bm-mini-shell.edge-right .bm-morph-layer-capsule:not(.is-layer-active) {
  transform: translate3d(386px, 44px, 0) scale(1.12, 1.06);
}

.bm-morph-layer-capsule.is-layer-active {
  z-index: 3;
  opacity: 1;
  visibility: visible;
  pointer-events: auto;
  transform: translate3d(0, 0, 0) scale(1);
  transition:
    opacity 400ms cubic-bezier(0.25, 0.94, 0.35, 1) 120ms,
    transform 560ms cubic-bezier(0.28, 0.94, 0.36, 1) 35ms,
    visibility 0ms linear 0ms;
}

/* Dual-Accent Ambient Aura */
.bm-mini-aura {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    radial-gradient(circle at 14% 20%, rgba(var(--mini-accent-rgb), 0.2) 0%, transparent 56%),
    radial-gradient(circle at 88% 85%, rgba(var(--mini-accent2-rgb), 0.15) 0%, transparent 54%);
  z-index: 0;
}

.bm-mini-stage.scheme-watercolor_light .bm-mini-aura {
  background:
    radial-gradient(circle at 14% 20%, rgba(var(--mini-accent-rgb), 0.14) 0%, transparent 56%),
    radial-gradient(circle at 88% 85%, rgba(var(--mini-accent2-rgb), 0.11) 0%, transparent 54%);
}

/* Edge Magnet Snap Glow */
.bm-mini-snap-edge-glow {
  position: absolute;
  pointer-events: none;
  z-index: 5;
  background: linear-gradient(90deg, var(--mini-accent-primary), var(--mini-accent-secondary));
  opacity: 0.92;
  filter: blur(2px);
}
.bm-mini-snap-edge-glow.glow-top {
  top: 0;
  left: 24px;
  right: 24px;
  height: 4px;
  border-radius: 999px;
}
.bm-mini-snap-edge-glow.glow-bottom {
  bottom: 0;
  left: 24px;
  right: 24px;
  height: 4px;
  border-radius: 999px;
}
.bm-mini-snap-edge-glow.glow-left {
  left: 0;
  top: 18px;
  bottom: 18px;
  width: 4px;
  border-radius: 999px;
  background: linear-gradient(180deg, var(--mini-accent-primary), var(--mini-accent-secondary));
}
.bm-mini-snap-edge-glow.glow-right {
  right: 0;
  top: 18px;
  bottom: 18px;
  width: 4px;
  border-radius: 999px;
  background: linear-gradient(180deg, var(--mini-accent-primary), var(--mini-accent-secondary));
}

/* ============================================================
   MODE 1: TOP / BOTTOM FLUSH NOTCH DYNAMIC ISLAND (332 x 34)
   ============================================================ */
.bm-mini-pill-h {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  padding: 0 9px 0 8px;
  display: flex;
  align-items: center;
  gap: 8px;
  box-sizing: border-box;
}

.bm-pill-cover-ring {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  flex-shrink: 0;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, var(--mini-accent-primary), var(--mini-accent-secondary));
  animation: bmMiniSpin 12s linear infinite;
  animation-play-state: paused;
}

.bm-pill-cover-ring.spinning {
  animation-play-state: running;
}

.bm-pill-cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.bm-pill-cover-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.bm-pill-lyric-wrap {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  display: flex;
  align-items: center;
}

.bm-pill-lyric-wrap.is-scrolling {
  -webkit-mask-image: linear-gradient(90deg, transparent 0%, #000 5%, #000 94%, transparent 100%);
  mask-image: linear-gradient(90deg, transparent 0%, #000 5%, #000 94%, transparent 100%);
}

.bm-pill-lyric-text {
  display: inline-block;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.01em;
  white-space: nowrap;
  color: var(--mini-text);
}

.bm-pill-lyric-text.is-placeholder {
  color: var(--mini-text-sub);
  font-weight: 500;
}

.bm-pill-lyric-text.is-marquee {
  padding-left: 4px;
  animation: bmHorizontalMarquee var(--marquee-dur, 7s) ease-in-out infinite;
}

.bm-pill-right-slot {
  position: relative;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.bm-pill-wave {
  display: flex;
  align-items: flex-end;
  gap: 2.2px;
  height: 11px;
  transition: opacity 180ms ease, transform 180ms ease;
}

.bm-pill-wave span {
  width: 2.4px;
  border-radius: 999px;
  background: linear-gradient(180deg, var(--mini-accent-primary), var(--mini-accent-secondary));
  animation: bmWaveBounce 0.9s ease-in-out infinite;
}

.bm-pill-wave span:nth-child(1) {
  height: 6px;
  animation-delay: 0s;
}
.bm-pill-wave span:nth-child(2) {
  height: 11px;
  animation-delay: 0.22s;
}
.bm-pill-wave span:nth-child(3) {
  height: 7px;
  animation-delay: 0.44s;
}

.bm-pill-wave.paused span {
  animation-play-state: paused;
  height: 4px;
  opacity: 0.5;
}

.bm-pill-quick-play {
  position: absolute;
  inset: 0;
  border: none;
  border-radius: 50%;
  background: linear-gradient(135deg, var(--mini-accent-primary), var(--mini-accent-secondary));
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  transform: scale(0.82);
  cursor: pointer;
  transition: opacity 180ms ease, transform 200ms cubic-bezier(0.16, 1, 0.3, 1);
}

.bm-mini-shell.shell-pill-h:hover .bm-pill-wave.hide-on-hover {
  opacity: 0;
  transform: scale(0.8);
}

.bm-mini-shell.shell-pill-h:hover .bm-pill-quick-play {
  opacity: 1;
  transform: scale(1);
}

/* ============================================================
   MODE 2: LEFT / RIGHT MICRO-FLOATING VINYL PICK (46 x 96)
   ============================================================ */
.bm-mini-ear-v {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  padding: 5px 4px 6px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: space-between;
  box-sizing: border-box;
}

.bm-ear-vinyl-stage {
  position: relative;
  width: 38px;
  height: 38px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.bm-ear-ring-svg {
  position: absolute;
  inset: 0;
  width: 38px;
  height: 38px;
  transform: rotate(-90deg);
  pointer-events: none;
}

.bm-ear-ring-track {
  fill: none;
  stroke: var(--mini-track-bg);
  stroke-width: 2.2;
}

.bm-ear-ring-fill {
  fill: none;
  stroke: var(--mini-accent-primary);
  stroke-width: 2.3;
  stroke-linecap: round;
}

.bm-ear-vinyl-disc {
  position: relative;
  width: 29px;
  height: 29px;
  border-radius: 50%;
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, var(--mini-accent-primary), var(--mini-accent-secondary));
  animation: bmMiniSpin 12s linear infinite;
  animation-play-state: paused;
}

.bm-ear-vinyl-disc.spinning {
  animation-play-state: running;
}

.bm-ear-vinyl-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.bm-ear-vinyl-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.bm-ear-vinyl-spindle {
  position: absolute;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--mini-bg);
}

.bm-ear-play-btn {
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 50%;
  background: linear-gradient(135deg, var(--mini-accent-primary), var(--mini-accent-secondary));
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  box-shadow: 0 2px 8px rgba(var(--mini-accent-rgb), 0.28);
  transition: transform 200ms cubic-bezier(0.16, 1, 0.3, 1), filter 180ms ease;
}

.bm-ear-play-btn:hover {
  transform: scale(1.07);
  filter: brightness(1.08);
}

/* ============================================================
   MODE 3: FREE FLOATING & DOCK-EXPANDED ROUNDED CARD (436 x 184)
   ============================================================ */
.bm-mini-card {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  padding: 9px 16px 11px;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

/* Row 1: Stealth Top Bar (Invisible until card hover or edge snap) */
.bm-card-header {
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  opacity: 0;
  transform: translateY(-4px);
  pointer-events: none;
  transition:
    opacity 200ms cubic-bezier(0.22, 1, 0.36, 1),
    transform 220ms cubic-bezier(0.22, 1, 0.36, 1);
}

.bm-card-header.is-revealed {
  opacity: 1;
  transform: translateY(0);
  pointer-events: auto;
}

.bm-card-status-pills {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.bm-card-quality-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 2px 7px;
  border-radius: 999px;
  background: var(--mini-bg-soft);
  color: var(--mini-accent-primary);
  letter-spacing: 0.03em;
  white-space: nowrap;
}

.bm-card-quality-tag.hires {
  background: linear-gradient(135deg, rgba(245, 194, 92, 0.24), rgba(235, 148, 56, 0.2));
  color: #e8a735;
}

.bm-card-effect-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--mini-bg-soft);
  color: var(--mini-text-sub);
  white-space: nowrap;
}

.bm-card-snap-pill {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  padding: 2px 9px;
  border-radius: 999px;
  background: linear-gradient(135deg, var(--mini-accent-primary), var(--mini-accent-secondary));
  color: var(--mini-on-accent);
  white-space: nowrap;
}

.bm-card-top-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.bm-card-icon-btn {
  width: 23px;
  height: 23px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--mini-text-sub);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 140ms ease, color 140ms ease, transform 140ms ease;
}

.bm-card-icon-btn:hover {
  background: var(--mini-bg-soft-hover);
  color: var(--mini-text);
  transform: scale(1.06);
}

.bm-card-icon-btn.is-active,
.bm-card-icon-btn.is-accent {
  background: var(--mini-bg-soft);
  color: var(--mini-accent-primary);
}

.bm-card-icon-btn.is-close:hover {
  background: rgba(239, 68, 68, 0.22);
  color: #f87171;
}

/* Row 2: Pure Showcase Cover + 3-Line Marquee Meta */
.bm-card-body {
  display: flex;
  align-items: center;
  gap: 14px;
  min-height: 72px;
}

.bm-card-cover-frame {
  position: relative;
  width: 72px;
  height: 72px;
  border-radius: 15px;
  overflow: hidden;
  flex-shrink: 0;
  background: linear-gradient(135deg, var(--mini-accent-primary), var(--mini-accent-secondary));
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.22);
}

.bm-card-cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.bm-card-cover-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.bm-card-cover-vinyl-dot {
  position: absolute;
  right: 5px;
  bottom: 5px;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--mini-accent-secondary);
  box-shadow: 0 0 8px rgba(var(--mini-accent2-rgb), 0.9);
}

.bm-card-meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.bm-card-line-wrap {
  width: 100%;
  overflow: hidden;
  display: flex;
  align-items: center;
}

.bm-card-line-wrap.is-scrolling {
  -webkit-mask-image: linear-gradient(90deg, transparent 0%, #000 4%, #000 94%, transparent 100%);
  mask-image: linear-gradient(90deg, transparent 0%, #000 4%, #000 94%, transparent 100%);
}

.bm-card-title {
  display: inline-block;
  font-size: 15px;
  font-weight: 700;
  line-height: 1.26;
  color: var(--mini-text);
  white-space: nowrap;
}

.bm-card-artist {
  display: inline-block;
  font-size: 12px;
  font-weight: 500;
  line-height: 1.26;
  color: var(--mini-text-sub);
  white-space: nowrap;
}

.bm-card-lyric {
  display: inline-block;
  font-size: 12px;
  font-weight: 600;
  line-height: 1.3;
  margin-top: 2px;
  color: var(--mini-accent-primary);
  white-space: nowrap;
}

.bm-card-lyric.is-placeholder {
  color: var(--mini-text-dim);
  font-weight: 500;
}

.bm-card-title.is-marquee,
.bm-card-artist.is-marquee,
.bm-card-lyric.is-marquee {
  padding-left: 3px;
  animation: bmHorizontalMarquee var(--marquee-dur, 7.5s) ease-in-out infinite;
}

/* Row 3: Scrub Progress */
.bm-card-progress-row {
  display: flex;
  align-items: center;
  gap: 9px;
  height: 16px;
}

.bm-time-text {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--mini-text-dim);
  min-width: 32px;
}

.bm-time-text:last-child {
  text-align: right;
}

.bm-progress-slider-wrap {
  position: relative;
  flex: 1;
  height: 14px;
  display: flex;
  align-items: center;
}

.bm-progress-track-visual {
  position: absolute;
  left: 0;
  right: 0;
  top: 50%;
  height: 5px;
  border-radius: 999px;
  background: var(--mini-track-bg);
  overflow: hidden;
  transform: translateY(-50%);
  pointer-events: none;
  transition: height 140ms ease;
}

.bm-progress-slider-wrap:hover .bm-progress-track-visual {
  height: 7px;
}

.bm-progress-fill-gpu {
  width: 100%;
  height: 100%;
  border-radius: 999px;
  transform-origin: left center;
  will-change: transform;
  background: linear-gradient(
    90deg,
    var(--mini-accent-primary) 0%,
    var(--mini-accent-secondary) 100%
  );
}

.bm-progress-range {
  position: relative;
  z-index: 2;
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 14px;
  border: none;
  border-radius: 999px;
  background: transparent;
  cursor: pointer;
  outline: none;
}

.bm-progress-range::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 11px;
  height: 11px;
  border-radius: 50%;
  border: none;
  background: #ffffff;
  box-shadow: 0 1px 5px rgba(0, 0, 0, 0.36);
  opacity: 0;
  transform: scale(0.75);
  transition: opacity 140ms ease, transform 140ms ease;
}

.bm-progress-range:hover::-webkit-slider-thumb,
.bm-progress-range:focus-visible::-webkit-slider-thumb {
  opacity: 1;
  transform: scale(1);
}

.bm-progress-range::-moz-range-thumb {
  width: 11px;
  height: 11px;
  border-radius: 50%;
  border: none;
  background: #ffffff;
  box-shadow: 0 1px 5px rgba(0, 0, 0, 0.36);
}

/* Row 4: Controls & Inline Expandable Volume */
.bm-card-controls-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  height: 38px;
}

.bm-transport-group {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 14px;
  transition:
    gap 460ms cubic-bezier(0.26, 0.92, 0.36, 1),
    transform 460ms cubic-bezier(0.26, 0.92, 0.36, 1);
}

.bm-card-controls-row.volume-expanded .bm-transport-group {
  gap: 9px;
}

.bm-ctrl-btn {
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--mini-text-sub);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  transition: background 180ms ease, color 180ms ease, transform 200ms cubic-bezier(0.26, 0.92, 0.36, 1);
}

.bm-ctrl-btn:hover {
  background: var(--mini-bg-soft-hover);
  color: var(--mini-text);
  transform: scale(1.08);
}

.bm-ctrl-btn.is-active {
  background: var(--mini-bg-soft);
  color: var(--mini-accent-primary);
}

.bm-hero-play-btn {
  width: 38px;
  height: 38px;
  border: none;
  border-radius: 50%;
  background: linear-gradient(135deg, var(--mini-accent-primary), var(--mini-accent-secondary));
  color: var(--mini-on-accent);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  box-shadow: 0 4px 12px rgba(var(--mini-accent-rgb), 0.28);
  transition: transform 240ms cubic-bezier(0.26, 0.92, 0.36, 1), filter 180ms ease;
}

.bm-hero-play-btn:hover {
  transform: scale(1.08);
  filter: brightness(1.08);
}

.bm-inline-volume-wrap {
  display: flex;
  align-items: center;
  gap: 0px;
  flex-shrink: 0;
  padding: 2px;
  border-radius: 999px;
  background: transparent;
  transition:
    background-color 420ms cubic-bezier(0.26, 0.92, 0.36, 1),
    padding 460ms cubic-bezier(0.26, 0.92, 0.36, 1),
    gap 460ms cubic-bezier(0.26, 0.92, 0.36, 1);
}

.bm-card-controls-row.volume-expanded .bm-inline-volume-wrap {
  background: var(--mini-bg-soft);
  padding: 3px 4px 3px 11px;
  gap: 6px;
}

.bm-inline-volume-slider-box {
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 0px;
  opacity: 0;
  overflow: hidden;
  pointer-events: none;
  transform: translate3d(8px, 0, 0) scaleX(0.92);
  transform-origin: 100% 50%;
  transition:
    max-width 420ms cubic-bezier(0.26, 0.92, 0.36, 1),
    opacity 280ms cubic-bezier(0.26, 0.92, 0.36, 1),
    transform 420ms cubic-bezier(0.26, 0.92, 0.36, 1);
}

.bm-inline-volume-slider-box.is-open {
  max-width: 132px;
  opacity: 1;
  pointer-events: auto;
  transform: translate3d(0, 0, 0) scaleX(1);
  transition:
    max-width 460ms cubic-bezier(0.26, 0.92, 0.36, 1),
    opacity 360ms cubic-bezier(0.26, 0.92, 0.36, 1) 45ms,
    transform 460ms cubic-bezier(0.26, 0.92, 0.36, 1);
}

.bm-volume-range {
  -webkit-appearance: none;
  appearance: none;
  width: 92px;
  height: 8px;
  border: none;
  border-radius: 999px;
  background: linear-gradient(
    90deg,
    var(--mini-accent-primary) 0%,
    var(--mini-accent-secondary) var(--mini-volume-pct),
    var(--mini-track-bg) var(--mini-volume-pct),
    var(--mini-track-bg) 100%
  );
  cursor: pointer;
  outline: none;
}

.bm-volume-range::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: none;
  background: #ffffff;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.38);
}

.bm-volume-range::-moz-range-thumb {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: none;
  background: #ffffff;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.38);
}

.bm-volume-pct-text {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--mini-text);
  min-width: 30px;
  text-align: right;
}

/* Custom In-App Glassmorphic Smart Tooltip */
.bm-mini-smart-tip {
  position: absolute;
  z-index: 30;
  padding: 4px 10px;
  border-radius: 999px;
  border: none;
  background: var(--mini-tip-bg);
  color: var(--mini-text);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.01em;
  white-space: nowrap;
  pointer-events: none;
  backdrop-filter: blur(18px) saturate(1.4);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.24);
}

.bm-mini-smart-tip.place-top {
  transform: translate(-50%, -100%);
}

.bm-mini-smart-tip.place-bottom {
  transform: translate(-50%, 0);
}

.bm-tip-pop-enter-active,
.bm-tip-pop-leave-active {
  transition: opacity 140ms ease, transform 160ms cubic-bezier(0.22, 1, 0.36, 1);
}

.bm-tip-pop-enter-from,
.bm-tip-pop-leave-to {
  opacity: 0;
}

/* Focus rings strictly keyboard-only */
.bm-card-icon-btn:focus-visible,
.bm-ctrl-btn:focus-visible,
.bm-hero-play-btn:focus-visible,
.bm-pill-quick-play:focus-visible,
.bm-ear-play-btn:focus-visible,
.bm-progress-range:focus-visible,
.bm-volume-range:focus-visible {
  outline: 2px solid var(--mini-accent-primary);
  outline-offset: 2px;
}

@keyframes bmMiniSpin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@keyframes bmWaveBounce {
  0%,
  100% {
    transform: scaleY(0.5);
    opacity: 0.72;
  }
  50% {
    transform: scaleY(1);
    opacity: 1;
  }
}

@keyframes bmHorizontalMarquee {
  0%,
  12% {
    transform: translate3d(0, 0, 0);
  }
  82%,
  100% {
    transform: translate3d(var(--marquee-dist, -60px), 0, 0);
  }
}

@media (prefers-reduced-motion: reduce) {
  .bm-mini-shell,
  .bm-mini-shell-anchor,
  .bm-morph-layer,
  .bm-transport-group,
  .bm-inline-volume-wrap,
  .bm-inline-volume-slider-box,
  .bm-hero-play-btn,
  .bm-ctrl-btn,
  .bm-card-icon-btn,
  .bm-ear-play-btn {
    transition: none !important;
  }
  .bm-pill-cover-ring,
  .bm-ear-vinyl-disc,
  .bm-pill-wave span,
  .bm-pill-lyric-text.is-marquee,
  .bm-card-title.is-marquee,
  .bm-card-artist.is-marquee,
  .bm-card-lyric.is-marquee {
    animation: none !important;
  }
}
</style>

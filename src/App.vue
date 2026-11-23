<script setup lang="ts">
import {
  onMounted,
  onBeforeUnmount,
  computed,
  ref,
  watch,
  nextTick,
  defineAsyncComponent,
} from "vue";
import { useAppStore } from "./stores/appStore";
import TitleBar from "./components/TitleBar.vue";
import PlayerBar from "./components/PlayerBar.vue";
import LoginModal from "./components/LoginModal.vue";
import SongContextMenu from "./components/SongContextMenu.vue";
import MultiArtistModal from "./components/MultiArtistModal.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import InputContextMenu from "./components/InputContextMenu.vue";
import PlumpIcon from "./components/PlumpIcon.vue";
import MotionFxOverlay from "./components/MotionFxOverlay.vue";
import DiscoverView from "./views/DiscoverView.vue";
import NowPlayingView from "./views/NowPlayingView.vue";
import { installDesktopNativeGuard } from "./utils/desktopNativeGuard";
import { matchKeyboardEventAccelerator } from "./utils/shortcuts";
import {
  APPEARANCE_PREFS_SYNC_CHANNEL,
  applyMotionLevelToDocument,
  applyUiDensityToDocument,
  normalizeMotionLevel,
  normalizeUiDensity,
} from "./utils/motionRuntime";
import {
  ensureAppPowerVisibilityTracking,
  isAppForegroundActive,
  subscribeAppPowerVisibility,
} from "./utils/windowPowerState";

const FocusPodModal = defineAsyncComponent(
  () => import("./components/FocusPodModal.vue")
);
const VipCenterModal = defineAsyncComponent(
  () => import("./components/VipCenterModal.vue")
);
const AccountProfileModal = defineAsyncComponent(
  () => import("./components/AccountProfileModal.vue")
);
const AccountManagerModal = defineAsyncComponent(
  () => import("./components/AccountManagerModal.vue")
);
const LyricPosterModal = defineAsyncComponent(
  () => import("./components/LyricPosterModal.vue")
);
const CommentsModal = defineAsyncComponent(
  () => import("./components/CommentsModal.vue")
);
const HdArtworkPreviewModal = defineAsyncComponent(
  () => import("./components/HdArtworkPreviewModal.vue")
);
const AddToPlaylistModal = defineAsyncComponent(
  () => import("./components/AddToPlaylistModal.vue")
);
const ShareMusicModal = defineAsyncComponent(
  () => import("./components/ShareMusicModal.vue")
);
const DetectedShareLinkModal = defineAsyncComponent(
  () => import("./components/DetectedShareLinkModal.vue")
);
const LibraryView = defineAsyncComponent(
  () => import("./views/LibraryView.vue")
);
const RadioView = defineAsyncComponent(
  () => import("./views/RadioView.vue")
);
const SearchPlaceholderView = defineAsyncComponent(
  () => import("./views/SearchPlaceholderView.vue")
);
const SettingsPlaceholderView = defineAsyncComponent(
  () => import("./views/SettingsPlaceholderView.vue")
);
const PlaylistDetailView = defineAsyncComponent(
  () => import("./views/PlaylistDetailView.vue")
);
const ArtistDetailView = defineAsyncComponent(
  () => import("./views/ArtistDetailView.vue")
);
const AlbumDetailView = defineAsyncComponent(
  () => import("./views/AlbumDetailView.vue")
);
const CloudDriveView = defineAsyncComponent(
  () => import("./views/CloudDriveView.vue")
);
const LocalMusicView = defineAsyncComponent(
  () => import("./views/LocalMusicView.vue")
);

const DesktopLyricWindow = defineAsyncComponent(
  () => import("./components/DesktopLyricWindow.vue")
);
const MiniPlayerWindow = defineAsyncComponent(
  () => import("./components/MiniPlayerWindow.vue")
);
import NowPlayingQueuePopover from "./components/NowPlayingQueuePopover.vue";

const isStandaloneDesktopLyricWindow =
  typeof window !== "undefined" &&
  window.location.search.includes("window=desktop_lyric");

const isStandaloneMiniPlayerWindow =
  typeof window !== "undefined" &&
  window.location.search.includes("window=mini_player");

const isStandaloneSubWindow =
  isStandaloneDesktopLyricWindow || isStandaloneMiniPlayerWindow;

const isTauriRuntime =
  typeof window !== "undefined" &&
  ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

const store = useAppStore();
const stageViewportRef = ref<HTMLElement | null>(null);
const desktopLyricRef = ref<HTMLElement | null>(null);
const isDraggingDesktopLyric = ref(false);
let dragOffsetX = 0;
let dragOffsetY = 0;

function onDesktopLyricMouseDown(e: MouseEvent) {
  if (store.desktopLyricLocked) return;
  const target = e.target as HTMLElement | null;
  if (target && (target.closest("button") || target.closest("input"))) return;
  const el = desktopLyricRef.value;
  if (!el) return;
  const rect = el.getBoundingClientRect();
  isDraggingDesktopLyric.value = true;
  dragOffsetX = e.clientX - rect.left;
  dragOffsetY = e.clientY - rect.top;

  const onMove = (ev: MouseEvent) => {
    if (!isDraggingDesktopLyric.value) return;
    const width = store.desktopLyricWidth || rect.width || 440;
    const height = rect.height || 100;
    const maxX = Math.max(12, window.innerWidth - width - 12);
    const maxY = Math.max(12, window.innerHeight - height - 12);
    store.desktopLyricX = Math.min(maxX, Math.max(12, Math.round(ev.clientX - dragOffsetX)));
    store.desktopLyricY = Math.min(maxY, Math.max(12, Math.round(ev.clientY - dragOffsetY)));
  };

  const onUp = () => {
    isDraggingDesktopLyric.value = false;
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };

  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

const desktopLyricPositionStyle = computed(() => {
  const w = store.desktopLyricWidth || 440;
  const base: Record<string, string> = {
    "--fdl-color": store.desktopLyricColor || "#7c6af6",
    "--fdl-size": `${store.desktopLyricFontSize || 22}px`,
    width: `${w}px`,
    maxWidth: "calc(100vw - 24px)",
  };
  if (store.desktopLyricX !== null && store.desktopLyricY !== null) {
    base.left = `${store.desktopLyricX}px`;
    base.top = `${store.desktopLyricY}px`;
    base.bottom = "auto";
    base.transform = "none";
  }
  return base;
});


const calibratedPosMs = computed(() => {
  return (
    store.currentPositionMs + store.lyricOffsetMs - store.bluetoothDelayMs
  );
});

const currentLyricPair = computed(() => {
  const lines = store.currentLyric.lines;
  if (!lines.length) {
    return {
      main: store.currentSong?.title || "微风音乐 · 概念版无损直连",
      sub: store.currentSong?.artist || "耳畔微风起 · 帧享真无损",
      words: [],
      lineStartMs: 0,
    };
  }
  const pos = calibratedPosMs.value;
  let active = lines[0];
  let nextLine = lines[1];
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].startTimeMs <= pos) {
      active = lines[i];
      nextLine = lines[i + 1];
    } else {
      break;
    }
  }
  return {
    main: active?.text || "",
    sub:
      (store.showTranslation && active?.translation) ||
      (store.showPhonetic && active?.phonetic) ||
      nextLine?.text ||
      "",
    words: active?.words || [],
    lineStartMs: active?.startTimeMs || 0,
  };
});

function desktopWordFillPercent(
  lineStartMs: number,
  wordStartMs: number,
  wordDurMs: number
): number {
  const pos = calibratedPosMs.value;
  const absStart =
    wordStartMs >= lineStartMs ? wordStartMs : lineStartMs + wordStartMs;
  if (pos <= absStart) return 0;
  if (pos >= absStart + Math.max(80, wordDurMs)) return 100;
  return Math.min(100, ((pos - absStart) / Math.max(80, wordDurMs)) * 100);
}

const greetingParts = computed(() => {
  const h = new Date(store.nowClockTick).getHours();
  if (h >= 5 && h < 11) return { lead: "晨光微风", detail: "用清澈旋律唤醒耳朵" };
  if (h >= 11 && h < 14) return { lead: "午间小憩", detail: "漫游属于你的声学绿洲" };
  if (h >= 14 && h < 18) return { lead: "午后灵感", detail: "无损音质伴你专注时光" };
  if (h >= 18 && h < 23) return { lead: "夜幕微蓝", detail: "卸下喧嚣沉浸私人旋律" };
  return { lead: "深夜静谧", detail: "让温柔乐音陪你入眠" };
});

const isPrimaryGreetingVisible = computed(() => {
  if (store.activeTab === "discover") {
    return (
      store.discoverSubView !== "new_songs" &&
      store.discoverSubView !== "rank" &&
      store.discoverSubView !== "radio"
    );
  }
  return (
    store.activeTab === "search" ||
    store.activeTab === "library" ||
    store.activeTab === "tuner" ||
    store.activeTab === "radio" ||
    store.activeTab === "settings"
  );
});

// 与页面进出动画严密同步的顶部问候栏可见态：仅在旧页退场完成(@before-enter)时切换，彻底消除 60px 布局跳变
const displayedGreetingVisible = ref(true);
let greetingSyncTimer: ReturnType<typeof setTimeout> | null = null;

function syncGreetingVisibilityNow() {
  if (greetingSyncTimer) {
    clearTimeout(greetingSyncTimer);
    greetingSyncTimer = null;
  }
  displayedGreetingVisible.value = isPrimaryGreetingVisible.value;
}

watch(isPrimaryGreetingVisible, () => {
  if (greetingSyncTimer) clearTimeout(greetingSyncTimer);
  if (store.motionLevel === "none") {
    displayedGreetingVisible.value = isPrimaryGreetingVisible.value;
    greetingSyncTimer = null;
    return;
  }
  greetingSyncTimer = setTimeout(() => {
    displayedGreetingVisible.value = isPrimaryGreetingVisible.value;
    greetingSyncTimer = null;
  }, store.motionLevel === "standard" ? 110 : 240);
});

const stageScrollTop = ref(0);
let lastStageScrollTop = 0;
const savedViewScrollMap = new Map<string, number>();

function getStageScrollKey(): string {
  if (store.activeTab === "discover") {
    const sub = store.discoverSubView || "home";
    if (sub === "rank") {
      return `discover:rank:${store.activeRank?.rankId || ""}`;
    }
    return `discover:${sub}`;
  }
  if (store.activeTab === "search") {
    if (store.searchViewMode === "results") {
      return `search:results:${(store.searchKeyword || "").trim()}`;
    }
    return "search:landing";
  }
  if (store.activeTab === "playlist_detail") {
    return `playlist:${store.selectedPlaylist?.id || ""}`;
  }
  if (store.activeTab === "artist_detail") {
    return `artist:${store.activeArtistDetail?.artistId || store.activeArtistDetail?.name || ""}`;
  }
  if (store.activeTab === "album_detail") {
    return `album:${store.activeAlbumDetail?.albumId || store.activeAlbumDetail?.albumName || ""}`;
  }
  return store.activeTab;
}

let activeScrollViewKey = "discover:home";
let pendingTargetScrollTop: number | null = 0;
let isNavBackTransitionPending = false;
let navBackPendingScrollTop: number | null = null;

function handleNavBackRestoreEvent(e: Event) {
  const ce = e as CustomEvent<{ scrollTop?: number }>;
  isNavBackTransitionPending = true;
  navBackPendingScrollTop = Math.max(0, Number(ce.detail?.scrollTop) || 0);
}

function handleStageScroll(e: Event) {
  const el = e.target as HTMLElement | null;
  if (!el) return;
  const top = Math.max(0, el.scrollTop || 0);
  stageScrollTop.value = top;
  savedViewScrollMap.set(activeScrollViewKey, top);
  if (top <= 24) {
    store.isStageScrolledDown = false;
  } else if (top > lastStageScrollTop + 6 && top > 36) {
    store.isStageScrolledDown = true;
  } else if (top < lastStageScrollTop - 8) {
    store.isStageScrolledDown = false;
  }
  lastStageScrollTop = top;
}

function setStageViewportScrollTop(targetTop: number) {
  const safeTop = Math.max(0, Math.round(targetTop));
  stageScrollTop.value = safeTop;
  lastStageScrollTop = safeTop;
  store.isStageScrolledDown = safeTop > 36;
  if (stageViewportRef.value) {
    stageViewportRef.value.scrollTop = safeTop;
  }
  const el = document.querySelector(".skyline-stage-viewport") as HTMLElement | null;
  if (el) {
    el.scrollTop = safeTop;
  }
}

function handleStageBeforeEnter() {
  syncGreetingVisibilityNow();
  const nextKey = getStageScrollKey();
  activeScrollViewKey = nextKey;
  const targetTop =
    pendingTargetScrollTop !== null
      ? pendingTargetScrollTop
      : savedViewScrollMap.get(nextKey) ?? 0;
  pendingTargetScrollTop = null;
  setStageViewportScrollTop(targetTop);
  nextTick(() => {
    setStageViewportScrollTop(targetTop);
    scheduleRefreshAllTabGliders();
  });
}

function resetStageViewportScroll() {
  handleStageBeforeEnter();
}

// 三级语义化路由切换过渡方向调度器
const TAB_ORDER: Record<string, number> = {
  discover: 0,
  search: 1,
  library: 2,
  tuner: 3,
  radio: 3,
  settings: 4,
};
const activeViewTransitionName = ref("view-slide-left");
const effectiveViewTransitionName = computed(() => {
  if (store.motionLevel === "none") return "";
  if (store.motionLevel === "standard") return "view-fade-fast";
  return activeViewTransitionName.value;
});

watch(
  () => store.activeTab,
  (newTab, oldTab) => {
    hideSmartTooltip(true);
    const isDetailNew =
      newTab === "playlist_detail" ||
      newTab === "album_detail" ||
      newTab === "artist_detail" ||
      newTab === "cloud_drive" ||
      newTab === "local_music";
    const isDetailOld =
      oldTab === "playlist_detail" ||
      oldTab === "album_detail" ||
      oldTab === "artist_detail" ||
      oldTab === "cloud_drive" ||
      oldTab === "local_music";
    if (isNavBackTransitionPending) {
      isNavBackTransitionPending = false;
      const targetKey = getStageScrollKey();
      pendingTargetScrollTop =
        navBackPendingScrollTop !== null
          ? navBackPendingScrollTop
          : savedViewScrollMap.get(targetKey) ?? 0;
      navBackPendingScrollTop = null;
      activeViewTransitionName.value =
        isDetailNew || isDetailOld ? "view-depth-pop" : "view-slide-right";
      return;
    }
    if (isDetailNew && !isDetailOld) {
      pendingTargetScrollTop = 0;
      activeViewTransitionName.value = "view-depth-push";
      return;
    }
    if (!isDetailNew && isDetailOld) {
      const targetKey = getStageScrollKey();
      pendingTargetScrollTop = savedViewScrollMap.get(targetKey) ?? 0;
      activeViewTransitionName.value = "view-depth-pop";
      return;
    }
    if (isDetailNew && isDetailOld) {
      pendingTargetScrollTop = 0;
      activeViewTransitionName.value = "view-depth-push";
      return;
    }
    pendingTargetScrollTop = 0;
    const newIdx = TAB_ORDER[newTab] ?? 0;
    const oldIdx = TAB_ORDER[oldTab] ?? 0;
    activeViewTransitionName.value =
      newIdx >= oldIdx ? "view-slide-left" : "view-slide-right";
  }
);

// ============================================================================
// 全局防截断、无原生矩形框的智能毛玻璃悬浮提示引擎 v3 (Global Smart Glass Tooltip)
// ============================================================================
const smartTooltipRef = ref<HTMLElement | null>(null);
const smartTip = ref({
  visible: false,
  text: "",
  x: 0,
  y: 0,
  placement: "top" as "top" | "bottom",
  gliding: false,
  multiline: false,
});
let activeTooltipTarget: HTMLElement | null = null;
let tooltipRafId: number | null = null;
let tooltipHoverTimer: ReturnType<typeof setTimeout> | null = null;
let lastTooltipHideTime = 0;
let domMutationObserver: MutationObserver | null = null;

const INTERACTIVE_TIP_SELECTOR =
  'button, a, [role="button"], input, select, [data-tip], [data-raw-title], .bento-listen-box, .clickable-meta-link, .inline-meta-btn, .inline-meta-jump, .np-cover-stage-box, .cover-wrap, .track-cover-wrap, .song-cover-wrap, .sl-cover-wrap, .artist-avatar-wrap, .artist-name-span, .clickable, .pill-badge, .q-tag, .s-badge, .mini-badge, .fs-badge, .pb-cover-frame, .pb-song-artist, .pb-quality-pill, .np-qf-idx, .sam-card, .spw-band-pill, .fdl-song-chip, .mini-cover-wrap';

const AUTO_ELLIPSIS_TEXT_SELECTOR =
  '.ns-song-title, .ns-song-artist, .fs-title, .fs-sub-line, .rcc-song-name, .rcc-song-artist, .rcc-track-main, .rcc-title, .rcc-sub-text, .rc-name, .rc-desc, .lib-pl-title, .lib-pl-sub, .lib-artist-name, .roc-title, .roc-top1-preview, .rp-msc-title, .rp-msc-desc, .rp-preview-song-title, .rp-preview-song-artist, .rp-console-title, .rp-console-desc, .row-song-name, .row-artist-sub, .shsr-title, .shsr-sub, .psc-title, .psc-sub, .pb-song-title, .pb-song-artist, .mini-song-title, .mini-song-artist, .mini-lyric-line, .lgtp-title, .lgtp-artist, .qi-title, .qi-artist, .sdp-title, .sdp-subtitle, .sdp-desc, .sdp-track-title, .sdp-track-sub, .sr-song-title, .sr-song-sub, .sr-media-title, .sr-media-sub, .aer-title, .aer-sub, .artist-short-intro, .preview-song, .preview-artist, .share-song-title, .share-song-artist, .shac-name';

const TAB_BUTTON_IGNORE_REDUNDANT_SELECTOR =
  ".app-sliding-tab-btn, .jelly-pill-tab-btn, .lib-tab-btn, .pill-tab-btn, .sr-tab-btn, .sl-region-pill, .sl-board-tab, .rom-cat-tab, .rom-cat-pill, .rp-cat-pill, .rp-category-pill, .rank-range-pill, .rank-cat-pill, .rpc-cat-pill, .lmv-sort-pill, .tuner-preset-pill, .nq-sort-pill, .vas-mode-pill, .vas-cat-btn, .vas-xo-chip, .nfw-seg-pill, .nfw-seg-option, .cm-sort-btn, .cm-owner-filter-btn, .zen-preset-pill, .zen-scene-pill, .dl-seg-btn, .ns-area-pill, .sort-pill, .artist-subnav-tab";

function resolveStartHtmlElement(rawTarget: EventTarget | null): HTMLElement | null {
  let node: Node | null = rawTarget instanceof Node ? rawTarget : null;
  while (node && !(node instanceof HTMLElement)) {
    node = node.parentElement || node.parentNode;
  }
  return node;
}

function normalizeSingleTitleNode(el: HTMLElement) {
  if (!el.hasAttribute("title")) return;
  const rawTitle = (el.getAttribute("title") || "").trim();
  el.removeAttribute("title");
  if (!rawTitle) return;
  el.setAttribute("data-raw-title", rawTitle);
  if (
    !el.hasAttribute("data-tip") &&
    el.matches('button, a, [role="button"], input, select, .bento-listen-box, .clickable')
  ) {
    el.setAttribute("data-tip", rawTitle);
  }
  if (!el.hasAttribute("aria-label")) {
    el.setAttribute("aria-label", rawTitle);
  }
}

function stripNativeTitlesInSubtree(root: ParentNode) {
  if (root instanceof HTMLElement && root.hasAttribute("title")) {
    normalizeSingleTitleNode(root);
  }
  const titledNodes = root.querySelectorAll?.("[title]");
  if (!titledNodes || titledNodes.length === 0) return;
  titledNodes.forEach((node) => {
    if (node instanceof HTMLElement) {
      normalizeSingleTitleNode(node);
    }
  });
}

function stripNativeTitlesInChain(startEl: HTMLElement | null) {
  let curr: HTMLElement | null = startEl;
  while (curr && curr !== document.body && curr !== document.documentElement) {
    if (curr.hasAttribute("title")) {
      normalizeSingleTitleNode(curr);
    }
    curr = curr.parentElement;
  }
}

function isElementTextTruncated(el: HTMLElement): boolean {
  if (el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 2) {
    return true;
  }
  const child = el.firstElementChild as HTMLElement | null;
  if (
    child &&
    (child.scrollWidth > child.clientWidth + 1 ||
      child.scrollHeight > child.clientHeight + 2)
  ) {
    return true;
  }
  return false;
}

function cleanElementVisibleText(el: HTMLElement): string {
  return (el.textContent || "")
    .replace(/\s+/g, " ")
    .replace(/^[·•\-—\s]+/, "")
    .trim();
}

function formatContextualTooltipText(el: HTMLElement, rawTip: string): string {
  const tip = rawTip.trim();
  const isMetaBtn = el.matches(
    ".inline-meta-btn, .clickable-meta-link, .inline-meta-jump, .pb-song-artist"
  );
  if (isMetaBtn) {
    const label = cleanElementVisibleText(el);
    if (!label) return tip;
    const isAlbumLink =
      el.classList.contains("fs-album") ||
      tip.includes("专辑") ||
      label.startsWith("《");
    if (
      !tip ||
      tip === "点击进入歌手主页" ||
      tip === "点击进入歌手独立主页" ||
      tip === "查看歌手主页" ||
      tip === "进入歌手主页"
    ) {
      return `歌手 · ${label}`;
    }
    if (
      tip === "点击进入专辑独立页面" ||
      tip === "点击进入专辑详情" ||
      tip === "查看专辑详情" ||
      isAlbumLink
    ) {
      const cleanAlbum = label.replace(/^[《「]|[》」]$/g, "");
      return `专辑 · 《${cleanAlbum}》`;
    }
  }
  return tip;
}

function resolveTooltipTarget(rawTarget: EventTarget | null): {
  el: HTMLElement;
  text: string;
} | null {
  if (store.isNowPlayingOpen) {
    return null;
  }
  const startEl = resolveStartHtmlElement(rawTarget);
  if (!startEl) return null;
  stripNativeTitlesInChain(startEl);
  if (
    startEl.closest(
      ".np-root, .np-immersive-win-controls, .pb-volume-control, [data-no-tip]"
    )
  ) {
    return null;
  }

  let curr: HTMLElement | null = startEl;
  while (curr && curr !== document.body && curr !== document.documentElement) {
    // 1. 优先忽略滑块胶囊 Tab 按钮与顶部展开激活导航自身与文字完全一致的冗余提示
    if (curr.matches(TAB_BUTTON_IGNORE_REDUNDANT_SELECTOR)) {
      const explicitTip = (curr.getAttribute("data-tip") || "").trim();
      const visibleLabel = cleanElementVisibleText(curr);
      if (!explicitTip || explicitTip === visibleLabel) {
        return null;
      }
    }
    if (
      curr.classList.contains("island-tab-btn") &&
      curr.classList.contains("active")
    ) {
      return null;
    }

    // 2. 显式配置的 data-tip
    const dataTip = (curr.getAttribute("data-tip") || "").trim();
    if (dataTip) {
      const formatted = formatContextualTooltipText(curr, dataTip);
      if (formatted) {
        return { el: curr, text: formatted };
      }
    }

    // 3. 歌手 / 专辑行内跳转按钮（即便未写 data-tip 也智能生成上下文提示）
    if (
      curr.matches(".inline-meta-btn, .clickable-meta-link, .inline-meta-jump")
    ) {
      const rawTitle = (curr.getAttribute("data-raw-title") || "").trim();
      const formatted = formatContextualTooltipText(curr, rawTitle);
      if (formatted) {
        return { el: curr, text: formatted };
      }
    }

    // 4. 由原生 title 转化来的 data-raw-title 或交互按钮的 aria-label
    const rawTitle = (
      curr.getAttribute("data-raw-title") ||
      (curr.matches('button, a, [role="button"]')
        ? curr.getAttribute("aria-label") || ""
        : "")
    ).trim();
    if (rawTitle) {
      const visibleText = cleanElementVisibleText(curr);
      const isInteractive = curr.matches(INTERACTIVE_TIP_SELECTOR);
      const isTruncated = isElementTextTruncated(curr);
      if (isInteractive && (rawTitle !== visibleText || !visibleText)) {
        return {
          el: curr,
          text: formatContextualTooltipText(curr, rawTitle),
        };
      }
      if (isTruncated) {
        return { el: curr, text: rawTitle };
      }
    }

    // 5. 自动检测所有带省略号截断的歌名 / 歌手 / 专辑 / 歌单 / 电台描述文本
    if (curr.matches(AUTO_ELLIPSIS_TEXT_SELECTOR) && isElementTextTruncated(curr)) {
      const fullText = cleanElementVisibleText(curr);
      if (fullText && fullText.length > 1) {
        return { el: curr, text: fullText };
      }
    }

    curr = curr.parentElement;
  }
  return null;
}

function updateSmartTooltipPosition() {
  if (!activeTooltipTarget || !smartTip.value.visible) return;
  if (!document.body.contains(activeTooltipTarget)) {
    hideSmartTooltip(true);
    return;
  }
  const rect = activeTooltipTarget.getBoundingClientRect();
  if (rect.width === 0 && rect.height === 0) {
    hideSmartTooltip(true);
    return;
  }
  const tipEl = smartTooltipRef.value;
  const tipW = tipEl ? tipEl.offsetWidth || 96 : 96;
  const tipH = tipEl ? tipEl.offsetHeight || 30 : 30;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const edgePad = 12;

  const preferBottom =
    activeTooltipTarget.getAttribute("data-tip-pos") === "bottom" ||
    Boolean(
      activeTooltipTarget.closest(
        ".skyline-top-floating-bar, .mini-top-drag-bar, .skyline-floating-center"
      )
    );

  let placement: "top" | "bottom" = preferBottom ? "bottom" : "top";
  let y = placement === "top" ? rect.top - tipH - 9 : rect.bottom + 9;

  if (placement === "top" && y < 48) {
    placement = "bottom";
    y = rect.bottom + 9;
  } else if (placement === "bottom" && y + tipH > vh - 12) {
    placement = "top";
    y = Math.max(10, rect.top - tipH - 9);
  }

  const centerX = rect.left + rect.width / 2;
  const clampedX = Math.max(
    edgePad,
    Math.min(vw - tipW - edgePad, Math.round(centerX - tipW / 2))
  );
  const clampedY = Math.max(8, Math.min(vh - tipH - 8, Math.round(y)));

  smartTip.value.placement = placement;
  smartTip.value.x = clampedX;
  smartTip.value.y = clampedY;
}

function showSmartTooltipFor(el: HTMLElement, text: string) {
  if (tooltipHoverTimer) {
    clearTimeout(tooltipHoverTimer);
    tooltipHoverTimer = null;
  }
  const now = performance.now();
  const isWarm = smartTip.value.visible || now - lastTooltipHideTime < 320;

  activeTooltipTarget = el;
  smartTip.value.text = text;
  smartTip.value.multiline = text.length > 36 || text.includes("\n");

  const commitShow = () => {
    const wasAlreadyVisible = smartTip.value.visible;
    activeTooltipTarget = el;
    smartTip.value.text = text;
    smartTip.value.multiline = text.length > 36 || text.includes("\n");
    smartTip.value.gliding =
      wasAlreadyVisible || performance.now() - lastTooltipHideTime < 320;
    smartTip.value.visible = true;
    updateSmartTooltipPosition();
    nextTick(() => {
      updateSmartTooltipPosition();
      if (tooltipRafId !== null) cancelAnimationFrame(tooltipRafId);
      tooltipRafId = requestAnimationFrame(updateSmartTooltipPosition);
    });
  };

  if (isWarm) {
    commitShow();
  } else {
    tooltipHoverTimer = setTimeout(() => {
      tooltipHoverTimer = null;
      if (activeTooltipTarget === el) {
        commitShow();
      }
    }, 45);
  }
}

function hideSmartTooltip(immediate = false) {
  if (tooltipHoverTimer) {
    clearTimeout(tooltipHoverTimer);
    tooltipHoverTimer = null;
  }
  activeTooltipTarget = null;
  if (smartTip.value.visible) {
    lastTooltipHideTime = immediate ? 0 : performance.now();
    smartTip.value.visible = false;
    smartTip.value.gliding = false;
  }
}

function handleGlobalPointerOver(e: PointerEvent) {
  const resolved = resolveTooltipTarget(e.target);
  if (!resolved) {
    if (activeTooltipTarget) hideSmartTooltip();
    return;
  }
  const latestTip = resolved.text.trim();
  if (!latestTip) {
    hideSmartTooltip();
    return;
  }
  // 同一按钮内部跨子元素 (<button> -> <svg> -> <path>) 移动时绝不重置正在等待或已显示的浮窗定时器
  if (
    activeTooltipTarget === resolved.el &&
    smartTip.value.text === latestTip &&
    (smartTip.value.visible || tooltipHoverTimer !== null)
  ) {
    if (smartTip.value.visible) {
      updateSmartTooltipPosition();
    }
    return;
  }
  showSmartTooltipFor(resolved.el, latestTip);
}

function handleGlobalPointerOut(e: PointerEvent) {
  if (!activeTooltipTarget) return;
  const related = e.relatedTarget as Node | null;
  if (related && activeTooltipTarget.contains(related)) return;
  hideSmartTooltip();
}

function handleGlobalFocusIn(e: FocusEvent) {
  const resolved = resolveTooltipTarget(e.target);
  if (resolved) {
    showSmartTooltipFor(resolved.el, resolved.text);
  }
}

// ============================================================================
// 全局三层分级 Tag / Tab 引擎 v5
// - L1 主视图/模式分段器 (SEG_TAB_CONTAINER_SELECTOR -> .is-segmented-tabs + GPU 弹簧滑块 .bm-seg-glider)
// - L2 多分类内容导航栏 (TAB_CONTAINER_SELECTOR -> .is-immersive-tabs + 阻尼弹簧流光下划线 + 边缘渐隐)
// - L3 排序/筛选微胶囊：独立零边框柔垫样式，不再被全局下划线劫持
// ============================================================================
const SEG_TAB_CONTAINER_SELECTOR = [
  ".vas-mode-tabs",
  ".nfw-pill-switcher",
  ".pill-tabs",
  ".sr-tabs-group",
  ".lmv-segmented-tabs",
  ".nq-seg-switcher",
  ".np-stage-mode-group",
  ".fp-mode-tabs",
  ".ts-mode-switch-group",
  ".dl-pod-tabs",
  ".bms-subtabs",
  ".bms-pill-group",
  ".bm-fd-scope-tabs",
  ".bmd-filter-pills",
  ".bm-share-edition-pills",
].join(", ");

const TAB_CONTAINER_SELECTOR = [
  ".app-sliding-tabs",
  ".lib-sliding-tabs",
  ".sl-region-tabs",
  ".sl-board-tabs",
  ".radio-plaza-cats",
  ".rom-category-tabs",
  ".vas-cat-pills",
  ".rank-cat-pills",
].join(", ");

const ALL_TAB_ENGINE_SELECTOR = `${TAB_CONTAINER_SELECTOR}, ${SEG_TAB_CONTAINER_SELECTOR}`;

const COMPACT_TAB_CONTAINER_SELECTOR = [
  ".app-sliding-tabs",
  ".vas-cat-pills",
  ".rank-cat-pills",
].join(", ");

const DIVIDER_TAB_CONTAINER_SELECTOR = [
  ".lib-sliding-tabs",
  ".sl-board-tabs",
  ".rom-category-tabs",
  ".radio-plaza-cats",
].join(", ");

const ALWAYS_DARK_SHELL_SELECTOR =
  ".np-stage-dark, .np-floating-window.is-dark-surface, .poster-floating-window.is-dark-surface, .comments-modal-shell.is-dark-surface, .nq-popover-window.is-dark-surface, .dl-shell:not(.is-light-shell)";

const TAB_SCOPE_SECTION_SELECTOR =
  ".discover-block, .discover-subpage, .search-landing-canvas, .search-results-canvas, .page-scroll-canvas, .rom-scroll-body, .library-fullwidth-gallery, .standalone-detail-page, .app-root";

interface RgbColor {
  r: number;
  g: number;
  b: number;
}

// D1 极夜冷曜：极光蓝青靛 × 冰川天青 × 极光青玉
const TAB_PALETTE_D1: RgbColor[] = [
  { r: 0x5e, g: 0x6a, b: 0xd2 }, // #5E6AD2 Aurora Indigo
  { r: 0x38, g: 0xbd, b: 0xf8 }, // #38BDF8 Glacier Cyan
  { r: 0x2d, g: 0xd4, b: 0xbf }, // #2DD4BF Aurora Mint
  { r: 0x81, g: 0x8c, b: 0xf8 }, // #818CF8 Starlight Violet
  { r: 0xfa, g: 0xcc, b: 0x15 }, // #FACC15 Polar Gold
];

// D2 微醺丝绒：落日玫瑰 × 琥珀流金 × 微醺兰紫
const TAB_PALETTE_D2: RgbColor[] = [
  { r: 0xf4, g: 0x3f, b: 0x5e }, // #F43F5E Sunset Rose
  { r: 0xf5, g: 0x9e, b: 0x0b }, // #F59E0B Tube-Amp Amber
  { r: 0xe8, g: 0x79, b: 0xf9 }, // #E879F9 Velvet Orchid
  { r: 0x34, g: 0xd3, b: 0x99 }, // #34D399 Absinthe Mint
  { r: 0xfb, g: 0xbf, b: 0x24 }, // #FBBF24 Champagne Gold
];

// L1 云宣水彩：晴空鸢尾 × 樱花粉红 × 天青水彩
const TAB_PALETTE_L1: RgbColor[] = [
  { r: 0x4f, g: 0x46, b: 0xe5 }, // #4F46E5 Iris Indigo
  { r: 0xec, g: 0x48, b: 0x99 }, // #EC4899 Sakura Rose
  { r: 0x02, g: 0x84, b: 0xc7 }, // #0284C7 Sky Azure
  { r: 0x0d, g: 0x94, b: 0x88 }, // #0D9488 Dew Mint
  { r: 0x7c, g: 0x3a, b: 0xed }, // #7C3AED Wisteria Violet
];

// L2 晨雾燕麦：北欧矿物青玉 × 赤陶胭脂 × 赭石琥珀
const TAB_PALETTE_L2: RgbColor[] = [
  { r: 0x0f, g: 0x76, b: 0x6e }, // #0F766E Nordic Mineral Jade
  { r: 0xbe, g: 0x12, b: 0x3c }, // #BE123C Terracotta Crimson
  { r: 0xb4, g: 0x53, b: 0x09 }, // #B45309 Ochre Amber
  { r: 0x03, g: 0x69, b: 0xa1 }, // #0369A1 Mineral Cobalt
  { r: 0x43, g: 0x38, b: 0xca }, // #4338CA Mineral Indigo
];

const SPRING_DAMPING_RATIO = 0.75;
const SPRING_STIFFNESS = 290;
const SPRING_DAMPING_C =
  2 * SPRING_DAMPING_RATIO * Math.sqrt(SPRING_STIFFNESS);
const INDICATOR_HEIGHT_PX = 3;

interface ImmersiveTabState {
  animatedIndex: number;
  velocity: number;
  targetIndex: number;
  lastActiveBtn: HTMLElement | null;
  currentLeftColor: RgbColor;
  currentRightColor: RgbColor;
  colorsInitialized: boolean;
  activeAnim: boolean;
  baseWidth: number;
  indicator: HTMLElement;
  centers: number[];
  bottoms: number[];
  buttonColors: RgbColor[];
}

const tabStates = new Map<HTMLElement, ImmersiveTabState>();
let tabResizeObserver: ResizeObserver | null = null;
let springLoopRafId: number | null = null;
let lastSpringFrameMs = 0;
let gliderRafId: number | null = null;
let gliderFollowupTimer: ReturnType<typeof setTimeout> | null = null;

function clampNum(v: number, min: number, max: number): number {
  return v < min ? min : v > max ? max : v;
}

function lerpColor(a: RgbColor, b: RgbColor, t: number): RgbColor {
  const k = clampNum(t, 0, 1);
  return {
    r: Math.round(a.r + (b.r - a.r) * k),
    g: Math.round(a.g + (b.g - a.g) * k),
    b: Math.round(a.b + (b.b - a.b) * k),
  };
}

function maxColorDiff(a: RgbColor, b: RgbColor): number {
  return Math.max(
    Math.abs(a.r - b.r),
    Math.abs(a.g - b.g),
    Math.abs(a.b - b.b)
  );
}

function rgbToCss(c: RgbColor): string {
  return `rgb(${c.r}, ${c.g}, ${c.b})`;
}

function rgbToRgba(c: RgbColor, alpha: number): string {
  return `rgba(${c.r}, ${c.g}, ${c.b}, ${alpha})`;
}

function prefersReducedTabMotion(): boolean {
  if (store.motionLevel === "none") return true;
  return (
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true
  );
}

function isContainerOnDarkSurface(container: HTMLElement): boolean {
  if (container.closest(ALWAYS_DARK_SHELL_SELECTOR)) return true;
  const scheme = String(store.resolvedColorScheme || "");
  if (scheme === "watercolor_light" || scheme === "light") return false;
  if (document.documentElement?.dataset?.theme === "watercolor_light") {
    return false;
  }
  return true;
}

function getDirectTabButtons(container: HTMLElement): HTMLElement[] {
  const list: HTMLElement[] = [];
  const children = container.children;
  for (let i = 0; i < children.length; i++) {
    const ch = children[i];
    if (ch instanceof HTMLElement && ch.matches("button, [role='tab']")) {
      list.push(ch);
    }
  }
  return list;
}

function isTabButtonActive(btn: HTMLElement): boolean {
  return (
    btn.classList.contains("active") ||
    btn.classList.contains("is-active") ||
    btn.getAttribute("aria-selected") === "true" ||
    btn.getAttribute("aria-checked") === "true"
  );
}

function ensureImmersiveIndicatorElement(container: HTMLElement): HTMLElement {
  const legacy = container.querySelector(":scope > .elastic-tab-glider");
  if (legacy) legacy.remove();

  let indicator = container.querySelector(
    ":scope > .immersive-tab-indicator"
  ) as HTMLElement | null;
  if (!indicator) {
    indicator = document.createElement("span");
    indicator.className = "immersive-tab-indicator";
    indicator.setAttribute("aria-hidden", "true");
    container.appendChild(indicator);
  }
  return indicator;
}

function measureTabButtonInContainer(
  container: HTMLElement,
  btn: HTMLElement
): { cx: number; bottom: number; width: number } {
  if (btn.offsetParent === container) {
    return {
      cx: btn.offsetLeft + btn.offsetWidth / 2,
      bottom: btn.offsetTop + btn.offsetHeight,
      width: btn.offsetWidth,
    };
  }
  const cRect = container.getBoundingClientRect();
  const bRect = btn.getBoundingClientRect();
  const left = bRect.left - cRect.left + container.scrollLeft;
  const top = bRect.top - cRect.top + container.scrollTop;
  return {
    cx: left + bRect.width / 2,
    bottom: top + bRect.height,
    width: bRect.width,
  };
}

function renderIndicatorFrame(
  state: ImmersiveTabState,
  colorFollowFactor: number
): boolean {
  const count = state.centers.length;
  if (count === 0 || state.buttonColors.length === 0) {
    state.indicator.style.opacity = "0";
    return true;
  }

  const lastIdx = count - 1;
  const clamped = clampNum(state.animatedIndex, 0, lastIdx);
  const lowerIdx = Math.floor(clamped);
  const upperIdx = Math.min(lastIdx, lowerIdx + 1);
  const fraction = clamped - lowerIdx;

  const c1 = state.centers[lowerIdx] ?? 0;
  const c2 = state.centers[upperIdx] ?? c1;
  const centerX = c1 + (c2 - c1) * fraction;
  const barW = state.baseWidth + Math.sin(fraction * Math.PI) * 14;
  const leftX = centerX - barW / 2;

  const y1 = state.bottoms[lowerIdx] ?? 0;
  const y2 = state.bottoms[upperIdx] ?? y1;
  const topY = y1 + (y2 - y1) * fraction - INDICATOR_HEIGHT_PX;

  const distToTarget = Math.abs(state.animatedIndex - state.targetIndex);
  const cLower = state.buttonColors[lowerIdx] ?? state.buttonColors[0];
  const cUpper = state.buttonColors[upperIdx] ?? cLower;

  let desiredLeft: RgbColor;
  let desiredRight: RgbColor;
  if (distToTarget > 0.01) {
    desiredLeft = lerpColor(cLower, cUpper, clampNum(fraction * 0.65, 0, 1));
    desiredRight = lerpColor(
      cLower,
      cUpper,
      clampNum(0.35 + fraction * 0.65, 0, 1)
    );
  } else {
    const targetIdx = clampNum(state.targetIndex, 0, lastIdx);
    const neighborIdx =
      targetIdx < lastIdx ? targetIdx + 1 : Math.max(0, targetIdx - 1);
    const cTarget = state.buttonColors[targetIdx] ?? cLower;
    const cNeighbor = state.buttonColors[neighborIdx] ?? cTarget;
    desiredLeft = cTarget;
    desiredRight = lerpColor(cTarget, cNeighbor, 0.36);
  }

  if (!state.colorsInitialized || colorFollowFactor >= 1) {
    state.currentLeftColor = desiredLeft;
    state.currentRightColor = desiredRight;
    state.colorsInitialized = true;
  } else {
    state.currentLeftColor = lerpColor(
      state.currentLeftColor,
      desiredLeft,
      colorFollowFactor
    );
    state.currentRightColor = lerpColor(
      state.currentRightColor,
      desiredRight,
      colorFollowFactor
    );
  }

  const midColor = lerpColor(
    state.currentLeftColor,
    state.currentRightColor,
    0.5
  );
  state.indicator.style.width = `${barW.toFixed(2)}px`;
  state.indicator.style.transform = `translate3d(${leftX.toFixed(2)}px, ${topY.toFixed(2)}px, 0)`;
  state.indicator.style.background = `linear-gradient(90deg, ${rgbToCss(state.currentLeftColor)}, ${rgbToCss(state.currentRightColor)})`;
  state.indicator.style.boxShadow = `0 2px 8px ${rgbToRgba(midColor, 0.32)}`;
  state.indicator.style.opacity = "1";

  return (
    maxColorDiff(state.currentLeftColor, desiredLeft) <= 1 &&
    maxColorDiff(state.currentRightColor, desiredRight) <= 1
  );
}

function stepSpringLoop(now: number) {
  springLoopRafId = null;
  if (!isAppForegroundActive()) {
    for (const [container, state] of tabStates.entries()) {
      if (!container.isConnected) {
        tabResizeObserver?.unobserve(container);
        tabStates.delete(container);
        continue;
      }
      if (!state.activeAnim) continue;
      state.animatedIndex = state.targetIndex;
      state.velocity = 0;
      state.activeAnim = false;
      renderIndicatorFrame(state, 1);
    }
    lastSpringFrameMs = 0;
    return;
  }
  const rawDt =
    lastSpringFrameMs > 0 ? (now - lastSpringFrameMs) / 1000 : 1 / 60;
  lastSpringFrameMs = now;
  const dt = clampNum(rawDt, 0.001, 0.032);
  const subDt = dt / 2;
  const colorFollowFactor = 1 - Math.exp(-22 * dt);

  let anyActive = false;
  for (const [container, state] of tabStates.entries()) {
    if (!container.isConnected) {
      tabResizeObserver?.unobserve(container);
      tabStates.delete(container);
      continue;
    }
    if (!state.activeAnim) continue;

    for (let s = 0; s < 2; s++) {
      const displacement = state.targetIndex - state.animatedIndex;
      const accel =
        SPRING_STIFFNESS * displacement - SPRING_DAMPING_C * state.velocity;
      state.velocity += accel * subDt;
      state.animatedIndex += state.velocity * subDt;
    }

    const colorSettled = renderIndicatorFrame(state, colorFollowFactor);
    const dist = Math.abs(state.targetIndex - state.animatedIndex);

    if (dist < 0.002 && Math.abs(state.velocity) < 0.004 && colorSettled) {
      state.animatedIndex = state.targetIndex;
      state.velocity = 0;
      state.activeAnim = false;
      renderIndicatorFrame(state, 1);
    } else {
      anyActive = true;
    }
  }

  if (anyActive) {
    springLoopRafId = requestAnimationFrame(stepSpringLoop);
  } else {
    lastSpringFrameMs = 0;
  }
}

function ensureSpringLoopRunning() {
  if (springLoopRafId === null) {
    lastSpringFrameMs = performance.now();
    springLoopRafId = requestAnimationFrame(stepSpringLoop);
  }
}

function syncSingleTabContainerGlider(
  container: HTMLElement,
  allowSpringAnimation = true
) {
  container.classList.add("is-immersive-tabs");
  const isCompact = container.matches(COMPACT_TAB_CONTAINER_SELECTOR);
  const isMini = container.classList.contains("dl-seg-pills");
  container.classList.toggle("is-immersive-compact", isCompact);
  container.classList.toggle("is-immersive-mini", isMini);
  container.classList.toggle(
    "has-tab-divider",
    container.matches(DIVIDER_TAB_CONTAINER_SELECTOR)
  );

  tabResizeObserver?.observe(container);

  const buttons = getDirectTabButtons(container);
  if (buttons.length === 0) return;

  const onDark = isContainerOnDarkSurface(container);
  const inDynamicDarkStage =
    store.isNowPlayingOpen &&
    store.dynamicCoverAccentEnabled &&
    Boolean(container.closest(".np-stage-dark, .np-floating-window.is-dark-surface, .comments-modal-shell.is-dark-surface, .nq-popover-window.is-dark-surface"));
  const palette = onDark
    ? !inDynamicDarkStage && store.darkThemeVariant === "velvet_nebula"
      ? TAB_PALETTE_D2
      : TAB_PALETTE_D1
    : store.lightThemeVariant === "nordic_oat"
    ? TAB_PALETTE_L2
    : TAB_PALETTE_L1;

  let activeIdx = -1;
  let activeBtn: HTMLElement | null = null;
  const buttonColors: RgbColor[] = [];
  for (let i = 0; i < buttons.length; i++) {
    const btn = buttons[i];
    const color = palette[i % palette.length];
    buttonColors.push(color);
    btn.style.setProperty("--tab-accent", rgbToCss(color));
    if (activeIdx === -1 && isTabButtonActive(btn)) {
      activeIdx = i;
      activeBtn = btn;
    }
  }

  const indicator = ensureImmersiveIndicatorElement(container);
  if (!activeBtn || activeIdx === -1) {
    indicator.style.opacity = "0";
    const existingState = tabStates.get(container);
    if (existingState) {
      existingState.activeAnim = false;
      existingState.lastActiveBtn = null;
    }
    return;
  }

  const centers: number[] = [];
  const bottoms: number[] = [];
  let activeWidth = 0;
  for (let i = 0; i < buttons.length; i++) {
    const metrics = measureTabButtonInContainer(container, buttons[i]);
    centers.push(metrics.cx);
    bottoms.push(metrics.bottom);
    if (i === activeIdx) activeWidth = metrics.width;
  }
  if (activeWidth <= 0) return;

  const rawBaseWidth = isMini ? 14 : isCompact ? 16 : 20;
  const baseWidth = Math.min(rawBaseWidth, Math.max(12, activeWidth + 4));
  const reducedMotion = prefersReducedTabMotion();
  const prev = tabStates.get(container);

  const prevBtn = prev?.lastActiveBtn ?? null;
  const isTabChanged = prevBtn !== null && prevBtn !== activeBtn;

  if (isTabChanged && allowSpringAnimation && prev) {
    const prevIdx = clampNum(
      Math.round(prev.targetIndex),
      0,
      buttons.length - 1
    );
    const dir = activeIdx >= prevIdx ? 1 : -1;
    const scopeSection =
      (container.closest(TAB_SCOPE_SECTION_SELECTOR) as HTMLElement | null) ||
      container;
    scopeSection.style.setProperty("--tag-slide-dir", String(dir));

    if (container.scrollWidth > container.clientWidth + 4) {
      const targetScrollLeft = Math.max(
        0,
        centers[activeIdx] - container.clientWidth / 2
      );
      container.scrollTo({
        left: targetScrollLeft,
        behavior: reducedMotion ? "auto" : "smooth",
      });
    }
  }

  if (!prev || !allowSpringAnimation || reducedMotion || prevBtn === null) {
    const seededColor = buttonColors[activeIdx];
    const nextState: ImmersiveTabState = prev ?? {
      animatedIndex: activeIdx,
      velocity: 0,
      targetIndex: activeIdx,
      lastActiveBtn: activeBtn,
      currentLeftColor: seededColor,
      currentRightColor: seededColor,
      colorsInitialized: false,
      activeAnim: false,
      baseWidth,
      indicator,
      centers,
      bottoms,
      buttonColors,
    };
    nextState.indicator = indicator;
    nextState.centers = centers;
    nextState.bottoms = bottoms;
    nextState.buttonColors = buttonColors;
    nextState.baseWidth = baseWidth;
    nextState.lastActiveBtn = activeBtn;
    nextState.targetIndex = activeIdx;
    nextState.animatedIndex = activeIdx;
    nextState.velocity = 0;
    nextState.activeAnim = false;
    tabStates.set(container, nextState);
    renderIndicatorFrame(nextState, 1);
    return;
  }

  prev.indicator = indicator;
  prev.centers = centers;
  prev.bottoms = bottoms;
  prev.buttonColors = buttonColors;
  prev.baseWidth = baseWidth;
  prev.lastActiveBtn = activeBtn;
  prev.targetIndex = activeIdx;
  prev.animatedIndex = clampNum(prev.animatedIndex, 0, buttons.length - 1);

  if (
    Math.abs(prev.targetIndex - prev.animatedIndex) > 0.002 ||
    Math.abs(prev.velocity) > 0.004 ||
    prev.activeAnim
  ) {
    prev.activeAnim = true;
    ensureSpringLoopRunning();
  } else {
    prev.animatedIndex = activeIdx;
    prev.velocity = 0;
    prev.activeAnim = false;
    renderIndicatorFrame(prev, 1);
  }
}

function syncScrollFadeAttr(container: HTMLElement) {
  const overflow = container.scrollWidth - container.clientWidth;
  if (overflow <= 4) {
    if (container.dataset.scrollFade !== "none") {
      container.dataset.scrollFade = "none";
    }
    return;
  }
  const hasLeft = container.scrollLeft > 4;
  const hasRight = container.scrollLeft < overflow - 4;
  const next = hasLeft && hasRight ? "both" : hasLeft ? "left" : hasRight ? "right" : "none";
  if (container.dataset.scrollFade !== next) {
    container.dataset.scrollFade = next;
  }
}

const scrollFadeBoundContainers = new WeakSet<HTMLElement>();

function ensureScrollFadeTracking(container: HTMLElement) {
  if (!scrollFadeBoundContainers.has(container)) {
    scrollFadeBoundContainers.add(container);
    container.addEventListener("scroll", () => syncScrollFadeAttr(container), { passive: true });
  }
  syncScrollFadeAttr(container);
}

interface SegmentedGliderState {
  glider: HTMLElement;
  lastActiveBtn: HTMLElement | null;
  initialized: boolean;
}

const segmentedTabStates = new WeakMap<HTMLElement, SegmentedGliderState>();

function ensureSegmentedGliderElement(container: HTMLElement): HTMLElement {
  let glider = container.querySelector(":scope > .bm-seg-glider") as HTMLElement | null;
  if (!glider) {
    glider = document.createElement("span");
    glider.className = "bm-seg-glider";
    glider.setAttribute("aria-hidden", "true");
    container.insertBefore(glider, container.firstChild);
  }
  return glider;
}

function measureButtonBoxInContainer(
  container: HTMLElement,
  btn: HTMLElement
): { left: number; top: number; width: number; height: number } {
  if (btn.offsetParent === container) {
    return {
      left: btn.offsetLeft,
      top: btn.offsetTop,
      width: btn.offsetWidth,
      height: btn.offsetHeight,
    };
  }
  const cRect = container.getBoundingClientRect();
  const bRect = btn.getBoundingClientRect();
  return {
    left: bRect.left - cRect.left + container.scrollLeft,
    top: bRect.top - cRect.top + container.scrollTop,
    width: bRect.width,
    height: bRect.height,
  };
}

function syncSingleSegmentedContainer(
  container: HTMLElement,
  allowAnimate = true
) {
  container.classList.add("is-segmented-tabs");
  tabResizeObserver?.observe(container);
  const buttons = getDirectTabButtons(container);
  if (buttons.length === 0) return;

  const activeIdx = buttons.findIndex((b) => isTabButtonActive(b));
  const glider = ensureSegmentedGliderElement(container);
  let st = segmentedTabStates.get(container);
  if (!st) {
    st = { glider, lastActiveBtn: null, initialized: false };
    segmentedTabStates.set(container, st);
  }
  st.glider = glider;

  if (activeIdx === -1) {
    glider.style.opacity = "0";
    st.lastActiveBtn = null;
    return;
  }

  const activeBtn = buttons[activeIdx];
  const box = measureButtonBoxInContainer(container, activeBtn);
  if (box.width <= 0 || box.height <= 0) return;

  const reduced = prefersReducedTabMotion();
  const shouldSnap = !allowAnimate || !st.initialized || reduced;
  if (shouldSnap) {
    glider.style.transition = "none";
  } else {
    glider.style.transition = "";
  }

  glider.style.width = `${box.width.toFixed(2)}px`;
  glider.style.height = `${box.height.toFixed(2)}px`;
  glider.style.transform = `translate3d(${box.left.toFixed(2)}px, ${box.top.toFixed(2)}px, 0)`;
  glider.style.opacity = "1";

  if (shouldSnap) {
    // 强制一次 reflow 使首帧定位不飞入，随后恢复平滑弹簧过渡
    void glider.offsetWidth;
    glider.style.transition = "";
  }
  st.lastActiveBtn = activeBtn;
  st.initialized = true;
}

function syncTabEngineContainer(
  node: HTMLElement,
  allowSpringAnimation = true
) {
  if (node.matches(TAB_CONTAINER_SELECTOR)) {
    syncSingleTabContainerGlider(node, allowSpringAnimation);
    ensureScrollFadeTracking(node);
  } else if (node.matches(SEG_TAB_CONTAINER_SELECTOR)) {
    syncSingleSegmentedContainer(node, allowSpringAnimation);
  }
}

function refreshAllTabGliders(allowSpringAnimation = true) {
  const l2Containers = document.querySelectorAll(TAB_CONTAINER_SELECTOR);
  l2Containers.forEach((node) => {
    if (node instanceof HTMLElement) {
      syncSingleTabContainerGlider(node, allowSpringAnimation);
      ensureScrollFadeTracking(node);
    }
  });
  const l1Containers = document.querySelectorAll(SEG_TAB_CONTAINER_SELECTOR);
  l1Containers.forEach((node) => {
    if (node instanceof HTMLElement) {
      syncSingleSegmentedContainer(node, allowSpringAnimation);
    }
  });
}

function scheduleRefreshAllTabGliders(allowSpringAnimation = true) {
  if (gliderRafId !== null) cancelAnimationFrame(gliderRafId);
  gliderRafId = requestAnimationFrame(() => {
    gliderRafId = null;
    refreshAllTabGliders(allowSpringAnimation);
  });
  if (gliderFollowupTimer) clearTimeout(gliderFollowupTimer);
  gliderFollowupTimer = setTimeout(() => {
    gliderFollowupTimer = null;
    refreshAllTabGliders(allowSpringAnimation);
  }, 64);
}

const dirtyTabContainers = new Set<HTMLElement>();
let dirtyTabRafId: number | null = null;

function scheduleDirtyTabGliders(allowSpringAnimation = true) {
  if (dirtyTabRafId !== null) return;
  dirtyTabRafId = requestAnimationFrame(() => {
    dirtyTabRafId = null;
    const targets = Array.from(dirtyTabContainers);
    dirtyTabContainers.clear();
    for (const container of targets) {
      if (container.isConnected) {
        syncTabEngineContainer(container, allowSpringAnimation);
      }
    }
  });
}

function handleGlobalClickForTabDirection(e: MouseEvent) {
  const target = resolveStartHtmlElement(e.target);
  if (!target) return;
  const tabContainer = target.closest(
    ALL_TAB_ENGINE_SELECTOR
  ) as HTMLElement | null;
  if (!tabContainer) return;
  const clickedBtn = target.closest("button, [role='tab']") as HTMLElement | null;
  if (!clickedBtn || !tabContainer.contains(clickedBtn)) return;

  const isL1Seg = tabContainer.matches(SEG_TAB_CONTAINER_SELECTOR);

  // 若首次交互前该容器尚未建立状态，在捕获阶段（Vue 切换 active 前）先锁定旧索引坐标
  if (isL1Seg) {
    if (!segmentedTabStates.has(tabContainer)) {
      syncSingleSegmentedContainer(tabContainer, false);
    }
  } else if (!tabStates.has(tabContainer)) {
    syncSingleTabContainerGlider(tabContainer, false);
  }

  const buttons = getDirectTabButtons(tabContainer);
  const newIdx = buttons.indexOf(clickedBtn);
  const oldIdx = buttons.findIndex((b) => isTabButtonActive(b));
  if (newIdx !== -1 && oldIdx !== -1 && newIdx !== oldIdx) {
    const dir = newIdx > oldIdx ? 1 : -1;
    const scopeSection =
      (tabContainer.closest(TAB_SCOPE_SECTION_SELECTOR) as HTMLElement | null) ||
      tabContainer;
    scopeSection.style.setProperty("--tag-slide-dir", String(dir));
  }
  nextTick(() => {
    if (isL1Seg) {
      syncSingleSegmentedContainer(tabContainer, true);
    } else {
      syncSingleTabContainerGlider(tabContainer, true);
      ensureScrollFadeTracking(tabContainer);
    }
    scheduleRefreshAllTabGliders(true);
  });
}

// ============================================================================
// 底栏小封面 <-> 播放页大封面 参数化二次贝塞尔同轨原路返回 FLIP 引擎 v4
// (Parametric Quadratic Bézier Arc FLIP Engine + 0.00deg Vinyl Angle Sync)
// ============================================================================
interface CoverBoxRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

interface BreezeVinylSharedState {
  angleDeg: number;
  angularVel: number;
  isFlipUnwinding: boolean;
  lastSyncWallMs: number;
  syncRotorDom?: (deg: number) => void;
  ensureStageLoop?: () => void;
}

function getSharedVinylState(): BreezeVinylSharedState {
  if (typeof window === "undefined") {
    return {
      angleDeg: 0,
      angularVel: 0,
      isFlipUnwinding: false,
      lastSyncWallMs: 0,
    };
  }
  const win = window as Window & { __breezeVinylState?: BreezeVinylSharedState };
  if (!win.__breezeVinylState) {
    win.__breezeVinylState = {
      angleDeg: 0,
      angularVel: 0,
      isFlipUnwinding: false,
      lastSyncWallMs: 0,
    };
  }
  return win.__breezeVinylState;
}

/** 将任意角度归一化至 [-180, 180] 最短路径区间 */
function normalizeSignedAngleDeg(deg: number): number {
  if (!Number.isFinite(deg)) return 0;
  let a = ((deg % 360) + 360) % 360;
  if (a > 180) a -= 360;
  return a;
}

/** 将任意角度归一化至 [0, 360) 标准正角区间 */
function normalizePositiveAngleDeg(deg: number): number {
  if (!Number.isFinite(deg)) return 0;
  return ((deg % 360) + 360) % 360;
}

/** 计算从 startDeg 到 endDeg 的最短旋转角差 [-180, 180] */
function getShortestAngleDelta(startDeg: number, endDeg: number): number {
  return normalizeSignedAngleDeg(endDeg - startDeg);
}

const isNpStageClosing = ref(false);
const flipCoverVisible = ref(false);
const isNpCoverFlying = ref(false);
const flipPortalRef = ref<HTMLElement | null>(null);
const flipInnerRef = ref<HTMLElement | null>(null);
const flipVinylOverlayRef = ref<HTMLElement | null>(null);
const flipVinylOverlayOpacity = ref("0");
const flipCoverOuterStyle = ref<Record<string, string>>({});
const flipCoverInnerStyle = ref<Record<string, string>>({});

let npStageCloseTimer: ReturnType<typeof setTimeout> | null = null;
let flipAnimRafId: number | null = null;
let flipSettleRafId: number | null = null;

// 归一化空间轨迹参数 u in [0, 1]：0 = 底栏封面静止位，1 = 播放页封面静止位
let flipProgressU = store.isNowPlayingOpen ? 1 : 0;
// 飞行门户当前实时旋转角与起飞基准角（度）
let flipCurrentAngleDeg = 0;
let flipStartAngleDeg = 0;

const FLIP_ENTER_DURATION_MS = 784;
const FLIP_EXIT_DURATION_MS = 756;

let lastRestingBottomCoverRect: CoverBoxRect = {
  left: 24,
  top: 600,
  width: 58,
  height: 58,
};
let lastNowPlayingCoverRect: CoverBoxRect | null = null;

function syncAppRootFlyingClass(flying: boolean) {
  isNpCoverFlying.value = flying;
  if (typeof document !== "undefined") {
    const appRootEl = document.querySelector(".app-root");
    if (appRootEl) {
      appRootEl.classList.toggle("is-np-cover-flying", flying);
    }
  }
}

function solveCubicBezier(
  p1x: number,
  p1y: number,
  p2x: number,
  p2y: number,
  t: number
): number {
  if (t <= 0) return 0;
  if (t >= 1) return 1;
  const cx = 3 * p1x;
  const bx = 3 * (p2x - p1x) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * p1y;
  const by = 3 * (p2y - p1y) - cy;
  const ay = 1 - cy - by;

  let u = t;
  for (let i = 0; i < 8; i++) {
    const x = ((ax * u + bx) * u + cx) * u - t;
    if (Math.abs(x) < 1e-6) break;
    const dx = (3 * ax * u + 2 * bx) * u + cx;
    if (Math.abs(dx) < 1e-6) break;
    u -= x / dx;
  }
  u = Math.max(0, Math.min(1, u));
  return ((ay * u + by) * u + cy) * u;
}

function clearFlipCoverSchedules() {
  if (flipAnimRafId !== null) {
    cancelAnimationFrame(flipAnimRafId);
    flipAnimRafId = null;
  }
  if (flipSettleRafId !== null) {
    cancelAnimationFrame(flipSettleRafId);
    flipSettleRafId = null;
  }
}

function measureRestingBottomPlayerBarCoverRect(): CoverBoxRect {
  const bottomEl = document.getElementById("playerbar-flip-cover-anchor");
  if (bottomEl) {
    const r = bottomEl.getBoundingClientRect();
    if (r.width > 10 && r.height > 10) {
      // 若底栏当前处于 .player-bar-hidden (translate3d(0, 108%, 0))，精确扣除底栏相对视口底部的隐藏位移量，还原其真实静止位坐标
      const barEl = bottomEl.closest(".fixed-bottom-player-bar") as HTMLElement | null;
      let hiddenShiftY = 0;
      if (barEl) {
        const barRect = barEl.getBoundingClientRect();
        const deltaBottom = barRect.bottom - window.innerHeight;
        if (Math.abs(deltaBottom) > 0.5) {
          hiddenShiftY = deltaBottom;
        }
      }
      const resting: CoverBoxRect = {
        left: r.left,
        top: r.top - hiddenShiftY,
        width: r.width,
        height: r.height,
      };
      lastRestingBottomCoverRect = resting;
      return resting;
    }
  }
  return lastRestingBottomCoverRect;
}

function computeFallbackNowPlayingRect(): CoverBoxRect {
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  const isVinyl = store.playerStageStyle === "vinyl_turntable";
  const workspaceW = Math.min(vw * 0.95, 2200);
  const workspaceLeft = (vw - workspaceW) / 2;
  const padX = Math.min(64, Math.max(28, vw * 0.03));
  const innerW = workspaceW - padX * 2;
  const leftColCenterX = workspaceLeft + padX + innerW * 0.25;

  const rawStageSize = isVinyl
    ? Math.min(444, Math.max(308, Math.min(vh * 0.41, vw * 0.265)))
    : Math.min(436, Math.max(300, Math.min(vh * 0.4, vw * 0.26)));
  const pauseScale = !isVinyl && !store.isPlaying ? 0.96 : 1;
  const platterSize = isVinyl ? rawStageSize * 0.92 : rawStageSize * pauseScale;

  const consoleStackEstH = 232;
  const gapEstH = Math.min(54, Math.max(34, vh * 0.046));
  const totalGroupH = rawStageSize + gapEstH + consoleStackEstH;
  const padTop = Math.min(72, Math.max(54, vh * 0.06)) + Math.min(30, Math.max(16, vh * 0.024));
  const padBottom = Math.min(36, Math.max(22, vh * 0.03)) + Math.min(14, Math.max(6, vh * 0.01));
  const availH = vh - padTop - padBottom;
  const groupStartY = padTop + Math.max(0, (availH - totalGroupH) / 2);

  // 黑胶模式下目标为唱片中心 66% 的圆形封面标，精确匹配新版黑胶封面占比
  const finalSize = isVinyl ? platterSize * 0.66 : platterSize;
  const targetLeft = leftColCenterX - finalSize / 2 + (isVinyl ? -5 : 0);
  const targetTop = groupStartY + (rawStageSize - finalSize) / 2;
  return {
    left: targetLeft,
    top: targetTop,
    width: finalSize,
    height: finalSize,
  };
}

function measureNowPlayingTargetRect(): CoverBoxRect {
  const isVinyl = store.playerStageStyle === "vinyl_turntable";
  const targetEl = document.getElementById("np-flip-target-anchor");
  const shellEl = document.querySelector(".np-stage-overlay-shell") as HTMLElement | null;
  if (targetEl) {
    const r = targetEl.getBoundingClientRect();
    if (r.width > 30 && r.height > 30) {
      // 将 #np-flip-target-anchor 的实时视口包围盒相对 .np-stage-overlay-shell 归一化还原为全屏静止展开态坐标，
      // 彻底消除 .np-stage-overlay-shell 位移与缩放造成的测量偏差
      let restLeft = r.left;
      let restTop = r.top;
      let restW = r.width;
      let restH = r.height;
      if (shellEl) {
        const sr = shellEl.getBoundingClientRect();
        const vw = window.innerWidth;
        const vh = window.innerHeight;
        const sx = sr.width > 100 && vw > 100 ? sr.width / vw : 1;
        const sy = sr.height > 100 && vh > 100 ? sr.height / vh : 1;
        restLeft = (r.left - sr.left) / sx;
        restTop = (r.top - sr.top) / sy;
        restW = r.width / sx;
        restH = r.height / sy;
      }

      if (isVinyl) {
        const labelSize = restW * 0.375;
        const measured: CoverBoxRect = {
          left: restLeft + (restW - labelSize) / 2,
          top: restTop + (restH - labelSize) / 2,
          width: labelSize,
          height: labelSize,
        };
        lastNowPlayingCoverRect = measured;
        return measured;
      }

      const measured: CoverBoxRect = {
        left: restLeft,
        top: restTop,
        width: restW,
        height: restH,
      };
      lastNowPlayingCoverRect = measured;
      return measured;
    }
  }
  return lastNowPlayingCoverRect || computeFallbackNowPlayingRect();
}

/**
 * 根据归一化空间参数 u in [0, 1] 与实时旋转角 angleDeg 计算二次贝塞尔曲线轨迹点并同步写入 DOM：
 * - u = 0 严格对应底栏小封面锚点 (#playerbar-flip-cover-anchor, 0.000deg)
 * - u = 1 严格对应播放页大封面锚点 (#np-flip-target-anchor, vinylAngleDeg)
 * 进入 (0 -> 1) 与退出 (1 -> 0) 共享同一几何映射函数，保证 100% 严格原路返回且旋转角零跳变。
 */
function applyFlipFrameAtU(
  rawU: number,
  bottomRect: CoverBoxRect,
  npRect: CoverBoxRect,
  isVinyl: boolean,
  angleDeg = 0
) {
  const u = Math.max(0, Math.min(1, rawU));
  const oneMinusU = 1 - u;

  // 起点与终点中心坐标
  const cx0 = bottomRect.left + bottomRect.width * 0.5;
  const cy0 = bottomRect.top + bottomRect.height * 0.5;
  const cx1 = npRect.left + npRect.width * 0.5;
  const cy1 = npRect.top + npRect.height * 0.5;

  // 二次贝塞尔控制点：构造自然柔和的向左上浮弧线，进入与退出完全共轨
  const ctrlX = cx0 + (cx1 - cx0) * 0.26;
  const ctrlY = cy0 + (cy1 - cy0) * 0.76;

  // 二次贝塞尔中心坐标 C(u) = (1-u)^2 * C0 + 2(1-u)u * C_ctrl + u^2 * C1
  const cx =
    oneMinusU * oneMinusU * cx0 +
    2 * oneMinusU * u * ctrlX +
    u * u * cx1;
  const cy =
    oneMinusU * oneMinusU * cy0 +
    2 * oneMinusU * u * ctrlY +
    u * u * cy1;

  // 宽高线性插值与当前左上角坐标
  const curW = bottomRect.width + (npRect.width - bottomRect.width) * u;
  const curH = bottomRect.height + (npRect.height - bottomRect.height) * u;
  const curLeft = cx - curW * 0.5;
  const curTop = cy - curH * 0.5;

  const baseW = Math.max(1, npRect.width);
  const baseH = Math.max(1, npRect.height);
  const sx = curW / baseW;
  const sy = curH / baseH;

  // 像素级圆角连续插值并补偿当前缩放比，彻底消除 px 与 % 混用造成的圆角跳变
  const startRadiusPx = 13;
  const endRadiusPx = isVinyl ? baseW * 0.5 : 28;
  const visualRadiusPx = startRadiusPx + (endRadiusPx - startRadiusPx) * u;
  const avgScale = Math.max(0.05, (sx + sy) * 0.5);
  const compensatedRadiusPx = visualRadiusPx / avgScale;

  const effectiveAngleDeg = isVinyl ? angleDeg : 0;
  const rad = (effectiveAngleDeg * Math.PI) / 180;
  const sinA = Math.sin(rad);
  const cosA = Math.cos(rad);

  let boxShadowStr: string;
  if (isVinyl) {
    // 黑胶模式：腾空弧顶拥有饱满悬浮景深投影，落入唱片中心凹槽 (u -> 1) 时平滑收紧贴合 .np-vinyl-label-art 真实内阴影
    const midAir = 4 * u * oneMinusU;
    const dropDist = 6 * oneMinusU + 18 * midAir + 4 * u;
    const dropDx = (dropDist * sinA).toFixed(2);
    const dropDy = (dropDist * cosA).toFixed(2);
    const dropBlur = Math.round(18 * oneMinusU + 36 * midAir + 16 * u);
    const dropAlpha = (0.28 * oneMinusU + 0.52 * midAir + 0.78 * u).toFixed(3);
    const insetAlpha = (0.42 * u).toFixed(3);
    const rimAlpha = (0.14 * u).toFixed(3);
    boxShadowStr = `${dropDx}px ${dropDy}px ${dropBlur}px rgba(0, 0, 0, ${dropAlpha}), inset 0 0 14px rgba(0, 0, 0, ${insetAlpha}), inset 0 0 0 1px rgba(255, 255, 255, ${rimAlpha})`;
  } else {
    const shadowAlpha = (0.32 + 0.34 * u).toFixed(3);
    const glowAlpha = (0.26 * u).toFixed(3);
    const shadowY = Math.round(8 + 20 * u);
    const shadowBlur = Math.round(20 + 44 * u);
    boxShadowStr = `0 ${shadowY}px ${shadowBlur}px -8px rgba(0, 0, 0, ${shadowAlpha}), 0 12px 34px -8px rgba(124, 106, 246, ${glowAlpha})`;
  }

  const vinylOverlayAlpha = isVinyl ? Math.pow(u, 2.2).toFixed(3) : "0";
  flipVinylOverlayOpacity.value = vinylOverlayAlpha;
  if (flipVinylOverlayRef.value) {
    flipVinylOverlayRef.value.style.opacity = vinylOverlayAlpha;
  }

  const outerStyles: Record<string, string> = {
    width: `${baseW.toFixed(2)}px`,
    height: `${baseH.toFixed(2)}px`,
    transform: `translate3d(${curLeft.toFixed(3)}px, ${curTop.toFixed(3)}px, 0) scale3d(${sx.toFixed(5)}, ${sy.toFixed(5)}, 1)`,
    opacity: "1",
    transition: "none",
  };

  const innerStyles: Record<string, string> = {
    transformOrigin: "50% 50%",
    transform: `translate3d(0, 0, 0) rotate3d(0, 0, 1, ${effectiveAngleDeg.toFixed(3)}deg)`,
    borderRadius: `${compensatedRadiusPx.toFixed(2)}px`,
    boxShadow: boxShadowStr,
    transition: "none",
  };

  flipCoverOuterStyle.value = outerStyles;
  flipCoverInnerStyle.value = innerStyles;

  const outerEl = flipPortalRef.value;
  const innerEl = flipInnerRef.value;
  if (outerEl && innerEl) {
    outerEl.classList.add("is-active");
    Object.assign(outerEl.style, outerStyles);
    Object.assign(innerEl.style, innerStyles);
  }
}

watch(
  () => store.isNowPlayingOpen,
  (opening) => {
    hideSmartTooltip(true);

    if (npStageCloseTimer) {
      clearTimeout(npStageCloseTimer);
      npStageCloseTimer = null;
    }
    if (opening) {
      isNpStageClosing.value = false;
    } else {
      isNpStageClosing.value = true;
      npStageCloseTimer = setTimeout(() => {
        isNpStageClosing.value = false;
        npStageCloseTimer = null;
      }, 784);
    }

    const sharedVinyl = getSharedVinylState();

    if (store.playerStageStyle === "pure_lyric" || store.motionLevel !== "best") {
      clearFlipCoverSchedules();
      sharedVinyl.isFlipUnwinding = false;
      flipProgressU = opening ? 1 : 0;
      flipCurrentAngleDeg = 0;
      flipStartAngleDeg = 0;
      flipCoverVisible.value = false;
      syncAppRootFlyingClass(false);
      flipCoverOuterStyle.value = {
        ...flipCoverOuterStyle.value,
        opacity: "0",
      };
      if (flipPortalRef.value) {
        flipPortalRef.value.style.opacity = "0";
        flipPortalRef.value.classList.remove("is-active");
      }
      if (!opening && store.motionLevel === "none") {
        if (npStageCloseTimer) {
          clearTimeout(npStageCloseTimer);
          npStageCloseTimer = null;
        }
        isNpStageClosing.value = false;
      }
      return;
    }

    const wasFlying = isNpCoverFlying.value && flipCoverVisible.value;
    clearFlipCoverSchedules();

    const isVinyl = store.playerStageStyle === "vinyl_turntable";
    let bottomRect = measureRestingBottomPlayerBarCoverRect();
    let npRect = measureNowPlayingTargetRect();

    // 若用户在飞行中途快速反向连击，直接从当前曲线参数 u 与当前实时旋转角原路无缝折返
    const startU = wasFlying ? flipProgressU : opening ? 0 : 1;
    const targetU = opening ? 1 : 0;
    const distanceRatio = Math.max(0.25, Math.abs(targetU - startU));
    const durationMs =
      (opening ? FLIP_ENTER_DURATION_MS : FLIP_EXIT_DURATION_MS) * distanceRatio;

    if (isVinyl) {
      if (opening) {
        // 从底栏展开到播放页 (u: 0 -> 1)：起始角 0deg（或半途折返角），同步锁定转盘起始相位并随飞行连续旋转承接
        sharedVinyl.isFlipUnwinding = false;
        flipStartAngleDeg = wasFlying
          ? normalizeSignedAngleDeg(flipCurrentAngleDeg)
          : 0;
        flipCurrentAngleDeg = flipStartAngleDeg;
        sharedVinyl.ensureStageLoop?.();
        sharedVinyl.syncRotorDom?.(flipStartAngleDeg);
      } else {
        // 从播放页收起到底栏 (u: 1 -> 0)：读取黑胶当前真实角度并归一化至 [-180, 180] 最短路径，随 u -> 0 平滑归零至 0deg
        sharedVinyl.isFlipUnwinding = true;
        const rawTakeoffAngle = wasFlying
          ? flipCurrentAngleDeg
          : sharedVinyl.angleDeg;
        flipStartAngleDeg = normalizeSignedAngleDeg(rawTakeoffAngle);
        flipCurrentAngleDeg = flipStartAngleDeg;
        sharedVinyl.syncRotorDom?.(flipStartAngleDeg);
      }
    } else {
      sharedVinyl.isFlipUnwinding = false;
      flipStartAngleDeg = 0;
      flipCurrentAngleDeg = 0;
    }

    flipCoverVisible.value = true;
    syncAppRootFlyingClass(true);
    flipProgressU = startU;

    // 首帧同步写入起点几何与精确旋转角状态，杜绝任何等待帧或角度跳变
    applyFlipFrameAtU(
      startU,
      bottomRect,
      npRect,
      isVinyl,
      flipCurrentAngleDeg
    );

    const startTime = performance.now();

    const stepFlipArc = (now: number) => {
      const elapsed = now - startTime;
      const tau = Math.max(0, Math.min(1, elapsed / durationMs));

      // 进入与退出均采用丝滑阻尼缓动曲线，并共享同一空间二次贝塞尔函数 applyFlipFrameAtU
      const eased = opening
        ? solveCubicBezier(0.22, 1, 0.36, 1, tau)
        : solveCubicBezier(0.22, 1, 0.36, 1, tau);

      const currentU = startU + (targetU - startU) * eased;
      flipProgressU = currentU;

      // 前段动态校准目标静息坐标，消除首帧布局微调误差
      if (tau < 0.4) {
        if (opening) {
          npRect = measureNowPlayingTargetRect();
        } else {
          bottomRect = measureRestingBottomPlayerBarCoverRect();
        }
      }

      if (isVinyl) {
        if (opening) {
          const spanU = Math.max(1e-5, 1 - startU);
          const progress = Math.max(0, Math.min(1, (currentU - startU) / spanU));
          const liveRotorDeg = sharedVinyl.angleDeg;
          const deltaDeg = getShortestAngleDelta(flipStartAngleDeg, liveRotorDeg);
          flipCurrentAngleDeg = flipStartAngleDeg + deltaDeg * progress;
        } else {
          const ratio =
            startU > 1e-5 ? Math.max(0, Math.min(1, currentU / startU)) : 0;
          flipCurrentAngleDeg = flipStartAngleDeg * ratio;
          sharedVinyl.syncRotorDom?.(flipCurrentAngleDeg);
        }
      } else {
        flipCurrentAngleDeg = 0;
      }

      applyFlipFrameAtU(
        currentU,
        bottomRect,
        npRect,
        isVinyl,
        flipCurrentAngleDeg
      );

      if (tau < 1) {
        flipAnimRafId = requestAnimationFrame(stepFlipArc);
      } else {
        flipAnimRafId = null;
        flipProgressU = targetU;
        sharedVinyl.isFlipUnwinding = false;

        if (isVinyl) {
          if (opening) {
            // u = 1 落地交接帧：将飞行门户旋转角与 NowPlayingCoverStage .np-vinyl-platter-rotor 精确锁定同一角度（误差 0.000deg）
            const finalLandingDeg = normalizePositiveAngleDeg(sharedVinyl.angleDeg);
            flipCurrentAngleDeg = finalLandingDeg;
            sharedVinyl.syncRotorDom?.(finalLandingDeg);
          } else {
            // u = 0 落地交接帧：精确归零至 0.000deg，与底栏方形小封面 100% 重合
            flipCurrentAngleDeg = 0;
            sharedVinyl.syncRotorDom?.(0);
          }
        } else {
          flipCurrentAngleDeg = 0;
        }

        applyFlipFrameAtU(
          targetU,
          bottomRect,
          npRect,
          isVinyl,
          flipCurrentAngleDeg
        );

        // 同步令终点真实封面恢复不透明，下一帧隐藏飞行门户，并在过渡帧内保持角度同步，彻底消除任何重影或空白闪屏
        syncAppRootFlyingClass(false);
        flipSettleRafId = requestAnimationFrame(() => {
          flipSettleRafId = null;
          flipCoverVisible.value = false;
          flipCoverOuterStyle.value = {
            ...flipCoverOuterStyle.value,
            opacity: "0",
          };
          if (flipPortalRef.value) {
            flipPortalRef.value.style.opacity = "0";
            flipPortalRef.value.classList.remove("is-active");
          }
        });
      }
    };

    flipAnimRafId = requestAnimationFrame(stepFlipArc);
  },
  { flush: "sync" }
);

// 监听二级页面、搜索模式、榜单与资产库切换：绝不在退场中途强行跳回顶部，而是预约在 @before-enter 时精准同步
watch(
  [
    () => store.searchViewMode,
    () => store.discoverSubView,
    () => store.libraryDetailMode,
    () => store.activeRank?.rankId,
  ],
  ([newSearchMode, newSubView], [oldSearchMode, oldSubView]) => {
    hideSmartTooltip(true);
    if (navBackPendingScrollTop !== null) {
      pendingTargetScrollTop = navBackPendingScrollTop;
      navBackPendingScrollTop = null;
      nextTick(() => {
        scheduleRefreshAllTabGliders(true);
      });
      return;
    }
    const isReturningToDiscoverHome =
      newSubView === "home" &&
      oldSubView !== "home" &&
      oldSubView !== undefined;
    const isReturningToSearchLanding =
      newSearchMode === "landing" && oldSearchMode === "results";

    if (isReturningToDiscoverHome) {
      pendingTargetScrollTop = savedViewScrollMap.get("discover:home") ?? 0;
    } else if (isReturningToSearchLanding) {
      pendingTargetScrollTop = savedViewScrollMap.get("search:landing") ?? 0;
    } else {
      pendingTargetScrollTop = 0;
    }
    nextTick(() => {
      scheduleRefreshAllTabGliders(true);
    });
  }
);

function handleGlobalKeyDown(e: KeyboardEvent) {
  // 正在设置页录制自定义快捷键时，交由录制器独占处理
  if (store.isRecordingShortcut) return;

  const target = e.target as HTMLElement | null;
  const isInput =
    target &&
    (target.tagName === "INPUT" ||
      target.tagName === "TEXTAREA" ||
      target.isContentEditable);

  const hk = store.inAppHotkeys;

  // 返回上一页（默认 Alt+Left）
  if (!isInput && hk.nav_back && matchKeyboardEventAccelerator(e, hk.nav_back)) {
    if (store.canNavigateBack) {
      e.preventDefault();
      store.navigateBack();
      return;
    }
  }

  // 快速搜索（默认 Ctrl+K，即使在输入框外或部分非输入控件也可直达）
  if (hk.focus_search && matchKeyboardEventAccelerator(e, hk.focus_search)) {
    e.preventDefault();
    store.isNowPlayingOpen = false;
    if (store.activeTab !== "search") {
      store.pushNavHistory();
    }
    store.activeTab = "search";
    nextTick(() => {
      const input = document.getElementById(
        "independent-search-input"
      ) as HTMLInputElement | null;
      if (input) {
        input.focus();
        input.select();
      }
    });
    return;
  }

  // Esc：按顶层优先级逐层收起右键菜单、弹窗、榜单总览或全屏播放舞台
  if (e.key === "Escape") {
    if (store.confirmDialog.open) {
      store.resolveConfirmDialog(false);
      return;
    }
    if (store.contextMenu.visible) {
      store.closeSongContextMenu();
      return;
    }
    if (store.isMultiArtistModalOpen) {
      store.isMultiArtistModalOpen = false;
      return;
    }
    if (store.isCommentsModalOpen) {
      store.closeCommentsModal();
      return;
    }
    if (store.hdArtworkPreviewTarget || store.hdArtworkPreviewUrl) {
      store.closeHdArtworkPreview();
      return;
    }
    if (store.detectedSharedTarget) {
      store.dismissDetectedSharedTarget();
      return;
    }
    if (store.shareModalSong || store.shareModalPlaylist) {
      store.closeShareModal();
      return;
    }
    if (store.songDetailModalSong) {
      store.songDetailModalSong = null;
      return;
    }
    if (store.downloadModalSong) {
      store.downloadModalSong = null;
      return;
    }
    if (store.addToPlaylistModalSong) {
      store.addToPlaylistModalSong = null;
      return;
    }
    if (store.isLyricPosterOpen) {
      store.isLyricPosterOpen = false;
      return;
    }
    if (store.isFocusPodOpen) {
      store.isFocusPodOpen = false;
      return;
    }
    if (store.isAccountProfileOpen) {
      store.isAccountProfileOpen = false;
      return;
    }
    if (store.isAccountManagerOpen) {
      store.isAccountManagerOpen = false;
      return;
    }
    if (store.isVipCenterOpen) {
      store.isVipCenterOpen = false;
      return;
    }
    if (store.isViperStudioModalOpen) {
      store.isViperStudioModalOpen = false;
      return;
    }
    if (store.isLoginModalOpen) {
      store.isLoginModalOpen = false;
      return;
    }
    if (store.isRankGridModalOpen) {
      store.isRankGridModalOpen = false;
      return;
    }
    if (store.isQueueOpen) {
      store.isQueueOpen = false;
      return;
    }
    // 派发自定义事件，供页面局部模态（新建/编辑歌单、云盘加单等）响应 Esc 关闭
    const escEv = new CustomEvent("breeze:escape-modal", { cancelable: true });
    if (!window.dispatchEvent(escEv)) {
      return;
    }
    if (store.isNowPlayingOpen && !isInput) {
      store.isNowPlayingOpen = false;
      return;
    }
  }

  if (isInput) return;

  // 播放 / 暂停（默认 Space）
  if (hk.toggle_play && matchKeyboardEventAccelerator(e, hk.toggle_play)) {
    e.preventDefault();
    store.togglePlayPause();
    return;
  }

  // 切换 Mini 灵动岛小窗（默认 Ctrl+Alt+M）
  if (hk.toggle_mini_mode && matchKeyboardEventAccelerator(e, hk.toggle_mini_mode)) {
    e.preventDefault();
    store.toggleMiniPlayerMode();
    return;
  }

  // 锁定 / 解锁桌面歌词穿透（默认 Ctrl+Shift+L）
  if (hk.toggle_lyric_lock && matchKeyboardEventAccelerator(e, hk.toggle_lyric_lock)) {
    e.preventDefault();
    store.desktopLyricLocked = !store.desktopLyricLocked;
    store.notify(
      store.desktopLyricLocked
        ? `已锁定桌面歌词（开启鼠标穿透，按 ${hk.toggle_lyric_lock} 可随时解锁）`
        : "已解锁桌面歌词，现在可自由拖动与调节"
    );
    return;
  }

  // 上一曲 / 下一曲（默认 Ctrl+Left / Ctrl+Right）
  if (hk.prev_song && matchKeyboardEventAccelerator(e, hk.prev_song)) {
    e.preventDefault();
    store.playPrev();
    return;
  }
  if (hk.next_song && matchKeyboardEventAccelerator(e, hk.next_song)) {
    e.preventDefault();
    store.playNext(false);
    return;
  }

  // 音量加 / 减（默认 Ctrl+Up / Ctrl+Down）
  if (hk.volume_up && matchKeyboardEventAccelerator(e, hk.volume_up)) {
    e.preventDefault();
    store.setVolume(Math.min(1, Number((store.volume + 0.05).toFixed(2))));
    return;
  }
  if (hk.volume_down && matchKeyboardEventAccelerator(e, hk.volume_down)) {
    e.preventDefault();
    store.setVolume(Math.max(0, Number((store.volume - 0.05).toFixed(2))));
    return;
  }

  // 喜欢 / 取消喜欢当前歌曲（默认 Ctrl+L）
  if (hk.toggle_like && matchKeyboardEventAccelerator(e, hk.toggle_like)) {
    e.preventDefault();
    if (store.currentSong) {
      store.toggleFavoriteSong(store.currentSong);
    }
    return;
  }
}

function handleGlobalMouseBack(e: MouseEvent) {
  if (isStandaloneSubWindow) return;
  if (e.button === 3) {
    e.preventDefault();
    if (e.type === "mouseup" && store.canNavigateBack) {
      store.navigateBack();
    }
  }
}

function handleWindowResize() {
  hideSmartTooltip(true);
  scheduleRefreshAllTabGliders(false);
}

function handlePointerDownHideTip() {
  hideSmartTooltip(true);
}

watch(
  () => [
    store.resolvedColorScheme,
    store.lightThemeVariant,
    store.darkThemeVariant,
  ],
  () => {
    for (const st of tabStates.values()) {
      st.colorsInitialized = false;
    }
    nextTick(() => {
      refreshAllTabGliders(false);
    });
  }
);

let uninstallNativeGuard: (() => void) | null = null;
let appearancePrefsChannel: BroadcastChannel | null = null;
let unsubscribePowerVis: (() => void) | null = null;

function isPotentialTabMutationTarget(el: HTMLElement): boolean {
  const tag = el.tagName;
  if (tag === "BUTTON" || tag === "A" || tag === "LABEL") return true;
  const role = el.getAttribute("role");
  if (role === "tab" || role === "radio" || role === "tablist" || role === "radiogroup") {
    return true;
  }
  const cls = typeof el.className === "string" ? el.className : "";
  if (!cls) return false;
  return (
    cls.includes("tab") ||
    cls.includes("pill") ||
    cls.includes("seg") ||
    cls.includes("mode") ||
    cls.includes("filter") ||
    cls.includes("nav") ||
    cls.includes("cat")
  );
}

onMounted(() => {
  ensureAppPowerVisibilityTracking();
  unsubscribePowerVis = subscribeAppPowerVisibility((active) => {
    if (!active) {
      if (springLoopRafId !== null) {
        cancelAnimationFrame(springLoopRafId);
        springLoopRafId = null;
        lastSpringFrameMs = 0;
      }
      if (dirtyTabRafId !== null) {
        cancelAnimationFrame(dirtyTabRafId);
        dirtyTabRafId = null;
      }
      hideSmartTooltip(true);
    } else if (dirtyTabContainers.size > 0) {
      scheduleDirtyTabGliders(false);
    }
  });
  uninstallNativeGuard = installDesktopNativeGuard({
    allowCustomInputMenu: !isStandaloneSubWindow,
  });
  if (typeof BroadcastChannel !== "undefined") {
    try {
      appearancePrefsChannel = new BroadcastChannel(APPEARANCE_PREFS_SYNC_CHANNEL);
      appearancePrefsChannel.onmessage = (ev) => {
        const data = ev?.data;
        if (!data || data.type !== "APPEARANCE_PREFS_UPDATED") return;
        const nextMotion = normalizeMotionLevel(data.motionLevel);
        const nextDensity = normalizeUiDensity(data.uiDensity);
        store.motionLevel = nextMotion;
        store.uiDensity = nextDensity;
        applyMotionLevelToDocument(nextMotion);
        applyUiDensityToDocument(nextDensity);
      };
    } catch {
      appearancePrefsChannel = null;
    }
  }
  document.addEventListener("click", handleGlobalClickForTabDirection, true);
  window.addEventListener("resize", handleWindowResize, { passive: true });

  if (typeof ResizeObserver !== "undefined") {
    tabResizeObserver = new ResizeObserver(() => {
      scheduleRefreshAllTabGliders(false);
    });
  }

  if (!isStandaloneSubWindow) {
    displayedGreetingVisible.value = isPrimaryGreetingVisible.value;
    store.bootstrap();
    window.addEventListener("keydown", handleGlobalKeyDown);
    window.addEventListener("mouseup", handleGlobalMouseBack);
    window.addEventListener("auxclick", handleGlobalMouseBack);
    window.addEventListener("breeze:nav-back-restore", handleNavBackRestoreEvent);
    window.addEventListener("breeze:subpage-before-enter", handleStageBeforeEnter);
    document.addEventListener("pointerover", handleGlobalPointerOver, true);
    document.addEventListener("pointerout", handleGlobalPointerOut, true);
    document.addEventListener("pointerdown", handlePointerDownHideTip, true);
    document.addEventListener("focusin", handleGlobalFocusIn, true);
    document.addEventListener("focusout", handlePointerDownHideTip, true);
    window.addEventListener("wheel", handlePointerDownHideTip, { passive: true, capture: true });

    nextTick(() => {
      stripNativeTitlesInSubtree(document.body);
      refreshAllTabGliders(false);
    });
  } else {
    nextTick(() => {
      refreshAllTabGliders(false);
    });
  }

  if (typeof MutationObserver !== "undefined") {
    domMutationObserver = new MutationObserver((mutations) => {
      let hasDirtyTabs = false;
      for (const m of mutations) {
        if (m.type === "attributes") {
          if (
            !isStandaloneSubWindow &&
            m.attributeName === "title" &&
            m.target instanceof HTMLElement
          ) {
            normalizeSingleTitleNode(m.target);
          } else if (
            (m.attributeName === "class" ||
              m.attributeName === "aria-selected" ||
              m.attributeName === "aria-checked") &&
            m.target instanceof HTMLElement &&
            !m.target.classList.contains("immersive-tab-indicator") &&
            !m.target.classList.contains("bm-seg-glider") &&
            isPotentialTabMutationTarget(m.target)
          ) {
            const container = m.target.closest(ALL_TAB_ENGINE_SELECTOR) as HTMLElement | null;
            if (container) {
              dirtyTabContainers.add(container);
              hasDirtyTabs = true;
            }
          }
        } else if (m.type === "childList") {
          for (let ni = 0; ni < m.addedNodes.length; ni++) {
            const node = m.addedNodes[ni];
            if (!(node instanceof HTMLElement)) continue;
            if (!isStandaloneSubWindow) {
              stripNativeTitlesInSubtree(node);
            }
            if (
              !node.classList.contains("immersive-tab-indicator") &&
              !node.classList.contains("bm-seg-glider")
            ) {
              const closestHost = isPotentialTabMutationTarget(node)
                ? (node.closest(ALL_TAB_ENGINE_SELECTOR) as HTMLElement | null)
                : null;
              if (closestHost) {
                dirtyTabContainers.add(closestHost);
                hasDirtyTabs = true;
              } else if (node.matches(ALL_TAB_ENGINE_SELECTOR)) {
                dirtyTabContainers.add(node);
                hasDirtyTabs = true;
              } else if (node.childElementCount > 0) {
                const nested = node.querySelectorAll(ALL_TAB_ENGINE_SELECTOR);
                nested.forEach((el) => {
                  if (el instanceof HTMLElement) {
                    dirtyTabContainers.add(el);
                    hasDirtyTabs = true;
                  }
                });
              }
            }
          }
        }
      }
      if (hasDirtyTabs && isAppForegroundActive()) {
        scheduleDirtyTabGliders(true);
      }
    });
    domMutationObserver.observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: isStandaloneSubWindow
        ? ["class", "aria-selected", "aria-checked"]
        : ["title", "class", "aria-selected", "aria-checked"],
    });
  }
});

onBeforeUnmount(() => {
  if (unsubscribePowerVis) {
    unsubscribePowerVis();
    unsubscribePowerVis = null;
  }
  if (typeof document !== "undefined") {
    document.documentElement.classList.remove("bm-window-hidden");
  }
  dirtyTabContainers.clear();
  if (dirtyTabRafId !== null) {
    cancelAnimationFrame(dirtyTabRafId);
    dirtyTabRafId = null;
  }
  if (appearancePrefsChannel) {
    try {
      appearancePrefsChannel.close();
    } catch {
      // ignore
    }
    appearancePrefsChannel = null;
  }
  if (uninstallNativeGuard) {
    uninstallNativeGuard();
    uninstallNativeGuard = null;
  }
  window.removeEventListener("keydown", handleGlobalKeyDown);
  window.removeEventListener("mouseup", handleGlobalMouseBack);
  window.removeEventListener("auxclick", handleGlobalMouseBack);
  window.removeEventListener("breeze:nav-back-restore", handleNavBackRestoreEvent);
  window.removeEventListener("breeze:subpage-before-enter", handleStageBeforeEnter);
  document.removeEventListener("click", handleGlobalClickForTabDirection, true);
  document.removeEventListener("pointerover", handleGlobalPointerOver, true);
  document.removeEventListener("pointerout", handleGlobalPointerOut, true);
  document.removeEventListener("pointerdown", handlePointerDownHideTip, true);
  document.removeEventListener("focusin", handleGlobalFocusIn, true);
  document.removeEventListener("focusout", handlePointerDownHideTip, true);
  window.removeEventListener("wheel", handlePointerDownHideTip, true);
  window.removeEventListener("resize", handleWindowResize);
  if (domMutationObserver) {
    domMutationObserver.disconnect();
    domMutationObserver = null;
  }
  if (tabResizeObserver) {
    tabResizeObserver.disconnect();
    tabResizeObserver = null;
  }
  tabStates.clear();
  if (tooltipHoverTimer) clearTimeout(tooltipHoverTimer);
  if (greetingSyncTimer) clearTimeout(greetingSyncTimer);
  if (gliderFollowupTimer) clearTimeout(gliderFollowupTimer);
  if (npStageCloseTimer) clearTimeout(npStageCloseTimer);
  clearFlipCoverSchedules();
  if (tooltipRafId !== null) cancelAnimationFrame(tooltipRafId);
  if (gliderRafId !== null) cancelAnimationFrame(gliderRafId);
  if (springLoopRafId !== null) cancelAnimationFrame(springLoopRafId);
});
</script>

<template>
  <!-- 全局防边缘截断、防原生矩形框的智能毛玻璃悬浮提示门户 (Teleport 至 body 顶层) -->
  <Teleport to="body">
    <div
      v-if="!isStandaloneSubWindow"
      ref="smartTooltipRef"
      class="global-smart-tooltip"
      :class="[
        `scheme-${store.resolvedColorScheme}`,
        `place-${smartTip.placement}`,
        {
          'is-visible': smartTip.visible && Boolean(smartTip.text),
          'is-gliding': smartTip.gliding,
          'is-multiline': smartTip.multiline,
        },
      ]"
      :style="{
        left: `${smartTip.x}px`,
        top: `${smartTip.y}px`,
      }"
      role="tooltip"
      aria-hidden="true"
    >
      {{ smartTip.text }}
    </div>
  </Teleport>

  <DesktopLyricWindow v-if="isStandaloneDesktopLyricWindow" />
  <MiniPlayerWindow
    v-else-if="isStandaloneMiniPlayerWindow || (!isTauriRuntime && store.isMiniPlayerMode)"
    :standalone="isStandaloneMiniPlayerWindow"
    :preview-store="store"
  />
  <div
    v-else
    class="app-root"
    :class="[
      `scheme-${store.resolvedColorScheme}`,
      `light-variant-${store.lightThemeVariant}`,
      `dark-variant-${store.darkThemeVariant}`,
      {
        'is-np-cover-flying': isNpCoverFlying,
        'is-now-playing-open': store.isNowPlayingOpen,
      },
    ]"
    @click="
      store.isSearchPaletteOpen = false;
      store.isGlobalQueueOpen = false;
      store.closeSongContextMenu();
    "
  >
    <!-- 全局浮动提示胶囊（灵动岛弹性舒展 + 复制/成功类 SVG 动态描边对勾） -->
    <transition name="bm-toast-island">
      <div
        v-if="store.statusBanner"
        :key="store.statusBanner"
        class="toast-banner"
      >
        <svg
          v-if="
            /复制|剪贴板|口令|已保存|已导出|领取成功|已完成|已切换/.test(
              store.statusBanner
            )
          "
          class="bm-toast-check-svg"
          viewBox="0 0 20 20"
          fill="none"
          aria-hidden="true"
        >
          <path
            class="bm-toast-check-path"
            d="M4.2 10.6L8.2 14.4L15.8 6.2"
            stroke="currentColor"
            stroke-width="2.3"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        <PlumpIcon v-else name="sparkles" :size="15" color="var(--primary)" />
        <span>{{ store.statusBanner }}</span>
      </div>
    </transition>

    <!-- 全站微观粒子与抛物线合成层画布 -->
    <MotionFxOverlay />

    <!-- 底栏小封面 <-> 播放页大封面 双轴 GPU 合成器抛物弧线 FLIP 飞入过渡门户（常驻 DOM 零挂载延迟） -->
    <div
      ref="flipPortalRef"
      class="flip-flying-cover-portal"
      :class="{ 'is-active': flipCoverVisible }"
      :style="flipCoverOuterStyle"
    >
      <div
        ref="flipInnerRef"
        class="flip-flying-cover-inner"
        :style="flipCoverInnerStyle"
      >
        <img
          v-if="store.currentSong?.coverUrl"
          :src="store.currentSong.coverUrl"
          alt=""
        />
        <div v-else class="flip-flying-cover-fallback">♪</div>
        <!-- 黑胶中心唱片纸标压线与金属轴孔同步过渡层（u=0 底栏态完全透明，u=1 落盘态与 .np-vinyl-label-art 100% 重合） -->
        <div
          ref="flipVinylOverlayRef"
          class="flip-flying-vinyl-overlay"
          :style="{ opacity: flipVinylOverlayOpacity }"
          aria-hidden="true"
        >
          <div class="flip-vinyl-press-groove"></div>
          <div class="flip-vinyl-spindle-hole"></div>
          <div class="flip-vinyl-label-glass"></div>
        </div>
      </div>
    </div>

    <!-- 全屏沉浸式背景渲染 + 顶层悬浮胶囊导航与主画布 -->
    <TitleBar />

    <div
      class="skyline-workspace"
      :class="{
        'is-np-open': store.isNowPlayingOpen,
        'is-np-closing': isNpStageClosing,
      }"
    >
      <main
        ref="stageViewportRef"
        class="skyline-stage-viewport"
        :class="{
          'viewport-scrolled': stageScrollTop > 18 && !store.isNowPlayingOpen,
          'viewport-locked-settings':
            store.activeTab === 'settings' && !store.isNowPlayingOpen,
        }"
        @scroll.passive="handleStageScroll"
      >
        <!-- 左上角时段语句：仅在五个一级主页面顶部显示，在旧页退场完毕 (@before-enter) 时无缝切换，杜绝 60px 布局跳动 -->
        <div
          v-if="displayedGreetingVisible"
          class="inflow-top-greeting-row"
          data-tauri-drag-region
        >
          <div class="inflow-greeting-statement" data-tauri-drag-region>
            <span class="inflow-greeting-dot"></span>
            <span class="inflow-greeting-text" data-tauri-drag-region>
              <span class="inflow-greeting-lead">{{ greetingParts.lead }}</span>
              <span class="inflow-greeting-detail"> · {{ greetingParts.detail }}</span>
            </span>
          </div>
        </div>

        <transition :name="effectiveViewTransitionName" mode="out-in" @before-enter="handleStageBeforeEnter">
          <DiscoverView
            v-if="store.activeTab === 'discover'"
            key="discover"
          />
          <LibraryView
            v-else-if="store.activeTab === 'library'"
            key="library"
          />
          <RadioView
            v-else-if="store.activeTab === 'radio' || store.activeTab === 'tuner'"
            key="tuner"
          />
          <PlaylistDetailView
            v-else-if="store.activeTab === 'playlist_detail'"
            :key="`playlist-detail-${store.selectedPlaylist?.id || ''}`"
          />
          <ArtistDetailView
            v-else-if="store.activeTab === 'artist_detail'"
            :key="`artist-detail-${store.activeArtistDetail?.artistId || store.activeArtistDetail?.name || ''}`"
          />
          <AlbumDetailView
            v-else-if="store.activeTab === 'album_detail'"
            :key="`album-detail-${store.activeAlbumDetail?.albumId || store.activeAlbumDetail?.albumName || ''}`"
          />
          <CloudDriveView
            v-else-if="store.activeTab === 'cloud_drive'"
            key="cloud_drive"
          />
          <LocalMusicView
            v-else-if="store.activeTab === 'local_music'"
            key="local_music"
          />
          <SearchPlaceholderView
            v-else-if="store.activeTab === 'search'"
            key="search"
          />
          <SettingsPlaceholderView v-else key="settings" />
        </transition>
      </main>
    </div>

    <!-- 独立全屏常驻预热播放舞台浮层：与底层主视口完全解耦，零 DOM 销毁重建、零透明窗掉帧 -->
    <div
      class="np-stage-overlay-shell"
      :class="{
        'is-open': store.isNowPlayingOpen,
        'is-closing': isNpStageClosing,
      }"
      :aria-hidden="!store.isNowPlayingOpen"
    >
      <NowPlayingView />
    </div>

    <!-- 浏览器预览回退桌面歌词舱（Tauri 桌面端由独立透明置顶窗口 desktop_lyric 渲染，避免双实例重叠） -->
    <transition name="pop-spring">
      <div
        v-if="store.desktopLyricEnabled && !isTauriRuntime"
        ref="desktopLyricRef"
        class="floating-desktop-lyric-bar"
        :class="{
          locked: store.desktopLyricLocked,
          dragging: isDraggingDesktopLyric,
        }"
        :style="desktopLyricPositionStyle"
        @mousedown="onDesktopLyricMouseDown"
      >
        <div v-if="!store.desktopLyricLocked" class="fdl-top-toolbar">
          <span class="fdl-song-chip" data-tip="按住歌词栏任意空白处即可在屏幕内自由拖动">
            {{ store.currentSong?.title || "微风桌面歌词" }}
          </span>
          <div class="fdl-btn-cluster">
            <button class="fdl-icon-btn" data-tip="上一曲" @click.stop="store.playPrev()">
              <PlumpIcon name="prev" :size="13" />
            </button>
            <button class="fdl-icon-btn" data-tip="播放 / 暂停" @click.stop="store.togglePlayPause()">
              <PlumpIcon :name="store.isPlaying ? 'pause' : 'play'" :size="13" />
            </button>
            <button class="fdl-icon-btn" data-tip="下一曲" @click.stop="store.playNext(false)">
              <PlumpIcon name="next" :size="13" />
            </button>
            <button
              class="fdl-icon-btn"
              data-tip="缩短歌词舱宽度"
              @click.stop="store.desktopLyricWidth = Math.max(340, (store.desktopLyricWidth || 440) - 30)"
            >
              <PlumpIcon name="width-shrink" :size="13" />
            </button>
            <button
              class="fdl-icon-btn"
              data-tip="加长歌词舱宽度"
              @click.stop="store.desktopLyricWidth = Math.min(720, (store.desktopLyricWidth || 440) + 30)"
            >
              <PlumpIcon name="width-expand" :size="13" />
            </button>
            <button
              class="fdl-icon-btn"
              data-tip="字号缩小"
              @click.stop="store.desktopLyricFontSize = Math.max(18, (store.desktopLyricFontSize || 24) - 2)"
            >
              <PlumpIcon name="font-minus" :size="13" />
            </button>
            <button
              class="fdl-icon-btn"
              data-tip="字号放大"
              @click.stop="store.desktopLyricFontSize = Math.min(36, (store.desktopLyricFontSize || 24) + 2)"
            >
              <PlumpIcon name="font-plus" :size="13" />
            </button>
            <button
              class="fdl-icon-btn"
              data-tip="切换流光配色"
              @click.stop="
                store.desktopLyricColor = ['#7c6af6', '#fbbf24', '#fb7185', '#2dd4bf', '#38bdf8'][
                  (['#7c6af6', '#fbbf24', '#fb7185', '#2dd4bf', '#38bdf8'].indexOf(store.desktopLyricColor) + 1) % 5
                ]
              "
            >
              <PlumpIcon name="palette-orb" :size="13" />
            </button>
            <button
              class="fdl-icon-btn"
              data-tip="锁定桌面歌词"
              @click.stop="store.desktopLyricLocked = true"
            >
              <PlumpIcon name="lock" :size="13" />
            </button>
            <button
              class="fdl-icon-btn"
              data-tip="关闭桌面歌词"
              @click.stop="store.desktopLyricEnabled = false"
            >
              <PlumpIcon name="close" :size="13" />
            </button>
          </div>
        </div>

        <div v-else class="fdl-locked-unlock-pill">
          <button
            class="fdl-icon-btn"
            data-tip="解锁桌面歌词"
            @click.stop="store.desktopLyricLocked = false"
          >
            <PlumpIcon name="unlock" :size="13" />
          </button>
        </div>

        <div class="fdl-lines">
          <div
            v-if="store.lyricWordByWord && currentLyricPair.words.length > 0"
            class="fdl-main krc-words-row"
          >
            <span
              v-for="(w, wIdx) in currentLyricPair.words"
              :key="wIdx"
              class="fdl-krc-word"
              :style="{
                '--fill-pct': `${desktopWordFillPercent(
                  currentLyricPair.lineStartMs,
                  w.startTimeMs,
                  w.durationMs
                )}%`,
              }"
            >
              {{ w.text }}
            </span>
          </div>
          <div v-else class="fdl-main">{{ currentLyricPair.main }}</div>
          <div v-if="currentLyricPair.sub" class="fdl-sub">
            {{ currentLyricPair.sub }}
          </div>
        </div>
      </div>
    </transition>

    <PlayerBar />
    <NowPlayingQueuePopover />

    <!-- 全局模态层 -->
    <LoginModal />
    <VipCenterModal />
    <AccountProfileModal />
    <AccountManagerModal />
    <FocusPodModal />
    <LyricPosterModal />
    <SongContextMenu />
    <InputContextMenu />
    <ConfirmDialog />
    <CommentsModal />
    <MultiArtistModal />

    <!-- 全局高清专辑封面缩放预览与保存工作台 -->
    <HdArtworkPreviewModal />
    <AddToPlaylistModal />

    <!-- 全局酷狗双端唤醒分享弹窗（支持单曲 / 歌单） -->
    <ShareMusicModal />

    <!-- 剪贴板识别酷狗分享链接确认弹窗 -->
    <DetectedShareLinkModal />
  </div>
</template>

<style>
@media (min-width: 1600px) {
  .dual-column-song-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr)) !important;
  }
}

/* ============================================================================
   飞行门户黑胶中心标签细节层（压盘凹线 + 金属中心转轴孔 + 微弧反光）
   在 u=1 交接帧与 NowPlayingCoverStage .np-vinyl-label-art 像素级重合
   ============================================================================ */
.flip-flying-cover-inner {
  position: relative;
  transform-origin: 50% 50% !important;
}

.flip-flying-vinyl-overlay {
  position: absolute;
  inset: 0;
  border-radius: inherit;
  pointer-events: none;
  opacity: 0;
}

.flip-vinyl-press-groove {
  position: absolute;
  inset: 5.2%;
  border-radius: 50%;
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow:
    0 0 0 1px rgba(0, 0, 0, 0.28),
    inset 0 1px 1px rgba(0, 0, 0, 0.24);
  pointer-events: none;
}

.flip-vinyl-spindle-hole {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  width: 15px;
  height: 15px;
  border-radius: 50%;
  background: radial-gradient(
    circle at 36% 34%,
    #f8fafc 0%,
    #94a3b8 38%,
    #334155 72%,
    #0b0f17 100%
  );
  box-shadow:
    0 2px 5px rgba(0, 0, 0, 0.78),
    0 0 0 2.2px rgba(10, 14, 22, 0.86),
    0 0 0 3.2px rgba(226, 232, 240, 0.24),
    inset 0 1px 1.5px rgba(255, 255, 255, 0.65);
  pointer-events: none;
}

.flip-vinyl-spindle-hole::after {
  content: "";
  position: absolute;
  inset: 5px;
  border-radius: 50%;
  background: #070a10;
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.95);
}

.flip-vinyl-label-glass {
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: linear-gradient(
    138deg,
    rgba(255, 255, 255, 0.11) 0%,
    rgba(255, 255, 255, 0.02) 28%,
    transparent 52%
  );
  pointer-events: none;
}

/* ============================================================================
   播放页进出场与飞行封面过渡时长 (784ms / 756ms，较原时长提速 30%) 同步节奏覆盖
   确保背景蒙层、舞台内部组件与二次贝塞尔飞行封面的收起/展开节奏从容一致
   ============================================================================ */
.app-root .skyline-workspace {
  transition:
    transform 0.756s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.7s cubic-bezier(0.22, 1, 0.36, 1),
    border-radius 0.756s cubic-bezier(0.22, 1, 0.36, 1) !important;
}

.app-root .skyline-workspace.is-np-open {
  transition:
    transform 0.784s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.7s cubic-bezier(0.22, 1, 0.36, 1),
    border-radius 0.784s cubic-bezier(0.22, 1, 0.36, 1) !important;
}

.app-root .fixed-bottom-player-bar {
  transition:
    transform 0.644s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.56s cubic-bezier(0.22, 1, 0.36, 1) !important;
}

.app-root .fixed-bottom-player-bar.player-bar-hidden {
  transition:
    transform 0.672s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.574s cubic-bezier(0.22, 1, 0.36, 1) !important;
}

.app-root .np-stage-overlay-shell {
  transition:
    transform 0.756s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.672s cubic-bezier(0.22, 1, 0.36, 1),
    border-radius 0.756s cubic-bezier(0.22, 1, 0.36, 1),
    visibility 0s linear 0.784s !important;
}

.app-root .np-stage-overlay-shell.is-open {
  transition:
    transform 0.784s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.7s cubic-bezier(0.22, 1, 0.36, 1),
    border-radius 0.784s cubic-bezier(0.22, 1, 0.36, 1),
    visibility 0s linear 0s !important;
}

.app-root .np-stage-overlay-shell .np-ambient-backdrop {
  transition:
    transform 0.812s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.728s cubic-bezier(0.22, 1, 0.36, 1) !important;
}

.app-root .np-stage-overlay-shell .np-top-left-back-btn {
  transition:
    transform 0.728s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.644s cubic-bezier(0.22, 1, 0.36, 1) !important;
}

.app-root .np-stage-overlay-shell:not(.is-open) .np-skyline-bar {
  transition:
    opacity 0.616s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.7s cubic-bezier(0.22, 1, 0.36, 1),
    filter 0.616s cubic-bezier(0.22, 1, 0.36, 1) !important;
}

.app-root .np-stage-overlay-shell .np-cover-side-actions {
  transition:
    transform 0.756s cubic-bezier(0.22, 1, 0.36, 1) 0.042s,
    opacity 0.672s cubic-bezier(0.22, 1, 0.36, 1) 0.042s !important;
}

.app-root .np-stage-overlay-shell .np-console-stack {
  transition:
    transform 0.784s cubic-bezier(0.22, 1, 0.36, 1),
    opacity 0.686s cubic-bezier(0.22, 1, 0.36, 1),
    filter 0.728s cubic-bezier(0.22, 1, 0.36, 1) !important;
}

.app-root .np-stage-overlay-shell .np-right-stage {
  transition:
    transform 0.784s cubic-bezier(0.22, 1, 0.36, 1) 0.028s,
    opacity 0.7s cubic-bezier(0.22, 1, 0.36, 1) 0.028s,
    filter 0.756s cubic-bezier(0.22, 1, 0.36, 1) 0.028s !important;
}
</style>

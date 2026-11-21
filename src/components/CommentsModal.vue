<script setup lang="ts">
import {
  ref,
  computed,
  watch,
  nextTick,
  onBeforeUnmount,
} from "vue";
import { useAppStore } from "../stores/appStore";
import {
  kugouClient,
  formatCommentCount,
  formatCommentTimeText,
} from "../api/kugou";
import type { CommentPageResult, SongComment } from "../types";
import { hexWithAlpha } from "../utils/coverPalette";
import { useFloatingModalTheme } from "../composables/useFloatingModalTheme";
import {
  buildReplyClusterTree,
  isSameCommentAuthor,
  type CommentTreeNode,
} from "../utils/commentTree";
import PlumpIcon from "./PlumpIcon.vue";
import CommentTreeBranch from "./CommentTreeBranch.vue";
import RollingCounter from "./RollingCounter.vue";
import { triggerOneShotClass } from "../utils/microMotionFx";

const store = useAppStore();
const targetSong = computed(() => store.commentsModalSong || store.currentSong);

// ==================== 1. 四主题 + 封面自适应主题变量 ====================
const {
  isLightSurface,
  accentPrimary,
  accentSecondary,
  onAccentPrimary,
  modalSurfaceStyle,
} = useFloatingModalTheme("auto");

const floatingShellStyle = computed(() => {
  const primary = accentPrimary.value;
  const secondary = accentSecondary.value;
  return {
    ...modalSurfaceStyle.value,
    "--cm-accent": primary,
    "--cm-accent-2": secondary,
    "--cm-accent-3": secondary,
    "--cm-on-accent": onAccentPrimary.value,
    "--cm-accent-soft": hexWithAlpha(
      primary,
      isLightSurface.value ? 0.14 : 0.18
    ),
    "--cm-accent-soft-strong": hexWithAlpha(
      primary,
      isLightSurface.value ? 0.22 : 0.26
    ),
    "--cm-accent-glow": hexWithAlpha(
      primary,
      isLightSurface.value ? 0.22 : 0.32
    ),
    "--cm-accent-2-soft": hexWithAlpha(
      secondary,
      isLightSurface.value ? 0.13 : 0.16
    ),
    "--cm-card-bg": "var(--fm-card)",
    "--cm-card-hover": "var(--fm-card-hover)",
    "--cm-card-elevated": "var(--fm-card-elevated)",
    "--cm-card-lowest": "var(--fm-input-bg)",
    "--cm-border-subtle": "transparent",
    "--cm-border-highlight": "transparent",
    "--cm-text-primary": "var(--fm-text-1)",
    "--cm-text-secondary": "var(--fm-text-2)",
    "--cm-text-muted": "var(--fm-text-3)",
    "--cm-branch-stroke": isLightSurface.value
      ? "rgba(24, 34, 48, 0.22)"
      : "rgba(255, 255, 255, 0.22)",
    "--cm-branch-node-bg": isLightSurface.value
      ? "rgba(24, 34, 48, 0.12)"
      : "rgba(255, 255, 255, 0.16)",
    "--cm-branch-node-color": isLightSurface.value ? "#334155" : "#E2E8F0",
  } as Record<string, string>;
});

// ==================== 2. 状态管理与静默预加载缓冲 ====================
const bodyScrollRef = ref<HTMLElement | null>(null);
const mainSentinelRef = ref<HTMLElement | null>(null);
const floorSentinelRef = ref<HTMLElement | null>(null);
const composerInputRef = ref<HTMLInputElement | null>(null);

const mainPage = ref(1);
const mainPageSize = 30;
const mainHasMore = ref(true);
const isLoadingMoreMain = ref(false);
const isSwitchingSort = ref(false);
const localMainTotal = ref(0);
const resolvedSpecialChildId = ref("");
const savedMainScrollTop = ref(0);

let mainRequestSeq = 0;
let prefetchedMainPageNum = 0;
let prefetchedMainPromise: Promise<CommentPageResult | null> | null = null;
let prefetchedMainResult: CommentPageResult | null = null;
const inflightInlinePrefetchIds = new Set<string>();

// 楼中楼状态
const activeFloorRoot = ref<SongComment | null>(null);
const rawFloorReplies = ref<SongComment[]>([]);
const floorPage = ref(1);
const floorPageSize = 40;
const floorHasMore = ref(true);
const floorTotalCount = ref(0);
const isLoadingFloor = ref(false);
const isLoadingMoreFloor = ref(false);
const floorOnlyOwner = ref(false);
const highlightedReplyId = ref<string | null>(null);

let floorRequestSeq = 0;
let prefetchedFloorPageNum = 0;
let prefetchedFloorPromise: Promise<CommentPageResult | null> | null = null;
let prefetchedFloorResult: CommentPageResult | null = null;
let highlightTimer: number | null = null;
let mainPrefetchTimer: number | null = null;
let floorPrefetchTimer: number | null = null;

// 对话上下文链路与大图预览
const conversationChain = ref<SongComment[] | null>(null);
const previewImageUrl = ref<string | null>(null);

// 输入与表情面板
const commentInput = ref("");
const replyTarget = ref<SongComment | null>(null);
const showEmojiPicker = ref(false);
const isSubmitting = ref(false);

const emojiList = [
  "😀", "😂", "🥹", "😍", "🤩", "😭", "🥺", "🥳",
  "😎", "🤔", "🙌", "👍", "👏", "❤️", "🔥", "✨",
  "🎵", "🎶", "🎧", "🎸", "🎹", "🥁", "🌙", "🌊",
  "🌸", "🍻", "💯", "🙏", "💔", "🫡", "🫶", "🎉",
];

const effectiveTotalCount = computed(() => {
  const storeTotal = store.activeSongCommentTotal || 0;
  const localTotal = localMainTotal.value || 0;
  const listLen = store.activeSongComments.length;
  return Math.max(storeTotal, localTotal, listLen);
});

const formattedTotalBadge = computed(() =>
  formatCommentCount(effectiveTotalCount.value)
);

const effectiveFloorTotalCount = computed(() => {
  if (!activeFloorRoot.value) return 0;
  return Math.max(
    activeFloorRoot.value.replyCount || 0,
    floorTotalCount.value || 0,
    rawFloorReplies.value.length
  );
});

function formatCompactNum(n?: number | null): string {
  return formatCommentCount(n);
}

function resolveDisplayTime(cmt: SongComment): string {
  if (cmt.timeText && cmt.timeText.trim().length > 0) return cmt.timeText;
  return formatCommentTimeText(cmt.timestampMs, cmt.createdTime);
}

function getVipTagInfo(
  cmt: SongComment
): { text: string; kind: "svip" | "concept" | "vip" | "pack" } | null {
  if (cmt.svipLevel && cmt.svipLevel > 0) {
    return { text: `SVIP·${cmt.svipLevel}`, kind: "svip" };
  }
  if (cmt.vipLabel && cmt.vipLabel.trim()) {
    const lbl = cmt.vipLabel.trim();
    if (lbl.startsWith("SVIP")) return { text: lbl, kind: "svip" };
    if (lbl.includes("概念")) return { text: lbl, kind: "concept" };
    if (lbl.includes("音乐包")) return { text: lbl, kind: "pack" };
    return { text: lbl, kind: "vip" };
  }
  if (cmt.vipType === 6) return { text: "豪华VIP", kind: "svip" };
  if (cmt.yType && cmt.yType > 0) return { text: "概念VIP", kind: "concept" };
  if (cmt.vipType && cmt.vipType > 0) return { text: "VIP", kind: "vip" };
  if (cmt.mType && cmt.mType > 0) return { text: "音乐包", kind: "pack" };
  if (cmt.vipLevel && cmt.vipLevel > 0) {
    return {
      text: cmt.vipLevel >= 6 ? "豪华VIP" : `VIP ${cmt.vipLevel}`,
      kind: cmt.vipLevel >= 6 ? "svip" : "vip",
    };
  }
  return null;
}

// ==================== 3. 静默预加载与 IntersectionObserver 引擎 ====================
let mainSentinelObserver: IntersectionObserver | null = null;
let floorSentinelObserver: IntersectionObserver | null = null;
let inlinePreviewObserver: IntersectionObserver | null = null;

function clearPendingTimers() {
  if (highlightTimer !== null) {
    window.clearTimeout(highlightTimer);
    highlightTimer = null;
  }
  if (mainPrefetchTimer !== null) {
    window.clearTimeout(mainPrefetchTimer);
    mainPrefetchTimer = null;
  }
  if (floorPrefetchTimer !== null) {
    window.clearTimeout(floorPrefetchTimer);
    floorPrefetchTimer = null;
  }
}

function teardownObservers() {
  mainSentinelObserver?.disconnect();
  mainSentinelObserver = null;
  floorSentinelObserver?.disconnect();
  floorSentinelObserver = null;
  inlinePreviewObserver?.disconnect();
  inlinePreviewObserver = null;
}

function resetMainPagingState() {
  mainPage.value = 1;
  mainHasMore.value = true;
  isLoadingMoreMain.value = false;
  prefetchedMainPageNum = 0;
  prefetchedMainPromise = null;
  prefetchedMainResult = null;
}

function resetFloorPagingState() {
  floorPage.value = 1;
  floorHasMore.value = true;
  isLoadingMoreFloor.value = false;
  prefetchedFloorPageNum = 0;
  prefetchedFloorPromise = null;
  prefetchedFloorResult = null;
}

function syncMainHasMoreFromStore() {
  const len = store.activeSongComments.length;
  const total = effectiveTotalCount.value;
  if (len === 0) {
    mainHasMore.value = false;
    return;
  }
  if (total > 0) {
    mainHasMore.value = len < total && mainPage.value < 100;
  } else {
    mainHasMore.value = len >= 20 && mainPage.value < 100;
  }
}

function scheduleSilentMainPrefetch(nextPage: number) {
  const song = targetSong.value;
  if (
    !song ||
    !store.isCommentsModalOpen ||
    activeFloorRoot.value ||
    !mainHasMore.value ||
    nextPage > 100 ||
    prefetchedMainPageNum === nextPage
  ) {
    return;
  }
  const reqSeq = mainRequestSeq;
  const sort = store.commentSortType;
  prefetchedMainPageNum = nextPage;
  prefetchedMainResult = null;
  prefetchedMainPromise = kugouClient
    .getSongCommentsPaged(song, sort, nextPage, mainPageSize)
    .then((res) => {
      if (reqSeq !== mainRequestSeq) return null;
      prefetchedMainResult = res;
      return res;
    })
    .catch(() => null);
}

async function loadMoreMainComments() {
  const song = targetSong.value;
  if (
    !song ||
    activeFloorRoot.value ||
    isLoadingMoreMain.value ||
    isSwitchingSort.value ||
    !mainHasMore.value
  ) {
    return;
  }
  isLoadingMoreMain.value = true;
  const reqSeq = mainRequestSeq;
  const nextP = mainPage.value + 1;
  const sort = store.commentSortType;

  try {
    let res: CommentPageResult | null = null;
    if (prefetchedMainPageNum === nextP) {
      if (prefetchedMainResult) {
        res = prefetchedMainResult;
      } else if (prefetchedMainPromise) {
        res = await prefetchedMainPromise;
      }
    }
    if (!res) {
      res = await kugouClient.getSongCommentsPaged(song, sort, nextP, mainPageSize);
    }
    if (reqSeq !== mainRequestSeq || !res) return;

    if (res.total > 0) {
      localMainTotal.value = Math.max(localMainTotal.value, res.total);
      store.setActiveSongCommentTotal(
        Math.max(store.activeSongCommentTotal, res.total)
      );
    }
    if (res.specialChildId && !resolvedSpecialChildId.value) {
      resolvedSpecialChildId.value = res.specialChildId;
    }

    const seen = new Set(store.activeSongComments.map((c) => c.id));
    const unique = (res.comments || []).filter((c) => !seen.has(c.id));
    if (unique.length > 0) {
      store.activeSongComments.push(...unique);
    }
    mainPage.value = nextP;
    mainHasMore.value =
      Boolean(res.hasMore) &&
      (res.comments?.length || 0) > 0 &&
      nextP < 100 &&
      (unique.length > 0 || (res.comments?.length || 0) >= mainPageSize);

    if (mainHasMore.value) {
      if (mainPrefetchTimer !== null) window.clearTimeout(mainPrefetchTimer);
      mainPrefetchTimer = window.setTimeout(
        () => scheduleSilentMainPrefetch(nextP + 1),
        80
      );
    }
  } catch {
    mainHasMore.value = false;
  } finally {
    if (reqSeq === mainRequestSeq) {
      isLoadingMoreMain.value = false;
    }
  }
}

function scheduleSilentFloorPrefetch(nextPage: number) {
  const song = targetSong.value;
  const root = activeFloorRoot.value;
  if (
    !song ||
    !root ||
    !store.isCommentsModalOpen ||
    !floorHasMore.value ||
    nextPage > 100 ||
    prefetchedFloorPageNum === nextPage
  ) {
    return;
  }
  const reqSeq = floorRequestSeq;
  const scid = root.specialChildId || resolvedSpecialChildId.value || "";
  prefetchedFloorPageNum = nextPage;
  prefetchedFloorResult = null;
  prefetchedFloorPromise = kugouClient
    .getCommentFloorRepliesPaged(song, root.id, scid, nextPage, floorPageSize)
    .then((res) => {
      if (reqSeq !== floorRequestSeq) return null;
      prefetchedFloorResult = res;
      return res;
    })
    .catch(() => null);
}

async function loadMoreFloorReplies() {
  const song = targetSong.value;
  const root = activeFloorRoot.value;
  if (
    !song ||
    !root ||
    isLoadingFloor.value ||
    isLoadingMoreFloor.value ||
    !floorHasMore.value
  ) {
    return;
  }
  isLoadingMoreFloor.value = true;
  const reqSeq = floorRequestSeq;
  const nextP = floorPage.value + 1;
  const scid = root.specialChildId || resolvedSpecialChildId.value || "";

  try {
    let res: CommentPageResult | null = null;
    if (prefetchedFloorPageNum === nextP) {
      if (prefetchedFloorResult) {
        res = prefetchedFloorResult;
      } else if (prefetchedFloorPromise) {
        res = await prefetchedFloorPromise;
      }
    }
    if (!res) {
      res = await kugouClient.getCommentFloorRepliesPaged(
        song,
        root.id,
        scid,
        nextP,
        floorPageSize
      );
    }
    if (reqSeq !== floorRequestSeq || !res) return;

    const seen = new Set(rawFloorReplies.value.map((r) => r.id));
    const unique = (res.comments || []).filter((r) => !seen.has(r.id));
    if (unique.length > 0) {
      rawFloorReplies.value = [...rawFloorReplies.value, ...unique];
      store.floorRepliesMap[root.id] = rawFloorReplies.value;
    }
    if (res.total > 0) {
      floorTotalCount.value = Math.max(
        floorTotalCount.value,
        res.total,
        rawFloorReplies.value.length
      );
    }
    floorPage.value = nextP;
    floorHasMore.value =
      Boolean(res.hasMore) &&
      (res.comments?.length || 0) > 0 &&
      unique.length > 0 &&
      nextP < 100;

    if (floorHasMore.value) {
      if (floorPrefetchTimer !== null) window.clearTimeout(floorPrefetchTimer);
      floorPrefetchTimer = window.setTimeout(
        () => scheduleSilentFloorPrefetch(nextP + 1),
        80
      );
    }
  } catch {
    floorHasMore.value = false;
  } finally {
    if (reqSeq === floorRequestSeq) {
      isLoadingMoreFloor.value = false;
    }
  }
}

async function prefetchInlineRepliesForCard(
  commentId: string,
  expandAll = false
) {
  const song = targetSong.value;
  if (!song || !commentId || inflightInlinePrefetchIds.has(commentId)) return;
  const cmt = store.activeSongComments.find((c) => c.id === commentId);
  if (!cmt || cmt.replyCount <= 0) return;
  const cachedLen = store.floorRepliesMap[commentId]?.length || 0;
  if (!expandAll && ((cmt.replies && cmt.replies.length > 0) || cachedLen > 0)) {
    return;
  }
  if (expandAll && cachedLen >= Math.min(cmt.replyCount, 20)) {
    return;
  }
  inflightInlinePrefetchIds.add(commentId);
  try {
    const scid = cmt.specialChildId || resolvedSpecialChildId.value || "";
    const res = await kugouClient.getCommentFloorRepliesPaged(
      song,
      cmt.id,
      scid,
      1,
      expandAll ? 20 : 8
    );
    if (res.comments && res.comments.length > 0) {
      cmt.replies = res.comments.slice(0, 6);
      store.floorRepliesMap[cmt.id] = res.comments;
    }
  } catch {
    // 静默忽略后台预加载失败
  } finally {
    inflightInlinePrefetchIds.delete(commentId);
  }
}

function setupObservers() {
  teardownObservers();
  const rootEl = bodyScrollRef.value;
  if (!rootEl) return;

  if (mainSentinelRef.value && !activeFloorRoot.value) {
    mainSentinelObserver = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          loadMoreMainComments();
        }
      },
      { root: rootEl, rootMargin: "0px 0px 380px 0px", threshold: 0.01 }
    );
    mainSentinelObserver.observe(mainSentinelRef.value);
  }

  if (floorSentinelRef.value && activeFloorRoot.value) {
    floorSentinelObserver = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          loadMoreFloorReplies();
        }
      },
      { root: rootEl, rootMargin: "0px 0px 340px 0px", threshold: 0.01 }
    );
    floorSentinelObserver.observe(floorSentinelRef.value);
  }

  if (!activeFloorRoot.value) {
    inlinePreviewObserver = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            const cid = (entry.target as HTMLElement).dataset.cmtId;
            if (cid) {
              prefetchInlineRepliesForCard(cid);
              inlinePreviewObserver?.unobserve(entry.target);
            }
          }
        }
      },
      { root: rootEl, rootMargin: "220px 0px 220px 0px", threshold: 0.01 }
    );
    rootEl
      .querySelectorAll<HTMLElement>(".cm-root-card[data-need-preview='1']")
      .forEach((el) => inlinePreviewObserver?.observe(el));
  }
}

function handleBodyScroll() {
  const el = bodyScrollRef.value;
  if (!el) return;
  const distToBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
  if (distToBottom <= 360) {
    if (activeFloorRoot.value) {
      loadMoreFloorReplies();
    } else {
      loadMoreMainComments();
    }
  }
}

watch(
  () => [
    store.isCommentsModalOpen,
    activeFloorRoot.value?.id,
    store.activeSongComments.length,
    rawFloorReplies.value.length,
  ],
  async () => {
    if (!store.isCommentsModalOpen) {
      teardownObservers();
      return;
    }
    await nextTick();
    setupObservers();
  }
);

function resetCommentsSongSession(fromSongSwitch: boolean) {
  mainRequestSeq += 1;
  floorRequestSeq += 1;
  clearPendingTimers();
  isSwitchingSort.value = false;
  resolvedSpecialChildId.value = "";
  activeFloorRoot.value = null;
  rawFloorReplies.value = [];
  floorTotalCount.value = 0;
  isLoadingFloor.value = false;
  floorOnlyOwner.value = false;
  highlightedReplyId.value = null;
  savedMainScrollTop.value = 0;
  conversationChain.value = null;
  previewImageUrl.value = null;
  replyTarget.value = null;
  showEmojiPicker.value = false;
  collapsedBranchIds.value = new Set();
  expandedMainTreeIds.value = new Set();
  inflightInlinePrefetchIds.clear();
  localMainTotal.value = fromSongSwitch ? 0 : store.activeSongCommentTotal || 0;
  resetMainPagingState();
  resetFloorPagingState();
  syncMainHasMoreFromStore();
  if (
    mainHasMore.value &&
    store.activeSongComments.length > 0 &&
    !store.isLoadingComments
  ) {
    if (mainPrefetchTimer !== null) window.clearTimeout(mainPrefetchTimer);
    mainPrefetchTimer = window.setTimeout(
      () => scheduleSilentMainPrefetch(2),
      140
    );
  }
  nextTick(() => {
    if (bodyScrollRef.value) bodyScrollRef.value.scrollTop = 0;
  });
}

function handleGlobalKeydown(e: KeyboardEvent) {
  if (e.key !== "Escape" || !store.isCommentsModalOpen) return;
  if (previewImageUrl.value) {
    previewImageUrl.value = null;
  } else if (conversationChain.value) {
    conversationChain.value = null;
  } else if (activeFloorRoot.value) {
    exitFloorDetail();
  } else {
    store.closeCommentsModal();
  }
}

watch(
  () => store.isCommentsModalOpen,
  (open) => {
    if (open) {
      resetCommentsSongSession(false);
      window.addEventListener("keydown", handleGlobalKeydown);
    } else {
      window.removeEventListener("keydown", handleGlobalKeydown);
      clearPendingTimers();
      teardownObservers();
    }
  }
);

watch(
  () => [targetSong.value?.hash || "", targetSong.value?.id || ""] as const,
  ([newHash, newId], [oldHash, oldId]) => {
    if (!store.isCommentsModalOpen) return;
    if (newHash === oldHash && newId === oldId) return;
    resetCommentsSongSession(true);
  }
);

watch(
  () => [store.isLoadingComments, store.activeSongComments] as const,
  ([loading, list]) => {
    if (!store.isCommentsModalOpen || activeFloorRoot.value || loading) return;
    if (list.length > 0) {
      const t = Number(list[0]?.totalCount || 0);
      if (t > 0) {
        const mergedTotal = Math.max(store.activeSongCommentTotal || 0, t);
        localMainTotal.value = mergedTotal;
        store.setActiveSongCommentTotal(mergedTotal);
      } else {
        localMainTotal.value = store.activeSongCommentTotal || list.length;
      }
      if (list[0]?.specialChildId && !resolvedSpecialChildId.value) {
        resolvedSpecialChildId.value = list[0].specialChildId;
      }
    } else {
      localMainTotal.value = store.activeSongCommentTotal || 0;
      resolvedSpecialChildId.value = "";
    }
    syncMainHasMoreFromStore();
    if (mainHasMore.value && mainPage.value === 1) {
      if (mainPrefetchTimer !== null) window.clearTimeout(mainPrefetchTimer);
      mainPrefetchTimer = window.setTimeout(
        () => scheduleSilentMainPrefetch(2),
        140
      );
    }
  },
  { deep: false }
);

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleGlobalKeydown);
  clearPendingTimers();
  teardownObservers();
});

// ==================== 4. 排序切换与全新“评论回复详情”视图 ====================
async function switchSort(sort: "hot" | "new") {
  const song = targetSong.value;
  if (
    !song ||
    (store.commentSortType === sort &&
      !isSwitchingSort.value &&
      store.activeSongComments.length > 0)
  ) {
    return;
  }
  mainRequestSeq += 1;
  const reqSeq = mainRequestSeq;
  store.commentSortType = sort;
  resetMainPagingState();
  isSwitchingSort.value = true;
  store.isLoadingComments = true;
  if (bodyScrollRef.value) bodyScrollRef.value.scrollTop = 0;

  try {
    const res = await kugouClient.getSongCommentsPaged(
      song,
      sort,
      1,
      mainPageSize
    );
    if (reqSeq !== mainRequestSeq) return;
    store.activeSongComments = res.comments || [];
    if (res.total > 0) {
      localMainTotal.value = res.total;
      store.setActiveSongCommentTotal(res.total);
    } else if (res.comments.length === 0 && sort === "hot") {
      localMainTotal.value = 0;
      store.setActiveSongCommentTotal(0);
    }
    if (res.specialChildId) {
      resolvedSpecialChildId.value = res.specialChildId;
    }
    mainHasMore.value = Boolean(res.hasMore);
    if (mainHasMore.value) {
      if (mainPrefetchTimer !== null) window.clearTimeout(mainPrefetchTimer);
      mainPrefetchTimer = window.setTimeout(
        () => scheduleSilentMainPrefetch(2),
        120
      );
    }
  } catch {
    if (reqSeq === mainRequestSeq) {
      store.activeSongComments = [];
      mainHasMore.value = false;
    }
  } finally {
    if (reqSeq === mainRequestSeq) {
      isSwitchingSort.value = false;
      store.isLoadingComments = false;
    }
  }
}

async function openFloorDetail(rootCmt: SongComment) {
  savedMainScrollTop.value = bodyScrollRef.value?.scrollTop || 0;
  floorRequestSeq += 1;
  const reqSeq = floorRequestSeq;
  activeFloorRoot.value = rootCmt;
  replyTarget.value = rootCmt;
  floorOnlyOwner.value = false;
  highlightedReplyId.value = null;
  resetFloorPagingState();

  const warmCached = store.floorRepliesMap[rootCmt.id]?.length
    ? store.floorRepliesMap[rootCmt.id]
    : rootCmt.replies?.length
    ? rootCmt.replies
    : [];
  rawFloorReplies.value = [...warmCached];
  floorTotalCount.value = Math.max(rootCmt.replyCount || 0, warmCached.length);

  nextTick(() => {
    if (bodyScrollRef.value) bodyScrollRef.value.scrollTop = 0;
  });

  const song = targetSong.value;
  if (!song) return;

  isLoadingFloor.value = true;
  try {
    const scid = rootCmt.specialChildId || resolvedSpecialChildId.value || "";
    const res = await kugouClient.getCommentFloorRepliesPaged(
      song,
      rootCmt.id,
      scid,
      1,
      floorPageSize
    );
    if (reqSeq !== floorRequestSeq) return;
    const list = res.comments || [];
    if (list.length > 0) {
      rawFloorReplies.value = list;
      store.floorRepliesMap[rootCmt.id] = list;
    }
    floorTotalCount.value = Math.max(
      rootCmt.replyCount || 0,
      res.total || 0,
      rawFloorReplies.value.length
    );
    floorHasMore.value = Boolean(res.hasMore);
    if (floorHasMore.value) {
      if (floorPrefetchTimer !== null) window.clearTimeout(floorPrefetchTimer);
      floorPrefetchTimer = window.setTimeout(
        () => scheduleSilentFloorPrefetch(2),
        120
      );
    }
  } catch {
    if (reqSeq === floorRequestSeq) {
      floorHasMore.value = false;
    }
  } finally {
    if (reqSeq === floorRequestSeq) {
      isLoadingFloor.value = false;
    }
  }
}

function exitFloorDetail() {
  floorRequestSeq += 1;
  activeFloorRoot.value = null;
  replyTarget.value = null;
  rawFloorReplies.value = [];
  floorOnlyOwner.value = false;
  highlightedReplyId.value = null;
  resetFloorPagingState();
  const restoreTop = savedMainScrollTop.value;
  nextTick(() => {
    if (bodyScrollRef.value) {
      bodyScrollRef.value.scrollTop = restoreTop;
    }
  });
}

// ==================== 5. 树状分支回复 Clustering 与折叠状态 ====================
const collapsedBranchIds = ref<Set<string>>(new Set());
const expandedMainTreeIds = ref<Set<string>>(new Set());

function toggleBranchCollapse(commentId: string) {
  const next = new Set(collapsedBranchIds.value);
  if (next.has(commentId)) {
    next.delete(commentId);
  } else {
    next.add(commentId);
  }
  collapsedBranchIds.value = next;
}

const ownerFloorReplyCount = computed(() => {
  const root = activeFloorRoot.value;
  if (!root) return 0;
  return rawFloorReplies.value.filter((r) => isSameCommentAuthor(r, root)).length;
});

const floorTreeNodes = computed<CommentTreeNode[]>(() => {
  const root = activeFloorRoot.value;
  if (!root) return [];
  const sourceList = floorOnlyOwner.value
    ? rawFloorReplies.value.filter((r) => isSameCommentAuthor(r, root))
    : rawFloorReplies.value;
  return buildReplyClusterTree(root, sourceList);
});

function getCommentReplySource(cmt: SongComment): SongComment[] {
  const cached = store.floorRepliesMap[cmt.id];
  if (cached && cached.length > 0) return cached;
  if (cmt.replies && cmt.replies.length > 0) return cmt.replies;
  return [];
}

function countTreeNodes(list: CommentTreeNode[]): number {
  return list.reduce((sum, n) => sum + n.descendantCount, 0);
}

function getMainCardTreeState(cmt: SongComment): {
  visibleRoots: CommentTreeNode[];
  isExpanded: boolean;
  canExpandMore: boolean;
  totalRepliesCount: number;
} {
  const source = getCommentReplySource(cmt);
  const allRoots = buildReplyClusterTree(cmt, source);
  const isExpanded = expandedMainTreeIds.value.has(cmt.id);
  const totalRepliesCount = Math.max(cmt.replyCount || 0, source.length);

  if (isExpanded) {
    return {
      visibleRoots: allRoots,
      isExpanded: true,
      canExpandMore: allRoots.length > 2 || totalRepliesCount > countTreeNodes(allRoots),
      totalRepliesCount,
    };
  }

  // 默认展示前 2 条高赞直接分支，保留局部嵌套对话结构
  const topBranches = allRoots
    .slice()
    .sort((a, b) => (b.comment.likeCount || 0) - (a.comment.likeCount || 0))
    .slice(0, 2);

  const visibleCount = countTreeNodes(topBranches);
  const canExpandMore =
    allRoots.length > 2 || totalRepliesCount > visibleCount;

  return {
    visibleRoots: topBranches,
    isExpanded: false,
    canExpandMore,
    totalRepliesCount,
  };
}

async function toggleMainTreeExpand(cmt: SongComment) {
  const next = new Set(expandedMainTreeIds.value);
  if (next.has(cmt.id)) {
    next.delete(cmt.id);
    expandedMainTreeIds.value = next;
    return;
  }
  next.add(cmt.id);
  expandedMainTreeIds.value = next;
  await prefetchInlineRepliesForCard(cmt.id, true);
}

function jumpToReplyCard(targetId: string | null) {
  const container = bodyScrollRef.value;
  if (!container || !targetId) return;
  const escapedId =
    typeof CSS !== "undefined" && typeof CSS.escape === "function"
      ? CSS.escape(targetId)
      : targetId.replace(/"/g, '\\"');
  const targetEl = container.querySelector<HTMLElement>(
    `.cm-reply-card[data-reply-id="${escapedId}"]`
  );
  if (!targetEl) return;
  targetEl.scrollIntoView({ behavior: "smooth", block: "center" });
  highlightedReplyId.value = targetId;
  if (highlightTimer !== null) window.clearTimeout(highlightTimer);
  highlightTimer = window.setTimeout(() => {
    highlightedReplyId.value = null;
    highlightTimer = null;
  }, 1800);
}

// ==================== 6. 对话上下文链路、内联预览与发评 ====================
function openConversationModal(reply: SongComment) {
  if (!activeFloorRoot.value) return;
  const chain: SongComment[] = [activeFloorRoot.value];
  const byId = new Map<string, SongComment>();
  byId.set(activeFloorRoot.value.id, activeFloorRoot.value);
  for (const r of rawFloorReplies.value) {
    byId.set(r.id, r);
  }

  const ancestorStack: SongComment[] = [];
  let cursor: SongComment | undefined = reply;
  const visited = new Set<string>([reply.id]);
  while (cursor && ancestorStack.length < 8) {
    const pid = (cursor.replyTargetCommentId || cursor.parentId || "").trim();
    if (
      !pid ||
      pid === "0" ||
      pid === activeFloorRoot.value.id ||
      visited.has(pid)
    ) {
      break;
    }
    visited.add(pid);
    const parent = byId.get(pid);
    if (!parent) break;
    ancestorStack.unshift(parent);
    cursor = parent;
  }

  const topCursor = ancestorStack[0] || reply;
  if (
    ancestorStack.length === 0 &&
    topCursor.replyChainHops &&
    topCursor.replyChainHops.length > 0
  ) {
    for (let i = topCursor.replyChainHops.length - 1; i >= 0; i--) {
      const hop = topCursor.replyChainHops[i];
      if (hop.userName === activeFloorRoot.value.userName) continue;
      chain.push({
        id: `hop_${i}`,
        userName: hop.userName,
        userAvatar: "",
        content: hop.content,
        createdTime: "历史引用",
        timeText: "历史引用",
        likeCount: 0,
        isLiked: false,
        location: "",
        replyCount: 0,
        specialChildId: "",
        replies: [],
      });
    }
  }

  chain.push(...ancestorStack);
  chain.push(reply);
  conversationChain.value = chain;
}

function selectReplyTarget(cmt: SongComment, focusInput = true) {
  replyTarget.value = cmt;
  if (focusInput) {
    nextTick(() => {
      composerInputRef.value?.focus();
    });
  }
}

function clearExplicitReplyTarget() {
  replyTarget.value = activeFloorRoot.value || null;
}

function insertEmoji(em: string) {
  commentInput.value += em;
  nextTick(() => {
    composerInputRef.value?.focus();
  });
}

async function handleSendComment() {
  const text = commentInput.value.trim();
  if (!text || isSubmitting.value) return;
  isSubmitting.value = true;
  try {
    const targetId = replyTarget.value?.id || activeFloorRoot.value?.id;
    const ok = await store.submitSongComment(text, targetId);
    if (ok) {
      if (activeFloorRoot.value) {
        const targetObj = replyTarget.value || activeFloorRoot.value;
        const isReplyingSub = targetObj.id !== activeFloorRoot.value.id;
        const newReply: SongComment = {
          id: `local_${Date.now()}`,
          userId: store.profile?.userId || store.session?.userId || "",
          userName: store.profile?.nickname || store.session?.nickname || "我",
          userAvatar:
            store.profile?.avatarUrl || store.session?.avatarUrl || "",
          content: text,
          createdTime: "刚刚",
          timeText: "刚刚",
          likeCount: 0,
          isLiked: false,
          location: store.profile?.ipLocation || "",
          replyCount: 0,
          specialChildId: activeFloorRoot.value.specialChildId || "",
          replies: [],
          replyToUser: targetObj.userName,
          replyUserName: targetObj.userName,
          replyQuotedContent: isReplyingSub ? targetObj.content : null,
          replyTargetCommentId: isReplyingSub ? targetObj.id : null,
          timestampMs: Date.now(),
        };
        rawFloorReplies.value = [newReply, ...rawFloorReplies.value];
        store.floorRepliesMap[activeFloorRoot.value.id] = rawFloorReplies.value;
        activeFloorRoot.value.replyCount += 1;
        floorTotalCount.value += 1;
      } else {
        localMainTotal.value = Math.max(
          localMainTotal.value + 1,
          store.activeSongComments.length
        );
        replyTarget.value = null;
      }
      commentInput.value = "";
      showEmojiPicker.value = false;
    }
  } finally {
    isSubmitting.value = false;
  }
}
</script>

<template>
  <!-- 透明非覆盖式乐评悬浮窗 -->
  <transition name="cm-float-spring">
    <div
      v-if="store.isCommentsModalOpen"
      class="comments-floating-layer"
      @click.self="store.closeCommentsModal()"
    >
      <div
        class="comments-modal-shell"
        :class="{ 'is-dark-surface': !isLightSurface }"
        :style="floatingShellStyle"
        @click.stop
      >
        <!-- ==================== 顶部导航与歌曲上下文栏 ==================== -->
        <div class="cm-header" :class="{ 'is-floor-mode': Boolean(activeFloorRoot) }">
          <!-- 楼中楼回复详情导航栏 -->
          <template v-if="activeFloorRoot">
            <div class="cm-header-left">
              <button
                class="cm-back-all-btn"
                title="返回全部评论"
                aria-label="返回全部评论"
                @click="exitFloorDetail()"
              >
                <PlumpIcon name="arrow-left" :size="15" />
                <span>返回全部评论</span>
              </button>

              <div v-if="targetSong" class="cm-floor-song-context">
                <img
                  v-if="targetSong.coverUrl"
                  :src="targetSong.coverUrl"
                  class="cm-song-cover-mini"
                  alt="cover"
                />
                <div class="cm-floor-song-text">
                  <span class="cm-floor-nav-title">评论回复详情</span>
                  <span class="cm-floor-nav-sub">
                    {{ targetSong.title }} · {{ targetSong.artist }}
                  </span>
                </div>
              </div>
            </div>

            <div class="cm-header-right">
              <span class="cm-floor-count-pill">
                <PlumpIcon name="comment" :size="12" />
                <span>共 {{ formatCompactNum(effectiveFloorTotalCount) }} 条回复</span>
              </span>
              <button
                class="icon-close-btn cm-close-btn"
                title="关闭评论弹窗"
                @click="store.closeCommentsModal()"
              >
                <PlumpIcon name="close" :size="16" />
              </button>
            </div>
          </template>

          <!-- 主评论列表顶部栏 -->
          <template v-else>
            <div class="cm-header-left">
              <div v-if="targetSong" class="cm-song-pill">
                <img
                  v-if="targetSong.coverUrl"
                  :src="targetSong.coverUrl"
                  class="cm-song-cover"
                  alt="cover"
                />
                <div class="cm-song-meta">
                  <div class="cm-song-title">
                    <span class="cm-song-name-text">{{ targetSong.title }}</span>
                    <span v-if="targetSong.isVip" class="mini-badge vip">VIP</span>
                    <span v-if="targetSong.isHiRes" class="mini-badge hires">Hi-Res</span>
                    <span v-else-if="targetSong.isSq" class="mini-badge sq">SQ</span>
                    <span class="cm-total-pill">
                      <PlumpIcon name="comment" :size="11" />
                      <span>{{ formattedTotalBadge }} 条乐评</span>
                    </span>
                  </div>
                  <div class="cm-song-sub">
                    <button
                      class="cm-link-inline"
                      @click="store.closeCommentsModal(); store.navigateToSongArtist(targetSong)"
                    >
                      {{ targetSong.artist }}
                    </button>
                    <template v-if="targetSong.album">
                      <span class="cm-sub-dot"> · </span>
                      <button
                        class="cm-link-inline"
                        @click="store.closeCommentsModal(); store.navigateToSongAlbum(targetSong)"
                      >
                        {{ targetSong.album }}
                      </button>
                    </template>
                  </div>
                </div>
              </div>
            </div>

            <div class="cm-header-right">
              <div class="cm-sort-capsule" role="tablist" aria-label="评论排序">
                <button
                  type="button"
                  role="tab"
                  class="cm-sort-btn"
                  :class="{ active: store.commentSortType === 'hot' }"
                  :aria-selected="store.commentSortType === 'hot'"
                  @click="switchSort('hot')"
                >
                  <PlumpIcon name="flame" :size="12" />
                  <span>最热评论</span>
                </button>
                <button
                  type="button"
                  role="tab"
                  class="cm-sort-btn"
                  :class="{ active: store.commentSortType === 'new' }"
                  :aria-selected="store.commentSortType === 'new'"
                  @click="switchSort('new')"
                >
                  <PlumpIcon name="clock" :size="12" />
                  <span>最新评论</span>
                </button>
              </div>

              <button
                class="icon-close-btn cm-close-btn"
                title="关闭评论弹窗"
                @click="store.closeCommentsModal()"
              >
                <PlumpIcon name="close" :size="16" />
              </button>
            </div>
          </template>
        </div>

        <!-- ==================== 主体滚动区 ==================== -->
        <div
          ref="bodyScrollRef"
          class="cm-body-scroll"
          @scroll.passive="handleBodyScroll"
        >
          <!-- A. 全新“查看评论回复详情”视图 -->
          <div
            v-if="activeFloorRoot"
            :key="`floor_${activeFloorRoot.id}`"
            class="cm-floor-view cm-view-enter"
          >
            <!-- 原评论（楼主）完整卡片 -->
            <section class="cm-floor-owner-card">
              <div class="cm-owner-eyebrow">
                <span class="cm-owner-pill-strong">原评 · 楼主</span>
                <span class="cm-owner-reply-total">
                  获赞 {{ formatCompactNum(activeFloorRoot.likeCount) }} · 共 {{ formatCompactNum(effectiveFloorTotalCount) }} 条讨论
                </span>
              </div>

              <div class="cm-card-top">
                <div class="cm-avatar-wrap lg">
                  <img
                    v-if="activeFloorRoot.userAvatar"
                    :src="activeFloorRoot.userAvatar"
                    class="cm-avatar lg"
                    alt="avatar"
                  />
                  <div v-else class="cm-avatar-fallback lg">
                    {{ activeFloorRoot.userName.slice(0, 1) }}
                  </div>
                  <img
                    v-if="activeFloorRoot.pendantUrl"
                    :src="activeFloorRoot.pendantUrl"
                    class="cm-avatar-pendant"
                    alt=""
                  />
                </div>

                <div class="cm-user-col">
                  <div class="cm-user-row">
                    <span class="cm-username lg">{{ activeFloorRoot.userName }}</span>
                    <span
                      v-if="activeFloorRoot.isHot || activeFloorRoot.likeCount >= 100"
                      class="cm-hot-badge"
                    >
                      HOT 热评
                    </span>
                    <span
                      v-if="getVipTagInfo(activeFloorRoot)"
                      class="cm-vip-tag"
                      :class="getVipTagInfo(activeFloorRoot)?.kind"
                    >
                      {{ getVipTagInfo(activeFloorRoot)?.text }}
                    </span>
                    <span v-if="activeFloorRoot.isCmtTalent" class="cm-talent-tag">
                      乐评达人
                    </span>
                    <span v-if="activeFloorRoot.isSinger" class="cm-singer-tag">
                      音乐人
                    </span>
                    <img
                      v-if="activeFloorRoot.nameplateUrl"
                      :src="activeFloorRoot.nameplateUrl"
                      class="cm-nameplate-icon"
                      alt=""
                    />
                  </div>
                  <div class="cm-meta-row">
                    <span :title="activeFloorRoot.createdTime">
                      {{ resolveDisplayTime(activeFloorRoot) }}
                    </span>
                    <span v-if="activeFloorRoot.location" class="cm-loc-chip">
                      · IP属地 {{ activeFloorRoot.location }}
                    </span>
                  </div>
                </div>

                <div class="cm-reply-actions">
                  <button
                    class="cm-action-pill"
                    :class="{
                      active: replyTarget?.id === activeFloorRoot.id,
                    }"
                    @click="selectReplyTarget(activeFloorRoot, true)"
                  >
                    <PlumpIcon name="comment" :size="12" />
                    <span>回复楼主</span>
                  </button>
                  <button
                    class="cm-like-btn"
                    :class="{ liked: activeFloorRoot.isLiked }"
                    @click="store.toggleLikeComment(activeFloorRoot)"
                  >
                    <PlumpIcon
                      :name="activeFloorRoot.isLiked ? 'heart-filled' : 'heart'"
                      :size="15"
                    />
                    <span>{{ formatCompactNum(activeFloorRoot.likeCount) }}</span>
                  </button>
                </div>
              </div>

              <div class="cm-owner-body-wrap">
                <div
                  class="cm-content-text lg"
                  @click="selectReplyTarget(activeFloorRoot, true)"
                >
                  {{ activeFloorRoot.content }}
                </div>

                <div
                  v-if="activeFloorRoot.images && activeFloorRoot.images.length > 0"
                  class="cm-img-grid"
                >
                  <img
                    v-for="(imgUrl, idx) in activeFloorRoot.images"
                    :key="idx"
                    :src="imgUrl"
                    class="cm-comment-img"
                    alt="comment"
                    @click="previewImageUrl = imgUrl"
                  />
                </div>
              </div>
            </section>

            <!-- 清晰视觉分隔与全部回复区标题栏 -->
            <div class="cm-floor-section-bar">
              <div class="cm-floor-section-left">
                <span class="cm-section-accent-bar"></span>
                <span class="cm-section-title">全部回复</span>
                <span class="cm-section-count">
                  {{ formatCompactNum(floorOnlyOwner ? ownerFloorReplyCount : effectiveFloorTotalCount) }}
                </span>
              </div>

              <div class="cm-floor-section-right">
                <div
                  v-if="ownerFloorReplyCount > 0"
                  class="cm-owner-filter-capsule"
                  role="tablist"
                  aria-label="回复筛选"
                >
                  <button
                    type="button"
                    role="tab"
                    class="cm-owner-filter-btn"
                    :class="{ active: !floorOnlyOwner }"
                    :aria-selected="!floorOnlyOwner"
                    @click="floorOnlyOwner = false"
                  >
                    全部回复
                  </button>
                  <button
                    type="button"
                    role="tab"
                    class="cm-owner-filter-btn"
                    :class="{ active: floorOnlyOwner }"
                    :aria-selected="floorOnlyOwner"
                    @click="floorOnlyOwner = true"
                  >
                    只看楼主 ({{ ownerFloorReplyCount }})
                  </button>
                </div>
                <span v-else class="cm-floor-hint-text">
                  点击任意回复即可直接针对该条回复
                </span>
              </div>
            </div>

            <!-- 全部子回复列表 -->
            <div
              v-if="isLoadingFloor && rawFloorReplies.length === 0"
              class="cm-skeleton-list"
            >
              <div v-for="idx in 4" :key="idx" class="cm-skeleton-card">
                <div class="cm-sk-avatar sm"></div>
                <div class="cm-sk-body">
                  <div class="cm-sk-line short"></div>
                  <div class="cm-sk-line long"></div>
                </div>
              </div>
            </div>

            <div
              v-else-if="floorTreeNodes.length === 0"
              class="cm-empty-state compact"
            >
              {{
                floorOnlyOwner
                  ? "当前已加载的回复中，楼主暂未再发言"
                  : "暂无更多回复，来写下你的看法吧"
              }}
            </div>

            <div v-else class="cm-floor-reply-list cm-floor-tree-wrap">
              <CommentTreeBranch
                v-if="activeFloorRoot && floorTreeNodes.length > 0"
                :nodes="floorTreeNodes"
                :root-owner="activeFloorRoot"
                :collapsed-ids="collapsedBranchIds"
                :highlighted-id="highlightedReplyId"
                :reply-target-id="replyTarget?.id"
                @toggle-collapse="toggleBranchCollapse"
                @toggle-like="store.toggleLikeComment($event)"
                @reply="selectReplyTarget($event, true)"
                @open-chain="openConversationModal($event)"
                @preview-image="previewImageUrl = $event"
              />
            </div>

            <!-- 楼中楼自动静默加载哨兵 -->
            <div ref="floorSentinelRef" class="cm-scroll-sentinel">
              <span v-if="isLoadingMoreFloor" class="cm-silent-loading-dot">
                正在加载更多回复...
              </span>
              <span
                v-else-if="!floorHasMore && rawFloorReplies.length > 0"
                class="cm-end-caption"
              >
                已展示全部 {{ rawFloorReplies.length }} 条回复
              </span>
            </div>
          </div>

          <!-- B. 主评论列表视图 -->
          <div v-else key="main_comments_view" class="cm-main-view cm-view-enter">
            <div
              v-if="store.isLoadingComments && store.activeSongComments.length === 0"
              class="cm-skeleton-list"
            >
              <div v-for="idx in 4" :key="idx" class="cm-skeleton-card">
                <div class="cm-sk-avatar"></div>
                <div class="cm-sk-body">
                  <div class="cm-sk-line short"></div>
                  <div class="cm-sk-line long"></div>
                  <div class="cm-sk-line mid"></div>
                </div>
              </div>
            </div>

            <div
              v-else-if="store.activeSongComments.length === 0"
              class="cm-empty-state"
            >
              这首歌还没有人留下足迹，快来写下第一条乐评吧 ✨
            </div>

            <div v-else class="cm-main-list">
              <article
                v-for="cmt in store.activeSongComments"
                :key="cmt.id"
                :data-cmt-id="cmt.id"
                :data-need-preview="
                  cmt.replyCount > 0 &&
                  (!cmt.replies || cmt.replies.length === 0) &&
                  !store.floorRepliesMap[cmt.id]?.length
                    ? '1'
                    : '0'
                "
                class="cm-root-card"
                :class="{ 'is-target': replyTarget?.id === cmt.id }"
              >
                <div class="cm-card-top">
                  <div class="cm-avatar-wrap">
                    <img
                      v-if="cmt.userAvatar"
                      :src="cmt.userAvatar"
                      class="cm-avatar"
                      loading="lazy"
                      decoding="async"
                      alt="avatar"
                    />
                    <div v-else class="cm-avatar-fallback">
                      {{ cmt.userName.slice(0, 1) }}
                    </div>
                    <img
                      v-if="cmt.pendantUrl"
                      :src="cmt.pendantUrl"
                      class="cm-avatar-pendant"
                      loading="lazy"
                      decoding="async"
                      alt=""
                    />
                  </div>

                  <div class="cm-user-col">
                    <div class="cm-user-row">
                      <span class="cm-username">{{ cmt.userName }}</span>
                      <span
                        v-if="cmt.isHot || cmt.likeCount >= 100"
                        class="cm-hot-badge"
                      >
                        HOT 热评
                      </span>
                      <span
                        v-if="getVipTagInfo(cmt)"
                        class="cm-vip-tag"
                        :class="getVipTagInfo(cmt)?.kind"
                      >
                        {{ getVipTagInfo(cmt)?.text }}
                      </span>
                      <span v-if="cmt.isCmtTalent" class="cm-talent-tag">
                        乐评达人
                      </span>
                      <span v-if="cmt.isSinger" class="cm-singer-tag">
                        音乐人
                      </span>
                      <img
                        v-if="cmt.nameplateUrl"
                        :src="cmt.nameplateUrl"
                        class="cm-nameplate-icon"
                        loading="lazy"
                        decoding="async"
                        alt=""
                      />
                    </div>
                    <div class="cm-meta-row">
                      <span :title="cmt.createdTime">
                        {{ resolveDisplayTime(cmt) }}
                      </span>
                      <span v-if="cmt.location" class="cm-loc-chip">
                        · IP属地 {{ cmt.location }}
                      </span>
                    </div>
                  </div>

                  <div class="cm-reply-actions">
                    <button
                      class="cm-action-pill"
                      :class="{ active: replyTarget?.id === cmt.id }"
                      title="回复该乐评"
                      @click="selectReplyTarget(cmt, true)"
                    >
                      <PlumpIcon name="comment" :size="12" />
                      <span>
                        {{
                          cmt.replyCount > 0
                            ? formatCompactNum(cmt.replyCount)
                            : "回复"
                        }}
                      </span>
                    </button>
                    <button
                      class="cm-like-btn"
                      :class="{ liked: cmt.isLiked }"
                      @click="
                        !cmt.isLiked &&
                          triggerOneShotClass(
                            $event.currentTarget as HTMLElement,
                            'is-bouncing',
                            460
                          );
                        store.toggleLikeComment(cmt);
                      "
                    >
                      <PlumpIcon
                        :name="cmt.isLiked ? 'heart-filled' : 'heart'"
                        :size="15"
                      />
                      <RollingCounter
                        :value="cmt.likeCount"
                        :display-text="formatCompactNum(cmt.likeCount)"
                      />
                    </button>
                  </div>
                </div>

                <!-- 评论主体 + 图二分支树连接干对齐用户头像中心 -->
                <div class="cm-root-body-wrap">
                  <!-- 从楼主头像正下方贯穿正文至树干分支的连续垂线 -->
                  <svg
                    v-if="getMainCardTreeState(cmt).visibleRoots.length > 0"
                    class="cm-root-stem-svg"
                    viewBox="0 0 2 100"
                    preserveAspectRatio="none"
                    aria-hidden="true"
                  >
                    <line
                      x1="1"
                      y1="0"
                      x2="1"
                      y2="100"
                      stroke="var(--cm-branch-stroke)"
                      stroke-width="1.5"
                    />
                  </svg>

                  <div class="cm-root-body-indent">
                    <div
                      class="cm-content-text"
                      @click="selectReplyTarget(cmt, true)"
                    >
                      {{ cmt.content }}
                    </div>

                    <!-- 评论配图网格 -->
                    <div
                      v-if="cmt.images && cmt.images.length > 0"
                      class="cm-img-grid"
                    >
                      <img
                        v-for="(imgUrl, idx) in cmt.images"
                        :key="idx"
                        :src="imgUrl"
                        class="cm-comment-img"
                        loading="lazy"
                        decoding="async"
                        alt="comment"
                        @click="previewImageUrl = imgUrl"
                      />
                    </div>
                  </div>
                </div>

                <!-- 图二圆弧分枝树状楼中楼回复（默认展示前 2 条高赞直接分支 + 一键展开全部树 / 楼层详情） -->
                <div
                  v-if="
                    cmt.replyCount > 0 ||
                    getMainCardTreeState(cmt).visibleRoots.length > 0
                  "
                  class="cm-root-tree-block"
                >
                  <CommentTreeBranch
                    v-if="getMainCardTreeState(cmt).visibleRoots.length > 0"
                    :nodes="getMainCardTreeState(cmt).visibleRoots"
                    :root-owner="cmt"
                    :collapsed-ids="collapsedBranchIds"
                    :reply-target-id="replyTarget?.id"
                    @toggle-collapse="toggleBranchCollapse"
                    @toggle-like="store.toggleLikeComment($event)"
                    @reply="selectReplyTarget($event, true)"
                    @open-chain="openConversationModal($event)"
                    @preview-image="previewImageUrl = $event"
                  />

                  <div class="cm-tree-footer-bar">
                    <button
                      v-if="
                        getMainCardTreeState(cmt).canExpandMore ||
                        expandedMainTreeIds.has(cmt.id)
                      "
                      type="button"
                      class="cm-tree-expand-btn"
                      @click.stop="toggleMainTreeExpand(cmt)"
                    >
                      <span>
                        {{
                          expandedMainTreeIds.has(cmt.id)
                            ? "收起内联回复 ▴"
                            : "原地展开回复 ▾"
                        }}
                      </span>
                    </button>

                    <button
                      type="button"
                      class="cm-open-floor-link"
                      @click.stop="openFloorDetail(cmt)"
                    >
                      <span>
                        进入楼层详情 ({{ formatCompactNum(Math.max(cmt.replyCount, getMainCardTreeState(cmt).totalRepliesCount)) }})
                      </span>
                      <PlumpIcon name="chevron-right" :size="13" />
                    </button>
                  </div>
                </div>
              </article>

              <!-- 自动静默加载哨兵 -->
              <div ref="mainSentinelRef" class="cm-scroll-sentinel">
                <span v-if="isLoadingMoreMain" class="cm-silent-loading-dot">
                  正在加载更多精彩乐评...
                </span>
                <span
                  v-else-if="!mainHasMore && store.activeSongComments.length > 0"
                  class="cm-end-caption"
                >
                  已展示当前排序全部 {{ store.activeSongComments.length }} 条乐评
                </span>
              </div>
            </div>
          </div>
        </div>

        <!-- ==================== 底部评论输入与表情栏 ==================== -->
        <div class="cm-composer-footer">
          <div v-if="replyTarget" class="cm-replying-banner">
            <div class="cm-replying-info">
              <span class="cm-replying-badge">
                {{
                  activeFloorRoot && replyTarget.id === activeFloorRoot.id
                    ? "回复楼主"
                    : "正在回复"
                }}
              </span>
              <span class="cm-replying-name">@{{ replyTarget.userName }}</span>
              <span class="cm-replying-excerpt">
                ：{{ replyTarget.content }}
              </span>
            </div>
            <button
              v-if="!(activeFloorRoot && replyTarget.id === activeFloorRoot.id)"
              class="cm-cancel-reply"
              @click="clearExplicitReplyTarget()"
            >
              {{ activeFloorRoot ? "改为回复楼主" : "取消回复" }}
            </button>
          </div>

          <div v-if="showEmojiPicker" class="cm-emoji-bar">
            <button
              v-for="em in emojiList"
              :key="em"
              class="cm-emoji-btn"
              @click="insertEmoji(em)"
            >
              {{ em }}
            </button>
          </div>

          <div class="cm-input-row">
            <button
              class="cm-emoji-trigger"
              :class="{ active: showEmojiPicker }"
              title="插入表情"
              @click="showEmojiPicker = !showEmojiPicker"
            >
              😊
            </button>
            <input
              ref="composerInputRef"
              v-model="commentInput"
              type="text"
              class="cm-text-input"
              :placeholder="
                replyTarget
                  ? `回复 @${replyTarget.userName}...`
                  : '写下听歌此刻的感受与共鸣...'
              "
              @keydown.enter="handleSendComment"
            />
            <button
              class="primary-btn cm-send-btn"
              :disabled="!commentInput.trim() || isSubmitting"
              @click="handleSendComment"
            >
              <span>{{ isSubmitting ? "发送中..." : "发送评论" }}</span>
            </button>
          </div>
        </div>

        <!-- ==================== 对话上下文脉络查看弹窗（深层引用时） ==================== -->
        <transition name="fade">
          <div
            v-if="conversationChain"
            class="cm-submodal-backdrop"
            @click.self="conversationChain = null"
          >
            <div class="cm-conversation-dialog">
              <div class="cm-conv-head">
                <div class="cm-conv-title-wrap">
                  <h4>完整对话脉络</h4>
                  <span class="cm-conv-count">{{ conversationChain.length }} 层对话</span>
                </div>
                <button class="icon-close-btn cm-close-btn" @click="conversationChain = null">
                  <PlumpIcon name="close" :size="14" />
                </button>
              </div>
              <div class="cm-conv-list">
                <div
                  v-for="(node, idx) in conversationChain"
                  :key="node.id + '_' + idx"
                  class="cm-conv-step"
                  :class="{ 'is-root': idx === 0 }"
                >
                  <div class="cm-conv-step-badge">{{ idx === 0 ? "楼" : idx }}</div>
                  <div class="cm-conv-step-body">
                    <div class="cm-conv-user">
                      <strong>
                        {{ node.userName }}
                        <span v-if="idx === 0" class="cm-conv-owner-tag">楼主</span>
                      </strong>
                      <span>
                        {{ resolveDisplayTime(node) }}
                        <template v-if="node.location"> · IP {{ node.location }}</template>
                      </span>
                    </div>
                    <div class="cm-conv-text">{{ node.content }}</div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </transition>

        <!-- ==================== 配图大图预览 ==================== -->
        <transition name="fade">
          <div
            v-if="previewImageUrl"
            class="cm-submodal-backdrop"
            @click="previewImageUrl = null"
          >
            <img :src="previewImageUrl" class="cm-preview-large-img" alt="preview" />
          </div>
        </transition>
      </div>
    </div>
  </transition>
</template>

<style scoped>
/* 透明非覆盖式弹出层 */
.comments-floating-layer {
  position: fixed;
  inset: 0;
  z-index: 2500;
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
}

/* 宽敞大气、简洁优雅的悬浮评论视窗（严格定高定宽不跳动） */
.comments-modal-shell {
  width: min(720px, 90vw);
  height: min(680px, 86vh);
  border-radius: var(--bm-r-xl, 24px);
  backdrop-filter: blur(28px) saturate(165%);
  -webkit-backdrop-filter: blur(28px) saturate(165%);
  color: var(--fm-text-1);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  position: relative;
}

/* ==================== 顶部导航栏 ==================== */
.cm-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 18px 26px 16px;
  background: var(--fm-card);
  flex-shrink: 0;
}

.cm-header-left {
  display: flex;
  align-items: center;
  gap: 14px;
  min-width: 0;
  flex: 1;
}

.cm-header-right {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

/* 返回全部评论导航按钮 */
.cm-back-all-btn {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  height: 36px;
  padding: 0 14px 0 11px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--cm-accent-soft, rgba(56, 189, 248, 0.16));
  color: var(--cm-accent, #38bdf8);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 700;
  cursor: pointer;
  flex-shrink: 0;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    box-shadow 0.2s ease;
}

.cm-back-all-btn:hover {
  transform: translateX(-2px);
  background: var(--cm-accent-soft-strong, rgba(56, 189, 248, 0.24));
  box-shadow: 0 4px 16px var(--cm-accent-glow, rgba(56, 189, 248, 0.24));
}

.cm-floor-song-context {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  padding-left: 14px;
}

.cm-song-cover-mini {
  width: 32px;
  height: 32px;
  border-radius: var(--bm-r-sm, 10px);
  object-fit: cover;
  flex-shrink: 0;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.36);
}

.cm-floor-song-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.cm-floor-nav-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: 800;
  color: var(--fm-text-1);
  letter-spacing: 0.01em;
}

.cm-floor-nav-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 280px;
}

.cm-floor-count-pill {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 32px;
  padding: 0 13px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-card);
  color: var(--fm-text-2);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

/* 主视图歌曲与总数胶囊 */
.cm-song-pill {
  display: flex;
  align-items: center;
  gap: 13px;
  min-width: 0;
}

.cm-song-cover {
  width: 44px;
  height: 44px;
  border-radius: var(--bm-r-md, 14px);
  object-fit: cover;
  flex-shrink: 0;
  box-shadow: 0 8px 20px rgba(0, 0, 0, 0.38);
}

.cm-song-meta {
  min-width: 0;
}

.cm-song-title {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: var(--bm-fs-song-title, 15px);
  font-weight: 800;
  color: var(--fm-text-1);
  min-width: 0;
}

.cm-song-name-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 310px;
}

.cm-total-pill {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--cm-accent, #38bdf8);
  background: var(--cm-accent-soft, rgba(56, 189, 248, 0.16));
  padding: 2.5px 10px;
  border-radius: var(--bm-r-pill, 999px);
  flex-shrink: 0;
}

.cm-song-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
  margin-top: 3px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.cm-link-inline {
  background: none;
  padding: 0;
  color: var(--cm-accent, #38bdf8);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  cursor: pointer;
}

.cm-link-inline {
  border-radius: var(--bm-r-xs, 6px);
  padding: 1px 5px;
  margin: -1px -5px;
  transition: color 0.16s ease, background-color 0.16s ease;
}

.cm-link-inline:hover {
  color: var(--cm-accent, var(--primary));
  background: color-mix(in srgb, var(--cm-accent, var(--primary)) 12%, transparent);
}

.cm-sort-capsule {
  display: flex;
  align-items: center;
  gap: 16px;
}

.cm-sort-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  background: transparent;
  color: var(--fm-text-2);
  padding: 6px 2px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  cursor: pointer;
  transition:
    transform 0.22s cubic-bezier(0.22, 1.35, 0.36, 1),
    color 0.22s ease;
}

.cm-sort-btn:hover:not(.active) {
  color: var(--fm-text-1);
  background: transparent;
}

.cm-sort-btn.active {
  background: transparent;
  color: var(--fm-text-1);
}

.cm-close-btn {
  width: 34px;
  height: 34px;
  background: var(--fm-chip-bg);
  color: var(--fm-text-1);
}

.cm-close-btn:hover {
  background: var(--fm-chip-hover);
  color: var(--fm-text-1);
}

/* ==================== 滚动主体 ==================== */
.cm-body-scroll {
  flex: 1;
  overflow-y: auto;
  overflow-anchor: auto;
  padding: 20px 26px 16px;
}

.cm-body-scroll::-webkit-scrollbar {
  width: 6px;
}

.cm-body-scroll::-webkit-scrollbar-thumb {
  background: var(--fm-chip-hover);
  border-radius: var(--bm-r-pill, 999px);
}

.cm-body-scroll::-webkit-scrollbar-thumb:hover {
  background: var(--fm-scroll-thumb);
}

.cm-view-enter {
  animation: cmViewSlideIn 0.26s cubic-bezier(0.22, 1, 0.36, 1);
}

@keyframes cmViewSlideIn {
  from {
    opacity: 0;
    transform: translate3d(0, 8px, 0);
  }
  to {
    opacity: 1;
    transform: translate3d(0, 0, 0);
  }
}

.cm-empty-state {
  text-align: center;
  padding: 76px 20px;
  color: var(--fm-text-3);
  font-size: var(--bm-fs-body, 14.5px);
}

.cm-empty-state.compact {
  padding: 48px 20px;
}

/* 骨架屏 */
.cm-skeleton-list {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.cm-skeleton-card {
  display: flex;
  gap: 14px;
  padding: 18px 20px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--fm-card);
}

.cm-sk-avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: var(--fm-card);
  flex-shrink: 0;
}

.cm-sk-avatar.sm {
  width: 34px;
  height: 34px;
}

.cm-sk-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 9px;
}

.cm-sk-line {
  height: 11px;
  border-radius: var(--bm-r-xs, 6px);
  background: linear-gradient(
    90deg,
    var(--fm-chip-bg) 25%, var(--fm-card-hover) 50%, var(--fm-chip-bg) 75%
  );
  background-size: 200% 100%;
  animation: cmSkeletonShimmer 1.4s infinite linear;
}

.cm-sk-line.short {
  width: 28%;
}

.cm-sk-line.mid {
  width: 58%;
}

.cm-sk-line.long {
  width: 88%;
}

@keyframes cmSkeletonShimmer {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}

/* ==================== 主评论卡片与头像/徽章体系 ==================== */
.cm-main-list {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.cm-root-card {
  padding: 18px 20px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--fm-card);
  transition:
    background 0.22s ease,
    box-shadow 0.22s ease;
}

.cm-root-card:hover {
  background: var(--fm-card-hover);
}

.cm-root-card.is-target {
  background: var(--fm-card-hover);
  box-shadow: 0 10px 28px var(--cm-accent-glow, rgba(56, 189, 248, 0.22));
}

.cm-card-top {
  display: flex;
  align-items: center;
  gap: 12px;
}

.cm-avatar-wrap {
  position: relative;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
}

.cm-avatar-wrap.lg {
  width: 46px;
  height: 46px;
}

.cm-avatar-wrap.sm {
  width: 34px;
  height: 34px;
}

.cm-avatar,
.cm-avatar-fallback {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  object-fit: cover;
}

.cm-avatar.lg,
.cm-avatar-fallback.lg {
  width: 46px;
  height: 46px;
  font-size: var(--bm-fs-subhead, 16.5px);
}

.cm-avatar.sm,
.cm-avatar-fallback.sm {
  width: 34px;
  height: 34px;
  font-size: var(--bm-fs-caption, 12.5px);
}

.cm-avatar-fallback {
  background: linear-gradient(
    135deg,
    var(--cm-accent, #38bdf8),
    var(--cm-accent-2, #6366f1)
  );
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-weight: 800;
  color: var(--cm-on-accent, #060b15);
  font-size: var(--bm-fs-body, 14.5px);
}

.cm-avatar-pendant {
  position: absolute;
  inset: -4px;
  width: calc(100% + 8px);
  height: calc(100% + 8px);
  object-fit: contain;
  pointer-events: none;
}

.cm-avatar-pendant.sm {
  inset: -3px;
  width: calc(100% + 6px);
  height: calc(100% + 6px);
}

.cm-user-col {
  flex: 1;
  min-width: 0;
}

.cm-user-row {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.cm-username {
  font-weight: 700;
  font-size: var(--bm-fs-body, 14.5px);
  color: var(--fm-text-1);
}

.cm-username.lg {
  font-size: var(--bm-fs-song-title, 15px);
}

.cm-username.sm {
  font-size: var(--bm-fs-meta, 13.5px);
}

.cm-nameplate-icon {
  height: 15px;
  width: auto;
  object-fit: contain;
}

.cm-nameplate-icon.sm {
  height: 13.5px;
}

.cm-hot-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 2px 7px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(244, 63, 94, 0.2);
  color: #fda4af;
  letter-spacing: 0.02em;
}

.cm-hot-badge.sm {
  font-size: var(--bm-fs-micro, 11px);
  padding: 2px 6px;
}

.cm-owner-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 2px 7px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--cm-accent-soft, rgba(56, 189, 248, 0.2));
  color: var(--cm-accent, #38bdf8);
}

.cm-vip-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 2px 6.5px;
  border-radius: var(--bm-r-xs, 6px);
  letter-spacing: 0.02em;
}

.cm-vip-tag.xs {
  font-size: var(--bm-fs-micro, 11px);
  padding: 1.5px 5.5px;
}

.cm-vip-tag.svip {
  background: linear-gradient(
    135deg,
    rgba(245, 158, 11, 0.28),
    rgba(251, 191, 36, 0.16)
  );
  color: #fcd34d;
}

.cm-vip-tag.concept {
  background: linear-gradient(
    135deg,
    rgba(56, 189, 248, 0.24),
    rgba(168, 85, 247, 0.22)
  );
  color: #7dd3fc;
}

.cm-vip-tag.vip {
  background: rgba(245, 158, 11, 0.2);
  color: #fbbf24;
}

.cm-vip-tag.pack {
  background: rgba(16, 185, 129, 0.18);
  color: #6ee7b7;
}

.cm-talent-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  padding: 2px 6px;
  border-radius: var(--bm-r-xs, 6px);
  background: rgba(168, 85, 247, 0.22);
  color: #d8b4fe;
}

.comments-modal-shell:not(.is-dark-surface) .cm-hot-badge {
  background: rgba(225, 29, 72, 0.14);
  color: #be123c;
}

.comments-modal-shell:not(.is-dark-surface) .cm-vip-tag.svip,
.comments-modal-shell:not(.is-dark-surface) .cm-vip-tag.vip {
  background: rgba(217, 119, 6, 0.14);
  color: #b45309;
}

.comments-modal-shell:not(.is-dark-surface) .cm-vip-tag.concept {
  background: rgba(14, 116, 144, 0.14);
  color: #0e7490;
}

.comments-modal-shell:not(.is-dark-surface) .cm-vip-tag.pack {
  background: rgba(5, 150, 105, 0.14);
  color: #047857;
}

.comments-modal-shell:not(.is-dark-surface) .cm-talent-tag {
  background: rgba(124, 58, 237, 0.14);
  color: #6d28d9;
}

.cm-talent-tag.xs {
  font-size: var(--bm-fs-micro, 11px);
  padding: 1.5px 5.5px;
}

.cm-singer-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  padding: 2px 6px;
  border-radius: var(--bm-r-xs, 6px);
  background: rgba(236, 72, 153, 0.22);
  color: #f9a8d4;
}

.cm-singer-tag.xs {
  font-size: var(--bm-fs-micro, 11px);
  padding: 1.5px 5.5px;
}

.cm-meta-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 5px;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
  margin-top: 3px;
}

.cm-loc-chip {
  color: var(--fm-text-3);
}

.cm-reply-actions {
  display: flex;
  align-items: center;
  gap: 7px;
  flex-shrink: 0;
}

.cm-action-pill {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--fm-card);
  color: var(--fm-text-2);
  padding: 5px 11px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  cursor: pointer;
  transition: all 0.2s ease;
}

.cm-action-pill.sm {
  padding: 4px 10px;
  font-size: var(--bm-fs-micro, 11px);
}

.cm-action-pill:hover,
.cm-action-pill.active {
  color: var(--cm-accent, #38bdf8);
  background: var(--cm-accent-soft, rgba(56, 189, 248, 0.16));
}

.cm-like-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  background: var(--fm-card);
  color: var(--fm-text-2);
  padding: 5px 11px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  cursor: pointer;
  transition:
    transform 0.2s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.2s ease,
    color 0.2s ease;
}

.cm-like-btn.sm {
  padding: 4px 9px;
  font-size: var(--bm-fs-micro, 11px);
  gap: 4px;
}

.cm-like-btn:hover {
  transform: scale(1.05);
  background: var(--fm-chip-hover);
}

.cm-like-btn.liked {
  color: #fb7185;
  background: rgba(251, 113, 133, 0.18);
}

/* 评论文本与图二分支树连续干对齐 */
.cm-root-body-wrap {
  position: relative;
  margin-top: 8px;
}

.cm-root-stem-svg {
  position: absolute;
  left: 19.25px;
  top: 0;
  width: 2px;
  height: 100%;
  pointer-events: none;
}

.cm-root-body-indent {
  padding-left: 52px;
}

.cm-owner-body-wrap {
  padding-left: 58px;
  margin-top: 12px;
}

.cm-root-tree-block {
  padding-left: 8px;
  margin-top: 2px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.cm-floor-tree-wrap {
  padding-left: 4px;
}

.cm-tree-footer-bar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  padding-left: 28px;
  padding-top: 2px;
}

.cm-tree-expand-btn {
  border: none;
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
  font-size: 12px;
  font-weight: 700;
  padding: 4px 11px;
  border-radius: 999px;
  cursor: pointer;
  transition: background 0.16s ease, color 0.16s ease;
}

.cm-tree-expand-btn:hover {
  background: var(--cm-accent-soft);
  color: var(--cm-accent);
}

.cm-content-text {
  font-size: var(--bm-fs-body, 14px);
  line-height: 1.66;
  color: var(--fm-text-1);
  white-space: pre-wrap;
  word-break: break-word;
  cursor: pointer;
}

.cm-content-text.lg {
  font-size: var(--bm-fs-song-title, 15px);
  line-height: 1.72;
  color: var(--fm-text-1);
}

.cm-content-text.reply {
  font-size: var(--bm-fs-body, 14px);
  line-height: 1.66;
  cursor: inherit;
}

.cm-img-grid {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 10px;
}

.cm-comment-img {
  width: 92px;
  height: 92px;
  border-radius: var(--bm-r-md, 14px);
  object-fit: cover;
  cursor: zoom-in;
  transition: transform 0.2s ease;
}

.cm-comment-img:hover {
  transform: scale(1.03);
}

.cm-comment-img.sm {
  width: 74px;
  height: 74px;
}

.cm-open-floor-link {
  border: none;
  background: transparent;
  padding: 4px 6px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--bm-fs-caption, 12px);
  font-weight: 700;
  color: var(--cm-accent);
  cursor: pointer;
  transition: background 0.16s ease;
}

.cm-open-floor-link:hover {
  background: var(--cm-accent-soft);
}

/* ==================== 全新“查看评论回复详情”视图 ==================== */
.cm-floor-view {
  display: flex;
  flex-direction: column;
}

/* 原评论（楼主）卡片 */
.cm-floor-owner-card {
  padding: 18px 22px 20px;
  border-radius: var(--bm-r-lg, 18px);
  background: linear-gradient(
    145deg,
    var(--fm-card-elevated),
    var(--fm-card)
  );
  box-shadow:
    0 12px 32px rgba(0, 0, 0, 0.24);
}

.cm-owner-eyebrow {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 14px;
  padding-bottom: 10px;
}

.cm-owner-pill-strong {
  display: inline-flex;
  align-items: center;
  padding: 3px 10px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  letter-spacing: 0.03em;
  background: var(--cm-accent, #38bdf8);
  color: var(--cm-on-accent, #060b15);
  box-shadow: 0 4px 12px var(--cm-accent-glow, rgba(56, 189, 248, 0.26));
}

.cm-owner-reply-total {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--fm-text-3);
}

/* 清晰视觉分隔 & 全部回复标题栏 */
.cm-floor-section-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin: 20px 2px 14px;
  padding-bottom: 10px;
}

.cm-floor-section-left {
  display: flex;
  align-items: center;
  gap: 9px;
}

.cm-section-accent-bar {
  width: 3.5px;
  height: 15px;
  border-radius: var(--bm-r-pill, 999px);
  background: linear-gradient(
    180deg,
    var(--cm-accent, #38bdf8),
    var(--cm-accent-2, #00e5ff)
  );
}

.cm-section-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.cm-section-count {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  padding: 1px 8px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-card);
  color: var(--fm-text-2);
}

.cm-floor-hint-text {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
}

.cm-owner-filter-capsule {
  display: inline-flex;
  align-items: center;
  gap: 14px;
}

.cm-owner-filter-btn {
  background: transparent;
  color: var(--fm-text-3);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  padding: 4px 2px;
  cursor: pointer;
  transition: color 0.2s ease;
}

.cm-owner-filter-btn:hover:not(.active) {
  color: var(--fm-text-1);
  background: transparent;
}

.cm-owner-filter-btn.active {
  background: transparent;
  color: var(--cm-accent, #38bdf8);
}

/* 全部子回复列表与对话脉络卡片 */
.cm-floor-reply-list {
  display: flex;
  flex-direction: column;
  gap: 11px;
}

.cm-reply-card {
  padding: 15px 18px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--fm-card);
  cursor: pointer;
  transition:
    background 0.2s ease,
    box-shadow 0.25s ease;
}

.cm-reply-card:hover {
  background: var(--fm-card-hover);
}

.cm-reply-card.is-target {
  background: var(--fm-card-elevated);
}

.cm-reply-card.is-highlighted {
  background: var(--cm-accent-soft, rgba(56, 189, 248, 0.16));
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.28);
}

.cm-reply-body-indent {
  padding-left: 46px;
  margin-top: 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* 明确“回复 @某人”指向行 */
.cm-reply-target-line {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 5px;
  font-size: var(--bm-fs-caption, 12.5px);
}

.cm-reply-to-label {
  color: var(--fm-text-3);
}

.cm-reply-mention {
  font-weight: 700;
  color: var(--cm-accent, #38bdf8);
}

.cm-chain-link-btn {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 2px 8px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--fm-card);
  color: var(--fm-text-2);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
}

.cm-chain-link-btn:hover {
  color: var(--cm-accent, #38bdf8);
  background: var(--cm-accent-soft, rgba(56, 189, 248, 0.16));
}

/* 被回复评论的引用上下文块 */
.cm-quote-block {
  padding: 9px 13px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--fm-input-bg);
  transition:
    background 0.2s ease;
}

.cm-quote-block.is-jumpable {
  cursor: pointer;
}

.cm-quote-block.is-jumpable:hover {
  background: var(--cm-accent-2-soft, rgba(0, 229, 255, 0.12));
}

.cm-quote-line {
  font-size: var(--bm-fs-caption, 12.5px);
  line-height: 1.55;
  color: var(--fm-text-2);
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}

.cm-quote-author {
  font-weight: 700;
  color: var(--cm-accent-2, #38bdf8);
}

.cm-quote-text {
  color: var(--fm-text-2);
}

/* ==================== 静默加载哨兵 ==================== */
.cm-scroll-sentinel {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 38px;
  padding: 12px 0 6px;
}

.cm-silent-loading-dot {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
  letter-spacing: 0.02em;
}

.cm-end-caption {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
}

/* ==================== 底部评论发送与表情栏 ==================== */
.cm-composer-footer {
  padding: 13px 26px 17px;
  background: var(--fm-card-elevated);
  flex-shrink: 0;
}

.cm-replying-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-2);
  background: var(--cm-accent-soft, rgba(56, 189, 248, 0.14));
  padding: 6px 12px;
  border-radius: var(--bm-r-sm, 10px);
  margin-bottom: 9px;
}

.cm-replying-info {
  display: flex;
  align-items: center;
  min-width: 0;
  flex: 1;
}

.cm-replying-badge {
  flex-shrink: 0;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 2px 6px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--cm-accent, #38bdf8);
  color: var(--cm-on-accent, #060b15);
  margin-right: 6px;
}

.cm-replying-name {
  flex-shrink: 0;
  font-weight: 700;
  color: var(--cm-accent, #38bdf8);
}

.cm-replying-excerpt {
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--fm-text-3);
}

.cm-cancel-reply {
  flex-shrink: 0;
  background: var(--fm-card);
  border-radius: var(--bm-r-xs, 6px);
  padding: 2px 8px;
  color: var(--fm-text-2);
  cursor: pointer;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
}

.cm-cancel-reply:hover {
  background: var(--fm-chip-hover);
  color: var(--fm-text-1);
}

.cm-emoji-bar {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  padding: 8px 10px;
  margin-bottom: 9px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--fm-card);
}

.cm-emoji-btn {
  background: none;
  font-size: 17px;
  padding: 3px 5px;
  cursor: pointer;
  border-radius: var(--bm-r-xs, 6px);
}

.cm-emoji-btn:hover {
  background: var(--fm-chip-hover);
}

.cm-input-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.cm-emoji-trigger {
  width: 38px;
  height: 38px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--fm-card);
  font-size: 18px;
  cursor: pointer;
  flex-shrink: 0;
  transition: background 0.2s ease;
}

.cm-emoji-trigger:hover,
.cm-emoji-trigger.active {
  background: var(--cm-accent-soft, rgba(56, 189, 248, 0.16));
}

.cm-text-input {
  flex: 1;
  height: 38px;
  padding: 0 14px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--fm-card);
  color: var(--fm-text-1);
  font-size: var(--bm-fs-meta, 13.5px);
  outline: none;
  transition: border-color 0.2s ease, background 0.2s ease;
}

.cm-text-input:focus {
  background: var(--fm-chip-hover);
}

.cm-text-input::placeholder {
  color: var(--fm-text-3);
}

.cm-send-btn {
  height: 38px;
  padding: 0 18px;
  border-radius: var(--bm-r-sm, 10px);
  background: linear-gradient(
    135deg,
    var(--cm-accent, #38bdf8),
    var(--cm-accent-2, #00e5ff)
  );
  color: var(--cm-on-accent, #060b15);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 800;
  cursor: pointer;
  flex-shrink: 0;
  box-shadow: 0 4px 16px var(--cm-accent-glow, rgba(56, 189, 248, 0.26));
  transition:
    transform 0.2s cubic-bezier(0.22, 1.35, 0.36, 1),
    filter 0.2s ease;
}

.cm-send-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  filter: brightness(1.06);
}

.cm-send-btn:disabled {
  opacity: 0.42;
  cursor: not-allowed;
  box-shadow: none;
}

/* ==================== 对话上下文链路与大图预览 ==================== */
.cm-submodal-backdrop {
  position: absolute;
  inset: 0;
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 20;
}

.cm-conversation-dialog {
  width: 540px;
  max-width: 90%;
  max-height: 78%;
  background: var(--fm-bg);
  color: var(--fm-text-1);
  border-radius: var(--bm-r-xl, 24px);
  padding: 20px 22px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  box-shadow: 0 26px 60px rgba(0, 0, 0, 0.68);
}

.cm-conv-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.cm-conv-title-wrap {
  display: flex;
  align-items: center;
  gap: 9px;
}

.cm-conv-title-wrap h4 {
  margin: 0;
  font-size: var(--bm-fs-song-title, 15px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.cm-conv-count {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  padding: 2px 8px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--cm-accent-soft, rgba(56, 189, 248, 0.18));
  color: var(--cm-accent, #38bdf8);
}

.cm-conv-list {
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-right: 4px;
}

.cm-conv-step {
  display: flex;
  gap: 11px;
  padding: 11px 13px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--fm-card);
}

.cm-conv-step.is-root {
  background: var(--cm-accent-soft, rgba(56, 189, 248, 0.12));
}

.cm-conv-step-badge {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--cm-accent, #38bdf8);
  color: var(--cm-on-accent, #060b15);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.cm-conv-step-body {
  flex: 1;
  min-width: 0;
}

.cm-conv-user {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-1);
  margin-bottom: 4px;
}

.cm-conv-owner-tag {
  margin-left: 5px;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 2px 6px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--cm-accent, #38bdf8);
  color: var(--cm-on-accent, #060b15);
}

.cm-conv-user > span {
  color: var(--fm-text-3);
  font-size: var(--bm-fs-micro, 11px);
}

.cm-conv-text {
  font-size: var(--bm-fs-meta, 13.5px);
  color: var(--fm-text-1);
  line-height: 1.55;
  word-break: break-word;
}

.cm-preview-large-img {
  max-width: 88%;
  max-height: 84%;
  border-radius: var(--bm-r-md, 14px);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
}

/* 灵动柔和弹簧弹出与收起动画 */
.cm-float-spring-enter-active {
  transition:
    opacity 0.34s cubic-bezier(0.16, 1, 0.3, 1),
    transform 0.42s cubic-bezier(0.22, 1.18, 0.36, 1);
}

.cm-float-spring-leave-active {
  transition:
    opacity 0.24s cubic-bezier(0.4, 0, 1, 1),
    transform 0.26s cubic-bezier(0.4, 0, 0.6, 1);
}

.cm-float-spring-enter-from {
  opacity: 0;
  transform: translate3d(0, 18px, 0) scale(0.92);
}

.cm-float-spring-leave-to {
  opacity: 0;
  transform: translate3d(0, 10px, 0) scale(0.95);
}
</style>

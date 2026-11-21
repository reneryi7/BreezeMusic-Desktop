<script setup lang="ts">
/**
 * NowPlayingQueuePopover.vue
 * 二合一统一播放队列浮窗（主界面底栏右下角 + 播放页播控台右侧 共用组件）
 * - 主界面内：跟随 D1 / D2 / L1 / L2 四主题视觉表面，锚定于底栏上方右下方
 * - 播放页内：适配 24/7 深色剧场与封面双主色取色系统，锚定于播控按钮右侧
 * - 顶栏精简 + 判别排序按需折叠，保留副歌速刷与定位/清空
 * - 零闪烁固定步距跟手拖拽与平滑让位补位
 * - 最近队列真 3D 立体弧度扇形视图（悬停微抬探头预览 + 点击/滚轮丝滑旋切 C 位）
 */
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useAppStore } from "../stores/appStore";
import type { Song } from "../types";
import { formatDuration, splitSongAuthors } from "../api/kugou";
import {
  hexWithAlpha,
  lerpHex,
} from "../utils/coverPalette";
import { useFloatingModalTheme } from "../composables/useFloatingModalTheme";
import PlumpIcon from "./PlumpIcon.vue";
import RollingCounter from "./RollingCounter.vue";

export type QueueDiscriminantSortMode = "artist" | "quality" | "duration";

export interface RecentQueueSnapshot {
  id: string;
  title: string;
  songs: Song[];
  savedAt?: number | string;
  savedAtText?: string;
  lastPlayedAt?: number | string;
  lastPlayedAtText?: string;
  createdAt?: number | string;
  updatedAt?: number | string;
  timestamp?: number | string;
  covers?: string[];
  source?: string;
  trackCount?: number;
  activeHash?: string;
}

const props = withDefaults(
  defineProps<{
    modelValue?: boolean;
    visible?: boolean;
    show?: boolean;
    recentQueues?: RecentQueueSnapshot[];
    scene?: "auto" | "main-bar" | "now-playing";
    climaxSkim?: boolean;
  }>(),
  {
    modelValue: undefined,
    visible: undefined,
    show: undefined,
    recentQueues: undefined,
    scene: "auto",
    climaxSkim: undefined,
  }
);

const emit = defineEmits<{
  (e: "update:modelValue", val: boolean): void;
  (e: "update:visible", val: boolean): void;
  (e: "update:show", val: boolean): void;
  (e: "close"): void;
  (e: "select-song", song: Song, index: number): void;
  (e: "reorder-queue", payload: { fromIndex: number; toIndex: number }): void;
  (
    e: "sort-queue",
    payload: { mode: QueueDiscriminantSortMode; direction: "asc" | "desc" }
  ): void;
  (e: "clear-queue"): void;
  (e: "restore-queue", queue: RecentQueueSnapshot): void;
  (
    e: "remove-recent-queue",
    payload: { queue: RecentQueueSnapshot; index: number }
  ): void;
  (e: "toggle-climax", enabled: boolean): void;
}>();

const store = useAppStore();

// ================= 1. 显隐统一兼容 & 双场景（主界面四主题 / 播放页封面取色） =================
const isOpen = computed(() => {
  if (props.modelValue !== undefined) return props.modelValue;
  if (props.visible !== undefined) return props.visible;
  if (props.show !== undefined) return props.show;
  return store.isQueueOpen;
});

const resolvedScene = computed<"main-bar" | "now-playing">(() => {
  if (props.scene !== "auto") return props.scene;
  return store.isNowPlayingOpen ? "now-playing" : "main-bar";
});

function closePopover() {
  store.isQueueOpen = false;
  store.queueAnchorRect = null;
  emit("update:modelValue", false);
  emit("update:visible", false);
  emit("update:show", false);
  emit("close");
}

// 播放页内队列弹窗动态锚定：紧贴播放页队列按钮旁边弹出（底栏打开时仍锚定在屏幕右侧）
const viewportSize = ref({
  w: typeof window !== "undefined" ? window.innerWidth : 1360,
  h: typeof window !== "undefined" ? window.innerHeight : 820,
});
const measuredNpAnchor = ref<{
  left: number;
  top: number;
  right: number;
  bottom: number;
  width: number;
  height: number;
  clearRight?: number;
} | null>(null);

function refreshNowPlayingAnchorRect() {
  if (typeof window === "undefined") return;
  viewportSize.value = { w: window.innerWidth, h: window.innerHeight };
  if (resolvedScene.value !== "now-playing" || !isOpen.value) {
    measuredNpAnchor.value = null;
    return;
  }
  // 测量黑胶/封面右侧竖排快捷按钮右边界，防止队列浮窗左沿半截遮挡侧栏图标
  let sideClearRight = 0;
  document.querySelectorAll(".np-side-action-btn").forEach((el) => {
    const sr = el.getBoundingClientRect();
    if (sr.width > 0 && sr.height > 0 && sr.right < window.innerWidth * 0.65) {
      sideClearRight = Math.max(sideClearRight, sr.right);
    }
  });
  const triggerBtn = document.querySelector(".np-queue-trigger-btn") as HTMLElement | null;
  if (triggerBtn) {
    const r = triggerBtn.getBoundingClientRect();
    if (r.width > 0 && r.height > 0) {
      measuredNpAnchor.value = {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
        width: r.width,
        height: r.height,
        clearRight: Math.max(r.right, sideClearRight),
      };
      return;
    }
  }
  measuredNpAnchor.value = store.queueAnchorRect
    ? {
        ...store.queueAnchorRect,
        clearRight: Math.max(store.queueAnchorRect.right, sideClearRight),
      }
    : null;
}

const activeNpAnchor = computed(() => {
  if (resolvedScene.value !== "now-playing") return null;
  return measuredNpAnchor.value || store.queueAnchorRect;
});

const anchoredNowPlayingStyle = computed<Record<string, string>>(() => {
  const anchor = activeNpAnchor.value;
  if (resolvedScene.value !== "now-playing" || !anchor) {
    return {} as Record<string, string>;
  }

  const vw = viewportSize.value.w;
  const vh = viewportSize.value.h;
  const edgePad = 18;
  const topSafe = 64;
  const popW =
    activeView.value === "recent"
      ? Math.min(548, Math.max(320, vw - edgePad * 2))
      : Math.min(468, Math.max(320, vw - edgePad * 2));
  const popH = Math.min(608, Math.max(340, vh - topSafe - 28));

  // 优先紧贴队列按钮（并避开黑胶侧栏竖排图标）右侧展开，若右侧空间不足则自动翻折到按钮上方，边缘永不裁切
  const gap = 16;
  const anchorRightEdge =
    "clearRight" in anchor && typeof anchor.clearRight === "number"
      ? anchor.clearRight
      : anchor.right;
  let left: number;
  let top: number;
  let transformOrigin: string;

  if (anchorRightEdge + gap + popW <= vw - edgePad) {
    left = anchorRightEdge + gap;
    top = Math.max(topSafe, Math.min(vh - popH - 22, anchor.bottom - popH + 44));
    transformOrigin = "bottom left";
  } else if (anchor.right + gap + popW <= vw - edgePad) {
    left = anchor.right + gap;
    top = Math.max(topSafe, Math.min(vh - popH - 22, anchor.bottom - popH + 44));
    transformOrigin = "bottom left";
  } else if (anchor.top - 14 - popH >= topSafe) {
    left = Math.max(edgePad, Math.min(vw - popW - edgePad, anchor.right - popW + 24));
    top = Math.max(topSafe, anchor.top - popH - 14);
    transformOrigin = "bottom right";
  } else {
    left = Math.max(edgePad, Math.min(vw - popW - edgePad, anchor.right + 12));
    top = Math.max(topSafe, Math.min(vh - popH - 22, anchor.bottom - popH + 44));
    transformOrigin = "bottom center";
  }

  return {
    position: "fixed",
    left: `${Math.round(left)}px`,
    top: `${Math.round(top)}px`,
    width: `${Math.round(popW)}px`,
    height: `${Math.round(popH)}px`,
    transformOrigin,
  };
});

watch(
  () => [isOpen.value, resolvedScene.value, store.queueAnchorRect],
  () => {
    if (isOpen.value && resolvedScene.value === "now-playing") {
      refreshNowPlayingAnchorRect();
      nextTick(() => refreshNowPlayingAnchorRect());
    } else {
      measuredNpAnchor.value = null;
    }
  },
  { immediate: true }
);

const {
  isLightSurface,
  isDynamicStage,
  palette,
  accentPrimary,
  accentSecondary,
  onAccentPrimary,
  onAccentSecondary,
  vipGold,
  modalSurfaceStyle,
} = useFloatingModalTheme(props.scene);

const readablePrimary = accentPrimary;
const readableSecondary = accentSecondary;
const onPrimaryText = onAccentPrimary;
const onSecondaryText = onAccentSecondary;

const themeSurfaceStyle = computed<Record<string, string>>(() => {
  const c1 = readablePrimary.value;
  const c2 = readableSecondary.value;
  const on1 = onPrimaryText.value;
  const on2 = onSecondaryText.value;
  const gold = vipGold.value;

  if (isDynamicStage.value) {
    const pal = palette.value as Record<string, any>;
    const cardHigh = pal.cardSurfaceElevated || pal.cardSurfacePrimary || "#1D2436";
    const cardMid = pal.bgPrimary || "#141824";
    const cardLow = pal.bgSecondary || "#0E121C";
    return {
      ...modalSurfaceStyle.value,
      "--nq-text-1": "#F5F7FA",
      "--nq-text-2": "rgba(232, 237, 245, 0.76)",
      "--nq-text-3": "rgba(210, 218, 232, 0.54)",
      "--nq-accent-1": c1,
      "--nq-accent-2": c2,
      "--nq-on-accent-1": on1,
      "--nq-on-accent-2": on2,
      "--nq-gold": gold,
      "--nq-gold-soft": hexWithAlpha(gold, 0.2),
      "--nq-danger": "#FB7185",
      "--nq-danger-soft": "rgba(244, 63, 94, 0.2)",
      "--nq-accent-1-soft": hexWithAlpha(c1, 0.2),
      "--nq-accent-1-glow": hexWithAlpha(c1, 0.3),
      "--nq-accent-2-soft": hexWithAlpha(c2, 0.18),
      "--nq-seg-track": "rgba(255, 255, 255, 0.06)",
      "--nq-chip-bg": "rgba(255, 255, 255, 0.085)",
      "--nq-chip-hover": "rgba(255, 255, 255, 0.15)",
      "--nq-row-bg": "rgba(255, 255, 255, 0.036)",
      "--nq-row-hover": "rgba(255, 255, 255, 0.085)",
      "--nq-row-active": `linear-gradient(90deg, ${hexWithAlpha(c1, 0.24)} 0%, ${hexWithAlpha(c2, 0.1)} 100%)`,
      "--nq-row-drag-bg": `linear-gradient(90deg, ${hexWithAlpha(cardHigh, 0.98)} 0%, ${hexWithAlpha(cardLow, 0.98)} 100%)`,
      "--nq-card-bg": `linear-gradient(164deg, ${cardMid} 0%, ${cardLow} 100%)`,
      "--nq-card-selected-bg": `linear-gradient(164deg, ${lerpHex(cardHigh, c1, 0.1)} 0%, ${cardMid} 100%)`,
      "--nq-card-sheen": `radial-gradient(circle at 14% 6%, ${hexWithAlpha(c1, 0.12)} 0%, transparent 48%), radial-gradient(circle at 86% 94%, ${hexWithAlpha(c2, 0.1)} 0%, transparent 46%)`,
      "--nq-card-shadow": "0 18px 40px rgba(0, 0, 0, 0.48)",
      "--nq-card-selected-shadow": `0 24px 54px rgba(0, 0, 0, 0.64), 0 0 24px ${hexWithAlpha(c1, 0.24)}`,
      "--nq-divider": "rgba(255, 255, 255, 0.085)",
      "--nq-scroll-thumb": "rgba(255, 255, 255, 0.2)",
      "--nq-toast-bg": "rgba(18, 24, 38, 0.96)",
      "--nq-toast-text": "#F5F7FA",
    };
  }

  // 主界面 / 非取色播放页：严格对齐 D1 / D2 / L1 / L2 四主题色板与实体/毛玻璃材质规范
  const isLight = store.resolvedColorScheme === "watercolor_light";
  const isOat = isLight && store.lightThemeVariant === "nordic_oat";
  const isVelvet = !isLight && store.darkThemeVariant === "velvet_nebula";

  if (isOat) {
    // L2 烘焙燕麦暖纸：零彩光实体块面三层阶差 (#F2ECE1 外窗 → #E8E0D0 侧翼/行底 → #FAF6EE C位卡) + 北欧青玉 #0F766E × 赤陶 #B45309
    return {
      ...modalSurfaceStyle.value,
      background: "#F2ECE1",
      backdropFilter: "none",
      WebkitBackdropFilter: "none",
      boxShadow: "0 24px 56px rgba(42, 33, 22, 0.18), 0 8px 20px rgba(42, 33, 22, 0.08)",
      "--nq-text-1": "#231D16",
      "--nq-text-2": "#574B3E",
      "--nq-text-3": "#7D7061",
      "--nq-accent-1": "#0F766E",
      "--nq-accent-2": "#B45309",
      "--nq-on-accent-1": "#FFFFFF",
      "--nq-on-accent-2": "#FFFFFF",
      "--nq-gold": "#B45309",
      "--nq-gold-soft": "rgba(180, 83, 9, 0.14)",
      "--nq-danger": "#C2410C",
      "--nq-danger-soft": "rgba(194, 65, 12, 0.15)",
      "--nq-accent-1-soft": "rgba(15, 118, 110, 0.14)",
      "--nq-accent-1-glow": "rgba(15, 118, 110, 0.24)",
      "--nq-accent-2-soft": "rgba(180, 83, 9, 0.13)",
      "--nq-seg-track": "#E6DDD0",
      "--nq-chip-bg": "#E8E0D0",
      "--nq-chip-hover": "#DDD3C0",
      "--nq-row-bg": "#EAE2D3",
      "--nq-row-hover": "#E2D8C6",
      "--nq-row-active": "linear-gradient(90deg, rgba(15, 118, 110, 0.16) 0%, rgba(180, 83, 9, 0.08) 100%)",
      "--nq-row-drag-bg": "#FAF6EE",
      "--nq-card-bg": "#E8E0D0",
      "--nq-card-selected-bg": "#FAF6EE",
      "--nq-card-sheen": "radial-gradient(circle at 14% 8%, rgba(15, 118, 110, 0.07) 0%, transparent 48%), radial-gradient(circle at 86% 92%, rgba(180, 83, 9, 0.06) 0%, transparent 46%)",
      "--nq-card-shadow": "0 12px 28px rgba(42, 33, 22, 0.11)",
      "--nq-card-selected-shadow": "0 18px 40px rgba(42, 33, 22, 0.16), 0 0 18px rgba(15, 118, 110, 0.10)",
      "--nq-divider": "rgba(35, 29, 22, 0.085)",
      "--nq-scroll-thumb": "rgba(87, 75, 62, 0.28)",
      "--nq-toast-bg": "#231D16",
      "--nq-toast-text": "#FAF6EE",
    };
  }

  if (isLight) {
    // L1 云宣瓷白水彩：冰霜毛玻璃 + 晴空鸢尾 #4F46E5 × 天青 #0284C7
    return {
      ...modalSurfaceStyle.value,
      "--nq-text-1": "#18181B",
      "--nq-text-2": "#475569",
      "--nq-text-3": "#64748B",
      "--nq-accent-1": "#4F46E5",
      "--nq-accent-2": "#0284C7",
      "--nq-on-accent-1": "#FFFFFF",
      "--nq-on-accent-2": "#FFFFFF",
      "--nq-gold": "#B45309",
      "--nq-gold-soft": "rgba(180, 83, 9, 0.13)",
      "--nq-danger": "#E11D48",
      "--nq-danger-soft": "rgba(225, 29, 72, 0.14)",
      "--nq-accent-1-soft": "rgba(79, 70, 229, 0.12)",
      "--nq-accent-1-glow": "rgba(79, 70, 229, 0.22)",
      "--nq-accent-2-soft": "rgba(2, 132, 199, 0.12)",
      "--nq-seg-track": "rgba(79, 70, 229, 0.075)",
      "--nq-chip-bg": "rgba(79, 70, 229, 0.08)",
      "--nq-chip-hover": "rgba(79, 70, 229, 0.14)",
      "--nq-row-bg": "rgba(255, 255, 255, 0.68)",
      "--nq-row-hover": "rgba(79, 70, 229, 0.08)",
      "--nq-row-active": "linear-gradient(90deg, rgba(79, 70, 229, 0.15) 0%, rgba(2, 132, 199, 0.09) 100%)",
      "--nq-row-drag-bg": "#FFFFFF",
      "--nq-card-bg": "rgba(241, 245, 252, 0.88)",
      "--nq-card-selected-bg": "rgba(255, 255, 255, 0.96)",
      "--nq-card-sheen": "radial-gradient(circle at 14% 8%, rgba(79, 70, 229, 0.08) 0%, transparent 48%), radial-gradient(circle at 86% 92%, rgba(2, 132, 199, 0.07) 0%, transparent 46%)",
      "--nq-card-shadow": "0 14px 32px rgba(79, 70, 229, 0.1)",
      "--nq-card-selected-shadow": "0 20px 44px rgba(79, 70, 229, 0.15), 0 0 20px rgba(2, 132, 199, 0.1)",
      "--nq-divider": "rgba(24, 24, 27, 0.075)",
      "--nq-scroll-thumb": "rgba(71, 85, 105, 0.24)",
      "--nq-toast-bg": "#18181B",
      "--nq-toast-text": "#FFFFFF",
    };
  }

  if (isVelvet) {
    // D2 微醺苦巧天鹅绒：实体丝绒无毛玻璃 (#1A1116 外窗 → #261820 侧翼 → #34212B C位) + 落日玫瑰 #F43F5E × 胆机琥珀 #F59E0B
    return {
      ...modalSurfaceStyle.value,
      background: "linear-gradient(168deg, #24161E 0%, #1A1116 100%)",
      backdropFilter: "none",
      WebkitBackdropFilter: "none",
      boxShadow: "0 28px 64px rgba(10, 4, 8, 0.68)",
      "--nq-text-1": "#F9EFE6",
      "--nq-text-2": "rgba(232, 215, 206, 0.78)",
      "--nq-text-3": "rgba(185, 159, 168, 0.66)",
      "--nq-accent-1": "#F43F5E",
      "--nq-accent-2": "#F59E0B",
      "--nq-on-accent-1": "#FFFFFF",
      "--nq-on-accent-2": "#1A1116",
      "--nq-gold": "#F59E0B",
      "--nq-gold-soft": "rgba(245, 158, 11, 0.2)",
      "--nq-danger": "#FB7185",
      "--nq-danger-soft": "rgba(244, 63, 94, 0.22)",
      "--nq-accent-1-soft": "rgba(244, 63, 94, 0.18)",
      "--nq-accent-1-glow": "rgba(244, 63, 94, 0.3)",
      "--nq-accent-2-soft": "rgba(245, 158, 11, 0.18)",
      "--nq-seg-track": "#2A1B23",
      "--nq-chip-bg": "#2E1E26",
      "--nq-chip-hover": "#3B2631",
      "--nq-row-bg": "#24171E",
      "--nq-row-hover": "#311F28",
      "--nq-row-active": "linear-gradient(90deg, rgba(244, 63, 94, 0.22) 0%, rgba(245, 158, 11, 0.12) 100%)",
      "--nq-row-drag-bg": "#38232E",
      "--nq-card-bg": "#25171F",
      "--nq-card-selected-bg": "#33202A",
      "--nq-card-sheen": "radial-gradient(circle at 14% 8%, rgba(244, 63, 94, 0.12) 0%, transparent 48%), radial-gradient(circle at 86% 92%, rgba(245, 158, 11, 0.1) 0%, transparent 46%)",
      "--nq-card-shadow": "0 18px 40px rgba(8, 3, 6, 0.56)",
      "--nq-card-selected-shadow": "0 24px 54px rgba(8, 3, 6, 0.7), 0 0 24px rgba(244, 63, 94, 0.22)",
      "--nq-divider": "rgba(249, 239, 230, 0.085)",
      "--nq-scroll-thumb": "rgba(249, 239, 230, 0.2)",
      "--nq-toast-bg": "#35202B",
      "--nq-toast-text": "#F9EFE6",
    };
  }

  // D1 极夜冷曜石：冰霜毛玻璃 + 极光蓝靛 #5E6AD2 × 冰川天青 #38BDF8
  return {
    ...modalSurfaceStyle.value,
    "--nq-text-1": "#EEF2F6",
    "--nq-text-2": "rgba(203, 213, 225, 0.8)",
    "--nq-text-3": "rgba(148, 163, 184, 0.68)",
    "--nq-accent-1": "#5E6AD2",
    "--nq-accent-2": "#38BDF8",
    "--nq-on-accent-1": "#FFFFFF",
    "--nq-on-accent-2": "#091017",
    "--nq-gold": "#FACC15",
    "--nq-gold-soft": "rgba(250, 204, 21, 0.18)",
    "--nq-danger": "#FB7185",
    "--nq-danger-soft": "rgba(244, 63, 94, 0.2)",
    "--nq-accent-1-soft": "rgba(94, 106, 210, 0.2)",
    "--nq-accent-1-glow": "rgba(94, 106, 210, 0.3)",
    "--nq-accent-2-soft": "rgba(56, 189, 248, 0.17)",
    "--nq-seg-track": "rgba(255, 255, 255, 0.06)",
    "--nq-chip-bg": "rgba(255, 255, 255, 0.08)",
    "--nq-chip-hover": "rgba(94, 106, 210, 0.18)",
    "--nq-row-bg": "rgba(255, 255, 255, 0.036)",
    "--nq-row-hover": "rgba(94, 106, 210, 0.11)",
    "--nq-row-active": "linear-gradient(90deg, rgba(94, 106, 210, 0.24) 0%, rgba(56, 189, 248, 0.1) 100%)",
    "--nq-row-drag-bg": "linear-gradient(90deg, #242C42 0%, #181E2E 100%)",
    "--nq-card-bg": "linear-gradient(164deg, #1A2132 0%, #111622 100%)",
    "--nq-card-selected-bg": "linear-gradient(164deg, #242D44 0%, #161D2E 100%)",
    "--nq-card-sheen": "radial-gradient(circle at 14% 8%, rgba(94, 106, 210, 0.14) 0%, transparent 48%), radial-gradient(circle at 86% 92%, rgba(56, 189, 248, 0.11) 0%, transparent 46%)",
    "--nq-card-shadow": "0 18px 42px rgba(0, 0, 0, 0.5)",
    "--nq-card-selected-shadow": "0 24px 54px rgba(0, 0, 0, 0.66), 0 0 24px rgba(94, 106, 210, 0.24)",
    "--nq-divider": "rgba(238, 242, 246, 0.085)",
    "--nq-scroll-thumb": "rgba(238, 242, 246, 0.2)",
    "--nq-toast-bg": "#1D2436",
    "--nq-toast-text": "#EEF2F6",
  };
});

// 队列元信息摘要：来源名称 + 曲目总数 + 总时长
const queueSummaryInfo = computed(() => {
  const list = store.playQueue || [];
  let totalMs = 0;
  let losslessCount = 0;
  for (const s of list) {
    totalMs += getSongDurationMs(s);
    if (getSongQualityRank(s).tier >= 3) losslessCount += 1;
  }
  const totalMin = Math.max(1, Math.round(totalMs / 60000));
  const hours = Math.floor(totalMin / 60);
  const mins = totalMin % 60;
  const durText =
    totalMs <= 0
      ? ""
      : hours > 0
      ? `${hours}小时${mins > 0 ? `${mins}分` : ""}`
      : `约 ${totalMin} 分钟`;
  const sourceTitle = (store.currentQueueTitle || "").trim() || "当前播放队列";
  return {
    sourceTitle,
    count: list.length,
    durText,
    losslessCount,
  };
});

// ================= 2. 视图切换 / 排序展开 / 轻量提示 =================
const activeView = ref<"queue" | "recent">("queue");
const isSortTrayOpen = ref(false);
const toastMessage = ref<string>("");
let toastTimer: number | null = null;

function showLocalToast(msg: string) {
  toastMessage.value = msg;
  if (toastTimer !== null) window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toastMessage.value = "";
    toastTimer = null;
  }, 2400);
}

let clockIntervalId: number | null = null;

function startQueueClock() {
  clockNow.value = Date.now();
  if (clockIntervalId !== null) return;
  clockIntervalId = window.setInterval(() => {
    clockNow.value = Date.now();
  }, 15000);
}

function stopQueueClock() {
  if (clockIntervalId !== null) {
    window.clearInterval(clockIntervalId);
    clockIntervalId = null;
  }
}

watch(isOpen, (open) => {
  if (open) {
    startQueueClock();
    confirmingClear.value = false;
    hoveredCardIdx.value = null;
    syncActiveQueueLastPlayTimestamp();
    syncInitialSelectedCard();
    nextTick(() => {
      if (activeView.value === "queue") {
        scrollToActiveSong(false);
      }
    });
  } else {
    stopQueueClock();
    cancelPointerDrag(true);
  }
});

// ================= 3. 副歌速刷 =================
const isClimaxActive = computed(() =>
  props.climaxSkim !== undefined ? props.climaxSkim : store.climaxModeEnabled
);

const activeChorusWindow = computed(() => {
  const song = store.currentSong;
  const pb = store.currentPlaybackInfo;
  const dur = store.durationMs || song?.durationMs || 0;
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
  const endMs =
    dur > 0 ? Math.min(85000, Math.max(startMs + 15000, dur - 3000)) : 85000;
  return { startMs, endMs, isDetected: false };
});

const chorusSmoothPosMs = ref(0);
let chorusRafId: number | null = null;

function stopChorusClock() {
  if (chorusRafId !== null) {
    cancelAnimationFrame(chorusRafId);
    chorusRafId = null;
  }
}

function startChorusClock() {
  if (chorusRafId !== null) return;
  const tick = () => {
    if (!isOpen.value || !isClimaxActive.value || !store.isPlaying) {
      chorusRafId = null;
      return;
    }
    chorusSmoothPosMs.value = store.getLivePlaybackPositionMs();
    chorusRafId = requestAnimationFrame(tick);
  };
  chorusRafId = requestAnimationFrame(tick);
}

watch(
  [isOpen, isClimaxActive, () => store.isPlaying],
  ([open, active, playing]) => {
    chorusSmoothPosMs.value = store.getLivePlaybackPositionMs();
    if (open && active && playing) {
      startChorusClock();
    } else {
      stopChorusClock();
    }
  },
  { immediate: true }
);

watch(
  () => store.currentPositionMs,
  (v) => {
    if (!store.isPlaying) chorusSmoothPosMs.value = v || 0;
  }
);

const chorusSkimProgressPct = computed(() => {
  const { startMs, endMs } = activeChorusWindow.value;
  const span = Math.max(1000, endMs - startMs);
  const pct = ((chorusSmoothPosMs.value - startMs) / span) * 100;
  return Math.max(0, Math.min(100, pct));
});

function jumpIntoChorusWindow(autoPlay = false) {
  if (!store.currentSong) return;
  store.seekToMs(activeChorusWindow.value.startMs);
  if (autoPlay && !store.isPlaying) {
    store.togglePlayPause();
  }
}

function handleToggleClimaxSkim() {
  const next = !isClimaxActive.value;
  store.climaxModeEnabled = next;
  emit("toggle-climax", next);

  if (next) {
    if (store.currentSong) {
      const { startMs, endMs } = activeChorusWindow.value;
      const pos = store.currentPositionMs;
      if (pos < startMs || pos >= endMs) {
        store.seekToMs(startMs);
      }
      if (!store.isPlaying) {
        store.togglePlayPause();
      }
      const rangeText = `${formatDuration(startMs, "ms")}~${formatDuration(endMs, "ms")}`;
      showLocalToast(`已开启副歌速刷 · ${rangeText}`);
    } else {
      showLocalToast("已开启副歌速刷模式");
    }
  } else {
    showLocalToast("已关闭副歌速刷，恢复整曲播放");
  }
}

// ================= 4. 三种判别排序（收折按需展开，不断播当前曲目） =================
const activeSortMode = ref<QueueDiscriminantSortMode | null>(null);
const sortDirections = ref<Record<QueueDiscriminantSortMode, "asc" | "desc">>({
  artist: "asc",
  quality: "desc",
  duration: "asc",
});

function getSongPrimaryArtist(song: Song): string {
  const authors = splitSongAuthors(song);
  if (authors.length > 0 && authors[0].name) return authors[0].name.trim();
  return (song.artist || "").trim();
}

function getSongQualityRank(song: Song): {
  tier: number;
  label: string;
  isGold: boolean;
} {
  if (
    song.isHiRes ||
    song.hasHiRes ||
    (song.sizeHiRes || 0) > 0 ||
    Boolean(song.hashHiRes)
  ) {
    return { tier: 4, label: "HR", isGold: true };
  }
  if (
    song.isLossless ||
    song.isSq ||
    song.hasSq ||
    (song.sizeFlac || 0) > 0 ||
    Boolean(song.hashFlac)
  ) {
    return { tier: 3, label: "SQ", isGold: false };
  }
  if (
    song.hasHighQuality ||
    (song.size320 || 0) > 0 ||
    Boolean(song.hash320)
  ) {
    return { tier: 2, label: "HQ", isGold: false };
  }
  return { tier: 1, label: "128K", isGold: false };
}

function getSongDurationMs(song: Song): number {
  if (song.durationMs && song.durationMs > 0) return song.durationMs;
  if (song.durationSeconds && song.durationSeconds > 0) {
    return song.durationSeconds * 1000;
  }
  return 0;
}

const sortButtons: Array<{
  mode: QueueDiscriminantSortMode;
  label: string;
  tip: string;
}> = [
  { mode: "artist", label: "歌手聚簇", tip: "按主歌手归纳队列（再点反序）" },
  { mode: "quality", label: "音质优先", tip: "按 Hi-Res > SQ > HQ > 标准重排" },
  { mode: "duration", label: "时长序列", tip: "按曲目时长短↔长排序" },
];

function applyDiscriminantSort(mode: QueueDiscriminantSortMode) {
  if (store.playQueue.length <= 1) {
    showLocalToast("队列曲目不足 2 首，无需重排");
    return;
  }

  let nextDir = sortDirections.value[mode];
  if (activeSortMode.value === mode) {
    nextDir = nextDir === "asc" ? "desc" : "asc";
    sortDirections.value[mode] = nextDir;
  } else {
    activeSortMode.value = mode;
  }

  const activeSongRef = store.currentSong;
  const activeHash = activeSongRef?.hash;
  const activeId = activeSongRef?.id;

  const sorted = [...store.playQueue].sort((a, b) => {
    let cmp = 0;
    if (mode === "artist") {
      cmp = getSongPrimaryArtist(a).localeCompare(getSongPrimaryArtist(b), "zh-CN");
      if (cmp === 0) cmp = (a.album || "").localeCompare(b.album || "", "zh-CN");
      if (cmp === 0) cmp = (a.title || "").localeCompare(b.title || "", "zh-CN");
    } else if (mode === "quality") {
      cmp = getSongQualityRank(a).tier - getSongQualityRank(b).tier;
      if (cmp === 0) {
        const sA = a.sizeHiRes || a.sizeFlac || a.size320 || a.size128 || 0;
        const sB = b.sizeHiRes || b.sizeFlac || b.size320 || b.size128 || 0;
        cmp = sA - sB;
      }
      if (cmp === 0) cmp = (a.title || "").localeCompare(b.title || "", "zh-CN");
    } else {
      cmp = getSongDurationMs(a) - getSongDurationMs(b);
      if (cmp === 0) cmp = (a.title || "").localeCompare(b.title || "", "zh-CN");
    }
    return nextDir === "asc" ? cmp : -cmp;
  });

  store.playQueue = sorted;
  if (activeSongRef) {
    let newIdx = sorted.indexOf(activeSongRef);
    if (newIdx < 0 && (activeHash || activeId)) {
      newIdx = sorted.findIndex(
        (s) =>
          (activeHash && s.hash === activeHash) || (activeId && s.id === activeId)
      );
    }
    if (newIdx >= 0) store.currentIndex = newIdx;
  }

  emit("sort-queue", { mode, direction: nextDir });
  showLocalToast(`已按「${sortButtons.find((b) => b.mode === mode)?.label}」重排`);
  nextTick(() => {
    window.setTimeout(() => scrollToActiveSong(true), 120);
  });
}

// ================= 5. 一键清空队列（二次确认） =================
const confirmingClear = ref(false);
let clearResetTimer: number | null = null;

function handleClearButtonClick() {
  if (store.playQueue.length === 0) {
    showLocalToast("当前队列已经是空的");
    return;
  }
  if (!confirmingClear.value) {
    confirmingClear.value = true;
    if (clearResetTimer !== null) window.clearTimeout(clearResetTimer);
    clearResetTimer = window.setTimeout(() => {
      confirmingClear.value = false;
      clearResetTimer = null;
    }, 3000);
    return;
  }
  if (clearResetTimer !== null) {
    window.clearTimeout(clearResetTimer);
    clearResetTimer = null;
  }
  confirmingClear.value = false;
  store.clearPlayQueue();
  emit("clear-queue");
  showLocalToast("已清空当前播放队列");
}

// ================= 6. 零闪烁跟手拖拽 + 平滑让位补位引擎 =================
interface KeyedQueueRow {
  uid: string;
  song: Song;
  origIndex: number;
}

function buildKeyedRows(songs: Song[]): KeyedQueueRow[] {
  const seenCount = new Map<string, number>();
  return songs.map((song, idx) => {
    const baseKey = song.hash || song.id || `song_${song.title}`;
    const count = seenCount.get(baseKey) || 0;
    seenCount.set(baseKey, count + 1);
    return { uid: `${baseKey}__${count}`, song, origIndex: idx };
  });
}

const queueListRef = ref<HTMLElement | null>(null);
const queueListInnerRef = ref<HTMLElement | null>(null);

const isDraggingRow = ref(false);
const isSettlingDrop = ref(false);
const isDropCommitting = ref(false);
const dragStartIdx = ref<number>(-1);
const dragTargetIdx = ref<number>(-1);
const dragOffsetY = ref<number>(0);
const dragRowStridePx = ref<number>(61);

let pendingDrag: {
  index: number;
  startX: number;
  startY: number;
  rowEl: HTMLElement;
  immediate: boolean;
} | null = null;

let activeDraggingRowEl: HTMLElement | null = null;
let dragStartClientY = 0;
let dragStartScrollTop = 0;
let dragLatestClientY = 0;
let dragMoveRafId: number | null = null;
let dragAutoScrollRafId: number | null = null;
let settleDropTimerId: number | null = null;
let suppressRowClickUntil = 0;

const baseKeyedRows = computed(() => buildKeyedRows(store.playQueue));
const isReorderActive = computed(
  () => isDraggingRow.value || isSettlingDrop.value
);

function isRowCurrentlyPlaying(song: Song, index: number): boolean {
  if (store.currentIndex >= 0) {
    return store.currentIndex === index;
  }
  return Boolean(store.currentSong?.hash && store.currentSong.hash === song.hash);
}

function navigateFromQueueArtist(song: Song) {
  closePopover();
  store.navigateToSongArtist(song);
}

function scrollToActiveSong(smooth = true) {
  const listEl = queueListRef.value;
  if (!listEl) return;
  const activeEl = listEl.querySelector(".nq-song-row.is-active") as HTMLElement | null;
  if (activeEl) {
    activeEl.scrollIntoView({
      behavior: smooth ? "smooth" : "auto",
      block: "center",
    });
  }
}

function measureRowStridePx(): number {
  const innerEl = queueListInnerRef.value;
  if (!innerEl) return 61;
  const rowEls = innerEl.querySelectorAll(".nq-song-row");
  if (rowEls.length >= 2) {
    const first = rowEls[0] as HTMLElement;
    const second = rowEls[1] as HTMLElement;
    const diff = second.offsetTop - first.offsetTop;
    if (diff > 28) return diff;
  }
  if (rowEls.length >= 1) {
    const first = rowEls[0] as HTMLElement;
    if (first.offsetHeight > 28) return first.offsetHeight + 5;
  }
  return 61;
}

function getDisplayOrderIndex(index: number): number {
  const startIdx = dragStartIdx.value;
  const targetIdx = dragTargetIdx.value;
  if (!isReorderActive.value || startIdx < 0 || targetIdx < 0) {
    return index + 1;
  }
  if (index === startIdx) return targetIdx + 1;
  if (startIdx < targetIdx && index > startIdx && index <= targetIdx) return index;
  if (startIdx > targetIdx && index >= targetIdx && index < startIdx) return index + 2;
  return index + 1;
}

function getRowDragStyle(index: number): Record<string, string | number> {
  const startIdx = dragStartIdx.value;
  const targetIdx = dragTargetIdx.value;
  if (!isReorderActive.value || startIdx < 0 || targetIdx < 0) {
    return {};
  }

  const stride = dragRowStridePx.value;
  if (index === startIdx) {
    const ty = Math.round(dragOffsetY.value);
    const scale = isSettlingDrop.value ? 1 : 1.018;
    return {
      zIndex: 42,
      pointerEvents: "none",
      transform: `translate3d(0, ${ty}px, 0) scale(${scale})`,
      transition: isSettlingDrop.value
        ? "transform 0.19s cubic-bezier(0.22, 1, 0.36, 1), box-shadow 0.19s ease"
        : "box-shadow 0.16s ease",
    };
  }

  let shiftPx = 0;
  if (startIdx < targetIdx && index > startIdx && index <= targetIdx) {
    shiftPx = -Math.round(stride);
  } else if (startIdx > targetIdx && index >= targetIdx && index < startIdx) {
    shiftPx = Math.round(stride);
  }

  return {
    zIndex: 1,
    transform: shiftPx !== 0 ? `translate3d(0, ${shiftPx}px, 0)` : "translate3d(0, 0, 0)",
    transition: "transform 0.24s cubic-bezier(0.22, 1, 0.36, 1)",
  };
}

function handleRowPointerDown(
  e: PointerEvent,
  _row: KeyedQueueRow,
  idx: number,
  fromHandle = false
) {
  if (e.button !== 0 || isSettlingDrop.value || isDropCommitting.value) return;
  const targetEl = e.target as HTMLElement | null;
  if (!fromHandle && targetEl?.closest("button")) return;
  if (store.playQueue.length <= 1) return;

  const rowEl = (e.currentTarget as HTMLElement).closest(
    ".nq-song-row"
  ) as HTMLElement | null;
  if (!rowEl) return;

  pendingDrag = {
    index: idx,
    startX: e.clientX,
    startY: e.clientY,
    rowEl,
    immediate: fromHandle,
  };
  dragLatestClientY = e.clientY;

  window.addEventListener("pointermove", handleWindowPointerMove);
  window.addEventListener("pointerup", handleWindowPointerUp);
  window.addEventListener("pointercancel", handleWindowPointerUp);
}

function activatePointerDrag(clientY: number) {
  if (!pendingDrag || !queueListRef.value) return;
  const stride = measureRowStridePx();
  dragRowStridePx.value = stride;
  dragStartIdx.value = pendingDrag.index;
  dragTargetIdx.value = pendingDrag.index;
  dragStartClientY = clientY;
  dragStartScrollTop = queueListRef.value.scrollTop;
  dragOffsetY.value = 0;
  activeDraggingRowEl = pendingDrag.rowEl;
  isSettlingDrop.value = false;
  isDraggingRow.value = true;

  updateDragPositionAndSlot(clientY);
  startAutoScrollLoop();
}

function updateDragPositionAndSlot(clientY: number) {
  if (!isDraggingRow.value || isSettlingDrop.value || !queueListRef.value) {
    return;
  }
  const startIdx = dragStartIdx.value;
  const maxIdx = baseKeyedRows.value.length - 1;
  if (startIdx < 0 || maxIdx <= 0) return;

  const stride = dragRowStridePx.value;
  const scrollDelta = queueListRef.value.scrollTop - dragStartScrollTop;
  const rawOffset = clientY - dragStartClientY + scrollDelta;
  const minOffsetPx = -startIdx * stride - stride * 0.25;
  const maxOffsetPx = (maxIdx - startIdx) * stride + stride * 0.25;
  dragOffsetY.value = Math.round(
    Math.max(minOffsetPx, Math.min(maxOffsetPx, rawOffset))
  );

  // 基于当前偏移直接按 0.5 槽位四舍五入 + 微小迟滞，手感跟手且无跳档
  const exactShift = dragOffsetY.value / stride;
  let nextShift = dragTargetIdx.value - startIdx;
  while (exactShift - nextShift >= 0.48 && startIdx + nextShift < maxIdx) {
    nextShift += 1;
  }
  while (exactShift - nextShift <= -0.48 && startIdx + nextShift > 0) {
    nextShift -= 1;
  }
  dragTargetIdx.value = Math.max(0, Math.min(maxIdx, startIdx + nextShift));
}

function startAutoScrollLoop() {
  stopAutoScrollLoop();
  const tick = () => {
    if (!isDraggingRow.value || isSettlingDrop.value || !queueListRef.value) {
      dragAutoScrollRafId = null;
      return;
    }
    const listEl = queueListRef.value;
    const rect = listEl.getBoundingClientRect();
    const edgeZone = 46;
    let delta = 0;
    if (dragLatestClientY < rect.top + edgeZone) {
      const ratio = Math.min(1, (rect.top + edgeZone - dragLatestClientY) / edgeZone);
      delta = -Math.ceil(ratio * 8);
    } else if (dragLatestClientY > rect.bottom - edgeZone) {
      const ratio = Math.min(1, (dragLatestClientY - (rect.bottom - edgeZone)) / edgeZone);
      delta = Math.ceil(ratio * 8);
    }
    if (delta !== 0) {
      const prevScroll = listEl.scrollTop;
      listEl.scrollTop += delta;
      if (listEl.scrollTop !== prevScroll) {
        updateDragPositionAndSlot(dragLatestClientY);
      }
    }
    dragAutoScrollRafId = requestAnimationFrame(tick);
  };
  dragAutoScrollRafId = requestAnimationFrame(tick);
}

function stopAutoScrollLoop() {
  if (dragAutoScrollRafId !== null) {
    cancelAnimationFrame(dragAutoScrollRafId);
    dragAutoScrollRafId = null;
  }
}

function handleWindowPointerMove(e: PointerEvent) {
  dragLatestClientY = e.clientY;
  if (!isDraggingRow.value) {
    if (!pendingDrag) return;
    const dy = Math.abs(e.clientY - pendingDrag.startY);
    const dx = Math.abs(e.clientX - pendingDrag.startX);
    const threshold = pendingDrag.immediate ? 2 : 5;
    if (dy >= threshold || dx >= threshold + 4) {
      activatePointerDrag(e.clientY);
    }
    return;
  }

  if (dragMoveRafId === null) {
    dragMoveRafId = requestAnimationFrame(() => {
      dragMoveRafId = null;
      if (isDraggingRow.value && !isSettlingDrop.value) {
        updateDragPositionAndSlot(dragLatestClientY);
      }
    });
  }
}

function finalizeSettledDrop(fromIndex: number, toIndex: number) {
  if (settleDropTimerId !== null) {
    window.clearTimeout(settleDropTimerId);
    settleDropTimerId = null;
  }
  isDropCommitting.value = true;
  isDraggingRow.value = false;
  isSettlingDrop.value = false;
  dragStartIdx.value = -1;
  dragTargetIdx.value = -1;
  dragOffsetY.value = 0;
  activeDraggingRowEl = null;

  if (fromIndex >= 0 && toIndex >= 0 && fromIndex !== toIndex) {
    activeSortMode.value = null;
    store.moveQueueItem(fromIndex, toIndex);
    emit("reorder-queue", { fromIndex, toIndex });
  }

  nextTick(() => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        isDropCommitting.value = false;
      });
    });
  });
}

function handleWindowPointerUp() {
  window.removeEventListener("pointermove", handleWindowPointerMove);
  window.removeEventListener("pointerup", handleWindowPointerUp);
  window.removeEventListener("pointercancel", handleWindowPointerUp);
  if (dragMoveRafId !== null) {
    cancelAnimationFrame(dragMoveRafId);
    dragMoveRafId = null;
  }
  stopAutoScrollLoop();
  pendingDrag = null;

  if (!isDraggingRow.value) {
    cancelPointerDrag(true);
    return;
  }

  const fromIndex = dragStartIdx.value;
  const toIndex = dragTargetIdx.value;
  suppressRowClickUntil = performance.now() + 240;

  const targetSettleOffsetPx = Math.round((toIndex - fromIndex) * dragRowStridePx.value);
  const currentOffset = dragOffsetY.value;

  if (Math.abs(targetSettleOffsetPx - currentOffset) < 1 && fromIndex === toIndex) {
    finalizeSettledDrop(fromIndex, toIndex);
    return;
  }

  isSettlingDrop.value = true;
  dragOffsetY.value = targetSettleOffsetPx;

  let settled = false;
  const done = () => {
    if (settled) return;
    settled = true;
    activeDraggingRowEl?.removeEventListener("transitionend", onEnd);
    finalizeSettledDrop(fromIndex, toIndex);
  };
  const onEnd = (ev: TransitionEvent) => {
    if (ev.propertyName === "transform") done();
  };
  activeDraggingRowEl?.addEventListener("transitionend", onEnd);
  settleDropTimerId = window.setTimeout(done, 210);
}

function cancelPointerDrag(aborted: boolean) {
  window.removeEventListener("pointermove", handleWindowPointerMove);
  window.removeEventListener("pointerup", handleWindowPointerUp);
  window.removeEventListener("pointercancel", handleWindowPointerUp);
  if (dragMoveRafId !== null) {
    cancelAnimationFrame(dragMoveRafId);
    dragMoveRafId = null;
  }
  if (settleDropTimerId !== null) {
    window.clearTimeout(settleDropTimerId);
    settleDropTimerId = null;
  }
  stopAutoScrollLoop();
  pendingDrag = null;
  activeDraggingRowEl = null;
  if (aborted || isDraggingRow.value || isSettlingDrop.value) {
    isDraggingRow.value = false;
    isSettlingDrop.value = false;
    isDropCommitting.value = false;
    dragStartIdx.value = -1;
    dragTargetIdx.value = -1;
    dragOffsetY.value = 0;
  }
}

function handleRowClick(song: Song, idx: number) {
  if (performance.now() < suppressRowClickUntil || isReorderActive.value) return;
  store.playSongFromList(song, store.playQueue);
  syncActiveQueueLastPlayTimestamp();
  emit("select-song", song, idx);
}

// ================= 7. 最近队列：真 3D 立体弧度扇形 + 悬停微抬探头预览 =================
const selectedCardIdx = ref<number>(0);
const hoveredCardIdx = ref<number | null>(null);
const clockNow = ref<number>(Date.now());
const activeQueueLastPlayMs = ref<number>(Date.now());
const queueLastPlayedOverrides = ref<Record<string, number>>({});

const recentQueueList = computed<RecentQueueSnapshot[]>(() => {
  const src =
    props.recentQueues !== undefined
      ? props.recentQueues
      : ((store.recentPlayQueues as unknown as RecentQueueSnapshot[]) || []);
  return src.slice(0, 5);
});

function isQueueMatchingCurrent(rq: RecentQueueSnapshot): boolean {
  const curSongs = store.playQueue;
  if (!curSongs.length || !rq.songs?.length) return false;
  if (curSongs.length !== rq.songs.length) return false;
  return (
    curSongs[0]?.hash === rq.songs[0]?.hash &&
    curSongs[curSongs.length - 1]?.hash === rq.songs[rq.songs.length - 1]?.hash
  );
}

function syncActiveQueueLastPlayTimestamp(forceTouch = false) {
  if (!store.playQueue.length) return;
  const now = Date.now();
  activeQueueLastPlayMs.value = now;
  clockNow.value = now;

  if (
    forceTouch &&
    typeof (store as Record<string, any>).touchActiveQueueLastPlayed === "function"
  ) {
    (store as Record<string, any>).touchActiveQueueLastPlayed(store.currentSong);
  }

  const matched = store.recentPlayQueues.find((q) =>
    isQueueMatchingCurrent(q as unknown as RecentQueueSnapshot)
  );
  if (matched) {
    queueLastPlayedOverrides.value[matched.id] = Math.max(
      now,
      Number(matched.lastPlayedAt || matched.savedAt || 0)
    );
  }
}

watch(
  () => [store.currentSong?.hash, store.isPlaying] as const,
  ([hash, playing]) => {
    if (hash || playing) syncActiveQueueLastPlayTimestamp(false);
  },
  { immediate: true }
);

function syncInitialSelectedCard() {
  const list = recentQueueList.value;
  if (list.length === 0) {
    selectedCardIdx.value = 0;
    return;
  }
  const activeIdx = list.findIndex((rq) => isQueueMatchingCurrent(rq));
  selectedCardIdx.value = activeIdx >= 0 ? activeIdx : 0;
}

watch(
  () => recentQueueList.value.length,
  (len) => {
    if (selectedCardIdx.value >= len) {
      selectedCardIdx.value = Math.max(0, len - 1);
    }
  }
);

function extractQueueCovers(rq: RecentQueueSnapshot): string[] {
  if (Array.isArray(rq.covers) && rq.covers.length > 0) {
    return rq.covers.filter(Boolean).slice(0, 3);
  }
  const unique: string[] = [];
  for (const s of rq.songs || []) {
    if (s.coverUrl && !unique.includes(s.coverUrl)) {
      unique.push(s.coverUrl);
      if (unique.length >= 3) break;
    }
  }
  return unique;
}

function resolveQueueLastPlayedMs(rq: RecentQueueSnapshot): number {
  const isCurrent = isQueueMatchingCurrent(rq);
  const overrideMs = rq.id ? queueLastPlayedOverrides.value[rq.id] || 0 : 0;
  const rawCandidates: Array<number | string | undefined> = [
    rq.lastPlayedAt,
    rq.updatedAt,
    rq.savedAt,
    rq.timestamp,
    rq.createdAt,
  ];
  let bestMs = overrideMs;
  if (isCurrent && (store.isPlaying || store.currentSong)) {
    bestMs = Math.max(bestMs, activeQueueLastPlayMs.value);
  }
  for (const raw of rawCandidates) {
    if (raw === undefined || raw === null || raw === "") continue;
    const num =
      typeof raw === "number"
        ? raw
        : /^\d+$/.test(String(raw).trim())
        ? Number(String(raw).trim())
        : Date.parse(String(raw));
    if (Number.isFinite(num) && num > 1000000000) {
      const normalized = num < 10000000000 ? num * 1000 : num;
      if (normalized > bestMs) bestMs = normalized;
    }
  }
  return bestMs;
}

function formatQueueLastPlayedTime(rq: RecentQueueSnapshot): string {
  const now = clockNow.value;
  if (isQueueMatchingCurrent(rq) && store.isPlaying) return "刚刚";
  const ms = resolveQueueLastPlayedMs(rq);
  if (!Number.isFinite(ms) || ms <= 0) {
    return rq.lastPlayedAtText || rq.savedAtText || "近期";
  }
  const diffSec = Math.max(0, Math.floor((now - ms) / 1000));
  if (diffSec < 60) return "刚刚";
  if (diffSec < 3600) return `${Math.max(1, Math.floor(diffSec / 60))}分钟前`;
  const d = new Date(ms);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const today = new Date(now);
  if (
    d.getFullYear() === today.getFullYear() &&
    d.getMonth() === today.getMonth() &&
    d.getDate() === today.getDate()
  ) {
    return `今天 ${hh}:${mm}`;
  }
  return `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${hh}:${mm}`;
}

function getQueueQualitySummary(rq: RecentQueueSnapshot): string {
  const songs = rq.songs || [];
  if (!songs.length) return "0 首";
  let losslessCount = 0;
  for (const s of songs) {
    if (getSongQualityRank(s).tier >= 3) losslessCount++;
  }
  if (losslessCount > 0) {
    return `${rq.trackCount || songs.length} 首 · ${losslessCount} 首无损`;
  }
  return `${rq.trackCount || songs.length} 首曲目`;
}

function isSongActiveInRecentQueue(
  rq: RecentQueueSnapshot,
  song: Song,
  sIdx: number
): boolean {
  if (isQueueMatchingCurrent(rq) && store.currentSong?.hash) {
    return store.currentSong.hash === song.hash;
  }
  if (rq.activeHash && song.hash) return rq.activeHash === song.hash;
  return sIdx === 0;
}

/**
 * 真 3D 弧度错位扇形布局：
 * - C 位（rel === 0）保持 translate3d(0,0,0) rotateZ(0) rotateY(0) scale(1)，确保文本封面 100% 物理像素锐利；
 * - 两侧卡片按真实扇面圆弧倾角（rotateZ）、立体透视偏转（rotateY）与纵向二次弧线落差分布；
 * - 悬停侧卡时仅原地微抬探头并微幅前移亮起，绝不抢换 C 位导致光标失焦跳变。
 */
function getFanCardStyle(idx: number): Record<string, string | number> {
  const total = recentQueueList.value.length;
  const focusIdx = Math.max(0, Math.min(total - 1, selectedCardIdx.value));
  const rel = idx - focusIdx;
  const absRel = Math.abs(rel);
  const isHovered = hoveredCardIdx.value === idx;

  if (rel === 0) {
    return {
      zIndex: 60,
      opacity: 1,
      transform: "translate3d(0px, 0px, 0px) rotateZ(0deg) rotateY(0deg) scale(1)",
    };
  }

  const stepX = total <= 2 ? 106 : total === 3 ? 90 : total === 4 ? 76 : 64;
  const tx = Math.round(rel * stepX);
  const baseArcY = absRel * absRel * 15 + absRel * 7;
  const ty = Math.round(baseArcY - (isHovered ? 18 : 0));
  const tz = Math.round(-absRel * 46 + (isHovered ? 24 : 0));
  const rotZ = Number((rel * 6.5 * (isHovered ? 0.72 : 1)).toFixed(2));
  const rotY = Number((-rel * 7.2 * (isHovered ? 0.75 : 1)).toFixed(2));
  const scale = Number(
    Math.max(0.82, 1 - absRel * 0.055 + (isHovered ? 0.035 : 0)).toFixed(3)
  );
  const opacity = Number(
    Math.max(0.68, 1 - absRel * 0.1 + (isHovered ? 0.16 : 0)).toFixed(2)
  );

  return {
    zIndex: isHovered ? 56 : 50 - absRel * 8,
    opacity,
    transform: `translate3d(${tx}px, ${ty}px, ${tz}px) rotateZ(${rotZ}deg) rotateY(${rotY}deg) scale(${scale})`,
  };
}

let lastWheelSwitchWall = 0;
function handleRecentDeckWheel(e: WheelEvent) {
  const total = recentQueueList.value.length;
  if (total <= 1) return;
  const now = performance.now();
  if (now - lastWheelSwitchWall < 150) return;
  const delta = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
  if (Math.abs(delta) < 8) return;
  lastWheelSwitchWall = now;
  hoveredCardIdx.value = null;
  if (delta > 0 && selectedCardIdx.value < total - 1) {
    selectedCardIdx.value += 1;
  } else if (delta < 0 && selectedCardIdx.value > 0) {
    selectedCardIdx.value -= 1;
  }
}

function handleCardClick(rq: RecentQueueSnapshot, idx: number) {
  if (selectedCardIdx.value !== idx) {
    hoveredCardIdx.value = null;
    selectedCardIdx.value = idx;
    return;
  }
  handleRestoreRecentQueue(rq, idx);
}

function handleRestoreRecentQueue(
  rq: RecentQueueSnapshot,
  idx: number,
  specificSong?: Song
) {
  if (!rq.songs || rq.songs.length === 0) {
    showLocalToast("该记忆队列暂无可播放曲目");
    return;
  }

  if (rq.id) {
    queueLastPlayedOverrides.value[rq.id] = Date.now();
  }
  emit("restore-queue", rq);

  if (specificSong) {
    store.playSongFromList(specificSong, rq.songs, rq.title);
  } else {
    const existsInStore = store.recentPlayQueues.some((q) => q.id === rq.id);
    if (existsInStore) {
      store.switchToRecentQueue(rq.id);
    } else {
      const targetSong =
        (rq.activeHash && rq.songs.find((s) => s.hash === rq.activeHash)) ||
        rq.songs[0];
      store.playSongFromList(targetSong, rq.songs, rq.title);
    }
  }

  syncActiveQueueLastPlayTimestamp();
  activeView.value = "queue";
  showLocalToast(
    specificSong
      ? `已切入「${rq.title}」· 《${specificSong.title}》`
      : `已切入队列「${rq.title}」`
  );
  nextTick(() => {
    window.setTimeout(() => scrollToActiveSong(true), 200);
  });
  void idx;
}

function handleRemoveRecentCard(rq: RecentQueueSnapshot, idx: number) {
  emit("remove-recent-queue", { queue: rq, index: idx });
  const storeIdx = store.recentPlayQueues.findIndex((q) => q.id === rq.id);
  if (storeIdx >= 0) {
    store.removeRecentQueue(storeIdx);
  }
  hoveredCardIdx.value = null;
  if (selectedCardIdx.value >= recentQueueList.value.length) {
    selectedCardIdx.value = Math.max(0, recentQueueList.value.length - 1);
  }
  showLocalToast(`已移除「${rq.title}」`);
}

function switchQueueTab(tab: "queue" | "recent") {
  activeView.value = tab;
  if (tab === "recent") {
    clockNow.value = Date.now();
    syncActiveQueueLastPlayTimestamp(true);
    syncInitialSelectedCard();
  } else {
    nextTick(() => {
      window.setTimeout(() => scrollToActiveSong(false), 160);
    });
  }
}

// ================= 8. 键盘快捷键 =================
function handleKeyDown(e: KeyboardEvent) {
  if (!isOpen.value) return;
  if (e.key === "Escape") {
    if (isDraggingRow.value) {
      cancelPointerDrag(true);
      return;
    }
    if (activeView.value === "recent") {
      switchQueueTab("queue");
      return;
    }
    closePopover();
    return;
  }

  if (activeView.value === "recent" && recentQueueList.value.length > 0) {
    if (e.key === "ArrowLeft" && selectedCardIdx.value > 0) {
      e.preventDefault();
      hoveredCardIdx.value = null;
      selectedCardIdx.value -= 1;
    } else if (
      e.key === "ArrowRight" &&
      selectedCardIdx.value < recentQueueList.value.length - 1
    ) {
      e.preventDefault();
      hoveredCardIdx.value = null;
      selectedCardIdx.value += 1;
    } else if (e.key === "Enter") {
      const target = recentQueueList.value[selectedCardIdx.value];
      if (target) {
        e.preventDefault();
        handleRestoreRecentQueue(target, selectedCardIdx.value);
      }
    }
  }
}

onMounted(() => {
  window.addEventListener("keydown", handleKeyDown);
  window.addEventListener("resize", refreshNowPlayingAnchorRect, { passive: true });
  if (isOpen.value) {
    startQueueClock();
    refreshNowPlayingAnchorRect();
    syncActiveQueueLastPlayTimestamp(true);
    syncInitialSelectedCard();
    nextTick(() => scrollToActiveSong(false));
  }
});

onBeforeUnmount(() => {
  stopChorusClock();
  stopQueueClock();
  window.removeEventListener("keydown", handleKeyDown);
  window.removeEventListener("resize", refreshNowPlayingAnchorRect);
  cancelPointerDrag(true);
  if (toastTimer !== null) window.clearTimeout(toastTimer);
  if (clearResetTimer !== null) window.clearTimeout(clearResetTimer);
});
</script>

<template>
  <Teleport to="body">
    <transition name="np-queue-spring">
      <div
        v-if="isOpen"
        class="nq-floating-layer"
        :class="[
          `scene-${resolvedScene}`,
          isLightSurface ? 'scheme-watercolor_light' : 'scheme-abyssal_dark',
        ]"
        @click.self="closePopover"
      >
        <div
          class="nq-popover-window"
          :class="[
            `scene-${resolvedScene}`,
            {
              'is-recent-mode': activeView === 'recent',
              'is-dark-surface': !isLightSurface,
              'is-anchor-tethered': Boolean(activeNpAnchor),
            },
          ]"
          :style="[themeSurfaceStyle, anchoredNowPlayingStyle]"
          @click.stop
        >
          <!-- 极简双场景共用顶栏 -->
          <header class="nq-header">
            <div class="nq-header-left">
              <div class="nq-seg-switcher" aria-label="队列视图切换">
                <button
                  type="button"
                  class="nq-seg-btn"
                  :class="{ active: activeView === 'queue' }"
                  :aria-pressed="activeView === 'queue'"
                  @click="switchQueueTab('queue')"
                >
                  <span>队列</span>
                  <span class="nq-seg-count">
                    <RollingCounter :value="store.playQueue.length" />
                  </span>
                </button>
                <button
                  type="button"
                  class="nq-seg-btn"
                  :class="{ active: activeView === 'recent' }"
                  :aria-pressed="activeView === 'recent'"
                  @click="switchQueueTab('recent')"
                >
                  <span>最近</span>
                  <span v-if="recentQueueList.length > 0" class="nq-seg-count">
                    {{ recentQueueList.length }}
                  </span>
                </button>
              </div>
            </div>

            <div class="nq-header-actions">
              <!-- 副歌速刷开关 -->
              <button
                type="button"
                class="nq-action-pill nq-skim-btn"
                :class="{ active: isClimaxActive }"
                data-tip="副歌高潮区间速刷"
                @click="handleToggleClimaxSkim"
              >
                <PlumpIcon name="sparkle" :size="13" />
                <span>副歌</span>
              </button>

              <!-- 排序展开开关（按需折叠） -->
              <button
                v-if="activeView === 'queue' && store.playQueue.length > 1"
                type="button"
                class="nq-icon-btn"
                :class="{ active: isSortTrayOpen || activeSortMode !== null }"
                data-tip="智能排序"
                @click="isSortTrayOpen = !isSortTrayOpen"
              >
                <PlumpIcon name="sort" :size="15" />
              </button>

              <!-- 定位在播歌曲 -->
              <button
                v-if="activeView === 'queue' && store.playQueue.length > 0"
                type="button"
                class="nq-icon-btn"
                data-tip="定位当前在播"
                @click="scrollToActiveSong(true)"
              >
                <PlumpIcon name="locate" :size="15" />
              </button>

              <!-- 清空队列（二次确认） -->
              <button
                v-if="activeView === 'queue' && store.playQueue.length > 0"
                type="button"
                class="nq-icon-btn nq-clear-icon"
                :class="{ confirming: confirmingClear }"
                :data-tip="confirmingClear ? '再次点击确认清空' : '清空队列'"
                @click="handleClearButtonClick"
              >
                <PlumpIcon name="trash" :size="15" />
              </button>

              <!-- 关闭 -->
              <button
                type="button"
                class="nq-icon-btn"
                data-tip="关闭队列"
                @click="closePopover"
              >
                <PlumpIcon name="close" :size="14" />
              </button>
            </div>
          </header>

          <!-- 轻量队列来源与总时长摘要条 -->
          <div
            v-if="activeView === 'queue' && store.playQueue.length > 0"
            class="nq-summary-strip"
          >
            <div class="nq-summary-source">
              <span class="nq-summary-dot"></span>
              <span class="nq-summary-title">{{ queueSummaryInfo.sourceTitle }}</span>
            </div>
            <div class="nq-summary-stats">
              <span>共 {{ queueSummaryInfo.count }} 首</span>
              <template v-if="queueSummaryInfo.durText">
                <span class="nq-summary-sep">·</span>
                <span>{{ queueSummaryInfo.durText }}</span>
              </template>
              <template v-if="queueSummaryInfo.losslessCount > 0">
                <span class="nq-summary-sep">·</span>
                <span class="nq-summary-sq">{{ queueSummaryInfo.losslessCount }} 无损</span>
              </template>
            </div>
          </div>

          <!-- 按需展开的轻量智能排序条（采用主题轻胶囊，不受全局下划线 Tab 劫持） -->
          <transition name="nq-banner-fold">
            <div
              v-if="isSortTrayOpen && activeView === 'queue' && store.playQueue.length > 1"
              class="nq-sort-tray"
            >
              <div class="nq-sort-pills" aria-label="队列排序">
                <button
                  v-for="btn in sortButtons"
                  :key="btn.mode"
                  type="button"
                  class="nq-sort-pill"
                  :class="{ active: activeSortMode === btn.mode }"
                  :aria-pressed="activeSortMode === btn.mode"
                  :data-tip="btn.tip"
                  @click="applyDiscriminantSort(btn.mode)"
                >
                  <PlumpIcon
                    :name="
                      btn.mode === 'artist'
                        ? 'artists'
                        : btn.mode === 'quality'
                          ? 'quality-gem'
                          : 'clock'
                    "
                    :size="13"
                  />
                  <span>{{ btn.label }}</span>
                  <span v-if="activeSortMode === btn.mode" class="nq-sort-dir">
                    {{ sortDirections[btn.mode] === "asc" ? "↑" : "↓" }}
                  </span>
                </button>
              </div>
              <span class="nq-drag-tip">拖动行调序</span>
            </div>
          </transition>

          <!-- 副歌速刷黄金区间状态条 -->
          <transition name="nq-banner-fold">
            <div v-if="isClimaxActive && store.currentSong" class="nq-chorus-banner">
              <div class="nq-cb-info">
                <span class="nq-cb-badge">
                  {{ activeChorusWindow.isDetected ? "精准高潮" : "黄金副歌" }}
                </span>
                <span class="nq-cb-time">
                  {{ formatDuration(activeChorusWindow.startMs, "ms") }} ~
                  {{ formatDuration(activeChorusWindow.endMs, "ms") }}
                </span>
              </div>
              <div class="nq-cb-right">
                <div class="nq-cb-progress-rail">
                  <div
                    class="nq-cb-progress-fill"
                    :style="{
                      transform: `scaleX(${(chorusSkimProgressPct / 100).toFixed(5)})`,
                    }"
                  ></div>
                </div>
                <button
                  type="button"
                  class="nq-cb-replay-btn"
                  @click="jumpIntoChorusWindow(true)"
                >
                  重听
                </button>
              </div>
            </div>
          </transition>

          <!-- 主内容舞台：当前列表 <-> 最近队列 3D 弧形扇面 -->
          <div class="nq-stage-container">
            <transition name="nq-view-switch" mode="out-in">
              <!-- 视图 A：当前播放队列 -->
              <div v-if="activeView === 'queue'" key="queue" class="nq-queue-pane">
                <div v-if="store.playQueue.length === 0" class="nq-empty-state">
                  <div class="nq-empty-disc">
                    <PlumpIcon name="disc" :size="34" />
                  </div>
                  <p class="nq-empty-title">当前播放队列为空</p>
                  <p class="nq-empty-sub">从发现页挑选好歌，或从「最近」视图恢复历史队列</p>
                  <button
                    v-if="recentQueueList.length > 0"
                    type="button"
                    class="nq-empty-cta"
                    @click="switchQueueTab('recent')"
                  >
                    <PlumpIcon name="history" :size="14" />
                    <span>恢复最近 {{ recentQueueList.length }} 组队列</span>
                  </button>
                </div>

                <div v-else class="nq-list-stage">
                  <div ref="queueListRef" class="nq-song-list">
                    <div ref="queueListInnerRef" class="nq-song-list-inner">
                      <div
                        class="nq-song-list-group"
                        :class="{
                          'is-drop-committing': isDropCommitting,
                          'is-reordering': isReorderActive,
                        }"
                      >
                        <div
                          v-for="(row, idx) in baseKeyedRows"
                          :key="row.uid"
                          class="nq-song-row"
                          :class="{
                            'is-active': isRowCurrentlyPlaying(row.song, idx),
                            'is-dragging-row': isReorderActive && dragStartIdx === idx,
                          }"
                          :style="getRowDragStyle(idx)"
                          @pointerdown="handleRowPointerDown($event, row, idx, false)"
                          @click="handleRowClick(row.song, idx)"
                          @contextmenu.prevent="
                            store.openSongContextMenu($event, row.song, store.playQueue)
                          "
                        >
                          <div
                            class="nq-drag-handle"
                            :class="{ active: isReorderActive && dragStartIdx === idx }"
                            data-tip="按住拖动排序"
                            @pointerdown.stop="handleRowPointerDown($event, row, idx, true)"
                          >
                            <i></i><i></i><i></i><i></i><i></i><i></i>
                          </div>

                          <div class="nq-row-index">
                            <span
                              v-if="
                                isRowCurrentlyPlaying(row.song, idx) &&
                                !(isReorderActive && dragStartIdx === idx)
                              "
                              class="nq-eq-bars"
                              :class="{ paused: !store.isPlaying }"
                            >
                              <i></i><i></i><i></i>
                            </span>
                            <span v-else>{{ String(getDisplayOrderIndex(idx)).padStart(2, "0") }}</span>
                          </div>

                          <div class="nq-row-cover-wrap">
                            <img
                              v-if="row.song.coverUrl"
                              :src="row.song.coverUrl"
                              class="nq-row-cover"
                              alt=""
                              loading="lazy"
                              decoding="async"
                            />
                            <div v-else class="nq-row-cover fallback">
                              <PlumpIcon name="disc" :size="17" />
                            </div>
                          </div>

                          <div class="nq-row-meta">
                            <div class="nq-row-title-line">
                              <span class="nq-row-title">{{ row.song.title }}</span>
                              <span
                                v-if="getSongQualityRank(row.song).tier >= 2"
                                class="nq-q-badge"
                                :class="{
                                  gold: getSongQualityRank(row.song).isGold,
                                  sq: getSongQualityRank(row.song).tier === 3,
                                }"
                              >
                                {{ getSongQualityRank(row.song).label }}
                              </span>
                              <span v-if="row.song.isVip" class="nq-vip-tag">VIP</span>
                            </div>
                            <div class="nq-row-artist-wrap">
                              <button
                                v-if="row.song.artist"
                                type="button"
                                class="nq-row-artist is-clickable"
                                title="点击进入歌手主页"
                                @pointerdown.stop
                                @click.stop="navigateFromQueueArtist(row.song)"
                              >
                                {{ row.song.artist }}
                              </button>
                              <span v-else class="nq-row-artist">未知艺术家</span>
                            </div>
                          </div>

                          <span class="nq-row-dur">
                            {{ formatDuration(getSongDurationMs(row.song), "ms") }}
                          </span>

                          <div class="nq-row-actions" @click.stop>
                            <button
                              v-if="!isRowCurrentlyPlaying(row.song, idx)"
                              type="button"
                              class="nq-row-act-btn"
                              data-tip="下一首播放"
                              @click="store.insertPlayNext(row.song)"
                            >
                              <PlumpIcon name="top" :size="13" />
                            </button>
                            <button
                              type="button"
                              class="nq-row-act-btn danger"
                              data-tip="移除"
                              @click="store.removeSongFromQueue(idx)"
                            >
                              <PlumpIcon name="close" :size="13" />
                            </button>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              </div>

              <!-- 视图 B：最近队列 3D 立体弧度扇形视图 -->
              <div v-else key="recent" class="nq-recent-pane">
                <div v-if="recentQueueList.length === 0" class="nq-empty-state">
                  <div class="nq-empty-disc">
                    <PlumpIcon name="history" :size="34" />
                  </div>
                  <p class="nq-empty-title">暂无最近队列记忆</p>
                  <p class="nq-empty-sub">播放歌单或专辑时将自动记忆最近 5 组队列</p>
                  <button
                    type="button"
                    class="nq-empty-cta"
                    @click="switchQueueTab('queue')"
                  >
                    <span>返回当前队列</span>
                  </button>
                </div>

                <div
                  v-else
                  class="nq-fan-stage-wrap"
                  @wheel.prevent="handleRecentDeckWheel"
                >
                  <div class="nq-fan-deck">
                    <article
                      v-for="(rq, idx) in recentQueueList"
                      :key="rq.id || idx"
                      class="nq-fan-card"
                      :class="{
                        'is-selected': selectedCardIdx === idx,
                        'is-hovered': hoveredCardIdx === idx && selectedCardIdx !== idx,
                        'is-current-match': isQueueMatchingCurrent(rq),
                      }"
                      :style="getFanCardStyle(idx)"
                      @mouseenter="hoveredCardIdx = idx"
                      @mouseleave="hoveredCardIdx === idx && (hoveredCardIdx = null)"
                      @click="handleCardClick(rq, idx)"
                    >
                      <div class="nq-fc-sheen"></div>

                      <div class="nq-fc-top">
                        <span
                          class="nq-fc-slot"
                          :class="{ 'is-live': isQueueMatchingCurrent(rq) }"
                        >
                          <i v-if="isQueueMatchingCurrent(rq)" class="nq-fc-live-dot"></i>
                          {{ isQueueMatchingCurrent(rq) ? "当前队列" : `记忆 #0${idx + 1}` }}
                        </span>

                        <div class="nq-fc-top-right">
                          <span class="nq-fc-time">
                            <PlumpIcon name="clock" :size="11" />
                            <span>{{ formatQueueLastPlayedTime(rq) }}</span>
                          </span>
                          <button
                            type="button"
                            class="nq-fc-del-btn"
                            data-tip="移除记忆"
                            @click.stop="handleRemoveRecentCard(rq, idx)"
                          >
                            <PlumpIcon name="close" :size="11" />
                          </button>
                        </div>
                      </div>

                      <div class="nq-fc-hero">
                        <div class="nq-fc-cover-stack">
                          <template v-if="extractQueueCovers(rq).length > 0">
                            <img
                              v-for="(url, cIdx) in extractQueueCovers(rq)"
                              :key="cIdx"
                              :src="url"
                              class="nq-fc-cover-img"
                              :class="`pos-${cIdx}`"
                              alt=""
                            />
                          </template>
                          <div v-else class="nq-fc-cover-fallback">
                            <PlumpIcon name="disc" :size="24" />
                          </div>
                        </div>

                        <div class="nq-fc-hero-meta">
                          <h4 class="nq-fc-title">{{ rq.title || "未命名播放队列" }}</h4>
                          <span class="nq-fc-summary-text">{{ getQueueQualitySummary(rq) }}</span>
                        </div>
                      </div>

                      <div class="nq-fc-divider"></div>

                      <ul class="nq-fc-tracks">
                        <li
                          v-for="(song, sIdx) in (rq.songs || []).slice(0, 5)"
                          :key="song.hash || sIdx"
                          class="nq-fc-track-item"
                          :class="{
                            'is-last-active': isSongActiveInRecentQueue(rq, song, sIdx),
                          }"
                          @click.stop="
                            selectedCardIdx === idx
                              ? handleRestoreRecentQueue(rq, idx, song)
                              : ((hoveredCardIdx = null), (selectedCardIdx = idx))
                          "
                        >
                          <span class="nq-fc-track-idx">
                            {{
                              isSongActiveInRecentQueue(rq, song, sIdx)
                                ? "♪"
                                : String(sIdx + 1).padStart(2, "0")
                            }}
                          </span>
                          <img
                            v-if="song.coverUrl"
                            :src="song.coverUrl"
                            class="nq-fc-mini-art"
                            alt=""
                            loading="lazy"
                            decoding="async"
                          />
                          <span v-else class="nq-fc-mini-dot">♪</span>
                          <div class="nq-fc-track-text">
                            <span class="nq-fc-track-name">{{ song.title }}</span>
                            <span class="nq-fc-track-artist">{{ song.artist || "未知歌手" }}</span>
                          </div>
                          <span class="nq-fc-track-dur">
                            {{ formatDuration(getSongDurationMs(song), "ms") }}
                          </span>
                        </li>
                      </ul>

                      <div class="nq-fc-footer-hint">
                        {{
                          selectedCardIdx === idx
                            ? "点击卡片切入播放"
                            : "点击卡片旋切至正面"
                        }}
                      </div>
                    </article>
                  </div>

                  <!-- 弧面索引圆点指示器 -->
                  <div v-if="recentQueueList.length > 1" class="nq-fan-dots">
                    <button
                      v-for="(rq, idx) in recentQueueList"
                      :key="rq.id || idx"
                      type="button"
                      class="nq-fan-dot-btn"
                      :class="{ active: selectedCardIdx === idx }"
                      :aria-label="`切换到第 ${idx + 1} 组队列`"
                      @click="
                        hoveredCardIdx = null;
                        selectedCardIdx = idx;
                      "
                    ></button>
                  </div>
                </div>
              </div>
            </transition>
          </div>

          <transition name="nq-toast-pop">
            <div v-if="toastMessage" class="nq-toast-pill">
              <span class="nq-toast-dot"></span>
              <span>{{ toastMessage }}</span>
            </div>
          </transition>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
/* ============================================================================
   1. 双场景统一非遮蔽悬浮层定位
   ============================================================================ */
.nq-floating-layer {
  position: fixed;
  inset: 0;
  z-index: 2320;
  background: transparent;
  display: flex;
  pointer-events: auto;
}

/* 主界面：紧贴底栏上方屏幕右下方 */
.nq-floating-layer.scene-main-bar {
  justify-content: flex-end;
  align-items: flex-end;
  padding: 20px 22px 104px 20px;
}

/* 播放页：安全靠右停靠，窄窗绝不裁切，宽屏不挡左栏大封面 */
.nq-floating-layer.scene-now-playing {
  justify-content: flex-end;
  align-items: center;
  padding: 64px 24px 36px 24px;
}

.nq-popover-window {
  position: relative;
  width: min(448px, 92vw);
  height: min(608px, 78vh);
  border-radius: var(--bm-r-xl, 24px);
  padding: 18px 18px 16px;
  color: var(--nq-text-1);
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow: hidden;
  user-select: none;
  transition:
    width 0.34s cubic-bezier(0.22, 1, 0.36, 1),
    height 0.34s cubic-bezier(0.22, 1, 0.36, 1),
    box-shadow 0.3s ease;
}

.nq-popover-window.scene-now-playing {
  width: min(560px, calc(100vw - 48px));
  height: min(650px, 82vh);
  padding: 20px 20px 18px;
}

.nq-popover-window.is-recent-mode {
  width: min(590px, calc(100vw - 48px));
  height: min(608px, 78vh);
}

.nq-popover-window.scene-now-playing.is-recent-mode {
  width: min(620px, calc(100vw - 48px));
  height: min(650px, 82vh);
}

/* ============================================================================
   2. 极简顶栏 & 分段切换器
   ============================================================================ */
.nq-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-shrink: 0;
}

.nq-header-left {
  display: flex;
  align-items: center;
  min-width: 0;
}

.nq-seg-switcher {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 3px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--nq-seg-track, var(--nq-chip-bg));
}

.nq-seg-btn {
  border: none;
  height: 29px;
  padding: 0 13px;
  border-radius: var(--bm-r-pill, 999px);
  background: transparent;
  color: var(--nq-text-2);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  transition:
    background 0.2s ease,
    color 0.18s ease,
    box-shadow 0.2s ease,
    transform 0.18s cubic-bezier(0.22, 1, 0.36, 1);
}

.nq-seg-btn:hover:not(.active) {
  color: var(--nq-text-1);
  background: var(--nq-accent-1-soft);
}

.nq-seg-btn.active {
  background: linear-gradient(
    135deg in oklab,
    var(--nq-accent-1) 0%,
    color-mix(in oklab, var(--nq-accent-1) 70%, var(--nq-accent-2)) 100%
  );
  color: var(--nq-on-accent-1);
  box-shadow: 0 4px 14px var(--nq-accent-1-glow);
}

.nq-summary-strip {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 2px 4px;
  flex-shrink: 0;
}

.nq-summary-source {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.nq-summary-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: linear-gradient(135deg, var(--nq-accent-1), var(--nq-accent-2));
  flex-shrink: 0;
}

.nq-summary-title {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 750;
  color: var(--nq-text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nq-summary-stats {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 650;
  font-variant-numeric: tabular-nums;
  color: var(--nq-text-3);
  white-space: nowrap;
  flex-shrink: 0;
}

.nq-summary-sep {
  opacity: 0.55;
}

.nq-summary-sq {
  color: var(--nq-accent-2);
  font-weight: 750;
}

.nq-seg-count {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  font-variant-numeric: tabular-nums;
  opacity: 0.85;
}

.nq-header-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.nq-action-pill {
  height: 30px;
  padding: 0 10px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--nq-chip-bg);
  color: var(--nq-text-2);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  transition: background-color 0.18s ease, color 0.18s ease, transform 0.18s ease;
}

.nq-action-pill:hover {
  background: var(--nq-chip-hover);
  color: var(--nq-text-1);
}

.nq-skim-btn.active {
  background: linear-gradient(135deg, var(--nq-accent-1), var(--nq-accent-2));
  color: var(--nq-on-accent-1);
  box-shadow: 0 4px 14px var(--nq-accent-1-glow);
}

.nq-icon-btn {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 50%;
  background: var(--nq-chip-bg);
  color: var(--nq-text-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background-color 0.16s ease, color 0.16s ease, transform 0.18s ease;
}

.nq-icon-btn:hover {
  background: var(--nq-chip-hover);
  color: var(--nq-text-1);
  transform: scale(1.06);
}

.nq-icon-btn.active {
  background: var(--nq-accent-1-soft);
  color: var(--nq-accent-1);
}

.nq-clear-icon.confirming {
  background: var(--nq-danger-soft);
  color: var(--nq-danger);
  animation: nqConfirmShake 0.3s ease;
}

/* ============================================================================
   3. 按需展开排序条 & 副歌速刷条
   ============================================================================ */
.nq-sort-tray {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 10px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--nq-row-bg);
  flex-shrink: 0;
}

.nq-sort-pills {
  display: flex;
  align-items: center;
  gap: 8px;
}

.nq-sort-pill {
  height: 26px;
  padding: 0 9px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: transparent;
  color: var(--nq-text-2);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  cursor: pointer;
  transition: background-color 0.15s ease, color 0.15s ease;
}

.nq-sort-pill:hover {
  background: var(--nq-chip-hover);
  color: var(--nq-text-1);
}

.nq-sort-pill.active {
  background: var(--nq-accent-1-soft);
  color: var(--nq-accent-1);
  font-weight: 800;
}

.nq-sort-dir {
  font-weight: 900;
}

.nq-drag-tip {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--nq-text-3);
  white-space: nowrap;
}

.nq-chorus-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 7px 11px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--nq-accent-1-soft);
  flex-shrink: 0;
}

.nq-cb-info {
  display: flex;
  align-items: center;
  gap: 8px;
}

.nq-cb-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  color: var(--nq-on-accent-2);
  background: var(--nq-accent-2);
  padding: 2px 7px;
  border-radius: var(--bm-r-pill, 999px);
}

.nq-cb-time {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  font-variant-numeric: tabular-nums;
  color: var(--nq-text-1);
}

.nq-cb-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.nq-cb-progress-rail {
  width: 60px;
  height: 4px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--nq-chip-bg);
  overflow: hidden;
}

.nq-cb-progress-fill {
  width: 100%;
  height: 100%;
  border-radius: var(--bm-r-pill, 999px);
  transform-origin: left center;
  will-change: transform;
  background: linear-gradient(90deg, var(--nq-accent-1), var(--nq-accent-2));
}

.nq-cb-replay-btn {
  border: none;
  padding: 2px 9px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--nq-chip-bg);
  color: var(--nq-text-1);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  cursor: pointer;
}

.nq-cb-replay-btn:hover {
  background: var(--nq-chip-hover);
}

/* ============================================================================
   4. 当前队列滚动列表 & 零闪烁拖拽补位
   ============================================================================ */
.nq-stage-container {
  position: relative;
  flex: 1 1 0%;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.nq-queue-pane,
.nq-recent-pane {
  flex: 1 1 0%;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.nq-list-stage {
  flex: 1 1 0%;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.nq-song-list {
  flex: 1 1 0%;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding-right: 4px;
}

.nq-song-list::-webkit-scrollbar {
  width: 5px;
}

.nq-song-list::-webkit-scrollbar-thumb {
  background: var(--nq-scroll-thumb);
  border-radius: var(--bm-r-pill, 999px);
}

.nq-song-list-inner {
  padding: 2px 0 6px;
}

.nq-song-list-group {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

/* 提交排序瞬间彻底禁用行级 CSS transition，根除松手 1 帧闪烁 */
.nq-song-list-group.is-drop-committing .nq-song-row {
  transition: none !important;
}

.nq-song-row {
  position: relative;
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 8px 10px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--nq-row-bg);
  cursor: pointer;
  backface-visibility: hidden;
  transition: background 0.18s ease, box-shadow 0.18s ease, transform 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.nq-song-row:hover {
  background: var(--nq-row-hover);
}

.nq-song-row.is-active {
  background: var(--nq-row-active);
}

.nq-song-row.is-dragging-row {
  background: var(--nq-row-drag-bg) !important;
  box-shadow:
    0 18px 40px rgba(0, 0, 0, 0.36),
    0 0 22px var(--nq-accent-1-glow) !important;
  cursor: grabbing;
}

.nq-drag-handle {
  width: 14px;
  height: 24px;
  display: grid;
  grid-template-columns: repeat(2, 3px);
  grid-template-rows: repeat(3, 3px);
  gap: 2.5px;
  align-content: center;
  justify-content: center;
  flex-shrink: 0;
  opacity: 0.36;
  cursor: grab;
  touch-action: none;
  transition: opacity 0.16s ease;
}

.nq-song-row:hover .nq-drag-handle,
.nq-drag-handle.active {
  opacity: 0.92;
}

.nq-drag-handle i {
  display: block;
  width: 3px;
  height: 3px;
  border-radius: 50%;
  background: var(--nq-text-2);
}

.nq-row-index {
  width: 22px;
  text-align: center;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--nq-text-3);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.nq-song-row.is-active .nq-row-index {
  color: var(--nq-accent-1);
}

.nq-eq-bars {
  display: inline-flex;
  align-items: flex-end;
  gap: 2px;
  height: 12px;
}

.nq-eq-bars i {
  display: block;
  width: 2.5px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--nq-accent-1);
  animation: nqEqBounce 0.9s ease-in-out infinite alternate;
}

.nq-eq-bars i:nth-child(1) {
  height: 6px;
}
.nq-eq-bars i:nth-child(2) {
  height: 12px;
  animation-delay: 0.25s;
}
.nq-eq-bars i:nth-child(3) {
  height: 8px;
  animation-delay: 0.45s;
}

.nq-eq-bars.paused i {
  animation-play-state: paused;
  height: 4px;
}

.nq-row-cover-wrap {
  width: 40px;
  height: 40px;
  border-radius: var(--bm-r-sm, 10px);
  overflow: hidden;
  flex-shrink: 0;
  background: var(--nq-chip-bg);
}

.nq-row-cover {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: flex;
  align-items: center;
  justify-content: center;
}

.nq-row-meta {
  flex: 1;
  min-width: 0;
}

.nq-row-title-line {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
}

.nq-row-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: 700;
  color: var(--nq-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nq-song-row.is-active .nq-row-title {
  color: var(--nq-accent-1);
  font-weight: 800;
}

.nq-q-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 1px 5px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--nq-accent-1-soft);
  color: var(--nq-accent-1);
  flex-shrink: 0;
}

.nq-q-badge.sq {
  background: var(--nq-accent-2-soft);
  color: var(--nq-accent-2);
}

.nq-q-badge.gold {
  background: var(--nq-gold-soft);
  color: var(--nq-gold);
}

.nq-vip-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 1px 5px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--nq-gold-soft);
  color: var(--nq-gold);
  flex-shrink: 0;
}

.nq-row-artist-wrap {
  margin-top: 2px;
  display: flex;
  align-items: center;
  min-width: 0;
  padding: 1px 5px;
  margin-left: -5px;
  margin-right: -5px;
}

.nq-row-artist {
  border: none;
  background: transparent;
  padding: 1px 5px;
  margin: -1px -5px;
  border-radius: var(--bm-r-xs, 6px);
  font-size: var(--bm-fs-micro, 11px);
  color: var(--nq-text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
  text-align: left;
  transition: color 0.16s ease, background-color 0.16s ease;
}

.nq-row-artist.is-clickable {
  cursor: pointer;
}

.nq-row-artist.is-clickable:hover {
  color: var(--nq-accent-1);
  background: var(--nq-accent-1-soft);
}

.nq-row-dur {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: var(--nq-text-3);
  flex-shrink: 0;
}

.nq-row-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  opacity: 0;
  transition: opacity 0.15s ease;
  flex-shrink: 0;
}

.nq-song-row:hover .nq-row-actions {
  opacity: 1;
}

.nq-row-act-btn {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--nq-chip-bg);
  color: var(--nq-text-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.nq-row-act-btn:hover {
  background: var(--nq-chip-hover);
  color: var(--nq-text-1);
}

.nq-row-act-btn.danger:hover {
  background: var(--nq-danger-soft);
  color: var(--nq-danger);
}

/* ============================================================================
   5. 最近队列：真 3D 立体弧度扇形舞台
   ============================================================================ */
.nq-fan-stage-wrap {
  flex: 1 1 0%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 6px 0 2px;
}

.nq-fan-deck {
  position: relative;
  flex: 1 1 0%;
  min-height: 0;
  width: 100%;
  perspective: 1200px;
  transform-style: preserve-3d;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: visible;
}

.nq-fan-card {
  position: absolute;
  width: min(358px, 78%);
  height: min(444px, 90%);
  border-radius: var(--bm-r-xl, 24px);
  padding: 16px 18px 14px;
  background: var(--nq-card-bg);
  box-shadow: var(--nq-card-shadow);
  display: flex;
  flex-direction: column;
  gap: 10px;
  cursor: pointer;
  overflow: hidden;
  transform-origin: 50% 82%;
  will-change: transform, opacity;
  transition:
    transform 0.42s cubic-bezier(0.22, 1.15, 0.36, 1),
    opacity 0.28s ease,
    box-shadow 0.32s ease;
}

.nq-fan-card.is-selected {
  background: var(--nq-card-selected-bg);
  box-shadow: var(--nq-card-selected-shadow);
}

.nq-fan-card.is-hovered {
  box-shadow:
    var(--nq-card-selected-shadow),
    0 12px 24px rgba(0, 0, 0, 0.18);
}

.nq-fc-sheen {
  position: absolute;
  inset: 0;
  background: var(--nq-card-sheen);
  pointer-events: none;
}

.nq-fc-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  position: relative;
  z-index: 2;
}

.nq-fc-slot {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  color: var(--nq-text-2);
  background: var(--nq-chip-bg);
  padding: 3px 9px;
  border-radius: var(--bm-r-pill, 999px);
}

.nq-fc-slot.is-live {
  background: linear-gradient(135deg, var(--nq-accent-1), var(--nq-accent-2));
  color: var(--nq-on-accent-1);
}

.nq-fc-live-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  animation: nqBlinkDot 1.4s ease-in-out infinite;
}

.nq-fc-top-right {
  display: flex;
  align-items: center;
  gap: 5px;
}

.nq-fc-time {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--nq-text-2);
  background: var(--nq-chip-bg);
  padding: 3px 8px;
  border-radius: var(--bm-r-pill, 999px);
}

.nq-fc-del-btn {
  width: 22px;
  height: 22px;
  border: none;
  border-radius: 50%;
  background: var(--nq-chip-bg);
  color: var(--nq-text-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  opacity: 0;
  cursor: pointer;
  transition: opacity 0.16s ease, background-color 0.16s ease, color 0.16s ease;
}

.nq-fan-card:hover .nq-fc-del-btn,
.nq-fan-card.is-selected .nq-fc-del-btn {
  opacity: 1;
}

.nq-fc-del-btn:hover {
  background: var(--nq-danger-soft);
  color: var(--nq-danger);
}

.nq-fc-hero {
  display: flex;
  align-items: center;
  gap: 12px;
  position: relative;
  z-index: 2;
}

.nq-fc-cover-stack {
  position: relative;
  width: 86px;
  height: 60px;
  flex-shrink: 0;
}

.nq-fc-cover-img {
  position: absolute;
  width: 58px;
  height: 58px;
  border-radius: var(--bm-r-md, 14px);
  object-fit: cover;
  box-shadow: 0 8px 18px rgba(0, 0, 0, 0.34);
  transition: transform 0.28s cubic-bezier(0.22, 1, 0.36, 1);
}

.nq-fc-cover-img.pos-0 {
  left: 0;
  top: 1px;
  z-index: 3;
}
.nq-fc-cover-img.pos-1 {
  left: 14px;
  top: 4px;
  z-index: 2;
  opacity: 0.82;
  transform: scale(0.92);
}
.nq-fc-cover-img.pos-2 {
  left: 26px;
  top: 7px;
  z-index: 1;
  opacity: 0.56;
  transform: scale(0.84);
}

.nq-fc-cover-fallback {
  width: 58px;
  height: 58px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--nq-chip-bg);
  color: var(--nq-accent-1);
  display: flex;
  align-items: center;
  justify-content: center;
}

.nq-fc-hero-meta {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.nq-fc-title {
  margin: 0;
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: 800;
  color: var(--nq-text-1);
  line-height: 1.3;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.nq-fc-summary-text {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  color: var(--nq-text-2);
}

.nq-fc-divider {
  height: 1px;
  background: var(--nq-divider);
  position: relative;
  z-index: 2;
}

.nq-fc-tracks {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 5px;
  flex: 1;
  min-height: 0;
  position: relative;
  z-index: 2;
}

.nq-fc-track-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--nq-row-bg);
  transition: background-color 0.15s ease, transform 0.15s ease;
}

.nq-fc-track-item:hover {
  background: var(--nq-row-hover);
  transform: translateX(2px);
}

.nq-fc-track-item.is-last-active {
  background: var(--nq-accent-1-soft);
}

.nq-fc-track-idx {
  width: 16px;
  text-align: center;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--nq-text-3);
}

.nq-fc-track-item.is-last-active .nq-fc-track-idx {
  color: var(--nq-accent-1);
}

.nq-fc-mini-art {
  width: 28px;
  height: 28px;
  border-radius: var(--bm-r-xs, 6px);
  object-fit: cover;
  flex-shrink: 0;
}

.nq-fc-mini-dot {
  width: 28px;
  height: 28px;
  border-radius: var(--bm-r-xs, 6px);
  background: var(--nq-chip-bg);
  color: var(--nq-accent-1);
  font-size: var(--bm-fs-micro, 11px);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.nq-fc-track-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.nq-fc-track-name {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--nq-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nq-fc-track-artist {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--nq-text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.nq-fc-track-dur {
  font-size: var(--bm-fs-micro, 11px);
  font-variant-numeric: tabular-nums;
  color: var(--nq-text-3);
}

.nq-fc-footer-hint {
  text-align: center;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  color: var(--nq-text-3);
  position: relative;
  z-index: 2;
}

.nq-fan-dots {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding-top: 8px;
  flex-shrink: 0;
}

.nq-fan-dot-btn {
  width: 7px;
  height: 7px;
  padding: 0;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--nq-chip-hover);
  cursor: pointer;
  transition: width 0.25s cubic-bezier(0.22, 1, 0.36, 1), background-color 0.2s ease;
}

.nq-fan-dot-btn.active {
  width: 22px;
  background: var(--nq-accent-1);
}

/* ============================================================================
   6. 空状态 & 内联 Toast & 动画
   ============================================================================ */
.nq-empty-state {
  flex: 1 1 0%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  text-align: center;
  padding: 24px;
}

.nq-empty-disc {
  width: 64px;
  height: 64px;
  border-radius: var(--bm-r-xl, 24px);
  background: var(--nq-chip-bg);
  color: var(--nq-accent-1);
  display: flex;
  align-items: center;
  justify-content: center;
}

.nq-empty-title {
  margin: 4px 0 0;
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: 800;
  color: var(--nq-text-1);
}

.nq-empty-sub {
  margin: 0;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--nq-text-2);
}

.nq-empty-cta {
  margin-top: 6px;
  height: 34px;
  padding: 0 14px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: linear-gradient(135deg, var(--nq-accent-1), var(--nq-accent-2));
  color: var(--nq-on-accent-1);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}

.nq-toast-pill {
  position: absolute;
  left: 50%;
  bottom: 14px;
  transform: translateX(-50%);
  z-index: 95;
  padding: 6px 13px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--nq-toast-bg);
  color: var(--nq-toast-text, #ffffff);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
  box-shadow: 0 10px 26px rgba(0, 0, 0, 0.36);
  pointer-events: none;
}

.nq-toast-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--nq-accent-2);
}

.np-queue-spring-enter-active {
  transition: opacity 0.24s cubic-bezier(0.16, 1, 0.3, 1);
}
.np-queue-spring-leave-active {
  transition: opacity 0.18s ease;
}
.np-queue-spring-enter-from,
.np-queue-spring-leave-to {
  opacity: 0;
}
.np-queue-spring-enter-active .nq-popover-window {
  transition: transform 0.36s var(--ease-spring-tactile, cubic-bezier(0.22, 1.15, 0.36, 1));
}
.np-queue-spring-leave-active .nq-popover-window {
  transition: transform 0.18s ease;
}
.np-queue-spring-enter-from .nq-popover-window {
  transform: translate3d(0, 16px, 0) scale(0.95);
}
.np-queue-spring-leave-to .nq-popover-window {
  transform: translate3d(0, 8px, 0) scale(0.96);
}

.nq-banner-fold-enter-active,
.nq-banner-fold-leave-active {
  transition: opacity 0.2s ease, transform 0.22s cubic-bezier(0.22, 1, 0.36, 1);
}
.nq-banner-fold-enter-from,
.nq-banner-fold-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}

.nq-view-switch-enter-active,
.nq-view-switch-leave-active {
  transition: opacity 0.2s ease, transform 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}
.nq-view-switch-enter-from {
  opacity: 0;
  transform: translateY(8px) scale(0.98);
}
.nq-view-switch-leave-to {
  opacity: 0;
  transform: translateY(-6px) scale(0.98);
}

.nq-toast-pop-enter-active,
.nq-toast-pop-leave-active {
  transition: opacity 0.18s ease, transform 0.22s cubic-bezier(0.22, 1.2, 0.36, 1);
}
.nq-toast-pop-enter-from,
.nq-toast-pop-leave-to {
  opacity: 0;
  transform: translate(-50%, 8px) scale(0.94);
}

@keyframes nqBlinkDot {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}

@keyframes nqEqBounce {
  0% {
    transform: scaleY(0.42);
  }
  100% {
    transform: scaleY(1);
  }
}

@keyframes nqConfirmShake {
  0%,
  100% {
    transform: translateX(0);
  }
  25% {
    transform: translateX(-3px);
  }
  75% {
    transform: translateX(3px);
  }
}

@media (max-width: 960px) {
  .nq-floating-layer.scene-now-playing {
    justify-content: center;
    padding: 24px;
  }
}
</style>

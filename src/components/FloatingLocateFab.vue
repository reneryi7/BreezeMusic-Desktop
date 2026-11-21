<template>
  <Teleport to="body">
    <div
      v-if="showLocateBtn || showTopBtn"
      class="bm-fab-stack"
      :class="[
        `scheme-${store.resolvedColorScheme}`,
        store.resolvedColorScheme === 'watercolor_light'
          ? `light-variant-${store.lightThemeVariant}`
          : `dark-variant-${store.darkThemeVariant}`,
      ]"
      :style="{ bottom: `${computedBottom}px` }"
    >
      <Transition name="bm-fab-pop">
        <button
          v-if="showLocateBtn"
          type="button"
          class="bm-circle-fab is-locate"
          :class="{ 'is-playing': store.isPlaying }"
          :data-tip="locateTooltip"
          :aria-label="locateTooltip"
          @click="handleLocateCurrent"
        >
          <PlumpIcon name="locate" :size="18" />
        </button>
      </Transition>

      <Transition name="bm-fab-pop">
        <button
          v-if="showTopBtn"
          ref="topBtnRef"
          type="button"
          class="bm-circle-fab is-top"
          :data-tip="`回到顶部（共 ${songs.length} 首）`"
          :aria-label="`回到顶部（共 ${songs.length} 首）`"
          @click="handleScrollTop"
        >
          <PlumpIcon name="top" :size="18" />
        </button>
      </Transition>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import { isSameTrackIdentity } from "../api/kugou";
import type { Song } from "../types";
import PlumpIcon from "./PlumpIcon.vue";

const props = withDefaults(
  defineProps<{
    songs: Song[];
    scrollContainerSelector?: string;
    targetContainerSelector?: string;
    hasBatchBar?: boolean;
    bottomOffsetPx?: number;
  }>(),
  {
    scrollContainerSelector: ".skyline-stage-viewport",
    targetContainerSelector: "",
    hasBatchBar: false,
    bottomOffsetPx: 108,
  }
);

const store = useAppStore();
const viewportScrollTop = ref(0);
const topBtnRef = ref<HTMLButtonElement | null>(null);
let boundScroller: HTMLElement | Window | null = null;
let pulseTimer: number | null = null;
let activePulseEl: HTMLElement | null = null;

function resolveLocateKey(song?: Partial<Song> | null): string {
  if (!song) return "";
  const hash = String(song.hash || "").trim();
  if (hash) return hash;
  return String(song.id || "").trim();
}

const matchedSongIndex = computed(() => {
  const cur = store.currentSong;
  if (!cur || props.songs.length < 20) return -1;
  return props.songs.findIndex((s) => isSameTrackIdentity(s, cur));
});

const matchedSong = computed(() =>
  matchedSongIndex.value >= 0 ? props.songs[matchedSongIndex.value] : null
);

const isStageActive = computed(
  () => !store.isNowPlayingOpen && !store.isMiniPlayerMode
);

// 定位当前播放歌曲按钮：列表 >= 20 首且当前播放曲目在本列表内时独立显示
const showLocateBtn = computed(
  () => isStageActive.value && props.songs.length >= 20 && matchedSongIndex.value >= 0
);

// 返回顶部按钮：列表 > 30 首且发生滚动 (> 260px) 时独立显示
const showTopBtn = computed(
  () => isStageActive.value && props.songs.length > 30 && viewportScrollTop.value > 260
);

const computedBottom = computed(
  () => props.bottomOffsetPx + (props.hasBatchBar ? 74 : 0)
);

const locateTooltip = computed(() => {
  if (store.currentSong?.title && matchedSongIndex.value >= 0) {
    return `定位当前播放：${store.currentSong.title}（第 ${matchedSongIndex.value + 1} 首）`;
  }
  return "定位到当前播放歌曲";
});

function readScrollTop(el: HTMLElement | Window | null): number {
  if (!el) return 0;
  if (el === window) return window.scrollY || document.documentElement.scrollTop || 0;
  return (el as HTMLElement).scrollTop || 0;
}

function handleScroll() {
  viewportScrollTop.value = readScrollTop(boundScroller);
}

function bindScroller() {
  unbindScroller();
  const el =
    (document.querySelector(props.scrollContainerSelector) as HTMLElement | null) ||
    window;
  boundScroller = el;
  el.addEventListener("scroll", handleScroll, { passive: true });
  handleScroll();
}

function unbindScroller() {
  if (!boundScroller) return;
  boundScroller.removeEventListener("scroll", handleScroll);
  boundScroller = null;
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches
  );
}

function triggerRowPulse(rowEl: HTMLElement) {
  if (activePulseEl && activePulseEl !== rowEl) {
    activePulseEl.classList.remove("usr-locate-pulse");
  }
  if (pulseTimer !== null) {
    window.clearTimeout(pulseTimer);
    pulseTimer = null;
  }
  rowEl.classList.remove("usr-locate-pulse");
  void rowEl.offsetWidth;
  rowEl.classList.add("usr-locate-pulse");
  activePulseEl = rowEl;
  pulseTimer = window.setTimeout(() => {
    rowEl.classList.remove("usr-locate-pulse");
    if (activePulseEl === rowEl) activePulseEl = null;
    pulseTimer = null;
  }, 1220);
}

function handleScrollTop() {
  const behavior: ScrollBehavior = prefersReducedMotion() ? "auto" : "smooth";

  // 键盘无障碍焦点保护：返回顶部按钮在滚回顶部后会隐藏，预先将焦点平滑交还给滚动容器防焦点丢失
  if (document.activeElement === topBtnRef.value) {
    if (boundScroller && boundScroller !== window) {
      const container = boundScroller as HTMLElement;
      if (!container.hasAttribute("tabindex")) {
        container.setAttribute("tabindex", "-1");
      }
      container.focus({ preventScroll: true });
    } else {
      topBtnRef.value?.blur();
    }
  }

  if (boundScroller === window) {
    window.scrollTo({ top: 0, behavior });
  } else if (boundScroller) {
    (boundScroller as HTMLElement).scrollTo({ top: 0, behavior });
  }
}

async function handleLocateCurrent() {
  const behavior: ScrollBehavior = prefersReducedMotion() ? "auto" : "smooth";
  await nextTick();
  const root: ParentNode =
    (props.targetContainerSelector
      ? document.querySelector(props.targetContainerSelector)
      : document.querySelector(props.scrollContainerSelector)) || document;

  const candidateKeys = [
    resolveLocateKey(matchedSong.value),
    resolveLocateKey(store.currentSong),
  ].filter(Boolean);

  let targetRow: HTMLElement | null = null;
  if (typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    for (const key of candidateKeys) {
      targetRow = root.querySelector(
        `[data-locate-song-key="${CSS.escape(key)}"]`
      ) as HTMLElement | null;
      if (targetRow) break;
    }
  }
  if (!targetRow) {
    targetRow = root.querySelector(".unified-song-row.active") as HTMLElement | null;
  }
  if (!targetRow && typeof CSS !== "undefined" && typeof CSS.escape === "function") {
    for (const key of candidateKeys) {
      targetRow = document.querySelector(
        `[data-locate-song-key="${CSS.escape(key)}"]`
      ) as HTMLElement | null;
      if (targetRow) break;
    }
  }

  if (targetRow) {
    targetRow.scrollIntoView({ behavior, block: "center", inline: "nearest" });
    triggerRowPulse(targetRow);
  }
}

watch(
  () => [props.scrollContainerSelector, props.songs.length],
  () => {
    nextTick(() => bindScroller());
  }
);

onMounted(() => {
  nextTick(() => bindScroller());
});

onBeforeUnmount(() => {
  unbindScroller();
  if (pulseTimer !== null) {
    window.clearTimeout(pulseTimer);
    pulseTimer = null;
  }
  if (activePulseEl) {
    activePulseEl.classList.remove("usr-locate-pulse");
    activePulseEl = null;
  }
});
</script>

<style>
.bm-fab-stack {
  position: fixed;
  right: 26px;
  z-index: 64;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  pointer-events: none;
  transition: bottom 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.bm-circle-fab {
  pointer-events: auto;
  width: 42px;
  height: 42px;
  border: none;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  background: color-mix(in srgb, var(--bg-elevated, #141924) 86%, var(--primary) 14%);
  backdrop-filter: blur(18px) saturate(145%);
  -webkit-backdrop-filter: blur(18px) saturate(145%);
  color: var(--text-primary);
  box-shadow:
    0 10px 26px rgba(0, 0, 0, 0.34),
    0 3px 10px color-mix(in srgb, var(--primary) 20%, transparent);
  transition:
    transform 0.24s cubic-bezier(0.22, 1.35, 0.36, 1),
    background-color 0.2s ease,
    color 0.2s ease,
    box-shadow 0.24s ease;
}

.bm-circle-fab.is-locate {
  background: var(--grad-kugou-primary, var(--primary));
  color: #ffffff;
  box-shadow:
    0 12px 28px color-mix(in srgb, var(--primary) 38%, rgba(0, 0, 0, 0.28)),
    0 4px 12px color-mix(in srgb, var(--primary) 26%, transparent);
}

.bm-circle-fab:hover {
  transform: translate3d(0, -2px, 0) scale(1.06);
}

.bm-circle-fab.is-top:hover {
  background: color-mix(in srgb, var(--bg-elevated, #141924) 74%, var(--primary) 26%);
  color: var(--primary);
}

.bm-circle-fab:active {
  transform: scale(0.93);
}

.bm-circle-fab:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 3px;
}

/* D2 丝绒星云 */
.bm-fab-stack.scheme-abyssal_dark.dark-variant-velvet_nebula .bm-circle-fab.is-top {
  background: #2a1d24;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  color: #f9efe6;
  box-shadow: 0 12px 26px rgba(8, 2, 6, 0.52);
}

.bm-fab-stack.scheme-abyssal_dark.dark-variant-velvet_nebula .bm-circle-fab.is-top:hover {
  background: #36242e;
  color: var(--primary);
}

/* L1 云宣水彩 */
.bm-fab-stack.scheme-watercolor_light .bm-circle-fab.is-top {
  background: rgba(255, 255, 255, 0.92);
  color: var(--text-primary);
  box-shadow:
    0 10px 24px rgba(15, 23, 42, 0.12),
    0 3px 10px color-mix(in srgb, var(--primary) 16%, transparent);
}

.bm-fab-stack.scheme-watercolor_light .bm-circle-fab.is-top:hover {
  background: #ffffff;
  color: var(--primary);
}

/* L2 烘焙燕麦 */
.bm-fab-stack.scheme-watercolor_light.light-variant-nordic_oat .bm-circle-fab.is-top {
  background: #faf6ee;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  color: #3d352c;
  box-shadow:
    0 10px 24px rgba(68, 56, 38, 0.14),
    0 3px 10px rgba(15, 118, 110, 0.14);
}

.bm-fab-stack.scheme-watercolor_light.light-variant-nordic_oat .bm-circle-fab.is-top:hover {
  background: #fffdf8;
  color: var(--primary);
}

.bm-fab-pop-enter-active {
  transition:
    opacity 0.22s ease,
    transform 0.34s var(--ease-spring-snappy, cubic-bezier(0.22, 1.35, 0.36, 1));
}

.bm-fab-pop-leave-active {
  transition:
    opacity 0.16s ease,
    transform 0.16s ease;
}

.bm-fab-pop-enter-from,
.bm-fab-pop-leave-to {
  opacity: 0;
  transform: translate3d(0, 12px, 0) scale(0.84);
}

@media (prefers-reduced-motion: reduce) {
  .bm-fab-stack,
  .bm-circle-fab,
  .bm-fab-pop-enter-active,
  .bm-fab-pop-leave-active {
    transition: none;
  }
}
</style>

import { isAppForegroundActive } from "./windowPowerState";

/**
 * 自适应单/双列歌曲网格与空间 FLIP 归位引擎
 * ============================================================================
 * 1. 容器宽度统一决策（均衡档）：
 *    - 首次渲染基准：容器宽 >= 1040px 为双列（单列宽 >= 510px），< 1040px 为单列；
 *      与 skyline-components.css 中的 @container (max-width: 1039px) 像素级对齐，首帧零跳变。
 *    - 滞回缓冲带（36px）：单→双需达到 >= 1056px，双→单需回落至 <= 1020px，
 *      彻底杜绝手动拖拽窗口边缘停在临界点附近时的鬼畜反复抖动。
 * 2. 空间 FLIP 真实归位：
 *    - 仅对视口可见区域内的直接歌曲行执行 GPU 合成 translate3d 位移（绝不使用 scaleX 压扁封面与字形），
 *      配合高阻尼弹簧曲线与 12ms 轻微波浪错峰；
 *    - 通过 O(log N) 二分定位可见窗口起始行，即便 5000 首本地曲库仍保持 < 1ms 测量开销；
 *    - 自动锁定滚动锚点（以视口首行曲目为基准校正 scrollTop），避免单↔双列高度翻倍/减半导致视野跳飞；
 *    - 全面遵守应用三档动效（best / standard / none）与系统 prefers-reduced-motion。
 */

export const SONG_GRID_BASE_THRESHOLD_PX = 1040;
export const SONG_GRID_ENTER_TWO_COL_PX = 1056;
export const SONG_GRID_EXIT_TWO_COL_PX = 1020;

export type SongGridColumnCount = 1 | 2;

const GRID_SELECTOR = ".frameless-songs-grid-2col";
const MAX_ANIMATED_ITEMS = 44;

export function resolveAdaptiveSongGridCols(
  widthPx: number,
  currentCols: SongGridColumnCount | null,
): SongGridColumnCount {
  if (!Number.isFinite(widthPx) || widthPx <= 0) {
    return currentCols ?? 1;
  }
  if (currentCols === null) {
    return widthPx >= SONG_GRID_BASE_THRESHOLD_PX ? 2 : 1;
  }
  if (currentCols === 1) {
    return widthPx >= SONG_GRID_ENTER_TWO_COL_PX ? 2 : 1;
  }
  return widthPx <= SONG_GRID_EXIT_TWO_COL_PX ? 1 : 2;
}

interface GridTrackState {
  cols: SongGridColumnCount | null;
  lastWidth: number;
  hasVisibleLayout: boolean;
  reflowClearTimer: number | null;
  activeAnims: Set<Animation>;
}

export interface AdaptiveSongGridController {
  scanSubtree: (root?: ParentNode) => void;
  dispose: () => void;
}

function getDirectGridItems(grid: HTMLElement): HTMLElement[] {
  const result: HTMLElement[] = [];
  const children = grid.children;
  for (let i = 0; i < children.length; i++) {
    const child = children[i];
    if (
      child instanceof HTMLElement &&
      (child.classList.contains("unified-song-row") ||
        child.classList.contains("sr-folded-children-fullrow"))
    ) {
      result.push(child);
    }
  }
  return result;
}

function findScrollHost(grid: HTMLElement): HTMLElement {
  let cur: HTMLElement | null = grid;
  while (cur && cur !== document.body && cur !== document.documentElement) {
    const style = window.getComputedStyle(cur);
    const oy = style.overflowY;
    if (
      (oy === "auto" || oy === "scroll" || oy === "overlay") &&
      cur.scrollHeight > cur.clientHeight + 2
    ) {
      return cur;
    }
    cur = cur.parentElement;
  }
  const viewport = grid.closest(".skyline-stage-viewport") as HTMLElement | null;
  if (viewport) return viewport;
  return document.documentElement;
}

/**
 * 二分查找第一个底部超过 topBound 的子项索引（回退 2 个槽位覆盖双列同行偏移）
 */
function findFirstIndexNearTop(items: HTMLElement[], topBound: number): number {
  const len = items.length;
  if (len <= 32) return 0;
  let lo = 0;
  let hi = len - 1;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    const rect = items[mid].getBoundingClientRect();
    if (rect.bottom < topBound) {
      lo = mid + 1;
    } else {
      hi = mid;
    }
  }
  return Math.max(0, lo - 2);
}

function getMotionTiming(): {
  animate: boolean;
  duration: number;
  staggerStep: number;
  maxDelay: number;
  easing: string;
} {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return { animate: false, duration: 0, staggerStep: 0, maxDelay: 0, easing: "ease" };
  }
  const motionLevel = document.documentElement.dataset.motionLevel || "best";
  const prefersReduced =
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (motionLevel === "none" || prefersReduced) {
    return { animate: false, duration: 0, staggerStep: 0, maxDelay: 0, easing: "ease" };
  }
  if (motionLevel === "standard") {
    return {
      animate: true,
      duration: 230,
      staggerStep: 7,
      maxDelay: 56,
      easing: "cubic-bezier(0.22, 1, 0.36, 1)",
    };
  }
  return {
    animate: true,
    duration: 340,
    staggerStep: 12,
    maxDelay: 108,
    easing: "cubic-bezier(0.22, 1, 0.36, 1)",
  };
}

export function initAdaptiveSongGridEngine(): AdaptiveSongGridController {
  if (typeof window === "undefined" || typeof document === "undefined") {
    return {
      scanSubtree: () => {},
      dispose: () => {},
    };
  }

  const states = new Map<HTMLElement, GridTrackState>();
  const pendingWidths = new Map<HTMLElement, number>();
  let flushRafId: number | null = null;
  let disposed = false;

  const cancelGridAnims = (state: GridTrackState) => {
    if (state.activeAnims.size === 0) return;
    for (const anim of state.activeAnims) {
      try {
        anim.cancel();
      } catch {
        // ignore
      }
    }
    state.activeAnims.clear();
  };

  const cleanupGrid = (grid: HTMLElement) => {
    const state = states.get(grid);
    if (!state) return;
    cancelGridAnims(state);
    if (state.reflowClearTimer !== null) {
      window.clearTimeout(state.reflowClearTimer);
      state.reflowClearTimer = null;
    }
    grid.classList.remove("is-col-reflowing");
    resizeObserver?.unobserve(grid);
    pendingWidths.delete(grid);
    states.delete(grid);
  };

  const pruneDisconnected = () => {
    for (const grid of states.keys()) {
      if (!grid.isConnected) {
        cleanupGrid(grid);
      }
    }
  };

  const applyColumnChange = (
    grid: HTMLElement,
    state: GridTrackState,
    nextCols: SongGridColumnCount,
    widthPx: number,
  ) => {
    const gridRect = grid.getBoundingClientRect();
    const isVisibleNow =
      grid.isConnected &&
      grid.offsetParent !== null &&
      gridRect.width > 1 &&
      gridRect.height > 1;

    const timing = getMotionTiming();
    const canAnimate =
      state.hasVisibleLayout &&
      state.cols !== null &&
      isVisibleNow &&
      isAppForegroundActive() &&
      timing.animate;

    if (!isVisibleNow) {
      cancelGridAnims(state);
      grid.dataset.songCols = String(nextCols);
      state.cols = nextCols;
      state.lastWidth = widthPx;
      return;
    }

    const items = getDirectGridItems(grid);
    if (items.length === 0) {
      cancelGridAnims(state);
      grid.dataset.songCols = String(nextCols);
      state.cols = nextCols;
      state.lastWidth = widthPx;
      state.hasVisibleLayout = true;
      return;
    }

    const scrollHost = findScrollHost(grid);
    const hostRect =
      scrollHost === document.documentElement
        ? { top: 0, bottom: window.innerHeight }
        : scrollHost.getBoundingClientRect();
    const viewTop = Math.max(0, hostRect.top);
    const viewBottom = Math.min(window.innerHeight, hostRect.bottom);
    const shouldLockScroll = scrollHost.scrollTop > 4;

    // 1. FIRST：在切列之前读取视口及下方缓冲区内元素的实时屏幕坐标（含进行中动画的当前插值位）
    const firstRects = new Map<HTMLElement, { left: number; top: number }>();
    let anchorEl: HTMLElement | null = null;
    let anchorTopBefore = 0;

    const captureTopBound = viewTop - 240;
    const captureBottomBound = nextCols === 2 ? viewBottom + 720 : viewBottom + 280;
    const firstStartIdx = findFirstIndexNearTop(items, captureTopBound);

    for (let i = firstStartIdx; i < items.length; i++) {
      const item = items[i];
      const rect = item.getBoundingClientRect();
      if (rect.top > captureBottomBound) break;
      if (rect.bottom < captureTopBound) continue;

      if (
        !anchorEl &&
        item.classList.contains("unified-song-row") &&
        rect.bottom > viewTop + 8 &&
        rect.top < viewBottom
      ) {
        anchorEl = item;
        anchorTopBefore = rect.top;
      }

      if (canAnimate) {
        firstRects.set(item, { left: rect.left, top: rect.top });
      }
    }

    // 2. 取消旧的 FLIP 动画，防止其 transform 污染 LAST 布局测量
    cancelGridAnims(state);

    // 3. 切换列数（临时关闭原生 overflow-anchor 避免与手工曲目锚点校正冲突）
    grid.classList.add("is-col-reflowing");
    grid.dataset.songCols = String(nextCols);
    state.cols = nextCols;
    state.lastWidth = widthPx;
    state.hasVisibleLayout = true;

    // 4. 滚动锚点保位：确保切换单/双列瞬间当前视野第一首歌原地不动
    if (shouldLockScroll && anchorEl) {
      const anchorTopAfter = anchorEl.getBoundingClientRect().top;
      const scrollDelta = anchorTopAfter - anchorTopBefore;
      if (Number.isFinite(scrollDelta) && Math.abs(scrollDelta) > 1.5) {
        scrollHost.scrollTop += scrollDelta;
      }
    }

    if (!canAnimate) {
      grid.classList.remove("is-col-reflowing");
      return;
    }

    // 5. LAST + INVERT + PLAY：仅对新视口可见行应用纯位移 WAAPI 动画（绝不 scaleX 压扁封面文字）
    const lastTopBound = viewTop - 160;
    const lastBottomBound = viewBottom + 180;
    const lastStartIdx = findFirstIndexNearTop(items, lastTopBound);
    let animatedCount = 0;

    for (let i = lastStartIdx; i < items.length; i++) {
      if (animatedCount >= MAX_ANIMATED_ITEMS) break;
      const item = items[i];
      if (typeof item.animate !== "function") continue;

      const lastRect = item.getBoundingClientRect();
      if (lastRect.top > lastBottomBound) break;
      if (lastRect.bottom < lastTopBound) continue;

      const delay = Math.min(timing.maxDelay, animatedCount * timing.staggerStep);
      const first = firstRects.get(item);

      let anim: Animation | null = null;
      if (first) {
        let dx = first.left - lastRect.left;
        let dy = first.top - lastRect.top;
        if (Math.abs(dx) < 1.5 && Math.abs(dy) < 1.5) {
          continue;
        }
        const maxDx = Math.max(540, lastRect.width * 1.45);
        dx = Math.max(-maxDx, Math.min(maxDx, dx));
        dy = Math.max(-420, Math.min(420, dy));

        anim = item.animate(
          [
            { transform: `translate3d(${dx.toFixed(1)}px, ${dy.toFixed(1)}px, 0)` },
            { transform: "translate3d(0, 0, 0)" },
          ],
          {
            duration: timing.duration,
            delay,
            easing: timing.easing,
            fill: "backwards",
          },
        );
      } else if (nextCols === 2) {
        // 单→双列时由下方新拉入视口底部的曲目：轻柔上浮显形
        anim = item.animate(
          [
            { transform: "translate3d(0, 26px, 0)", opacity: 0.01 },
            { transform: "translate3d(0, 0, 0)", opacity: 1 },
          ],
          {
            duration: Math.round(timing.duration * 0.85),
            delay,
            easing: timing.easing,
            fill: "backwards",
          },
        );
      }

      if (anim) {
        animatedCount += 1;
        state.activeAnims.add(anim);
        const currentAnim = anim;
        const onSettled = () => {
          state.activeAnims.delete(currentAnim);
        };
        currentAnim.onfinish = onSettled;
        currentAnim.oncancel = onSettled;
      }
    }

    if (state.reflowClearTimer !== null) {
      window.clearTimeout(state.reflowClearTimer);
    }
    const clearAfterMs = animatedCount > 0 ? timing.duration + timing.maxDelay + 48 : 0;
    if (clearAfterMs === 0) {
      grid.classList.remove("is-col-reflowing");
      state.reflowClearTimer = null;
    } else {
      state.reflowClearTimer = window.setTimeout(() => {
        state.reflowClearTimer = null;
        grid.classList.remove("is-col-reflowing");
      }, clearAfterMs);
    }
  };

  const flushPendingResizes = () => {
    flushRafId = null;
    if (disposed) return;

    const batch = Array.from(pendingWidths.entries());
    pendingWidths.clear();

    for (const [grid, measuredWidth] of batch) {
      if (!grid.isConnected) {
        cleanupGrid(grid);
        continue;
      }
      const state = states.get(grid);
      if (!state) continue;

      if (!Number.isFinite(measuredWidth) || measuredWidth <= 1) {
        continue;
      }

      const nextCols = resolveAdaptiveSongGridCols(measuredWidth, state.cols);
      const gridRect = grid.getBoundingClientRect();
      const isVisibleNow = grid.offsetParent !== null && gridRect.height > 1;

      if (state.cols === null) {
        // 首次观测到有效宽度：静默对齐，与首帧 @container 保持一致不播 FLIP
        grid.dataset.songCols = String(nextCols);
        state.cols = nextCols;
        state.lastWidth = measuredWidth;
        if (isVisibleNow) {
          state.hasVisibleLayout = true;
        }
        continue;
      }

      if (nextCols !== state.cols) {
        applyColumnChange(grid, state, nextCols, measuredWidth);
      } else {
        state.lastWidth = measuredWidth;
        if (isVisibleNow) {
          state.hasVisibleLayout = true;
        }
      }
    }
  };

  const scheduleFlush = () => {
    if (flushRafId !== null || disposed) return;
    flushRafId = window.requestAnimationFrame(flushPendingResizes);
  };

  const resizeObserver =
    typeof ResizeObserver !== "undefined"
      ? new ResizeObserver((entries) => {
          if (disposed) return;
          for (let i = 0; i < entries.length; i++) {
            const entry = entries[i];
            const target = entry.target;
            if (!(target instanceof HTMLElement)) continue;
            const inlineSize =
              Array.isArray(entry.contentBoxSize) && entry.contentBoxSize.length > 0
                ? entry.contentBoxSize[0].inlineSize
                : entry.contentRect.width;
            if (!Number.isFinite(inlineSize) || inlineSize <= 0) continue;
            const state = states.get(target);
            if (!state) continue;
            if (state.cols !== null && Math.abs(inlineSize - state.lastWidth) < 0.75) {
              continue;
            }
            pendingWidths.set(target, inlineSize);
          }
          if (pendingWidths.size > 0) {
            scheduleFlush();
          }
        })
      : null;

  const observeGrid = (grid: HTMLElement) => {
    if (disposed || states.has(grid)) return;
    const rect = grid.getBoundingClientRect();
    const hasWidth = Number.isFinite(rect.width) && rect.width > 1;
    const isVisible = grid.offsetParent !== null && hasWidth && rect.height > 1;
    const initialCols = hasWidth ? resolveAdaptiveSongGridCols(rect.width, null) : null;

    if (initialCols !== null) {
      grid.dataset.songCols = String(initialCols);
    }

    states.set(grid, {
      cols: initialCols,
      lastWidth: hasWidth ? rect.width : 0,
      hasVisibleLayout: isVisible,
      reflowClearTimer: null,
      activeAnims: new Set<Animation>(),
    });

    resizeObserver?.observe(grid);
  };

  const scanSubtree = (root: ParentNode = document) => {
    if (disposed) return;
    pruneDisconnected();
    if (root instanceof HTMLElement && root.matches(GRID_SELECTOR)) {
      observeGrid(root);
    }
    if ("querySelectorAll" in root) {
      const grids = root.querySelectorAll<HTMLElement>(GRID_SELECTOR);
      for (let i = 0; i < grids.length; i++) {
        observeGrid(grids[i]);
      }
    }
  };

  scanSubtree(document);

  return {
    scanSubtree,
    dispose: () => {
      disposed = true;
      if (flushRafId !== null) {
        window.cancelAnimationFrame(flushRafId);
        flushRafId = null;
      }
      pendingWidths.clear();
      for (const grid of Array.from(states.keys())) {
        cleanupGrid(grid);
      }
      resizeObserver?.disconnect();
    },
  };
}

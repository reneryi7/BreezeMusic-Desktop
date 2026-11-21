import { computed, onBeforeUnmount, onMounted, ref, watch, type CSSProperties, type Ref } from "vue";

export interface ViewportStaggerOptions {
  /** 单行（含行间距）近似高度，默认 64px（对应 .unified-song-row） */
  rowHeight?: number;
  /** 默认列数（2 表示双列歌曲网格，1 表示单列） */
  defaultCols?: number;
  /** 单列折叠断点宽度（window.innerWidth <= breakpoint 退化为 1 列），默认 920 */
  singleColBreakpoint?: number;
  /** 首次挂载前估算视口顶部预留高度（顶栏+页头+底栏），默认 210px */
  estimatedChromeOffset?: number;
}

/**
 * 视口自适应级联进场计算器
 * =========================================================================
 * - 动态跟随默认窗口、手动拖拽缩放、以及全屏最大化窗口的可视区域覆盖全部可见歌曲行（不再局限于前 7 行）；
 * - 完整保留修改前原版 `bm-stagger-rise` 逐项级联质感（0.34s var(--ease-apple-fluid) + 22ms 线性顺序错峰）；
 * - 优先使用原生 CSS `sibling-index()`，仅在不支持时通过 JS 注入 `--bm-stagger-idx` 回退。
 */
const supportsNativeSiblingIndex =
  typeof CSS !== "undefined" &&
  typeof CSS.supports === "function" &&
  CSS.supports("animation-delay: calc(sibling-index() * 0.1s)");

export function useViewportStagger(
  containerRef: Ref<HTMLElement | null>,
  options: ViewportStaggerOptions = {},
) {
  const rowHeight = Math.max(36, options.rowHeight ?? 64);
  const defaultCols = Math.max(1, options.defaultCols ?? 2);
  const singleColBreakpoint = options.singleColBreakpoint ?? 920;
  const estimatedChromeOffset = options.estimatedChromeOffset ?? 210;

  const initialWindowW = typeof window !== "undefined" ? window.innerWidth : 1280;
  const initialWindowH = typeof window !== "undefined" ? window.innerHeight : 820;

  const cols = ref(
    defaultCols > 1 && initialWindowW <= singleColBreakpoint ? 1 : defaultCols,
  );
  const visibleRows = ref(
    Math.max(6, Math.ceil(Math.max(320, initialWindowH - estimatedChromeOffset) / rowHeight) + 1),
  );

  let resizeObserver: ResizeObserver | null = null;
  let rafId: number | null = null;

  function detectGridCols(el: HTMLElement | null): number {
    if (defaultCols <= 1) return 1;
    if (el && typeof window !== "undefined") {
      const style = window.getComputedStyle(el);
      if (style.display.includes("grid") && style.gridTemplateColumns && style.gridTemplateColumns !== "none") {
        const trackCount = style.gridTemplateColumns
          .trim()
          .split(/\s+/)
          .filter((token) => token && token !== "0px").length;
        if (trackCount >= 1 && trackCount <= 8) return trackCount;
      }
    }
    const w = typeof window !== "undefined" ? window.innerWidth : 1280;
    return w <= singleColBreakpoint ? 1 : defaultCols;
  }

  function measure() {
    if (typeof window === "undefined") return;
    const winH = window.innerHeight;
    const el = containerRef.value;
    cols.value = detectGridCols(el);

    let usableH = Math.max(320, winH - estimatedChromeOffset);
    if (el) {
      const rect = el.getBoundingClientRect();
      const scrollHost = el.closest(".skyline-stage-viewport") as HTMLElement | null;
      const hostRect = scrollHost?.getBoundingClientRect();
      const viewTop = hostRect ? hostRect.top : 0;
      const viewBottom = hostRect ? hostRect.bottom : winH - 88;
      // 若列表在初始滚动位（rect.top 在视口内），精确覆盖容器顶部到视口底部；
      // 若已被用户向下滚动，直接覆盖完整滚动视口高度
      const effectiveTop = rect.top > viewTop && rect.top < viewBottom - 120 ? rect.top : viewTop;
      const measured = viewBottom - effectiveTop;
      if (Number.isFinite(measured) && measured > 160) {
        usableH = measured;
      }
    }

    // +1 行缓冲确保底边半露行同样拥有完整进场，不留突兀断层
    visibleRows.value = Math.max(5, Math.min(28, Math.ceil(usableH / rowHeight) + 1));
  }

  function scheduleMeasure() {
    if (typeof window === "undefined") return;
    if (rafId !== null) return;
    rafId = window.requestAnimationFrame(() => {
      rafId = null;
      measure();
    });
  }

  function bindObservers(el: HTMLElement | null) {
    if (resizeObserver) {
      resizeObserver.disconnect();
      resizeObserver = null;
    }
    if (typeof ResizeObserver === "undefined") return;
    resizeObserver = new ResizeObserver(() => scheduleMeasure());
    if (el) {
      resizeObserver.observe(el);
      const scrollHost = el.closest(".skyline-stage-viewport") as HTMLElement | null;
      if (scrollHost) resizeObserver.observe(scrollHost);
    }
  }

  watch(containerRef, (el) => {
    measure();
    bindObservers(el);
  });

  onMounted(() => {
    measure();
    bindObservers(containerRef.value);
    window.addEventListener("resize", scheduleMeasure, { passive: true });
  });

  onBeforeUnmount(() => {
    if (rafId !== null) {
      window.cancelAnimationFrame(rafId);
      rafId = null;
    }
    if (resizeObserver) {
      resizeObserver.disconnect();
      resizeObserver = null;
    }
    if (typeof window !== "undefined") {
      window.removeEventListener("resize", scheduleMeasure);
    }
  });

  const visibleLimit = computed(() => visibleRows.value * cols.value);

  function getStaggerClass(index: number): string {
    return index < visibleLimit.value ? "bm-vp-stagger-item" : "";
  }

  function getStaggerStyle(index: number): CSSProperties | undefined {
    if (index >= visibleLimit.value || supportsNativeSiblingIndex) {
      return undefined;
    }
    return {
      "--bm-stagger-idx": index,
    } as CSSProperties;
  }

  return {
    cols,
    visibleRows,
    visibleLimit,
    getStaggerClass,
    getStaggerStyle,
    refreshMetrics: measure,
  };
}

import { nextTick, onBeforeUnmount, onMounted, ref, watch, type Ref } from "vue";

export interface UseInfiniteScrollOptions {
  sentinelRef: Ref<HTMLElement | null>;
  enabled: Ref<boolean> | (() => boolean);
  loading: Ref<boolean> | (() => boolean);
  hasMore: Ref<boolean> | (() => boolean);
  onLoadMore: () => Promise<void> | void;
  scrollContainerSelector?: string;
  rootMargin?: string;
}

function readFlag(flag: Ref<boolean> | (() => boolean)): boolean {
  return typeof flag === "function" ? Boolean(flag()) : Boolean(flag.value);
}

/**
 * 统一无感滚动分页预加载器
 * - 提前 650px 静默触发下一页拉取，避免触底停顿与手动点击
 * - 兼顾大屏未铺满自动续拉与请求并发锁、异常退避（出错时允许用户手动点击重试而不死循环）
 */
export function useInfiniteScroll(options: UseInfiniteScrollOptions) {
  const {
    sentinelRef,
    enabled,
    loading,
    hasMore,
    onLoadMore,
    scrollContainerSelector = ".skyline-stage-viewport",
    rootMargin = "650px 0px",
  } = options;

  const hasError = ref(false);
  let observer: IntersectionObserver | null = null;
  let inFlight = false;
  let underfillTimer: number | null = null;

  function canTrigger(): boolean {
    return (
      !inFlight &&
      !hasError.value &&
      readFlag(enabled) &&
      readFlag(hasMore) &&
      !readFlag(loading)
    );
  }

  async function triggerLoad(manual = false) {
    if (manual) {
      hasError.value = false;
    }
    if (!canTrigger()) return;
    inFlight = true;
    try {
      await onLoadMore();
    } catch {
      hasError.value = true;
    } finally {
      inFlight = false;
      scheduleUnderfillCheck();
    }
  }

  function isSentinelNearViewport(): boolean {
    const sentinel = sentinelRef.value;
    if (!sentinel || sentinel.offsetParent === null) return false;
    const rootEl = document.querySelector(scrollContainerSelector) as HTMLElement | null;
    const sRect = sentinel.getBoundingClientRect();
    if (rootEl) {
      const rRect = rootEl.getBoundingClientRect();
      return sRect.top <= rRect.bottom + 650 && sRect.bottom >= rRect.top - 100;
    }
    return sRect.top <= window.innerHeight + 650;
  }

  function scheduleUnderfillCheck() {
    if (underfillTimer !== null) {
      window.clearTimeout(underfillTimer);
      underfillTimer = null;
    }
    underfillTimer = window.setTimeout(() => {
      underfillTimer = null;
      if (canTrigger() && isSentinelNearViewport()) {
        void triggerLoad(false);
      }
    }, 120);
  }

  function rebindObserver() {
    if (observer) {
      observer.disconnect();
      observer = null;
    }
    if (typeof IntersectionObserver === "undefined") return;
    const sentinel = sentinelRef.value;
    if (!sentinel) return;
    const rootEl = document.querySelector(scrollContainerSelector) as HTMLElement | null;
    observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting) && canTrigger()) {
          void triggerLoad(false);
        }
      },
      {
        root: rootEl || null,
        rootMargin,
        threshold: 0.01,
      }
    );
    observer.observe(sentinel);
    scheduleUnderfillCheck();
  }

  watch(
    [sentinelRef, () => readFlag(enabled), () => readFlag(hasMore), () => readFlag(loading)],
    ([, isEnabled, more, isLoading]) => {
      if (!isEnabled) {
        hasError.value = false;
      }
      nextTick(() => {
        rebindObserver();
        if (isEnabled && more && !isLoading) {
          scheduleUnderfillCheck();
        }
      });
    }
  );

  onMounted(() => {
    nextTick(() => rebindObserver());
  });

  onBeforeUnmount(() => {
    if (observer) {
      observer.disconnect();
      observer = null;
    }
    if (underfillTimer !== null) {
      window.clearTimeout(underfillTimer);
      underfillTimer = null;
    }
  });

  return {
    hasError,
    retryLoad: () => triggerLoad(true),
    resetError: () => {
      hasError.value = false;
    },
  };
}

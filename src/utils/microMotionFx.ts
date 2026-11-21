/**
 * BreezeMusic 全站微观动效发射器 & 主题圆形光圈切换引擎
 * =========================================================================
 * - 所有特效严格遵守三档动效策略（仅在 effectiveLevel === "best" 触发高表现力特效）
 * - 纯 GPU 合成属性驱动，事件解耦，不侵入业务状态与 DOM 结构
 */

import { getMotionRuntimeProfile } from "./motionRuntime";

export type MotionFxOrigin =
  | MouseEvent
  | PointerEvent
  | HTMLElement
  | { x: number; y: number }
  | null
  | undefined;

export interface HeartBurstEventPayload {
  kind: "heart";
  x: number;
  y: number;
  tone?: "coral" | "primary" | "gold";
  compact?: boolean;
}

export interface FlyToQueueEventPayload {
  kind: "fly-queue";
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  coverUrl?: string;
  targetAnchorEl?: HTMLElement | null;
}

export interface VipCelebrateEventPayload {
  kind: "vip-celebrate";
  x: number;
  y: number;
}

export type MicroMotionFxEvent =
  | HeartBurstEventPayload
  | FlyToQueueEventPayload
  | VipCelebrateEventPayload;

type FxListener = (evt: MicroMotionFxEvent) => void;

const listeners = new Set<FxListener>();
let lastPointerPos = { x: 0, y: 0, ts: 0 };
let pointerTrackingInstalled = false;

export function installGlobalPointerTracker(): void {
  if (pointerTrackingInstalled || typeof window === "undefined") return;
  pointerTrackingInstalled = true;
  window.addEventListener(
    "pointerdown",
    (e) => {
      lastPointerPos = { x: e.clientX, y: e.clientY, ts: performance.now() };
    },
    { passive: true, capture: true }
  );
}

export function subscribeMicroMotionFx(listener: FxListener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function emitFx(evt: MicroMotionFxEvent): void {
  listeners.forEach((fn) => {
    try {
      fn(evt);
    } catch {
      // isolate individual listener errors
    }
  });
}

export function resolveOriginPoint(
  origin?: MotionFxOrigin
): { x: number; y: number } | null {
  if (typeof window === "undefined") return null;

  if (origin && typeof origin === "object") {
    if ("clientX" in origin && "clientY" in origin) {
      if (origin.clientX !== 0 || origin.clientY !== 0) {
        return { x: origin.clientX, y: origin.clientY };
      }
      const target = origin.currentTarget || origin.target;
      if (target instanceof HTMLElement) {
        const r = target.getBoundingClientRect();
        if (r.width > 0 || r.height > 0) {
          return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
        }
      }
    } else if (origin instanceof HTMLElement) {
      const r = origin.getBoundingClientRect();
      if (r.width > 0 || r.height > 0) {
        return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
      }
    } else if ("x" in origin && "y" in origin) {
      return { x: origin.x, y: origin.y };
    }
  }

  // 回退至 600ms 内最近一次真实指针点击位置，便于从 store/菜单间接调用时仍精准锚定点击点
  if (performance.now() - lastPointerPos.ts < 650) {
    return { x: lastPointerPos.x, y: lastPointerPos.y };
  }

  return null;
}

/** 触发红心/点赞粒子爆发（仅 best 档位生效） */
export function triggerHeartBurst(
  origin?: MotionFxOrigin,
  options?: { tone?: "coral" | "primary" | "gold"; compact?: boolean }
): void {
  const profile = getMotionRuntimeProfile();
  if (!profile.allowParticles) return;
  const pt = resolveOriginPoint(origin);
  if (!pt) return;
  emitFx({
    kind: "heart",
    x: pt.x,
    y: pt.y,
    tone: options?.tone ?? "coral",
    compact: Boolean(options?.compact),
  });
}

/** 查找当前可见的队列按钮落袋锚点 */
function findQueueTargetAnchor(): HTMLElement | null {
  if (typeof document === "undefined") return null;
  const candidates = [
    document.querySelector(".np-queue-trigger-btn") as HTMLElement | null,
    document.getElementById("playerbar-queue-anchor"),
    document.querySelector(".pb-queue-pill-btn") as HTMLElement | null,
  ];
  for (const el of candidates) {
    if (!el) continue;
    const r = el.getBoundingClientRect();
    if (r.width > 4 && r.height > 4 && r.bottom > 0 && r.top < window.innerHeight) {
      return el;
    }
  }
  return null;
}

/** 给目标队列按钮施加一次无侵入落袋弹跳脉冲 */
export function pulseQueueCatchAnchor(anchorEl?: HTMLElement | null): void {
  const profile = getMotionRuntimeProfile();
  if (profile.effectiveLevel === "none") return;
  const target = anchorEl || findQueueTargetAnchor();
  if (!target) return;
  target.classList.remove("bm-queue-catch-pulse");
  void target.offsetWidth;
  target.classList.add("bm-queue-catch-pulse");
  window.setTimeout(() => {
    target.classList.remove("bm-queue-catch-pulse");
  }, 480);
}

/** 触发「加入队列 / 下一首播放」抛物线音符微珠飞入队列按钮 */
export function triggerFlyToQueue(
  origin?: MotionFxOrigin,
  coverUrl?: string
): void {
  const profile = getMotionRuntimeProfile();
  const anchorEl = findQueueTargetAnchor();
  if (!profile.allowParticles) {
    if (profile.effectiveLevel === "standard") {
      pulseQueueCatchAnchor(anchorEl);
    }
    return;
  }
  const from = resolveOriginPoint(origin);
  if (!from || !anchorEl) {
    pulseQueueCatchAnchor(anchorEl);
    return;
  }
  const rect = anchorEl.getBoundingClientRect();
  emitFx({
    kind: "fly-queue",
    fromX: from.x,
    fromY: from.y,
    toX: rect.left + rect.width / 2,
    toY: rect.top + rect.height / 2,
    coverUrl,
    targetAnchorEl: anchorEl,
  });
}

/** 触发 VIP 每日签到成功流金星尘庆祝动效 */
export function triggerVipCelebrate(origin?: MotionFxOrigin): void {
  const profile = getMotionRuntimeProfile();
  if (!profile.allowParticles) return;
  const pt =
    resolveOriginPoint(origin) ||
    (typeof window !== "undefined"
      ? { x: window.innerWidth * 0.42, y: window.innerHeight * 0.52 }
      : null);
  if (!pt) return;
  emitFx({
    kind: "vip-celebrate",
    x: pt.x,
    y: pt.y,
  });
}

/** 给任意图标按钮施加单次方向/弹性微动画类 */
export function triggerOneShotClass(
  el: HTMLElement | null | undefined,
  className: string,
  durationMs = 420
): void {
  if (!el) return;
  const profile = getMotionRuntimeProfile();
  if (profile.effectiveLevel === "none") return;
  el.classList.remove(className);
  void el.offsetWidth;
  el.classList.add(className);
  window.setTimeout(() => {
    el.classList.remove(className);
  }, durationMs);
}

let activeThemeTransition = false;

/**
 * 主题/四变体全屏圆形光圈扩散切换（基于 View Transitions API）
 * - best 档位 + 支持 document.startViewTransition：以点击按钮中心为圆心平滑扩散浸染全屏
 * - standard / none / 不支持环境：直接静默执行 mutate()，零闪烁零阻塞
 */
export function runThemeCircularTransition(
  origin: MotionFxOrigin,
  mutate: () => void | Promise<void>
): void {
  const profile = getMotionRuntimeProfile();
  const doc = typeof document !== "undefined" ? (document as Document & {
    startViewTransition?: (cb: () => void | Promise<void>) => {
      ready: Promise<void>;
      finished: Promise<void>;
    };
  }) : null;

  if (
    !doc ||
    !profile.allowViewTransition ||
    typeof doc.startViewTransition !== "function" ||
    activeThemeTransition
  ) {
    void mutate();
    return;
  }

  const pt = resolveOriginPoint(origin) ?? {
    x: window.innerWidth - 96,
    y: 34,
  };
  const maxRadius = Math.hypot(
    Math.max(pt.x, window.innerWidth - pt.x),
    Math.max(pt.y, window.innerHeight - pt.y)
  );

  const root = doc.documentElement;
  root.style.setProperty("--bm-vt-x", `${Math.round(pt.x)}px`);
  root.style.setProperty("--bm-vt-y", `${Math.round(pt.y)}px`);
  root.style.setProperty("--bm-vt-r", `${Math.ceil(maxRadius)}px`);
  root.classList.add("bm-vt-theme-circular");
  activeThemeTransition = true;

  let transition:
    | {
        ready: Promise<void>;
        finished: Promise<void>;
      }
    | undefined;

  try {
    transition = doc.startViewTransition(() => mutate());
  } catch {
    root.classList.remove("bm-vt-theme-circular");
    activeThemeTransition = false;
    void mutate();
    return;
  }

  transition.ready
    .then(() => {
      try {
        root.animate(
          {
            clipPath: [
              `circle(0px at ${pt.x}px ${pt.y}px)`,
              `circle(${Math.ceil(maxRadius)}px at ${pt.x}px ${pt.y}px)`,
            ],
          },
          {
            duration: 460,
            easing: "cubic-bezier(0.22, 1, 0.36, 1)",
            pseudoElement: "::view-transition-new(root)",
          }
        );
      } catch {
        // fallback handled by CSS keyframes
      }
    })
    .catch(() => {
      // ignore skipped transitions
    });

  transition.finished.finally(() => {
    root.classList.remove("bm-vt-theme-circular");
    root.style.removeProperty("--bm-vt-x");
    root.style.removeProperty("--bm-vt-y");
    root.style.removeProperty("--bm-vt-r");
    activeThemeTransition = false;
  });
}

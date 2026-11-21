/**
 * BreezeMusic 全局三档动效引擎 & 界面信息密度运行时
 * =========================================================================
 * - 动效档位 (MotionLevel):
 *   - "best"     最佳效果：宏观 Apple 流体克制 × 微观 CSS linear() 弹簧触感、
 *                FLIP 封面飞行、圆形光圈切主题、粒子反馈、3D 微倾、电动推子、60fps 光学舞台
 *   - "standard" 普通效果：流畅简洁淡入过渡、精简位移与节流渲染（~30fps），关闭粒子与 3D 倾斜
 *   - "none"     无效果：零位移动画、零粒子、零循环动画、WebGL 渲染单帧静态底色后停帧
 * - 同时自动响应系统 prefers-reduced-motion: reduce 无障碍设置。
 * - 界面密度 (UiDensityMode):
 *   - "compact" | "standard" | "spacious"
 *   - 仅精确作用于歌曲列表行、歌单/专辑卡片栅格、设置项行、下载行与侧边导航间距，
 *     不改动 TitleBar / PlayerBar / 播放页沉浸舞台 / 桌面歌词 / 迷你播放器。
 */

import type { MotionLevel, UiDensityMode } from "../types";

export const MOTION_LEVEL_STORAGE_KEY = "bm_motion_level_v1";
export const UI_DENSITY_STORAGE_KEY = "bm_ui_density_v1";
export const APPEARANCE_PREFS_SYNC_CHANNEL = "bm_appearance_prefs_sync_v1";

export function normalizeMotionLevel(raw: unknown): MotionLevel {
  if (raw === "none" || raw === "standard" || raw === "best") return raw;
  return "best";
}

export function normalizeUiDensity(raw: unknown): UiDensityMode {
  if (raw === "compact" || raw === "spacious" || raw === "standard") return raw;
  return "standard";
}

export function loadPersistedMotionLevel(): MotionLevel {
  try {
    return normalizeMotionLevel(localStorage.getItem(MOTION_LEVEL_STORAGE_KEY));
  } catch {
    return "best";
  }
}

export function loadPersistedUiDensity(): UiDensityMode {
  try {
    return normalizeUiDensity(localStorage.getItem(UI_DENSITY_STORAGE_KEY));
  } catch {
    return "standard";
  }
}

let reducedMotionMql: MediaQueryList | null = null;
let cachedReducedMotion = false;
let reducedMotionListenerBound = false;

export function isSystemReducedMotionPreferred(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  if (!reducedMotionMql) {
    reducedMotionMql = window.matchMedia("(prefers-reduced-motion: reduce)");
    cachedReducedMotion = reducedMotionMql.matches;
    if (!reducedMotionListenerBound) {
      reducedMotionListenerBound = true;
      const update = (e: MediaQueryListEvent) => {
        cachedReducedMotion = e.matches;
      };
      if (typeof reducedMotionMql.addEventListener === "function") {
        reducedMotionMql.addEventListener("change", update);
      } else if (typeof (reducedMotionMql as any).addListener === "function") {
        (reducedMotionMql as any).addListener(update);
      }
    }
  }
  return cachedReducedMotion;
}

export function applyMotionLevelToDocument(level: MotionLevel): void {
  if (typeof document === "undefined") return;
  const safe = normalizeMotionLevel(level);
  document.documentElement.dataset.motionLevel = safe;
  try {
    localStorage.setItem(MOTION_LEVEL_STORAGE_KEY, safe);
  } catch {
    // ignore storage quota errors
  }
}

export function applyUiDensityToDocument(density: UiDensityMode): void {
  if (typeof document === "undefined") return;
  const safe = normalizeUiDensity(density);
  document.documentElement.dataset.uiDensity = safe;
  try {
    localStorage.setItem(UI_DENSITY_STORAGE_KEY, safe);
  } catch {
    // ignore storage quota errors
  }
}

export function readCurrentMotionLevel(): MotionLevel {
  if (typeof document === "undefined") return "best";
  return normalizeMotionLevel(document.documentElement.dataset.motionLevel);
}

/** 计算最终生效动效档位（若系统开启减弱动态效果，自动收敛至 none） */
export function resolveEffectiveMotionLevel(level?: MotionLevel): MotionLevel {
  const configured = normalizeMotionLevel(level ?? readCurrentMotionLevel());
  if (configured === "none" || isSystemReducedMotionPreferred()) {
    return "none";
  }
  return configured;
}

export function broadcastMotionAndDensity(
  motionLevel: MotionLevel,
  uiDensity: UiDensityMode
): void {
  if (typeof BroadcastChannel === "undefined") return;
  try {
    const ch = new BroadcastChannel(APPEARANCE_PREFS_SYNC_CHANNEL);
    ch.postMessage({
      type: "APPEARANCE_PREFS_UPDATED",
      motionLevel: normalizeMotionLevel(motionLevel),
      uiDensity: normalizeUiDensity(uiDensity),
    });
    ch.close();
  } catch {
    // ignore
  }
}

export interface MotionRuntimeProfile {
  level: MotionLevel;
  effectiveLevel: MotionLevel;
  reducedMotion: boolean;
  allowSpring: boolean;
  allowFlipArc: boolean;
  allowAmbientCanvas: boolean;
  allowParticles: boolean;
  allow3DTilt: boolean;
  allowViewTransition: boolean;
  allowMotorizedFader: boolean;
  allowAmbientBreathing: boolean;
  canvasTargetFps: number;
  transitionDurationScale: number;
}

const MOTION_PROFILE_CACHE = {
  none: {
    none: Object.freeze<MotionRuntimeProfile>({
      level: "none",
      effectiveLevel: "none",
      reducedMotion: true,
      allowSpring: false,
      allowFlipArc: false,
      allowAmbientCanvas: false,
      allowParticles: false,
      allow3DTilt: false,
      allowViewTransition: false,
      allowMotorizedFader: false,
      allowAmbientBreathing: false,
      canvasTargetFps: 0,
      transitionDurationScale: 0,
    }),
    standard: Object.freeze<MotionRuntimeProfile>({
      level: "standard",
      effectiveLevel: "none",
      reducedMotion: true,
      allowSpring: false,
      allowFlipArc: false,
      allowAmbientCanvas: false,
      allowParticles: false,
      allow3DTilt: false,
      allowViewTransition: false,
      allowMotorizedFader: false,
      allowAmbientBreathing: false,
      canvasTargetFps: 0,
      transitionDurationScale: 0,
    }),
    best: Object.freeze<MotionRuntimeProfile>({
      level: "best",
      effectiveLevel: "none",
      reducedMotion: true,
      allowSpring: false,
      allowFlipArc: false,
      allowAmbientCanvas: false,
      allowParticles: false,
      allow3DTilt: false,
      allowViewTransition: false,
      allowMotorizedFader: false,
      allowAmbientBreathing: false,
      canvasTargetFps: 0,
      transitionDurationScale: 0,
    }),
  },
  standard: Object.freeze<MotionRuntimeProfile>({
    level: "standard",
    effectiveLevel: "standard",
    reducedMotion: false,
    allowSpring: false,
    allowFlipArc: false,
    allowAmbientCanvas: true,
    allowParticles: false,
    allow3DTilt: false,
    allowViewTransition: false,
    allowMotorizedFader: false,
    allowAmbientBreathing: false,
    canvasTargetFps: 30,
    transitionDurationScale: 0.72,
  }),
  best: Object.freeze<MotionRuntimeProfile>({
    level: "best",
    effectiveLevel: "best",
    reducedMotion: false,
    allowSpring: true,
    allowFlipArc: true,
    allowAmbientCanvas: true,
    allowParticles: true,
    allow3DTilt: true,
    allowViewTransition: true,
    allowMotorizedFader: true,
    allowAmbientBreathing: true,
    canvasTargetFps: 120,
    transitionDurationScale: 1,
  }),
};

/** 供 JS 动效模块（FLIP 封面、WebGL 光学背景、粒子、电动推子、3D 微倾等）读取当前档位参数 */
export function getMotionRuntimeProfile(
  level?: MotionLevel
): MotionRuntimeProfile {
  const configured = normalizeMotionLevel(level ?? readCurrentMotionLevel());
  const reduced = isSystemReducedMotionPreferred();
  if (configured === "none" || reduced) {
    return MOTION_PROFILE_CACHE.none[configured];
  }
  if (configured === "standard") {
    return MOTION_PROFILE_CACHE.standard;
  }
  return MOTION_PROFILE_CACHE.best;
}

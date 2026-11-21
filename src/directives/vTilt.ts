/**
 * v-tilt — 克制型封面卡片 3D 视差微倾指令
 * =========================================================================
 * - 仅在 effectiveLevel === "best" 且指针为 mouse 时启用
 * - 默认最大倾角 ±3.5°（优雅立体、绝不眩晕）
 * - 自动识别 L2 晨雾燕麦（nordic_oat）手抄暖纸主题：自动收敛倾角至 ±2.2° 并禁用镜面高光，保纸质温润质感
 * - 通过 CSS 变量 --bm-tilt-rx / --bm-tilt-ry 写入，由 motion-craft.css 统一驱动合成层 transform
 */

import type { Directive } from "vue";
import { getMotionRuntimeProfile } from "../utils/motionRuntime";

export interface TiltBindingOptions {
  maxDeg?: number;
  disabled?: boolean;
}

interface TiltState {
  rafId: number | null;
  maxDeg: number;
  disabled: boolean;
  onEnter: (e: PointerEvent) => void;
  onMove: (e: PointerEvent) => void;
  onLeave: () => void;
}

const stateMap = new WeakMap<HTMLElement, TiltState>();

function isNordicOatPaperTheme(): boolean {
  if (typeof document === "undefined") return false;
  const root = document.documentElement;
  return (
    root.dataset.theme === "watercolor_light" &&
    root.dataset.lightVariant === "nordic_oat"
  );
}

/**
 * 离场复位：仅复位 3D 倾角并移除 `.bm-tilting` 让光晕在原出口坐标自然淡出。
 * 刻意保留 `--bm-tilt-gx` / `--bm-tilt-gy`，避免淡出期间径向渐变回弹至 `50% 50%` 引发中心闪屏。
 */
function resetTilt(el: HTMLElement, clearGlowPosition = false) {
  el.classList.remove("bm-tilting");
  el.style.removeProperty("--bm-tilt-rx");
  el.style.removeProperty("--bm-tilt-ry");
  if (clearGlowPosition) {
    el.style.removeProperty("--bm-tilt-gx");
    el.style.removeProperty("--bm-tilt-gy");
  }
}

function writeGlowFromPointer(el: HTMLElement, clientX: number, clientY: number) {
  const rect = el.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return null;
  const nx = Math.max(-0.5, Math.min(0.5, (clientX - rect.left) / rect.width - 0.5));
  const ny = Math.max(-0.5, Math.min(0.5, (clientY - rect.top) / rect.height - 0.5));
  const gx = ((nx + 0.5) * 100).toFixed(1);
  const gy = ((ny + 0.5) * 100).toFixed(1);
  el.style.setProperty("--bm-tilt-gx", `${gx}%`);
  el.style.setProperty("--bm-tilt-gy", `${gy}%`);
  return { nx, ny };
}

function parseOptions(value: unknown): { maxDeg: number; disabled: boolean } {
  if (typeof value === "number" && Number.isFinite(value)) {
    return { maxDeg: Math.max(1, Math.min(6, value)), disabled: false };
  }
  if (typeof value === "boolean") {
    return { maxDeg: 3.5, disabled: !value };
  }
  if (value && typeof value === "object") {
    const obj = value as TiltBindingOptions;
    return {
      maxDeg:
        typeof obj.maxDeg === "number" && Number.isFinite(obj.maxDeg)
          ? Math.max(1, Math.min(6, obj.maxDeg))
          : 3.5,
      disabled: Boolean(obj.disabled),
    };
  }
  return { maxDeg: 3.5, disabled: false };
}

export const vTilt: Directive<HTMLElement, TiltBindingOptions | number | boolean | undefined> = {
  mounted(el, binding) {
    const opts = parseOptions(binding.value);
    el.classList.add("bm-tilt-surface");

    const state: TiltState = {
      rafId: null,
      maxDeg: opts.maxDeg,
      disabled: opts.disabled,
      onEnter(e: PointerEvent) {
        if (state.disabled || e.pointerType !== "mouse") return;
        const profile = getMotionRuntimeProfile();
        if (!profile.allow3DTilt) return;
        writeGlowFromPointer(el, e.clientX, e.clientY);
      },
      onMove(e: PointerEvent) {
        if (state.disabled || e.pointerType !== "mouse") return;
        const profile = getMotionRuntimeProfile();
        if (!profile.allow3DTilt) {
          if (el.classList.contains("bm-tilting")) resetTilt(el);
          return;
        }
        const clientX = e.clientX;
        const clientY = e.clientY;
        if (state.rafId !== null) return;
        state.rafId = window.requestAnimationFrame(() => {
          state.rafId = null;
          const pos = writeGlowFromPointer(el, clientX, clientY);
          if (!pos) return;
          const maxAngle = isNordicOatPaperTheme()
            ? Math.min(state.maxDeg, 2.2)
            : state.maxDeg;
          const rx = (-pos.ny * 2 * maxAngle).toFixed(2);
          const ry = (pos.nx * 2 * maxAngle).toFixed(2);
          el.classList.add("bm-tilting");
          el.style.setProperty("--bm-tilt-rx", `${rx}deg`);
          el.style.setProperty("--bm-tilt-ry", `${ry}deg`);
        });
      },
      onLeave() {
        if (state.rafId !== null) {
          window.cancelAnimationFrame(state.rafId);
          state.rafId = null;
        }
        resetTilt(el);
      },
    };

    stateMap.set(el, state);
    el.addEventListener("pointerenter", state.onEnter, { passive: true });
    el.addEventListener("pointermove", state.onMove, { passive: true });
    el.addEventListener("pointerleave", state.onLeave, { passive: true });
    el.addEventListener("pointercancel", state.onLeave, { passive: true });
  },

  updated(el, binding) {
    const state = stateMap.get(el);
    if (!state) return;
    const opts = parseOptions(binding.value);
    state.maxDeg = opts.maxDeg;
    state.disabled = opts.disabled;
    if (state.disabled) resetTilt(el);
  },

  unmounted(el) {
    const state = stateMap.get(el);
    if (!state) return;
    if (state.rafId !== null) {
      window.cancelAnimationFrame(state.rafId);
      state.rafId = null;
    }
    el.removeEventListener("pointerenter", state.onEnter);
    el.removeEventListener("pointermove", state.onMove);
    el.removeEventListener("pointerleave", state.onLeave);
    el.removeEventListener("pointercancel", state.onLeave);
    resetTilt(el, true);
    stateMap.delete(el);
  },
};

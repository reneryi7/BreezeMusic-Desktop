/**
 * 全局主窗口前后台电源与可见性协调器：
 * 联合 WebView2 Page Visibility API (`document.hidden`) 与 Tauri 原生窗口事件
 * (`bm-main-window-visibility`: 托盘隐藏 / 任务栏最小化 / 切换 Mini 小窗 / 恢复显示)，
 * 确保无边框或原生 SW_HIDE 隐藏主窗口后 0ms 彻底暂停所有前端视觉 rAF、WebGL2、Canvas、频谱采样与后台轮询，
 * 同时 100% 保持音频播放不断流。
 */

type PowerVisibilityListener = (active: boolean) => void;

let nativeMainWindowVisible = true;
let initialized = false;
let lastBroadcastActive = true;
const listeners = new Set<PowerVisibilityListener>();

export function isAppForegroundActive(): boolean {
  if (typeof document !== "undefined" && document.hidden) {
    return false;
  }
  return nativeMainWindowVisible;
}

function notifyPowerListenersIfChanged() {
  const next = isAppForegroundActive();
  if (next === lastBroadcastActive) return;
  lastBroadcastActive = next;
  if (typeof document !== "undefined") {
    document.documentElement.classList.toggle("bm-window-hidden", !next);
  }
  for (const cb of listeners) {
    try {
      cb(next);
    } catch {
      // ignore individual subscriber failure
    }
  }
}

export function setNativeMainWindowVisible(visible: boolean) {
  nativeMainWindowVisible = Boolean(visible);
  notifyPowerListenersIfChanged();
}

export function ensureAppPowerVisibilityTracking() {
  if (initialized || typeof document === "undefined") return;
  initialized = true;
  lastBroadcastActive = isAppForegroundActive();
  document.documentElement.classList.toggle("bm-window-hidden", !lastBroadcastActive);
  document.addEventListener("visibilitychange", notifyPowerListenersIfChanged);
}

export function subscribeAppPowerVisibility(
  listener: PowerVisibilityListener
): () => void {
  ensureAppPowerVisibilityTracking();
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

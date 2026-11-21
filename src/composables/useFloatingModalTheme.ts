import { computed } from "vue";
import { useAppStore } from "../stores/appStore";
import {
  buildBorderlessCssVars,
  hexWithAlpha,
  lerpHex,
  pickReadableOnAccent,
  resolveSemanticColorRoles,
} from "../utils/coverPalette";

export type FloatingModalSceneMode = "auto" | "now-playing" | "main-bar";

/**
 * 全局全浮窗四主题 × 智能取色统一表面样式引擎：
 * - 播放页且开启封面取色（或深色剧场）时：读取封面 OKLab 双主色与深邃分级底色；
 * - 关闭封面取色或在主界面任一页面唤起时：完整适配 D1 / D2 / L1 / L2 四主题表面、字体对比度与 `color-scheme`；
 * - 严格遵守 100% 无框设计（Zero-Border Architecture）与 `color-mix(in oklab, ...)`。
 */
export function useFloatingModalTheme(scene: FloatingModalSceneMode = "auto") {
  const store = useAppStore();

  const resolvedScene = computed<"now-playing" | "main-bar">(() => {
    if (scene === "now-playing") return "now-playing";
    if (scene === "main-bar") return "main-bar";
    return store.isNowPlayingOpen ? "now-playing" : "main-bar";
  });

  const isLightSurface = computed<boolean>(() => {
    if (resolvedScene.value === "main-bar") {
      return store.resolvedColorScheme === "watercolor_light";
    }
    return store.isNowPlayingLightStage;
  });

  const palette = computed(() => store.effectiveCoverPalette);
  const semanticRoles = computed(() =>
    resolveSemanticColorRoles(palette.value, !isLightSurface.value)
  );
  const borderlessVars = computed(() =>
    buildBorderlessCssVars(palette.value, !isLightSurface.value)
  );

  const isDynamicStage = computed(
    () =>
      resolvedScene.value === "now-playing" &&
      Boolean(store.dynamicCoverAccentEnabled)
  );

  const accentPrimary = computed<string>(() => {
    if (isDynamicStage.value) {
      return palette.value.dominantAccent || "#5E6AD2";
    }
    const isLight = store.resolvedColorScheme === "watercolor_light";
    if (isLight) {
      return store.lightThemeVariant === "nordic_oat" ? "#0F766E" : "#4F46E5";
    }
    return store.darkThemeVariant === "velvet_nebula" ? "#F43F5E" : "#5E6AD2";
  });

  const accentSecondary = computed<string>(() => {
    if (isDynamicStage.value) {
      return palette.value.secondaryAccent || "#38BDF8";
    }
    const isLight = store.resolvedColorScheme === "watercolor_light";
    if (isLight) {
      return store.lightThemeVariant === "nordic_oat" ? "#B45309" : "#0284C7";
    }
    return store.darkThemeVariant === "velvet_nebula" ? "#F59E0B" : "#38BDF8";
  });

  const onAccentPrimary = computed(() => pickReadableOnAccent(accentPrimary.value));
  const onAccentSecondary = computed(() => pickReadableOnAccent(accentSecondary.value));
  const vipGold = computed(() =>
    isLightSurface.value ? "#B45309" : "#FACC15"
  );

  const modalSurfaceStyle = computed<Record<string, string>>(() => {
    const c1 = accentPrimary.value;
    const c2 = accentSecondary.value;
    const on1 = onAccentPrimary.value;
    const on2 = onAccentSecondary.value;

    if (isDynamicStage.value) {
      const pal = palette.value;
      const sr = semanticRoles.value;
      if (isLightSurface.value) {
        // 封面取色 · 浅色漫染舞台浮窗：随封面色温流转的柔和染宣毛玻璃 + 深墨字阶 + WCAG 加深主辅色
        const bg1 = pal.bgPrimary || "#F7F5F2";
        const bg2 = pal.bgSecondary || "#F1EFEA";
        const cardBase = pal.cardSurfacePrimary || "#FAF9F7";
        const cardHigh = pal.cardSurfaceElevated || "#FFFFFF";
        const ink1 = pal.inkPrimary || "#1D1B1A";
        const ink2 = pal.inkSecondary || "rgba(29, 27, 26, 0.72)";
        const ink3 = pal.inkMuted || "rgba(29, 27, 26, 0.50)";
        return {
          ...borderlessVars.value,
          colorScheme: "light",
          background: `
            radial-gradient(ellipse 84% 52% at 18% 0%, ${hexWithAlpha(c1, 0.16)} 0%, transparent 64%),
            radial-gradient(ellipse 74% 48% at 88% 100%, ${hexWithAlpha(c2, 0.13)} 0%, transparent 64%),
            linear-gradient(168deg, ${hexWithAlpha(bg1, 0.92)} 0%, ${hexWithAlpha(bg2, 0.94)} 100%)
          `,
          backdropFilter: "blur(32px) saturate(1.36)",
          WebkitBackdropFilter: "blur(32px) saturate(1.36)",
          boxShadow: `0 24px 56px ${hexWithAlpha(c1, 0.14)}, 0 10px 26px rgba(24, 24, 27, 0.10)`,
          "--fm-bg": bg1,
          "--fm-card": hexWithAlpha(cardBase, 0.78),
          "--fm-card-elevated": hexWithAlpha(cardHigh, 0.92),
          "--fm-card-hover": hexWithAlpha(c1, 0.085),
          "--fm-card-active": `linear-gradient(135deg, ${hexWithAlpha(c1, 0.17)} 0%, ${hexWithAlpha(c2, 0.12)} 100%)`,
          "--fm-input-bg": hexWithAlpha(ink1, 0.065),
          "--fm-chip-bg": hexWithAlpha(ink1, 0.06),
          "--fm-chip-hover": hexWithAlpha(c1, 0.12),
          "--fm-text-1": ink1,
          "--fm-text-2": ink2,
          "--fm-text-3": ink3,
          "--fm-primary": c1,
          "--fm-secondary": c2,
          "--fm-on-primary": on1,
          "--fm-on-secondary": on2,
          "--fm-primary-soft": hexWithAlpha(c1, 0.14),
          "--fm-primary-glow": hexWithAlpha(c1, 0.26),
          "--fm-secondary-soft": hexWithAlpha(c2, 0.13),
          "--fm-divider": hexWithAlpha(ink1, 0.085),
          "--fm-scroll-thumb": hexWithAlpha(ink1, 0.22),
          "--fm-shadow": `0 14px 32px ${hexWithAlpha(c1, 0.12)}`,
        };
      }
      const bg1 = pal.bgPrimary || sr.surfaceContainerLow || "#141824";
      const bg2 = pal.bgSecondary || sr.surfaceContainerLowest || "#0E121C";
      const bg3 = pal.bgTertiary || sr.surfaceDim || "#080B12";
      const cardHigh = pal.cardSurfaceElevated || pal.cardSurfacePrimary || "#1D2436";
      return {
        ...borderlessVars.value,
        colorScheme: "dark",
        background: `
          radial-gradient(ellipse 84% 52% at 20% 0%, ${hexWithAlpha(c1, 0.14)} 0%, transparent 64%),
          radial-gradient(ellipse 70% 48% at 88% 100%, ${hexWithAlpha(c2, 0.11)} 0%, transparent 64%),
          linear-gradient(168deg, ${hexWithAlpha(bg1, 0.94)} 0%, ${hexWithAlpha(bg2, 0.96)} 56%, ${hexWithAlpha(bg3, 0.98)} 100%)
        `,
        backdropFilter: "blur(34px) saturate(1.42)",
        WebkitBackdropFilter: "blur(34px) saturate(1.42)",
        boxShadow:
          "0 28px 68px -12px rgba(0, 0, 0, 0.68), 0 12px 28px -8px rgba(0, 0, 0, 0.44)",
        "--fm-bg": bg1,
        "--fm-card": hexWithAlpha(cardHigh, 0.56),
        "--fm-card-elevated": hexWithAlpha(lerpHex(cardHigh, c1, 0.12), 0.72),
        "--fm-card-hover": "rgba(255, 255, 255, 0.085)",
        "--fm-card-active": `linear-gradient(135deg, ${hexWithAlpha(c1, 0.24)} 0%, ${hexWithAlpha(c2, 0.12)} 100%)`,
        "--fm-input-bg": "rgba(255, 255, 255, 0.07)",
        "--fm-chip-bg": "rgba(255, 255, 255, 0.085)",
        "--fm-chip-hover": "rgba(255, 255, 255, 0.15)",
        "--fm-text-1": "#F5F7FA",
        "--fm-text-2": "rgba(232, 237, 245, 0.76)",
        "--fm-text-3": "rgba(210, 218, 232, 0.54)",
        "--fm-primary": c1,
        "--fm-secondary": c2,
        "--fm-on-primary": on1,
        "--fm-on-secondary": on2,
        "--fm-primary-soft": hexWithAlpha(c1, 0.18),
        "--fm-primary-glow": hexWithAlpha(c1, 0.3),
        "--fm-secondary-soft": hexWithAlpha(c2, 0.16),
        "--fm-divider": "rgba(255, 255, 255, 0.085)",
        "--fm-scroll-thumb": "rgba(255, 255, 255, 0.18)",
        "--fm-shadow": "0 20px 44px rgba(0, 0, 0, 0.52)",
      };
    }

    // 非取色模式 / 主界面场景：D1 / D2 / L1 / L2 四主题严格特化
    const isLight = store.resolvedColorScheme === "watercolor_light";
    const isOat = isLight && store.lightThemeVariant === "nordic_oat";
    const isVelvet = !isLight && store.darkThemeVariant === "velvet_nebula";

    if (isOat) {
      return {
        ...borderlessVars.value,
        colorScheme: "light",
        background: "linear-gradient(168deg, #FAF6EE 0%, #F2ECE1 60%, #EDE4D4 100%)",
        backdropFilter: "none",
        WebkitBackdropFilter: "none",
        boxShadow: "0 24px 56px rgba(42, 33, 22, 0.18), 0 8px 20px rgba(42, 33, 22, 0.08)",
        "--fm-bg": "#F2ECE1",
        "--fm-card": "#E8E0D0",
        "--fm-card-elevated": "#FAF6EE",
        "--fm-card-hover": "#E2D9C7",
        "--fm-card-active": "linear-gradient(135deg, rgba(15, 118, 110, 0.16) 0%, rgba(180, 83, 9, 0.10) 100%)",
        "--fm-input-bg": "#E8E0D0",
        "--fm-chip-bg": "#EAE2D3",
        "--fm-chip-hover": "#DFD5C3",
        "--fm-text-1": "#231D16",
        "--fm-text-2": "#574B3E",
        "--fm-text-3": "#7D7061",
        "--fm-primary": c1,
        "--fm-secondary": c2,
        "--fm-on-primary": on1,
        "--fm-on-secondary": on2,
        "--fm-primary-soft": "rgba(15, 118, 110, 0.13)",
        "--fm-primary-glow": "rgba(15, 118, 110, 0.24)",
        "--fm-secondary-soft": "rgba(180, 83, 9, 0.13)",
        "--fm-divider": "rgba(35, 29, 22, 0.08)",
        "--fm-scroll-thumb": "rgba(87, 75, 62, 0.28)",
        "--fm-shadow": "0 14px 32px rgba(42, 33, 22, 0.12)",
      };
    }

    if (isLight) {
      return {
        ...borderlessVars.value,
        colorScheme: "light",
        background: `
          radial-gradient(ellipse 84% 52% at 18% 0%, rgba(125, 211, 252, 0.22) 0%, transparent 64%),
          radial-gradient(ellipse 74% 48% at 88% 100%, rgba(244, 114, 182, 0.16) 0%, transparent 64%),
          rgba(248, 250, 255, 0.90)
        `,
        backdropFilter: "blur(32px) saturate(1.38)",
        WebkitBackdropFilter: "blur(32px) saturate(1.38)",
        boxShadow: "0 24px 56px rgba(79, 70, 229, 0.14), 0 10px 26px rgba(24, 24, 27, 0.09)",
        "--fm-bg": "#F6F8FC",
        "--fm-card": "rgba(255, 255, 255, 0.72)",
        "--fm-card-elevated": "rgba(255, 255, 255, 0.92)",
        "--fm-card-hover": "rgba(79, 70, 229, 0.075)",
        "--fm-card-active": "linear-gradient(135deg, rgba(79, 70, 229, 0.15) 0%, rgba(2, 132, 199, 0.11) 100%)",
        "--fm-input-bg": "rgba(226, 232, 240, 0.55)",
        "--fm-chip-bg": "rgba(79, 70, 229, 0.085)",
        "--fm-chip-hover": "rgba(79, 70, 229, 0.15)",
        "--fm-text-1": "#18181B",
        "--fm-text-2": "#475569",
        "--fm-text-3": "#64748B",
        "--fm-primary": c1,
        "--fm-secondary": c2,
        "--fm-on-primary": on1,
        "--fm-on-secondary": on2,
        "--fm-primary-soft": "rgba(79, 70, 229, 0.12)",
        "--fm-primary-glow": "rgba(79, 70, 229, 0.22)",
        "--fm-secondary-soft": "rgba(2, 132, 199, 0.12)",
        "--fm-divider": "rgba(24, 24, 27, 0.075)",
        "--fm-scroll-thumb": "rgba(71, 85, 105, 0.24)",
        "--fm-shadow": "0 14px 34px rgba(79, 70, 229, 0.10)",
      };
    }

    if (isVelvet) {
      return {
        ...borderlessVars.value,
        colorScheme: "dark",
        background: `
          radial-gradient(ellipse 84% 52% at 18% 0%, rgba(244, 63, 94, 0.14) 0%, transparent 64%),
          radial-gradient(ellipse 74% 48% at 88% 100%, rgba(245, 158, 11, 0.11) 0%, transparent 64%),
          linear-gradient(168deg, rgba(36, 22, 31, 0.96) 0%, rgba(24, 15, 20, 0.98) 100%)
        `,
        backdropFilter: "none",
        WebkitBackdropFilter: "none",
        boxShadow: "0 28px 64px rgba(10, 4, 8, 0.68)",
        "--fm-bg": "#1A1116",
        "--fm-card": "rgba(48, 28, 39, 0.72)",
        "--fm-card-elevated": "#35202C",
        "--fm-card-hover": "rgba(244, 63, 94, 0.11)",
        "--fm-card-active": "linear-gradient(135deg, rgba(244, 63, 94, 0.22) 0%, rgba(245, 158, 11, 0.12) 100%)",
        "--fm-input-bg": "rgba(249, 239, 230, 0.075)",
        "--fm-chip-bg": "rgba(249, 239, 230, 0.08)",
        "--fm-chip-hover": "rgba(244, 63, 94, 0.18)",
        "--fm-text-1": "#F9EFE6",
        "--fm-text-2": "rgba(232, 215, 206, 0.78)",
        "--fm-text-3": "rgba(185, 159, 168, 0.66)",
        "--fm-primary": c1,
        "--fm-secondary": c2,
        "--fm-on-primary": on1,
        "--fm-on-secondary": on2,
        "--fm-primary-soft": "rgba(244, 63, 94, 0.18)",
        "--fm-primary-glow": "rgba(244, 63, 94, 0.30)",
        "--fm-secondary-soft": "rgba(245, 158, 11, 0.16)",
        "--fm-divider": "rgba(249, 239, 230, 0.085)",
        "--fm-scroll-thumb": "rgba(249, 239, 230, 0.18)",
        "--fm-shadow": "0 18px 42px rgba(8, 4, 6, 0.54)",
      };
    }

    // D1 obsidian_aurora
    return {
      ...borderlessVars.value,
      colorScheme: "dark",
      background: `
        radial-gradient(ellipse 84% 52% at 18% 0%, rgba(94, 106, 210, 0.14) 0%, transparent 64%),
        radial-gradient(ellipse 74% 48% at 88% 100%, rgba(56, 189, 248, 0.11) 0%, transparent 64%),
        linear-gradient(168deg, rgba(20, 24, 35, 0.94) 0%, rgba(12, 14, 20, 0.97) 100%)
      `,
      backdropFilter: "blur(32px) saturate(1.38)",
      WebkitBackdropFilter: "blur(32px) saturate(1.38)",
      boxShadow: "0 28px 64px rgba(0, 0, 0, 0.64)",
      "--fm-bg": "#0C0E14",
      "--fm-card": "rgba(28, 34, 49, 0.72)",
      "--fm-card-elevated": "#222A3D",
      "--fm-card-hover": "rgba(94, 106, 210, 0.11)",
      "--fm-card-active": "linear-gradient(135deg, rgba(94, 106, 210, 0.24) 0%, rgba(56, 189, 248, 0.12) 100%)",
      "--fm-input-bg": "rgba(255, 255, 255, 0.075)",
      "--fm-chip-bg": "rgba(255, 255, 255, 0.085)",
      "--fm-chip-hover": "rgba(94, 106, 210, 0.18)",
      "--fm-text-1": "#EEF2F6",
      "--fm-text-2": "rgba(203, 213, 225, 0.80)",
      "--fm-text-3": "rgba(148, 163, 184, 0.68)",
      "--fm-primary": c1,
      "--fm-secondary": c2,
      "--fm-on-primary": on1,
      "--fm-on-secondary": on2,
      "--fm-primary-soft": "rgba(94, 106, 210, 0.18)",
      "--fm-primary-glow": "rgba(94, 106, 210, 0.30)",
      "--fm-secondary-soft": "rgba(56, 189, 248, 0.16)",
      "--fm-divider": "rgba(238, 242, 246, 0.085)",
      "--fm-scroll-thumb": "rgba(238, 242, 246, 0.18)",
      "--fm-shadow": "0 18px 42px rgba(0, 0, 0, 0.52)",
    };
  });

  function activeTileStyle(active: boolean, useGold = false): Record<string, string> {
    if (!active) return {};
    const accent = useGold ? vipGold.value : accentPrimary.value;
    const sec = accentSecondary.value;
    const baseOp = isLightSurface.value ? 0.14 : 0.22;
    const secOp = isLightSurface.value ? 0.08 : 0.12;
    return {
      background: `linear-gradient(135deg, ${hexWithAlpha(accent, baseOp)} 0%, ${hexWithAlpha(sec, secOp)} 100%)`,
      boxShadow: `0 10px 26px ${hexWithAlpha(accent, isLightSurface.value ? 0.14 : 0.24)}`,
    };
  }

  function rangeProgressStyle(val: number, min: number, max: number): Record<string, string> {
    const pct =
      max > min ? Math.max(0, Math.min(100, ((val - min) / (max - min)) * 100)) : 0;
    return {
      background: `linear-gradient(90deg, ${accentPrimary.value} 0%, ${accentSecondary.value} ${pct.toFixed(1)}%, var(--fm-chip-bg) ${pct.toFixed(1)}%, var(--fm-chip-bg) 100%)`,
    };
  }

  return {
    resolvedScene,
    isLightSurface,
    isDynamicStage,
    palette,
    semanticRoles,
    accentPrimary,
    accentSecondary,
    onAccentPrimary,
    onAccentSecondary,
    vipGold,
    modalSurfaceStyle,
    activeTileStyle,
    rangeProgressStyle,
  };
}

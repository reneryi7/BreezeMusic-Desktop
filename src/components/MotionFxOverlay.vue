<script setup lang="ts">
/**
 * MotionFxOverlay.vue
 * 全站零交互透明微动效覆盖层（仅承载红心微粒、入队抛物线微珠、VIP 流金星尘）
 * - 纯 GPU transform/opacity Web Animations API 驱动，动画结束立即移除节点，零内存/DOM 残留
 * - pointer-events: none，严禁阻塞任何鼠标或键盘事件
 */
import { onBeforeUnmount, onMounted, ref } from "vue";
import {
  installGlobalPointerTracker,
  pulseQueueCatchAnchor,
  subscribeMicroMotionFx,
  type FlyToQueueEventPayload,
  type HeartBurstEventPayload,
  type MicroMotionFxEvent,
  type VipCelebrateEventPayload,
} from "../utils/microMotionFx";

const overlayRef = ref<HTMLElement | null>(null);
let unsubscribe: (() => void) | null = null;
const activeAnimations = new Set<Animation>();

function trackAndCleanup(el: HTMLElement, anim: Animation) {
  activeAnimations.add(anim);
  const cleanup = () => {
    activeAnimations.delete(anim);
    if (el.parentNode) el.parentNode.removeChild(el);
  };
  anim.onfinish = cleanup;
  anim.oncancel = cleanup;
}

function spawnHeartBurst(evt: HeartBurstEventPayload) {
  const root = overlayRef.value;
  if (!root) return;

  const count = evt.compact ? 6 : 8;
  const maxDist = evt.compact ? 26 : 36;
  const colors =
    evt.tone === "gold"
      ? ["var(--gold, #facc15)", "#fde68a", "var(--coral, #fb7185)"]
      : evt.tone === "primary"
      ? ["var(--primary, #5e6ad2)", "var(--cyan, #38bdf8)", "var(--coral, #fb7185)"]
      : ["var(--coral, #fb7185)", "#ff4d6d", "var(--primary-light, #818cf8)"];

  // 柔和扩散光环（无实线描边，仅柔光渐隐）
  const halo = document.createElement("span");
  halo.className = "bm-fx-heart-halo";
  halo.style.left = `${evt.x}px`;
  halo.style.top = `${evt.y}px`;
  halo.style.setProperty("--fx-color", colors[0]);
  root.appendChild(halo);
  trackAndCleanup(
    halo,
    halo.animate(
      [
        { transform: "translate(-50%, -50%) scale(0.35)", opacity: 0.72 },
        { transform: "translate(-50%, -50%) scale(1.85)", opacity: 0 },
      ],
      { duration: 420, easing: "cubic-bezier(0.22, 1, 0.36, 1)", fill: "forwards" }
    )
  );

  for (let i = 0; i < count; i++) {
    const angle = (Math.PI * 2 * i) / count + (Math.random() - 0.5) * 0.38;
    const dist = maxDist * (0.68 + Math.random() * 0.42);
    const dx = Math.cos(angle) * dist;
    const dy = Math.sin(angle) * dist - (evt.compact ? 4 : 7);
    const isMiniHeart = i % 2 === 0;
    const p = document.createElement("span");
    p.className = isMiniHeart ? "bm-fx-heart-spark" : "bm-fx-dot-spark";
    p.style.left = `${evt.x}px`;
    p.style.top = `${evt.y}px`;
    p.style.setProperty("--fx-color", colors[i % colors.length]);
    if (isMiniHeart) {
      p.textContent = "♥";
      p.style.fontSize = `${evt.compact ? 8 : 10}px`;
    }
    root.appendChild(p);

    const rot = (Math.random() - 0.5) * 52;
    const anim = p.animate(
      [
        {
          transform: "translate(-50%, -50%) translate3d(0, 0, 0) scale(0.3) rotate(0deg)",
          opacity: 0.96,
        },
        {
          transform: `translate(-50%, -50%) translate3d(${(dx * 0.76).toFixed(1)}px, ${(dy * 0.76).toFixed(1)}px, 0) scale(1.08) rotate(${(rot * 0.6).toFixed(1)}deg)`,
          opacity: 0.92,
          offset: 0.56,
        },
        {
          transform: `translate(-50%, -50%) translate3d(${dx.toFixed(1)}px, ${(dy + 8).toFixed(1)}px, 0) scale(0.25) rotate(${rot.toFixed(1)}deg)`,
          opacity: 0,
        },
      ],
      {
        duration: 460 + Math.round(Math.random() * 110),
        easing: "cubic-bezier(0.22, 1, 0.36, 1)",
        fill: "forwards",
      }
    );
    trackAndCleanup(p, anim);
  }
}

function spawnFlyToQueue(evt: FlyToQueueEventPayload) {
  const root = overlayRef.value;
  if (!root) {
    pulseQueueCatchAnchor(evt.targetAnchorEl);
    return;
  }

  const orb = document.createElement("div");
  orb.className = "bm-fx-queue-orb";
  orb.style.left = `${evt.fromX}px`;
  orb.style.top = `${evt.fromY}px`;
  if (evt.coverUrl) {
    const img = document.createElement("img");
    img.src = evt.coverUrl;
    img.alt = "";
    img.className = "bm-fx-queue-orb-img";
    orb.appendChild(img);
  } else {
    orb.textContent = "♪";
  }
  root.appendChild(orb);

  // 采样 12 段二阶贝塞尔抛物线，GPU 合成层平滑无抖动
  const dx = evt.toX - evt.fromX;
  const dy = evt.toY - evt.fromY;
  const dist = Math.hypot(dx, dy);
  const lift = Math.min(130, Math.max(46, dist * 0.24));
  const ctrlX = dx * 0.46;
  const ctrlY = Math.min(0, dy * 0.35) - lift;

  const steps = 12;
  const keyframes: Keyframe[] = [];
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const inv = 1 - t;
    const x = 2 * inv * t * ctrlX + t * t * dx;
    const y = 2 * inv * t * ctrlY + t * t * dy;
    const scale = t < 0.22 ? 0.72 + (t / 0.22) * 0.38 : 1.1 - ((t - 0.22) / 0.78) * 0.66;
    const opacity = t < 0.12 ? t / 0.12 : t > 0.85 ? (1 - t) / 0.15 : 0.96;
    keyframes.push({
      transform: `translate(-50%, -50%) translate3d(${x.toFixed(1)}px, ${y.toFixed(1)}px, 0) scale(${scale.toFixed(3)})`,
      opacity: Number(opacity.toFixed(3)),
      offset: Number(t.toFixed(3)),
    });
  }

  const duration = Math.min(540, Math.max(340, Math.round(280 + dist * 0.2)));
  const anim = orb.animate(keyframes, {
    duration,
    easing: "cubic-bezier(0.26, 0.86, 0.34, 1)",
    fill: "forwards",
  });

  activeAnimations.add(anim);
  const done = () => {
    activeAnimations.delete(anim);
    if (orb.parentNode) orb.parentNode.removeChild(orb);
    pulseQueueCatchAnchor(evt.targetAnchorEl);
  };
  anim.onfinish = done;
  anim.oncancel = () => {
    activeAnimations.delete(anim);
    if (orb.parentNode) orb.parentNode.removeChild(orb);
  };
}

function spawnVipCelebrate(evt: VipCelebrateEventPayload) {
  const root = overlayRef.value;
  if (!root) return;

  const count = 14;
  const palette = ["#facc15", "#fde047", "#f59e0b", "#fbbf24", "#fffbeb"];
  for (let i = 0; i < count; i++) {
    const angle = -Math.PI + (Math.PI * 2 * i) / count + (Math.random() - 0.5) * 0.28;
    const dist = 36 + Math.random() * 52;
    const dx = Math.cos(angle) * dist;
    const dy = Math.sin(angle) * dist - 18;
    const star = document.createElement("span");
    star.className = i % 3 === 0 ? "bm-fx-vip-star" : "bm-fx-dot-spark";
    star.style.left = `${evt.x}px`;
    star.style.top = `${evt.y}px`;
    star.style.setProperty("--fx-color", palette[i % palette.length]);
    if (i % 3 === 0) star.textContent = "✦";
    root.appendChild(star);

    const anim = star.animate(
      [
        {
          transform: "translate(-50%, -50%) translate3d(0, 0, 0) scale(0.35) rotate(0deg)",
          opacity: 1,
        },
        {
          transform: `translate(-50%, -50%) translate3d(${(dx * 0.72).toFixed(1)}px, ${(dy * 0.72).toFixed(1)}px, 0) scale(1.15) rotate(95deg)`,
          opacity: 0.95,
          offset: 0.54,
        },
        {
          transform: `translate(-50%, -50%) translate3d(${dx.toFixed(1)}px, ${(dy + 22).toFixed(1)}px, 0) scale(0.2) rotate(185deg)`,
          opacity: 0,
        },
      ],
      {
        duration: 620 + Math.round(Math.random() * 180),
        easing: "cubic-bezier(0.22, 1, 0.36, 1)",
        fill: "forwards",
      }
    );
    trackAndCleanup(star, anim);
  }
}

function handleFxEvent(evt: MicroMotionFxEvent) {
  if (evt.kind === "heart") {
    spawnHeartBurst(evt);
  } else if (evt.kind === "fly-queue") {
    spawnFlyToQueue(evt);
  } else if (evt.kind === "vip-celebrate") {
    spawnVipCelebrate(evt);
  }
}

onMounted(() => {
  installGlobalPointerTracker();
  unsubscribe = subscribeMicroMotionFx(handleFxEvent);
});

onBeforeUnmount(() => {
  unsubscribe?.();
  unsubscribe = null;
  activeAnimations.forEach((a) => {
    try {
      a.cancel();
    } catch {
      // ignore
    }
  });
  activeAnimations.clear();
});
</script>

<template>
  <Teleport to="body">
    <div
      ref="overlayRef"
      class="bm-motion-fx-overlay"
      aria-hidden="true"
    ></div>
  </Teleport>
</template>

<style>
.bm-motion-fx-overlay {
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 9990;
  overflow: hidden;
  contain: layout style;
}

.bm-fx-heart-halo {
  position: fixed;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: radial-gradient(
    circle,
    color-mix(in srgb, var(--fx-color, #fb7185) 54%, transparent) 0%,
    transparent 72%
  );
  will-change: transform, opacity;
  pointer-events: none;
}

.bm-fx-heart-spark {
  position: fixed;
  line-height: 1;
  color: var(--fx-color, #fb7185);
  text-shadow: 0 2px 8px color-mix(in srgb, var(--fx-color, #fb7185) 48%, transparent);
  will-change: transform, opacity;
  pointer-events: none;
  user-select: none;
}

.bm-fx-vip-star {
  position: fixed;
  font-size: 11px;
  line-height: 1;
  color: var(--fx-color, #facc15);
  text-shadow: 0 0 10px color-mix(in srgb, var(--fx-color, #facc15) 68%, transparent);
  will-change: transform, opacity;
  pointer-events: none;
  user-select: none;
}

.bm-fx-dot-spark {
  position: fixed;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--fx-color, #fb7185);
  box-shadow: 0 0 8px color-mix(in srgb, var(--fx-color, #fb7185) 55%, transparent);
  will-change: transform, opacity;
  pointer-events: none;
}

.bm-fx-queue-orb {
  position: fixed;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--grad-kugou-primary, linear-gradient(135deg, #5e6ad2, #38bdf8));
  color: #ffffff;
  font-size: 12px;
  font-weight: 800;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  box-shadow:
    0 6px 16px color-mix(in srgb, var(--primary, #5e6ad2) 48%, transparent),
    0 0 12px color-mix(in srgb, var(--cyan, #38bdf8) 38%, transparent);
  will-change: transform, opacity;
  pointer-events: none;
}

.bm-fx-queue-orb-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
</style>

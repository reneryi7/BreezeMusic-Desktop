<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  watch,
} from "vue";
import { useAppStore } from "../stores/appStore";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();

const modalCardRef = ref<HTMLElement | null>(null);
const avatarRingRef = ref<HTMLElement | null>(null);
const flyingAvatarRef = ref<HTMLElement | null>(null);

const isRendered = ref(false);
const isMeasuringBeforeEnter = ref(false);
const isAvatarFlightActive = ref(false);

const apmViewport = ref({
  w: typeof window !== "undefined" ? window.innerWidth : 1360,
  h: typeof window !== "undefined" ? window.innerHeight : 820,
});
const liveCardRect = ref<{
  left: number;
  top: number;
  width: number;
  height: number;
} | null>(null);

const flyingAvatar = reactive({
  visible: false,
  src: "",
  destWidth: 72,
  destHeight: 72,
});

let animGeneration = 0;
let pendingRafId: number | null = null;
let pendingFinishRafId: number | null = null;
let activeCardAnim: Animation | null = null;
let activeFlightAnim: Animation | null = null;
let overlayPointerDownSelf = false;

interface RectBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

function prefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

function cubicBezierY(
  t: number,
  p1x: number,
  p1y: number,
  p2x: number,
  p2y: number
): number {
  const sampleX = (u: number) => {
    const inv = 1 - u;
    return 3 * inv * inv * u * p1x + 3 * inv * u * u * p2x + u * u * u;
  };
  const sampleDerivX = (u: number) => {
    const inv = 1 - u;
    return (
      3 * inv * inv * p1x +
      6 * inv * u * (p2x - p1x) +
      3 * u * u * (1 - p2x)
    );
  };
  const sampleY = (u: number) => {
    const inv = 1 - u;
    return 3 * inv * inv * u * p1y + 3 * inv * u * u * p2y + u * u * u;
  };

  let u = t;
  for (let i = 0; i < 6; i++) {
    const err = sampleX(u) - t;
    if (Math.abs(err) < 1e-5) break;
    const d = sampleDerivX(u);
    if (Math.abs(d) < 1e-5) break;
    u -= err / d;
  }
  u = Math.max(0, Math.min(1, u));
  return sampleY(u);
}

function buildAvatarFlightKeyframes(
  origin: RectBox,
  dest: RectBox
): Keyframe[] {
  const cx0 = origin.left + origin.width * 0.5;
  const cy0 = origin.top + origin.height * 0.5;
  const cx1 = dest.left + dest.width * 0.5;
  const cy1 = dest.top + dest.height * 0.5;

  const dx = cx1 - cx0;
  const dy = cy1 - cy0;
  const dist = Math.hypot(dx, dy);

  // Smooth upward-lift quadratic Bezier control point clamped cleanly inside viewport
  const lift = Math.min(68, Math.max(20, dist * 0.14));
  const ctrlX = cx0 + dx * 0.42;
  const minCtrlY = dest.height * 0.4;
  const ctrlY = Math.max(
    minCtrlY,
    cy0 <= cy1 ? cy0 - lift * 0.55 : Math.min(cy0, cy1) - lift
  );

  const steps = 24;
  const frames: Keyframe[] = [];
  const baseW = Math.max(1, dest.width);
  const baseH = Math.max(1, dest.height);

  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const u =
      i === 0 ? 0 : i === steps ? 1 : cubicBezierY(t, 0.22, 1, 0.36, 1);
    const inv = 1 - u;

    const cx = inv * inv * cx0 + 2 * inv * u * ctrlX + u * u * cx1;
    const cy = inv * inv * cy0 + 2 * inv * u * ctrlY + u * u * cy1;
    const curW = origin.width + (dest.width - origin.width) * u;
    const curH = origin.height + (dest.height - origin.height) * u;

    const left = cx - curW * 0.5;
    const top = cy - curH * 0.5;
    const sx = curW / baseW;
    const sy = curH / baseH;

    frames.push({
      offset: t,
      transform: `translate3d(${left.toFixed(2)}px, ${top.toFixed(
        2
      )}px, 0) scale(${sx.toFixed(4)}, ${sy.toFixed(4)})`,
    });
  }

  return frames;
}

function clearScheduledFrames() {
  if (pendingRafId !== null) {
    cancelAnimationFrame(pendingRafId);
    pendingRafId = null;
  }
  if (pendingFinishRafId !== null) {
    cancelAnimationFrame(pendingFinishRafId);
    pendingFinishRafId = null;
  }
}

function cancelActiveAnimations() {
  clearScheduledFrames();
  if (activeCardAnim) {
    activeCardAnim.cancel();
    activeCardAnim = null;
  }
  if (activeFlightAnim) {
    activeFlightAnim.cancel();
    activeFlightAnim = null;
  }
}

function hideFlyingAvatarImmediately() {
  flyingAvatar.visible = false;
  if (flyingAvatarRef.value) {
    flyingAvatarRef.value.classList.remove("is-active");
  }
}

function closeAccountProfile() {
  store.isAccountProfileOpen = false;
}

function onOverlayPointerDown(e: PointerEvent) {
  overlayPointerDownSelf = e.target === e.currentTarget;
}

function onOverlayClick(e: MouseEvent) {
  if (e.target === e.currentTarget && overlayPointerDownSelf) {
    closeAccountProfile();
  }
  overlayPointerDownSelf = false;
}

function measureAvatarRingInnerRect(): RectBox | null {
  const ringEl = avatarRingRef.value;
  if (!ringEl) return null;
  const rr = ringEl.getBoundingClientRect();
  if (rr.width <= 0 || rr.height <= 0) return null;
  const cs = getComputedStyle(ringEl);
  const pl = parseFloat(cs.paddingLeft) || 0;
  const pt = parseFloat(cs.paddingTop) || 0;
  const pr = parseFloat(cs.paddingRight) || 0;
  const pb = parseFloat(cs.paddingBottom) || 0;
  return {
    left: rr.left + pl,
    top: rr.top + pt,
    width: Math.max(1, rr.width - pl - pr),
    height: Math.max(1, rr.height - pt - pb),
  };
}

function runEnterSequence(gen: number) {
  if (gen !== animGeneration || !store.isAccountProfileOpen) return;

  const cardEl = modalCardRef.value;
  if (!cardEl) {
    isMeasuringBeforeEnter.value = false;
    isAvatarFlightActive.value = false;
    hideFlyingAvatarImmediately();
    return;
  }

  const reduced = prefersReducedMotion();
  const origin = store.accountProfileOrigin;
  const destRect = measureAvatarRingInnerRect();

  isMeasuringBeforeEnter.value = false;

  if (reduced) {
    isAvatarFlightActive.value = false;
    hideFlyingAvatarImmediately();
    return;
  }

  // Silky popover enter (adapts origin cleanly for top-right vs Library identity card overlay)
  const isCardAnchor = origin?.anchorMode === "card-overlay";
  if (typeof cardEl.animate === "function") {
    activeCardAnim = cardEl.animate(
      [
        {
          opacity: 0,
          transform: isCardAnchor
            ? "translate3d(-6px, -8px, 0) scale(0.95)"
            : "translate3d(6px, -10px, 0) scale(0.94)",
        },
        {
          opacity: 1,
          transform: "translate3d(0, 0, 0) scale(1)",
        },
      ],
      {
        duration: 280,
        easing: "cubic-bezier(0.22, 1, 0.36, 1)",
        fill: "forwards",
      }
    );
    activeCardAnim.onfinish = () => {
      if (gen !== animGeneration) return;
      activeCardAnim?.cancel();
      activeCardAnim = null;
    };
  }

  // Silky avatar flight from clicked avatar rect into .apm-avatar-ring
  const flyingEl = flyingAvatarRef.value;
  if (
    origin &&
    destRect &&
    flyingEl &&
    typeof flyingEl.animate === "function"
  ) {
    flyingAvatar.src = origin.src || avatarSrc.value;
    flyingAvatar.destWidth = destRect.width;
    flyingAvatar.destHeight = destRect.height;
    flyingEl.style.width = `${destRect.width}px`;
    flyingEl.style.height = `${destRect.height}px`;
    flyingAvatar.visible = true;
    flyingEl.classList.add("is-active");

    const keyframes = buildAvatarFlightKeyframes(origin, destRect);
    activeFlightAnim = flyingEl.animate(keyframes, {
      duration: 340,
      easing: "linear",
      fill: "forwards",
    });

    activeFlightAnim.onfinish = async () => {
      if (gen !== animGeneration) return;
      // Reveal destination avatar underneath while flying element still covers it pixel-for-pixel
      isAvatarFlightActive.value = false;
      await nextTick();
      if (gen !== animGeneration) return;
      pendingFinishRafId = requestAnimationFrame(() => {
        pendingFinishRafId = null;
        if (gen !== animGeneration) return;
        hideFlyingAvatarImmediately();
        if (activeFlightAnim) {
          activeFlightAnim.cancel();
          activeFlightAnim = null;
        }
      });
    };
  } else {
    isAvatarFlightActive.value = false;
    hideFlyingAvatarImmediately();
  }
}

watch(
  () => store.isAccountProfileOpen,
  async (open) => {
    const gen = ++animGeneration;
    cancelActiveAnimations();

    if (open) {
      liveCardRect.value = null;
      if (typeof window !== "undefined") {
        apmViewport.value = { w: window.innerWidth, h: window.innerHeight };
      }
      const origin = store.accountProfileOrigin;
      const shouldFly = Boolean(origin && !prefersReducedMotion());
      isMeasuringBeforeEnter.value = true;
      isAvatarFlightActive.value = shouldFly;
      flyingAvatar.visible = false;
      flyingAvatar.src = origin?.src || avatarSrc.value;
      isRendered.value = true;

      await nextTick();
      if (gen !== animGeneration || !store.isAccountProfileOpen) return;

      pendingRafId = requestAnimationFrame(() => {
        pendingRafId = null;
        runEnterSequence(gen);
      });
    } else {
      isMeasuringBeforeEnter.value = false;
      isAvatarFlightActive.value = false;
      hideFlyingAvatarImmediately();

      if (!isRendered.value) {
        liveCardRect.value = null;
        store.accountProfileOrigin = null;
        return;
      }

      const cardEl = modalCardRef.value;
      if (
        prefersReducedMotion() ||
        !cardEl ||
        typeof cardEl.animate !== "function"
      ) {
        isRendered.value = false;
        liveCardRect.value = null;
        store.accountProfileOrigin = null;
        return;
      }

      const isCardAnchor = store.accountProfileOrigin?.anchorMode === "card-overlay";
      activeCardAnim = cardEl.animate(
        [
          {
            opacity: 1,
            transform: "translate3d(0, 0, 0) scale(1)",
          },
          {
            opacity: 0,
            transform: isCardAnchor
              ? "translate3d(-4px, -6px, 0) scale(0.96)"
              : "translate3d(4px, -8px, 0) scale(0.95)",
          },
        ],
        {
          duration: 200,
          easing: "cubic-bezier(0.4, 0, 0.2, 1)",
          fill: "forwards",
        }
      );

      activeCardAnim.onfinish = () => {
        if (gen !== animGeneration) return;
        isRendered.value = false;
        activeCardAnim = null;
        liveCardRect.value = null;
        store.accountProfileOrigin = null;
      };
    }
  },
  { immediate: true }
);

function handleApmResize() {
  if (typeof window === "undefined") return;
  apmViewport.value = { w: window.innerWidth, h: window.innerHeight };
  if (store.isAccountProfileOpen && store.accountProfileOrigin?.anchorMode === "card-overlay") {
    const el = document.querySelector(".bento-identity-card") as HTMLElement | null;
    if (el) {
      const r = el.getBoundingClientRect();
      if (r.width > 0 && r.height > 0) {
        liveCardRect.value = {
          left: r.left,
          top: r.top,
          width: r.width,
          height: r.height,
        };
      }
    }
  }
}

onMounted(() => {
  window.addEventListener("resize", handleApmResize);
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", handleApmResize);
  animGeneration++;
  cancelActiveAnimations();
});

const currentUid = computed(
  () => store.session?.userId || store.profile?.userId || ""
);

const displayName = computed(
  () => store.profile?.nickname || store.session?.nickname || "微风乐迷"
);

const avatarSrc = computed(
  () => store.profile?.avatarUrl || store.session?.avatarUrl || ""
);

const levelNum = computed(
  () => store.profile?.userGrade || store.profile?.userLevel || 1
);

const nextLevelNum = computed(() => {
  const ng = store.profile?.nextGrade;
  return ng && ng > levelNum.value ? ng : levelNum.value + 1;
});

const totalMinutes = computed(() => {
  const p = store.profile;
  if (!p) return 0;
  if (p.listenDurationSec && p.listenDurationSec > 0) {
    return Math.floor(p.listenDurationSec / 60);
  }
  return p.listenDurationMinutes ?? p.listenDurationMin ?? 0;
});

const listenHoursMain = computed(() => {
  const total = totalMinutes.value;
  const h = Math.floor(total / 60);
  const m = total % 60;
  if (h > 0) return `${h} 小时 ${m} 分钟`;
  return `${m} 分钟`;
});

const listenMinutesDetail = computed(() => {
  if (totalMinutes.value > 0) {
    return `云端累计 ${totalMinutes.value.toLocaleString()} 分钟`;
  }
  return store.profile?.listenDurationFormatted || "云端累计 0 分钟";
});

const vipStatusLabel = computed(() => {
  if (store.vipStatus?.svipActive || store.profile?.svipActive) return "概念 VIP";
  if (
    store.vipStatus?.tvipActive ||
    store.profile?.tvipActive ||
    store.profile?.isVip
  ) {
    return "畅听 VIP";
  }
  return "概念版用户";
});

const isVipActive = computed(() =>
  Boolean(
    store.vipStatus?.svipActive ||
      store.vipStatus?.tvipActive ||
      store.profile?.svipActive ||
      store.profile?.tvipActive ||
      store.profile?.isVip
  )
);

const levelProgressPercent = computed(() => {
  const p = store.profile;
  if (!p) return 0;
  if (typeof p.levelProgressPct === "number" && p.levelProgressPct > 0) {
    return Math.min(100, Math.max(0, Math.round(p.levelProgressPct)));
  }
  const base = p.gradeBaseScore ?? 0;
  const cur = p.userScore ?? 0;
  const next = p.nextLevelScore ?? 0;
  if (next > base && cur >= base) {
    return Math.min(
      100,
      Math.max(4, Math.round(((cur - base) / (next - base)) * 100))
    );
  }
  return 0;
});

const growthDetailText = computed(() => {
  const p = store.profile;
  const cur = p?.userScore || 0;
  const next = p?.nextLevelScore || 0;
  const remScore = p?.nextLevelRemainScore ?? Math.max(0, next - cur);
  const speed = p?.gradeSpeed || 1;
  if (cur > 0 && next > 0) {
    const speedStr = Number.isInteger(speed)
      ? `${speed}.0x`
      : `${speed.toFixed(1)}x`;
    return `成长值 ${cur.toLocaleString()} / ${next.toLocaleString()} · 距 Lv.${nextLevelNum.value} 差 ${remScore.toLocaleString()} 分（${speedStr}）`;
  }
  return `当前等级 Lv.${levelNum.value} · 云端双水位线实时同步`;
});

const regionLabel = computed(() => {
  const p = store.profile;
  if (!p) return "未知地区";
  const region = [p.province, p.city].filter(Boolean).join(" · ");
  if (region) return region;
  if (p.ipLocation) return `IP ${p.ipLocation}`;
  return "音乐星球";
});

const musicAgeLabel = computed(() => {
  const years = store.profile?.musicAgeYears;
  if (years && years > 0) return `乐龄 ${years} 年`;
  return "云端档案已认证";
});

const isCardOverlayMode = computed(
  () =>
    store.accountProfileOrigin?.anchorMode === "card-overlay" &&
    Boolean(liveCardRect.value || store.accountProfileOrigin?.cardRect)
);

const modalPositionStyle = computed<Record<string, string>>(() => {
  const card = liveCardRect.value || store.accountProfileOrigin?.cardRect;
  if (!isCardOverlayMode.value || !card) {
    return {} as Record<string, string>;
  }
  const vw = apmViewport.value.w;
  const vh = apmViewport.value.h;
  const modalW = Math.min(480, vw - 32);
  const estModalH = Math.min(540, Math.max(340, vh - 96));
  const left = Math.max(16, Math.min(vw - modalW - 16, card.left));
  const top = Math.max(64, Math.min(vh - estModalH - 22, card.top));
  return {
    left: `${Math.round(left)}px`,
    top: `${Math.round(top)}px`,
    right: "auto",
    transformOrigin: "top left",
  };
});

const bannerBgStyle = computed(() => {
  const bg = store.profile?.bgPicUrl?.trim();
  if (bg) {
    return {
      backgroundImage: `linear-gradient(180deg, rgba(15, 23, 42, 0.25) 0%, rgba(15, 23, 42, 0.72) 100%), url("${bg}")`,
      backgroundSize: "cover",
      backgroundPosition: "center",
    };
  }
  return {};
});
</script>

<template>
  <Teleport to="body">
    <div
      v-if="isRendered"
      class="apm-popover-overlay"
      :class="[
        `scheme-${store.resolvedColorScheme}`,
        store.resolvedColorScheme === 'watercolor_light'
          ? `light-variant-${store.lightThemeVariant}`
          : `dark-variant-${store.darkThemeVariant}`,
      ]"
      @pointerdown="onOverlayPointerDown"
      @click="onOverlayClick"
    >
      <div
        ref="modalCardRef"
        class="account-profile-modal"
        :class="{
          'is-hidden-before-enter': isMeasuringBeforeEnter,
          'is-card-overlay': isCardOverlayMode,
        }"
        :style="modalPositionStyle"
        role="dialog"
        aria-modal="true"
        aria-label="个人音乐档案"
        @click.stop
      >
        <!-- 顶部个人主页横幅与大头像 -->
        <div class="apm-banner" :style="bannerBgStyle">
          <div class="apm-banner-top-bar">
            <span class="apm-banner-chip">个人音乐档案</span>
            <div class="apm-banner-actions">
              <button
                type="button"
                class="apm-icon-btn"
                :disabled="store.isSyncingListeningGrade"
                :data-tip="store.isSyncingListeningGrade ? '正在同步档案...' : '同步云端档案'"
                aria-label="同步云端档案"
                @click="store.syncListeningGradeCloud(true)"
              >
                <PlumpIcon
                  name="refresh"
                  :size="15"
                  :class="{ 'is-spinning': store.isSyncingListeningGrade }"
                />
              </button>
              <button
                type="button"
                class="apm-icon-btn"
                aria-label="关闭个人档案"
                @click="closeAccountProfile"
              >
                <PlumpIcon name="close" :size="15" />
              </button>
            </div>
          </div>
        </div>

        <div class="apm-body">
          <div class="apm-identity-head">
            <div ref="avatarRingRef" class="apm-avatar-ring">
              <img
                v-if="avatarSrc"
                :src="avatarSrc"
                class="apm-avatar-img"
                :class="{ 'is-flight-hidden': isAvatarFlightActive }"
                draggable="false"
                alt="用户头像"
              />
              <div
                v-else
                class="apm-avatar-img apm-avatar-fallback"
                :class="{ 'is-flight-hidden': isAvatarFlightActive }"
              >
                {{ displayName.slice(0, 1) }}
              </div>
            </div>

            <div class="apm-identity-meta">
              <div class="apm-name-badges">
                <h3 class="apm-nickname">{{ displayName }}</h3>
                <span class="apm-lv-badge">Lv.{{ levelNum }}</span>
                <span class="apm-vip-badge" :class="{ active: isVipActive }">
                  {{ vipStatusLabel }}
                </span>
              </div>
              <div class="apm-sub-meta">
                <span>UID: {{ currentUid || "未获取" }}</span>
                <span class="apm-dot">·</span>
                <span>{{ musicAgeLabel }}</span>
                <span class="apm-dot">·</span>
                <span>{{ regionLabel }}</span>
              </div>
              <div
                v-if="isVipActive && store.profile?.vipExpireText"
                class="apm-vip-expire"
              >
                {{ store.profile.vipExpireText }}
              </div>
            </div>
          </div>

          <!-- 个人签名 -->
          <div class="apm-signature-box">
            <span class="apm-quote-mark">“</span>
            <p class="apm-signature-text">
              {{
                store.profile?.signature?.trim() ||
                "在微风里收藏每一首心动旋律，暂未设置个性签名。"
              }}
            </p>
          </div>

          <!-- 核心三格音乐档案指标 -->
          <div class="apm-stats-grid">
            <div class="apm-stat-card accent-azure">
              <div class="apm-stat-label">云端累计听歌</div>
              <div class="apm-stat-value">{{ listenHoursMain }}</div>
              <div class="apm-stat-sub">{{ listenMinutesDetail }}</div>
            </div>

            <div class="apm-stat-card accent-jade">
              <div class="apm-stat-label">听歌等级</div>
              <div class="apm-stat-value">Lv.{{ levelNum }}</div>
              <div class="apm-stat-sub">目标 Lv.{{ nextLevelNum }}</div>
            </div>

            <div class="apm-stat-card accent-amber">
              <div class="apm-stat-label">音乐社交</div>
              <div class="apm-stat-value">
                {{ store.profile?.fans ?? 0 }}
                <span class="apm-stat-unit">粉丝</span>
              </div>
              <div class="apm-stat-sub">
                关注 {{ store.profile?.follows ?? 0 }} · 访客
                {{ store.profile?.visitors ?? 0 }}
              </div>
            </div>
          </div>

          <!-- 成长值进度条 -->
          <div class="apm-growth-panel">
            <div class="apm-growth-head">
              <span class="apm-growth-title">等级成长进度</span>
              <span class="apm-growth-pct">{{ levelProgressPercent }}%</span>
            </div>
            <div class="apm-growth-track">
              <div
                class="apm-growth-fill"
                :style="{ width: `${levelProgressPercent}%` }"
              ></div>
            </div>
            <div class="apm-growth-desc">{{ growthDetailText }}</div>
          </div>
        </div>
      </div>

      <!-- 丝滑头像上浮飞行替身 (WAAPI translate3d + scale) -->
      <div
        ref="flyingAvatarRef"
        class="apm-flying-avatar"
        :class="{ 'is-active': flyingAvatar.visible }"
        :style="{
          width: `${flyingAvatar.destWidth}px`,
          height: `${flyingAvatar.destHeight}px`,
        }"
        aria-hidden="true"
      >
        <img
          v-if="flyingAvatar.src"
          :src="flyingAvatar.src"
          class="apm-flying-avatar-img"
          draggable="false"
          alt=""
        />
        <div v-else class="apm-flying-avatar-fallback">
          {{ displayName.slice(0, 1) }}
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.apm-popover-overlay {
  position: fixed;
  inset: 0;
  z-index: 1200;
  background: transparent;
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
}

.account-profile-modal {
  position: fixed;
  top: 68px;
  right: 16px;
  width: min(480px, calc(100vw - 32px));
  max-height: calc(100vh - 96px);
  overflow-y: auto;
  overflow-x: hidden;
  padding: 0;
  border-radius: var(--bm-r-xl, 24px);
  background: linear-gradient(
    180deg,
    rgba(24, 24, 30, 0.99) 0%,
    rgba(17, 17, 22, 0.99) 100%
  );
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
  border: none;
  color: #f8fafc;
  box-shadow:
    0 24px 56px rgba(0, 0, 0, 0.56),
    0 8px 22px rgba(0, 0, 0, 0.34);
  transform-origin: top right;
  will-change: transform, opacity;
}

[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .account-profile-modal {
  background: linear-gradient(180deg, rgba(28, 22, 34, 0.99) 0%, rgba(19, 15, 24, 0.99) 100%);
}

.account-profile-modal.is-hidden-before-enter {
  opacity: 0;
  transform: none;
}

[data-theme="watercolor_light"] .account-profile-modal,
.scheme-watercolor_light .account-profile-modal,
.scheme-light .account-profile-modal {
  background: linear-gradient(180deg, #ffffff 0%, #f9f7f2 100%);
  border: none;
  color: #181a20;
  box-shadow:
    0 24px 56px rgba(24, 26, 32, 0.14),
    0 8px 20px rgba(24, 26, 32, 0.07);
}

[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .account-profile-modal {
  background: #fcf9f2;
  color: #221f1a;
  box-shadow: 0 24px 56px -12px rgba(45, 38, 28, 0.16);
}

.apm-banner {
  height: 108px;
  padding: 14px 18px;
  border-radius: var(--bm-r-xl, 24px) var(--bm-r-xl, 24px) 0 0;
  background: linear-gradient(
    135deg,
    rgba(56, 189, 248, 0.28) 0%,
    rgba(45, 212, 191, 0.2) 55%,
    rgba(15, 23, 42, 0.4) 100%
  );
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
}

[data-theme="watercolor_light"] .apm-banner,
.scheme-watercolor_light .apm-banner,
.scheme-light .apm-banner {
  background: linear-gradient(
    135deg,
    rgba(224, 242, 254, 0.96) 0%,
    rgba(204, 251, 241, 0.9) 60%,
    rgba(241, 245, 249, 0.94) 100%
  );
}

.apm-banner-top-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.apm-banner-chip {
  padding: 4px 11px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  letter-spacing: 0.03em;
  background: rgba(15, 23, 42, 0.78);
  color: #f8fafc;
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
  box-shadow: 0 2px 8px rgba(2, 6, 23, 0.2);
}

[data-theme="watercolor_light"] .apm-banner-chip,
.scheme-watercolor_light .apm-banner-chip,
.scheme-light .apm-banner-chip {
  background: rgba(255, 255, 255, 0.94);
  color: var(--text-primary);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .apm-banner-chip,
.scheme-watercolor_light.light-variant-nordic_oat .apm-banner-chip {
  background: #fcf9f2;
  color: #221f1a;
  box-shadow: 0 2px 8px rgba(45, 36, 22, 0.1);
}

.apm-banner-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.apm-icon-btn {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: none;
  background: rgba(15, 23, 42, 0.74);
  color: #f8fafc;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
  box-shadow: 0 2px 8px rgba(2, 6, 23, 0.24);
  transition: transform 0.18s ease, background 0.18s ease, color 0.18s ease;
}

.apm-icon-btn:hover:not(:disabled) {
  background: rgba(14, 165, 233, 0.85);
  color: #ffffff;
  transform: translateY(-1px);
}

.apm-icon-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

[data-theme="watercolor_light"] .apm-icon-btn,
.scheme-watercolor_light .apm-icon-btn,
.scheme-light .apm-icon-btn {
  background: rgba(255, 255, 255, 0.94);
  color: var(--text-primary);
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
}

[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .apm-icon-btn,
.scheme-watercolor_light.light-variant-nordic_oat .apm-icon-btn {
  background: #fcf9f2;
  color: #221f1a;
  box-shadow: 0 2px 10px rgba(45, 36, 22, 0.1);
}

[data-theme="watercolor_light"] .apm-icon-btn:hover:not(:disabled),
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .apm-icon-btn:hover:not(:disabled),
.scheme-watercolor_light .apm-icon-btn:hover:not(:disabled),
.scheme-watercolor_light.light-variant-nordic_oat .apm-icon-btn:hover:not(:disabled),
.scheme-light .apm-icon-btn:hover:not(:disabled) {
  background: #ffffff;
  color: var(--primary);
  box-shadow: 0 4px 14px color-mix(in srgb, var(--primary) 20%, transparent);
}

.apm-body {
  padding: 0 22px 22px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.apm-identity-head {
  display: flex;
  align-items: flex-end;
  gap: 14px;
  margin-top: -34px;
}

.apm-avatar-ring {
  width: 78px;
  height: 78px;
  border-radius: 50%;
  padding: 3px;
  background: var(--grad-kugou-primary);
  box-shadow: 0 10px 24px color-mix(in srgb, var(--primary) 26%, transparent);
  flex-shrink: 0;
}

.apm-avatar-img {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  object-fit: cover;
  display: block;
}

.apm-avatar-img.is-flight-hidden {
  opacity: 0;
}

.apm-avatar-fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-elevated, #1c1d26);
  color: var(--primary);
  font-size: 26px;
  font-weight: 800;
}

[data-theme="watercolor_light"] .apm-avatar-fallback,
.scheme-watercolor_light .apm-avatar-fallback,
.scheme-light .apm-avatar-fallback {
  background: #e0f2fe;
  color: #0284c7;
}

.apm-flying-avatar {
  position: fixed;
  left: 0;
  top: 0;
  margin: 0;
  border-radius: 50%;
  overflow: hidden;
  pointer-events: none;
  z-index: 1300;
  transform-origin: top left;
  will-change: transform, opacity;
  opacity: 0;
  visibility: hidden;
  box-shadow:
    0 14px 32px rgba(2, 132, 199, 0.3),
    0 6px 16px rgba(0, 0, 0, 0.22);
}

.apm-flying-avatar.is-active {
  opacity: 1;
  visibility: visible;
}

.apm-flying-avatar-img,
.apm-flying-avatar-fallback {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  object-fit: cover;
  display: block;
}

.apm-flying-avatar-fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-elevated, #1c1d26);
  color: var(--primary);
  font-size: 26px;
  font-weight: 800;
}

[data-theme="watercolor_light"] .apm-flying-avatar-fallback,
.scheme-watercolor_light .apm-flying-avatar-fallback,
.scheme-light .apm-flying-avatar-fallback {
  background: #e0f2fe;
  color: #0284c7;
}

.apm-identity-meta {
  flex: 1;
  min-width: 0;
  padding-top: 38px;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.apm-name-badges {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.apm-nickname {
  margin: 0;
  font-size: 18.5px;
  font-weight: 800;
  color: #f8fafc;
}

[data-theme="watercolor_light"] .apm-nickname,
.scheme-watercolor_light .apm-nickname,
.scheme-light .apm-nickname {
  color: var(--text-primary);
}

.apm-lv-badge {
  padding: 2px 9px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  color: #2dd4bf;
  background: rgba(45, 212, 191, 0.16);
}

[data-theme="watercolor_light"] .apm-lv-badge,
.scheme-watercolor_light .apm-lv-badge,
.scheme-light .apm-lv-badge {
  color: #0f766e;
  background: rgba(13, 148, 136, 0.12);
}

.apm-vip-badge {
  padding: 2px 9px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  color: rgba(226, 232, 240, 0.85);
  background: rgba(255, 255, 255, 0.09);
}

.apm-vip-badge.active {
  color: #fde68a;
  background: linear-gradient(
    135deg,
    rgba(245, 158, 11, 0.28),
    rgba(217, 119, 6, 0.18)
  );
}

[data-theme="watercolor_light"] .apm-vip-badge,
.scheme-watercolor_light .apm-vip-badge,
.scheme-light .apm-vip-badge {
  color: #5a6072;
  background: rgba(24, 26, 32, 0.06);
}
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .apm-vip-badge,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .apm-signature-box,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .apm-stat-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .apm-growth-panel {
  background: #e4ddd0;
  color: #221f1a;
}

[data-theme="watercolor_light"] .apm-vip-badge.active,
.scheme-watercolor_light .apm-vip-badge.active,
.scheme-light .apm-vip-badge.active {
  color: #b45309;
  background: linear-gradient(
    135deg,
    rgba(251, 191, 36, 0.24),
    rgba(245, 158, 11, 0.14)
  );
}

.apm-sub-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  font-size: var(--bm-fs-caption, 12.5px);
  color: rgba(226, 232, 240, 0.78);
}

[data-theme="watercolor_light"] .apm-sub-meta,
.scheme-watercolor_light .apm-sub-meta,
.scheme-light .apm-sub-meta {
  color: #334155;
}

.apm-dot {
  opacity: 0.55;
}

.apm-vip-expire {
  font-size: var(--bm-fs-caption, 12.5px);
  color: #fbbf24;
  font-weight: 600;
}

[data-theme="watercolor_light"] .apm-vip-expire,
.scheme-watercolor_light .apm-vip-expire,
.scheme-light .apm-vip-expire {
  color: #b45309;
}

.apm-signature-box {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 11px 14px;
  border-radius: var(--bm-r-md, 14px);
  background: rgba(255, 255, 255, 0.05);
}

[data-theme="watercolor_light"] .apm-signature-box,
.scheme-watercolor_light .apm-signature-box,
.scheme-light .apm-signature-box {
  background: #f8fafc;
}

.apm-quote-mark {
  font-size: 18px;
  line-height: 1;
  color: #38bdf8;
  font-weight: 800;
}

[data-theme="watercolor_light"] .apm-quote-mark,
.scheme-watercolor_light .apm-quote-mark,
.scheme-light .apm-quote-mark {
  color: #0284c7;
}

.apm-signature-text {
  margin: 0;
  font-size: var(--bm-fs-caption, 12.5px);
  line-height: 1.5;
  color: rgba(226, 232, 240, 0.84);
}

[data-theme="watercolor_light"] .apm-signature-text,
.scheme-watercolor_light .apm-signature-text,
.scheme-light .apm-signature-text {
  color: #334155;
}

.apm-stats-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.apm-stat-card {
  padding: 12px 13px;
  border-radius: var(--bm-r-lg, 18px);
  background: rgba(255, 255, 255, 0.05);
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.apm-stat-card.accent-azure {
  background: linear-gradient(
    145deg,
    rgba(56, 189, 248, 0.14),
    rgba(15, 23, 42, 0.25)
  );
}

.apm-stat-card.accent-jade {
  background: linear-gradient(
    145deg,
    rgba(45, 212, 191, 0.14),
    rgba(15, 23, 42, 0.25)
  );
}

.apm-stat-card.accent-amber {
  background: linear-gradient(
    145deg,
    rgba(245, 158, 11, 0.14),
    rgba(15, 23, 42, 0.25)
  );
}

[data-theme="watercolor_light"] .apm-stat-card,
.scheme-watercolor_light .apm-stat-card,
.scheme-light .apm-stat-card {
  background: #f8fafc;
}

[data-theme="watercolor_light"] .apm-stat-card.accent-azure,
.scheme-watercolor_light .apm-stat-card.accent-azure,
.scheme-light .apm-stat-card.accent-azure {
  background: linear-gradient(145deg, #f0f9ff, #ffffff);
}

[data-theme="watercolor_light"] .apm-stat-card.accent-jade,
.scheme-watercolor_light .apm-stat-card.accent-jade,
.scheme-light .apm-stat-card.accent-jade {
  background: linear-gradient(145deg, #f0fdfa, #ffffff);
}

[data-theme="watercolor_light"] .apm-stat-card.accent-amber,
.scheme-watercolor_light .apm-stat-card.accent-amber,
.scheme-light .apm-stat-card.accent-amber {
  background: linear-gradient(145deg, #fffbeb, #ffffff);
}

.apm-stat-label {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  color: rgba(148, 163, 184, 0.82);
}

[data-theme="watercolor_light"] .apm-stat-label,
.scheme-watercolor_light .apm-stat-label,
.scheme-light .apm-stat-label {
  color: #64748b;
}

.apm-stat-value {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: 800;
  font-variant-numeric: tabular-nums;
  color: #f8fafc;
  line-height: 1.25;
}

[data-theme="watercolor_light"] .apm-stat-value,
.scheme-watercolor_light .apm-stat-value,
.scheme-light .apm-stat-value {
  color: var(--text-primary);
}

.apm-stat-unit {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  margin-left: 2px;
  color: rgba(226, 232, 240, 0.75);
}

[data-theme="watercolor_light"] .apm-stat-unit,
.scheme-watercolor_light .apm-stat-unit,
.scheme-light .apm-stat-unit {
  color: #334155;
}

.apm-stat-sub {
  font-size: var(--bm-fs-micro, 11px);
  color: rgba(226, 232, 240, 0.72);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

[data-theme="watercolor_light"] .apm-stat-sub,
.scheme-watercolor_light .apm-stat-sub,
.scheme-light .apm-stat-sub {
  color: #475569;
}

.apm-growth-panel {
  padding: 13px 15px;
  border-radius: var(--bm-r-lg, 18px);
  background: rgba(255, 255, 255, 0.05);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

[data-theme="watercolor_light"] .apm-growth-panel,
.scheme-watercolor_light .apm-growth-panel,
.scheme-light .apm-growth-panel {
  background: #f8fafc;
}

.apm-growth-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
}

.apm-growth-title {
  color: rgba(226, 232, 240, 0.85);
}

[data-theme="watercolor_light"] .apm-growth-title,
.scheme-watercolor_light .apm-growth-title,
.scheme-light .apm-growth-title {
  color: #334155;
}

.apm-growth-pct {
  color: #38bdf8;
  font-variant-numeric: tabular-nums;
}

[data-theme="watercolor_light"] .apm-growth-pct,
.scheme-watercolor_light .apm-growth-pct,
.scheme-light .apm-growth-pct {
  color: #0284c7;
}

.apm-growth-track {
  height: 7px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(148, 163, 184, 0.18);
  overflow: hidden;
}

.apm-growth-fill {
  height: 100%;
  border-radius: var(--bm-r-pill, 999px);
  background: linear-gradient(90deg, #38bdf8 0%, #2dd4bf 100%);
  transition: width 0.35s cubic-bezier(0.22, 1, 0.36, 1);
}

.apm-growth-desc {
  font-size: var(--bm-fs-caption, 12.5px);
  color: rgba(148, 163, 184, 0.86);
}

[data-theme="watercolor_light"] .apm-growth-desc,
.scheme-watercolor_light .apm-growth-desc,
.scheme-light .apm-growth-desc {
  color: #475569;
}

@media (prefers-reduced-motion: reduce) {
  .account-profile-modal,
  .apm-icon-btn,
  .apm-growth-fill,
  .apm-flying-avatar {
    transition: none !important;
    animation: none !important;
  }

  .apm-icon-btn:hover:not(:disabled) {
    transform: none;
  }
}

@media (max-width: 560px) {
  .apm-stats-grid {
    grid-template-columns: 1fr;
  }
}
</style>

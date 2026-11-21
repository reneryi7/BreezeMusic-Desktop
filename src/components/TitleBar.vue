<template>
  <!-- 顶层全屏无框悬浮交互层：居中「灵动岛」果冻导航（含左翼液滴分裂返回球） + 右上角纯图标控制舱 -->
  <div class="skyline-floating-overlay">
    <!-- 顶部边缘窗口拖拽热区 -->
    <div
      class="skyline-top-drag-strip"
      data-tauri-drag-region
      @mouseenter="store.isStageScrolledDown = false"
    ></div>

    <!-- 居中悬浮灵动岛：左翼液滴分裂返回胶囊 + 五合一果冻展开主导航 -->
    <div
      class="skyline-floating-center"
      :class="{ 'is-np-hidden': store.isNowPlayingOpen }"
      :aria-hidden="store.isNowPlayingOpen"
    >
      <!-- 左翼液滴分裂返回球：进入任何二级独立页面时从灵动岛左侧弹性分裂弹出 -->
      <Transition name="island-split">
        <button
          v-if="showIslandBackOrb"
          type="button"
          class="island-split-back-orb"
          data-tip="返回上一页"
          data-tip-pos="bottom"
          aria-label="返回上一页"
          @click="handleIslandBack"
        >
          <PlumpIcon name="back" :size="18" />
        </button>
      </Transition>

      <nav class="skyline-nav-capsule dynamic-island-capsule" aria-label="主导航">
        <button
          v-for="item in navItems"
          :key="item.id"
          type="button"
          class="skyline-nav-btn island-tab-btn"
          :class="{
            active: isNavItemActive(item.id),
            'icon-only': !isNavItemActive(item.id),
          }"
          :aria-label="item.label"
          :aria-current="isNavItemActive(item.id) ? 'page' : undefined"
          :data-tip="item.hint"
          data-tip-pos="bottom"
          @click="handleNavClick(item.id)"
          @dblclick.stop="handleNavDblClick(item.id)"
        >
          <PlumpIcon
            :name="item.icon"
            :size="18"
            :color="isNavItemActive(item.id) ? '#ffffff' : 'currentColor'"
            :accent="
              isNavItemActive(item.id)
                ? 'rgba(255, 255, 255, 0.92)'
                : 'var(--primary)'
            "
          />
          <span class="island-tab-label-wrap">
            <span class="island-tab-label-inner">{{ item.label }}</span>
          </span>
        </button>
      </nav>
    </div>

    <!-- 右上角控制区：播放页显示无框沉浸式三键（最小化/最大化/关闭），其余页面显示完整悬浮控制舱 -->
    <div class="skyline-floating-right">
      <div
        class="np-immersive-win-controls"
        :class="{
          'is-active': store.isNowPlayingOpen,
          'is-light-stage': store.isNowPlayingLightStage,
        }"
        :aria-hidden="!store.isNowPlayingOpen"
      >
        <button
          type="button"
          class="np-immersive-win-btn"
          @click="minimizeWin"
        >
          <PlumpIcon name="minimize" :size="15" />
        </button>
        <button
          type="button"
          class="np-immersive-win-btn"
          @click="toggleMaximizeWin"
        >
          <PlumpIcon name="maximize" :size="14" />
        </button>
        <button
          type="button"
          class="np-immersive-win-btn is-close"
          @click="closeWin"
        >
          <PlumpIcon name="close" :size="15" />
        </button>
      </div>

      <div
        class="skyline-win-floating-capsule skyline-right-pure-capsule"
        :class="{ 'is-np-hidden': store.isNowPlayingOpen }"
        :aria-hidden="store.isNowPlayingOpen"
      >
        <button
          type="button"
          class="pure-avatar-orb-btn"
          :aria-label="
            store.isLoggedIn
              ? `${store.profile?.nickname || '音乐行者'} · 点击查看个人资料`
              : '点击登录酷狗账号'
          "
          :aria-haspopup="store.isLoggedIn ? 'dialog' : undefined"
          :aria-expanded="store.isLoggedIn ? store.isAccountProfileOpen : undefined"
          :data-tip="
            store.isLoggedIn
              ? `${store.profile?.nickname || '音乐行者'} · 点击查看个人资料`
              : '点击登录酷狗账号'
          "
          data-tip-pos="bottom"
          @click="handleAvatarOrbClick"
        >
          <img
            v-if="store.isLoggedIn && (store.profile?.avatarUrl || store.session?.avatarUrl)"
            :src="store.profile?.avatarUrl || store.session?.avatarUrl"
            draggable="false"
            alt="avatar"
          />
          <PlumpIcon v-else name="user" :size="16" color="#ffffff" accent="#ffffff" />
          <span v-if="store.isLoggedIn" class="pure-avatar-vip-dot"></span>
        </button>

        <button
          type="button"
          class="win-capsule-btn"
          data-tip="切换 Mini 灵动岛小窗 (Ctrl+Alt+M)"
          data-tip-pos="bottom"
          @click="store.toggleMiniPlayerMode()"
        >
          <PlumpIcon name="mini-island" :size="16" />
        </button>

        <button
          type="button"
          class="win-capsule-pill-variant"
          :class="`is-${currentVariantKey}`"
          :data-tip="variantCapsuleTip"
          data-tip-pos="bottom"
          @click="handleCycleCurrentVariant($event)"
        >
          <span class="variant-dot" aria-hidden="true"></span>
          <span class="variant-text">{{ currentVariantMeta.shortBadge }}</span>
        </button>

        <button
          type="button"
          class="win-capsule-btn theme-btn"
          :data-tip="themeBtnTitle"
          data-tip-pos="bottom"
          @click="toggleColorScheme($event)"
        >
          <transition name="bm-theme-icon-swap" mode="out-in">
            <PlumpIcon
              :key="store.resolvedColorScheme === 'watercolor_light' ? 'moon' : 'sun'"
              :name="
                store.resolvedColorScheme === 'watercolor_light' ? 'moon' : 'sun'
              "
              :size="16"
            />
          </transition>
        </button>

        <button
          type="button"
          class="win-capsule-btn"
          data-tip="最小化"
          data-tip-pos="bottom"
          @click="minimizeWin"
        >
          <PlumpIcon name="minimize" :size="14" />
        </button>
        <button
          type="button"
          class="win-capsule-btn"
          data-tip="最大化 / 还原"
          data-tip-pos="bottom"
          @click="toggleMaximizeWin"
        >
          <PlumpIcon name="maximize" :size="14" />
        </button>
        <button
          type="button"
          class="win-capsule-btn close"
          data-tip="关闭窗口"
          data-tip-pos="bottom"
          @click="closeWin"
        >
          <PlumpIcon name="close" :size="14" />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.skyline-floating-center {
  transition:
    opacity 0.28s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.32s cubic-bezier(0.22, 1, 0.36, 1);
  will-change: opacity, transform;
}

.skyline-floating-center.is-np-hidden {
  opacity: 0;
  transform: translate3d(-50%, -14px, 0) scale3d(0.95, 0.95, 1);
  pointer-events: none !important;
}

.skyline-floating-right {
  position: fixed;
  top: 14px;
  right: clamp(12px, 1.15vw, 18px);
  display: inline-flex;
  align-items: center;
  justify-content: flex-end;
  pointer-events: auto;
  z-index: 125;
}

.skyline-win-floating-capsule {
  transition:
    opacity 0.26s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.3s cubic-bezier(0.22, 1, 0.36, 1);
  will-change: opacity, transform;
}

.skyline-win-floating-capsule.is-np-hidden {
  opacity: 0;
  transform: translate3d(0, -10px, 0) scale3d(0.95, 0.95, 1);
  pointer-events: none !important;
}

.np-immersive-win-controls {
  position: absolute;
  right: 0;
  top: 50%;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 2px 0;
  opacity: 0;
  transform: translate3d(0, calc(-50% - 8px), 0) scale3d(0.95, 0.95, 1);
  pointer-events: none;
  transition:
    opacity 0.28s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.32s cubic-bezier(0.22, 1, 0.36, 1);
  will-change: opacity, transform;
}

.np-immersive-win-controls.is-active {
  opacity: 1;
  transform: translate3d(0, -50%, 0) scale3d(1, 1, 1);
  pointer-events: auto;
}

.np-immersive-win-btn {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: transparent;
  color: rgba(255, 255, 255, 0.68);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition:
    color 0.2s ease,
    background 0.22s cubic-bezier(0.22, 1, 0.36, 1);
}

.np-immersive-win-btn:hover {
  color: #ffffff;
  background: rgba(255, 255, 255, 0.12);
}

.np-immersive-win-btn.is-close:hover {
  color: #ffffff;
  background: rgba(255, 77, 109, 0.82);
}

.np-immersive-win-controls.is-light-stage .np-immersive-win-btn {
  color: rgba(24, 34, 48, 0.72);
}

.np-immersive-win-controls.is-light-stage .np-immersive-win-btn:hover {
  color: #182230;
  background: rgba(24, 34, 48, 0.08);
}

.np-immersive-win-controls.is-light-stage .np-immersive-win-btn.is-close:hover {
  color: #ffffff;
  background: rgba(244, 63, 94, 0.88);
}

/* 右上角 L1/L2 或 D1/D2 快速对比切换微胶囊（100% 零边框，柔和色垫区分） */
.win-capsule-pill-variant {
  height: 28px;
  padding: 0 10px 0 8px;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(in srgb, var(--primary) 14%, var(--bg-pill));
  color: var(--primary);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-heavy, 800);
  letter-spacing: 0.02em;
  white-space: nowrap;
  flex-shrink: 0;
  cursor: pointer;
  margin-left: 2px;
  transition:
    transform 0.2s cubic-bezier(0.22, 1, 0.36, 1),
    background 0.2s ease,
    color 0.2s ease;
}

.win-capsule-pill-variant:hover {
  background: color-mix(in srgb, var(--primary) 22%, var(--bg-pill-hover));
  transform: translateY(-1px);
}

.scheme-watercolor_light.light-variant-nordic_oat .win-capsule-pill-variant {
  background: #E8E1D5;
  color: var(--primary);
}

.scheme-watercolor_light.light-variant-nordic_oat .win-capsule-pill-variant:hover {
  background: #DED5C6;
}

.win-capsule-pill-variant .variant-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  background: linear-gradient(135deg, #38bdf8 0%, #f43f5e 52%, #a78bfa 100%);
}

.win-capsule-pill-variant.is-nordic_oat .variant-dot {
  background: linear-gradient(135deg, #b45309 0%, #be123c 55%, #0f766e 100%);
}

.win-capsule-pill-variant.is-obsidian_aurora .variant-dot {
  background: linear-gradient(135deg, #7c6af6 0%, #2dd4bf 55%, #fb7185 100%);
}

.win-capsule-pill-variant.is-velvet_nebula .variant-dot {
  background: linear-gradient(135deg, #c084fc 0%, #fb7185 52%, #fbbf24 100%);
}

@media (max-width: 960px) {
  .win-capsule-pill-variant .variant-text {
    display: none;
  }
  .win-capsule-pill-variant {
    padding: 0 7px;
  }
}
</style>

<script setup lang="ts">
import { computed, nextTick } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore, type MainTab } from "../stores/appStore";
import { runThemeCircularTransition } from "../utils/microMotionFx";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();

function handleAvatarOrbClick(event: MouseEvent) {
  if (store.isLoggedIn) {
    store.openAccountProfile(event.currentTarget as HTMLElement | null, {
      anchorMode: "top-right",
    });
  } else {
    store.isLoginModalOpen = true;
  }
}

const showIslandBackOrb = computed(() => {
  if (store.isNowPlayingOpen) return false;
  if (store.navHistoryStack.length > 0) return true;
  if (
    store.activeTab === "playlist_detail" ||
    store.activeTab === "album_detail" ||
    store.activeTab === "artist_detail" ||
    store.activeTab === "cloud_drive" ||
    store.activeTab === "local_music"
  ) {
    return true;
  }
  if (store.activeTab === "discover" && store.discoverSubView !== "home") {
    return true;
  }
  if (store.activeTab === "search" && store.searchViewMode === "results") {
    return true;
  }
  return false;
});

function handleIslandBack() {
  store.navigateBack();
}

const navItems: Array<{
  id: MainTab;
  label: string;
  hint: string;
  icon:
    | "nav-discover"
    | "nav-search"
    | "nav-library"
    | "nav-tuner"
    | "nav-settings";
}> = [
  {
    id: "discover",
    label: "发现",
    hint: "发现 · 双击刷新推荐",
    icon: "nav-discover",
  },
  {
    id: "search",
    label: "搜索",
    hint: "搜索 (Ctrl+K)",
    icon: "nav-search",
  },
  {
    id: "library",
    label: "我的",
    hint: "我的音乐 · 双击同步资产",
    icon: "nav-library",
  },
  {
    id: "tuner",
    label: "调音台",
    hint: "调音台 · 均衡器与声学音效",
    icon: "nav-tuner",
  },
  {
    id: "settings",
    label: "设置",
    hint: "设置 · 主题与声学偏好",
    icon: "nav-settings",
  },
];

function isNavItemActive(id: MainTab): boolean {
  if (store.isNowPlayingOpen) return false;
  if (id === "discover") {
    return (
      store.activeTab === "discover" ||
      store.activeTab === "playlist_detail" ||
      store.activeTab === "album_detail" ||
      store.activeTab === "artist_detail"
    );
  }
  if (id === "library") {
    return (
      store.activeTab === "library" ||
      store.activeTab === "cloud_drive" ||
      store.activeTab === "local_music"
    );
  }
  if (id === "tuner") {
    return store.activeTab === "tuner" || store.activeTab === "radio";
  }
  return store.activeTab === id;
}

function handleNavClick(id: MainTab) {
  store.isNowPlayingOpen = false;
  store.clearNavHistory();
  if (id === "discover") {
    store.discoverHeroExpanding = "none";
    store.discoverSubView = "home";
  } else if (id === "search") {
    store.openSearchLanding(true);
    store.activeTab = "search";
    nextTick(() => {
      const input = document.getElementById(
        "independent-search-input"
      ) as HTMLInputElement | null;
      if (input && store.searchViewMode === "landing") {
        input.focus();
      }
    });
    return;
  }
  store.activeTab = id;
}

function handleNavDblClick(id: MainTab) {
  store.isNowPlayingOpen = false;
  store.clearNavHistory();
  store.activeTab = id;
  if (id === "discover") {
    store.discoverHeroExpanding = "none";
    store.discoverSubView = "home";
    store.refreshDiscoverAll();
  } else if (id === "search") {
    store.openSearchLanding(true);
    store.loadSearchDiscovery(true).then(() => {
      store.notify("实时热搜歌曲与热门歌手已刷新 ✨");
    });
  } else if (id === "library") {
    store.notify("正在同步云端音乐资产与听歌等级...");
    Promise.all([
      store.refreshAccountAndVip(false),
      store.loadUserPlaylists(),
      store.loadFollowedArtists(),
      store.loadPurchasedAssets(),
    ]).then(() => {
      store.notify("云端音乐资产同步完成 ✨");
    });
  }
}

const currentVariantKey = computed(() =>
  store.resolvedColorScheme === "watercolor_light"
    ? store.lightThemeVariant
    : store.darkThemeVariant
);

const currentVariantMeta = computed(() =>
  store.resolvedColorScheme === "watercolor_light"
    ? store.lightThemeVariantMeta
    : store.darkThemeVariantMeta
);

const variantCapsuleTip = computed(() => {
  return store.resolvedColorScheme === "watercolor_light"
    ? `浅色预设：${store.lightThemeVariantMeta.name} · 点击对比 L1 / L2`
    : `深色预设：${store.darkThemeVariantMeta.name} · 点击对比 D1 / D2`;
});

const themeBtnTitle = computed(() => {
  return store.resolvedColorScheme === "watercolor_light"
    ? `当前：${store.lightThemeVariantMeta.name} · 点击切换 ${store.darkThemeVariantMeta.name}（深色）`
    : `当前：${store.darkThemeVariantMeta.name} · 点击切换 ${store.lightThemeVariantMeta.name}（浅色）`;
});

function handleCycleCurrentVariant(event?: MouseEvent) {
  runThemeCircularTransition(event, async () => {
    const next = store.cycleCurrentModeVariant();
    const modeLabel =
      store.resolvedColorScheme === "watercolor_light" ? "浅色" : "深色";
    store.notify(`已切换${modeLabel}方案：「${next.name}」`);
    await nextTick();
  });
}

function toggleColorScheme(event?: MouseEvent) {
  runThemeCircularTransition(event, async () => {
    if (store.resolvedColorScheme === "watercolor_light") {
      store.colorScheme = "abyssal_dark";
      store.notify(`已切换为「${store.darkThemeVariantMeta.name}（深色）」主题`);
    } else {
      store.colorScheme = "watercolor_light";
      store.notify(`已切换为「${store.lightThemeVariantMeta.name}（浅色）」主题`);
    }
    await nextTick();
  });
}

async function minimizeWin() {
  try {
    await getCurrentWindow().minimize();
  } catch {
    store.notify("浏览器预览模式：最小化窗口");
  }
}

async function toggleMaximizeWin() {
  try {
    await getCurrentWindow().toggleMaximize();
  } catch {
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen().catch(() => {});
    } else {
      document.exitFullscreen().catch(() => {});
    }
  }
}

async function closeWin() {
  try {
    if (store.closeToTray) {
      await getCurrentWindow().hide();
      store.notify("微风音乐已最小化至系统托盘");
    } else {
      await getCurrentWindow().close();
    }
  } catch {
    store.notify("浏览器预览模式：关闭窗口");
  }
}
</script>

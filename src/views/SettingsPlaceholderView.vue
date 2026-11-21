<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import type { SettingsSectionId } from "../types";
import {
  APP_VERSION_TAG,
  SETTINGS_NAV_GROUPS,
  SETTINGS_NAV_ITEMS,
  SETTINGS_SEARCH_INDEX,
  type SettingsNavItem,
  type SettingsSearchEntry,
} from "../utils/settingsCatalog";
import PlumpIcon from "../components/PlumpIcon.vue";
import SettingsAppearancePanel from "../components/settings/SettingsAppearancePanel.vue";
import SettingsTypographyPanel from "../components/settings/SettingsTypographyPanel.vue";
import SettingsGeneralPanel from "../components/settings/SettingsGeneralPanel.vue";
import SettingsPlaybackPanel from "../components/settings/SettingsPlaybackPanel.vue";
import SettingsLyricsPanel from "../components/settings/SettingsLyricsPanel.vue";
import SettingsLocalMusicPanel from "../components/settings/SettingsLocalMusicPanel.vue";
import SettingsDownloadsPanel from "../components/settings/SettingsDownloadsPanel.vue";
import SettingsShortcutsPanel from "../components/settings/SettingsShortcutsPanel.vue";
import SettingsStorageNetworkPanel from "../components/settings/SettingsStorageNetworkPanel.vue";
import SettingsAboutPanel from "../components/settings/SettingsAboutPanel.vue";

const store = useAppStore();

const searchQuery = ref("");
const highlightedSettingId = ref("");
const detailStageRef = ref<HTMLElement | null>(null);
const tabButtonRefs = ref<Record<string, HTMLButtonElement | null>>({});
let highlightTimer: ReturnType<typeof setTimeout> | null = null;

const visibleNavItems = computed<SettingsNavItem[]>(() =>
  SETTINGS_NAV_ITEMS.filter(
    (item) => !item.hiddenUnlessUnlocked || store.downloadFeatureUnlocked
  )
);

const activeSection = computed<SettingsSectionId>({
  get: () => {
    if (
      store.settingsActiveSection === "downloads" &&
      !store.downloadFeatureUnlocked
    ) {
      return "about";
    }
    return store.settingsActiveSection;
  },
  set: (val) => {
    if (val === "downloads" && !store.downloadFeatureUnlocked) return;
    store.settingsActiveSection = val;
  },
});

const activeNavMeta = computed<SettingsNavItem>(
  () =>
    visibleNavItems.value.find((item) => item.id === activeSection.value) ||
    visibleNavItems.value[0]
);

const sectionDescriptions: Record<SettingsSectionId, string> = {
  appearance: "主题氛围、动效档位、信息密度与沉浸播放舞台默认偏好",
  typography: "通过独立字体预览窗口挑选或导入字体，精确调节字号、字重与行距",
  general: "账号会话、启动默认首页、托盘驻留与 Windows 自启动行为",
  playback: "流媒体音质、硬件声卡通道、跨曲淡变、安全音量与延迟校准",
  lyrics: "管理桌面透明悬浮歌词与沉浸播放页歌词排版动效",
  local_music: "集中管理本地音乐多目录、扫描过滤规则、去重策略与元数据缓存",
  downloads: "多任务并发下载队列、历史下载归档管理与纯 Rust 标签命名偏好",
  shortcuts: "自定义系统级全局热键与应用内快捷键，实时检测按键冲突",
  storage_network: "四维磁盘缓存占用清理、LRU 配额管理与音源网关链路诊断",
  about: "软件版本信息、项目渊源、核心引擎架构与运行目录入口",
};

const groupedNavItems = computed(() =>
  SETTINGS_NAV_GROUPS.map((group) => ({
    ...group,
    items: visibleNavItems.value.filter((item) => item.group === group.id),
  })).filter((g) => g.items.length > 0)
);

const trimmedQuery = computed(() => searchQuery.value.trim().toLowerCase());
const isSearching = computed(() => trimmedQuery.value.length > 0);

const searchResults = computed<SettingsSearchEntry[]>(() => {
  const q = trimmedQuery.value;
  if (!q) return [];
  const tokens = q.split(/\s+/).filter(Boolean);
  return SETTINGS_SEARCH_INDEX.filter((entry) => {
    if (entry.section === "downloads" && !store.downloadFeatureUnlocked) {
      return false;
    }
    const haystack = [
      entry.title,
      entry.description,
      entry.sectionTitle,
      ...entry.keywords,
    ]
      .join(" ")
      .toLowerCase();
    return tokens.every((tk) => haystack.includes(tk));
  });
});

function getNavDynamicBadge(id: SettingsSectionId): string {
  if (id === "appearance") {
    return store.isDarkTheme
      ? store.darkThemeVariantMeta.shortBadge.split(" ")[0]
      : store.lightThemeVariantMeta.shortBadge.split(" ")[0];
  }
  if (id === "typography") {
    const pct = Math.round(store.typographyConfig.uiFontScale * 100);
    if (pct !== 100) return `${pct}%`;
    if (store.typographyConfig.uiFontFamily) return "自定";
    return "";
  }
  if (id === "playback") {
    const q = store.preferredQuality;
    if (q === "high") return "Hi-Res";
    if (q === "flac") return "SQ";
    if (q === "320") return "320K";
    return "128K";
  }
  if (id === "lyrics") {
    return store.desktopLyricEnabled ? "桌面开" : "";
  }
  if (id === "local_music") {
    return store.localMusicSongs.length > 0
      ? `${store.localMusicSongs.length}`
      : "";
  }
  if (id === "downloads") {
    const running = store.downloadQueue.filter(
      (t) =>
        t.status === "queued" ||
        t.status === "resolving" ||
        t.status === "downloading" ||
        t.status === "tagging"
    ).length;
    if (running > 0) return `${running} 进行中`;
    return store.downloadHistory.length > 0
      ? `${store.downloadHistory.length}`
      : "";
  }
  if (id === "shortcuts") {
    if (
      store.osGlobalHotkeysEnabled &&
      store.globalHotkeyFailedActions.length > 0
    ) {
      return `${store.globalHotkeyFailedActions.length} 冲突`;
    }
    return store.osGlobalHotkeysEnabled ? "全局开" : "";
  }
  return "";
}

function setTabBtnRef(id: string, el: unknown) {
  tabButtonRefs.value[id] = el instanceof HTMLButtonElement ? el : null;
}

function scrollDetailToTop(smooth = false) {
  const stage = detailStageRef.value;
  if (!stage) return;
  if (stage.scrollTop <= 0) return;
  stage.scrollTo({
    top: 0,
    behavior: smooth && store.motionLevel !== "none" ? "smooth" : "auto",
  });
}

function selectSection(id: SettingsSectionId) {
  activeSection.value = id;
  if (isSearching.value) {
    searchQuery.value = "";
  }
  highlightedSettingId.value = "";
  scrollDetailToTop(false);
}

watch(activeSection, () => {
  scrollDetailToTop(false);
});

function handleTabKeyDown(e: KeyboardEvent, currentId: SettingsSectionId) {
  const list = visibleNavItems.value;
  const idx = list.findIndex((i) => i.id === currentId);
  if (idx < 0) return;

  let nextIdx = idx;
  if (e.key === "ArrowDown" || e.key === "ArrowRight") {
    e.preventDefault();
    nextIdx = (idx + 1) % list.length;
  } else if (e.key === "ArrowUp" || e.key === "ArrowLeft") {
    e.preventDefault();
    nextIdx = (idx - 1 + list.length) % list.length;
  } else if (e.key === "Home") {
    e.preventDefault();
    nextIdx = 0;
  } else if (e.key === "End") {
    e.preventDefault();
    nextIdx = list.length - 1;
  } else {
    return;
  }

  const nextItem = list[nextIdx];
  selectSection(nextItem.id);
  nextTick(() => {
    tabButtonRefs.value[nextItem.id]?.focus();
  });
}

function jumpToSearchEntry(entry: SettingsSearchEntry) {
  searchQuery.value = "";
  activeSection.value = entry.section;
  highlightedSettingId.value = entry.id;

  if (highlightTimer) {
    clearTimeout(highlightTimer);
    highlightTimer = null;
  }

  nextTick(() => {
    setTimeout(() => {
      const target = detailStageRef.value?.querySelector(
        `[data-setting-id="${entry.id}"]`
      ) as HTMLElement | null;
      if (target) {
        target.scrollIntoView({
          behavior: store.motionLevel === "none" ? "auto" : "smooth",
          block: "center",
        });
      }
    }, 180);
    highlightTimer = setTimeout(() => {
      highlightedSettingId.value = "";
      highlightTimer = null;
    }, 2400);
  });
}

function handleSearchKeyDown(e: KeyboardEvent) {
  if (e.isComposing) return;
  if (e.key === "Escape") {
    e.stopPropagation();
    searchQuery.value = "";
  } else if (e.key === "Enter" && searchResults.value.length > 0) {
    e.preventDefault();
    jumpToSearchEntry(searchResults.value[0]);
  }
}

onMounted(() => {
  store.refreshCacheStats();
  store.refreshAutostartState();
});

onBeforeUnmount(() => {
  if (highlightTimer) {
    clearTimeout(highlightTimer);
    highlightTimer = null;
  }
});
</script>

<template>
  <section class="bms-settings-shell" aria-label="微风音乐设置中心">
    <!-- 左侧固定导航栏：精简抬头 + 搜索 + 单行清爽分类，永不随右侧内容滚动 -->
    <aside class="bms-nav-rail" aria-label="设置分类导航">
      <div class="bms-rail-head">
        <div class="bms-rail-title-wrap">
          <div class="bms-rail-logo-orb">
            <PlumpIcon name="nav-settings" :size="16" color="var(--primary)" />
          </div>
          <span class="bms-rail-title">设置中心</span>
        </div>
        <span class="bms-rail-version">{{ APP_VERSION_TAG }}</span>
      </div>

      <search class="bms-search-box" aria-label="检索设置项">
        <PlumpIcon name="search" :size="15" color="var(--text-muted)" />
        <label for="bms-settings-search" class="bms-visually-hidden">
          搜索全部设置项
        </label>
        <input
          id="bms-settings-search"
          v-model="searchQuery"
          type="search"
          class="bms-search-input"
          placeholder="搜索主题、字体、音质、本地…"
          autocomplete="off"
          autocorrect="off"
          autocapitalize="none"
          spellcheck="false"
          @keydown="handleSearchKeyDown"
        />
        <button
          v-if="searchQuery"
          type="button"
          class="bms-search-clear"
          aria-label="清空设置搜索词"
          @click="searchQuery = ''"
        >
          <PlumpIcon name="close" :size="11" />
        </button>
      </search>

      <div
        class="bms-nav-groups"
        role="tablist"
        aria-label="设置分类"
        aria-orientation="vertical"
      >
        <div
          v-for="group in groupedNavItems"
          :key="group.id"
          class="bms-nav-group"
        >
          <div class="bms-nav-group-label" aria-hidden="true">
            {{ group.label }}
          </div>
          <button
            v-for="item in group.items"
            :key="item.id"
            :id="`bms-tab-${item.id}`"
            :ref="(el) => setTabBtnRef(item.id, el)"
            type="button"
            role="tab"
            class="bms-nav-item"
            :aria-selected="!isSearching && activeSection === item.id ? 'true' : 'false'"
            :tabindex="!isSearching && activeSection === item.id ? 0 : -1"
            @click="selectSection(item.id)"
            @keydown="handleTabKeyDown($event, item.id)"
          >
            <span class="bms-nav-pill-bar" aria-hidden="true"></span>
            <span class="bms-nav-icon-box">
              <PlumpIcon
                :name="item.icon"
                :size="16"
                :color="!isSearching && activeSection === item.id ? '#ffffff' : item.accentVar"
              />
            </span>
            <span class="bms-nav-title">{{ item.title }}</span>
            <span
              v-if="getNavDynamicBadge(item.id)"
              class="bms-nav-badge"
            >
              {{ getNavDynamicBadge(item.id) }}
            </span>
          </button>
        </div>
      </div>
    </aside>

    <!-- 右侧独立滚动详情舞台 -->
    <div
      ref="detailStageRef"
      class="bms-detail-stage"
      role="tabpanel"
      :aria-labelledby="isSearching ? undefined : `bms-tab-${activeSection}`"
    >
      <div class="bms-detail-inner">
        <!-- 搜索命中模式 -->
        <template v-if="isSearching">
          <header class="bms-detail-header">
            <div class="bms-detail-title-group">
              <h1 class="bms-detail-title">
                搜索「{{ searchQuery.trim() }}」
              </h1>
              <p class="bms-detail-desc">
                共找到 {{ searchResults.length }} 项相关设置，点击即可直达并高亮定位。
              </p>
            </div>
            <button
              type="button"
              class="bms-btn ghost"
              @click="searchQuery = ''"
            >
              <PlumpIcon name="close" :size="13" />
              <span>返回当前分类</span>
            </button>
          </header>

          <div v-if="searchResults.length > 0" class="bms-search-results-panel">
            <button
              v-for="entry in searchResults"
              :key="entry.id"
              type="button"
              class="bms-search-result-btn"
              @click="jumpToSearchEntry(entry)"
            >
              <div class="bms-sr-main">
                <div class="bms-sr-top">
                  <span class="bms-sr-section-chip">{{ entry.sectionTitle }}</span>
                  <span class="bms-sr-title">{{ entry.title }}</span>
                </div>
                <span class="bms-sr-desc">{{ entry.description }}</span>
              </div>
              <PlumpIcon name="chevron-right" :size="16" color="var(--text-muted)" />
            </button>
          </div>

          <div v-else class="bms-empty-search">
            <PlumpIcon name="search" :size="28" color="var(--text-muted)" />
            <p>未找到与「{{ searchQuery.trim() }}」匹配的设置项</p>
            <button type="button" class="bms-btn" @click="searchQuery = ''">
              清空关键词
            </button>
          </div>
        </template>

        <!-- 正常分类详情模式 -->
        <template v-else>
          <header class="bms-detail-header">
            <div class="bms-detail-title-group">
              <h1 class="bms-detail-title">{{ activeNavMeta.title }}</h1>
              <p class="bms-detail-desc">
                {{ sectionDescriptions[activeSection] }}
              </p>
            </div>
          </header>

          <Transition
            :name="store.motionLevel === 'none' ? '' : 'bms-pane-fade'"
            mode="out-in"
          >
            <SettingsAppearancePanel
              v-if="activeSection === 'appearance'"
              key="appearance"
              :highlighted-id="highlightedSettingId"
            />
            <SettingsTypographyPanel
              v-else-if="activeSection === 'typography'"
              key="typography"
              :highlighted-id="highlightedSettingId"
            />
            <SettingsGeneralPanel
              v-else-if="activeSection === 'general'"
              key="general"
              :highlighted-id="highlightedSettingId"
            />
            <SettingsPlaybackPanel
              v-else-if="activeSection === 'playback'"
              key="playback"
              :highlighted-id="highlightedSettingId"
            />
            <SettingsLyricsPanel
              v-else-if="activeSection === 'lyrics'"
              key="lyrics"
              :highlighted-id="highlightedSettingId"
            />
            <SettingsLocalMusicPanel
              v-else-if="activeSection === 'local_music'"
              key="local_music"
              :highlighted-id="highlightedSettingId"
            />
            <SettingsDownloadsPanel
              v-else-if="activeSection === 'downloads' && store.downloadFeatureUnlocked"
              key="downloads"
              :highlighted-id="highlightedSettingId"
            />
            <SettingsShortcutsPanel
              v-else-if="activeSection === 'shortcuts'"
              key="shortcuts"
              :highlighted-id="highlightedSettingId"
            />
            <SettingsStorageNetworkPanel
              v-else-if="activeSection === 'storage_network'"
              key="storage_network"
              :highlighted-id="highlightedSettingId"
            />
            <SettingsAboutPanel
              v-else
              key="about"
              :highlighted-id="highlightedSettingId"
            />
          </Transition>
        </template>
      </div>
    </div>
  </section>
</template>

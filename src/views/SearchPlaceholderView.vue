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
import { formatDuration, kugouClient, resolveSongQualityBadge } from "../api/kugou";
import { useViewportStagger } from "../composables/useViewportStagger";
import type { FollowedArtist, Playlist, SearchCategoryTab, Song } from "../types";
import PlumpIcon from "../components/PlumpIcon.vue";
import FloatingLocateFab from "../components/FloatingLocateFab.vue";

const store = useAppStore();

const locateSongs = computed<Song[]>(() =>
  store.searchViewMode === "results" && store.searchCategoryTab === "songs"
    ? store.searchResultsSongs
    : []
);

const searchInputRef = ref<HTMLInputElement | null>(null);
const searchFormWrapRef = ref<HTMLElement | null>(null);
const searchSongsGridRef = ref<HTMLElement | null>(null);
const infiniteSentinelRef = ref<HTMLElement | null>(null);
const searchSongsStagger = useViewportStagger(searchSongsGridRef, {
  defaultCols: 2,
  rowHeight: 64,
  estimatedChromeOffset: 220,
});
const searchFocused = ref(false);
const activeBoardIndex = ref(0);
const expandedFoldKeys = reactive(new Set<string>());

const artistRegions = [
  { id: 1, label: "华语" },
  { id: 2, label: "欧美" },
  { id: 3, label: "日韩" },
];

const resultCategoryTabs = computed<
  Array<{ id: SearchCategoryTab; label: string; count?: number }>
>(() => [
  { id: "all", label: "综合" },
  { id: "songs", label: "单曲", count: store.searchTotals.songs },
  { id: "albums", label: "专辑", count: store.searchTotals.albums },
  { id: "artists", label: "歌手", count: store.searchTotals.artists },
  { id: "playlists", label: "歌单", count: store.searchTotals.playlists },
]);

const visibleHotBoards = computed(() =>
  (store.searchDiscovery.hotBoards || [])
    .filter((b) => b.entries && b.entries.length > 0)
    .slice(0, 9)
);

type SearchAllSectionId = "artists" | "songs" | "albums" | "playlists";

const isArtistSearchIntent = computed(() => {
  const kw = store.searchKeyword.trim().toLowerCase().replace(/\s+/g, "");
  if (!kw || store.searchResultsArtists.length === 0) return false;
  const topArtist = store.searchResultsArtists[0];
  const artistName = (topArtist?.name || "")
    .trim()
    .toLowerCase()
    .replace(/\s+/g, "");
  if (!artistName) return false;

  if (artistName === kw) return true;

  const exactDifferentSongMatch = store.searchResultsSongs
    .slice(0, 5)
    .some((s) => {
      const st = (s.title || "").trim().toLowerCase().replace(/\s+/g, "");
      return st === kw && artistName !== kw;
    });
  if (exactDifferentSongMatch) return false;

  if (
    kw.length >= 2 &&
    (artistName.startsWith(kw) ||
      (artistName.length >= 2 &&
        kw.includes(artistName) &&
        kw.length <= artistName.length + 2))
  ) {
    return true;
  }
  return false;
});

const orderedAllSections = computed<SearchAllSectionId[]>(() =>
  isArtistSearchIntent.value
    ? ["artists", "songs", "albums", "playlists"]
    : ["songs", "artists", "playlists", "albums"]
);

const currentBoardEntries = computed(() => {
  const boards = visibleHotBoards.value;
  if (boards.length > 0) {
    const idx = Math.min(activeBoardIndex.value, boards.length - 1);
    return boards[idx]?.entries || [];
  }
  return store.hotSearchKeywords.map((kw) => ({
    keyword: kw,
    reason: "",
    iconType: 0,
  }));
});

const showSuggestPopover = computed(
  () =>
    searchFocused.value &&
    store.searchKeyword.trim().length > 0 &&
    store.searchSuggestions.length > 0
);

function getCleanEntryReason(entry: { keyword: string; reason?: string }): string {
  const raw = (entry.reason || "").trim();
  if (!raw) return "";
  const rNorm = raw.replace(/(\.\.\.|…)+$/g, "").trim().toLowerCase();
  const kwNorm = (entry.keyword || "").trim().toLowerCase();
  if (
    !rNorm ||
    rNorm === kwNorm ||
    kwNorm.startsWith(rNorm) ||
    rNorm.startsWith(kwNorm) ||
    kwNorm.includes(rNorm)
  ) {
    return "";
  }
  return raw;
}

let suggestTimer: ReturnType<typeof setTimeout> | null = null;
let suggestSeq = 0;

watch(
  () => store.searchKeyword,
  (val) => {
    if (suggestTimer) {
      clearTimeout(suggestTimer);
      suggestTimer = null;
    }
    const kw = (val || "").trim();
    if (!kw || !searchFocused.value) {
      store.searchSuggestions = [];
      return;
    }
    const seq = ++suggestSeq;
    suggestTimer = setTimeout(async () => {
      try {
        const tips = await kugouClient.getSearchSuggest(kw);
        if (seq === suggestSeq) {
          store.searchSuggestions = (tips || []).filter((t) => t && t.trim() !== kw).slice(0, 8);
        }
      } catch {
        if (seq === suggestSeq) {
          store.searchSuggestions = [];
        }
      }
    }, 200);
  }
);

function getSongKey(song: Song, idx: number): string {
  return `${song.hash || song.id || "s"}_${song.albumAudioId || 0}_${idx}`;
}

function getFoldKey(song: Song): string {
  return `${song.hash || song.id}_${song.albumAudioId || 0}_${song.albumId || ""}`;
}

function toggleFoldedVersions(song: Song) {
  const k = getFoldKey(song);
  if (expandedFoldKeys.has(k)) {
    expandedFoldKeys.delete(k);
  } else {
    expandedFoldKeys.add(k);
  }
}

function formatArtistStats(art: FollowedArtist): string {
  const parts: string[] = [];
  if (art.songCount && art.songCount > 0) {
    parts.push(`单曲 ${art.songCount}`);
  }
  if (art.albumCount && art.albumCount > 0) {
    parts.push(`专辑 ${art.albumCount}`);
  }
  if (art.fansCount && art.fansCount > 0) {
    parts.push(
      art.fansCount >= 10000
        ? `${(art.fansCount / 10000).toFixed(1).replace(/\.0$/, "")}w 粉丝`
        : `${art.fansCount} 粉丝`
    );
  }
  return parts.join(" · ") || "热门音乐人";
}

function formatCompactArtistMeta(art: FollowedArtist): string {
  if (art.fansCount && art.fansCount > 0) {
    return art.fansCount >= 10000
      ? `${(art.fansCount / 10000).toFixed(1).replace(/\.0$/, "")}w 粉丝`
      : `${art.fansCount} 粉丝`;
  }
  if (art.songCount && art.songCount > 0) {
    return `${art.songCount} 首单曲`;
  }
  if (art.albumCount && art.albumCount > 0) {
    return `${art.albumCount} 张专辑`;
  }
  return "热门音乐人";
}

function handleSearchKeydown(e: KeyboardEvent) {
  if (e.key === "Enter") {
    if (e.isComposing || e.keyCode === 229) return;
    e.preventDefault();
    submitSearch();
  } else if (e.key === "Escape") {
    searchFocused.value = false;
  }
}

function submitSearch(customKw?: string) {
  const target = (
    customKw !== undefined ? customKw : store.searchKeyword
  ).trim();
  if (!target) {
    store.openSearchLanding(true);
    return;
  }
  searchFocused.value = false;
  expandedFoldKeys.clear();
  store.searchCategoryTab = "all";
  store.executeSearch(target);
}

function clearSearchInput() {
  store.searchKeyword = "";
  store.searchSuggestions = [];
  expandedFoldKeys.clear();
  store.openSearchLanding(true);
  nextTick(() => {
    searchInputRef.value?.focus();
  });
}

function removeSingleHistory(item: string) {
  store.searchHistory = store.searchHistory.filter((h) => h !== item);
  localStorage.setItem(
    "bm_search_history",
    JSON.stringify(store.searchHistory)
  );
}

function openSearchedPlaylist(pl: Playlist) {
  store.openPlaylistDetail(pl);
}

function playAllHotSongs() {
  const list = store.searchDiscovery.hotSongs;
  if (list.length > 0) {
    store.playSongFromList(list[0], list, "实时热搜歌曲");
  }
}

const currentTabHasMore = computed(() => {
  const tab = store.searchCategoryTab;
  if (tab === "songs") return store.searchHasMore.songs;
  if (tab === "albums") return store.searchHasMore.albums;
  if (tab === "artists") return store.searchHasMore.artists;
  if (tab === "playlists") return store.searchHasMore.playlists;
  return false;
});

const currentTabLoadingMore = computed(() => {
  const tab = store.searchCategoryTab;
  if (tab === "songs") return store.isLoadingMoreSearch.songs;
  if (tab === "albums") return store.isLoadingMoreSearch.albums;
  if (tab === "artists") return store.isLoadingMoreSearch.artists;
  if (tab === "playlists") return store.isLoadingMoreSearch.playlists;
  return false;
});

function triggerLoadMoreCurrentTab() {
  const tab = store.searchCategoryTab;
  if (
    tab === "songs" ||
    tab === "albums" ||
    tab === "artists" ||
    tab === "playlists"
  ) {
    store.loadMoreSearchCategory(tab);
  }
}

let intersectionObserver: IntersectionObserver | null = null;

function setupInfiniteObserver() {
  if (intersectionObserver) {
    intersectionObserver.disconnect();
    intersectionObserver = null;
  }
  if (typeof IntersectionObserver === "undefined") return;
  const rootEl = document.querySelector(
    ".skyline-stage-viewport"
  ) as HTMLElement | null;
  intersectionObserver = new IntersectionObserver(
    (entries) => {
      const hit = entries.some((entry) => entry.isIntersecting);
      if (
        hit &&
        store.searchViewMode === "results" &&
        currentTabHasMore.value &&
        !currentTabLoadingMore.value &&
        !store.isSearching
      ) {
        triggerLoadMoreCurrentTab();
      }
    },
    {
      root: rootEl || null,
      rootMargin: "650px 0px",
      threshold: 0.01,
    }
  );
  if (infiniteSentinelRef.value) {
    intersectionObserver.observe(infiniteSentinelRef.value);
  }
}

watch(
  [
    () => store.searchViewMode,
    () => store.searchCategoryTab,
    () => store.isSearching,
    infiniteSentinelRef,
  ],
  () => {
    nextTick(() => {
      setupInfiniteObserver();
    });
  }
);

function handleOutsideMouseDown(e: MouseEvent) {
  const target = e.target as Node;
  if (searchFormWrapRef.value && !searchFormWrapRef.value.contains(target)) {
    searchFocused.value = false;
  }
}

function notifySearchStageBeforeEnter() {
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent("breeze:subpage-before-enter"));
  }
}

watch(
  () => store.searchViewMode,
  (mode) => {
    if (mode === "landing") {
      store.searchKeyword = "";
      store.searchSuggestions = [];
      searchFocused.value = false;
    }
  }
);

onMounted(() => {
  if (![1, 2, 3].includes(store.hotArtistCategory)) {
    store.hotArtistCategory = 1;
  }
  if (store.searchViewMode === "landing") {
    store.searchKeyword = "";
    store.searchSuggestions = [];
  }
  store.loadSearchDiscovery(true);
  window.addEventListener("mousedown", handleOutsideMouseDown);
  nextTick(() => {
    setupInfiniteObserver();
    if (store.searchViewMode === "landing") {
      searchInputRef.value?.focus();
    }
  });
});

onBeforeUnmount(() => {
  window.removeEventListener("mousedown", handleOutsideMouseDown);
  if (suggestTimer) {
    clearTimeout(suggestTimer);
    suggestTimer = null;
  }
  if (intersectionObserver) {
    intersectionObserver.disconnect();
    intersectionObserver = null;
  }
});
</script>

<template>
  <div class="page-scroll-canvas search-hub-page" @click="store.closeSongContextMenu()">
    <!-- ==================== 顶部核心搜索栏 ==================== -->
    <div
      ref="searchFormWrapRef"
      class="search-command-shell"
      :class="{ 'is-results-mode': store.searchViewMode === 'results' }"
    >
      <div class="search-command-center">
        <div class="search-command-row">
          <form
            role="search"
            class="search-hero-form"
            :class="{ focused: searchFocused }"
            @submit.prevent="submitSearch()"
          >
            <label class="visually-hidden" for="independent-search-input">
              搜索歌曲、歌手、专辑、歌单
            </label>
            <PlumpIcon
              name="nav-search"
              :size="19"
              color="var(--primary)"
              accent="var(--cyan, #00e5ff)"
              class="shf-leading-icon"
            />
            <input
              id="independent-search-input"
              ref="searchInputRef"
              v-model="store.searchKeyword"
              type="search"
              name="q"
              class="shf-input"
              placeholder="搜索歌曲、歌手、专辑、歌单..."
              autocomplete="off"
              autocorrect="off"
              autocapitalize="off"
              spellcheck="false"
              @focus="searchFocused = true"
              @keydown="handleSearchKeydown"
            />
            <button
              v-if="store.searchKeyword"
              type="button"
              class="shf-clear-btn"
              aria-label="清空搜索词"
              data-tip="清空搜索词"
              @click="clearSearchInput"
            >
              <PlumpIcon name="close" :size="13" />
            </button>
            <button
              type="submit"
              class="shf-submit-btn"
              data-tip="搜索 (Enter)"
            >
              <PlumpIcon name="search" :size="17" color="#ffffff" />
            </button>
          </form>
        </div>

        <!-- 实时联想词浮层 -->
        <transition name="fade-pop">
          <div v-if="showSuggestPopover" class="shf-suggest-popover">
            <button
              v-for="(sug, idx) in store.searchSuggestions"
              :key="`${sug}_${idx}`"
              type="button"
              class="shf-suggest-item"
              @click="submitSearch(sug)"
            >
              <PlumpIcon name="nav-search" :size="15" />
              <span>{{ sug }}</span>
            </button>
          </div>
        </transition>

        <!-- 最近搜索历史（仅在探索态展示，简洁清爽） -->
        <div
          v-if="
            store.searchViewMode === 'landing' && store.searchHistory.length > 0
          "
          class="search-history-bar"
        >
          <span class="shb-title">历史记录</span>
          <div class="shb-pills">
            <div
              v-for="item in store.searchHistory.slice(0, 12)"
              :key="item"
              class="shb-pill"
            >
              <button
                type="button"
                class="shb-pill-text"
                @click="submitSearch(item)"
              >
                {{ item }}
              </button>
              <button
                type="button"
                class="shb-pill-del"
                data-tip="删除该条历史"
                @click.stop="removeSingleHistory(item)"
              >
                <PlumpIcon name="close" :size="11" />
              </button>
            </div>
          </div>
          <button
            type="button"
            class="shb-clear-all"
            data-tip="清空全部搜索历史"
            @click="store.clearSearchHistory()"
          >
            <PlumpIcon name="trash" :size="14" />
          </button>
        </div>
      </div>
    </div>

    <!-- ==================== 视图一：独立搜索探索页（实时热门歌手 + 实时热搜歌曲与热搜榜） ==================== -->
    <transition
      :name="store.searchViewMode === 'results' ? 'subpage-drill-in' : 'subpage-drill-out'"
      mode="out-in"
      @before-enter="notifySearchStageBeforeEnter"
    >
    <div
      v-if="store.searchViewMode === 'landing'"
      key="search-landing"
      class="search-landing-canvas"
    >
      <!-- 1. 实时热门歌手陈列廊 -->
      <section class="sl-panel sl-artists-panel">
        <div class="sl-panel-head">
          <div class="sl-panel-title-wrap">
            <h3 class="sl-panel-title">热门歌手</h3>
            <div class="sl-region-tabs">
              <button
                v-for="reg in artistRegions"
                :key="reg.id"
                type="button"
                class="sl-region-pill"
                :class="{ active: store.hotArtistCategory === reg.id }"
                @click="store.switchHotArtistCategory(reg.id)"
              >
                {{ reg.label }}
              </button>
            </div>
          </div>
        </div>

        <transition name="tag-content-morph" mode="out-in">
          <div
            v-if="
              (store.isLoadingSearchDiscovery || store.isChangingHotArtistCat) &&
              store.searchDiscovery.hotArtists.length === 0
            "
            key="hot-artists-loading"
            class="sl-empty-hint"
          >
            正在获取热门歌手...
          </div>

          <div
            v-else
            :key="`${store.hotArtistCategory}_${store.searchDiscovery.hotArtists[0]?.artistId || 'empty'}`"
            class="sl-hot-artists-grid"
          >
            <button
              v-for="art in store.searchDiscovery.hotArtists.slice(0, 12)"
              :key="art.artistId"
              type="button"
              class="sl-hot-artist-card"
              @click="store.openArtistDetail(art.artistId, art.name, art.avatarUrl)"
            >
              <div class="shac-avatar-box">
                <img
                  v-if="art.avatarUrl"
                  :src="art.avatarUrl"
                  class="shac-avatar"
                  loading="lazy"
                  decoding="async"
                  draggable="false"
                  alt=""
                />
                <div v-else class="shac-avatar fallback">
                  {{ art.name.slice(0, 1) }}
                </div>
              </div>
              <span class="shac-name">{{ art.name }}</span>
              <span class="shac-meta">{{ formatCompactArtistMeta(art) }}</span>
            </button>
          </div>
        </transition>
      </section>

      <!-- 2. 双栏实时热搜矩阵：左侧「热搜歌曲」实时可播榜 + 右侧「热搜词榜」多维风向标 -->
      <div class="sl-dual-bento">
        <!-- 左栏：实时热搜歌曲（可直接点击播放） -->
        <section class="sl-panel sl-hot-songs-panel">
          <div class="sl-panel-head">
            <div class="sl-panel-title-wrap">
              <PlumpIcon
                name="flame"
                :size="18"
                color="var(--bm-accent-azure, var(--primary))"
                accent="var(--cyan)"
              />
              <h3 class="sl-panel-title">热搜歌曲</h3>
            </div>
            <div class="sl-head-actions">
              <button
                v-if="store.searchDiscovery.hotSongs.length > 0"
                type="button"
                class="sl-play-all-pill"
                data-tip="播放全部热搜歌曲"
                @click="playAllHotSongs"
              >
                <PlumpIcon name="play" :size="14" color="#ffffff" />
                <span>播放</span>
              </button>
            </div>
          </div>

          <div
            v-if="
              store.isLoadingSearchDiscovery &&
              store.searchDiscovery.hotSongs.length === 0
            "
            class="sl-empty-hint"
          >
            正在加载热搜歌曲...
          </div>

          <transition v-else name="tag-content-morph" mode="out-in">
            <div
              :key="store.searchDiscovery.hotSongs[0]?.hash || 'hot-songs'"
              class="frameless-songs-grid-2col"
            >
              <div
                v-for="(song, idx) in store.searchDiscovery.hotSongs.slice(0, 12)"
                :key="getSongKey(song, idx)"
                class="unified-song-row"
                :class="{ active: store.currentSong?.hash === song.hash }"
                @click="
                  store.playSongFromList(
                    song,
                    store.searchDiscovery.hotSongs,
                    '实时热搜歌曲'
                  )
                "
                @contextmenu.prevent="
                  store.openSongContextMenu(
                    $event,
                    song,
                    store.searchDiscovery.hotSongs
                  )
                "
              >
                <div class="usr-index-slot" :class="{ 'is-top': idx < 3 }">
                  <span
                    v-if="store.currentSong?.hash === song.hash"
                    class="mini-eq-bars"
                    :class="{ paused: !store.isPlaying }"
                  >
                    <i></i><i></i><i></i>
                  </span>
                  <span v-else>{{ String(idx + 1).padStart(2, "0") }}</span>
                </div>
                <div class="usr-cover-wrap">
                  <img
                    v-if="song.coverUrl"
                    :src="song.coverUrl"
                    loading="lazy"
                    decoding="async"
                    draggable="false"
                    alt=""
                  />
                  <div v-else class="usr-cover-fallback">
                    <PlumpIcon name="disc" :size="16" />
                  </div>
                </div>
                <div class="usr-meta-stack">
                  <div class="usr-line-1">
                    <span class="usr-title">{{ song.title }}</span>
                  </div>
                  <div class="usr-line-2">
                    <span
                      v-if="resolveSongQualityBadge(song) || song.isVip"
                      class="usr-badges"
                    >
                      <span
                        v-if="resolveSongQualityBadge(song)"
                        class="usr-badge-quality"
                        :class="`tier-${resolveSongQualityBadge(song)!.tier}`"
                      >
                        {{ resolveSongQualityBadge(song)!.label }}
                      </span>
                      <span v-if="song.isVip" class="usr-badge-vip">VIP</span>
                    </span>
                    <span class="usr-subtext">
                      <button
                        type="button"
                        class="usr-artist inline-meta-jump"
                        @click.stop="store.navigateToSongArtist(song)"
                      >
                        {{ song.artist }}
                      </button>
                      <template v-if="song.album">
                        <span class="usr-sep"> · </span>
                        <button
                          type="button"
                          class="usr-album inline-meta-jump"
                          @click.stop="store.navigateToSongAlbum(song)"
                        >
                          {{ song.album }}
                        </button>
                      </template>
                    </span>
                  </div>
                </div>
                <div class="usr-right-zone">
                  <div class="usr-hover-actions" @click.stop>
                    <button
                      type="button"
                      class="usr-act-btn"
                      data-tip="下一首播放"
                      @click="store.insertPlayNext(song)"
                    >
                      <PlumpIcon name="plus" :size="13" />
                    </button>
                    <button
                      type="button"
                      class="usr-act-btn"
                      :class="{ liked: store.isSongLiked(song) }"
                      data-tip="喜欢"
                      @click="store.toggleFavoriteSong(song)"
                    >
                      <PlumpIcon
                        :name="store.isSongLiked(song) ? 'heart-filled' : 'heart'"
                        :size="13"
                      />
                    </button>
                  </div>
                  <span class="usr-duration">{{ formatDuration(song) }}</span>
                </div>
              </div>
            </div>
          </transition>
        </section>

        <!-- 右栏：实时多维热搜榜 -->
        <section class="sl-panel sl-hot-boards-panel">
          <div class="sl-panel-head">
            <div class="sl-board-tabs">
              <template v-if="visibleHotBoards.length > 0">
                <button
                  v-for="(board, bIdx) in visibleHotBoards"
                  :key="board.name"
                  type="button"
                  class="sl-board-tab"
                  :class="{ active: activeBoardIndex === bIdx }"
                  @click="activeBoardIndex = bIdx"
                >
                  {{ board.name }}
                </button>
              </template>
              <h3 v-else class="sl-panel-title">热搜词榜</h3>
            </div>
          </div>

          <transition name="tag-content-morph" mode="out-in">
            <div
              :key="`${activeBoardIndex}_${currentBoardEntries[0]?.keyword || 'board'}`"
              class="sl-board-entries"
            >
              <button
                v-for="(entry, eIdx) in currentBoardEntries.slice(0, 18)"
                :key="`${entry.keyword}_${eIdx}`"
                type="button"
                class="sl-board-entry"
                @click="submitSearch(entry.keyword)"
              >
                <span class="sbe-rank" :class="{ top: eIdx < 3 }">
                  {{ String(eIdx + 1).padStart(2, "0") }}
                </span>
                <span class="sbe-keyword">{{ entry.keyword }}</span>
                <span v-if="getCleanEntryReason(entry)" class="sbe-reason">
                  {{ getCleanEntryReason(entry) }}
                </span>
                <span v-else-if="eIdx < 3 || entry.iconType === 1" class="sbe-hot-dot">
                  热
                </span>
              </button>
            </div>
          </transition>
        </section>
      </div>
    </div>

    <!-- ==================== 视图二：全量无上限搜索结果页（综合 / 单曲 / 专辑 / 歌手 / 歌单） ==================== -->
    <div v-else key="search-results" class="search-results-canvas">
      <!-- 五维结果导航栏（严格顺序：综合、单曲、专辑、歌手、歌单） -->
      <div class="sr-tabs-bar" role="tablist" aria-label="搜索结果分类">
        <div class="sr-tabs-group">
          <button
            v-for="tab in resultCategoryTabs"
            :key="tab.id"
            type="button"
            role="tab"
            class="sr-tab-btn"
            :aria-selected="store.searchCategoryTab === tab.id"
            :class="{ active: store.searchCategoryTab === tab.id }"
            @click="store.searchCategoryTab = tab.id"
          >
            <span>{{ tab.label }}</span>
            <em v-if="tab.count !== undefined && tab.count > 0" class="sr-tab-count">
              {{ tab.count }}
            </em>
          </button>
        </div>

        <button
          v-if="
            (store.searchCategoryTab === 'songs' ||
              store.searchCategoryTab === 'all') &&
            store.searchResultsSongs.length > 0
          "
          type="button"
          class="sl-play-all-pill"
          data-tip="播放全部搜索结果单曲"
          @click="
            store.playSongFromList(
              store.searchResultsSongs[0],
              store.searchResultsSongs,
              `搜索：${store.searchKeyword}`
            )
          "
        >
          <PlumpIcon name="play" :size="15" color="#ffffff" />
          <span>播放全部</span>
        </button>
      </div>

      <transition name="tag-content-morph" mode="out-in">
      <div v-if="store.isSearching" key="searching" class="sr-loading-state">
        正在检索「{{ store.searchKeyword }}」的全部内容...
      </div>

      <!-- 1. 综合 Tab：按搜索意图动态排序（歌手意图：歌手置顶 -> 单曲双列 -> 专辑 -> 歌单；歌曲意图：单曲双列置顶 -> 歌手 -> 歌单 -> 专辑） -->
      <div
        v-else-if="store.searchCategoryTab === 'all'"
        key="tab-all"
        class="search-comprehensive-grid"
      >
        <template v-for="sec in orderedAllSections" :key="sec">
          <!-- 歌手区块 -->
          <section v-if="sec === 'artists' && store.searchResultsArtists.length > 0">
            <div class="sr-sec-head">
              <h4 class="sr-sec-title">歌手</h4>
              <button
                type="button"
                class="sr-more-link"
                @click="store.searchCategoryTab = 'artists'"
              >
                全部 {{ store.searchTotals.artists }} 位 ›
              </button>
            </div>
            <div class="artist-cards-grid">
              <div
                v-for="art in store.searchResultsArtists.slice(0, 6)"
                :key="art.artistId"
                class="plump-artist-card"
                @click="
                  store.openArtistDetail(art.artistId, art.name, art.avatarUrl)
                "
              >
                <img
                  v-if="art.avatarUrl"
                  :src="art.avatarUrl"
                  class="pac-avatar"
                  loading="lazy"
                  decoding="async"
                  alt=""
                />
                <div v-else class="pac-avatar fallback">
                  {{ art.name.slice(0, 1) }}
                </div>
                <div class="pac-info">
                  <div class="pac-name">{{ art.name }}</div>
                  <div class="pac-sub">{{ formatArtistStats(art) }}</div>
                </div>
                <button
                  type="button"
                  class="tonal-pill-btn sm"
                  @click.stop="store.toggleFollowArtist(art)"
                >
                  {{ art.isFollowed ? "已关注" : "关注" }}
                </button>
              </div>
            </div>
          </section>

          <!-- 单曲区块（2 列双行统一格式） -->
          <section v-else-if="sec === 'songs' && store.searchResultsSongs.length > 0">
            <div class="sr-sec-head">
              <h4 class="sr-sec-title">单曲</h4>
              <button
                type="button"
                class="sr-more-link"
                @click="store.searchCategoryTab = 'songs'"
              >
                全部 {{ store.searchTotals.songs }} 首 ›
              </button>
            </div>
            <div class="frameless-songs-grid-2col">
              <div
                v-for="(song, idx) in store.searchResultsSongs.slice(0, 12)"
                :key="getSongKey(song, idx)"
                class="unified-song-row"
                :class="{
                  active: store.currentSong?.hash === song.hash,
                }"
                @click="
                  store.playSongFromList(
                    song,
                    store.searchResultsSongs,
                    `搜索：${store.searchKeyword}`
                  )
                "
                @contextmenu.prevent="
                  store.openSongContextMenu(
                    $event,
                    song,
                    store.searchResultsSongs
                  )
                "
              >
                <div class="usr-index-slot">
                  <span
                    v-if="store.currentSong?.hash === song.hash"
                    class="mini-eq-bars"
                    :class="{ paused: !store.isPlaying }"
                  >
                    <i></i><i></i><i></i>
                  </span>
                  <span v-else>{{ String(idx + 1).padStart(2, "0") }}</span>
                </div>
                <div class="usr-cover-wrap">
                  <img
                    v-if="song.coverUrl"
                    :src="song.coverUrl"
                    loading="lazy"
                    decoding="async"
                    alt=""
                  />
                  <div v-else class="usr-cover-fallback">
                    <PlumpIcon name="disc" :size="18" />
                  </div>
                </div>
                <div class="usr-meta-stack">
                  <div class="usr-line-1">
                    <span class="usr-title">{{ song.title }}</span>
                  </div>
                  <div class="usr-line-2">
                    <span
                      v-if="resolveSongQualityBadge(song) || song.isVip"
                      class="usr-badges"
                    >
                      <span
                        v-if="resolveSongQualityBadge(song)"
                        class="usr-badge-quality"
                        :class="`tier-${resolveSongQualityBadge(song)!.tier}`"
                      >
                        {{ resolveSongQualityBadge(song)!.label }}
                      </span>
                      <span v-if="song.isVip" class="usr-badge-vip">VIP</span>
                    </span>
                    <span class="usr-subtext">
                      <button
                        type="button"
                        class="usr-artist inline-meta-jump"
                        @click.stop="store.navigateToSongArtist(song)"
                      >
                        {{ song.artist }}
                      </button>
                      <template v-if="song.album">
                        <span class="usr-sep"> · </span>
                        <button
                          type="button"
                          class="usr-album inline-meta-jump"
                          @click.stop="store.navigateToSongAlbum(song)"
                        >
                          {{ song.album }}
                        </button>
                      </template>
                    </span>
                  </div>
                </div>
                <div class="usr-right-zone">
                  <div class="usr-hover-actions" @click.stop>
                    <button
                      type="button"
                      class="usr-act-btn"
                      data-tip="下一首播放"
                      @click="store.insertPlayNext(song)"
                    >
                      <PlumpIcon name="plus" :size="13" />
                    </button>
                    <button
                      type="button"
                      class="usr-act-btn"
                      :class="{ liked: store.isSongLiked(song) }"
                      data-tip="喜欢"
                      @click="store.toggleFavoriteSong(song)"
                    >
                      <PlumpIcon
                        :name="store.isSongLiked(song) ? 'heart-filled' : 'heart'"
                        :size="13"
                      />
                    </button>
                    <button
                      type="button"
                      class="usr-act-btn"
                      data-tip="添加到歌单"
                      @click="store.addToPlaylistModalSong = song"
                    >
                      <PlumpIcon name="playlist-add" :size="13" />
                    </button>
                  </div>
                  <span class="usr-duration">{{ formatDuration(song) }}</span>
                </div>
              </div>
            </div>
          </section>

          <!-- 专辑区块 -->
          <section v-else-if="sec === 'albums' && store.searchResultsAlbums.length > 0">
            <div class="sr-sec-head">
              <h4 class="sr-sec-title">专辑</h4>
              <button
                type="button"
                class="sr-more-link"
                @click="store.searchCategoryTab = 'albums'"
              >
                全部 {{ store.searchTotals.albums }} 张 ›
              </button>
            </div>
            <div class="album-cards-grid">
              <div
                v-for="alb in store.searchResultsAlbums.slice(0, 8)"
                :key="alb.albumId"
                class="plump-album-card"
                @click="
                  store.openAlbumDetail(
                    alb.albumId,
                    alb.albumName,
                    alb.artistName,
                    alb.coverUrl
                  )
                "
              >
                <img
                  v-if="alb.coverUrl"
                  :src="alb.coverUrl"
                  class="palb-cover"
                  loading="lazy"
                  decoding="async"
                  alt=""
                />
                <div class="palb-info">
                  <div class="palb-name">{{ alb.albumName }}</div>
                  <div class="palb-sub">
                    {{ alb.artistName }}
                    <template v-if="alb.publishDate">
                      · {{ alb.publishDate }}
                    </template>
                  </div>
                </div>
              </div>
            </div>
          </section>

          <!-- 歌单区块 -->
          <section v-else-if="sec === 'playlists' && store.searchResultsPlaylists.length > 0">
            <div class="sr-sec-head">
              <h4 class="sr-sec-title">歌单</h4>
              <button
                type="button"
                class="sr-more-link"
                @click="store.searchCategoryTab = 'playlists'"
              >
                全部 {{ store.searchTotals.playlists }} 个 ›
              </button>
            </div>
            <div class="album-cards-grid">
              <div
                v-for="pl in store.searchResultsPlaylists.slice(0, 8)"
                :key="pl.id"
                class="plump-album-card"
                @click="openSearchedPlaylist(pl)"
              >
                <img
                  v-if="pl.coverUrl"
                  :src="pl.coverUrl"
                  class="palb-cover"
                  loading="lazy"
                  decoding="async"
                  alt=""
                />
                <div class="palb-info">
                  <div class="palb-name">{{ pl.title }}</div>
                  <div class="palb-sub">
                    {{ pl.trackCount }} 首
                    <template v-if="pl.creatorName || pl.creator">
                      · {{ pl.creatorName || pl.creator }}
                    </template>
                  </div>
                </div>
                <button
                  type="button"
                  class="tonal-pill-btn sm"
                  @click.stop="store.toggleCollectPlaylist(pl)"
                >
                  {{ store.isPlaylistCollected(pl) ? "已收藏" : "收藏" }}
                </button>
              </div>
            </div>
          </section>
        </template>

        <div
          v-if="
            store.searchResultsSongs.length === 0 &&
            store.searchResultsAlbums.length === 0 &&
            store.searchResultsArtists.length === 0 &&
            store.searchResultsPlaylists.length === 0
          "
          class="sr-empty-state"
        >
          未找到与「{{ store.searchKeyword }}」匹配的结果
        </div>
      </div>

      <!-- 2. 单曲全量无上限列表（统一 2 列双行网格 + 折叠版本支持） -->
      <div v-else-if="store.searchCategoryTab === 'songs'" key="tab-songs">
        <div
          v-if="store.searchResultsSongs.length === 0"
          class="sr-empty-state"
        >
          暂无匹配单曲
        </div>
        <div v-else ref="searchSongsGridRef" class="frameless-songs-grid-2col">
          <template
            v-for="(song, idx) in store.searchResultsSongs"
            :key="getSongKey(song, idx)"
          >
            <div
              :data-locate-song-key="song.hash || song.id"
              class="unified-song-row"
              :class="[
                { active: store.currentSong?.hash === song.hash },
                searchSongsStagger.getStaggerClass(idx),
              ]"
              :style="searchSongsStagger.getStaggerStyle(idx)"
              @click="
                store.playSongFromList(
                  song,
                  store.searchResultsSongs,
                  `搜索：${store.searchKeyword}`
                )
              "
              @contextmenu.prevent="
                store.openSongContextMenu(
                  $event,
                  song,
                  store.searchResultsSongs
                )
              "
            >
              <div class="usr-index-slot">
                <span
                  v-if="store.currentSong?.hash === song.hash"
                  class="mini-eq-bars"
                  :class="{ paused: !store.isPlaying }"
                >
                  <i></i><i></i><i></i>
                </span>
                <span v-else>{{ String(idx + 1).padStart(2, "0") }}</span>
              </div>

              <div class="usr-cover-wrap">
                <img
                  v-if="song.coverUrl"
                  :src="song.coverUrl"
                  loading="lazy"
                  decoding="async"
                  alt=""
                />
                <div v-else class="usr-cover-fallback">
                  <PlumpIcon name="disc" :size="16" />
                </div>
              </div>

              <div class="usr-meta-stack">
                <div class="usr-line-1">
                  <span class="usr-title">{{ song.title }}</span>
                </div>
                <div class="usr-line-2">
                  <span
                    v-if="resolveSongQualityBadge(song) || song.isVip"
                    class="usr-badges"
                  >
                    <span
                      v-if="resolveSongQualityBadge(song)"
                      class="usr-badge-quality"
                      :class="`tier-${resolveSongQualityBadge(song)!.tier}`"
                    >
                      {{ resolveSongQualityBadge(song)!.label }}
                    </span>
                    <span v-if="song.isVip" class="usr-badge-vip">VIP</span>
                  </span>
                  <span class="usr-subtext">
                    <button
                      type="button"
                      class="usr-artist inline-meta-jump"
                      @click.stop="store.navigateToSongArtist(song)"
                    >
                      {{ song.artist }}
                    </button>
                    <template v-if="song.album">
                      <span class="usr-sep"> · </span>
                      <button
                        type="button"
                        class="usr-album inline-meta-jump"
                        @click.stop="store.navigateToSongAlbum(song)"
                      >
                        {{ song.album }}
                      </button>
                    </template>
                  </span>
                </div>
              </div>

              <div class="usr-right-zone">
                <div class="usr-hover-actions" @click.stop>
                  <button
                    v-if="song.foldedVersions && song.foldedVersions.length > 0"
                    type="button"
                    class="sr-fold-ver-btn"
                    :class="{ expanded: expandedFoldKeys.has(getFoldKey(song)) }"
                    data-tip="展开/收起其他版本"
                    @click.stop="toggleFoldedVersions(song)"
                  >
                    <span>
                      {{
                        expandedFoldKeys.has(getFoldKey(song))
                          ? "收起"
                          : `${song.foldedVersions.length}版本`
                      }}
                    </span>
                    <PlumpIcon name="chevron-down" :size="10" class="sfvb-arrow" />
                  </button>
                  <button
                    type="button"
                    class="usr-act-btn"
                    data-tip="下一首播放"
                    @click="store.insertPlayNext(song)"
                  >
                    <PlumpIcon name="plus" :size="13" />
                  </button>
                  <button
                    type="button"
                    class="usr-act-btn"
                    :class="{ liked: store.isSongLiked(song) }"
                    data-tip="喜欢"
                    @click="store.toggleFavoriteSong(song)"
                  >
                    <PlumpIcon
                      :name="store.isSongLiked(song) ? 'heart-filled' : 'heart'"
                      :size="13"
                    />
                  </button>
                  <button
                    type="button"
                    class="usr-act-btn"
                    data-tip="添加到歌单"
                    @click="store.addToPlaylistModalSong = song"
                  >
                    <PlumpIcon name="playlist-add" :size="13" />
                  </button>
                </div>
                <span class="usr-duration">{{ formatDuration(song) }}</span>
              </div>
            </div>

            <!-- 展开的官方折叠版本列表（横跨双列，保持统一双行契约） -->
            <div
              v-if="
                song.foldedVersions &&
                song.foldedVersions.length > 0 &&
                expandedFoldKeys.has(getFoldKey(song))
              "
              class="sr-folded-children-fullrow"
            >
              <div class="frameless-songs-grid-2col">
                <div
                  v-for="(ver, vIdx) in song.foldedVersions"
                  :key="getSongKey(ver, vIdx)"
                  :data-locate-song-key="ver.hash || ver.id"
                  class="unified-song-row sr-folded-row"
                  :class="{
                    active: store.currentSong?.hash === ver.hash,
                  }"
                  @click="
                    store.playSongFromList(
                      ver,
                      song.foldedVersions,
                      `${song.title} · 版本合集`
                    )
                  "
                  @contextmenu.prevent="
                    store.openSongContextMenu($event, ver, song.foldedVersions)
                  "
                >
                  <div class="usr-index-slot">
                    <span
                      v-if="store.currentSong?.hash === ver.hash"
                      class="mini-eq-bars"
                      :class="{ paused: !store.isPlaying }"
                    >
                      <i></i><i></i><i></i>
                    </span>
                    <span v-else class="sfr-branch">↳</span>
                  </div>
                  <div class="usr-cover-wrap">
                    <img
                      v-if="ver.coverUrl || song.coverUrl"
                      :src="ver.coverUrl || song.coverUrl"
                      loading="lazy"
                      decoding="async"
                      alt=""
                    />
                    <div v-else class="usr-cover-fallback">
                      <PlumpIcon name="disc" :size="15" />
                    </div>
                  </div>
                  <div class="usr-meta-stack">
                    <div class="usr-line-1">
                      <span class="usr-title">{{ ver.title }}</span>
                    </div>
                    <div class="usr-line-2">
                      <span
                        v-if="resolveSongQualityBadge(ver) || ver.isVip"
                        class="usr-badges"
                      >
                        <span
                          v-if="resolveSongQualityBadge(ver)"
                          class="usr-badge-quality"
                          :class="`tier-${resolveSongQualityBadge(ver)!.tier}`"
                        >
                          {{ resolveSongQualityBadge(ver)!.label }}
                        </span>
                        <span v-if="ver.isVip" class="usr-badge-vip">VIP</span>
                      </span>
                      <span class="usr-subtext">
                        <button
                          type="button"
                          class="usr-artist inline-meta-jump"
                          @click.stop="store.navigateToSongArtist(ver)"
                        >
                          {{ ver.artist }}
                        </button>
                        <template v-if="ver.album">
                          <span class="usr-sep"> · </span>
                          <button
                            type="button"
                            class="usr-album inline-meta-jump"
                            @click.stop="store.navigateToSongAlbum(ver)"
                          >
                            {{ ver.album }}
                          </button>
                        </template>
                      </span>
                    </div>
                  </div>
                  <div class="usr-right-zone">
                    <div class="usr-hover-actions" @click.stop>
                      <button
                        type="button"
                        class="usr-act-btn"
                        data-tip="下一首播放"
                        @click="store.insertPlayNext(ver)"
                      >
                        <PlumpIcon name="plus" :size="13" />
                      </button>
                      <button
                        type="button"
                        class="usr-act-btn"
                        :class="{ liked: store.isSongLiked(ver) }"
                        data-tip="喜欢"
                        @click="store.toggleFavoriteSong(ver)"
                      >
                        <PlumpIcon
                          :name="store.isSongLiked(ver) ? 'heart-filled' : 'heart'"
                          :size="13"
                        />
                      </button>
                      <button
                        type="button"
                        class="usr-act-btn"
                        data-tip="添加到歌单"
                        @click="store.addToPlaylistModalSong = ver"
                      >
                        <PlumpIcon name="playlist-add" :size="13" />
                      </button>
                    </div>
                    <span class="usr-duration">{{ formatDuration(ver) }}</span>
                  </div>
                </div>
              </div>
            </div>
          </template>
        </div>
      </div>

      <!-- 3. 专辑全量无上限列表 -->
      <div v-else-if="store.searchCategoryTab === 'albums'" key="tab-albums">
        <div
          v-if="store.searchResultsAlbums.length === 0"
          class="sr-empty-state"
        >
          暂无匹配专辑
        </div>
        <div v-else class="album-cards-grid">
          <div
            v-for="alb in store.searchResultsAlbums"
            :key="alb.albumId"
            class="plump-album-card"
            @click="
              store.openAlbumDetail(
                alb.albumId,
                alb.albumName,
                alb.artistName,
                alb.coverUrl
              )
            "
          >
            <img
              v-if="alb.coverUrl"
              :src="alb.coverUrl"
              class="palb-cover"
              loading="lazy"
              decoding="async"
              alt=""
            />
            <div class="palb-info">
              <div class="palb-name">{{ alb.albumName }}</div>
              <div class="palb-sub">
                {{ alb.artistName }}
                <template v-if="alb.publishDate">
                  · {{ alb.publishDate }}
                </template>
                <template v-if="alb.songCount > 0">
                  · {{ alb.songCount }} 首
                </template>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 4. 歌手全量无上限列表 -->
      <div v-else-if="store.searchCategoryTab === 'artists'" key="tab-artists">
        <div
          v-if="store.searchResultsArtists.length === 0"
          class="sr-empty-state"
        >
          暂无匹配歌手
        </div>
        <div v-else class="artist-cards-grid">
          <div
            v-for="art in store.searchResultsArtists"
            :key="art.artistId"
            class="plump-artist-card"
            @click="
              store.openArtistDetail(art.artistId, art.name, art.avatarUrl)
            "
          >
            <img
              v-if="art.avatarUrl"
              :src="art.avatarUrl"
              class="pac-avatar"
              loading="lazy"
              decoding="async"
              alt=""
            />
            <div v-else class="pac-avatar fallback">
              {{ art.name.slice(0, 1) }}
            </div>
            <div class="pac-info">
              <div class="pac-name">{{ art.name }}</div>
              <div class="pac-sub">{{ formatArtistStats(art) }}</div>
            </div>
            <button
              type="button"
              class="tonal-pill-btn sm"
              @click.stop="store.toggleFollowArtist(art)"
            >
              {{ art.isFollowed ? "已关注" : "关注" }}
            </button>
          </div>
        </div>
      </div>

      <!-- 5. 歌单全量无上限列表 -->
      <div v-else-if="store.searchCategoryTab === 'playlists'" key="tab-playlists">
        <div
          v-if="store.searchResultsPlaylists.length === 0"
          class="sr-empty-state"
        >
          暂无匹配歌单
        </div>
        <div v-else class="album-cards-grid">
          <div
            v-for="pl in store.searchResultsPlaylists"
            :key="pl.id"
            class="plump-album-card"
            @click="openSearchedPlaylist(pl)"
          >
            <img
              v-if="pl.coverUrl"
              :src="pl.coverUrl"
              class="palb-cover"
              loading="lazy"
              decoding="async"
              alt=""
            />
            <div class="palb-info">
              <div class="palb-name">{{ pl.title }}</div>
              <div class="palb-sub">
                {{ pl.trackCount }} 首
                <template v-if="pl.creatorName || pl.creator">
                  · {{ pl.creatorName || pl.creator }}
                </template>
              </div>
            </div>
            <button
              type="button"
              class="tonal-pill-btn sm"
              @click.stop="store.toggleCollectPlaylist(pl)"
            >
              {{ store.isPlaylistCollected(pl) ? "已收藏" : "收藏" }}
            </button>
          </div>
        </div>
      </div>
      </transition>

      <!-- 底部自动触底无上限加载哨兵 -->
      <div
        v-if="store.searchCategoryTab !== 'all' && !store.isSearching"
        ref="infiniteSentinelRef"
        class="sr-infinite-sentinel"
      >
        <button
          v-if="currentTabHasMore"
          type="button"
          class="tonal-pill-btn sm"
          :disabled="currentTabLoadingMore"
          @click="triggerLoadMoreCurrentTab"
        >
          {{ currentTabLoadingMore ? "正在加载更多结果..." : "继续加载更多" }}
        </button>
        <span
          v-else-if="
            (store.searchCategoryTab === 'songs' &&
              store.searchResultsSongs.length > 0) ||
            (store.searchCategoryTab === 'albums' &&
              store.searchResultsAlbums.length > 0) ||
            (store.searchCategoryTab === 'artists' &&
              store.searchResultsArtists.length > 0) ||
            (store.searchCategoryTab === 'playlists' &&
              store.searchResultsPlaylists.length > 0)
          "
          class="sr-all-loaded-hint"
        >
          已加载全部结果
        </span>
      </div>
    </div>
    </transition>

    <FloatingLocateFab :songs="locateSongs" />
  </div>
</template>

<style scoped>
.search-hub-page {
  display: flex;
  flex-direction: column;
  gap: 20px;
  padding-bottom: 36px;
}

/* ==================== 顶部沉浸搜索框 ==================== */
.search-command-shell {
  position: relative;
  width: 100%;
  margin: 12px 0 0;
  display: flex;
  align-items: flex-start;
  justify-content: center;
}

.search-command-center {
  position: relative;
  max-width: 780px;
  width: 100%;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

@media (max-width: 920px) {
  .search-command-shell.is-results-mode .search-command-center {
    max-width: min(780px, calc(100% - 108px));
  }
}

.search-command-row {
  display: flex;
  align-items: center;
  width: 100%;
}

.search-back-landing-btn {
  position: absolute;
  left: 0;
  top: 3px;
  width: 44px;
  height: 44px;
  padding: 0;
  border-radius: 50%;
  border: none;
  background: var(--bg-card);
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.14);
  z-index: 5;
  transition: background 0.2s ease, transform 0.2s ease, color 0.2s ease;
}

.scheme-watercolor_light .search-back-landing-btn,
.scheme-light .search-back-landing-btn {
  background: rgba(255, 255, 255, 0.92);
  box-shadow: 0 6px 18px rgba(24, 26, 32, 0.06);
}

.search-back-landing-btn:hover {
  background: var(--bg-card-hover);
  color: var(--primary);
  transform: translateX(-2px);
}

.search-hero-form {
  flex: 1;
  min-width: 0;
  height: 50px;
  padding: 0 6px 0 18px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--bg-card);
  border: none;
  box-shadow: 0 12px 32px color-mix(in srgb, var(--primary) 12%, transparent);
  display: flex;
  align-items: center;
  gap: 10px;
  transition: box-shadow 0.22s ease, background 0.22s ease;
}

.scheme-watercolor_light .search-hero-form,
.scheme-light .search-hero-form {
  background: rgba(255, 255, 255, 0.94);
  box-shadow: 0 12px 30px color-mix(in srgb, var(--primary) 10%, transparent);
}

.search-hero-form.focused {
  box-shadow: 0 16px 38px color-mix(in srgb, var(--primary) 24%, transparent);
}

.shf-leading-icon {
  flex-shrink: 0;
}

.shf-input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: none;
  outline: none;
  background: transparent;
  font-size: var(--bm-fs-song-title, 15px);
  font-weight: var(--bm-fw-medium, 500);
  color: var(--text-primary);
}

.shf-input::-webkit-search-cancel-button {
  -webkit-appearance: none;
  appearance: none;
}

.shf-input::placeholder {
  color: var(--text-muted);
}

.shf-clear-btn {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  border: none;
  background: color-mix(in srgb, var(--text-primary) 8%, transparent);
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
}

.shf-clear-btn:hover {
  color: var(--text-primary);
  background: color-mix(in srgb, var(--text-primary) 14%, transparent);
}

.shf-submit-btn {
  height: 38px;
  padding: 0 20px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: var(--grad-kugou-primary, linear-gradient(135deg, var(--primary), var(--primary-light, #818cf8)));
  color: #ffffff;
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  flex-shrink: 0;
  transition: transform 0.18s ease, box-shadow 0.18s ease;
}

.shf-submit-btn:hover {
  transform: scale(1.02);
  box-shadow: 0 6px 16px color-mix(in srgb, var(--primary) 34%, transparent);
}

/* 实时搜索建议浮层 */
.shf-suggest-popover {
  position: absolute;
  top: 56px;
  left: 0;
  right: 0;
  padding: 8px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bg-elevated);
  backdrop-filter: blur(28px);
  border: none;
  box-shadow: 0 20px 48px rgba(0, 0, 0, 0.34);
  z-index: 80;
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 4px;
}

.scheme-watercolor_light .shf-suggest-popover,
.scheme-light .shf-suggest-popover {
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 18px 42px rgba(15, 23, 42, 0.12);
}

@media (max-width: 720px) {
  .shf-suggest-popover {
    grid-template-columns: 1fr;
  }
}

.shf-suggest-item {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 9px 12px;
  border-radius: var(--bm-r-md, 14px);
  border: none;
  background: transparent;
  color: var(--text-primary);
  font-size: var(--bm-fs-body, 14.5px);
  text-align: left;
  cursor: pointer;
  overflow: hidden;
}

.shf-suggest-item span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shf-suggest-item:hover {
  background: color-mix(in srgb, var(--primary) 12%, transparent);
  color: var(--primary);
}

/* 搜索历史条 */
.search-history-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  padding: 0 6px;
}

.shb-title {
  font-size: var(--bm-fs-meta, 13.5px);
  color: var(--text-muted);
  flex-shrink: 0;
}

.shb-pills {
  display: flex;
  align-items: center;
  gap: 7px;
  flex-wrap: wrap;
  flex: 1;
}

.shb-pill {
  display: inline-flex;
  align-items: center;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--bg-card);
  border: none;
  overflow: hidden;
}

.scheme-watercolor_light .shb-pill,
.scheme-light .shb-pill {
  background: rgba(255, 255, 255, 0.82);
}

.shb-pill-text {
  padding: 4px 8px 4px 12px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--bm-fs-meta, 13.5px);
  cursor: pointer;
}

.shb-pill-text:hover {
  color: var(--primary);
}

.shb-pill-del {
  padding: 4px 9px 4px 2px;
  border: none;
  background: transparent;
  color: var(--text-muted);
  font-size: var(--bm-fs-meta, 13.5px);
  cursor: pointer;
}

.shb-pill-del:hover {
  color: var(--coral);
}

.shb-clear-all {
  border: none;
  background: transparent;
  color: var(--text-muted);
  font-size: var(--bm-fs-caption, 12.5px);
  cursor: pointer;
}

.shb-clear-all:hover {
  color: var(--coral);
}

/* ==================== 独立搜索探索页 ==================== */
.search-landing-canvas {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.sl-panel {
  padding: 8px 4px;
  border-radius: var(--bm-r-xl, 24px);
  background: transparent;
  border: none;
  box-shadow: none;
}

.scheme-watercolor_light .sl-panel,
.scheme-light .sl-panel {
  background: transparent;
  box-shadow: none;
}

.sl-panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
  flex-wrap: wrap;
}

.sl-panel-title-wrap {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.sl-panel-title {
  font-size: 18px;
  font-weight: var(--bm-fw-heavy, 800);
  margin: 0;
  color: var(--text-primary);
}

.sl-region-tabs {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(in srgb, var(--text-primary) 6%, transparent);
}

.sl-region-pill {
  padding: 4px 12px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
  transition: all 0.18s ease;
}

.sl-region-pill.active {
  background: var(--primary);
  color: #ffffff;
}

.sl-head-actions {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.sl-hot-artists-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(126px, 1fr));
  gap: 12px;
}

.sl-hot-artist-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 12px 10px;
  border-radius: var(--bm-r-lg, 18px);
  border: none;
  background: transparent;
  color: var(--text-primary);
  cursor: pointer;
  transition: transform 0.24s cubic-bezier(0.22, 1.35, 0.36, 1), background 0.2s ease;
}

.scheme-watercolor_light .sl-hot-artist-card,
.scheme-light .sl-hot-artist-card {
  background: transparent;
}

.sl-hot-artist-card:hover {
  transform: translateY(-5px);
  background: var(--song-hover-mist, color-mix(in srgb, var(--primary) 8%, transparent));
}

.shac-avatar-box {
  position: relative;
  width: 68px;
  height: 68px;
}

.shac-avatar {
  width: 68px;
  height: 68px;
  border-radius: 50%;
  object-fit: cover;
  box-shadow: 0 8px 18px rgba(0, 0, 0, 0.22);
}

.shac-avatar.fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 22px;
  font-weight: var(--bm-fw-heavy, 800);
  color: #ffffff;
  background: var(--grad-kugou-primary, linear-gradient(135deg, var(--primary), var(--primary-light, #818cf8)));
}

.shac-rank-badge {
  position: absolute;
  right: -2px;
  bottom: -2px;
  width: 21px;
  height: 21px;
  border-radius: 50%;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: #ffffff;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 3px 8px rgba(0, 0, 0, 0.28);
}

.shac-rank-badge.top-1 {
  background: linear-gradient(135deg, #ff5a79, #ff8a5b);
}
.shac-rank-badge.top-2 {
  background: linear-gradient(135deg, #f59e0b, #fbbf24);
}
.shac-rank-badge.top-3 {
  background: var(--grad-kugou-primary, linear-gradient(135deg, var(--primary), var(--primary-light, #818cf8)));
}

.shac-name {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-bold, 700);
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shac-meta {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-muted);
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 双栏热搜矩阵 */
.sl-dual-bento {
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(300px, 1fr);
  gap: 18px;
  align-items: stretch;
}

.sl-hot-songs-panel,
.sl-hot-boards-panel {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.sl-hot-songs-panel {
  container-type: inline-size;
}

.sl-board-entries {
  flex: 1;
  align-content: space-between;
}

@media (max-width: 1180px) {
  .sl-dual-bento {
    grid-template-columns: 1fr;
  }
}

.sl-play-all-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 15px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: var(--grad-kugou-primary, linear-gradient(135deg, var(--primary), var(--primary-light, #818cf8)));
  color: #ffffff;
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  transition: transform 0.18s ease, box-shadow 0.18s ease;
}

.sl-play-all-pill:hover {
  transform: translateY(-1px);
  box-shadow: 0 6px 16px color-mix(in srgb, var(--primary) 32%, transparent);
}

/* ==================== 全局自适应单/双列统一歌曲列表（由 skyline-components.css 与 adaptiveSongGrid.ts 统一驱动） ==================== */
/* .unified-song-row 统一样式已由 src/styles/skyline-components.css 提供 */

/* 右侧热搜词榜 */
.sl-board-tabs {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.sl-board-tab {
  padding: 5px 12px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  transition: color 0.18s ease, background 0.18s ease;
}

.sl-board-tab:hover {
  color: var(--text-primary);
}

.sl-board-tab.active {
  background: var(--primary);
  color: #ffffff;
}

.sl-board-entries {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 4px 10px;
}

@media (max-width: 520px) {
  .sl-board-entries {
    grid-template-columns: 1fr;
  }
}

.sl-board-entry {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: var(--bm-r-md, 14px);
  border: none;
  background: transparent;
  color: var(--text-primary);
  text-align: left;
  cursor: pointer;
  min-width: 0;
  transition: background 0.16s ease, transform 0.16s ease;
}

.sl-board-entry:hover {
  background: color-mix(in srgb, var(--primary) 10%, transparent);
  transform: translateX(2px);
}

.sbe-rank {
  width: 18px;
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-heavy, 800);
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
  flex-shrink: 0;
}

.sbe-rank.top {
  color: var(--primary);
}

.sbe-keyword {
  flex: 1;
  min-width: 0;
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-semibold, 600);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sbe-reason {
  font-size: var(--bm-fs-micro, 11px);
  padding: 1px 6px;
  border-radius: var(--bm-r-xs, 6px);
  background: color-mix(in srgb, var(--primary) 12%, transparent);
  color: var(--primary);
  max-width: 68px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex-shrink: 0;
}

.sbe-hot-dot {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-heavy, 800);
  padding: 1px 5px;
  border-radius: var(--bm-r-xs, 6px);
  background: color-mix(in srgb, var(--primary) 14%, transparent);
  color: var(--primary);
  flex-shrink: 0;
}

.sl-empty-hint {
  padding: 28px 0;
  text-align: center;
  color: var(--text-muted);
  font-size: var(--bm-fs-body, 14.5px);
}

/* ==================== 搜索结果页 ==================== */
.search-results-canvas {
  display: flex;
  flex-direction: column;
  gap: 16px;
  container-type: inline-size;
}

.search-comprehensive-grid {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.sr-tabs-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  padding-bottom: 4px;
  border: none;
}

.sr-tabs-group {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.sr-tab-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: transparent;
  color: var(--bm-text-2, var(--text-secondary));
  font-size: var(--bm-fs-song-title, 15px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  transition: all 0.2s ease;
}

.sr-tab-btn:hover {
  color: var(--bm-text-1, var(--text-primary));
  background: color-mix(in srgb, var(--text-primary) 6%, transparent);
}

.sr-tab-btn.active {
  background: var(--grad-kugou-primary, linear-gradient(135deg, var(--primary), var(--primary-light, #818cf8)));
  color: #ffffff;
  box-shadow: 0 6px 16px color-mix(in srgb, var(--primary) 28%, transparent);
}

.sr-tab-count {
  font-style: normal;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  opacity: 0.86;
}

.sr-sec-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.sr-sec-title {
  font-size: 17.5px;
  font-weight: var(--bm-fw-heavy, 800);
  margin: 0;
  color: var(--bm-text-1, var(--text-primary));
}

.sr-more-link {
  border: none;
  background: transparent;
  color: var(--bm-text-2, var(--text-secondary));
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
}

.sr-more-link:hover {
  color: var(--bm-accent-azure, var(--primary));
}

.sr-folded-children-fullrow {
  grid-column: 1 / -1;
  position: relative;
  margin: 2px 0 6px 26px;
  padding: 6px 10px 6px 14px;
  border-radius: var(--bm-r-md, 14px);
  background: color-mix(in srgb, var(--text-primary) 3.5%, transparent);
  animation: sr-fold-reveal 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.sr-folded-children-fullrow::before {
  content: "";
  position: absolute;
  left: 6px;
  top: 10px;
  bottom: 10px;
  width: 2px;
  border-radius: 2px;
  background: color-mix(in srgb, var(--bm-accent-azure, var(--primary)) 35%, transparent);
}

.sr-folded-row {
  height: 54px;
}

.sr-fold-ver-btn {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px 7px;
  border-radius: var(--bm-r-xs, 6px);
  border: none;
  background: color-mix(in srgb, var(--bm-accent-azure, var(--primary)) 12%, transparent);
  color: var(--bm-accent-azure, var(--primary));
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  flex-shrink: 0;
  white-space: nowrap;
  transition: background 0.2s ease, color 0.2s ease;
}

.sr-fold-ver-btn .sfvb-arrow {
  transition: transform 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.sr-fold-ver-btn.expanded .sfvb-arrow {
  transform: rotate(180deg);
}

.sr-fold-ver-btn:hover,
.sr-fold-ver-btn.expanded {
  background: var(--bm-accent-azure, var(--primary));
  color: #ffffff;
}

.sr-folded-children {
  position: relative;
  margin-left: 40px;
  padding: 6px 8px 6px 14px;
  border-radius: var(--bm-r-md, 14px);
  background: color-mix(in srgb, var(--text-primary) 3.5%, transparent);
  display: flex;
  flex-direction: column;
  gap: 2px;
  animation: sr-fold-reveal 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.sr-folded-children::before {
  content: "";
  position: absolute;
  left: 6px;
  top: 10px;
  bottom: 10px;
  width: 2px;
  border-radius: 2px;
  background: color-mix(in srgb, var(--primary) 35%, transparent);
}

.sr-folded-row {
  padding: 6px 10px;
  border-radius: var(--bm-r-sm, 10px);
}

@keyframes sr-fold-reveal {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.sfr-branch {
  color: var(--text-muted);
  font-size: var(--bm-fs-meta, 13.5px);
  flex-shrink: 0;
}

.sr-loading-state,
.sr-empty-state {
  padding: 48px 16px;
  text-align: center;
  color: var(--text-muted);
  font-size: var(--bm-fs-song-title, 15px);
}

.sr-infinite-sentinel {
  display: flex;
  justify-content: center;
  align-items: center;
  padding: 20px 0 8px;
}

.sr-all-loaded-hint {
  font-size: var(--bm-fs-meta, 13.5px);
  color: var(--text-muted);
}

.inline-meta-jump {
  background: none;
  border: none;
  padding: 1px 5px;
  margin: -1px -5px;
  border-radius: var(--bm-r-xs, 6px);
  color: inherit;
  font: inherit;
  cursor: pointer;
  text-decoration: none;
  transition: color 0.16s ease, background-color 0.16s ease;
}

.inline-meta-jump:hover {
  color: var(--bm-accent-azure, var(--primary));
  background: color-mix(in srgb, var(--primary) 11%, transparent);
}

/* ==================== 水彩晨曦浅色主题高对比度适配 ==================== */
[data-theme="watercolor_light"] .shf-input,
.scheme-watercolor_light .shf-input,
[data-theme="watercolor_light"] .sl-panel-title,
.scheme-watercolor_light .sl-panel-title,
[data-theme="watercolor_light"] .shac-name,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .shac-name,
.scheme-watercolor_light .shac-name,
.scheme-watercolor_light.light-variant-nordic_oat .shac-name,
[data-theme="watercolor_light"] .sl-board-entry,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sl-board-entry,
.scheme-watercolor_light .sl-board-entry,
.scheme-watercolor_light.light-variant-nordic_oat .sl-board-entry,
[data-theme="watercolor_light"] .sr-sec-title,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sr-sec-title,
.scheme-watercolor_light .sr-sec-title,
.scheme-watercolor_light.light-variant-nordic_oat .sr-sec-title {
  color: var(--text-primary);
}

[data-theme="watercolor_light"] .shf-input::placeholder,
.scheme-watercolor_light .shf-input::placeholder,
[data-theme="watercolor_light"] .shb-title,
.scheme-watercolor_light .shb-title,
[data-theme="watercolor_light"] .shac-meta,
.scheme-watercolor_light .shac-meta,
[data-theme="watercolor_light"] .sbe-rank,
.scheme-watercolor_light .sbe-rank,
[data-theme="watercolor_light"] .sr-empty-state,
.scheme-watercolor_light .sr-empty-state,
[data-theme="watercolor_light"] .sr-loading-state,
.scheme-watercolor_light .sr-loading-state,
[data-theme="watercolor_light"] .sr-all-loaded-hint,
.scheme-watercolor_light .sr-all-loaded-hint,
[data-theme="watercolor_light"] .sl-empty-hint,
.scheme-watercolor_light .sl-empty-hint {
  color: var(--text-secondary);
}

[data-theme="watercolor_light"] .shb-pill-text,
.scheme-watercolor_light .shb-pill-text,
[data-theme="watercolor_light"] .sl-region-pill:not(.active),
.scheme-watercolor_light .sl-region-pill:not(.active),
[data-theme="watercolor_light"] .sl-board-tab:not(.active),
.scheme-watercolor_light .sl-board-tab:not(.active),
[data-theme="watercolor_light"] .sr-tab-btn:not(.active),
.scheme-watercolor_light .sr-tab-btn:not(.active),
[data-theme="watercolor_light"] .sr-more-link,
.scheme-watercolor_light .sr-more-link {
  color: var(--text-primary);
}

[data-theme="watercolor_light"] .shb-pill,
.scheme-watercolor_light .shb-pill,
[data-theme="watercolor_light"] .sl-region-tabs,
.scheme-watercolor_light .sl-region-tabs {
  background: rgba(24, 26, 32, 0.055);
}

[data-theme="watercolor_light"] .search-hero-form,
.scheme-watercolor_light .search-hero-form {
  background: rgba(255, 255, 255, 0.94);
  border: none;
  box-shadow: 0 12px 30px color-mix(in srgb, var(--primary) 10%, transparent);
}

/* L2 晨雾燕麦实色纸感 */
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .search-back-landing-btn,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .search-hero-form {
  background: #fcf9f2;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow: 0 10px 24px -6px rgba(45, 38, 28, 0.09);
}
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .shb-pill,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sl-region-tabs {
  background: #e4ddd0;
}
</style>

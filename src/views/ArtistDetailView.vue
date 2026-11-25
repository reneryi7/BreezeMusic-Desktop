<script setup lang="ts">
import { ref, computed } from "vue";
import { useAppStore } from "../stores/appStore";
import { formatDuration, resolveSongQualityBadge } from "../api/kugou";
import { useViewportStagger } from "../composables/useViewportStagger";
import PlumpIcon from "../components/PlumpIcon.vue";
import FloatingLocateFab from "../components/FloatingLocateFab.vue";
import { useInfiniteScroll } from "../composables/useInfiniteScroll";

const store = useAppStore();

const songFilterKeyword = ref("");
const isBioExpanded = ref(false);
const artistSongsGridRef = ref<HTMLElement | null>(null);
const artistSongsSentinelRef = ref<HTMLElement | null>(null);
const artistAlbumsSentinelRef = ref<HTMLElement | null>(null);
const artistStagger = useViewportStagger(artistSongsGridRef, {
  defaultCols: 2,
  rowHeight: 64,
  estimatedChromeOffset: 170,
});

const { hasError: songsLoadMoreError, retryLoad: retryLoadMoreSongs } =
  useInfiniteScroll({
    sentinelRef: artistSongsSentinelRef,
    enabled: computed(
      () =>
        store.artistDetailSubTab === "songs" &&
        !songFilterKeyword.value.trim() &&
        !store.isLoadingArtist
    ),
    loading: computed(() => store.isLoadingMoreArtistSongs),
    hasMore: computed(() => store.artistSongsHasMore),
    onLoadMore: () => store.loadMoreArtistSongs(),
  });

const { hasError: albumsLoadMoreError, retryLoad: retryLoadMoreAlbums } =
  useInfiniteScroll({
    sentinelRef: artistAlbumsSentinelRef,
    enabled: computed(
      () => store.artistDetailSubTab === "albums" && !store.isLoadingArtist
    ),
    loading: computed(() => store.isLoadingMoreArtistAlbums),
    hasMore: computed(() => store.artistAlbumsHasMore),
    onLoadMore: () => store.loadMoreArtistAlbums(),
  });

const artist = computed(() => store.activeArtistDetail);

const filteredSongs = computed(() => {
  const kw = songFilterKeyword.value.trim().toLowerCase();
  if (!kw) return store.activeArtistSongs;
  return store.activeArtistSongs.filter(
    (s) =>
      s.title.toLowerCase().includes(kw) ||
      (s.album || "").toLowerCase().includes(kw)
  );
});

const displaySongCount = computed(() => {
  const c = artist.value?.songCount || 0;
  return Math.max(c, store.activeArtistSongs.length);
});

const displayAlbumCount = computed(() => {
  const c = artist.value?.albumCount || 0;
  return Math.max(c, store.activeArtistAlbums.length);
});

const displayMvCount = computed(() => artist.value?.mvCount || 0);

const formattedFans = computed(() => {
  const fans = artist.value?.fansCount || 0;
  if (!fans || fans <= 0) return "";
  if (fans >= 10000) {
    return `${(fans / 10000).toFixed(1).replace(/\.0$/, "")}w`;
  }
  return `${fans}`;
});

const bioParagraphs = computed(() => {
  const raw = (artist.value?.longIntro || artist.value?.intro || "").trim();
  if (!raw) {
    return [
      `${artist.value?.name || "该歌手"}，正版曲库收录其高解析度录音室专辑与热门代表作。`,
    ];
  }
  return raw
    .split(/\r?\n+/)
    .map((p) => p.trim())
    .filter((p) => p.length > 0);
});

const fullBioText = computed(() => bioParagraphs.value.join("\n"));

const canExpandBio = computed(() => fullBioText.value.length > 84);

function playAllSongs() {
  if (filteredSongs.value.length === 0) return;
  store.playSongFromList(
    filteredSongs.value[0],
    filteredSongs.value,
    `${artist.value?.name || "歌手"} · 热门作品`
  );
}

async function refreshArtistPage() {
  if (!artist.value) return;
  await store.openArtistDetail(
    artist.value.artistId || "0",
    artist.value.name,
    artist.value.avatarUrl
  );
  store.notify(`已刷新歌手「${artist.value.name}」主页`);
}
</script>

<template>
  <section class="standalone-detail-page artist-standalone-page">
    <div class="sdp-two-pane-layout">
      <!-- 左侧固定舱：高清头像 + 歌手名 + 核心数字统计 + 操作组 + 可折叠简介 -->
      <aside class="sdp-left-hero-card">
        <div class="artist-cover-showcase">
          <div v-tilt="{ maxDeg: 3.5 }" class="artist-avatar-ring">
            <img
              v-if="artist?.avatarUrl"
              :src="artist.avatarUrl"
              class="artist-hero-avatar"
              alt="artist"
            />
            <div v-else class="artist-hero-avatar fallback">
              <PlumpIcon name="user" :size="52" color="#fff" />
            </div>
          </div>
        </div>

        <h1 class="sdp-left-title">{{ artist?.name || "歌手主页" }}</h1>

        <div class="artist-stats-strip">
          <button
            type="button"
            class="artist-stat-chip"
            @click="store.artistDetailSubTab = 'songs'"
          >
            <strong>{{ displaySongCount }}</strong>
            <span>单曲</span>
          </button>
          <button
            type="button"
            class="artist-stat-chip"
            @click="store.artistDetailSubTab = 'albums'"
          >
            <strong>{{ displayAlbumCount }}</strong>
            <span>专辑</span>
          </button>
          <div v-if="displayMvCount > 0" class="artist-stat-chip static">
            <strong>{{ displayMvCount }}</strong>
            <span>MV</span>
          </div>
          <div v-if="formattedFans" class="artist-stat-chip static">
            <strong>{{ formattedFans }}</strong>
            <span>粉丝</span>
          </div>
        </div>

        <div class="sdp-left-actions">
          <button
            type="button"
            class="sdp-primary-play-btn"
            :disabled="filteredSongs.length === 0"
            @click="playAllSongs"
          >
            <PlumpIcon name="play" :size="16" color="#fff" />
            <span>播放热门</span>
          </button>

          <button
            v-if="artist"
            type="button"
            class="sdp-icon-fab"
            :class="{ active: artist.isFollowed }"
            :data-tip="artist.isFollowed ? '取消关注' : '关注歌手'"
            @click="store.toggleFollowArtist(artist)"
          >
            <PlumpIcon :name="artist.isFollowed ? 'check' : 'plus'" :size="16" />
          </button>

          <button
            type="button"
            class="sdp-icon-fab"
            :disabled="store.isLoadingArtist"
            data-tip="刷新歌手主页"
            @click="refreshArtistPage"
          >
            <PlumpIcon
              name="refresh"
              :size="16"
              :class="{ 'is-spinning': store.isLoadingArtist }"
            />
          </button>
        </div>

        <div class="artist-intro-box">
          <p
            class="artist-intro-text"
            :class="{ expanded: isBioExpanded, 'is-collapsible': canExpandBio }"
          >
            {{ fullBioText }}
          </p>
          <button
            v-if="canExpandBio"
            type="button"
            class="artist-intro-toggle"
            @click="isBioExpanded = !isBioExpanded"
          >
            <span>{{ isBioExpanded ? "收起简介" : "展开简介" }}</span>
            <PlumpIcon
              name="chevron-down"
              :size="12"
              class="toggle-arrow"
              :class="{ rotated: isBioExpanded }"
            />
          </button>
        </div>
      </aside>

      <!-- 右侧滚动流：顶部滑块标签 + 快捷筛选/排序 + 曲目表/专辑网格 -->
      <div class="sdp-right-tracks-pane">
        <div class="artist-right-toolbar">
          <div class="pill-tabs" role="tablist" aria-label="歌手详情导航">
            <button
              type="button"
              role="tab"
              class="pill-tab-btn"
              :class="{ active: store.artistDetailSubTab === 'songs' }"
              :aria-selected="store.artistDetailSubTab === 'songs'"
              @click="store.artistDetailSubTab = 'songs'"
            >
              热门单曲
            </button>
            <button
              type="button"
              role="tab"
              class="pill-tab-btn"
              :class="{ active: store.artistDetailSubTab === 'albums' }"
              :aria-selected="store.artistDetailSubTab === 'albums'"
              @click="store.artistDetailSubTab = 'albums'"
            >
              全部专辑
            </button>
          </div>

          <div v-if="store.artistDetailSubTab === 'songs'" class="artist-songs-tools">
            <div class="sdp-search-input-box">
              <PlumpIcon name="search" :size="14" />
              <input
                v-model="songFilterKeyword"
                type="text"
                placeholder="筛选歌曲或专辑..."
              />
              <button
                v-if="songFilterKeyword"
                type="button"
                class="clear-btn"
                @click="songFilterKeyword = ''"
              >
                <PlumpIcon name="close" :size="12" />
              </button>
            </div>

            <div class="sdp-sort-pills">
              <button
                type="button"
                class="sort-pill"
                :class="{ active: store.artistSongsSort === 'hot' }"
                @click="store.switchArtistSongSort('hot')"
              >
                最热
              </button>
              <button
                type="button"
                class="sort-pill"
                :class="{ active: store.artistSongsSort === 'new' }"
                @click="store.switchArtistSongSort('new')"
              >
                最新
              </button>
            </div>
          </div>

          <div v-else-if="store.artistDetailSubTab === 'albums'" class="artist-songs-tools">
            <div class="sdp-sort-pills">
              <button
                type="button"
                class="sort-pill"
                :class="{ active: store.artistAlbumsSort === 'new' }"
                @click="store.switchArtistAlbumSort('new')"
              >
                最新发行
              </button>
              <button
                type="button"
                class="sort-pill"
                :class="{ active: store.artistAlbumsSort === 'hot' }"
                @click="store.switchArtistAlbumSort('hot')"
              >
                最热专辑
              </button>
            </div>
          </div>
        </div>

        <transition name="tag-content-morph" mode="out-in">
          <!-- 1. 单曲列表 -->
          <div
            v-if="store.artistDetailSubTab === 'songs'"
            :key="`songs_${store.artistSongsSort}`"
            class="artist-tab-panel"
          >
            <div
              v-if="store.isLoadingArtist && store.activeArtistSongs.length === 0"
              class="sdp-empty-state"
            >
              正在加载歌手热门作品...
            </div>
            <div v-else-if="filteredSongs.length === 0" class="sdp-empty-state">
              暂无匹配单曲
            </div>

            <div v-else class="artist-songs-wrap">
              <div ref="artistSongsGridRef" class="frameless-songs-grid-2col">
                <div
                  v-for="(song, idx) in filteredSongs"
                  :key="song.hash + '_' + idx"
                  :data-locate-song-key="song.hash || song.id"
                  class="unified-song-row"
                  :class="[
                    { active: store.currentSong?.hash === song.hash },
                    artistStagger.getStaggerClass(idx),
                  ]"
                  :style="artistStagger.getStaggerStyle(idx)"
                  @click="store.playSongFromList(song, filteredSongs, `${artist?.name} · 热门作品`)"
                  @contextmenu.prevent="store.openSongContextMenu($event, song, filteredSongs)"
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
                      <PlumpIcon name="disc" :size="18" color="#fff" />
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
                          class="usr-artist usr-meta-link"
                          data-tip="查看歌手主页"
                          @click.stop="store.navigateToSongArtist(song)"
                        >
                          {{ song.artist }}
                        </button>
                        <template v-if="song.album">
                          <span class="usr-sep"> · </span>
                          <button
                            type="button"
                            class="usr-album usr-meta-link"
                            data-tip="查看专辑详情"
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
                        class="row-icon-btn"
                        :class="{ liked: store.isSongLiked(song) }"
                        :data-tip="store.isSongLiked(song) ? '取消喜欢' : '收藏到我喜欢'"
                        @click="store.toggleFavoriteSong(song)"
                      >
                        <PlumpIcon :name="store.isSongLiked(song) ? 'heart-filled' : 'heart'" :size="15" />
                      </button>
                      <button
                        type="button"
                        class="row-icon-btn"
                        data-tip="添加到歌单"
                        @click="store.addToPlaylistModalSong = song"
                      >
                        <PlumpIcon name="playlist-add" :size="15" />
                      </button>
                      <button
                        type="button"
                        class="row-icon-btn"
                        data-tip="查看评论"
                        @click="store.openCommentsModal(song)"
                      >
                        <PlumpIcon name="comment" :size="15" />
                      </button>
                    </div>
                    <span class="usr-duration">{{ formatDuration(song) }}</span>
                  </div>
                </div>
              </div>

              <div
                v-if="store.artistSongsHasMore && !songFilterKeyword"
                ref="artistSongsSentinelRef"
                class="sdp-load-more-row"
              >
                <span v-if="store.isLoadingMoreArtistSongs" class="sdp-auto-loading-hint">
                  <PlumpIcon name="refresh" :size="14" class="is-spinning" />
                  <span>正在静默加载更多单曲...</span>
                </span>
                <button
                  v-else-if="songsLoadMoreError"
                  type="button"
                  class="tonal-pill-btn"
                  @click="retryLoadMoreSongs()"
                >
                  网络波动，点击继续加载
                </button>
              </div>
            </div>
          </div>

          <!-- 2. 专辑网格视图 -->
          <div
            v-else-if="store.artistDetailSubTab === 'albums'"
            :key="`albums_${store.artistAlbumsSort}`"
            class="artist-tab-panel"
          >
            <div v-if="store.activeArtistAlbums.length === 0" class="sdp-empty-state">
              暂无收录专辑数据
            </div>
            <template v-else>
              <div class="artist-albums-grid">
                <div
                  v-for="alb in store.activeArtistAlbums"
                  :key="alb.albumId + '_' + alb.albumName"
                  class="artist-album-card"
                  @click="store.openAlbumDetail(alb.albumId, alb.albumName, alb.artistName || artist?.name || '', alb.coverUrl)"
                >
                  <div class="aac-cover-wrap">
                    <img
                      v-if="alb.coverUrl"
                      :src="alb.coverUrl"
                      class="aac-cover"
                      loading="lazy"
                      alt="album"
                    />
                    <div v-else class="aac-cover fallback">
                      <PlumpIcon name="disc" :size="36" color="#fff" />
                    </div>
                  </div>
                  <div class="aac-title">{{ alb.albumName }}</div>
                  <div class="aac-sub">
                    <span>{{ alb.publishDate || "数字专辑" }}</span>
                    <span v-if="alb.songCount"> · {{ alb.songCount }} 首</span>
                  </div>
                </div>
              </div>

              <div
                v-if="store.artistAlbumsHasMore"
                ref="artistAlbumsSentinelRef"
                class="sdp-load-more-row"
              >
                <span v-if="store.isLoadingMoreArtistAlbums" class="sdp-auto-loading-hint">
                  <PlumpIcon name="refresh" :size="14" class="is-spinning" />
                  <span>正在静默加载更多专辑...</span>
                </span>
                <button
                  v-else-if="albumsLoadMoreError"
                  type="button"
                  class="tonal-pill-btn"
                  @click="retryLoadMoreAlbums()"
                >
                  网络波动，点击继续加载
                </button>
              </div>
            </template>
          </div>
        </transition>
      </div>
    </div>

    <FloatingLocateFab
      :songs="store.artistDetailSubTab === 'songs' ? filteredSongs : []"
    />
  </section>
</template>

<style scoped>
.standalone-detail-page {
  padding: 12px 24px 36px;
}

.sdp-two-pane-layout {
  display: grid;
  grid-template-columns: minmax(280px, 308px) minmax(0, 1fr);
  gap: clamp(20px, 2.2vw, 32px);
  align-items: start;
}

.sdp-left-hero-card {
  position: sticky;
  top: 12px;
  max-height: calc(100vh - 180px);
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 14px;
  padding: 20px 18px 22px;
  border-radius: var(--bm-r-xl, 24px);
  background: color-mix(in srgb, var(--bg-glass) 78%, transparent);
  box-shadow: 0 16px 38px rgba(0, 0, 0, 0.24);
}

.sdp-left-hero-card::-webkit-scrollbar {
  width: 0;
  height: 0;
}

.scheme-watercolor_light .sdp-left-hero-card,
.scheme-light .sdp-left-hero-card {
  background: rgba(255, 255, 255, 0.76);
  box-shadow: 0 14px 34px rgba(24, 26, 32, 0.06);
}

.artist-cover-showcase {
  display: flex;
  justify-content: center;
  padding-top: 4px;
}

.artist-avatar-ring {
  width: 184px;
  height: 184px;
  border-radius: 50%;
  padding: 4px;
  background: var(--grad-kugou-primary);
  box-shadow: 0 18px 38px color-mix(in srgb, var(--primary) 24%, transparent);
}

.artist-hero-avatar {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  object-fit: cover;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--grad-kugou-primary);
}

.sdp-left-title {
  margin: 0;
  font-size: 24px;
  font-weight: var(--bm-fw-heavy, 800);
  line-height: 1.25;
  color: var(--text-primary);
  letter-spacing: -0.02em;
}

.artist-stats-strip {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-wrap: wrap;
  gap: 8px;
}

.artist-stat-chip {
  display: inline-flex;
  align-items: baseline;
  gap: 4px;
  padding: 5px 10px;
  border-radius: var(--bm-r-sm, 10px);
  border: none;
  background: color-mix(in srgb, var(--text-primary) 5%, transparent);
  color: var(--text-secondary);
  font-size: var(--bm-fs-caption, 12.5px);
  cursor: pointer;
  transition: background 0.2s ease, color 0.2s ease;
}

.artist-stat-chip.static {
  cursor: default;
}

.artist-stat-chip:not(.static):hover {
  background: color-mix(in srgb, var(--primary) 12%, transparent);
  color: var(--primary);
}

.artist-stat-chip strong {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.sdp-left-actions {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  width: 100%;
}

.sdp-primary-play-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  height: 40px;
  padding: 0 20px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: var(--grad-kugou-primary);
  color: #ffffff;
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  box-shadow: 0 10px 22px color-mix(in srgb, var(--primary) 30%, transparent);
  transition: transform 0.22s cubic-bezier(0.22, 1, 0.36, 1), box-shadow 0.22s ease;
}

.sdp-primary-play-btn:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 14px 26px color-mix(in srgb, var(--primary) 38%, transparent);
}

.sdp-primary-play-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.sdp-icon-fab {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  border: none;
  background: color-mix(in srgb, var(--text-primary) 7%, transparent);
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  transition: transform 0.22s cubic-bezier(0.22, 1, 0.36, 1), background 0.2s ease, color 0.2s ease;
}

.sdp-icon-fab:hover:not(:disabled) {
  transform: translateY(-2px);
  background: color-mix(in srgb, var(--primary) 12%, transparent);
  color: var(--primary);
}

.sdp-icon-fab.active {
  background: color-mix(in srgb, var(--primary) 16%, transparent);
  color: var(--primary);
}

.artist-intro-box {
  width: 100%;
  text-align: left;
  padding-top: 4px;
  interpolate-size: allow-keywords;
}

.artist-intro-text {
  margin: 0;
  font-size: var(--bm-fs-caption, 12.5px);
  line-height: 1.65;
  color: var(--text-muted);
  overflow: hidden;
  white-space: pre-line;
  height: auto;
  transition: height 0.3s cubic-bezier(0.22, 1, 0.36, 1);
}

.artist-intro-text.is-collapsible:not(.expanded) {
  height: 4.95em;
  mask-image: linear-gradient(to bottom, #000 58%, transparent 100%);
  -webkit-mask-image: linear-gradient(to bottom, #000 58%, transparent 100%);
}

.artist-intro-text.expanded {
  height: auto;
  mask-image: none;
  -webkit-mask-image: none;
}

.sdp-auto-loading-hint {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-muted);
  padding: 6px 14px;
}

.artist-intro-toggle {
  margin-top: 6px;
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--primary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
}

.artist-intro-toggle .toggle-arrow {
  transition: transform 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.artist-intro-toggle .toggle-arrow.rotated {
  transform: rotate(180deg);
}

/* ==================== 右侧滚动流 ==================== */
.sdp-right-tracks-pane {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
  container-type: inline-size;
}

.artist-right-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.pill-tabs {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(in srgb, var(--text-primary) 5%, transparent);
}

.pill-tab-btn {
  padding: 7px 16px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  transition: color 0.2s ease, background 0.2s ease;
}

.pill-tab-btn.active {
  background: var(--primary);
  color: #ffffff;
}

.artist-songs-tools {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.sdp-search-input-box {
  display: flex;
  align-items: center;
  gap: 7px;
  height: 34px;
  padding: 0 12px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(in srgb, var(--text-primary) 6%, transparent);
  color: var(--text-muted);
  width: 210px;
}

.sdp-search-input-box input {
  border: none;
  outline: none;
  background: transparent;
  color: var(--text-primary);
  font-size: var(--bm-fs-meta, 13.5px);
  width: 100%;
}

.sdp-search-input-box .clear-btn {
  border: none;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  display: flex;
  padding: 0;
}

.sdp-sort-pills {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px;
  border-radius: var(--bm-r-pill, 999px);
  background: color-mix(in srgb, var(--text-primary) 5%, transparent);
}

.sort-pill {
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  padding: 5px 12px;
  border-radius: var(--bm-r-pill, 999px);
  cursor: pointer;
}

.sort-pill.active {
  background: var(--primary);
  color: #ffffff;
}

.artist-tab-panel {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.artist-songs-wrap {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* 全局统一自适应单/双列曲目网格（阈值与空间 FLIP 由 skyline-components.css 与 adaptiveSongGrid.ts 统一驱动） */
.frameless-songs-grid-2col {
  gap: 6px 14px;
}

/* .unified-song-row 统一样式已由 src/styles/skyline-components.css 提供 */

.sdp-load-more-row {
  display: flex;
  justify-content: center;
  padding: 20px 0 8px;
}

.tonal-pill-btn {
  padding: 9px 22px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: color-mix(in srgb, var(--primary) 12%, transparent);
  color: var(--primary);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  transition: background 0.2s ease, transform 0.2s ease;
}

.tonal-pill-btn:hover:not(:disabled) {
  background: color-mix(in srgb, var(--primary) 20%, transparent);
  transform: translateY(-1px);
}

.artist-albums-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
  gap: 18px;
}

.artist-album-card {
  cursor: pointer;
  transition: transform 0.26s cubic-bezier(0.22, 1, 0.36, 1);
}

.artist-album-card:hover {
  transform: translateY(-5px);
}

.aac-cover-wrap {
  width: 100%;
  aspect-ratio: 1 / 1;
  border-radius: var(--bm-r-lg, 18px);
  overflow: hidden;
  margin-bottom: 10px;
  box-shadow: 0 12px 26px rgba(0, 0, 0, 0.16);
  transition: box-shadow 0.26s cubic-bezier(0.22, 1, 0.36, 1);
}

.artist-album-card:hover .aac-cover-wrap {
  box-shadow: 0 18px 34px color-mix(in srgb, var(--primary) 26%, transparent);
}

.aac-cover {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-elevated);
}

.aac-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.aac-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-muted);
  margin-top: 3px;
}

.sdp-empty-state {
  text-align: center;
  padding: 56px 20px;
  color: var(--text-muted);
}

.spin-anim {
  animation: artist-spin 0.9s linear infinite;
}

@keyframes artist-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 960px) {
  .sdp-two-pane-layout {
    grid-template-columns: 1fr;
  }
  .sdp-left-hero-card {
    position: static;
    max-height: none;
  }
}

/* D2 丝绒暗紫 */
[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .sdp-left-hero-card {
  background: color-mix(in srgb, var(--bg-glass) 82%, transparent);
  box-shadow: 0 18px 40px rgba(8, 4, 16, 0.34);
}

/* L1 棉纸水彩 */
[data-theme="watercolor_light"] .sdp-left-hero-card,
.scheme-watercolor_light .sdp-left-hero-card {
  background: rgba(255, 255, 255, 0.78);
  backdrop-filter: blur(22px) saturate(140%);
  -webkit-backdrop-filter: blur(22px) saturate(140%);
  box-shadow: 0 14px 34px rgba(24, 26, 32, 0.06);
}
[data-theme="watercolor_light"] .sdp-left-title,
.scheme-watercolor_light .sdp-left-title,
[data-theme="watercolor_light"] .artist-stat-chip strong,
.scheme-watercolor_light .artist-stat-chip strong,
[data-theme="watercolor_light"] .aac-title,
.scheme-watercolor_light .aac-title,
[data-theme="watercolor_light"] .usr-title,
.scheme-watercolor_light .usr-title {
  color: #181a20;
}
[data-theme="watercolor_light"] .artist-stat-chip span,
.scheme-watercolor_light .artist-stat-chip span,
[data-theme="watercolor_light"] .artist-intro-text,
.scheme-watercolor_light .artist-intro-text,
[data-theme="watercolor_light"] .aac-sub,
.scheme-watercolor_light .aac-sub,
[data-theme="watercolor_light"] .usr-subtext,
.scheme-watercolor_light .usr-subtext {
  color: var(--bm-text-2);
}
[data-theme="watercolor_light"] .artist-stat-chip,
.scheme-watercolor_light .artist-stat-chip,
[data-theme="watercolor_light"] .artist-intro-box,
.scheme-watercolor_light .artist-intro-box {
  background: rgba(24, 26, 32, 0.045);
}
[data-theme="watercolor_light"] .sdp-search-input-box,
.scheme-watercolor_light .sdp-search-input-box {
  background: rgba(255, 255, 255, 0.78);
  border: none;
  box-shadow: 0 4px 12px rgba(24, 26, 32, 0.04);
  color: var(--bm-text-2);
}
[data-theme="watercolor_light"] .pill-tab-btn,
.scheme-watercolor_light .pill-tab-btn {
  color: var(--bm-text-2);
}
[data-theme="watercolor_light"] .pill-tab-btn.active,
.scheme-watercolor_light .pill-tab-btn.active {
  color: #ffffff;
}
[data-theme="watercolor_light"] .sdp-search-input-box input,
.scheme-watercolor_light .sdp-search-input-box input {
  color: #181a20;
}
[data-theme="watercolor_light"] .sdp-search-input-box input::placeholder,
.scheme-watercolor_light .sdp-search-input-box input::placeholder,
[data-theme="watercolor_light"] .sdp-empty-state,
.scheme-watercolor_light .sdp-empty-state {
  color: #788096;
}

/* L2 晨雾燕麦实色纸感 */
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-left-hero-card {
  background: #fcf9f2;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow: 0 12px 28px -8px rgba(45, 38, 28, 0.09);
}
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-left-title,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .artist-stat-chip strong,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .aac-title,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-search-input-box input {
  color: #221f1a;
}
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .artist-stat-chip span,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .artist-intro-text,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .aac-sub,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .pill-tab-btn,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sort-pill {
  color: #625b50;
}
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-search-input-box input::placeholder,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-empty-state {
  color: #8f8678;
}
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .artist-stat-chip,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .artist-intro-box,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .pill-tabs,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-search-input-box,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-sort-pills {
  background: #e4ddd0;
  box-shadow: none;
}
</style>

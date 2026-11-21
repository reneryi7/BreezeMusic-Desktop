<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import { useFloatingModalTheme } from "../composables/useFloatingModalTheme";
import { hexWithAlpha } from "../utils/coverPalette";
import type { Playlist, Song } from "../types";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();
const {
  isLightSurface,
  accentPrimary,
  onAccentPrimary,
  modalSurfaceStyle,
} = useFloatingModalTheme("auto");

const filterQuery = ref("");
const isCreatingMode = ref(false);
const newPlaylistName = ref("");
const isSubmittingCreate = ref(false);
const pendingListId = ref<number | null>(null);
const collectedListIds = ref<Set<number>>(new Set());
const justCreatedListId = ref<number | null>(null);

const targetSong = computed<Song | null>(() => store.addToPlaylistModalSong);

function songMatches(a: Song, b: Song): boolean {
  const hashA = String(a.hash || "").trim().toLowerCase();
  const hashB = String(b.hash || "").trim().toLowerCase();
  if (hashA && hashB && hashA === hashB) return true;
  if (a.id && b.id && a.id === b.id) return true;
  return false;
}

watch(
  () => targetSong.value,
  (song) => {
    filterQuery.value = "";
    isCreatingMode.value = false;
    newPlaylistName.value = "";
    pendingListId.value = null;
    justCreatedListId.value = null;
    const seeded = new Set<number>();
    if (song && store.selectedPlaylist?.listId) {
      const hit = store.playlistSongs.some((item) => songMatches(item, song));
      if (hit) {
        seeded.add(store.selectedPlaylist.listId);
      }
    }
    collectedListIds.value = seeded;
  }
);

const likedPlaylistEntry = computed<Playlist | null>(() => {
  return (
    store.userPlaylists.find((p) => p.isLiked || (p.title || p.name || "").trim() === "我喜欢") ||
    null
  );
});

const isLikedCollected = computed(() =>
  targetSong.value ? store.isSongLiked(targetSong.value) : false
);

const likedTrackCount = computed(() => {
  if (likedPlaylistEntry.value && likedPlaylistEntry.value.trackCount > 0) {
    return likedPlaylistEntry.value.trackCount;
  }
  return store.likedSongs.length;
});

const createdPlaylists = computed(() => {
  const list = store.userPlaylists.filter((p) => {
    if (!p.isCreatedByMe || p.isLiked) return false;
    const title = (p.title || p.name || "").trim();
    return title !== "默认收藏" && title !== "我喜欢";
  });
  const q = filterQuery.value.trim().toLowerCase();
  if (!q) return list;
  return list.filter((p) => {
    const title = (p.title || p.name || "").toLowerCase();
    return title.includes(q);
  });
});

function isPlaylistCollected(pl: Playlist): boolean {
  if (!targetSong.value || !pl.listId) return false;
  if (collectedListIds.value.has(pl.listId)) return true;
  if (store.selectedPlaylist?.listId === pl.listId) {
    return store.playlistSongs.some((item) => songMatches(item, targetSong.value!));
  }
  return false;
}

async function handleToggleFavoriteRow() {
  const song = targetSong.value;
  if (!song) return;
  await store.toggleFavoriteSong(song);
}

async function handleSelectPlaylist(pl: Playlist) {
  const song = targetSong.value;
  if (!song || !pl.listId || pendingListId.value === pl.listId) return;
  pendingListId.value = pl.listId;
  try {
    const ok = await store.addSongToUserPlaylist(pl.listId, song, { keepOpen: true });
    if (ok) {
      const next = new Set(collectedListIds.value);
      next.add(pl.listId);
      collectedListIds.value = next;
    }
  } finally {
    pendingListId.value = null;
  }
}

async function handleCreateAndCollect() {
  const song = targetSong.value;
  const trimmed = newPlaylistName.value.trim();
  if (!song || !trimmed || isSubmittingCreate.value) return;

  isSubmittingCreate.value = true;
  const existingIds = new Set(store.userPlaylists.map((p) => p.listId));
  try {
    const createdOk = await store.createNewPlaylist(trimmed);
    if (!createdOk) return;

    const candidates = store.userPlaylists.filter((p) => {
      const title = (p.title || p.name || "").trim();
      return title === trimmed && p.listId > 0;
    });
    const newlyCreated =
      candidates.find((p) => !existingIds.has(p.listId)) ||
      candidates.sort((a, b) => b.listId - a.listId)[0];

    if (newlyCreated?.listId) {
      await store.addSongToUserPlaylist(newlyCreated.listId, song, { keepOpen: true });
      const next = new Set(collectedListIds.value);
      next.add(newlyCreated.listId);
      collectedListIds.value = next;
      justCreatedListId.value = newlyCreated.listId;
      store.notify(`已新建「${trimmed}」并自动收录《${song.title}》`);
    }

    filterQuery.value = "";
    newPlaylistName.value = "";
    isCreatingMode.value = false;
  } finally {
    isSubmittingCreate.value = false;
  }
}

function closeModal() {
  store.addToPlaylistModalSong = null;
}

const likeBadgeStyle = computed(() => {
  if (isLikedCollected.value) {
    return {
      background: hexWithAlpha("#fb7185", isLightSurface.value ? 0.16 : 0.22),
      color: isLightSurface.value ? "#be123c" : "#fda4af",
    };
  }
  return {
    background: "var(--fm-chip-bg)",
    color: "var(--fm-text-2)",
  };
});

function rowStyle(collected: boolean, highlighted: boolean) {
  if (highlighted || collected) {
    return {
      background: `linear-gradient(135deg, ${hexWithAlpha(
        accentPrimary.value,
        isLightSurface.value ? 0.14 : 0.22
      )}, var(--fm-card-elevated))`,
    };
  }
  return {};
}
</script>

<template>
  <Teleport to="body">
    <transition name="np-float-spring">
      <div
        v-if="targetSong"
        class="np-floating-layer playlist-modal-layer"
        @click.self="closeModal"
      >
        <div
          class="np-floating-window w-playlist"
          :class="{ 'is-dark-surface': !isLightSurface }"
          :style="modalSurfaceStyle"
          role="dialog"
          aria-modal="true"
          aria-label="收藏至歌单"
          @click.stop
        >
          <!-- 顶部标题栏 -->
          <div class="pm-header">
            <div class="pm-title-group">
              <div class="pm-icon-orb">
                <PlumpIcon name="np-playlist-add" :size="18" :color="accentPrimary" />
              </div>
              <div class="pm-title-copy">
                <h3 class="pm-title">收藏至歌单</h3>
                <p class="pm-subtitle" :title="targetSong.title">
                  《{{ targetSong.title }}》 · {{ targetSong.artist || "未知歌手" }}
                </p>
              </div>
            </div>
            <button
              type="button"
              class="pm-close-btn"
              aria-label="关闭收藏至歌单窗口"
              @click="closeModal"
            >
              <PlumpIcon name="close" :size="14" />
            </button>
          </div>

          <!-- 顶部快捷筛选 + 新建触发 -->
          <div class="pm-toolbar">
            <label class="pm-search-box">
              <PlumpIcon name="search" :size="14" color="var(--fm-text-3)" />
              <input
                v-model="filterQuery"
                type="text"
                class="pm-search-input"
                placeholder="快捷筛选歌单名称..."
              />
              <button
                v-if="filterQuery"
                type="button"
                class="pm-clear-btn"
                aria-label="清空筛选"
                @click="filterQuery = ''"
              >
                <PlumpIcon name="close" :size="11" />
              </button>
            </label>

            <button
              type="button"
              class="pm-new-toggle-btn"
              :style="
                isCreatingMode
                  ? { background: accentPrimary, color: onAccentPrimary }
                  : {}
              "
              @click="isCreatingMode = !isCreatingMode"
            >
              <PlumpIcon
                :name="isCreatingMode ? 'close' : 'plus'"
                :size="13"
                :color="isCreatingMode ? onAccentPrimary : accentPrimary"
              />
              <span>{{ isCreatingMode ? "取消" : "新建" }}</span>
            </button>
          </div>

          <!-- 新建歌单并自动收录表单 -->
          <transition name="pm-collapse">
            <form
              v-if="isCreatingMode"
              class="pm-create-panel"
              @submit.prevent="handleCreateAndCollect"
            >
              <input
                v-model="newPlaylistName"
                type="text"
                class="pm-create-input"
                maxlength="40"
                placeholder="输入新歌单名，创建后将自动收录本曲..."
                autofocus
              />
              <button
                type="submit"
                class="pm-create-submit"
                :disabled="!newPlaylistName.trim() || isSubmittingCreate"
                :style="{ background: accentPrimary, color: onAccentPrimary }"
              >
                {{ isSubmittingCreate ? "创建中..." : "创建并收录" }}
              </button>
            </form>
          </transition>

          <!-- 固定区域滚动体 -->
          <div class="pm-list-scroll">
            <!-- 置顶：我喜欢 -->
            <button
              type="button"
              class="pm-playlist-row pm-liked-row"
              :class="{ 'is-collected': isLikedCollected }"
              @click="handleToggleFavoriteRow"
            >
              <div class="pm-row-left">
                <div
                  class="pm-row-cover pm-liked-cover"
                  :style="{
                    background: isLikedCollected
                      ? 'linear-gradient(135deg, rgba(251,113,133,0.28), rgba(244,63,94,0.16))'
                      : 'var(--fm-chip-bg)',
                  }"
                >
                  <PlumpIcon
                    name="heart"
                    :size="18"
                    :color="isLikedCollected ? '#f43f5e' : 'var(--fm-text-2)'"
                  />
                </div>
                <div class="pm-row-meta">
                  <div class="pm-row-title-line">
                    <span class="pm-row-title">我喜欢</span>
                    <span class="pm-pinned-tag">置顶</span>
                  </div>
                  <div class="pm-row-count">{{ likedTrackCount }} 首红心曲目</div>
                </div>
              </div>
              <span class="pm-status-chip" :style="likeBadgeStyle">
                {{ isLikedCollected ? "♥ 已喜欢" : "加入喜欢" }}
              </span>
            </button>

            <div class="pm-section-caption">
              <span>自建云歌单 ({{ createdPlaylists.length }})</span>
              <span class="pm-section-hint">点击歌单可多选收录</span>
            </div>

            <!-- 其余自建歌单 -->
            <button
              v-for="pl in createdPlaylists"
              :key="pl.id"
              type="button"
              class="pm-playlist-row"
              :class="{
                'is-collected': isPlaylistCollected(pl),
                'is-just-created': justCreatedListId === pl.listId,
              }"
              :style="rowStyle(isPlaylistCollected(pl), justCreatedListId === pl.listId)"
              @click="handleSelectPlaylist(pl)"
            >
              <div class="pm-row-left">
                <div class="pm-row-cover">
                  <img
                    v-if="pl.coverUrl"
                    :src="pl.coverUrl"
                    alt=""
                    class="pm-cover-img"
                  />
                  <PlumpIcon
                    v-else
                    name="library"
                    :size="17"
                    :color="isPlaylistCollected(pl) ? accentPrimary : 'var(--fm-text-2)'"
                  />
                </div>
                <div class="pm-row-meta">
                  <div class="pm-row-title-line">
                    <span class="pm-row-title">{{ pl.title }}</span>
                    <span v-if="justCreatedListId === pl.listId" class="pm-new-badge">
                      刚创建
                    </span>
                  </div>
                  <div class="pm-row-count">{{ pl.trackCount }} 首曲目</div>
                </div>
              </div>

              <span
                class="pm-status-chip"
                :style="
                  isPlaylistCollected(pl)
                    ? {
                        background: hexWithAlpha(
                          accentPrimary,
                          isLightSurface ? 0.16 : 0.24
                        ),
                        color: accentPrimary,
                      }
                    : {
                        background: 'var(--fm-chip-bg)',
                        color: 'var(--fm-text-2)',
                      }
                "
              >
                <template v-if="pendingListId === pl.listId">收录中...</template>
                <template v-else-if="isPlaylistCollected(pl)">✓ 已收录</template>
                <template v-else>+ 收录</template>
              </span>
            </button>

            <div v-if="createdPlaylists.length === 0" class="pm-empty-state">
              <PlumpIcon name="library" :size="24" color="var(--fm-text-3)" />
              <p v-if="filterQuery.trim()">
                没有匹配「{{ filterQuery.trim() }}」的歌单，可直接点击右上角「新建」创建
              </p>
              <p v-else>暂无其他自建歌单，可点击上方「新建」立即创建</p>
            </div>
          </div>

          <div class="pm-footer">
            <span class="pm-footer-note">支持多选收藏 · 操作实时同步云歌单</span>
            <button
              type="button"
              class="pm-done-btn"
              :style="{ background: accentPrimary, color: onAccentPrimary }"
              @click="closeModal"
            >
              完成
            </button>
          </div>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.playlist-modal-layer {
  position: fixed;
  inset: 0;
  z-index: 3200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.w-playlist {
  width: min(440px, 92vw);
  height: min(560px, 84vh);
  border-radius: var(--bm-r-xl, 24px);
  backdrop-filter: blur(26px) saturate(165%);
  -webkit-backdrop-filter: blur(26px) saturate(165%);
  padding: 20px 22px;
  color: var(--fm-text-1);
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow: hidden;
  border: none;
}

.pm-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-shrink: 0;
}

.pm-title-group {
  display: flex;
  align-items: center;
  gap: 11px;
  min-width: 0;
}

.pm-icon-orb {
  width: 38px;
  height: 38px;
  border-radius: 13px;
  background: var(--fm-card-elevated);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.pm-title-copy {
  min-width: 0;
}

.pm-title {
  margin: 0;
  font-size: 16px;
  font-weight: 800;
  color: var(--fm-text-1);
  line-height: 1.25;
}

.pm-subtitle {
  margin: 2px 0 0;
  font-size: 12px;
  color: var(--fm-text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pm-close-btn {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  border: none;
  background: var(--fm-card);
  color: var(--fm-text-2);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  transition: background 0.16s ease, color 0.16s ease, transform 0.16s ease;
}

.pm-close-btn:hover {
  background: var(--fm-card-hover);
  color: var(--fm-text-1);
  transform: scale(1.06);
}

.pm-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.pm-search-box {
  flex: 1;
  min-width: 0;
  height: 36px;
  padding: 0 12px;
  border-radius: 12px;
  background: var(--fm-input-bg);
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: text;
}

.pm-search-box:focus-within {
  outline: 2px solid var(--fm-primary);
  outline-offset: -2px;
}

.pm-search-input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  color: var(--fm-text-1);
  font-size: 12.5px;
  outline: none;
}

.pm-search-input::placeholder {
  color: var(--fm-text-3);
}

.pm-clear-btn {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  border: none;
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
}

.pm-new-toggle-btn {
  height: 36px;
  padding: 0 13px;
  border-radius: 12px;
  border: none;
  background: var(--fm-card-elevated);
  color: var(--fm-text-1);
  font-size: 12.5px;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
  flex-shrink: 0;
  transition: transform 0.16s ease, background 0.16s ease;
}

.pm-new-toggle-btn:hover {
  background: var(--fm-card-hover);
  transform: translateY(-1px);
}

.pm-create-panel {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-radius: 14px;
  background: var(--fm-card-elevated);
  flex-shrink: 0;
}

.pm-create-input {
  flex: 1;
  min-width: 0;
  height: 34px;
  padding: 0 10px;
  border-radius: 10px;
  border: none;
  background: var(--fm-input-bg);
  color: var(--fm-text-1);
  font-size: 12.5px;
  outline: none;
}

.pm-create-input::placeholder {
  color: var(--fm-text-3);
}

.pm-create-input:focus-visible {
  outline: 2px solid var(--fm-primary);
  outline-offset: -2px;
}

.pm-create-submit {
  height: 34px;
  padding: 0 14px;
  border-radius: 10px;
  border: none;
  font-size: 12px;
  font-weight: 800;
  cursor: pointer;
  flex-shrink: 0;
  transition: opacity 0.16s ease, transform 0.16s ease;
}

.pm-create-submit:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.pm-list-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-right: 4px;
}

.pm-list-scroll::-webkit-scrollbar {
  width: 5px;
}

.pm-list-scroll::-webkit-scrollbar-thumb {
  border-radius: 999px;
  background: var(--fm-divider);
}

.pm-section-caption {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 4px 0;
  font-size: 11.5px;
  font-weight: 700;
  color: var(--fm-text-3);
}

.pm-section-hint {
  font-weight: 500;
  font-size: 11px;
}

.pm-playlist-row {
  width: 100%;
  border: none;
  border-radius: 14px;
  padding: 10px 12px;
  background: var(--fm-card);
  color: var(--fm-text-1);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  cursor: pointer;
  text-align: left;
  transition: background 0.16s ease, transform 0.16s ease;
}

.pm-playlist-row:hover {
  background: var(--fm-card-hover);
  transform: translateY(-1px);
}

.pm-liked-row {
  background: var(--fm-card-elevated);
}

.pm-row-left {
  display: flex;
  align-items: center;
  gap: 11px;
  min-width: 0;
  flex: 1;
}

.pm-row-cover {
  width: 40px;
  height: 40px;
  border-radius: 11px;
  background: var(--fm-chip-bg);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  flex-shrink: 0;
}

.pm-cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.pm-row-meta {
  min-width: 0;
  flex: 1;
}

.pm-row-title-line {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}

.pm-row-title {
  font-size: 13.5px;
  font-weight: 700;
  color: var(--fm-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pm-pinned-tag,
.pm-new-badge {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 999px;
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
  flex-shrink: 0;
}

.pm-row-count {
  margin-top: 2px;
  font-size: 11.5px;
  color: var(--fm-text-3);
}

.pm-status-chip {
  padding: 5px 10px;
  border-radius: 999px;
  font-size: 11.5px;
  font-weight: 700;
  flex-shrink: 0;
  transition: background 0.16s ease, color 0.16s ease;
}

.pm-empty-state {
  flex: 1;
  min-height: 160px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 20px;
  text-align: center;
  color: var(--fm-text-3);
  font-size: 12.5px;
}

.pm-empty-state p {
  margin: 0;
  max-width: 260px;
  line-height: 1.5;
}

.pm-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding-top: 4px;
  flex-shrink: 0;
}

.pm-footer-note {
  font-size: 11.5px;
  color: var(--fm-text-3);
}

.pm-done-btn {
  height: 34px;
  padding: 0 18px;
  border-radius: 999px;
  border: none;
  font-size: 12.5px;
  font-weight: 800;
  cursor: pointer;
  transition: transform 0.16s ease, filter 0.16s ease;
}

.pm-done-btn:hover {
  transform: translateY(-1px);
  filter: brightness(1.06);
}

.pm-collapse-enter-active,
.pm-collapse-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}

.pm-collapse-enter-from,
.pm-collapse-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}
</style>

<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import { formatDuration, resolveSongQualityBadge } from "../api/kugou";
import { useViewportStagger } from "../composables/useViewportStagger";
import type { Song } from "../types";
import PlumpIcon from "../components/PlumpIcon.vue";
import FloatingLocateFab from "../components/FloatingLocateFab.vue";

const store = useAppStore();

const searchKeyword = ref("");
const sortMode = ref<"default" | "title" | "artist" | "album" | "duration">("default");
const isIntroExpanded = ref(false);
const songGridRef = ref<HTMLElement | null>(null);
const playlistStagger = useViewportStagger(songGridRef, {
  defaultCols: 2,
  rowHeight: 64,
  estimatedChromeOffset: 170,
});

// 批量多选模式状态
const isBatchMode = ref(false);
const selectedKeys = ref<Set<string>>(new Set());
const lastClickedIndex = ref<number | null>(null);
const showBatchAddToPlaylistModal = ref(false);

// 编辑歌单弹窗状态
const isEditModalOpen = ref(false);
const editTitle = ref("");
const editIntro = ref("");
const editCoverUrl = ref("");
const isSavingEdit = ref(false);

// 删除确认弹窗
const showDeleteConfirmModal = ref(false);

const playlist = computed(() => store.selectedPlaylist);
const isLikedPlaylist = computed(
  () => Boolean(playlist.value?.isLiked || playlist.value?.title === "我喜欢")
);
const canEditOrRemove = computed(
  () => Boolean(playlist.value?.isCreatedByMe && playlist.value?.listId)
);
const isCollectedPlaylist = computed(() =>
  playlist.value ? store.isPlaylistCollected(playlist.value) : false
);

watch(
  () => store.selectedPlaylist?.id,
  () => {
    searchKeyword.value = "";
    sortMode.value = "default";
    isBatchMode.value = false;
    selectedKeys.value = new Set();
    lastClickedIndex.value = null;
    isIntroExpanded.value = false;
  }
);

const filteredSongs = computed<Song[]>(() => {
  const kw = searchKeyword.value.trim().toLowerCase();
  let list = [...store.playlistSongs];
  if (kw) {
    list = list.filter(
      (s) =>
        s.title.toLowerCase().includes(kw) ||
        s.artist.toLowerCase().includes(kw) ||
        (s.album || "").toLowerCase().includes(kw)
    );
  }
  if (sortMode.value === "title") {
    list.sort((a, b) => a.title.localeCompare(b.title, "zh-CN"));
  } else if (sortMode.value === "artist") {
    list.sort((a, b) => a.artist.localeCompare(b.artist, "zh-CN"));
  } else if (sortMode.value === "album") {
    list.sort((a, b) => (a.album || "").localeCompare(b.album || "", "zh-CN"));
  } else if (sortMode.value === "duration") {
    list.sort((a, b) => (b.durationMs || 0) - (a.durationMs || 0));
  }
  return list;
});

const coverImage = computed(() => {
  return (
    playlist.value?.coverUrl ||
    store.playlistSongs[0]?.coverUrl ||
    ""
  );
});

const playlistIntroText = computed(() => {
  const raw = (
    playlist.value?.intro ||
    playlist.value?.description ||
    ""
  ).trim();
  if (raw) return raw;
  if (isLikedPlaylist.value) {
    return "你在酷狗概念版标记红心的全部珍藏单曲，支持云端实时双向同步与无损畅听。";
  }
  return `由 ${playlist.value?.creator || "微风乐迷"} 创建的精选歌单，共收录 ${
    Math.max(playlist.value?.trackCount || 0, store.playlistSongs.length)
  } 首高品质曲目。`;
});

const isIntroExpandable = computed(() => playlistIntroText.value.length > 56);

function toggleIntroExpanded() {
  if (!isIntroExpandable.value) return;
  isIntroExpanded.value = !isIntroExpanded.value;
}

function playAll() {
  if (filteredSongs.value.length === 0) return;
  store.playSongFromList(
    filteredSongs.value[0],
    filteredSongs.value,
    playlist.value?.title || "歌单播放"
  );
}

function openEditModal() {
  if (!playlist.value) return;
  editTitle.value = playlist.value.title || playlist.value.name || "";
  editIntro.value = playlist.value.intro || playlist.value.description || "";
  editCoverUrl.value = playlist.value.coverUrl || "";
  isEditModalOpen.value = true;
}

async function savePlaylistEdit() {
  if (!playlist.value?.listId || !editTitle.value.trim()) return;
  isSavingEdit.value = true;
  try {
    const ok = await store.updatePlaylistMeta(
      playlist.value.listId,
      editTitle.value.trim(),
      editIntro.value.trim(),
      editCoverUrl.value.trim()
    );
    if (ok) {
      isEditModalOpen.value = false;
    }
  } finally {
    isSavingEdit.value = false;
  }
}

function getSongUid(song: Song, idx?: number): string {
  if (song.hash) return `${song.hash}_${song.albumAudioId || 0}`;
  if (song.id) return `${song.id}_${song.albumAudioId || 0}`;
  return `${song.title}::${song.artist}::${idx ?? 0}`;
}

function appendAllToQueue() {
  if (filteredSongs.value.length === 0) return;
  store.batchAppendSongsToQueue(filteredSongs.value);
}

async function handleToggleCollectPlaylist() {
  if (!playlist.value) return;
  await store.toggleCollectPlaylist(playlist.value);
}

function toggleBatchMode() {
  isBatchMode.value = !isBatchMode.value;
  selectedKeys.value = new Set();
  lastClickedIndex.value = null;
}

function toggleSongChecked(song: Song, idx?: number) {
  const uid = getSongUid(song, idx);
  const next = new Set(selectedKeys.value);
  if (next.has(uid)) {
    next.delete(uid);
  } else {
    next.add(uid);
  }
  selectedKeys.value = next;
  if (typeof idx === "number") {
    lastClickedIndex.value = idx;
  }
}

function handleRowClick(event: MouseEvent, song: Song, idx: number) {
  if (!isBatchMode.value) {
    store.playSongFromList(song, filteredSongs.value, playlist.value?.title || "歌单");
    return;
  }

  if (event.shiftKey && lastClickedIndex.value !== null) {
    const start = Math.min(lastClickedIndex.value, idx);
    const end = Math.max(lastClickedIndex.value, idx);
    const next = new Set(selectedKeys.value);
    for (let i = start; i <= end; i += 1) {
      const target = filteredSongs.value[i];
      if (target) {
        next.add(getSongUid(target, i));
      }
    }
    selectedKeys.value = next;
    lastClickedIndex.value = idx;
    return;
  }

  toggleSongChecked(song, idx);
}

function toggleSelectAll() {
  if (selectedKeys.value.size >= filteredSongs.value.length && filteredSongs.value.length > 0) {
    selectedKeys.value = new Set();
  } else {
    selectedKeys.value = new Set(filteredSongs.value.map((s, idx) => getSongUid(s, idx)));
  }
  lastClickedIndex.value = null;
}

function toggleInvertSelection() {
  const next = new Set<string>();
  filteredSongs.value.forEach((song, idx) => {
    const uid = getSongUid(song, idx);
    if (!selectedKeys.value.has(uid)) {
      next.add(uid);
    }
  });
  selectedKeys.value = next;
  lastClickedIndex.value = null;
}

const selectedSongsList = computed(() =>
  filteredSongs.value.filter((s, idx) => selectedKeys.value.has(getSongUid(s, idx)))
);

function batchPlaySelected() {
  if (selectedSongsList.value.length === 0) return;
  store.playSongFromList(
    selectedSongsList.value[0],
    selectedSongsList.value,
    `${playlist.value?.title || "歌单"} · 已选曲目`
  );
  isBatchMode.value = false;
  selectedKeys.value = new Set();
  lastClickedIndex.value = null;
}

function batchPlayNextSelected() {
  const list = selectedSongsList.value;
  if (list.length === 0) return;
  if (store.playQueue.length === 0) {
    store.playSongFromList(list[0], list, `${playlist.value?.title || "歌单"} · 下一首`);
    isBatchMode.value = false;
    selectedKeys.value = new Set();
    lastClickedIndex.value = null;
    return;
  }
  for (let i = list.length - 1; i >= 0; i -= 1) {
    store.insertPlayNext(list[i]);
  }
  store.notify(`已将 ${list.length} 首曲目设为下一首播放`);
  isBatchMode.value = false;
  selectedKeys.value = new Set();
  lastClickedIndex.value = null;
}

function handleBatchAppendQueue() {
  if (selectedSongsList.value.length === 0) return;
  store.batchAppendSongsToQueue(selectedSongsList.value);
  isBatchMode.value = false;
  selectedKeys.value = new Set();
  lastClickedIndex.value = null;
}

async function handleBatchDownloadSelected() {
  if (selectedSongsList.value.length === 0) return;
  await store.batchDownloadSongs(selectedSongsList.value);
}

async function handleBatchAddToTarget(listId: number) {
  if (selectedSongsList.value.length === 0) return;
  await store.batchAddSongsToPlaylist(listId, selectedSongsList.value);
  showBatchAddToPlaylistModal.value = false;
  isBatchMode.value = false;
  selectedKeys.value = new Set();
  lastClickedIndex.value = null;
}

async function handleBatchRemoveSelected() {
  if (selectedSongsList.value.length === 0) return;
  const ok = await store.batchRemoveSongsFromPlaylist(selectedSongsList.value);
  if (ok) {
    isBatchMode.value = false;
    selectedKeys.value = new Set();
    lastClickedIndex.value = null;
  }
}

async function confirmDeleteOrUncollect() {
  if (!playlist.value) return;
  showDeleteConfirmModal.value = false;
  if (playlist.value.isCreatedByMe && playlist.value.listId) {
    await store.deletePlaylist(playlist.value);
    store.navigateBack();
  } else {
    await store.toggleCollectPlaylist(playlist.value);
    store.navigateBack();
  }
}
</script>

<template>
  <section class="standalone-detail-page playlist-standalone-page">
    <!-- 歌单双栏大屏无框布局：左栏固定艺术海报 + 右栏无界曲目流（返回操作由顶部灵动岛左翼分裂圆钮承载） -->
    <div class="sdp-two-pane-layout">
      <!-- 左栏：封面海报、元信息、可折叠简介、主次动作组 -->
      <aside class="sdp-left-hero-card">
        <div v-tilt="{ maxDeg: 3.5 }" class="sdp-cover-showcase">
          <img
            v-if="coverImage"
            :src="coverImage"
            class="sdp-cover-img"
            alt="cover"
          />
          <div v-else class="sdp-cover-fallback">
            <PlumpIcon :name="isLikedPlaylist ? 'heart-filled' : 'library'" :size="52" color="#fff" />
          </div>
        </div>

        <h1 class="sdp-title">{{ playlist?.title || "精选歌单" }}</h1>

        <div class="sdp-creator-row">
          <span class="sdp-creator-tag">
            {{
              isLikedPlaylist
                ? "我喜欢的音乐"
                : playlist?.isCreatedByMe
                ? "自建歌单"
                : isCollectedPlaylist
                ? "收藏歌单"
                : "精选歌单"
            }}
          </span>
          <span class="sdp-creator-name">
            {{ playlist?.creator || store.profile?.nickname || "酷狗概念版" }} ·
            {{ Math.max(playlist?.trackCount || 0, store.playlistSongs.length) }} 首
          </span>
        </div>

        <div
          class="sdp-intro-box"
          :class="{
            'is-expandable': isIntroExpandable,
            expanded: isIntroExpanded || !isIntroExpandable,
          }"
          @click="toggleIntroExpanded"
        >
          <p class="sdp-intro-text">{{ playlistIntroText }}</p>
          <span v-if="isIntroExpandable" class="sdp-intro-more">
            {{ isIntroExpanded ? "收起简介" : "展开简介" }}
          </span>
        </div>

        <div class="sdp-action-matrix">
          <button
            type="button"
            class="sdp-primary-action-btn"
            :disabled="filteredSongs.length === 0"
            @click="playAll"
          >
            <PlumpIcon name="play" :size="16" color="#fff" />
            <span>播放全部</span>
          </button>

          <div class="sdp-secondary-actions">
            <button
              v-if="!isLikedPlaylist && !playlist?.isCreatedByMe && playlist"
              type="button"
              class="sdp-sec-btn"
              :class="{ 'is-collected': isCollectedPlaylist }"
              @click="handleToggleCollectPlaylist"
            >
              <PlumpIcon :name="isCollectedPlaylist ? 'heart-filled' : 'heart'" :size="15" />
              <span>{{ isCollectedPlaylist ? "已收藏" : "收藏" }}</span>
            </button>
            <button
              type="button"
              class="sdp-sec-btn"
              :disabled="filteredSongs.length === 0"
              @click="appendAllToQueue"
            >
              <PlumpIcon name="plus" :size="15" />
              <span>追加</span>
            </button>
            <button
              type="button"
              class="sdp-sec-btn"
              :class="{ active: isBatchMode }"
              :disabled="filteredSongs.length === 0 && !isBatchMode"
              @click="toggleBatchMode"
            >
              <PlumpIcon name="batch-check" :size="15" />
              <span>{{ isBatchMode ? "退出批量" : "批量操作" }}</span>
            </button>
            <button
              type="button"
              class="sdp-sec-btn"
              :disabled="store.isLoadingLibrary"
              @click="store.refreshCurrentPlaylistOnly()"
            >
              <PlumpIcon name="refresh" :size="15" :class="{ 'is-spinning': store.isLoadingLibrary }" />
              <span>刷新</span>
            </button>
            <button
              v-if="canEditOrRemove && !isLikedPlaylist"
              type="button"
              class="sdp-sec-btn"
              @click="openEditModal"
            >
              <PlumpIcon name="edit" :size="15" />
              <span>编辑</span>
            </button>
            <button
              v-if="canEditOrRemove && !isLikedPlaylist"
              type="button"
              class="sdp-sec-btn danger"
              @click="showDeleteConfirmModal = true"
            >
              <PlumpIcon name="trash" :size="15" />
              <span>删除</span>
            </button>
          </div>
        </div>
      </aside>

      <!-- 右栏：歌单内搜索、排序、批量管理栏与高密度曲目列表 -->
      <div class="sdp-right-tracks-pane">
        <!-- 歌单内搜索与排序栏 -->
        <div class="sdp-tracks-toolbar">
          <div class="sdp-search-input-box">
            <PlumpIcon name="search" :size="14" />
            <input
              v-model="searchKeyword"
              type="text"
              placeholder="搜索歌曲、歌手或专辑..."
            />
            <button v-if="searchKeyword" type="button" class="clear-btn" @click="searchKeyword = ''">
              <PlumpIcon name="close" :size="12" />
            </button>
          </div>

          <div class="sdp-sort-pills">
            <button
              type="button"
              class="sort-pill"
              :class="{ active: sortMode === 'default' }"
              @click="sortMode = 'default'"
            >
              默认
            </button>
            <button
              type="button"
              class="sort-pill"
              :class="{ active: sortMode === 'title' }"
              @click="sortMode = 'title'"
            >
              歌名
            </button>
            <button
              type="button"
              class="sort-pill"
              :class="{ active: sortMode === 'artist' }"
              @click="sortMode = 'artist'"
            >
              歌手
            </button>
            <button
              type="button"
              class="sort-pill"
              :class="{ active: sortMode === 'album' }"
              @click="sortMode = 'album'"
            >
              专辑
            </button>
            <button
              type="button"
              class="sort-pill"
              :class="{ active: sortMode === 'duration' }"
              @click="sortMode = 'duration'"
            >
              时长
            </button>
          </div>
        </div>

        <!-- 批量多选操作浮动条 -->
        <transition name="fade">
          <div v-if="isBatchMode" class="batch-floating-action-bar">
            <div class="bfab-left">
              <button type="button" class="bfab-pill" @click="toggleSelectAll">
                {{
                  filteredSongs.length > 0 && selectedKeys.size === filteredSongs.length
                    ? "取消全选"
                    : "全选"
                }}
              </button>
              <button
                type="button"
                class="bfab-pill"
                :disabled="filteredSongs.length === 0"
                @click="toggleInvertSelection"
              >
                反选
              </button>
              <span class="bfab-count">
                已选 {{ selectedKeys.size }} 首<span class="bfab-hint">（按住 Shift 可连选）</span>
              </span>
            </div>

            <div class="bfab-actions">
              <button
                type="button"
                class="bfab-btn primary"
                :disabled="selectedKeys.size === 0"
                @click="batchPlaySelected"
              >
                <PlumpIcon name="play" :size="14" color="#fff" />
                <span>播放</span>
              </button>
              <button
                type="button"
                class="bfab-btn"
                :disabled="selectedKeys.size === 0"
                @click="batchPlayNextSelected"
              >
                <PlumpIcon name="plus" :size="14" />
                <span>下一首播放</span>
              </button>
              <button
                type="button"
                class="bfab-btn"
                :disabled="selectedKeys.size === 0"
                @click="showBatchAddToPlaylistModal = true"
              >
                <PlumpIcon name="playlist-add" :size="14" />
                <span>添加到歌单</span>
              </button>
              <button
                v-if="store.downloadFeatureUnlocked"
                type="button"
                class="bfab-btn"
                :disabled="selectedKeys.size === 0 || store.isDownloadingSong"
                @click="handleBatchDownloadSelected"
              >
                <PlumpIcon name="download" :size="14" />
                <span>下载</span>
              </button>
              <button
                v-if="canEditOrRemove"
                type="button"
                class="bfab-btn danger"
                :disabled="selectedKeys.size === 0"
                @click="handleBatchRemoveSelected"
              >
                <PlumpIcon name="trash" :size="14" />
                <span>移出歌单</span>
              </button>
              <button type="button" class="bfab-btn ghost" @click="toggleBatchMode">
                <span>取消</span>
              </button>
            </div>
          </div>
        </transition>

        <div v-if="store.isLoadingLibrary && store.playlistSongs.length === 0" class="sdp-empty-state">
          正在从酷狗概念版云端加载歌单曲目...
        </div>
        <div v-else-if="filteredSongs.length === 0" class="sdp-empty-state">
          {{ searchKeyword ? "未找到匹配的歌单曲目" : "当前歌单暂无曲目" }}
        </div>

        <div v-else ref="songGridRef" class="frameless-songs-grid-2col">
          <div
            v-for="(song, idx) in filteredSongs"
            :key="getSongUid(song, idx) + '_' + idx"
            :data-locate-song-key="song.hash || song.id"
            class="unified-song-row"
            :class="[
              {
                active: store.currentSong?.hash === song.hash,
                selected: isBatchMode && selectedKeys.has(getSongUid(song, idx)),
                'is-batch-mode': isBatchMode,
              },
              playlistStagger.getStaggerClass(idx),
            ]"
            :style="playlistStagger.getStaggerStyle(idx)"
            @click="handleRowClick($event, song, idx)"
            @contextmenu.prevent="!isBatchMode && store.openSongContextMenu($event, song, playlist)"
          >
            <div class="usr-index-slot">
              <span
                v-if="isBatchMode"
                class="usr-batch-checkbox"
                :class="{ checked: selectedKeys.has(getSongUid(song, idx)) }"
              >
                <PlumpIcon
                  v-if="selectedKeys.has(getSongUid(song, idx))"
                  name="check"
                  :size="12"
                  color="#fff"
                />
              </span>
              <span
                v-else-if="store.currentSong?.hash === song.hash"
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
                    :data-tip="isBatchMode ? '点击选中歌曲' : '查看歌手主页'"
                    @click="
                      isBatchMode
                        ? null
                        : ($event.stopPropagation(), store.navigateToSongArtist(song))
                    "
                  >
                    {{ song.artist }}
                  </button>
                  <template v-if="song.album">
                    <span class="usr-sep"> · </span>
                    <button
                      type="button"
                      class="usr-album usr-meta-link"
                      :data-tip="isBatchMode ? '点击选中歌曲' : '查看专辑详情'"
                      @click="
                        isBatchMode
                          ? null
                          : ($event.stopPropagation(), store.navigateToSongAlbum(song))
                      "
                    >
                      {{ song.album }}
                    </button>
                  </template>
                </span>
              </div>
            </div>

            <div class="usr-right-zone">
              <div v-if="!isBatchMode" class="usr-hover-actions" @click.stop>
                <button
                  type="button"
                  class="row-icon-btn"
                  :class="{ liked: store.isSongLiked(song) }"
                  :data-tip="store.isSongLiked(song) ? '取消喜欢' : '收藏到我喜欢'"
                  @click="store.toggleFavoriteSong(song)"
                >
                  <PlumpIcon
                    :name="store.isSongLiked(song) ? 'heart-filled' : 'heart'"
                    :size="15"
                  />
                </button>
                <button
                  type="button"
                  class="row-icon-btn"
                  data-tip="查看歌曲评论"
                  @click="store.openCommentsModal(song)"
                >
                  <PlumpIcon name="comment" :size="15" />
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
                  v-if="canEditOrRemove"
                  type="button"
                  class="row-icon-btn danger"
                  data-tip="从当前歌单移除"
                  @click="store.removeSongFromCurrentPlaylist(song)"
                >
                  <PlumpIcon name="close" :size="14" />
                </button>
              </div>
              <span class="usr-duration">{{ formatDuration(song) }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 编辑歌单信息弹窗 -->
    <transition name="fade">
      <div
        v-if="isEditModalOpen"
        class="modal-backdrop"
        @click.self="isEditModalOpen = false"
      >
        <div class="compact-glass-modal" style="width: 440px">
          <div class="modal-header">
            <h3>编辑歌单信息</h3>
            <button class="icon-close-btn" @click="isEditModalOpen = false">
              <PlumpIcon name="close" :size="15" />
            </button>
          </div>

          <div style="display: flex; flex-direction: column; gap: 12px; margin-top: 8px">
            <div>
              <div class="cfg-label">歌单名称</div>
              <input
                v-model="editTitle"
                type="text"
                class="glass-input"
                placeholder="输入歌单名称..."
              />
            </div>
            <div>
              <div class="cfg-label">歌单简介 / 心情描述</div>
              <textarea
                v-model="editIntro"
                rows="3"
                class="glass-input"
                style="height: auto; padding: 10px 12px; resize: vertical"
                placeholder="写下这张歌单的灵感与介绍..."
              ></textarea>
            </div>
            <div>
              <div class="cfg-label">自定义封面图片 URL (可选)</div>
              <input
                v-model="editCoverUrl"
                type="text"
                class="glass-input"
                placeholder="留空则默认使用首曲高清封面"
              />
            </div>

            <div style="display: flex; justify-content: flex-end; gap: 10px; margin-top: 6px">
              <button class="tonal-pill-btn" @click="isEditModalOpen = false">取消</button>
              <button
                class="primary-btn"
                :disabled="!editTitle.trim() || isSavingEdit"
                @click="savePlaylistEdit"
              >
                {{ isSavingEdit ? "正在保存..." : "保存并同步云端" }}
              </button>
            </div>
          </div>
        </div>
      </div>
    </transition>

    <!-- 批量添加到歌单弹窗 -->
    <transition name="fade">
      <div
        v-if="showBatchAddToPlaylistModal"
        class="modal-backdrop"
        @click.self="showBatchAddToPlaylistModal = false"
      >
        <div class="compact-glass-modal" style="width: 420px">
          <div class="modal-header">
            <h3>批量收藏 {{ selectedKeys.size }} 首歌曲至歌单</h3>
            <button class="icon-close-btn" @click="showBatchAddToPlaylistModal = false">
              <PlumpIcon name="close" :size="15" />
            </button>
          </div>
          <div class="playlist-pick-list">
            <button
              v-for="pl in store.userPlaylists.filter((p) => p.isCreatedByMe && p.listId)"
              :key="pl.id"
              class="playlist-pick-row"
              @click="handleBatchAddToTarget(pl.listId)"
            >
              <div class="ppr-left">
                <PlumpIcon :name="pl.isLiked ? 'heart' : 'library'" :size="18" />
                <div>
                  <div class="ppr-title">{{ pl.title }}</div>
                  <div class="ppr-count">{{ pl.trackCount }} 首曲目</div>
                </div>
              </div>
              <PlumpIcon name="plus" :size="15" />
            </button>
          </div>
        </div>
      </div>
    </transition>

    <!-- 删除/取消收藏确认弹窗 -->
    <transition name="fade">
      <div
        v-if="showDeleteConfirmModal"
        class="modal-backdrop"
        @click.self="showDeleteConfirmModal = false"
      >
        <div class="compact-glass-modal" style="width: 380px">
          <div class="modal-header">
            <h3>{{ playlist?.isCreatedByMe ? "确认删除云歌单" : "确认取消收藏歌单" }}</h3>
            <button class="icon-close-btn" @click="showDeleteConfirmModal = false">
              <PlumpIcon name="close" :size="15" />
            </button>
          </div>
          <p style="font-size: 14px; color: var(--text-secondary); margin: 12px 0 18px">
            确定要{{ playlist?.isCreatedByMe ? "删除" : "取消收藏" }}歌单「{{ playlist?.title }}」吗？该操作将同步至酷狗概念版云端。
          </p>
          <div style="display: flex; justify-content: flex-end; gap: 10px">
            <button class="tonal-pill-btn" @click="showDeleteConfirmModal = false">取消</button>
            <button class="primary-btn" style="background: #f43f5e" @click="confirmDeleteOrUncollect">
              确认{{ playlist?.isCreatedByMe ? "删除" : "取消收藏" }}
            </button>
          </div>
        </div>
      </div>
    </transition>

    <FloatingLocateFab
      :songs="filteredSongs"
      :has-batch-bar="isBatchMode"
    />
  </section>
</template>

<style scoped>
.standalone-detail-page {
  padding: 12px 24px 36px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}
.sdp-two-pane-layout {
  display: grid;
  grid-template-columns: minmax(280px, 308px) minmax(0, 1fr);
  gap: clamp(20px, 2.2vw, 32px);
  align-items: start;
}
@media (max-width: 960px) {
  .sdp-two-pane-layout {
    grid-template-columns: 1fr;
  }
  .sdp-left-hero-card {
    position: static !important;
    max-height: none !important;
  }
}
.sdp-left-hero-card {
  padding: 18px 18px 20px;
  border-radius: var(--bm-r-xl, 24px);
  background: color-mix(in srgb, var(--bg-glass) 76%, transparent);
  box-shadow: 0 16px 38px rgba(0, 0, 0, 0.24);
  display: flex;
  flex-direction: column;
  gap: 13px;
  position: sticky;
  top: 12px;
  max-height: calc(100vh - 180px);
  overflow-y: auto;
}
.sdp-left-hero-card::-webkit-scrollbar {
  width: 0;
  height: 0;
}
.scheme-watercolor_light .sdp-left-hero-card,
.scheme-light .sdp-left-hero-card {
  background: rgba(255, 255, 255, 0.72);
  box-shadow: 0 14px 34px rgba(24, 26, 32, 0.06);
}
.sdp-cover-showcase {
  width: 100%;
  aspect-ratio: 1 / 1;
  border-radius: var(--bm-r-lg, 18px);
  overflow: hidden;
  position: relative;
  box-shadow: 0 18px 40px -12px rgba(15, 23, 42, 0.28);
  background: var(--grad-kugou-primary);
}
.sdp-cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.sdp-cover-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
.sdp-title {
  font-size: 21px;
  font-weight: var(--bm-fw-heavy, 800);
  line-height: 1.3;
  margin: 0;
  color: var(--text-primary);
}
.sdp-creator-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.sdp-creator-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-bold, 700);
  padding: 2px 8px;
  border-radius: var(--bm-r-xs, 6px);
  background: color-mix(in srgb, var(--primary) 13%, transparent);
  color: var(--primary);
}
.sdp-creator-name {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
}
.sdp-intro-box {
  padding: 10px 12px;
  border-radius: var(--bm-r-md, 14px);
  background: color-mix(in srgb, var(--text-primary) 4.5%, transparent);
  border: none;
  cursor: default;
  interpolate-size: allow-keywords;
}
.sdp-intro-box.is-expandable {
  cursor: pointer;
}
.sdp-intro-text {
  margin: 0;
  font-size: var(--bm-fs-caption, 12.5px);
  line-height: 1.6;
  color: var(--text-muted);
  height: auto;
  overflow: hidden;
  transition: height 0.32s cubic-bezier(0.22, 1, 0.36, 1);
}
.sdp-intro-box.is-expandable:not(.expanded) .sdp-intro-text {
  height: 4.8em;
  mask-image: linear-gradient(to bottom, #000 58%, transparent 100%);
  -webkit-mask-image: linear-gradient(to bottom, #000 58%, transparent 100%);
}
.sdp-intro-box.expanded .sdp-intro-text {
  height: auto;
  mask-image: none;
  -webkit-mask-image: none;
}
.sdp-intro-more {
  display: inline-block;
  margin-top: 4px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--primary);
}
.sdp-action-matrix {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 2px;
}
.sdp-primary-action-btn {
  width: 100%;
  height: 40px;
  border-radius: var(--bm-r-md, 14px);
  border: none;
  background: var(--grad-kugou-primary);
  color: #ffffff;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  box-shadow: 0 10px 22px color-mix(in srgb, var(--primary) 28%, transparent);
  transition: transform 0.22s cubic-bezier(0.22, 1, 0.36, 1), box-shadow 0.22s ease;
}
.sdp-primary-action-btn:hover:not(:disabled) {
  transform: translateY(-2px);
  box-shadow: 0 14px 26px color-mix(in srgb, var(--primary) 36%, transparent);
}
.sdp-primary-action-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.sdp-secondary-actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}
.sdp-sec-btn {
  height: 38px;
  padding: 0 12px;
  border-radius: var(--bm-r-sm, 10px);
  border: none;
  background: color-mix(in srgb, var(--text-primary) 6%, transparent);
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
  transition: transform 0.18s ease, background 0.18s ease, color 0.18s ease;
}
.sdp-sec-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  background: var(--primary-soft, color-mix(in srgb, var(--primary) 24%, transparent));
  color: var(--primary);
  }
.sdp-sec-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.sdp-sec-btn.active {
  background: var(--grad-kugou-primary, linear-gradient(135deg, var(--primary), var(--cyan)));
  color: #ffffff;
    box-shadow: 0 8px 20px -4px color-mix(in srgb, var(--primary) 24%, transparent);
}
.sdp-sec-btn.is-collected {
  background: rgba(255, 77, 109, 0.14);
  color: var(--coral);
  }
.sdp-sec-btn.is-collected:hover:not(:disabled) {
  background: rgba(255, 77, 109, 0.2);
  color: var(--coral);
  }
.sdp-sec-btn.danger:hover:not(:disabled) {
  background: rgba(244, 63, 94, 0.16);
  color: #f43f5e;
  box-shadow: 0 4px 14px -4px rgba(244, 63, 94, 0.28);
}
.sdp-right-tracks-pane {
  border-radius: var(--bm-r-xl, 24px);
  background: transparent;
  border: none;
  padding: 4px 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 0;
  container-type: inline-size;
}
.sdp-tracks-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}
.sdp-search-input-box {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 14px;
  height: 38px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--bg-glass);
  border: none;
  min-width: 240px;
  flex: 1;
  max-width: 340px;
  color: var(--text-secondary);
}
.sdp-search-input-box input {
  background: transparent;
  border: none;
  color: var(--text-primary);
  font-size: var(--bm-fs-meta, 13.5px);
  width: 100%;
  outline: none;
}
.sdp-search-input-box input::placeholder {
  color: var(--text-muted);
}
.clear-btn {
  background: none;
  border: none;
  color: var(--text-secondary);
  cursor: pointer;
}
.sdp-sort-pills {
  display: flex;
  gap: 4px;
  padding: 4px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--bg-glass);
  border: none;
  flex-wrap: wrap;
}
.sort-pill {
  border: none;
  background: transparent;
  color: var(--text-secondary);
  padding: 5px 13px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  transition: all 0.22s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.sort-pill.active {
  background: var(--grad-kugou-primary);
  color: #ffffff;
  box-shadow: 0 4px 12px -2px color-mix(in srgb, var(--primary) 34%, transparent);
}

/* 批量操作浮动控制栏 */
.batch-floating-action-bar {
  position: sticky;
  top: 72px;
  z-index: 28;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  padding: 10px 14px;
  border-radius: var(--bm-r-lg, 18px);
  background: color-mix(in srgb, var(--bg-glass) 92%, transparent);
  backdrop-filter: blur(18px);
  -webkit-backdrop-filter: blur(18px);
  border: none;
  box-shadow: 0 14px 34px rgba(0, 0, 0, 0.28);
}
.bfab-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.bfab-pill {
  height: 30px;
  padding: 0 11px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: color-mix(in srgb, var(--text-primary) 7%, transparent);
  color: var(--text-primary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
  transition: background 0.16s ease, color 0.16s ease;
}
.bfab-pill:hover:not(:disabled) {
  background: var(--primary-soft);
  color: var(--primary);
}
.bfab-pill:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.bfab-count {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
  margin-left: 2px;
}
.bfab-hint {
  font-weight: var(--bm-fw-medium, 500);
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
  margin-left: 4px;
}
.bfab-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.bfab-btn {
  height: 32px;
  padding: 0 12px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: color-mix(in srgb, var(--text-primary) 7%, transparent);
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
  transition: background 0.16s ease, transform 0.16s ease, color 0.16s ease;
}
.bfab-btn:hover:not(:disabled) {
  background: var(--primary-soft);
  color: var(--primary);
  transform: translateY(-1px);
}
.bfab-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.bfab-btn.primary {
    background: var(--grad-kugou-primary, linear-gradient(135deg, var(--primary), var(--cyan)));
  color: #ffffff;
}
.bfab-btn.primary:hover:not(:disabled) {
  color: #ffffff;
  box-shadow: 0 6px 16px color-mix(in srgb, var(--primary) 24%, transparent);
}
.bfab-btn.danger:hover:not(:disabled) {
  background: rgba(244, 63, 94, 0.16);
  color: #f43f5e;
  box-shadow: 0 4px 14px -4px rgba(244, 63, 94, 0.28);
}
.bfab-btn.ghost {
  background: transparent;
  color: var(--text-secondary);
}

/* 全局统一 2 列双行曲目网格 */
.frameless-songs-grid-2col {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px 14px;
}
@container (max-width: 780px) {
  .frameless-songs-grid-2col {
    grid-template-columns: minmax(0, 1fr);
  }
}
@media (max-width: 980px) {
  .frameless-songs-grid-2col {
    grid-template-columns: minmax(0, 1fr);
  }
}
/* .unified-song-row 统一样式已由 src/styles/skyline-components.css 提供 */
.sdp-empty-state {
  text-align: center;
  padding: 56px 20px;
  color: var(--text-secondary);
  font-size: var(--bm-fs-body, 14.5px);
}
.spin-anim {
  animation: playlist-spin 0.9s linear infinite;
}
@keyframes playlist-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

/* D2 丝绒暗紫 */
[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .sdp-left-hero-card {
  background: color-mix(in srgb, var(--bg-glass) 82%, transparent);
  box-shadow: 0 18px 40px rgba(8, 4, 16, 0.34);
}
[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .batch-floating-action-bar {
  background: color-mix(in srgb, var(--bg-glass) 92%, transparent);
  box-shadow: 0 16px 36px rgba(8, 4, 16, 0.42);
}

/* L1 棉纸水彩 */
[data-theme="watercolor_light"] .sdp-left-hero-card,
.scheme-watercolor_light .sdp-left-hero-card {
  background: rgba(255, 255, 255, 0.78);
  backdrop-filter: blur(22px) saturate(140%);
  -webkit-backdrop-filter: blur(22px) saturate(140%);
  box-shadow: 0 14px 34px rgba(24, 26, 32, 0.06);
}
[data-theme="watercolor_light"] .sdp-title,
.scheme-watercolor_light .sdp-title,
[data-theme="watercolor_light"] .bfab-count,
.scheme-watercolor_light .bfab-count {
  color: #181a20;
}
[data-theme="watercolor_light"] .sdp-creator-name,
.scheme-watercolor_light .sdp-creator-name,
[data-theme="watercolor_light"] .sdp-intro-text,
.scheme-watercolor_light .sdp-intro-text,
[data-theme="watercolor_light"] .bfab-hint,
.scheme-watercolor_light .bfab-hint {
  color: var(--bm-text-2);
}
[data-theme="watercolor_light"] .sdp-intro-box,
.scheme-watercolor_light .sdp-intro-box {
  background: rgba(24, 26, 32, 0.045);
}
[data-theme="watercolor_light"] .sdp-sec-btn,
.scheme-watercolor_light .sdp-sec-btn {
  background: rgba(255, 255, 255, 0.78);
  color: var(--bm-text-1);
  box-shadow: 0 4px 12px rgba(24, 26, 32, 0.04);
}
[data-theme="watercolor_light"] .sdp-sec-btn.is-collected,
.scheme-watercolor_light .sdp-sec-btn.is-collected {
  background: color-mix(in srgb, var(--coral) 13%, transparent);
  color: var(--coral);
}
[data-theme="watercolor_light"] .sdp-search-input-box,
.scheme-watercolor_light .sdp-search-input-box {
  background: rgba(255, 255, 255, 0.78);
  box-shadow: 0 4px 12px rgba(24, 26, 32, 0.04);
  color: var(--bm-text-2);
}
[data-theme="watercolor_light"] .sdp-search-input-box input,
.scheme-watercolor_light .sdp-search-input-box input {
  color: var(--bm-text-1);
}
[data-theme="watercolor_light"] .sdp-search-input-box input::placeholder,
.scheme-watercolor_light .sdp-search-input-box input::placeholder {
  color: var(--bm-text-3);
}
[data-theme="watercolor_light"] .batch-floating-action-bar,
.scheme-watercolor_light .batch-floating-action-bar {
  background: rgba(255, 255, 255, 0.92);
  box-shadow: 0 12px 28px rgba(24, 26, 32, 0.1);
}
[data-theme="watercolor_light"] .bfab-pill,
.scheme-watercolor_light .bfab-pill,
[data-theme="watercolor_light"] .bfab-btn:not(.primary),
.scheme-watercolor_light .bfab-btn:not(.primary) {
  background: rgba(24, 26, 32, 0.06);
  color: #181a20;
}
[data-theme="watercolor_light"] .sdp-empty-state,
.scheme-watercolor_light .sdp-empty-state {
  color: #788096;
}

/* L2 晨雾燕麦实色纸感 */
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-left-hero-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .batch-floating-action-bar {
  background: #fcf9f2;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow: 0 12px 28px -8px rgba(45, 38, 28, 0.09);
}
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-title,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .bfab-count,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-search-input-box input,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .bfab-pill,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .bfab-btn:not(.primary),
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-sec-btn {
  color: #221f1a;
}
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-creator-name,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-intro-text,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .bfab-hint,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sort-pill {
  color: #625b50;
}
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-search-input-box input::placeholder,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-empty-state {
  color: #8f8678;
}
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-intro-box,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-sec-btn:not(.is-collected),
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-search-input-box,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .sdp-sort-pills,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .bfab-pill,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .bfab-btn:not(.primary) {
  background: #e4ddd0;
  box-shadow: none;
}
</style>

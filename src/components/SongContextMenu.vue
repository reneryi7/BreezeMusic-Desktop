<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import type { AudioQualityValue } from "../types";
import { buildAudioDetailModel } from "../utils/audioDetailModel";
import PlumpIcon from "./PlumpIcon.vue";
import SongDetailGrid from "./SongDetailGrid.vue";

const store = useAppStore();
const menuElRef = ref<HTMLElement | null>(null);
const menuOrigin = ref("top left");

const unifiedSongDetailModel = computed(() => {
  const target = store.songDetailModalSong;
  const isCurrent =
    Boolean(target && store.currentSong) &&
    ((target?.hash && target.hash === store.currentSong?.hash) ||
      (target?.id && target.id === store.currentSong?.id));
  const detail =
    store.activeSongDetailInfo ||
    (isCurrent ? store.currentSongDetailInfo : null);
  return buildAudioDetailModel(
    target,
    detail,
    isCurrent ? store.currentPlaybackInfo : null
  );
});

function handleDetailEscapeKey(e: KeyboardEvent) {
  if (e.key === "Escape" && store.songDetailModalSong && !store.isNowPlayingOpen) {
    store.songDetailModalSong = null;
  }
}

onMounted(() => {
  window.addEventListener("keydown", handleDetailEscapeKey);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleDetailEscapeKey);
});

watch(
  () => [store.contextMenu.visible, store.contextMenu.x, store.contextMenu.y] as const,
  ([visible, rawX, rawY]) => {
    if (!visible) return;
    const origX = rawX;
    const origY = rawY;
    nextTick(() => {
      const el = menuElRef.value;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const pad = 12;
      const maxX = Math.max(pad, window.innerWidth - rect.width - pad);
      const maxY = Math.max(pad, window.innerHeight - rect.height - pad);
      const flipRight = origX > maxX;
      const flipBottom = origY > maxY;
      menuOrigin.value = `${flipBottom ? "bottom" : "top"} ${flipRight ? "right" : "left"}`;
      if (store.contextMenu.x > maxX) store.contextMenu.x = maxX;
      if (store.contextMenu.y > maxY) store.contextMenu.y = maxY;
      if (store.contextMenu.x < pad) store.contextMenu.x = pad;
      if (store.contextMenu.y < pad) store.contextMenu.y = pad;
    });
  }
);

function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return "流式直连解析";
  return (bytes / (1024 * 1024)).toFixed(2) + " MB";
}

const canRemoveFromCurrentPlaylist = computed(() => {
  if (
    store.activeTab !== "playlist_detail" &&
    (store.activeTab !== "library" || store.libraryDetailMode !== "playlist")
  ) {
    return false;
  }
  if (store.selectedPlaylist?.isLiked) return true;
  if (store.selectedPlaylist?.isCreatedByMe && store.selectedPlaylist?.listId) {
    return true;
  }
  return false;
});

function handlePlayNow() {
  if (store.contextMenu.song) {
    store.playSongFromList(store.contextMenu.song);
  }
  store.closeSongContextMenu();
}

function handlePlayNext() {
  if (store.contextMenu.song) {
    store.insertPlayNext(store.contextMenu.song);
  }
  store.closeSongContextMenu();
}

function handleAppendQueue() {
  if (store.contextMenu.song) {
    store.appendSongToQueue(store.contextMenu.song);
  }
  store.closeSongContextMenu();
}

function handleToggleLike() {
  if (store.contextMenu.song) {
    store.toggleFavoriteSong(store.contextMenu.song);
  }
  store.closeSongContextMenu();
}

function handleOpenAddToPlaylist() {
  if (store.contextMenu.song) {
    store.addToPlaylistModalSong = store.contextMenu.song;
  }
  store.closeSongContextMenu();
}

function handleRemoveFromPlaylist() {
  if (store.contextMenu.song) {
    store.removeSongFromCurrentPlaylist(store.contextMenu.song);
  }
  store.closeSongContextMenu();
}

function handleViewArtist() {
  const s = store.contextMenu.song;
  store.closeSongContextMenu();
  if (s) {
    store.navigateToSongArtist(s);
  }
}

function handleViewAlbum() {
  const s = store.contextMenu.song;
  store.closeSongContextMenu();
  if (s) {
    store.navigateToSongAlbum(s);
  }
}

function handleOpenComments() {
  const s = store.contextMenu.song;
  store.closeSongContextMenu();
  if (s) {
    store.openCommentsModal(s);
  }
}

function handlePreviewHdCover() {
  const s = store.contextMenu.song;
  if (s?.coverUrl) {
    store.hdArtworkPreviewUrl = s.coverUrl.replace("/400/", "/1000/");
  }
  store.closeSongContextMenu();
}

function handleShareSong() {
  const s = store.contextMenu.song;
  if (s) {
    store.shareModalSong = s;
  }
  store.closeSongContextMenu();
}

function handleOpenDetail() {
  if (store.contextMenu.song) {
    store.openSongDetailModal(store.contextMenu.song);
  }
  store.closeSongContextMenu();
}

function handleOpenDownloadModal() {
  if (store.contextMenu.song) {
    store.downloadModalSong = store.contextMenu.song;
  }
  store.closeSongContextMenu();
}

function handleSaveLyricOnlyFromMenu() {
  if (store.contextMenu.song) {
    store.saveSongLyricOnly(store.contextMenu.song);
  }
  store.closeSongContextMenu();
}

const activeQueueTask = computed(
  () =>
    store.downloadQueue.find(
      (t) =>
        t.status === "resolving" ||
        t.status === "downloading" ||
        t.status === "tagging" ||
        t.status === "queued"
    ) || null
);

const downloadTierChoices = computed(() => {
  const s = store.downloadModalSong;
  if (!s) return [];
  const tiers: {
    val: AudioQualityValue;
    label: string;
    badge: string;
    sizeText: string;
    available: boolean;
  }[] = [
    {
      val: "high",
      label: "Hi-Res 高解析度母带 (24bit FLAC)",
      badge: "Hi-Res",
      sizeText: formatBytes(s.sizeHiRes || 36_500_000),
      available: Boolean(s.isHiRes || s.hashHiRes),
    },
    {
      val: "flac",
      label: "SQ 无损音质 (16bit/44.1kHz FLAC)",
      badge: "SQ",
      sizeText: formatBytes(s.sizeFlac || 24_800_000),
      available: Boolean(s.isLossless || s.hashFlac || s.isHiRes),
    },
    {
      val: "320",
      label: "HQ 高品音质 (320kbps MP3)",
      badge: "320K",
      sizeText: formatBytes(s.size320 || 9_200_000),
      available: true,
    },
    {
      val: "128",
      label: "标准音质 (128kbps MP3)",
      badge: "128K",
      sizeText: formatBytes(s.size128 || 3_800_000),
      available: true,
    },
  ];
  return tiers;
});
</script>

<template>
  <!-- 1. 全局鼠标右键上下文菜单（四组结构化精简菜单 · 触点四象限弹簧绽放） -->
  <transition name="bm-ctx-spring">
    <div
      v-if="store.contextMenu.visible && store.contextMenu.song"
      ref="menuElRef"
      class="song-context-menu"
      :style="{
        left: `${store.contextMenu.x}px`,
        top: `${store.contextMenu.y}px`,
        transformOrigin: menuOrigin,
      }"
      @click.stop
    >
      <div class="scm-head">
        <img
          v-if="store.contextMenu.song.coverUrl"
          :src="store.contextMenu.song.coverUrl"
          class="scm-cover"
          alt=""
        />
        <div class="scm-meta">
          <div class="scm-title">{{ store.contextMenu.song.title }}</div>
          <div class="scm-artist">{{ store.contextMenu.song.artist }}</div>
        </div>
      </div>

      <div class="scm-divider" role="separator"></div>

      <!-- 组 1：播放队列 -->
      <button class="scm-item" @click="handlePlayNow">
        <PlumpIcon name="play" :size="15" color="var(--primary)" />
        <span class="scm-item-label">立即播放</span>
      </button>

      <button class="scm-item" @click="handlePlayNext">
        <PlumpIcon name="top" :size="15" :accent="'var(--cyan)'" />
        <span class="scm-item-label">下一首播放</span>
      </button>

      <button class="scm-item" @click="handleAppendQueue">
        <PlumpIcon name="queue" :size="15" />
        <span class="scm-item-label">加入播放队列</span>
      </button>

      <div class="scm-divider" role="separator"></div>

      <!-- 组 2：收藏与歌单 -->
      <button class="scm-item" @click="handleToggleLike">
        <PlumpIcon
          :name="store.isSongLiked(store.contextMenu.song) ? 'heart-filled' : 'heart'"
          :size="15"
          :color="
            store.isSongLiked(store.contextMenu.song)
              ? '#ff4d6d'
              : 'currentColor'
          "
        />
        <span class="scm-item-label">
          {{
            store.isSongLiked(store.contextMenu.song)
              ? "取消喜欢"
              : "加入我喜欢"
          }}
        </span>
      </button>

      <button class="scm-item" @click="handleOpenAddToPlaylist">
        <PlumpIcon name="playlist-add" :size="15" />
        <span class="scm-item-label">收藏到歌单...</span>
      </button>

      <button
        v-if="canRemoveFromCurrentPlaylist"
        class="scm-item is-danger"
        @click="handleRemoveFromPlaylist"
      >
        <PlumpIcon name="close" :size="15" color="var(--coral)" />
        <span class="scm-item-label">从当前歌单移除</span>
      </button>

      <div class="scm-divider" role="separator"></div>

      <!-- 组 3：跳转歌手与专辑 -->
      <button class="scm-item" @click="handleViewArtist">
        <PlumpIcon name="user" :size="15" />
        <span class="scm-item-label">查看歌手：{{ store.contextMenu.song.artist }}</span>
      </button>

      <button
        v-if="store.contextMenu.song.album"
        class="scm-item"
        @click="handleViewAlbum"
      >
        <PlumpIcon name="disc" :size="15" />
        <span class="scm-item-label">查看专辑：{{ store.contextMenu.song.album }}</span>
      </button>

      <div class="scm-divider" role="separator"></div>

      <!-- 组 4：详情与分享 -->
      <button class="scm-item" @click="handleOpenComments">
        <PlumpIcon name="comment" :size="15" />
        <span class="scm-item-label">查看歌曲评论</span>
      </button>

      <button
        v-if="store.contextMenu.song.coverUrl"
        class="scm-item"
        @click="handlePreviewHdCover"
      >
        <PlumpIcon name="image" :size="15" />
        <span class="scm-item-label">查看高清封面</span>
      </button>

      <button class="scm-item" @click="handleShareSong">
        <PlumpIcon name="share" :size="15" />
        <span class="scm-item-label">分享歌曲...</span>
      </button>

      <button class="scm-item" @click="handleOpenDetail">
        <PlumpIcon name="wave" :size="15" />
        <span class="scm-item-label">歌曲详情与参数</span>
      </button>

      <!-- 仅当用户解锁实验性离线保存模块后才显示离线项 -->
      <template v-if="store.downloadFeatureUnlocked">
        <button
          class="scm-item highlight"
          @click="handleOpenDownloadModal"
        >
          <PlumpIcon name="download" :size="15" color="var(--primary)" accent="var(--cyan)" />
          <span class="scm-item-label">下载歌曲 / 歌词...</span>
        </button>
        <button
          class="scm-item"
          @click="handleSaveLyricOnlyFromMenu"
        >
          <PlumpIcon name="lyric" :size="15" color="var(--violet)" />
          <span class="scm-item-label">仅保存 .lrc 歌词</span>
        </button>
      </template>
    </div>
  </transition>

  <!-- 2. 添加到自建歌单弹窗已统一抽取至 AddToPlaylistModal.vue（App.vue 全局单例挂载） -->

  <!-- 3. 音频流与母带元数据百科详情弹窗 (与全屏播放页 NowPlayingDialogs.vue 共享 12 维度数据模型与组件) -->
  <transition name="fade">
    <div
      v-if="store.songDetailModalSong && !store.isNowPlayingOpen"
      class="modal-backdrop"
      @click.self="store.songDetailModalSong = null"
    >
      <div
        class="compact-glass-modal song-detail-unified-modal"
        aria-labelledby="scm-detail-title"
      >
        <div class="modal-header">
          <div>
            <h3 id="scm-detail-title">音频流与母带元数据</h3>
            <p class="vip-sub-caption">发行档案与硬件级声学流实时解析</p>
          </div>
          <button
            type="button"
            class="icon-close-btn"
            aria-label="关闭歌曲详情弹窗"
            @click="store.songDetailModalSong = null"
          >
            <PlumpIcon name="close" :size="15" />
          </button>
        </div>

        <div class="song-spec-sheet">
          <div class="spec-hero-row">
            <img
              v-if="store.songDetailModalSong.coverUrl"
              :src="store.songDetailModalSong.coverUrl"
              class="spec-cover"
              alt=""
            />
            <div class="spec-hero-copy">
              <div class="spec-song-title">
                {{ unifiedSongDetailModel.title }}
              </div>
              <div class="spec-song-artist">
                {{ unifiedSongDetailModel.artist }}
              </div>
              <div class="spec-badges-row">
                <span class="mini-badge hires">
                  {{ store.songDetailModalSong.highestQuality || "标准" }}
                </span>
                <span v-if="unifiedSongDetailModel.hasAccompaniment" class="mini-badge sq">
                  支持伴奏
                </span>
                <span v-if="store.isLoadingSongDetail" class="mini-badge vip">
                  正在读取信息...
                </span>
              </div>
            </div>
          </div>

          <SongDetailGrid
            :model="unifiedSongDetailModel"
            :loading="store.isLoadingSongDetail"
            variant="adaptive"
          />
        </div>
      </div>
    </div>
  </transition>

  <!-- 4. 多音质下载档位选择与二进制标签内嵌弹窗 (播放页开启时由 NowPlayingDialogs.vue 统一接管) -->
  <transition name="fade">
    <div
      v-if="store.downloadModalSong && !store.isNowPlayingOpen"
      class="modal-backdrop"
      @click.self="store.downloadModalSong = null"
    >
      <div class="compact-glass-modal" style="width: 480px">
        <div class="modal-header">
          <div>
            <h3>下载歌曲</h3>
            <p class="vip-sub-caption">
              支持多音质选择，自动内嵌高清封面、歌曲标签与歌词
            </p>
          </div>
          <button
            class="icon-close-btn"
            @click="store.downloadModalSong = null"
          >
            <PlumpIcon name="close" :size="15" />
          </button>
        </div>

        <div class="spec-hero-row">
          <img
            v-if="store.downloadModalSong.coverUrl"
            :src="store.downloadModalSong.coverUrl"
            class="spec-cover"
          />
          <div>
            <div class="spec-song-title">
              {{ store.downloadModalSong.title }}
            </div>
            <div class="spec-song-artist">
              {{ store.downloadModalSong.artist }} ·
              {{ store.downloadModalSong.album || "单曲" }}
            </div>
          </div>
        </div>

        <!-- 行内快捷内嵌与防重复下载开关（与播放页下载弹窗 4 项完全对齐） -->
        <div style="display: flex; flex-wrap: wrap; gap: 8px; margin: 10px 0">
          <label class="bg-pill-btn" style="cursor: pointer; display: inline-flex; align-items: center; gap: 6px">
            <input v-model="store.embedCoverAndTags" type="checkbox" />
            <span>内嵌高清封面与元数据</span>
          </label>
          <label class="bg-pill-btn" style="cursor: pointer; display: inline-flex; align-items: center; gap: 6px">
            <input v-model="store.embedLyricsInAudio" type="checkbox" />
            <span>内嵌歌词到音频</span>
          </label>
          <label class="bg-pill-btn" style="cursor: pointer; display: inline-flex; align-items: center; gap: 6px">
            <input v-model="store.exportStandaloneLrc" type="checkbox" />
            <span>导出同名 .lrc 歌词</span>
          </label>
          <label class="bg-pill-btn" style="cursor: pointer; display: inline-flex; align-items: center; gap: 6px">
            <input v-model="store.skipExistingDownloaded" type="checkbox" />
            <span>跳过同名已下载文件</span>
          </label>
        </div>

        <div class="playlist-pick-list">
          <button
            v-for="tier in downloadTierChoices"
            :key="tier.val"
            class="playlist-pick-row"
            :style="{ opacity: tier.available ? 1 : 0.65 }"
            @click="
              store.triggerDownloadSong(store.downloadModalSong!, tier.val)
            "
          >
            <div class="ppr-left">
              <span
                class="mini-badge"
                :class="
                  tier.val === 'high'
                    ? 'hires'
                    : tier.val === 'flac'
                    ? 'sq'
                    : 'vip'
                "
              >
                {{ tier.badge }}
              </span>
              <div>
                <div class="ppr-title">{{ tier.label }}</div>
                <div class="ppr-count">预估大小：{{ tier.sizeText }}</div>
              </div>
            </div>
            <PlumpIcon name="download" :size="16" color="var(--cyan)" />
          </button>
        </div>

        <!-- 统一归口的歌词单独保存与复制操作区 -->
        <div
          style="
            display: flex;
            align-items: center;
            justify-content: space-between;
            gap: 8px;
            flex-wrap: wrap;
            margin-top: 12px;
            padding-top: 10px;
          "
        >
          <button
            type="button"
            class="bms-btn primary sm"
            :disabled="store.isSavingLyricOnly"
            @click="
              store.downloadModalSong &&
                store.saveSongLyricOnly(store.downloadModalSong)
            "
          >
            <PlumpIcon name="lyric" :size="13" color="#ffffff" />
            <span>{{ store.isSavingLyricOnly ? "正在保存 .lrc..." : "仅保存 .lrc 歌词文件" }}</span>
          </button>
          <button
            type="button"
            class="bms-btn ghost sm"
            @click="
              store.downloadModalSong &&
                store.copySongLrcToClipboard(store.downloadModalSong)
            "
          >
            <span>复制标准 .lrc</span>
          </button>
        </div>

        <div
          v-if="activeQueueTask"
          role="status"
          aria-live="polite"
          style="
            margin-top: 10px;
            padding: 10px 12px;
            border-radius: 14px;
            background: var(--bg-surface);
            display: flex;
            flex-direction: column;
            gap: 6px;
          "
        >
          <div style="display: flex; justify-content: space-between; font-size: 12px; font-weight: 700; color: var(--text-secondary);">
            <span>正在下载：{{ activeQueueTask.song.title }}</span>
            <span style="color: var(--primary); font-variant-numeric: tabular-nums;">{{ Math.round(activeQueueTask.percent) }}%</span>
          </div>
          <progress
            class="bms-native-progress"
            :max="100"
            :value="
              activeQueueTask.status === 'downloading'
                ? Math.round(activeQueueTask.percent)
                : undefined
            "
            :aria-label="`下载进度：${activeQueueTask.song.title}`"
          ></progress>
        </div>
      </div>
    </div>
  </transition>
</template>

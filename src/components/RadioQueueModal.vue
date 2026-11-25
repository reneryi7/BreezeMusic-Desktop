<template>
  <Teleport to="body">
    <Transition name="rqm-modal-fade">
      <div
        v-if="open && channel"
        class="rqm-backdrop"
        @click="emit('close')"
      >
        <div
          class="rqm-dialog"
          role="dialog"
          aria-modal="true"
          :aria-label="`电台队列预览 · ${channel.name}`"
          @click.stop
        >
          <!-- 顶部电台信息 -->
          <div class="rqm-header">
            <div class="rqm-cover-stage" :class="{ playing: isPlayingChannel }">
              <img
                v-if="displayCover"
                :src="displayCover"
                class="rqm-cover-img"
                draggable="false"
                alt=""
              />
              <div v-else class="rqm-cover-fallback">
                <PlumpIcon name="radio" :size="30" color="#fff" />
              </div>
              <span v-if="isPlayingChannel" class="rqm-cover-ring" aria-hidden="true"></span>
            </div>

            <div class="rqm-meta">
              <div class="rqm-badges-row">
                <span class="rqm-freq-pill">{{ channel.freqLabel || "FM 98.8" }}</span>
                <span v-if="channel.category" class="rqm-cat-pill">{{ channel.category }}</span>
                <span v-if="channel.listenerCountText" class="rqm-stat-text">
                  {{ channel.listenerCountText }}
                </span>
              </div>
              <h3 class="rqm-title">{{ channel.name }}</h3>
              <p class="rqm-desc">
                {{ channel.description || "实时调频筛选同氛围曲目，点击曲目即可点播或一键加入播放队列。" }}
              </p>
            </div>

            <button
              type="button"
              class="rqm-close-btn"
              title="关闭弹窗"
              @click="emit('close')"
            >
              <PlumpIcon name="close" :size="16" />
            </button>
          </div>

          <!-- 中部待播队列列表（统一双行歌单行契约） -->
          <div class="rqm-queue-section">
            <div class="rqm-queue-bar">
              <span class="rqm-queue-label">当前待播队列</span>
              <span class="rqm-queue-count">
                {{ loading && songs.length === 0 ? "正在调频获取..." : `${songs.length} 首候选曲目` }}
              </span>
            </div>

            <div class="rqm-queue-scroll">
              <div v-if="loading && songs.length === 0" class="rqm-skeleton-list">
                <div v-for="i in 6" :key="i" class="rqm-skeleton-row"></div>
              </div>

              <div v-else-if="songs.length === 0" class="rqm-empty-state">
                <PlumpIcon name="radio" :size="28" />
                <span>当前频段暂无候选曲目，可点击「刷新」重新拉取</span>
              </div>

              <div v-else class="frameless-songs-grid-2col">
                <div
                  v-for="(song, idx) in songs"
                  :key="`${song.hash}_${idx}`"
                  class="unified-song-row"
                  :class="{ active: store.currentSong?.hash === song.hash }"
                  @click="emit('play-song', song)"
                  @contextmenu.prevent="store.openSongContextMenu($event, song, songs)"
                >
                  <div class="usr-index-slot">
                    <span
                      v-if="store.currentSong?.hash === song.hash && store.isPlaying"
                      class="mini-eq-bars"
                    >
                      <i></i><i></i><i></i>
                    </span>
                    <span v-else class="usr-index-num">{{ String(idx + 1).padStart(2, "0") }}</span>
                  </div>

                  <div class="usr-cover-wrap">
                    <img
                      v-if="song.coverUrl"
                      :src="song.coverUrl"
                      loading="lazy"
                      draggable="false"
                      alt=""
                    />
                  </div>

                  <div class="usr-meta-stack">
                    <div class="usr-line-1">
                      <span class="usr-title">{{ song.title }}</span>
                    </div>
                    <div class="usr-line-2">
                      <span class="usr-badges">
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
                        <span
                          class="usr-artist is-clickable"
                          @click.stop="handleNavigateArtist(song)"
                        >
                          {{ song.artist }}
                        </span>
                        <template v-if="song.album">
                          <span class="usr-sep"> · </span>
                          <span
                            class="usr-album is-clickable"
                            @click.stop="handleNavigateAlbum(song)"
                          >
                            {{ song.album }}
                          </span>
                        </template>
                      </span>
                    </div>
                  </div>

                  <div class="usr-right-zone">
                    <div class="usr-hover-actions" @click.stop>
                      <button
                        type="button"
                        class="usr-action-btn"
                        :class="{ liked: store.isSongLiked(song.hash) }"
                        :data-tip="store.isSongLiked(song.hash) ? '取消喜欢' : '收藏到我喜欢'"
                        @click="store.toggleFavoriteSong(song)"
                      >
                        <PlumpIcon
                          :name="store.isSongLiked(song.hash) ? 'heart-filled' : 'heart'"
                          :size="15"
                        />
                      </button>
                      <button
                        type="button"
                        class="usr-action-btn"
                        data-tip="添加到播放队列末尾"
                        @click="store.appendSongToQueue(song)"
                      >
                        <PlumpIcon name="plus" :size="15" />
                      </button>
                    </div>
                    <span class="usr-duration">{{ formatDuration(song) }}</span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- 底部严格 4 个核心操作按钮：播放、刷新、追加、跳转电台广场 -->
          <div class="rqm-footer">
            <div class="rqm-footer-primary-group">
              <button
                type="button"
                class="rqm-btn rqm-btn-primary"
                :disabled="loading && songs.length === 0"
                @click="emit('play')"
              >
                <PlumpIcon :name="isPlayingChannel ? 'pause' : 'play'" :size="15" color="#fff" />
                <span>{{ isPlayingChannel ? "暂停" : "播放" }}</span>
              </button>
              <button
                type="button"
                class="rqm-btn rqm-btn-secondary"
                :disabled="loading"
                @click="emit('refresh')"
              >
                <PlumpIcon
                  name="refresh"
                  :size="15"
                  :class="{ 'is-spinning': loading }"
                />
                <span>刷新</span>
              </button>
              <button
                type="button"
                class="rqm-btn rqm-btn-secondary"
                :disabled="songs.length === 0"
                @click="emit('append')"
              >
                <PlumpIcon name="plus" :size="15" />
                <span>追加</span>
              </button>
            </div>

            <button
              type="button"
              class="rqm-btn rqm-btn-ghost"
              @click="emit('go-plaza')"
            >
              <PlumpIcon name="radio" :size="15" />
              <span>跳转电台广场</span>
              <PlumpIcon name="chevron-right" :size="14" />
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted } from "vue";
import { useAppStore } from "../stores/appStore";
import { formatDuration, resolveSongQualityBadge } from "../api/kugou";
import type { FmChannel, Song } from "../types";
import PlumpIcon from "./PlumpIcon.vue";

const props = defineProps<{
  open: boolean;
  channel: FmChannel | null;
  songs: Song[];
  loading: boolean;
  coverUrl?: string;
  isPlayingChannel?: boolean;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "play"): void;
  (e: "refresh"): void;
  (e: "append"): void;
  (e: "go-plaza"): void;
  (e: "play-song", song: Song): void;
}>();

const store = useAppStore();

const displayCover = computed(
  () => props.coverUrl || props.channel?.coverUrl || props.songs[0]?.coverUrl || ""
);

function handleNavigateArtist(song: Song) {
  emit("close");
  store.navigateToSongArtist(song);
}

function handleNavigateAlbum(song: Song) {
  emit("close");
  store.navigateToSongAlbum(song);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && props.open) {
    emit("close");
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
});
</script>

<style scoped>
.rqm-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.rqm-dialog {
  width: min(860px, calc(100vw - 40px));
  max-height: 78vh;
  border-radius: var(--bm-r-xl, 24px);
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 18px;
  background: rgba(18, 19, 26, 0.98);
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  border: none;
  box-shadow: 0 28px 64px rgba(0, 0, 0, 0.54);
  color: var(--text-primary, #f5f5f7);
}

.rqm-header {
  position: relative;
  display: flex;
  align-items: center;
  gap: 16px;
  padding-right: 40px;
}

.rqm-cover-stage {
  position: relative;
  width: 76px;
  height: 76px;
  flex-shrink: 0;
  border-radius: var(--bm-r-lg, 18px);
  overflow: hidden;
  background: linear-gradient(135deg, #0d4a3e, #0d9488);
  box-shadow: 0 10px 24px rgba(13, 148, 136, 0.28);
}

.rqm-cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.rqm-cover-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.rqm-cover-ring {
  position: absolute;
  inset: -4px;
  border-radius: var(--bm-r-lg, 18px);
  border: none;
  pointer-events: none;
  animation: rqmBreatheRing 2.2s ease-in-out infinite;
}

@keyframes rqmBreatheRing {
  0%,
  100% {
    opacity: 0.45;
    transform: scale(0.98);
  }
  50% {
    opacity: 1;
    transform: scale(1.02);
  }
}

.rqm-meta {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.rqm-badges-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.rqm-freq-pill {
  padding: 2px 9px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  letter-spacing: 0.04em;
  background: rgba(45, 212, 191, 0.16);
  color: #5eead4;
  border: none;
}

.rqm-cat-pill {
  padding: 2px 8px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  background: rgba(255, 255, 255, 0.09);
  color: var(--bm-text-2, #cbd5e1);
}

.rqm-stat-text {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--bm-text-3, #94a3b8);
}

.rqm-title {
  margin: 0;
  font-size: 20px;
  font-weight: 800;
  line-height: 1.25;
  color: var(--bm-text-1, #f8fafc);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.rqm-desc {
  margin: 0;
  font-size: var(--bm-fs-meta, 13.5px);
  line-height: 1.45;
  color: var(--bm-text-2, #cbd5e1);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.rqm-close-btn {
  position: absolute;
  top: 0;
  right: 0;
  width: 32px;
  height: 32px;
  border-radius: var(--bm-r-pill, 999px);
  border: none;
  background: rgba(255, 255, 255, 0.07);
  color: var(--bm-text-2, #cbd5e1);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: transform 0.18s ease, background 0.18s ease, color 0.18s ease;
}

.rqm-close-btn:hover {
  background: rgba(255, 255, 255, 0.16);
  color: var(--bm-text-1, #ffffff);
  transform: scale(1.06);
}

.rqm-queue-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  flex: 1;
}

.rqm-queue-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 4px;
}

.rqm-queue-label {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 700;
  color: var(--bm-text-1, #f8fafc);
}

.rqm-queue-count {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--bm-text-3, #94a3b8);
}

.rqm-queue-scroll {
  max-height: 380px;
  overflow-y: auto;
  padding-right: 4px;
  container-type: inline-size;
}

.rqm-skeleton-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.rqm-skeleton-row {
  height: 54px;
  border-radius: var(--bm-r-md, 14px);
  background: rgba(255, 255, 255, 0.06);
  animation: rqmPulse 1.3s ease-in-out infinite;
}

@keyframes rqmPulse {
  0%,
  100% {
    opacity: 0.45;
  }
  50% {
    opacity: 0.85;
  }
}

.rqm-empty-state {
  min-height: 160px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  font-size: var(--bm-fs-meta, 13.5px);
  color: var(--bm-text-2, #cbd5e1);
}

.rqm-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-top: 12px;
  border-top: none;
}

.rqm-footer-primary-group {
  display: flex;
  align-items: center;
  gap: 10px;
}

.rqm-btn {
  height: 36px;
  padding: 0 15px;
  border-radius: var(--bm-r-pill, 999px);
  background-clip: padding-box;
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: 650;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  cursor: pointer;
  border: none;
  transition: transform 0.18s ease, box-shadow 0.18s ease, background 0.18s ease, opacity 0.18s ease;
}

.rqm-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.rqm-btn-primary {
  background: var(--bm-accent-jade, #0d9488);
  color: #ffffff;
  box-shadow: 0 6px 18px rgba(13, 148, 136, 0.34);
}

.rqm-btn-primary:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 8px 22px rgba(13, 148, 136, 0.46);
}

.rqm-btn-secondary {
  background: rgba(255, 255, 255, 0.08);
  background: rgba(255, 255, 255, 0.08);
  color: var(--bm-text-1, #f8fafc);
}

.rqm-btn-secondary:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.15);
  transform: translateY(-1px);
}

.rqm-btn-ghost {
  background: rgba(13, 148, 136, 0.12);
  box-shadow: inset 0 0 0 0 transparent;
  color: #5eead4;
}

.rqm-btn-ghost:hover {
  background: rgba(13, 148, 136, 0.22);
  transform: translateY(-1px);
}

.is-clickable {
  cursor: pointer;
  border-radius: var(--bm-r-xs, 6px);
  padding: 1px 5px;
  margin: -1px -5px;
  text-decoration: none;
  transition: color 0.16s ease, background-color 0.16s ease;
}

.is-clickable:hover {
  color: var(--primary);
  background: color-mix(in srgb, var(--primary) 11%, transparent);
}

.rqm-modal-fade-enter-active,
.rqm-modal-fade-leave-active {
  transition: opacity 0.22s ease;
}

.rqm-modal-fade-enter-active .rqm-dialog,
.rqm-modal-fade-leave-active .rqm-dialog {
  transition: transform 0.24s cubic-bezier(0.22, 1, 0.36, 1), opacity 0.22s ease;
}

.rqm-modal-fade-enter-from,
.rqm-modal-fade-leave-to {
  opacity: 0;
}

.rqm-modal-fade-enter-from .rqm-dialog,
.rqm-modal-fade-leave-to .rqm-dialog {
  opacity: 0;
  transform: translateY(10px) scale(0.97);
}

[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .rqm-dialog,
.scheme-abyssal_dark.dark-variant-velvet_nebula .rqm-dialog {
  background: rgba(26, 20, 32, 0.98);
  color: #f5eff7;
}

[data-theme="watercolor_light"] .rqm-backdrop,
[data-color-scheme="light"] .rqm-backdrop,
.scheme-watercolor_light .rqm-backdrop,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-backdrop,
.scheme-light .rqm-backdrop {
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

[data-theme="watercolor_light"] .rqm-dialog,
[data-color-scheme="light"] .rqm-dialog,
.scheme-watercolor_light .rqm-dialog,
.scheme-light .rqm-dialog {
  background: rgba(255, 255, 255, 0.96);
  box-shadow: 0 24px 60px rgba(0, 0, 0, 0.14);
  color: var(--text-primary);
}

[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-dialog,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-dialog {
  background: #fcf9f2;
  box-shadow: 0 24px 60px rgba(45, 36, 22, 0.16);
  color: #221f1a;
}

[data-theme="watercolor_light"] .rqm-title,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-title,
.scheme-watercolor_light .rqm-title,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-title,
.scheme-light .rqm-title,
[data-theme="watercolor_light"] .rqm-queue-label,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-queue-label,
.scheme-watercolor_light .rqm-queue-label,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-queue-label,
.scheme-light .rqm-queue-label {
  color: var(--text-primary);
}

[data-theme="watercolor_light"] .rqm-desc,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-desc,
.scheme-watercolor_light .rqm-desc,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-desc,
.scheme-light .rqm-desc,
[data-theme="watercolor_light"] .rqm-queue-count,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-queue-count,
.scheme-watercolor_light .rqm-queue-count,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-queue-count,
.scheme-light .rqm-queue-count,
[data-theme="watercolor_light"] .rqm-stat-text,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-stat-text,
.scheme-watercolor_light .rqm-stat-text,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-stat-text,
.scheme-light .rqm-stat-text {
  color: var(--text-secondary);
}

[data-theme="watercolor_light"] .rqm-freq-pill,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-freq-pill,
.scheme-watercolor_light .rqm-freq-pill,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-freq-pill,
.scheme-light .rqm-freq-pill {
  background: color-mix(in srgb, var(--mint) 14%, transparent);
  color: var(--mint);
}

[data-theme="watercolor_light"] .rqm-cat-pill,
.scheme-watercolor_light .rqm-cat-pill,
.scheme-light .rqm-cat-pill {
  background: rgba(0, 0, 0, 0.05);
  color: var(--text-secondary);
}

[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-cat-pill,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-cat-pill {
  background: #e4ddd0;
  color: #625b50;
}

[data-theme="watercolor_light"] .rqm-close-btn,
.scheme-watercolor_light .rqm-close-btn,
.scheme-light .rqm-close-btn {
  background: rgba(0, 0, 0, 0.05);
  color: var(--text-secondary);
}

[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-close-btn,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-close-btn {
  background: #e4ddd0;
  color: #625b50;
}

[data-theme="watercolor_light"] .rqm-close-btn:hover,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-close-btn:hover,
.scheme-watercolor_light .rqm-close-btn:hover,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-close-btn:hover,
.scheme-light .rqm-close-btn:hover {
  background: var(--bg-hover-square);
  color: var(--text-primary);
}

[data-theme="watercolor_light"] .rqm-btn-secondary,
.scheme-watercolor_light .rqm-btn-secondary,
.scheme-light .rqm-btn-secondary {
  background: rgba(0, 0, 0, 0.055);
  color: var(--text-primary);
}

[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-btn-secondary,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-btn-secondary {
  background: #e4ddd0;
  color: #221f1a;
}

[data-theme="watercolor_light"] .rqm-btn-secondary:hover:not(:disabled),
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-btn-secondary:hover:not(:disabled),
.scheme-watercolor_light .rqm-btn-secondary:hover:not(:disabled),
.scheme-watercolor_light.light-variant-nordic_oat .rqm-btn-secondary:hover:not(:disabled),
.scheme-light .rqm-btn-secondary:hover:not(:disabled) {
  background: var(--bg-hover-square);
}

[data-theme="watercolor_light"] .rqm-btn-ghost,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-btn-ghost,
.scheme-watercolor_light .rqm-btn-ghost,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-btn-ghost,
.scheme-light .rqm-btn-ghost {
  background: color-mix(in srgb, var(--mint) 12%, transparent);
  color: var(--mint);
}

[data-theme="watercolor_light"] .rqm-btn-ghost:hover,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-btn-ghost:hover,
.scheme-watercolor_light .rqm-btn-ghost:hover,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-btn-ghost:hover,
.scheme-light .rqm-btn-ghost:hover {
  background: color-mix(in srgb, var(--mint) 20%, transparent);
}

[data-theme="watercolor_light"] .rqm-skeleton-row,
.scheme-watercolor_light .rqm-skeleton-row,
.scheme-light .rqm-skeleton-row {
  background: rgba(0, 0, 0, 0.055);
}

[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .rqm-skeleton-row,
.scheme-watercolor_light.light-variant-nordic_oat .rqm-skeleton-row {
  background: #e4ddd0;
}
</style>

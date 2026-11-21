<script setup lang="ts">
import { computed } from "vue";
import { useAppStore } from "../stores/appStore";
import type { SongAuthor } from "../types";
import { hexWithAlpha } from "../utils/coverPalette";
import { useFloatingModalTheme } from "../composables/useFloatingModalTheme";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();
const {
  isLightSurface,
  accentPrimary,
  accentSecondary,
  onAccentPrimary,
  modalSurfaceStyle,
} = useFloatingModalTheme("auto");

const dialogSurfaceStyle = computed<Record<string, string>>(() => ({
  ...modalSurfaceStyle.value,
  width: "min(450px, 92vw)",
  "--ma-primary": accentPrimary.value,
  "--ma-secondary": accentSecondary.value,
  "--ma-on-primary": onAccentPrimary.value,
}));

function chooseArtist(author: SongAuthor) {
  store.isMultiArtistModalOpen = false;
  store.navigateToSongArtist(null, author);
}
</script>

<template>
  <Teleport to="body">
    <transition name="fade">
      <div
        v-if="store.isMultiArtistModalOpen"
        class="ma-modal-backdrop"
        @click.self="store.isMultiArtistModalOpen = false"
      >
        <div
          class="multi-artist-tonal-modal"
          :class="{ 'is-dark-surface': !isLightSurface }"
          :style="dialogSurfaceStyle"
          @click.stop
        >
          <div class="ma-header">
            <div class="ma-title-row">
              <div
                class="ma-icon-orb"
                :style="{ background: hexWithAlpha(accentPrimary, 0.16) }"
              >
                <PlumpIcon name="user" :size="19" :color="accentPrimary" />
              </div>
              <div class="ma-title-meta">
                <h3 class="ma-title">选择要前往的歌手主页</h3>
                <p class="ma-sub">
                  《{{ store.multiArtistSongTitle || "合唱单曲" }}》包含
                  {{ store.multiArtistCandidates.length }} 位创作者
                </p>
              </div>
            </div>
            <button
              type="button"
              class="ma-close-btn"
              aria-label="关闭歌手选择弹窗"
              @click="store.isMultiArtistModalOpen = false"
            >
              <PlumpIcon name="close" :size="15" />
            </button>
          </div>

          <div class="ma-pick-list">
            <button
              v-for="(author, idx) in store.multiArtistCandidates"
              :key="`${author.id}_${idx}`"
              type="button"
              class="multi-artist-pick-row"
              @click="chooseArtist(author)"
            >
              <div
                class="map-avatar-wrap"
                :style="{
                  background: `linear-gradient(135deg, ${accentPrimary}, ${accentSecondary})`,
                }"
              >
                <img
                  v-if="author.avatar || author.avatarUrl"
                  :src="author.avatar || author.avatarUrl"
                  alt="artist"
                  class="map-avatar-img"
                />
                <div v-else class="map-avatar-fallback">
                  {{ author.name.slice(0, 1) }}
                </div>
              </div>
              <div class="map-meta">
                <div class="map-name">{{ author.name }}</div>
                <div class="map-sub">
                  {{
                    author.id && author.id !== "0"
                      ? `歌手 ID: ${author.id} · 点击查看代表作与专辑`
                      : "点击搜索并进入该歌手主页"
                  }}
                </div>
              </div>
              <PlumpIcon
                name="chevron-right"
                :size="16"
                :color="accentPrimary"
              />
            </button>
          </div>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.ma-modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 2360;
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}

.multi-artist-tonal-modal {
  border-radius: var(--bm-r-xl, 24px);
  padding: 20px 22px;
  border: none;
  color: var(--fm-text-1);
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.ma-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.ma-title-row {
  display: flex;
  align-items: center;
  gap: 11px;
  min-width: 0;
}

.ma-icon-orb {
  width: 38px;
  height: 38px;
  border-radius: var(--bm-r-md, 14px);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.ma-title-meta {
  min-width: 0;
}

.ma-title {
  margin: 0;
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: 800;
  color: var(--fm-text-1);
}

.ma-sub {
  margin: 2px 0 0;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
}

.ma-close-btn {
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 50%;
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  transition: background 0.18s ease, color 0.18s ease, transform 0.2s ease;
}

.ma-close-btn:hover {
  background: var(--fm-chip-hover);
  color: var(--fm-text-1);
  transform: scale(1.06);
}

.ma-pick-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 360px;
  overflow-y: auto;
  padding: 2px;
}

.multi-artist-pick-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 11px 14px;
  border-radius: var(--bm-r-lg, 18px);
  border: none;
  background: var(--fm-card);
  color: var(--fm-text-1);
  text-align: left;
  cursor: pointer;
  transition:
    transform 0.2s cubic-bezier(0.22, 1.35, 0.36, 1),
    background 0.18s ease;
}

.multi-artist-pick-row:hover {
  background: var(--fm-card-hover);
  transform: translateX(3px);
}

.map-avatar-wrap {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  overflow: hidden;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.map-avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.map-avatar-fallback {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: 800;
  color: var(--ma-on-primary, #ffffff);
}

.map-meta {
  flex: 1;
  min-width: 0;
}

.map-name {
  font-size: var(--bm-fs-song-title, 15px);
  font-weight: 800;
  color: var(--fm-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.map-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--fm-text-3);
  margin-top: 2px;
}
</style>

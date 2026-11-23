<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import { useFloatingModalTheme } from "../composables/useFloatingModalTheme";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();
const { modalSurfaceStyle, isLightSurface, accentPrimary } =
  useFloatingModalTheme("auto");

const dialogRef = ref<HTMLDialogElement | null>(null);
const isImporting = ref(false);
let pointerDownOutside = false;

const target = computed(() => store.detectedSharedTarget);
const isSong = computed(() => target.value?.targetType === "song" && Boolean(target.value.song));
const isPlaylist = computed(
  () =>
    target.value?.targetType === "playlist" &&
    Boolean(target.value.playlist || (target.value.songs && target.value.songs.length > 0))
);

const displayCover = computed(() => {
  if (isSong.value) return target.value?.song?.coverUrl || "";
  return target.value?.playlist?.coverUrl || target.value?.songs?.[0]?.coverUrl || "";
});

const displayTitle = computed(() => {
  if (isSong.value) return target.value?.song?.title || "酷狗分享单曲";
  return target.value?.playlist?.title || "酷狗分享歌单";
});

const displaySubtitle = computed(() => {
  if (isSong.value && target.value?.song) {
    const s = target.value.song;
    return s.album ? `${s.artist} · 《${s.album}》` : s.artist || "未知歌手";
  }
  const pl = target.value?.playlist;
  const creator = pl?.creatorName || pl?.creator || "来自酷狗分享链接";
  const count = Math.max(pl?.trackCount || 0, target.value?.songs?.length || 0);
  return count > 0 ? `${creator} · 共 ${count} 首` : creator;
});

function closeDialog() {
  const dlg = dialogRef.value;
  if (dlg && dlg.open) {
    dlg.close("dismiss");
  } else {
    store.dismissDetectedSharedTarget();
  }
}

function handleDialogClose() {
  const dlg = dialogRef.value;
  if (dlg) dlg.returnValue = "";
  isImporting.value = false;
  if (store.detectedSharedTarget) {
    store.dismissDetectedSharedTarget();
  }
}

function isPointerOutsideDialog(e: PointerEvent, dlg: HTMLDialogElement): boolean {
  const r = dlg.getBoundingClientRect();
  return (
    e.clientX < r.left ||
    e.clientX > r.right ||
    e.clientY < r.top ||
    e.clientY > r.bottom
  );
}

function handleDialogPointerDown(e: PointerEvent) {
  const dlg = dialogRef.value;
  if (!dlg || "closedBy" in HTMLDialogElement.prototype) return;
  pointerDownOutside = isPointerOutsideDialog(e, dlg);
}

function handleDialogPointerUp(e: PointerEvent) {
  const dlg = dialogRef.value;
  if (!dlg || "closedBy" in HTMLDialogElement.prototype) return;
  if (pointerDownOutside && isPointerOutsideDialog(e, dlg)) {
    dlg.close("backdrop");
  }
  pointerDownOutside = false;
}

function handlePlaySongNow() {
  const song = target.value?.song;
  if (!song) return;
  const snapshot = { ...song };
  closeDialog();
  store.playSongFromList(snapshot, [snapshot], "分享单曲");
  store.isNowPlayingOpen = true;
}

function handlePlaySongNext() {
  const song = target.value?.song;
  if (!song) return;
  const snapshot = { ...song };
  closeDialog();
  store.insertPlayNext(snapshot);
  store.notify(`已加入下一首播放：${snapshot.title}`);
}

async function handleOpenPlaylist() {
  const cur = target.value;
  if (!cur) return;
  const pl = cur.playlist ? { ...cur.playlist } : null;
  const songs = [...(cur.songs || [])];
  closeDialog();
  await store.openResolvedSharedPlaylist(pl, songs);
}

async function handleImportPlaylist() {
  const cur = target.value;
  if (!cur || isImporting.value) return;
  const title = cur.playlist?.title || "导入酷狗分享歌单";
  const songs = [...(cur.songs || [])];
  isImporting.value = true;
  try {
    closeDialog();
    await store.importSongsAsNewPlaylist(title, songs);
  } finally {
    isImporting.value = false;
  }
}

watch(
  target,
  async (val) => {
    const dlg = dialogRef.value;
    if (!val) {
      if (dlg && dlg.open) dlg.close("sync");
      return;
    }
    await nextTick();
    if (dialogRef.value && !dialogRef.value.open) {
      try {
        dialogRef.value.showModal();
      } catch {
        // ignore
      }
    }
  },
  { immediate: true }
);
</script>

<template>
  <dialog
    ref="dialogRef"
    class="bm-detect-dialog"
    :class="{ 'is-light-surface': isLightSurface }"
    :style="modalSurfaceStyle"
    closedby="any"
    aria-labelledby="bm-detect-title"
    @close="handleDialogClose"
    @pointerdown="handleDialogPointerDown"
    @pointerup="handleDialogPointerUp"
  >
    <div v-if="target && (isSong || isPlaylist)" class="bm-detect-shell">
      <header class="bm-detect-header">
        <span class="bm-detect-badge-orb" aria-hidden="true">
          <PlumpIcon name="link" :size="17" color="var(--fm-on-primary, #ffffff)" />
        </span>
        <h2 id="bm-detect-title" class="bm-detect-title">
          {{ isSong ? "识别到酷狗分享单曲" : "识别到酷狗分享歌单" }}
        </h2>
        <button
          type="button"
          class="bm-detect-close-btn"
          aria-label="关闭识别弹窗"
          @click="closeDialog"
        >
          <PlumpIcon name="close" :size="14" />
        </button>
      </header>

      <div class="bm-detect-card">
        <div class="bm-detect-cover-wrap">
          <img
            v-if="displayCover"
            :src="displayCover"
            :alt="displayTitle"
            class="bm-detect-cover"
          />
          <div v-else class="bm-detect-cover-fallback">
            <PlumpIcon
              :name="isPlaylist ? 'queue' : 'music-note'"
              :size="24"
              :color="accentPrimary"
            />
          </div>
        </div>
        <div class="bm-detect-meta">
          <div class="bm-detect-name">{{ displayTitle }}</div>
          <div class="bm-detect-sub">{{ displaySubtitle }}</div>
        </div>
      </div>

      <footer class="bm-detect-footer">
        <button type="button" class="bm-detect-btn ghost" @click="closeDialog">
          忽略
        </button>

        <div v-if="isSong" class="bm-detect-primary-group">
          <button
            type="button"
            class="bm-detect-btn tonal"
            @click="handlePlaySongNext"
          >
            下一首播放
          </button>
          <button
            type="button"
            class="bm-detect-btn primary"
            autofocus
            @click="handlePlaySongNow"
          >
            立即播放
          </button>
        </div>

        <div v-else class="bm-detect-primary-group">
          <button
            v-if="target.songs && target.songs.length > 0 && store.isLoggedIn"
            type="button"
            class="bm-detect-btn tonal"
            :disabled="isImporting"
            @click="handleImportPlaylist"
          >
            导入为自建歌单
          </button>
          <button
            type="button"
            class="bm-detect-btn primary"
            autofocus
            @click="handleOpenPlaylist"
          >
            查看歌单
          </button>
        </div>
      </footer>
    </div>
  </dialog>
</template>

<style scoped>
.bm-detect-dialog {
  margin: auto;
  padding: 0;
  border: none;
  border-radius: 24px;
  width: min(440px, calc(100vw - 36px));
  color: var(--fm-text-1);
  overflow: hidden;
  contain: layout style;
}

.bm-detect-dialog[open] {
  display: block;
  overscroll-behavior: contain;
  animation: bm-detect-pop 0.22s cubic-bezier(0.22, 1, 0.36, 1);
}

.bm-detect-dialog::backdrop {
  background: rgba(6, 10, 18, 0.52);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  overflow: hidden;
  overscroll-behavior: contain;
}

.bm-detect-dialog.is-light-surface::backdrop {
  background: rgba(24, 28, 38, 0.28);
}

@keyframes bm-detect-pop {
  from {
    opacity: 0;
    transform: translateY(10px) scale(0.975);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

@media (prefers-reduced-motion: reduce) {
  .bm-detect-dialog[open] {
    animation: none;
  }
}

.bm-detect-shell {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.bm-detect-header {
  display: flex;
  align-items: center;
  gap: 10px;
}

.bm-detect-badge-orb {
  width: 34px;
  height: 34px;
  border-radius: 11px;
  background: linear-gradient(135deg, var(--fm-primary) 0%, var(--fm-secondary) 100%);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  box-shadow: 0 8px 18px -6px var(--fm-primary-glow);
}

.bm-detect-title {
  margin: 0;
  flex: 1;
  font-size: 16px;
  font-weight: 800;
  color: var(--fm-text-1);
}

.bm-detect-close-btn {
  width: 30px;
  height: 30px;
  border-radius: 10px;
  border: none;
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background-color 0.16s ease, color 0.16s ease;
}

.bm-detect-close-btn:hover {
  background: var(--fm-chip-hover);
  color: var(--fm-text-1);
}

.bm-detect-card {
  border-radius: 16px;
  background: var(--fm-card);
  padding: 12px;
  display: flex;
  align-items: center;
  gap: 12px;
}

.bm-detect-cover-wrap {
  width: 56px;
  height: 56px;
  border-radius: 12px;
  overflow: hidden;
  flex-shrink: 0;
  background: var(--fm-card-elevated);
  display: flex;
  align-items: center;
  justify-content: center;
}

.bm-detect-cover {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.bm-detect-cover-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--fm-primary-soft);
}

.bm-detect-meta {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.bm-detect-name {
  font-size: 15px;
  font-weight: 700;
  color: var(--fm-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bm-detect-sub {
  font-size: 12.5px;
  color: var(--fm-text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bm-detect-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.bm-detect-primary-group {
  display: flex;
  align-items: center;
  gap: 8px;
}

.bm-detect-btn {
  border: none;
  border-radius: 12px;
  padding: 9px 14px;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
  transition: background-color 0.16s ease, transform 0.14s ease, box-shadow 0.18s ease;
}

.bm-detect-btn:active {
  transform: scale(0.98);
}

.bm-detect-btn:focus-visible {
  outline: 2px solid var(--fm-primary);
  outline-offset: 2px;
}

.bm-detect-btn.ghost {
  background: transparent;
  color: var(--fm-text-2);
}

.bm-detect-btn.ghost:hover {
  background: var(--fm-chip-bg);
  color: var(--fm-text-1);
}

.bm-detect-btn.tonal {
  background: var(--fm-primary-soft);
  color: color-mix(in oklab, var(--fm-primary) 42%, var(--fm-text-1));
}

.bm-detect-dialog.is-light-surface .bm-detect-btn.tonal {
  color: var(--fm-primary);
}

.bm-detect-btn.tonal:hover {
  background: var(--fm-card-hover);
}

.bm-detect-btn.primary {
  background: var(--fm-primary);
  color: var(--fm-on-primary, #ffffff);
  box-shadow: 0 8px 20px -6px var(--fm-primary-glow);
}

.bm-detect-btn.primary:hover {
  filter: brightness(1.06);
}
</style>

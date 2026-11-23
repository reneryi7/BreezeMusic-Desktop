<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import { kugouClient } from "../api/kugou";
import { useFloatingModalTheme } from "../composables/useFloatingModalTheme";
import {
  SHARE_TARGET_EDITIONS,
  buildPlaylistSharePayload,
  buildSongSharePayload,
  copyShareStringToClipboard,
  getPrimaryUrlForEdition,
  getShareTextForEdition,
  type ShareMusicPayload,
  type ShareTargetEdition,
} from "../utils/kugouShare";
import type { Playlist, Song } from "../types";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();
const { modalSurfaceStyle, isLightSurface, accentPrimary, accentSecondary } =
  useFloatingModalTheme("auto");

const dialogRef = ref<HTMLDialogElement | null>(null);
const selectedEdition = ref<ShareTargetEdition>("standard");
const enrichedSong = ref<Song | null>(null);
const enrichedPlaylist = ref<Playlist | null>(null);
const isEnriching = ref(false);
const showAdvancedLinks = ref(false);

const copiedKey = ref<string>("");
const failedKey = ref<string>("");
const statusLiveMessage = ref<string>("");
let lastLiveAnnouncement = "";
let copyResetTimer: number | null = null;
let enrichSeq = 0;

// 兼容不支持 closedby="any" 的环境：记录外点按下并在外点抬起时关闭
let pointerDownOutside = false;

const isOpen = computed(
  () => Boolean(store.shareModalSong || store.shareModalPlaylist)
);

const activeEditionMeta = computed(
  () =>
    SHARE_TARGET_EDITIONS.find((e) => e.id === selectedEdition.value) ||
    SHARE_TARGET_EDITIONS[0]
);

const payload = computed<ShareMusicPayload | null>(() => {
  if (enrichedSong.value) {
    return buildSongSharePayload(enrichedSong.value);
  }
  if (enrichedPlaylist.value) {
    return buildPlaylistSharePayload(enrichedPlaylist.value);
  }
  return null;
});

const activeShareText = computed(() =>
  payload.value ? getShareTextForEdition(payload.value, selectedEdition.value) : ""
);

const activePrimaryUrl = computed(() =>
  payload.value ? getPrimaryUrlForEdition(payload.value, selectedEdition.value) : ""
);

function resetCopyFeedback() {
  if (copyResetTimer !== null) {
    window.clearTimeout(copyResetTimer);
    copyResetTimer = null;
  }
  copiedKey.value = "";
  failedKey.value = "";
  statusLiveMessage.value = "";
}

function reportCopyResult(key: string, ok: boolean, successLabel: string) {
  if (copyResetTimer !== null) {
    window.clearTimeout(copyResetTimer);
    copyResetTimer = null;
  }
  copiedKey.value = ok ? key : "";
  failedKey.value = ok ? "" : key;
  let msg = ok ? successLabel : "复制失败，请重试";
  if (msg === lastLiveAnnouncement) {
    msg += "\u00A0";
  }
  lastLiveAnnouncement = msg;
  statusLiveMessage.value = "";
  requestAnimationFrame(() => {
    statusLiveMessage.value = msg;
  });
  copyResetTimer = window.setTimeout(() => {
    copiedKey.value = "";
    failedKey.value = "";
    statusLiveMessage.value = "";
    copyResetTimer = null;
  }, 2200);
}

async function handleCopyText(key: string, text: string, label: string) {
  if (!text) return;
  const ok = await copyShareStringToClipboard(text);
  reportCopyResult(key, ok, label);
  if (ok) {
    store.notify(label);
  } else {
    store.notify("写入剪贴板失败，请重试");
  }
}

async function handleOpenInBrowser(url: string) {
  if (!url) return;
  try {
    await kugouClient.openExternalUrl(url);
    store.notify("已在系统浏览器打开官方酷狗分享页");
  } catch {
    try {
      window.open(url, "_blank", "noopener,noreferrer");
    } catch {
      store.notify("无法打开浏览器，请先复制链接手动访问");
    }
  }
}

function handleOpenLyricPoster() {
  const target = enrichedSong.value || store.shareModalSong;
  if (!target) return;
  const snapshot = { ...target };
  requestCloseDialog();
  store.openLyricPosterModal(snapshot);
}

function requestCloseDialog() {
  const dlg = dialogRef.value;
  if (dlg && dlg.open) {
    dlg.close("dismiss");
  } else {
    store.closeShareModal();
  }
}

function handleDialogClose() {
  const dlg = dialogRef.value;
  if (dlg) {
    dlg.returnValue = "";
  }
  resetCopyFeedback();
  showAdvancedLinks.value = false;
  if (store.shareModalSong || store.shareModalPlaylist) {
    store.closeShareModal();
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

watch(
  () => [store.shareModalSong, store.shareModalPlaylist] as const,
  async ([song, playlist]) => {
    const seq = ++enrichSeq;
    resetCopyFeedback();
    showAdvancedLinks.value = false;

    if (!song && !playlist) {
      enrichedSong.value = null;
      enrichedPlaylist.value = null;
      isEnriching.value = false;
      const dlg = dialogRef.value;
      if (dlg && dlg.open) {
        dlg.close("sync");
      }
      return;
    }

    selectedEdition.value = "standard";
    enrichedSong.value = song ? { ...song } : null;
    enrichedPlaylist.value = playlist ? { ...playlist } : null;

    await nextTick();
    const dlg = dialogRef.value;
    if (dlg && !dlg.open) {
      try {
        dlg.showModal();
      } catch {
        // ignore if already open
      }
    }

    // 异步实时校准官方双端唤醒所需的标准 128k MP3 hash / album_audio_id 或歌单 specialId / globalCollectionId
    isEnriching.value = true;
    try {
      if (song) {
        const updated = await kugouClient.enrichSongForShare(song);
        if (seq === enrichSeq && store.shareModalSong) {
          enrichedSong.value = updated;
        }
      } else if (playlist) {
        const updated = await kugouClient.enrichPlaylistForShare(playlist);
        if (seq === enrichSeq && store.shareModalPlaylist) {
          enrichedPlaylist.value = updated;
        }
      }
    } catch {
      // 浏览器预览或弱网时保持原始对象生成链接，不阻塞分享
    } finally {
      if (seq === enrichSeq) {
        isEnriching.value = false;
      }
    }
  },
  { immediate: true }
);

onBeforeUnmount(() => {
  resetCopyFeedback();
});
</script>

<template>
  <dialog
    ref="dialogRef"
    class="bm-share-dialog"
    :class="{ 'is-light-surface': isLightSurface }"
    :style="modalSurfaceStyle"
    closedby="any"
    aria-labelledby="bm-share-modal-title"
    @close="handleDialogClose"
    @pointerdown="handleDialogPointerDown"
    @pointerup="handleDialogPointerUp"
  >
    <div v-if="isOpen && payload" class="bm-share-shell">
      <!-- 1. 顶部名片区 -->
      <header class="bm-share-header">
        <div class="bm-share-hero">
          <div class="bm-share-cover-wrap">
            <img
              v-if="payload.coverUrl"
              :src="payload.coverUrl"
              :alt="payload.title"
              class="bm-share-cover"
            />
            <div v-else class="bm-share-cover-fallback">
              <PlumpIcon
                :name="payload.isPlaylist ? 'queue' : 'music-note'"
                :size="24"
                :color="accentPrimary"
              />
            </div>
          </div>

          <div class="bm-share-meta">
            <div class="bm-share-title-row">
              <span class="bm-share-type-badge">
                {{ payload.isPlaylist ? "分享歌单" : "分享单曲" }}
              </span>
              <h2 id="bm-share-modal-title" class="bm-share-title">
                {{ payload.title }}
              </h2>
            </div>
            <p class="bm-share-subtitle">
              {{ payload.subtitle }}
            </p>
            <div v-if="isEnriching" class="bm-share-enrich-hint">
              <span class="bm-share-pulse-dot" aria-hidden="true"></span>
              <span>正在校准官方酷狗双端唤醒元数据…</span>
            </div>
          </div>
        </div>

        <button
          type="button"
          class="bm-share-close-btn"
          aria-label="关闭分享窗口"
          @click="requestCloseDialog"
        >
          <PlumpIcon name="close" :size="15" />
        </button>
      </header>

      <!-- 2. 主体滚动区 -->
      <div class="bm-share-body">
        <!-- 目标唤醒版本选择 -->
        <section class="bm-share-section" aria-label="目标唤醒版本选择">
          <div class="bm-share-section-label">目标唤醒版本选择</div>
          <div class="bm-share-edition-pills" role="radiogroup" aria-label="目标唤醒版本">
            <button
              v-for="ed in SHARE_TARGET_EDITIONS"
              :key="ed.id"
              type="button"
              role="radio"
              class="bm-share-edition-pill"
              :class="{ active: selectedEdition === ed.id }"
              :aria-checked="selectedEdition === ed.id"
              @click="selectedEdition = ed.id"
            >
              <span>{{ ed.shortBadge }}</span>
            </button>
          </div>
          <p class="bm-share-edition-desc">
            {{ activeEditionMeta.description }}
          </p>
        </section>

        <!-- 分享文案预览 -->
        <section class="bm-share-preview-card" aria-label="分享口令预览">
          <div class="bm-share-preview-head">
            <span class="bm-share-preview-tag">分享口令与直达链接预览</span>
            <span class="bm-share-live-pill" :class="{ visible: Boolean(statusLiveMessage) }" role="status">
              {{ statusLiveMessage }}
            </span>
          </div>
          <pre class="bm-share-preview-code">{{ activeShareText }}</pre>
        </section>

        <!-- 主分享操作卡片组 -->
        <div class="bm-share-actions">
          <button
            type="button"
            class="bm-share-action-card is-primary-card"
            :data-copied="copiedKey === 'share-text' ? '' : undefined"
            :data-failed="failedKey === 'share-text' ? '' : undefined"
            autofocus
            @click="
              handleCopyText(
                'share-text',
                activeShareText,
                `已复制「${payload.title}」分享口令与唤醒链接`
              )
            "
          >
            <span class="bm-share-action-orb primary">
              <PlumpIcon
                :name="copiedKey === 'share-text' ? 'check' : 'copy'"
                :size="18"
                color="var(--fm-on-primary, #ffffff)"
              />
            </span>
            <span class="bm-share-action-copy">
              <span class="bm-share-action-title">
                {{
                  copiedKey === "share-text"
                    ? "已复制完整分享口令与唤醒链接"
                    : "复制分享口令与唤醒链接"
                }}
              </span>
              <span class="bm-share-action-sub">
                粘贴发给微信 / QQ 好友即可唤起{{ activeEditionMeta.shortBadge }}，也支持在本应用搜索或剪贴板中自动解析
              </span>
            </span>
          </button>

          <div class="bm-share-action-grid">
            <button
              type="button"
              class="bm-share-action-card"
              :data-copied="copiedKey === 'direct-url' ? '' : undefined"
              :data-failed="failedKey === 'direct-url' ? '' : undefined"
              @click="
                handleCopyText(
                  'direct-url',
                  activePrimaryUrl,
                  `已复制${activeEditionMeta.shortBadge}直达链接`
                )
              "
            >
              <span class="bm-share-action-orb secondary">
                <PlumpIcon
                  :name="copiedKey === 'direct-url' ? 'check' : 'link'"
                  :size="17"
                  :color="accentSecondary"
                />
              </span>
              <span class="bm-share-action-copy">
                <span class="bm-share-action-title">
                  {{ copiedKey === "direct-url" ? "已复制直达链" : "仅复制直达链接" }}
                </span>
                <span class="bm-share-action-sub">纯净官方 H5 链接</span>
              </span>
            </button>

            <button
              type="button"
              class="bm-share-action-card"
              @click="handleOpenInBrowser(activePrimaryUrl)"
            >
              <span class="bm-share-action-orb accent">
                <PlumpIcon name="share" :size="17" :color="accentPrimary" />
              </span>
              <span class="bm-share-action-copy">
                <span class="bm-share-action-title">浏览器打开分享页</span>
                <span class="bm-share-action-sub">预览落地页或唤端</span>
              </span>
            </button>
          </div>

          <button
            v-if="!payload.isPlaylist"
            type="button"
            class="bm-share-action-card"
            @click="handleOpenLyricPoster"
          >
            <span class="bm-share-action-orb poster">
              <PlumpIcon name="poster" :size="18" :color="accentPrimary" :accent="accentSecondary" />
            </span>
            <span class="bm-share-action-copy">
              <span class="bm-share-action-title">生成 2K / 4K 高清艺术歌词海报</span>
              <span class="bm-share-action-sub">
                挑选精选歌词与双主色风格，生成高定视觉卡片保存或复制到剪贴板
              </span>
            </span>
          </button>
        </div>

        <!-- 折叠高级协议链接 -->
        <div class="bm-share-advanced">
          <button
            type="button"
            class="bm-share-advanced-toggle"
            :aria-expanded="showAdvancedLinks"
            @click="showAdvancedLinks = !showAdvancedLinks"
          >
            <span>更多官方唤醒协议与桌面网页链接</span>
            <span class="bm-share-advanced-arrow" :class="{ open: showAdvancedLinks }">▾</span>
          </button>

          <div v-if="showAdvancedLinks" class="bm-share-advanced-list">
            <div class="bm-share-adv-row">
              <div class="bm-share-adv-info">
                <span class="bm-share-adv-name">桌面端官方试听页</span>
                <span class="bm-share-adv-url">{{ payload.standardH5PlayUrl }}</span>
              </div>
              <button
                type="button"
                class="bm-share-mini-copy-btn"
                aria-label="复制桌面端官方试听页链接"
                @click="
                  handleCopyText(
                    'adv-web',
                    payload.standardH5PlayUrl,
                    '已复制桌面端官方试听页链接'
                  )
                "
              >
                <PlumpIcon :name="copiedKey === 'adv-web' ? 'check' : 'copy'" :size="14" />
                <span>{{ copiedKey === "adv-web" ? "已复制" : "复制" }}</span>
              </button>
            </div>

            <div class="bm-share-adv-row">
              <div class="bm-share-adv-info">
                <span class="bm-share-adv-name">酷狗音乐标准版 Scheme</span>
                <span class="bm-share-adv-url">{{ payload.standardSchemeUri }}</span>
              </div>
              <button
                type="button"
                class="bm-share-mini-copy-btn"
                aria-label="复制酷狗标准版唤醒协议"
                @click="
                  handleCopyText(
                    'adv-std-scheme',
                    payload.standardSchemeUri,
                    '已复制酷狗标准版唤醒协议'
                  )
                "
              >
                <PlumpIcon :name="copiedKey === 'adv-std-scheme' ? 'check' : 'copy'" :size="14" />
                <span>{{ copiedKey === "adv-std-scheme" ? "已复制" : "复制" }}</span>
              </button>
            </div>

            <div class="bm-share-adv-row">
              <div class="bm-share-adv-info">
                <span class="bm-share-adv-name">酷狗概念版 Scheme</span>
                <span class="bm-share-adv-url">{{ payload.conceptSchemeUri }}</span>
              </div>
              <button
                type="button"
                class="bm-share-mini-copy-btn"
                aria-label="复制酷狗概念版唤醒协议"
                @click="
                  handleCopyText(
                    'adv-concept-scheme',
                    payload.conceptSchemeUri,
                    '已复制酷狗概念版唤醒协议'
                  )
                "
              >
                <PlumpIcon :name="copiedKey === 'adv-concept-scheme' ? 'check' : 'copy'" :size="14" />
                <span>{{ copiedKey === "adv-concept-scheme" ? "已复制" : "复制" }}</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </dialog>
</template>

<style scoped>
.bm-share-dialog {
  margin: auto;
  padding: 0;
  border: none;
  border-radius: 26px;
  width: min(500px, calc(100vw - 36px));
  max-height: min(86vh, 690px);
  color: var(--fm-text-1);
  overflow: hidden;
  contain: layout style;
}

.bm-share-dialog[open] {
  display: flex;
  flex-direction: column;
  overscroll-behavior: contain;
  animation: bm-share-pop 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.bm-share-dialog::backdrop {
  background: rgba(6, 10, 18, 0.56);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  overflow: hidden;
  overscroll-behavior: contain;
}

.bm-share-dialog.is-light-surface::backdrop {
  background: rgba(24, 28, 38, 0.32);
}

@keyframes bm-share-pop {
  from {
    opacity: 0;
    transform: translateY(12px) scale(0.975);
  }
  to {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

@media (prefers-reduced-motion: reduce) {
  .bm-share-dialog[open] {
    animation: none;
  }
}

.bm-share-shell {
  display: flex;
  flex-direction: column;
  max-height: min(86vh, 690px);
  overflow: hidden;
}

.bm-share-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 20px 20px 14px;
  background: linear-gradient(180deg, var(--fm-card-elevated) 0%, transparent 100%);
}

.bm-share-hero {
  display: flex;
  align-items: center;
  gap: 14px;
  min-width: 0;
  flex: 1;
}

.bm-share-cover-wrap {
  width: 58px;
  height: 58px;
  border-radius: 15px;
  overflow: hidden;
  flex-shrink: 0;
  background: var(--fm-card);
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 8px 20px rgba(0, 0, 0, 0.16);
}

.bm-share-cover {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.bm-share-cover-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--fm-primary-soft);
}

.bm-share-meta {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.bm-share-title-row {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.bm-share-type-badge {
  flex-shrink: 0;
  padding: 2px 8px;
  border-radius: 7px;
  font-size: 11px;
  font-weight: 700;
  color: color-mix(in oklab, var(--fm-primary) 44%, var(--fm-text-1));
  background: var(--fm-primary-soft);
  letter-spacing: 0.02em;
}

.bm-share-dialog.is-light-surface .bm-share-type-badge {
  color: var(--fm-primary);
}

.bm-share-title {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
  color: var(--fm-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bm-share-subtitle {
  margin: 0;
  font-size: 12.5px;
  color: var(--fm-text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bm-share-enrich-hint {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  color: var(--fm-text-3);
}

.bm-share-pulse-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--fm-primary);
  animation: bm-share-dot-pulse 1.2s ease-in-out infinite;
}

@keyframes bm-share-dot-pulse {
  0%, 100% {
    opacity: 0.35;
    transform: scale(0.85);
  }
  50% {
    opacity: 1;
    transform: scale(1.15);
  }
}

.bm-share-close-btn {
  width: 32px;
  height: 32px;
  border-radius: 12px;
  border: none;
  background: var(--fm-chip-bg);
  color: var(--fm-text-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  transition: background-color 0.18s ease, color 0.18s ease, transform 0.16s ease;
}

.bm-share-close-btn:hover {
  background: var(--fm-chip-hover);
  color: var(--fm-text-1);
}

.bm-share-close-btn:focus-visible {
  outline: 2px solid var(--fm-primary);
  outline-offset: 2px;
}

.bm-share-body {
  padding: 6px 20px 20px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.bm-share-section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.bm-share-section-label {
  font-size: 12px;
  font-weight: 700;
  color: var(--fm-text-2);
  letter-spacing: 0.02em;
}

.bm-share-edition-pills {
  position: relative;
  isolation: isolate;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 4px;
  padding: 4px;
  border-radius: 999px;
  background: var(--fm-chip-bg);
}

.bm-share-edition-pill {
  position: relative;
  z-index: 1;
  border: none;
  border-radius: 999px;
  padding: 8px 10px;
  background: transparent;
  color: var(--fm-text-2);
  font-size: 12.5px;
  font-weight: 700;
  cursor: pointer;
  transition: background-color 0.2s ease, color 0.2s ease, box-shadow 0.2s ease;
}

.bm-share-edition-pill:hover:not(.active) {
  color: var(--fm-text-1);
}

.bm-share-edition-pill.active {
  background: linear-gradient(
    135deg in oklab,
    var(--fm-primary) 0%,
    color-mix(in oklab, var(--fm-primary) 72%, var(--fm-secondary)) 100%
  );
  color: var(--fm-on-primary, #ffffff);
  font-weight: 700;
  box-shadow: 0 4px 14px -4px var(--fm-primary-glow);
}

.bm-share-edition-pill:focus-visible {
  outline: 2px solid var(--fm-primary);
  outline-offset: 1px;
}

.bm-share-edition-desc {
  margin: 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--fm-text-3);
}

.bm-share-preview-card {
  border-radius: 16px;
  background: var(--fm-card);
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.bm-share-preview-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.bm-share-preview-tag {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--fm-text-3);
}

.bm-share-live-pill {
  font-size: 11px;
  font-weight: 600;
  color: color-mix(in oklab, var(--fm-primary) 48%, var(--fm-text-1));
  opacity: 0;
  transition: opacity 0.2s ease;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 240px;
}

.bm-share-dialog.is-light-surface .bm-share-live-pill {
  color: var(--fm-primary);
}

.bm-share-live-pill.visible {
  opacity: 1;
}

.bm-share-preview-code {
  margin: 0;
  font-family: inherit;
  font-size: 12.5px;
  line-height: 1.55;
  color: var(--fm-text-1);
  white-space: pre-wrap;
  word-break: break-all;
  max-height: 108px;
  overflow-y: auto;
  user-select: text;
}

.bm-share-actions {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.bm-share-action-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.bm-share-action-card {
  width: 100%;
  border: none;
  border-radius: 16px;
  padding: 12px 14px;
  background: var(--fm-card);
  color: var(--fm-text-1);
  display: flex;
  align-items: center;
  gap: 12px;
  text-align: left;
  cursor: pointer;
  transition: background-color 0.18s ease, transform 0.16s ease, box-shadow 0.2s ease;
}

.bm-share-action-card:hover {
  background: var(--fm-card-hover);
  transform: translateY(-1px);
}

.bm-share-action-card:active {
  transform: translateY(0) scale(0.99);
}

.bm-share-action-card:focus-visible {
  outline: 2px solid var(--fm-primary);
  outline-offset: 2px;
}

.bm-share-action-card.is-primary-card {
  background: var(--fm-card-active);
  box-shadow: 0 10px 26px -12px var(--fm-primary-glow);
}

.bm-share-action-card.is-primary-card:hover {
  box-shadow: 0 14px 30px -10px var(--fm-primary-glow);
}

.bm-share-action-orb {
  width: 38px;
  height: 38px;
  border-radius: 12px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.bm-share-action-orb.primary {
  background: var(--fm-primary);
  box-shadow: 0 6px 16px -4px var(--fm-primary-glow);
}

.bm-share-action-orb.secondary {
  background: var(--fm-secondary-soft);
}

.bm-share-action-orb.accent {
  background: var(--fm-primary-soft);
}

.bm-share-action-orb.poster {
  background: linear-gradient(135deg, var(--fm-primary-soft) 0%, var(--fm-secondary-soft) 100%);
}

.bm-share-action-copy {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.bm-share-action-title {
  font-size: 13.5px;
  font-weight: 700;
  color: var(--fm-text-1);
  line-height: 1.3;
}

.bm-share-action-sub {
  font-size: 11.5px;
  color: var(--fm-text-2);
  line-height: 1.4;
}

.bm-share-advanced {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.bm-share-advanced-toggle {
  border: none;
  border-radius: 12px;
  padding: 8px 12px;
  background: transparent;
  color: var(--fm-text-3);
  font-size: 12px;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  transition: background-color 0.16s ease, color 0.16s ease;
}

.bm-share-advanced-toggle:hover {
  background: var(--fm-card);
  color: var(--fm-text-2);
}

.bm-share-advanced-arrow {
  display: inline-block;
  transition: transform 0.2s ease;
}

.bm-share-advanced-arrow.open {
  transform: rotate(180deg);
}

.bm-share-advanced-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.bm-share-adv-row {
  border-radius: 14px;
  background: var(--fm-card);
  padding: 10px 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.bm-share-adv-info {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.bm-share-adv-name {
  font-size: 12px;
  font-weight: 700;
  color: var(--fm-text-1);
}

.bm-share-adv-url {
  font-size: 11px;
  color: var(--fm-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bm-share-mini-copy-btn {
  min-width: 58px;
  min-height: 28px;
  padding: 4px 10px;
  border-radius: 9px;
  border: none;
  background: var(--fm-chip-bg);
  color: var(--fm-text-1);
  font-size: 11.5px;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  cursor: pointer;
  flex-shrink: 0;
  transition: background-color 0.16s ease, color 0.16s ease;
}

.bm-share-mini-copy-btn:hover {
  background: var(--fm-chip-hover);
  color: var(--fm-primary);
}

.bm-share-mini-copy-btn:focus-visible {
  outline: 2px solid var(--fm-primary);
  outline-offset: 2px;
}
</style>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../stores/appStore";
import { useFloatingModalTheme } from "../composables/useFloatingModalTheme";
import {
  buildCoverResolutionCandidates,
  resolveBestCoverWithMetrics,
} from "../utils/coverPalette";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();
const { modalSurfaceStyle, accentPrimary, onAccentPrimary, isLightSurface } =
  useFloatingModalTheme("auto");

const isVisible = computed(() =>
  Boolean(store.hdArtworkPreviewTarget?.url || store.hdArtworkPreviewUrl)
);
const activePreviewTarget = computed(() => {
  if (store.hdArtworkPreviewTarget?.url) {
    return store.hdArtworkPreviewTarget;
  }
  if (store.hdArtworkPreviewUrl) {
    return {
      url: store.hdArtworkPreviewUrl,
      title: "专辑封面预览",
      artist: "高清封面画廊",
      album: "",
      rawCoverTemplate: store.hdArtworkPreviewUrl,
      hdCoverCandidates: [] as string[],
      song: null,
      initialMetrics: null,
    };
  }
  return null;
});
const previewTitle = computed(
  () => activePreviewTarget.value?.title || activePreviewTarget.value?.song?.title || "专辑封面预览"
);
const previewArtist = computed(
  () => activePreviewTarget.value?.artist || activePreviewTarget.value?.song?.artist || "高清封面画廊"
);
const previewAlbum = computed(
  () => activePreviewTarget.value?.album || activePreviewTarget.value?.song?.album || ""
);
const displayUrl = ref<string>("");
const imgMetrics = ref<{ width: number; height: number } | null>(null);
const isProbingHd = ref(false);
const isSaving = ref(false);

const zoom = ref<number>(1);
const panX = ref<number>(0);
const panY = ref<number>(0);
const isDragging = ref(false);
let dragStartX = 0;
let dragStartY = 0;
let dragBasePanX = 0;
let dragBasePanY = 0;
let probeSeq = 0;

function closePreview() {
  store.closeHdArtworkPreview();
}

function resetViewport() {
  zoom.value = 1;
  panX.value = 0;
  panY.value = 0;
}

watch(
  () => [
    activePreviewTarget.value?.url || "",
    activePreviewTarget.value?.song?.hash || "",
    activePreviewTarget.value?.song?.id || "",
  ] as const,
  async ([rawUrl]) => {
    resetViewport();
    imgMetrics.value = null;
    if (!rawUrl) {
      displayUrl.value = "";
      isProbingHd.value = false;
      return;
    }

    const req = ++probeSeq;
    const target = activePreviewTarget.value;
    const seedTemplate = target?.rawCoverTemplate || rawUrl;
    const extraCandidates = [
      ...(target?.hdCoverCandidates || []),
      rawUrl,
    ];
    const candidates = buildCoverResolutionCandidates(seedTemplate, extraCandidates);
    displayUrl.value = candidates[0] || rawUrl;
    if (target?.initialMetrics && target.url) {
      displayUrl.value = target.url;
      imgMetrics.value = { ...target.initialMetrics };
    }

    isProbingHd.value = true;
    try {
      const best = await resolveBestCoverWithMetrics(seedTemplate, extraCandidates);
      if (req !== probeSeq) return;
      if (best) {
        displayUrl.value = best.url;
        imgMetrics.value = { width: best.width, height: best.height };
      }
    } finally {
      if (req === probeSeq) {
        isProbingHd.value = false;
      }
    }
  },
  { immediate: true }
);

function onImageLoad(ev: Event) {
  const img = ev.target as HTMLImageElement | null;
  if (!img || !img.naturalWidth) return;
  imgMetrics.value = {
    width: img.naturalWidth,
    height: img.naturalHeight,
  };
}

function stepZoom(delta: number) {
  const next = Math.max(0.5, Math.min(4.0, Number((zoom.value + delta).toFixed(2))));
  zoom.value = next;
  if (next <= 1.02) {
    panX.value = 0;
    panY.value = 0;
  }
}

function handleWheelZoom(ev: WheelEvent) {
  const delta = ev.deltaY < 0 ? 0.2 : -0.2;
  stepZoom(delta);
}

function handleDblClickToggle() {
  if (Math.abs(zoom.value - 1) > 0.15) {
    resetViewport();
  } else {
    zoom.value = 2;
  }
}

function handlePointerDown(ev: PointerEvent) {
  if (ev.button !== 0 || zoom.value <= 1.02) return;
  isDragging.value = true;
  dragStartX = ev.clientX;
  dragStartY = ev.clientY;
  dragBasePanX = panX.value;
  dragBasePanY = panY.value;
  (ev.currentTarget as HTMLElement)?.setPointerCapture?.(ev.pointerId);
}

function handlePointerMove(ev: PointerEvent) {
  if (!isDragging.value) return;
  panX.value = dragBasePanX + (ev.clientX - dragStartX);
  panY.value = dragBasePanY + (ev.clientY - dragStartY);
}

function handlePointerUp(ev: PointerEvent) {
  if (!isDragging.value) return;
  isDragging.value = false;
  (ev.currentTarget as HTMLElement)?.releasePointerCapture?.(ev.pointerId);
}

function sanitizeFilePart(s: string): string {
  return (s || "cover").replace(/[\\/:*?"<>|]/g, "_").trim() || "cover";
}

async function encodeUrlToPngBase64(url: string): Promise<string | null> {
  // 1. 先尝试 Rust 安全拉取绕过 CORS 限制
  try {
    const rawDataUrl = await invoke<string>("fetch_cover_image_base64_cmd", { url });
    if (rawDataUrl && rawDataUrl.startsWith("data:image/")) {
      const img = new Image();
      const loaded = await new Promise<boolean>((resolve) => {
        img.onload = () => resolve(true);
        img.onerror = () => resolve(false);
        img.src = rawDataUrl;
      });
      if (loaded && img.naturalWidth > 0) {
        const canvas = document.createElement("canvas");
        canvas.width = img.naturalWidth;
        canvas.height = img.naturalHeight;
        const ctx = canvas.getContext("2d");
        ctx?.drawImage(img, 0, 0);
        return canvas.toDataURL("image/png");
      }
      return rawDataUrl;
    }
  } catch {
    // 浏览器直接测试环境无 Tauri，走 DOM 导出兜底
  }

  // 2. Web / CORS 画布编码兜底
  try {
    const resp = await fetch(url, { mode: "cors" });
    if (resp.ok) {
      const blob = await resp.blob();
      const bmp = await createImageBitmap(blob);
      const canvas = document.createElement("canvas");
      canvas.width = bmp.width;
      canvas.height = bmp.height;
      const ctx = canvas.getContext("2d");
      ctx?.drawImage(bmp, 0, 0);
      bmp.close?.();
      return canvas.toDataURL("image/png");
    }
  } catch {
    // ignore
  }
  return null;
}

async function handleSaveArtwork() {
  if (isSaving.value || !displayUrl.value) return;
  isSaving.value = true;
  try {
    const songTitle = sanitizeFilePart(previewTitle.value || "BreezeMusic_Cover");
    const suggested = `${songTitle}_HD.png`;
    const base64Data = await encodeUrlToPngBase64(displayUrl.value);
    if (base64Data) {
      try {
        const savedPath = await invoke<string | null>(
          "save_image_base64_with_dialog_cmd",
          {
            suggestedName: suggested,
            dataBase64: base64Data,
          }
        );
        if (savedPath) {
          store.notify(`已保存高清封面：${savedPath}`);
        }
        return;
      } catch {
        // Web 模式回退浏览器直接下载
        const a = document.createElement("a");
        a.href = base64Data;
        a.download = suggested;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        store.notify("已导出高清封面图片");
        return;
      }
    }
    // 最末级回退：直接触发图片下载链接
    const a = document.createElement("a");
    a.href = displayUrl.value;
    a.target = "_blank";
    a.download = suggested;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  } finally {
    isSaving.value = false;
  }
}

function handleKeyDown(ev: KeyboardEvent) {
  if (!isVisible.value) return;
  if (ev.key === "Escape") {
    closePreview();
  } else if (ev.key === "+" || ev.key === "=") {
    stepZoom(0.25);
  } else if (ev.key === "-") {
    stepZoom(-0.25);
  } else if (ev.key === "0") {
    resetViewport();
  }
}

watch(isVisible, (visible) => {
  if (typeof window === "undefined") return;
  if (visible) {
    window.addEventListener("keydown", handleKeyDown);
  } else {
    window.removeEventListener("keydown", handleKeyDown);
  }
});

onBeforeUnmount(() => {
  if (typeof window !== "undefined") {
    window.removeEventListener("keydown", handleKeyDown);
  }
});

const resolutionText = computed(() => {
  if (imgMetrics.value) {
    return `${imgMetrics.value.width} × ${imgMetrics.value.height}`;
  }
  return isProbingHd.value ? "匹配原图中..." : "高清原图";
});
</script>

<template>
  <Teleport to="body">
    <transition name="np-float-spring">
      <div
        v-if="isVisible"
        class="hd-artwork-backdrop"
        :class="{ 'is-light': isLightSurface }"
        @click.self="closePreview"
      >
        <div
          class="hd-artwork-window"
          :style="modalSurfaceStyle"
          role="dialog"
          aria-modal="true"
          aria-label="高清封面预览"
          @click.stop
        >
          <!-- 顶部栏 -->
          <header class="hd-art-header">
            <div class="hd-art-title-wrap">
              <h3 class="hd-art-title">
                {{ previewTitle }}
              </h3>
              <span class="hd-art-sub">
                {{ previewArtist }}
                <template v-if="previewAlbum">
                  · 《{{ previewAlbum }}》
                </template>
              </span>
            </div>

            <div class="hd-art-head-right">
              <span class="hd-res-badge" :class="{ probing: isProbingHd }">
                {{ resolutionText }}
              </span>
              <button
                type="button"
                class="hd-icon-close"
                aria-label="关闭预览"
                @click="closePreview"
              >
                <PlumpIcon name="close" :size="15" />
              </button>
            </div>
          </header>

          <!-- 可缩放/拖拽看图舞台 -->
          <div
            class="hd-art-viewport"
            :class="{ 'is-pannable': zoom > 1.02, 'is-dragging': isDragging }"
            @wheel.prevent="handleWheelZoom"
            @dblclick="handleDblClickToggle"
            @pointerdown="handlePointerDown"
            @pointermove="handlePointerMove"
            @pointerup="handlePointerUp"
            @pointercancel="handlePointerUp"
          >
            <img
              v-if="displayUrl"
              :src="displayUrl"
              class="hd-art-image"
              :style="{
                transform: `translate3d(${panX}px, ${panY}px, 0) scale(${zoom})`,
              }"
              draggable="false"
              alt="Album Artwork"
              @load="onImageLoad"
            />
          </div>

          <!-- 底部精简控制坞 -->
          <footer class="hd-art-footer">
            <div class="hd-zoom-controls">
              <button
                type="button"
                class="hd-tool-btn"
                title="缩小"
                :disabled="zoom <= 0.55"
                @click="stepZoom(-0.25)"
              >
                −
              </button>
              <button
                type="button"
                class="hd-zoom-readout"
                title="点击恢复适应窗口 (100%)"
                @click="resetViewport"
              >
                {{ Math.round(zoom * 100) }}%
              </button>
              <button
                type="button"
                class="hd-tool-btn"
                title="放大"
                :disabled="zoom >= 3.95"
                @click="stepZoom(0.25)"
              >
                +
              </button>
            </div>

            <div class="hd-action-right">
              <span class="hd-hint-tip">滚轮缩放 · 双击放大</span>
              <button
                type="button"
                class="hd-save-btn"
                :disabled="isSaving || !displayUrl"
                :style="{
                  background: accentPrimary,
                  color: onAccentPrimary,
                }"
                @click="handleSaveArtwork"
              >
                <PlumpIcon name="download" :size="15" :color="onAccentPrimary" />
                <span>{{ isSaving ? "保存中..." : "保存原图" }}</span>
              </button>
            </div>
          </footer>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.hd-artwork-backdrop {
  position: fixed;
  inset: 0;
  z-index: 9400;
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 20px;
}

.hd-artwork-backdrop.is-light {
  background: transparent;
}

.hd-artwork-window {
  width: min(860px, 86vw);
  height: min(680px, 84vh);
  border-radius: var(--bm-r-xl, 24px);
  padding: 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow: hidden;
  color: var(--fm-text-1);
}

.hd-art-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-shrink: 0;
}

.hd-art-title-wrap {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.hd-art-title {
  margin: 0;
  font-size: 16.5px;
  font-weight: 800;
  color: var(--fm-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.hd-art-sub {
  font-size: 12.5px;
  color: var(--fm-text-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.hd-art-head-right {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.hd-res-badge {
  padding: 5px 12px;
  border-radius: 999px;
  background: var(--fm-primary-soft);
  color: var(--fm-primary);
  font-size: 12px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.hd-icon-close {
  width: 34px;
  height: 34px;
  border-radius: 50%;
  border: none;
  background: var(--fm-chip-bg);
  color: var(--fm-text-1);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.18s ease, transform 0.18s ease;
}

.hd-icon-close:hover {
  background: var(--fm-chip-hover);
  transform: scale(1.06);
}

.hd-art-viewport {
  flex: 1;
  min-height: 0;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--fm-card);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  position: relative;
  user-select: none;
  cursor: zoom-in;
}

.hd-art-viewport.is-pannable {
  cursor: grab;
}

.hd-art-viewport.is-dragging {
  cursor: grabbing;
}

.hd-art-image {
  max-width: 92%;
  max-height: 92%;
  object-fit: contain;
  border-radius: 16px;
  box-shadow: 0 22px 48px rgba(0, 0, 0, 0.34);
  transition: transform 0.16s cubic-bezier(0.22, 1, 0.36, 1);
  will-change: transform;
}

.hd-art-viewport.is-dragging .hd-art-image {
  transition: none;
}

.hd-art-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-shrink: 0;
}

.hd-zoom-controls {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px;
  border-radius: 999px;
  background: var(--fm-card);
}

.hd-tool-btn,
.hd-zoom-readout {
  height: 30px;
  min-width: 32px;
  padding: 0 10px;
  border-radius: 999px;
  border: none;
  background: transparent;
  color: var(--fm-text-1);
  font-size: 13px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  cursor: pointer;
  transition: background 0.16s ease, opacity 0.16s ease;
}

.hd-tool-btn:hover:not(:disabled),
.hd-zoom-readout:hover {
  background: var(--fm-chip-hover);
}

.hd-tool-btn:disabled {
  opacity: 0.36;
  cursor: not-allowed;
}

.hd-action-right {
  display: flex;
  align-items: center;
  gap: 14px;
}

.hd-hint-tip {
  font-size: 12px;
  color: var(--fm-text-3);
}

.hd-save-btn {
  height: 38px;
  padding: 0 18px;
  border-radius: 999px;
  border: none;
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 13.5px;
  font-weight: 700;
  cursor: pointer;
  transition: transform 0.18s ease, box-shadow 0.18s ease, filter 0.18s ease;
}

.hd-save-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  filter: brightness(1.06);
  box-shadow: 0 10px 24px var(--fm-primary-glow);
}

.hd-save-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>

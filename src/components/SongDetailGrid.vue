<script setup lang="ts">
import type { AudioDetailModel } from "../utils/audioDetailModel";
import { useAppStore } from "../stores/appStore";

const props = withDefaults(
  defineProps<{
    model: AudioDetailModel;
    loading?: boolean;
    loadingText?: string;
    variant?: "immersive" | "adaptive";
    showAvailableQualities?: boolean;
  }>(),
  {
    loading: false,
    loadingText: "正在深度解析发行元数据与音频流头...",
    variant: "immersive",
    showAvailableQualities: true,
  }
);

const store = useAppStore();

async function copyField(label: string, val?: string) {
  if (!val) return;
  try {
    await navigator.clipboard.writeText(val);
    store.notify(`已复制 ${label}：${val}`);
  } catch {
    store.notify(`${label}: ${val}`);
  }
}
</script>

<template>
  <div class="sdg-root" :class="`variant-${props.variant}`">
    <div v-if="loading" class="sdg-loading">
      {{ loadingText }}
    </div>

    <div class="sdg-grid">
      <div class="sdg-item">
        <span class="sdg-k">歌曲名称</span>
        <span class="sdg-v">{{ model.title }}</span>
      </div>

      <div class="sdg-item">
        <span class="sdg-k">演唱歌手</span>
        <span class="sdg-v">{{ model.artist }}</span>
      </div>

      <div class="sdg-item">
        <span class="sdg-k">所属专辑</span>
        <span class="sdg-v">{{ model.album }}</span>
      </div>

      <div class="sdg-item">
        <span class="sdg-k">发行时间</span>
        <span class="sdg-v">{{ model.publishDate }}</span>
      </div>

      <div class="sdg-item">
        <span class="sdg-k">发行所属</span>
        <span class="sdg-v">{{ model.publishCompany }}</span>
      </div>

      <div class="sdg-item">
        <span class="sdg-k">语种 / 流派</span>
        <span class="sdg-v">{{ model.languageGenre }}</span>
      </div>

      <div v-if="model.credits.length > 0" class="sdg-item full">
        <span class="sdg-k">词曲创作信息</span>
        <div class="sdg-chips">
          <span v-for="cr in model.credits" :key="cr.role" class="sdg-chip">
            <em>{{ cr.role }}</em>
            <span>{{ cr.name }}</span>
          </span>
        </div>
      </div>

      <div class="sdg-item">
        <span class="sdg-k">当前音质与编码</span>
        <span class="sdg-v accent">{{ model.qualityCodec }}</span>
      </div>

      <div class="sdg-item">
        <span class="sdg-k">文件大小与时长</span>
        <span class="sdg-v">{{ model.sizeDuration }}</span>
      </div>

      <div class="sdg-item full">
        <span class="sdg-k">采样率 / 位深 / 码率 / 声道</span>
        <span class="sdg-v">{{ model.streamSpec }}</span>
      </div>

      <div
        v-if="showAvailableQualities && model.availableQualities.length > 0"
        class="sdg-item full"
      >
        <span class="sdg-k">可选音源档位</span>
        <div class="sdg-chips">
          <span
            v-for="q in model.availableQualities"
            :key="q.id"
            class="sdg-chip"
            :class="{ gold: q.isGold }"
          >
            <em>{{ q.badge }}</em>
            <span>{{ q.sizeText }}</span>
          </span>
          <span v-if="model.hasAccompaniment" class="sdg-chip mint">
            <em>KTV</em>
            <span>已匹配官方伴奏</span>
          </span>
        </div>
      </div>

      <div class="sdg-item full">
        <span class="sdg-k">副歌高潮区间</span>
        <span class="sdg-v">{{ model.climaxRange }}</span>
      </div>

      <button
        type="button"
        class="sdg-item full"
        :class="{ clickable: Boolean(model.trackHash) }"
        :disabled="!model.trackHash"
        @click="copyField('音轨 Hash', model.trackHash)"
      >
        <span class="sdg-k">
          音轨 Hash {{ model.trackHash ? "(点击复制)" : "" }}
        </span>
        <span class="sdg-v mono">
          {{ model.trackHash || "暂无" }}
        </span>
      </button>

      <button
        type="button"
        class="sdg-item full"
        :class="{ clickable: Boolean(model.albumAudioId) }"
        :disabled="!model.albumAudioId"
        @click="copyField('Album Audio ID', model.albumAudioId)"
      >
        <span class="sdg-k">
          Album Audio ID {{ model.albumAudioId ? "(点击复制)" : "" }}
        </span>
        <span class="sdg-v mono">
          {{ model.albumAudioId || "暂无" }}
        </span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.sdg-root {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-inline-size: 0;
}

.sdg-root.variant-immersive {
  --sdg-card-bg: var(
    --fm-card,
    var(--np-card-bg, var(--np-surface-container, rgba(255, 255, 255, 0.05)))
  );
  --sdg-card-hover: var(
    --fm-card-hover,
    var(--np-card-hover-bg, var(--np-surface-container-high, rgba(255, 255, 255, 0.09)))
  );
  --sdg-chip-bg: var(
    --fm-chip-bg,
    var(--np-card-elevated, var(--np-surface-container-high, rgba(255, 255, 255, 0.08)))
  );
  --sdg-text-primary: var(--fm-text-1, #ffffff);
  --sdg-text-muted: var(--fm-text-3, rgba(255, 255, 255, 0.58));
  --sdg-accent: var(--fm-primary, var(--np-primary, var(--primary, #5e6ad2)));
  --sdg-gold: var(--np-vip-gold, var(--gold, #b45309));
  --sdg-mint: var(--fm-secondary, var(--mint, #0f766e));
}

.sdg-root.variant-adaptive {
  --sdg-card-bg: var(--bm-surface-row-hover, var(--bg-glass));
  --sdg-card-hover: var(--bg-pill);
  --sdg-chip-bg: var(--bg-pill);
  --sdg-text-primary: var(--bm-text-1, var(--text-primary));
  --sdg-text-muted: var(--bm-text-3, var(--text-muted));
  --sdg-accent: var(--primary);
  --sdg-gold: var(--gold);
  --sdg-mint: var(--mint);
}

.sdg-loading {
  margin-bottom: 4px;
  padding: 8px 12px;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--sdg-chip-bg);
  color: var(--sdg-accent);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  text-align: center;
}

.sdg-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

@media (max-width: 520px) {
  .sdg-grid {
    grid-template-columns: 1fr;
  }
}

.sdg-item {
  appearance: none;
  border: none;
  padding: 8px 12px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--sdg-card-bg);
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  gap: 3px;
  text-align: left;
  min-inline-size: 0;
  font-size: var(--bm-fs-caption, 12.5px);
  transition:
    background 0.18s ease,
    transform 0.18s ease;
}

.sdg-item.full {
  grid-column: 1 / -1;
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.sdg-item.clickable {
  cursor: pointer;
}

.sdg-item.clickable:hover {
  background: var(--sdg-card-hover);
}

.sdg-item.clickable:focus-visible {
  outline: 2px solid var(--sdg-accent);
  outline-offset: 2px;
}

.sdg-item:disabled {
  cursor: default;
  opacity: 1;
}

.sdg-k {
  color: var(--sdg-text-muted);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 600;
  flex-shrink: 0;
}

.sdg-v {
  color: var(--sdg-text-primary);
  font-weight: 700;
  font-size: var(--bm-fs-caption, 12.5px);
  min-inline-size: 0;
  word-break: break-word;
}

.sdg-item.full .sdg-v {
  text-align: right;
  word-break: break-all;
}

.sdg-v.accent {
  color: var(--sdg-accent);
}

.sdg-v.mono {
  font-family: Consolas, monospace;
  font-size: var(--bm-fs-micro, 11px);
}

.sdg-chips {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
  min-inline-size: 0;
}

.sdg-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 3px 9px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--sdg-chip-bg);
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--sdg-text-primary);
  font-weight: 700;
}

.sdg-chip em {
  font-style: normal;
  font-size: var(--bm-fs-micro, 11px);
  color: var(--sdg-accent);
  font-weight: 800;
}

.sdg-chip.gold em {
  color: var(--sdg-gold);
}

.sdg-chip.mint em {
  color: var(--sdg-mint);
}
</style>

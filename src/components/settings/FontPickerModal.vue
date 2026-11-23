<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useAppStore } from "../../stores/appStore";
import type { CustomFontRecord, SystemFontFamilyInfo } from "../../types";
import {
  formatWeightLabel,
  quoteFontFamilyName,
} from "../../utils/fontRuntime";
import PlumpIcon from "../PlumpIcon.vue";

export type FontPickerScope = "ui" | "stageLyric" | "desktopLyric";

const props = defineProps<{
  open: boolean;
  initialScope: FontPickerScope;
}>();

const emit = defineEmits<{
  (e: "close"): void;
}>();

const store = useAppStore();
const dialogRef = ref<HTMLDialogElement | null>(null);
const searchInputRef = ref<HTMLInputElement | null>(null);

const scope = ref<FontPickerScope>(props.initialScope);
const filterMode = ref<"all" | "cjk" | "custom" | "variable" | "mono">("all");
const searchQuery = ref("");
const sampleText = ref("微风拂面，听见 24-Bit / 96kHz 无损细节 · BreezeMusic");
const previewWeight = ref<number>(400);

interface CandidateFontItem {
  key: string;
  source: "default" | "custom" | "system";
  familyName: string;
  displayName: string;
  customFontId: string;
  weights: number[];
  isVariable: boolean;
  weightMin: number;
  weightMax: number;
  supportsCjk: boolean;
  isMonospace: boolean;
  formatLabel: string;
}

const selectedCandidate = ref<CandidateFontItem | null>(null);

const scopeTabs: ReadonlyArray<{ id: FontPickerScope; label: string }> = [
  { id: "ui", label: "全局界面" },
  { id: "stageLyric", label: "播放页歌词" },
  { id: "desktopLyric", label: "桌面歌词" },
];

const filterTabs: ReadonlyArray<{
  id: "all" | "cjk" | "custom" | "variable" | "mono";
  label: string;
}> = [
  { id: "all", label: "全部" },
  { id: "cjk", label: "中文优化" },
  { id: "custom", label: "已导入" },
  { id: "variable", label: "可变字体" },
  { id: "mono", label: "等宽" },
];

const samplePresets = [
  { label: "中英", text: "微风拂面，听见 24-Bit / 96kHz 无损细节 · BreezeMusic" },
  { label: "歌词", text: "晚风轻轻吹过城市的霓虹，星河落入眼眸 — Midnight Breeze" },
  { label: "数字", text: "FLAC 1411 kbps · 03:45 / 04:29 · -14.0 LUFS" },
];

const defaultCandidate: CandidateFontItem = {
  key: "__default__",
  source: "default",
  familyName: "",
  displayName: "系统默认字体",
  customFontId: "",
  weights: [300, 400, 600, 700],
  isVariable: false,
  weightMin: 300,
  weightMax: 700,
  supportsCjk: true,
  isMonospace: false,
  formatLabel: "默认推荐",
};

const allCandidates = computed<CandidateFontItem[]>(() => {
  const list: CandidateFontItem[] = [];
  for (const cf of store.customFonts) {
    list.push({
      key: `custom:${cf.id}`,
      source: "custom",
      familyName: cf.cssFamilyName,
      displayName: cf.displayName || cf.familyName,
      customFontId: cf.id,
      weights: cf.weights?.length ? cf.weights : [400],
      isVariable: Boolean(cf.isVariable),
      weightMin: cf.weightMin || 100,
      weightMax: cf.weightMax || 900,
      supportsCjk: Boolean(cf.supportsCjk),
      isMonospace: false,
      formatLabel: (cf.format || "TTF").toUpperCase(),
    });
  }
  for (const sf of store.systemFonts) {
    list.push({
      key: `sys:${sf.familyName}`,
      source: "system",
      familyName: sf.familyName,
      displayName: sf.displayName || sf.familyName,
      customFontId: "",
      weights: sf.weights?.length ? sf.weights : [400],
      isVariable: Boolean(sf.isVariable),
      weightMin: sf.weightMin || 400,
      weightMax: sf.weightMax || 400,
      supportsCjk: Boolean(sf.supportsCjk),
      isMonospace: Boolean(sf.isMonospace),
      formatLabel: sf.isVariable ? "可变" : `${sf.weights?.length || 1} 字重`,
    });
  }
  return list;
});

const filteredCandidates = computed<CandidateFontItem[]>(() => {
  const q = searchQuery.value.trim().toLowerCase();
  return allCandidates.value.filter((item) => {
    if (filterMode.value === "custom" && item.source !== "custom") return false;
    if (filterMode.value === "cjk" && !item.supportsCjk) return false;
    if (filterMode.value === "variable" && !item.isVariable) return false;
    if (filterMode.value === "mono" && !item.isMonospace) return false;
    if (!q) return true;
    return (
      item.displayName.toLowerCase().includes(q) ||
      item.familyName.toLowerCase().includes(q)
    );
  });
});

function getCurrentActiveKeyForScope(sc: FontPickerScope): string {
  const cfg = store.typographyConfig;
  if (sc === "ui") {
    if (!cfg.uiFontFamily) return "__default__";
    return cfg.uiCustomFontId ? `custom:${cfg.uiCustomFontId}` : `sys:${cfg.uiFontFamily}`;
  }
  if (sc === "stageLyric") {
    if (cfg.stageLyricMode !== "custom" || !cfg.stageLyricFontFamily) return "__default__";
    return cfg.stageLyricCustomFontId
      ? `custom:${cfg.stageLyricCustomFontId}`
      : `sys:${cfg.stageLyricFontFamily}`;
  }
  if (cfg.desktopLyricMode !== "custom" || !cfg.desktopLyricFontFamily) return "__default__";
  return cfg.desktopLyricCustomFontId
    ? `custom:${cfg.desktopLyricCustomFontId}`
    : `sys:${cfg.desktopLyricFontFamily}`;
}

function syncSelectionFromCurrentScope() {
  const activeKey = getCurrentActiveKeyForScope(scope.value);
  if (activeKey === "__default__") {
    selectedCandidate.value = defaultCandidate;
  } else {
    selectedCandidate.value =
      allCandidates.value.find((c) => c.key === activeKey) || defaultCandidate;
  }
  if (scope.value === "ui") {
    previewWeight.value = store.typographyConfig.uiBaseWeight || 400;
  } else if (scope.value === "stageLyric") {
    previewWeight.value = store.typographyConfig.stageLyricWeight || 700;
  } else {
    previewWeight.value = store.typographyConfig.desktopLyricWeight || 700;
  }
}

const activePreviewCandidate = computed(
  () => selectedCandidate.value || defaultCandidate
);

const previewFontStack = computed(() => {
  const c = activePreviewCandidate.value;
  if (!c.familyName) {
    return '"Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei UI", sans-serif';
  }
  return `${quoteFontFamilyName(c.familyName)}, "Microsoft YaHei UI", sans-serif`;
});

function itemPreviewFont(item: CandidateFontItem): string {
  if (!item.familyName) {
    return '"Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei UI", sans-serif';
  }
  return `${quoteFontFamilyName(item.familyName)}, "Microsoft YaHei UI", sans-serif`;
}

function selectItem(item: CandidateFontItem) {
  selectedCandidate.value = item;
}

async function applyCandidate(item: CandidateFontItem) {
  if (scope.value === "ui") {
    if (item.source === "default") {
      await store.updateTypographyConfig({
        uiFontFamily: "",
        uiCustomFontId: "",
        uiFontDisplayName: "系统默认",
      });
    } else {
      await store.updateTypographyConfig({
        uiFontFamily: item.familyName,
        uiCustomFontId: item.customFontId,
        uiFontDisplayName: item.displayName,
      });
    }
    store.notify(`已将全局界面字体切换为「${item.displayName}」`);
  } else if (scope.value === "stageLyric") {
    if (item.source === "default") {
      await store.updateTypographyConfig({
        stageLyricMode: "preset",
        stageLyricFontFamily: "",
        stageLyricCustomFontId: "",
        stageLyricFontDisplayName: "",
      });
      store.notify("播放页歌词已恢复沿用内置预设字体");
    } else {
      await store.updateTypographyConfig({
        stageLyricMode: "custom",
        stageLyricFontFamily: item.familyName,
        stageLyricCustomFontId: item.customFontId,
        stageLyricFontDisplayName: item.displayName,
      });
      store.notify(`播放页歌词字体已设为「${item.displayName}」`);
    }
  } else {
    if (item.source === "default") {
      await store.updateTypographyConfig({
        desktopLyricMode: "preset",
        desktopLyricFontFamily: "",
        desktopLyricCustomFontId: "",
        desktopLyricFontDisplayName: "",
      });
      store.notify("桌面悬浮歌词已恢复沿用内置预设字体");
    } else {
      await store.updateTypographyConfig({
        desktopLyricMode: "custom",
        desktopLyricFontFamily: item.familyName,
        desktopLyricCustomFontId: item.customFontId,
        desktopLyricFontDisplayName: item.displayName,
      });
      store.notify(`桌面悬浮歌词字体已设为「${item.displayName}」`);
    }
  }
  closeModal();
}

async function handleDeleteCustomFont(e: MouseEvent, item: CandidateFontItem) {
  e.stopPropagation();
  if (!item.customFontId) return;
  const ok = await store.askConfirm({
    title: "卸载自定义字体",
    message: `确定移除「${item.displayName}」吗？若当前正在使用将自动回退系统默认。`,
    confirmText: "确认卸载",
    tone: "danger",
  });
  if (!ok) return;
  await store.removeCustomFont(item.customFontId);
  if (selectedCandidate.value?.key === item.key) {
    selectedCandidate.value = defaultCandidate;
  }
}

function closeModal() {
  const dlg = dialogRef.value;
  if (dlg && dlg.open) {
    dlg.close();
  }
  emit("close");
}

function handleDialogClose() {
  emit("close");
}

/** 遵循 modern-web-guidance (light-dismiss-a-dialog)：仅在不支持原生 closedby 的引擎上启用点击边界回退 */
function handleBackdropClick(e: MouseEvent) {
  if ("closedBy" in HTMLDialogElement.prototype) return;
  const dlg = dialogRef.value;
  if (!dlg || e.target !== dlg) return;
  const rect = dlg.getBoundingClientRect();
  const inBounds =
    e.clientX >= rect.left &&
    e.clientX <= rect.right &&
    e.clientY >= rect.top &&
    e.clientY <= rect.bottom;
  if (!inBounds) {
    closeModal();
  }
}

watch(
  () => props.open,
  async (isOpen) => {
    const dlg = dialogRef.value;
    if (isOpen) {
      scope.value = props.initialScope;
      searchQuery.value = "";
      if (store.systemFonts.length === 0 && !store.isScanningFonts) {
        void store.refreshInstalledFonts(true);
      }
      syncSelectionFromCurrentScope();
      await nextTick();
      if (dlg && !dlg.open) {
        dlg.showModal();
      }
      searchInputRef.value?.focus();
    } else if (dlg && dlg.open) {
      dlg.close();
    }
  }
);

watch(scope, () => {
  syncSelectionFromCurrentScope();
});

onBeforeUnmount(() => {
  const dlg = dialogRef.value;
  if (dlg && dlg.open) {
    dlg.close();
  }
});
</script>

<template>
  <dialog
    ref="dialogRef"
    class="bm-font-dialog bm-top-layer-dialog"
    closedby="any"
    aria-labelledby="bm-font-dialog-title"
    @close="handleDialogClose"
    @click="handleBackdropClick"
  >
    <div class="bm-fd-shell">
      <!-- 顶栏 -->
      <header class="bm-fd-header">
        <div class="bm-fd-head-left">
          <div class="bm-fd-title-row">
            <PlumpIcon name="font-plus" :size="18" color="var(--primary)" />
            <h2 id="bm-font-dialog-title" class="bm-fd-title">字体选择与预览</h2>
          </div>
          <div class="bm-fd-scope-tabs" role="tablist" aria-label="目标作用域">
            <button
              v-for="tab in scopeTabs"
              :key="tab.id"
              type="button"
              role="tab"
              class="bm-fd-scope-btn"
              :aria-selected="scope === tab.id ? 'true' : 'false'"
              @click="scope = tab.id"
            >
              {{ tab.label }}
            </button>
          </div>
        </div>

        <div class="bm-fd-head-actions">
          <button
            type="button"
            class="bms-btn sm"
            :disabled="store.isImportingFonts"
            @click="store.importCustomFontsFromPicker()"
          >
            <PlumpIcon name="upload" :size="13" />
            <span>{{ store.isImportingFonts ? "导入中…" : "导入本地字体" }}</span>
          </button>
          <button
            type="button"
            class="bm-fd-close-btn"
            aria-label="关闭字体选择窗口"
            @click="closeModal"
          >
            <PlumpIcon name="close" :size="15" />
          </button>
        </div>
      </header>

      <!-- 检索与筛选条 -->
      <div class="bm-fd-toolbar">
        <div class="bm-fd-search">
          <PlumpIcon name="search" :size="14" color="var(--text-muted)" />
          <input
            ref="searchInputRef"
            v-model="searchQuery"
            type="search"
            class="bm-fd-search-input"
            placeholder="搜索本机字体名称（如 微软雅黑、思源、霞鹜、Inter…）"
            autocomplete="off"
            spellcheck="false"
          />
          <button
            v-if="searchQuery"
            type="button"
            class="bms-search-clear"
            aria-label="清空搜索"
            @click="searchQuery = ''"
          >
            <PlumpIcon name="close" :size="10" />
          </button>
        </div>

        <div class="bm-fd-filters">
          <button
            v-for="ft in filterTabs"
            :key="ft.id"
            type="button"
            class="bm-fd-filter-pill"
            :class="{ active: filterMode === ft.id }"
            @click="filterMode = ft.id"
          >
            {{ ft.label }}
            <span v-if="ft.id === 'custom' && store.customFonts.length > 0" class="bm-fd-count">
              {{ store.customFonts.length }}
            </span>
          </button>
          <button
            type="button"
            class="bm-fd-icon-btn"
            :disabled="store.isScanningFonts"
            aria-label="重新扫描系统字体"
            data-tip="重新扫描本机字体"
            @click="store.refreshInstalledFonts(false)"
          >
            <PlumpIcon name="refresh" :size="14" />
          </button>
        </div>
      </div>

      <!-- 双栏主体：左侧字体列表 + 右侧实时预览舱 -->
      <div class="bm-fd-body">
        <div class="bm-fd-list-pane" role="listbox" aria-label="可用字体列表">
          <!-- 恢复系统默认项 -->
          <button
            type="button"
            role="option"
            class="bm-fd-font-card"
            :class="{
              'is-selected': activePreviewCandidate.key === '__default__',
              'is-current': getCurrentActiveKeyForScope(scope) === '__default__',
            }"
            @click="selectItem(defaultCandidate)"
            @dblclick="applyCandidate(defaultCandidate)"
          >
            <div class="bm-fd-font-top">
              <span class="bm-fd-font-name">系统默认字体</span>
              <span class="bms-chip muted">推荐</span>
            </div>
            <div class="bm-fd-font-glyph" :style="{ fontFamily: itemPreviewFont(defaultCandidate) }">
              微风音乐 · BreezeMusic 1234
            </div>
          </button>

          <div v-if="store.isScanningFonts && allCandidates.length === 0" class="bm-fd-empty">
            正在扫描本机已安装字体…
          </div>

          <button
            v-for="item in filteredCandidates"
            :key="item.key"
            type="button"
            role="option"
            class="bm-fd-font-card"
            :class="{
              'is-selected': activePreviewCandidate.key === item.key,
              'is-current': getCurrentActiveKeyForScope(scope) === item.key,
            }"
            @click="selectItem(item)"
            @dblclick="applyCandidate(item)"
          >
            <div class="bm-fd-font-top">
              <span class="bm-fd-font-name">{{ item.displayName }}</span>
              <div class="bm-fd-card-tags">
                <span v-if="item.source === 'custom'" class="bms-chip mint">已导入</span>
                <span v-if="item.isVariable" class="bms-chip">可变</span>
                <span v-else class="bm-fd-sub-tag">{{ item.formatLabel }}</span>
                <button
                  v-if="item.source === 'custom'"
                  type="button"
                  class="bm-fd-del-btn"
                  aria-label="卸载此自定义字体"
                  @click="handleDeleteCustomFont($event, item)"
                >
                  <PlumpIcon name="trash" :size="12" />
                </button>
              </div>
            </div>
            <div class="bm-fd-font-glyph" :style="{ fontFamily: itemPreviewFont(item) }">
              微风音乐 · BreezeMusic 01:28
            </div>
          </button>

          <div v-if="!store.isScanningFonts && filteredCandidates.length === 0" class="bm-fd-empty">
            没有匹配的字体，可调整筛选或点击右上角导入本地 .ttf / .otf / .woff2 / .ttc 文件
          </div>
        </div>

        <!-- 右侧实时预览舱 -->
        <aside class="bm-fd-preview-pane">
          <div class="bm-fd-preview-meta">
            <div class="bm-fd-preview-name">{{ activePreviewCandidate.displayName }}</div>
            <div class="bm-fd-preview-sub">
              <span>{{ activePreviewCandidate.familyName || "Segoe UI / Microsoft YaHei UI" }}</span>
              <span>·</span>
              <span>{{ activePreviewCandidate.supportsCjk ? "支持中文" : "拉丁为主（中文自动安全回退）" }}</span>
            </div>
          </div>

          <!-- 预览样本 1：界面排版 -->
          <div
            class="bm-fd-stage-box"
            :style="{ fontFamily: previewFontStack, fontWeight: previewWeight }"
          >
            <div class="bm-fd-stage-label">界面排版预览</div>
            <div class="bm-fd-stage-headline">{{ sampleText || "微风拂面，听见无损细节" }}</div>
            <div class="bm-fd-stage-meta">
              FLAC 24-Bit / 96kHz · 1411 kbps · 03:45 / 04:29
            </div>
          </div>

          <!-- 预览样本 2：歌词视觉 -->
          <div
            class="bm-fd-stage-box lyric-tone"
            :style="{ fontFamily: previewFontStack }"
          >
            <div class="bm-fd-stage-label">歌词渲染预览</div>
            <div class="bm-fd-lyric-main" :style="{ fontWeight: Math.min(900, previewWeight + 200) }">
              晚风轻轻吹过城市的霓虹，星河落入眼眸
            </div>
            <div class="bm-fd-lyric-sub">
              The midnight breeze carries lossless melodies softly
            </div>
          </div>

          <!-- 预览文案与字重试看 -->
          <div class="bm-fd-preview-controls">
            <div class="bm-fd-sample-presets">
              <button
                v-for="p in samplePresets"
                :key="p.label"
                type="button"
                class="bm-fd-preset-chip"
                @click="sampleText = p.text"
              >
                {{ p.label }}
              </button>
              <span class="bm-fd-weight-pill">试看字重 {{ formatWeightLabel(previewWeight) }}</span>
            </div>
            <input
              v-model="sampleText"
              type="text"
              class="bm-fd-sample-input"
              placeholder="输入任意文字即时试看字形…"
              maxlength="120"
            />
          </div>
        </aside>
      </div>

      <!-- 底栏操作 -->
      <footer class="bm-fd-footer">
        <span class="bm-fd-footer-hint">
          共 {{ allCandidates.length }} 款可用字体 · 双击列表项可快速应用
        </span>
        <div class="bm-fd-footer-actions">
          <button type="button" class="bms-btn ghost" @click="closeModal">
            取消
          </button>
          <button
            type="button"
            class="bms-btn primary"
            @click="applyCandidate(activePreviewCandidate)"
          >
            <PlumpIcon name="check" :size="14" color="#ffffff" />
            <span>
              应用到{{ scope === "ui" ? "全局界面" : scope === "stageLyric" ? "播放页歌词" : "桌面歌词" }}
            </span>
          </button>
        </div>
      </footer>
    </div>
  </dialog>
</template>

<style scoped>
.bm-font-dialog {
  width: min(880px, 88vw);
  height: min(650px, 82vh);
  max-width: 92vw;
  max-height: 86vh;
  margin: auto;
  padding: 0;
  border: none;
  border-radius: var(--bm-r-xl, 24px);
  background: var(--bg-elevated);
  color: var(--text-primary);
  box-shadow: var(--shadow-elevated);
  overflow: hidden;
}

.bm-font-dialog:modal {
  overscroll-behavior: contain;
}

.bm-font-dialog::backdrop {
  background: rgba(6, 10, 18, 0.56);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  overscroll-behavior: contain;
  overflow: hidden;
}

.bm-fd-shell {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 18px 20px 16px;
  gap: 14px;
}

.bm-fd-header {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}

.bm-fd-head-left {
  display: flex;
  align-items: center;
  gap: 14px;
  flex-wrap: wrap;
}

.bm-fd-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.bm-fd-title {
  margin: 0;
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
}

.bm-fd-scope-tabs {
  position: relative;
  isolation: isolate;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--bms-surface-well);
}

.bm-fd-scope-btn {
  position: relative;
  z-index: 1;
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  padding: 5px 12px;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  white-space: nowrap;
  transition: background 0.18s ease, color 0.18s ease;
}

.bm-fd-scope-btn:hover:not([aria-selected="true"]) {
  color: var(--text-primary);
  background: var(--bms-neutral-control, var(--bg-hover-square));
}

.bm-fd-scope-btn[aria-selected="true"] {
  background: var(--bms-focus-grad, var(--grad-kugou-primary));
  color: #ffffff;
}

.bm-fd-head-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.bm-fd-close-btn {
  width: 34px;
  height: 34px;
  border: none;
  border-radius: 50%;
  background: var(--bg-surface);
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.18s ease, color 0.18s ease;
}

.bm-fd-close-btn:hover {
  background: var(--bg-pill-hover);
  color: var(--text-primary);
}

.bm-fd-toolbar {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  flex-wrap: wrap;
}

.bm-fd-search {
  flex: 1 1 260px;
  min-block-size: 38px;
  padding: 5px 12px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--bg-surface);
  display: flex;
  align-items: center;
  gap: 8px;
}

.bm-fd-search-input {
  flex: 1;
  min-width: 0;
  border: none;
  background: transparent;
  color: var(--text-primary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
}

.bm-fd-filters {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-wrap: wrap;
}

.bm-fd-filter-pill {
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  padding: 5px 11px;
  background: var(--bms-neutral-control, var(--bg-surface));
  color: var(--text-secondary);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-bold, 700);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  cursor: pointer;
}

.bm-fd-filter-pill.active {
  background: var(--primary-soft);
  color: var(--primary-text, var(--primary));
}

.bm-fd-count {
  padding: 0 5px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--primary) 18%, transparent);
}

.bm-fd-icon-btn {
  width: 30px;
  height: 30px;
  border: none;
  border-radius: 50%;
  background: var(--bg-surface);
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.bm-fd-body {
  flex: 1 1 0;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1.08fr) minmax(0, 0.92fr);
  gap: 14px;
}

.bm-fd-list-pane {
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-right: 4px;
}

.bm-fd-list-pane::-webkit-scrollbar {
  width: 5px !important;
  display: block !important;
}

.bm-fd-list-pane::-webkit-scrollbar-thumb {
  border-radius: 999px;
  background: color-mix(in srgb, var(--text-muted) 32%, transparent);
}

.bm-fd-font-card {
  width: 100%;
  border: none;
  border-radius: var(--bm-r-md, 14px);
  padding: 11px 14px;
  background: var(--bg-surface);
  color: var(--text-primary);
  text-align: left;
  display: flex;
  flex-direction: column;
  gap: 5px;
  cursor: pointer;
  transition: background 0.16s ease, box-shadow 0.2s ease;
}

.bm-fd-font-card:hover {
  background: var(--bg-card-hover);
}

.bm-fd-font-card.is-selected {
  background: color-mix(in srgb, var(--primary) 16%, var(--bg-surface));
  box-shadow: 0 8px 20px -10px color-mix(in srgb, var(--primary) 42%, transparent);
}

.bm-fd-font-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.bm-fd-font-name {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bm-fd-card-tags {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.bm-fd-sub-tag {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-muted);
}

.bm-fd-del-btn {
  width: 24px;
  height: 24px;
  border: none;
  border-radius: 50%;
  background: var(--accent-coral-soft);
  color: var(--coral);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.bm-fd-font-glyph {
  font-size: var(--bm-fs-body, 14.5px);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bm-fd-empty {
  padding: 28px 16px;
  text-align: center;
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-muted);
}

.bm-fd-preview-pane {
  min-height: 0;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bg-surface);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  overflow-y: auto;
  overscroll-behavior: contain;
}

.bm-fd-preview-name {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
}

.bm-fd-preview-sub {
  margin-top: 3px;
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-muted);
}

.bm-fd-stage-box {
  border-radius: var(--bm-r-md, 14px);
  padding: 14px 16px;
  background: var(--bg-card);
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.bm-fd-stage-box.lyric-tone {
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--primary) 12%, var(--bg-card)),
    color-mix(in srgb, var(--violet) 10%, var(--bg-card))
  );
}

.bm-fd-stage-label {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-muted);
}

.bm-fd-stage-headline {
  font-size: var(--bm-fs-title, 20px);
  line-height: var(--bm-ui-line-height, 1.48);
  color: var(--text-primary);
  word-break: break-word;
}

.bm-fd-stage-meta {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.bm-fd-lyric-main {
  font-size: var(--bm-fs-subhead, 16.5px);
  line-height: 1.45;
  color: var(--text-primary);
}

.bm-fd-lyric-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
}

.bm-fd-preview-controls {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.bm-fd-sample-presets {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}

.bm-fd-preset-chip {
  border: none;
  border-radius: 999px;
  padding: 3px 10px;
  background: var(--bg-pill);
  color: var(--text-secondary);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
}

.bm-fd-preset-chip:hover {
  background: var(--bg-pill-hover);
  color: var(--text-primary);
}

.bm-fd-weight-pill {
  margin-left: auto;
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-muted);
}

.bm-fd-sample-input {
  width: 100%;
  min-block-size: 36px;
  padding: 7px 12px;
  border: none;
  border-radius: var(--bm-r-sm, 10px);
  background: var(--bg-card);
  color: var(--text-primary);
  font-size: var(--bm-fs-caption, 12.5px);
}

.bm-fd-footer {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-top: 4px;
}

.bm-fd-footer-hint {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-muted);
}

.bm-fd-footer-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* L2 晨雾燕麦实体暖纸特化（严禁使用 :global） */
.scheme-watercolor_light.light-variant-nordic_oat .bm-font-dialog {
  background: #FAF6EE;
}

.scheme-watercolor_light.light-variant-nordic_oat .bm-fd-scope-tabs,
.scheme-watercolor_light.light-variant-nordic_oat .bm-fd-search,
.scheme-watercolor_light.light-variant-nordic_oat .bm-fd-filter-pill,
.scheme-watercolor_light.light-variant-nordic_oat .bm-fd-font-card,
.scheme-watercolor_light.light-variant-nordic_oat .bm-fd-preview-pane,
.scheme-watercolor_light.light-variant-nordic_oat .bm-fd-close-btn,
.scheme-watercolor_light.light-variant-nordic_oat .bm-fd-icon-btn {
  background: #F2ECE1;
}

.scheme-watercolor_light.light-variant-nordic_oat .bm-fd-stage-box,
.scheme-watercolor_light.light-variant-nordic_oat .bm-fd-sample-input {
  background: #FAF6EE;
}

/* D2 丝绒星云苦巧天鹅绒特化 */
.scheme-abyssal_dark.dark-variant-velvet_nebula .bm-font-dialog {
  background: var(--bms-surface-card);
}

.scheme-abyssal_dark.dark-variant-velvet_nebula .bm-fd-font-card,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bm-fd-preview-pane {
  background: var(--bms-surface-row);
}

.scheme-abyssal_dark.dark-variant-velvet_nebula .bm-fd-search {
  background: var(--bms-surface-well);
}

@media (max-width: 760px) {
  .bm-fd-body {
    grid-template-columns: 1fr;
  }
}
</style>

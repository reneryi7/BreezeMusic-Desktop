<script setup lang="ts">
import { computed, ref } from "vue";
import { useAppStore } from "../../stores/appStore";
import type {
  TypographyDesktopLyricMode,
  TypographyLatinFallbackId,
  TypographyStageLyricMode,
} from "../../types";
import {
  buildUiFontFamilyStack,
  computeDerivedFontWeights,
  formatWeightLabel,
} from "../../utils/fontRuntime";
import PlumpIcon from "../PlumpIcon.vue";
import SettingsCard from "./SettingsCard.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSwitch from "./SettingsSwitch.vue";
import FontPickerModal, { type FontPickerScope } from "./FontPickerModal.vue";

defineProps<{
  highlightedId?: string;
}>();

const store = useAppStore();

const pickerOpen = ref(false);
const pickerInitialScope = ref<FontPickerScope>("ui");

function openFontPicker(scope: FontPickerScope) {
  pickerInitialScope.value = scope;
  pickerOpen.value = true;
}

const scalePresets = [
  { value: 0.8, label: "80% 紧凑" },
  { value: 0.9, label: "90% 偏小" },
  { value: 1.0, label: "100% 标准" },
  { value: 1.15, label: "115% 偏大" },
  { value: 1.3, label: "130% 舒朗" },
];

const weightOptions = [
  { value: 300, label: "细体 300" },
  { value: 400, label: "常规 400" },
  { value: 500, label: "中等 500" },
  { value: 600, label: "半粗 600" },
  { value: 700, label: "粗体 700" },
];

const lineHeightOptions = [
  { value: 1.38, label: "紧凑 1.38" },
  { value: 1.48, label: "标准 1.48" },
  { value: 1.62, label: "舒展 1.62" },
];

const latinFallbackOptions: ReadonlyArray<{
  id: TypographyLatinFallbackId;
  label: string;
}> = [
  { id: "auto", label: "跟随主字体" },
  { id: "segoe", label: "Segoe UI" },
  { id: "bahnschrift", label: "Bahnschrift" },
  { id: "inter", label: "Inter" },
  { id: "mono", label: "Cascadia 等宽" },
];

const stageModeOptions: ReadonlyArray<{
  id: TypographyStageLyricMode;
  label: string;
}> = [
  { id: "preset", label: "沿用歌词预设" },
  { id: "follow_ui", label: "跟随全局界面" },
  { id: "custom", label: "独立指定" },
];

const desktopModeOptions: ReadonlyArray<{
  id: TypographyDesktopLyricMode;
  label: string;
}> = [
  { id: "preset", label: "沿用歌词预设" },
  { id: "follow_stage", label: "跟随播放页" },
  { id: "follow_ui", label: "跟随全局界面" },
  { id: "custom", label: "独立指定" },
];

const uiScalePercent = computed(() =>
  Math.round(store.typographyConfig.uiFontScale * 100)
);

const derivedWeights = computed(() =>
  computeDerivedFontWeights(store.typographyConfig.uiBaseWeight)
);

const activeUiStack = computed(() =>
  buildUiFontFamilyStack(store.typographyConfig)
);

const uiSlotSummary = computed(() =>
  store.typographyConfig.uiFontFamily
    ? store.typographyConfig.uiFontDisplayName || store.typographyConfig.uiFontFamily
    : "系统默认"
);

const stageSlotSummary = computed(() => {
  const cfg = store.typographyConfig;
  if (cfg.stageLyricMode === "preset") return "沿用播放页预设";
  if (cfg.stageLyricMode === "follow_ui") return `跟随全局（${uiSlotSummary.value}）`;
  return cfg.stageLyricFontDisplayName || cfg.stageLyricFontFamily || "未指定";
});

const desktopSlotSummary = computed(() => {
  const cfg = store.typographyConfig;
  if (cfg.desktopLyricMode === "preset") return "沿用桌面歌词预设";
  if (cfg.desktopLyricMode === "follow_stage") return "跟随播放页歌词";
  if (cfg.desktopLyricMode === "follow_ui") return `跟随全局（${uiSlotSummary.value}）`;
  return cfg.desktopLyricFontDisplayName || cfg.desktopLyricFontFamily || "未指定";
});

function setUiScale(scale: number) {
  void store.updateTypographyConfig({ uiFontScale: scale });
}

function handleScaleSliderInput(e: Event) {
  const pct = Number((e.target as HTMLInputElement).value);
  if (Number.isFinite(pct)) {
    void store.updateTypographyConfig({ uiFontScale: pct / 100 });
  }
}

function setUiWeight(weight: number) {
  void store.updateTypographyConfig({ uiBaseWeight: weight });
}

function setLineHeight(lh: number) {
  void store.updateTypographyConfig({ uiLineHeight: lh });
}

function setLatinFallback(id: TypographyLatinFallbackId) {
  void store.updateTypographyConfig({ latinFallback: id });
}

function setStageLyricMode(mode: TypographyStageLyricMode) {
  if (mode === "custom" && !store.typographyConfig.stageLyricFontFamily) {
    openFontPicker("stageLyric");
    return;
  }
  void store.updateTypographyConfig({ stageLyricMode: mode });
}

function setDesktopLyricMode(mode: TypographyDesktopLyricMode) {
  if (mode === "custom" && !store.typographyConfig.desktopLyricFontFamily) {
    openFontPicker("desktopLyric");
    return;
  }
  void store.updateTypographyConfig({ desktopLyricMode: mode });
}
</script>

<template>
  <div class="bms-panel-stack">
    <!-- 卡片 1：三作用域字体总览（点击弹窗挑选，告别长屏刷列表） -->
    <SettingsCard
      icon="font-plus"
      icon-color="var(--violet)"
      title="字体与作用域"
      subtitle="在独立预览窗口中挑选系统字体或导入本地字体，分别作用于界面与歌词"
    >
      <template #actions>
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
          class="bms-btn sm ghost"
          @click="store.resetTypographyConfig()"
        >
          <PlumpIcon name="reset-rotate" :size="13" />
          <span>恢复默认</span>
        </button>
      </template>

      <div
        class="bmt-slot-grid"
        data-setting-id="typography-preview"
        :class="{ 'is-flash-highlight': highlightedId === 'typography-preview' }"
      >
        <!-- 槽位 1：全局界面 -->
        <div class="bmt-slot-card">
          <div class="bmt-slot-top">
            <span class="bmt-slot-label">全局界面</span>
            <span class="bms-chip">{{ formatWeightLabel(store.typographyConfig.uiBaseWeight) }}</span>
          </div>
          <div class="bmt-slot-name" :style="{ fontFamily: activeUiStack }">
            {{ uiSlotSummary }}
          </div>
          <div class="bmt-slot-sample" :style="{ fontFamily: activeUiStack }">
            微风拂面 · BreezeMusic 12:34
          </div>
          <div class="bmt-slot-actions">
            <button type="button" class="bms-btn primary sm" @click="openFontPicker('ui')">
              <span>选择字体…</span>
            </button>
          </div>
        </div>

        <!-- 槽位 2：播放页歌词 -->
        <div class="bmt-slot-card">
          <div class="bmt-slot-top">
            <span class="bmt-slot-label">播放页歌词</span>
            <span class="bms-chip muted">
              {{ store.typographyConfig.stageLyricMode === "custom" ? "独立字体" : store.typographyConfig.stageLyricMode === "follow_ui" ? "跟随界面" : "内置预设" }}
            </span>
          </div>
          <div class="bmt-slot-name">{{ stageSlotSummary }}</div>
          <div class="bmt-mode-pills">
            <button
              v-for="opt in stageModeOptions"
              :key="opt.id"
              type="button"
              class="bmt-mini-pill"
              :class="{ active: store.typographyConfig.stageLyricMode === opt.id }"
              @click="setStageLyricMode(opt.id)"
            >
              {{ opt.label }}
            </button>
          </div>
          <div class="bmt-slot-actions">
            <button type="button" class="bms-btn sm" @click="openFontPicker('stageLyric')">
              <span>挑选歌词字体…</span>
            </button>
          </div>
        </div>

        <!-- 槽位 3：桌面悬浮歌词 -->
        <div class="bmt-slot-card">
          <div class="bmt-slot-top">
            <span class="bmt-slot-label">桌面悬浮歌词</span>
            <span class="bms-chip muted">
              {{ store.typographyConfig.desktopLyricMode === "custom" ? "独立字体" : store.typographyConfig.desktopLyricMode === "follow_stage" ? "跟随播放页" : store.typographyConfig.desktopLyricMode === "follow_ui" ? "跟随界面" : "内置预设" }}
            </span>
          </div>
          <div class="bmt-slot-name">{{ desktopSlotSummary }}</div>
          <div class="bmt-mode-pills">
            <button
              v-for="opt in desktopModeOptions"
              :key="opt.id"
              type="button"
              class="bmt-mini-pill"
              :class="{ active: store.typographyConfig.desktopLyricMode === opt.id }"
              @click="setDesktopLyricMode(opt.id)"
            >
              {{ opt.label }}
            </button>
          </div>
          <div class="bmt-slot-actions">
            <button type="button" class="bms-btn sm" @click="openFontPicker('desktopLyric')">
              <span>挑选桌面字体…</span>
            </button>
          </div>
        </div>
      </div>
    </SettingsCard>

    <!-- 卡片 2：字号缩放 (75%–145%)、字重粗细与行距 -->
    <SettingsCard
      icon="sliders"
      icon-color="var(--primary)"
      title="字号缩放、字重与排版舒适度"
      subtitle="全软件实时联动生效，支持超宽 75%–145% 缩放与五级派生字重"
    >
      <!-- 全局字号缩放 -->
      <SettingsRow
        setting-id="typography-weight-scale"
        :highlighted="highlightedId === 'typography-weight-scale'"
        vertical
        title="全局字号缩放比例"
        description="按比例放大或收紧导航、列表、卡片与弹窗文字，适应不同分辨率与观看距离"
        :badge="`${uiScalePercent}%`"
      >
        <div class="bmt-scale-stack">
          <div class="bms-pill-group">
            <button
              v-for="p in scalePresets"
              :key="p.value"
              type="button"
              class="bms-pill-opt"
              :class="{ active: Math.abs(store.typographyConfig.uiFontScale - p.value) < 0.005 }"
              @click="setUiScale(p.value)"
            >
              {{ p.label }}
            </button>
          </div>
          <div class="bms-slider-wrap bmt-wide-slider">
            <span class="bmt-slider-bound">75%</span>
            <input
              type="range"
              class="bms-range"
              min="75"
              max="145"
              step="1"
              :value="uiScalePercent"
              aria-label="全局字号缩放百分比"
              @input="handleScaleSliderInput"
            />
            <span class="bmt-slider-bound">145%</span>
          </div>
        </div>
      </SettingsRow>

      <!-- 字重粗细 -->
      <SettingsRow
        vertical
        title="全局基础字重"
        description="同步驱动正文、标签与各级标题的视觉粗细（300 细体自动切换 Windows Light 细体字面）"
        :badge="formatWeightLabel(store.typographyConfig.uiBaseWeight)"
      >
        <div class="bmt-weight-stack">
          <div class="bms-pill-group">
            <button
              v-for="w in weightOptions"
              :key="w.value"
              type="button"
              class="bms-pill-opt"
              :class="{ active: store.typographyConfig.uiBaseWeight === w.value }"
              @click="setUiWeight(w.value)"
            >
              {{ w.label }}
            </button>
          </div>

          <!-- 实时粗细对比样本条 -->
          <div class="bmt-weight-strip">
            <span class="bmt-ws-item" :style="{ fontWeight: derivedWeights.regular }">
              正文 Regular ({{ derivedWeights.regular }})
            </span>
            <span class="bmt-ws-item" :style="{ fontWeight: derivedWeights.semibold }">
              副标题 Semibold ({{ derivedWeights.semibold }})
            </span>
            <span class="bmt-ws-item" :style="{ fontWeight: derivedWeights.heavy }">
              大标题 Heavy ({{ derivedWeights.heavy }})
            </span>
          </div>
        </div>
      </SettingsRow>

      <!-- 全局行距 -->
      <SettingsRow
        title="全局文字行距"
        description="调整多行说明、歌词简介与歌单介绍的纵向阅读呼吸感"
        :badge="`${store.typographyConfig.uiLineHeight.toFixed(2)}×`"
      >
        <div class="bms-pill-group">
          <button
            v-for="lh in lineHeightOptions"
            :key="lh.value"
            type="button"
            class="bms-pill-opt"
            :class="{ active: Math.abs(store.typographyConfig.uiLineHeight - lh.value) < 0.01 }"
            @click="setLineHeight(lh.value)"
          >
            {{ lh.label }}
          </button>
        </div>
      </SettingsRow>

      <!-- 西文与数字混排 -->
      <SettingsRow
        title="英文与数字混排回退"
        description="优先为西文与码率数字匹配高辨识度字体，中文继续由主字体渲染"
      >
        <div class="bms-pill-group">
          <button
            v-for="opt in latinFallbackOptions"
            :key="opt.id"
            type="button"
            class="bms-pill-opt"
            :class="{ active: store.typographyConfig.latinFallback === opt.id }"
            @click="setLatinFallback(opt.id)"
          >
            {{ opt.label }}
          </button>
        </div>
      </SettingsRow>

      <!-- 等宽数字对齐 -->
      <SettingsRow
        title="强制等宽数字对齐 (tabular-nums)"
        description="播放进度计时、文件大小与码率数字逐位对齐，跳秒时不产生横向抖动"
      >
        <SettingsSwitch
          :model-value="store.typographyConfig.tabularNumbers"
          aria-label="切换等宽数字对齐"
          @update:model-value="
            (val) => store.updateTypographyConfig({ tabularNumbers: val })
          "
        />
      </SettingsRow>
    </SettingsCard>

    <FontPickerModal
      :open="pickerOpen"
      :initial-scope="pickerInitialScope"
      @close="pickerOpen = false"
    />
  </div>
</template>

<style scoped>
.bmt-slot-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 14px;
  border-radius: var(--bm-r-lg, 18px);
}

.bmt-slot-grid.is-flash-highlight {
  box-shadow: 0 0 28px -2px color-mix(in srgb, var(--primary) 50%, transparent);
}

.bmt-slot-card {
  border-radius: 14px;
  padding: 15px 16px;
  background: var(--bms-surface-tile);
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.bmt-slot-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.bmt-slot-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-secondary);
}

.bmt-slot-name {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bmt-slot-sample {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bmt-mode-pills {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.bmt-mini-pill {
  border: none;
  border-radius: var(--bm-r-pill, 999px);
  padding: 3px 9px;
  background: var(--bms-surface-well);
  color: var(--text-secondary);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-semibold, 600);
  cursor: pointer;
}

.bmt-mini-pill.active {
  background: var(--primary-soft);
  color: var(--primary-text, var(--primary));
}

.bmt-slot-actions {
  margin-top: auto;
  padding-top: 4px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.bmt-scale-stack,
.bmt-weight-stack {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.bmt-wide-slider {
  width: 100%;
  max-width: 560px;
}

.bmt-slider-bound {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}

.bmt-weight-strip {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 16px;
  padding: 10px 14px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--bms-surface-well);
}

.bmt-ws-item {
  font-size: var(--bm-fs-meta, 13.5px);
  color: var(--text-primary);
}

/* L2 晨雾燕麦 / D2 丝绒星云特化 */
.scheme-watercolor_light.light-variant-nordic_oat .bmt-slot-card,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bmt-slot-card {
  background: var(--bms-surface-row);
}

.scheme-watercolor_light.light-variant-nordic_oat .bmt-mini-pill,
.scheme-watercolor_light.light-variant-nordic_oat .bmt-weight-strip {
  background: #FAF6EE;
}

@container settings-stage (max-width: 820px) {
  .bmt-slot-grid {
    grid-template-columns: 1fr;
  }
}
</style>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useAppStore } from "../../stores/appStore";
import type {
  DesktopLyricLayoutMode,
  DesktopLyricScenePresetId,
  DesktopLyricStrokeColorMode,
  DesktopLyricStrokeWidth,
  LyricAlignment,
  LyricFontFamily,
  LyricFontWeightLevel,
  LyricHighlightThemeId,
  LyricScenePresetId,
  LyricUnsungPresetId,
} from "../../types";
import {
  DESKTOP_LYRIC_SCENE_PRESETS,
  DESKTOP_STROKE_COLOR_OPTIONS,
  DESKTOP_STROKE_WIDTH_OPTIONS,
  LYRIC_FONT_FAMILY_OPTIONS,
  LYRIC_FONT_WEIGHT_OPTIONS,
  LYRIC_MOTION_STYLE_OPTIONS,
  LYRIC_SCENE_PRESETS,
  LYRIC_SCROLL_CASCADE_OPTIONS,
  LYRIC_SUNG_PRESETS,
  LYRIC_UNSUNG_PRESETS,
} from "../../utils/lyricTypography";
import PlumpIcon from "../PlumpIcon.vue";
import SettingsCard from "./SettingsCard.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSwitch from "./SettingsSwitch.vue";

const props = defineProps<{
  highlightedId?: string;
}>();

const store = useAppStore();
const activeTab = ref<"desktop" | "stage">("desktop");

watch(
  () => props.highlightedId,
  (id) => {
    if (id === "lyrics-stage-prefs") activeTab.value = "stage";
    else if (id === "lyrics-desktop-main" || id === "lyrics-desktop-style") {
      activeTab.value = "desktop";
    }
  },
  { immediate: true }
);

function selectStageLyricPresetFamily(fam: LyricFontFamily) {
  store.lyricFontFamily = fam;
  if (store.typographyConfig.stageLyricMode !== "preset") {
    void store.updateTypographyConfig({ stageLyricMode: "preset" });
    store.notify("已切换回内置歌词预设字体，使当前字形选择立即生效");
  }
}

function selectStageLyricWeight(w: LyricFontWeightLevel) {
  store.lyricFontWeight = w;
  if (store.typographyConfig.stageLyricMode === "custom") {
    void store.updateTypographyConfig({ stageLyricWeight: w });
  }
}

function selectDesktopLyricPresetFamily(fam: LyricFontFamily) {
  store.desktopLyricFontFamily = fam;
  if (store.typographyConfig.desktopLyricMode !== "preset") {
    void store.updateTypographyConfig({ desktopLyricMode: "preset" });
    store.notify("已切换回桌面歌词内置预设字体，使当前字形选择立即生效");
  }
}

function selectDesktopLyricWeight(w: LyricFontWeightLevel) {
  store.desktopLyricFontWeight = w;
  if (store.typographyConfig.desktopLyricMode === "custom") {
    void store.updateTypographyConfig({ desktopLyricWeight: w });
  }
}

function handleApplyDesktopPreset(
  presetId: Exclude<DesktopLyricScenePresetId, "custom">
) {
  store.applyDesktopLyricScenePreset(presetId);
  const found = DESKTOP_LYRIC_SCENE_PRESETS.find((p) => p.id === presetId);
  if (found) {
    store.notify(`已应用桌面歌词「${found.label}」场景预设`);
  }
}

function selectDesktopSungPreset(id: Exclude<LyricHighlightThemeId, "custom">) {
  store.setDesktopLyricColorScheme(id);
}

function pickDesktopSungCustomPrimary(hex: string) {
  store.desktopLyricCustomPrimary = hex;
  store.setDesktopLyricColorScheme("custom", hex);
}

function pickDesktopSungCustomSecondary(hex: string) {
  store.desktopLyricCustomSecondary = hex;
  store.setDesktopLyricColorScheme("custom");
}

function selectDesktopUnsungPreset(id: Exclude<LyricUnsungPresetId, "custom">) {
  store.desktopLyricUnsungPreset = id;
}

function pickDesktopUnsungCustom(hex: string) {
  store.desktopLyricUnsungCustomHex = hex;
  store.desktopLyricUnsungPreset = "custom";
}

function selectDesktopStrokeColor(mode: DesktopLyricStrokeColorMode) {
  store.desktopLyricStrokeColorMode = mode;
}

function pickDesktopStrokeCustom(hex: string) {
  store.desktopLyricStrokeCustomHex = hex;
  store.desktopLyricStrokeColorMode = "custom";
}

function handleApplyScenePreset(presetId: Exclude<LyricScenePresetId, "custom">) {
  store.applyLyricScenePreset(presetId);
  const found = LYRIC_SCENE_PRESETS.find((p) => p.id === presetId);
  if (found) {
    store.notify(`已应用「${found.label}」歌词场景预设`);
  }
}

const dlLayoutOptions: ReadonlyArray<{ id: DesktopLyricLayoutMode; label: string }> = [
  { id: "center", label: "双行居中" },
  { id: "ktv", label: "KTV 交替" },
  { id: "single", label: "极简单行" },
  { id: "left", label: "双行居左" },
];

const dlStrokeOptions = DESKTOP_STROKE_WIDTH_OPTIONS;
const dlStrokeColorOptions = DESKTOP_STROKE_COLOR_OPTIONS;
const dlScenePresetOptions = DESKTOP_LYRIC_SCENE_PRESETS;
const dlSungPresets = LYRIC_SUNG_PRESETS;
const dlUnsungPresets = LYRIC_UNSUNG_PRESETS;

const activeDesktopPresetBadge = computed(() => {
  if (store.activeDesktopLyricPreset === "custom") return "自定义微调";
  const found = DESKTOP_LYRIC_SCENE_PRESETS.find(
    (p) => p.id === store.activeDesktopLyricPreset
  );
  return found ? `当前：${found.label}` : "自定义微调";
});

const fontFamilyOptions = LYRIC_FONT_FAMILY_OPTIONS;
const fontWeightOptions = LYRIC_FONT_WEIGHT_OPTIONS;
const motionStyleOptions = LYRIC_MOTION_STYLE_OPTIONS;
const scrollCascadeOptions = LYRIC_SCROLL_CASCADE_OPTIONS;
const scenePresetOptions = LYRIC_SCENE_PRESETS;

const alignmentOptions: ReadonlyArray<{ id: LyricAlignment; label: string }> = [
  { id: "left", label: "居左" },
  { id: "center", label: "居中" },
  { id: "right", label: "居右" },
];

const autoReturnOptions: ReadonlyArray<2 | 3 | 5> = [2, 3, 5];

const unlockShortcutLabel = computed(
  () => store.globalHotkeys.toggle_lyric_lock || "Ctrl+Shift+L"
);

const lyricOffsetText = computed(() => {
  const ms = store.lyricOffsetMs || 0;
  if (ms === 0) return "0.0s 准时";
  const sec = (ms / 1000).toFixed(1);
  return ms > 0 ? `+${sec}s (提前)` : `${sec}s (延后)`;
});

const activeScenePresetBadge = computed(() => {
  if (store.activeLyricScenePreset === "custom") return "自定义微调";
  const found = LYRIC_SCENE_PRESETS.find(
    (p) => p.id === store.activeLyricScenePreset
  );
  return found ? `当前：${found.label}` : "自定义微调";
});

function handleSyncFromNowPlaying() {
  store.syncDesktopLyricFromNowPlaying();
  store.notify("已将播放页歌词配色与字体风格同步至桌面歌词");
}

function handleResetDesktopLyric() {
  store.resetDesktopLyricSettingsToDefault();
  store.notify("桌面歌词外观与排版已恢复出厂默认");
}

function handleResetStageLyric() {
  store.resetLyricSettingsToDefault();
  store.notify("播放页歌词偏好已恢复出厂默认");
}
</script>

<template>
  <div class="bms-panel-stack">
    <div class="bms-subtabs" role="tablist" aria-label="歌词设置分区">
      <button
        type="button"
        role="tab"
        class="bms-subtab-btn"
        :class="{ active: activeTab === 'desktop' }"
        :aria-selected="activeTab === 'desktop'"
        @click="activeTab = 'desktop'"
      >
        <PlumpIcon
          name="mini-island"
          :size="14"
          :color="activeTab === 'desktop' ? 'var(--primary-text, var(--primary))' : 'var(--text-muted)'"
        />
        <span>桌面悬浮歌词</span>
        <span v-if="store.desktopLyricEnabled" class="bms-subtab-count">已开启</span>
      </button>
      <button
        type="button"
        role="tab"
        class="bms-subtab-btn"
        :class="{ active: activeTab === 'stage' }"
        :aria-selected="activeTab === 'stage'"
        @click="activeTab = 'stage'"
      >
        <PlumpIcon
          name="lyric"
          :size="14"
          :color="activeTab === 'stage' ? 'var(--primary-text, var(--primary))' : 'var(--text-muted)'"
        />
        <span>播放页歌词</span>
      </button>
    </div>

    <Transition
      :name="store.motionLevel === 'none' ? '' : 'bms-pane-fade'"
      mode="out-in"
    >
    <!-- 1. 独立透明桌面悬浮歌词（与悬浮窗工作台 100% 双向实时同步） -->
    <SettingsCard
      v-if="activeTab === 'desktop'"
      key="desktop"
      title="独立透明桌面悬浮歌词"
      subtitle="100% 纯净无底浮空置顶透明窗，搭载播放页同源 4 套 GPU 逐字引擎、圆角抗锯齿轮廓与多线程硬件级鼠标穿透"
      icon="lyric"
      icon-color="var(--violet)"
    >
      <template #headerRight>
        <div class="bms-head-btn-row">
          <button type="button" class="bms-btn sm" @click="handleSyncFromNowPlaying">
            <PlumpIcon name="refresh" :size="12" />
            <span>同步播放页风格</span>
          </button>
          <button type="button" class="bms-btn sm ghost" @click="handleResetDesktopLyric">
            <PlumpIcon name="reset-rotate" :size="12" />
            <span>恢复默认</span>
          </button>
        </div>
      </template>

      <SettingsRow
        title="开启桌面悬浮歌词"
        description="开启后在任意桌面窗口、全屏视频与游戏上方纯净浮空展示当前歌曲逐字歌词"
        icon="lyric"
        icon-color="var(--violet)"
        :badge="store.desktopLyricEnabled ? '显示中' : '已关闭'"
        :badge-tone="store.desktopLyricEnabled ? 'mint' : 'muted'"
        label-for="bms-sw-dl-enabled"
        setting-id="lyrics-desktop-main"
        :highlighted="highlightedId === 'lyrics-desktop-main'"
      >
        <SettingsSwitch
          id="bms-sw-dl-enabled"
          v-model="store.desktopLyricEnabled"
          aria-label="开启桌面悬浮歌词"
        />
      </SettingsRow>

      <SettingsRow
        title="锁定桌面歌词（硬件级鼠标穿透）"
        :description="`锁定时鼠标将穿透歌词直接点击下方应用，可通过此处、托盘或 ${unlockShortcutLabel} 解锁`"
        :icon="store.desktopLyricLocked ? 'lock' : 'unlock'"
        icon-color="var(--coral)"
        label-for="bms-sw-dl-lock"
      >
        <SettingsSwitch
          id="bms-sw-dl-lock"
          v-model="store.desktopLyricLocked"
          :disabled="!store.desktopLyricEnabled"
          aria-label="锁定桌面歌词"
        />
      </SettingsRow>

      <SettingsRow
        title="锁定态鼠标靠近柔和避让"
        description="锁定穿透时，鼠标移入歌词区域自动柔和淡隐歌词避免遮挡底下文字与按钮，移开后瞬时恢复"
        icon="eye"
        icon-color="var(--cyan)"
        label-for="bms-sw-dl-dodge"
      >
        <SettingsSwitch
          id="bms-sw-dl-dodge"
          v-model="store.desktopLyricHoverDodge"
          aria-label="锁定态鼠标靠近柔和避让"
        />
      </SettingsRow>

      <SettingsRow
        title="一键屏幕归位"
        description="若多屏切换或误拖出边界，可立即将桌面歌词归位到当前显示器安全区域"
        icon="locate"
        icon-color="var(--cyan)"
      >
        <div class="bms-pill-group">
          <button
            type="button"
            class="bms-pill-opt"
            @click="store.snapDesktopLyricPosition('top')"
          >
            顶部居中
          </button>
          <button
            type="button"
            class="bms-pill-opt"
            @click="store.snapDesktopLyricPosition('center')"
          >
            屏幕中央
          </button>
          <button
            type="button"
            class="bms-pill-opt"
            @click="store.snapDesktopLyricPosition('bottom')"
          >
            底部居中
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="桌面歌词一键高定场景预设"
        description="针对 100% 纯净无底浮空特调的五套黄金配比，一键切换动效、轮廓、柔影与字形（不影响自定义配色与偏移）"
        icon="sparkles"
        icon-color="var(--primary)"
        :badge="activeDesktopPresetBadge"
        :badge-tone="store.activeDesktopLyricPreset === 'custom' ? 'muted' : 'primary'"
        setting-id="lyrics-desktop-style"
        :highlighted="highlightedId === 'lyrics-desktop-style'"
        vertical
      >
        <div class="bms-scene-preset-grid" role="radiogroup" aria-label="桌面歌词场景预设">
          <button
            v-for="preset in dlScenePresetOptions"
            :key="preset.id"
            type="button"
            role="radio"
            class="bms-scene-preset-tile"
            :class="{ active: store.activeDesktopLyricPreset === preset.id }"
            :aria-checked="store.activeDesktopLyricPreset === preset.id"
            @click="handleApplyDesktopPreset(preset.id)"
          >
            <span
              class="bms-scene-dot"
              :style="{ background: preset.accentSwatch }"
              aria-hidden="true"
            ></span>
            <span class="bms-scene-name">{{ preset.label }}</span>
            <span class="bms-scene-tag">{{ preset.tagline }}</span>
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="桌面逐字动效引擎"
        description="复用播放页同源四套纯 GPU 合成逐字引擎（桌面端自动收敛微浮幅度以减少余光干扰），支持独立记忆"
        icon="wave"
        icon-color="var(--cyan)"
        vertical
      >
        <div class="bms-motion-engine-grid" role="radiogroup" aria-label="桌面歌词逐字动效引擎">
          <button
            v-for="ms in motionStyleOptions"
            :key="ms.id"
            type="button"
            role="radio"
            class="bms-motion-engine-card"
            :class="{ active: store.desktopLyricMotionStyle === ms.id }"
            :aria-checked="store.desktopLyricMotionStyle === ms.id"
            @click="store.desktopLyricMotionStyle = ms.id"
          >
            <div class="bms-me-top">
              <span class="bms-me-title">{{ ms.label }}</span>
              <span v-if="ms.badge" class="bms-me-badge">{{ ms.badge }}</span>
            </div>
            <span class="bms-me-desc">{{ ms.desc }}</span>
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="已唱流光与未唱双通道配色"
        description="整行连贯 OKLab 双色流光彻底消除逐词斑马纹，支持纯色/渐变与未唱底色独立精细定制"
        icon="palette-orb"
        icon-color="var(--primary)"
        vertical
      >
        <div class="bms-inline-groups">
          <div class="bms-pill-group" role="radiogroup" aria-label="已唱色彩模式">
            <span class="bms-group-hint">已唱模式：</span>
            <button
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.desktopLyricSungMode === 'solid' }"
              :aria-checked="store.desktopLyricSungMode === 'solid'"
              @click="store.desktopLyricSungMode = 'solid'"
            >
              纯净单色
            </button>
            <button
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.desktopLyricSungMode === 'gradient' }"
              :aria-checked="store.desktopLyricSungMode === 'gradient'"
              @click="store.desktopLyricSungMode = 'gradient'"
            >
              整行流光渐变
            </button>
          </div>
        </div>

        <div class="bms-dl-swatch-row">
          <button
            v-for="p in dlSungPresets"
            :key="p.id"
            type="button"
            class="bms-dl-swatch-chip"
            :class="{ active: store.desktopLyricColorScheme === p.id }"
            @click="selectDesktopSungPreset(p.id)"
          >
            <span
              class="bms-dl-swatch-dot"
              :style="{
                background: p.isDynamic
                  ? `conic-gradient(from 30deg, ${p.darkPrimary}, ${p.darkSecondary}, #f43f5e, ${p.darkPrimary})`
                  : store.desktopLyricSungMode === 'solid'
                  ? p.darkPrimary
                  : `linear-gradient(135deg, ${p.darkPrimary} 0%, ${p.darkSecondary} 100%)`,
              }"
            ></span>
            <span>{{ p.label }}</span>
          </button>

          <label
            class="bms-dl-swatch-chip"
            :class="{ active: store.desktopLyricColorScheme === 'custom' }"
          >
            <input
              type="color"
              class="bms-dl-color-input"
              aria-label="自定义桌面已唱起色"
              :value="store.desktopLyricCustomPrimary"
              @input="pickDesktopSungCustomPrimary(($event.target as HTMLInputElement).value)"
            />
            <span>{{ store.desktopLyricSungMode === 'solid' ? '自选纯色' : '自选起色' }}</span>
          </label>

          <label
            v-if="store.desktopLyricSungMode === 'gradient'"
            class="bms-dl-swatch-chip"
            :class="{ active: store.desktopLyricColorScheme === 'custom' }"
          >
            <input
              type="color"
              class="bms-dl-color-input"
              aria-label="自定义桌面已唱终色"
              :value="store.desktopLyricCustomSecondary"
              @input="pickDesktopSungCustomSecondary(($event.target as HTMLInputElement).value)"
            />
            <span>自选终色</span>
          </label>
        </div>

        <div class="bms-dl-swatch-row">
          <span class="bms-group-hint">未唱底色：</span>
          <button
            v-for="u in dlUnsungPresets"
            :key="u.id"
            type="button"
            class="bms-dl-swatch-chip"
            :class="{ active: store.desktopLyricUnsungPreset === u.id }"
            @click="selectDesktopUnsungPreset(u.id)"
          >
            <span class="bms-dl-swatch-dot" :style="{ background: u.swatchDark }"></span>
            <span>{{ u.label }}</span>
          </button>
          <label
            class="bms-dl-swatch-chip"
            :class="{ active: store.desktopLyricUnsungPreset === 'custom' }"
          >
            <input
              type="color"
              class="bms-dl-color-input"
              aria-label="自定义桌面未唱文字色"
              :value="store.desktopLyricUnsungCustomHex"
              @input="pickDesktopUnsungCustom(($event.target as HTMLInputElement).value)"
            />
            <span>自选</span>
          </label>
        </div>
      </SettingsRow>

      <SettingsRow
        title="圆角抗锯齿轮廓与光学柔影"
        description="默认零描边搭配三层光学空气柔影最轻盈通透；开启轮廓时采用 SVG 圆角外扩工艺彻底消除汉字折角尖刺"
        icon="sparkles"
        icon-color="var(--gold)"
        vertical
      >
        <div class="bms-inline-groups">
          <div class="bms-pill-group" role="radiogroup" aria-label="桌面歌词轮廓粗细">
            <span class="bms-group-hint">圆角轮廓：</span>
            <button
              v-for="st in dlStrokeOptions"
              :key="st.id"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.desktopLyricStrokeWidth === st.id }"
              :aria-checked="store.desktopLyricStrokeWidth === st.id"
              @click="store.desktopLyricStrokeWidth = st.id"
            >
              {{ st.label }}
            </button>
          </div>

          <div
            v-if="store.desktopLyricStrokeWidth !== 'none'"
            class="bms-pill-group"
            role="radiogroup"
            aria-label="桌面歌词轮廓颜色"
          >
            <span class="bms-group-hint">轮廓色：</span>
            <button
              v-for="sc in dlStrokeColorOptions"
              :key="sc.id"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.desktopLyricStrokeColorMode === sc.id }"
              :aria-checked="store.desktopLyricStrokeColorMode === sc.id"
              @click="selectDesktopStrokeColor(sc.id)"
            >
              {{ sc.label }}
            </button>
            <label
              class="bms-dl-swatch-chip compact"
              :class="{ active: store.desktopLyricStrokeColorMode === 'custom' }"
            >
              <input
                type="color"
                class="bms-dl-color-input"
                aria-label="自定义桌面轮廓颜色"
                :value="store.desktopLyricStrokeCustomHex"
                @input="pickDesktopStrokeCustom(($event.target as HTMLInputElement).value)"
              />
              <span>自选</span>
            </label>
          </div>
        </div>
      </SettingsRow>

      <SettingsRow
        title="桌面歌词排版布局与字形字重"
        :description="
          store.desktopLyricFontOverride.familyStack
            ? `当前已由「字体与排版」自定义覆写：${store.desktopLyricFontOverride.label}`
            : '支持经典 KTV 上左下右近似同级交替、超长句平滑随唱横推，以及四款内置字形与四档真实字重'
        "
        icon="lyric-layout-center"
        icon-color="var(--primary)"
        :badge="store.desktopLyricFontOverride.familyStack ? '已自定义字体' : ''"
        badge-tone="primary"
        vertical
      >
        <div class="bms-inline-groups">
          <div class="bms-pill-group" role="radiogroup" aria-label="桌面歌词排版布局">
            <span class="bms-group-hint">布局形态：</span>
            <button
              v-for="opt in dlLayoutOptions"
              :key="opt.id"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.desktopLyricLayoutMode === opt.id }"
              :aria-checked="store.desktopLyricLayoutMode === opt.id"
              @click="store.desktopLyricLayoutMode = opt.id"
            >
              {{ opt.label }}
            </button>
          </div>

          <button
            type="button"
            class="bms-btn sm"
            @click="store.openSettingsSection('typography')"
          >
            <PlumpIcon name="font-plus" :size="12" />
            <span>前往「字体与排版」自定义电脑 / 导入字体</span>
          </button>
        </div>

        <div class="bms-inline-groups">
          <div class="bms-pill-group" role="radiogroup" aria-label="桌面歌词字体">
            <button
              v-for="f in fontFamilyOptions"
              :key="f.id"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{
                active:
                  store.desktopLyricFontFamily === f.id &&
                  store.typographyConfig.desktopLyricMode === 'preset',
              }"
              :aria-checked="
                store.desktopLyricFontFamily === f.id &&
                store.typographyConfig.desktopLyricMode === 'preset'
              "
              @click="selectDesktopLyricPresetFamily(f.id)"
            >
              {{ f.label }}
            </button>
          </div>
          <div class="bms-pill-group" role="radiogroup" aria-label="桌面歌词字重">
            <button
              v-for="w in fontWeightOptions"
              :key="w.id"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.desktopLyricFontWeight === w.id }"
              :aria-checked="store.desktopLyricFontWeight === w.id"
              @click="selectDesktopLyricWeight(w.id)"
            >
              {{ w.label }}
            </button>
          </div>
        </div>
      </SettingsRow>

      <SettingsRow
        title="字号、宽幅、辉光与透明度"
        description="实时微调透明悬浮窗的视觉尺码、行距、逐字辉光、未唱透明与整体不透明度"
        icon="font-plus"
        icon-color="var(--primary)"
        vertical
      >
        <div class="bms-dual-sliders">
          <div class="bms-slider-field">
            <label for="bms-dl-fontsize" class="bms-slider-label">字号大小</label>
            <div class="bms-slider-wrap">
              <input
                id="bms-dl-fontsize"
                v-model.number="store.desktopLyricFontSize"
                type="range"
                min="18"
                max="56"
                step="1"
                class="bms-range"
              />
              <span class="bms-range-val">{{ store.desktopLyricFontSize }} px</span>
            </div>
          </div>

          <div class="bms-slider-field">
            <label for="bms-dl-width" class="bms-slider-label">窗口宽幅</label>
            <div class="bms-slider-wrap">
              <input
                id="bms-dl-width"
                v-model.number="store.desktopLyricWidth"
                type="range"
                min="440"
                max="1280"
                step="20"
                class="bms-range"
              />
              <span class="bms-range-val">{{ store.desktopLyricWidth }} px</span>
            </div>
          </div>

          <div class="bms-slider-field">
            <label for="bms-dl-spacing" class="bms-slider-label">双行间距</label>
            <div class="bms-slider-wrap">
              <input
                id="bms-dl-spacing"
                v-model.number="store.desktopLyricLineSpacing"
                type="range"
                min="0"
                max="16"
                step="1"
                class="bms-range"
              />
              <span class="bms-range-val">{{ store.desktopLyricLineSpacing }} px</span>
            </div>
          </div>

          <div class="bms-slider-field">
            <label for="bms-dl-glow" class="bms-slider-label">逐字辉光强度</label>
            <div class="bms-slider-wrap">
              <input
                id="bms-dl-glow"
                v-model.number="store.desktopLyricGlowIntensity"
                type="range"
                min="0"
                max="150"
                step="5"
                class="bms-range"
              />
              <span class="bms-range-val">{{ Math.round(store.desktopLyricGlowIntensity) }}%</span>
            </div>
          </div>

          <div class="bms-slider-field">
            <label for="bms-dl-unsung-op" class="bms-slider-label">未唱文字不透明度</label>
            <div class="bms-slider-wrap">
              <input
                id="bms-dl-unsung-op"
                v-model.number="store.desktopLyricUnsungOpacity"
                type="range"
                min="0.35"
                max="0.98"
                step="0.01"
                class="bms-range"
              />
              <span class="bms-range-val">{{ Math.round(store.desktopLyricUnsungOpacity * 100) }}%</span>
            </div>
          </div>

          <div class="bms-slider-field">
            <label for="bms-dl-opacity" class="bms-slider-label">整体不透明度</label>
            <div class="bms-slider-wrap">
              <input
                id="bms-dl-opacity"
                v-model.number="store.desktopLyricOpacity"
                type="range"
                min="0.35"
                max="1"
                step="0.05"
                class="bms-range"
              />
              <span class="bms-range-val">{{ Math.round(store.desktopLyricOpacity * 100) }}%</span>
            </div>
          </div>
        </div>
      </SettingsRow>

      <div class="bms-mini-toggle-grid">
        <label class="bms-mini-toggle-card" for="bms-sw-dl-krc">
          <span>KRC 逐字演绎</span>
          <SettingsSwitch id="bms-sw-dl-krc" v-model="store.desktopLyricWordByWord" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-dl-bounce">
          <span>逐字微浮弹性</span>
          <SettingsSwitch
            id="bms-sw-dl-bounce"
            v-model="store.desktopLyricWordBounceEnabled"
            :disabled="!store.desktopLyricWordByWord || store.desktopLyricMotionStyle === 'apple_minimal'"
          />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-dl-harmony">
          <span>和声/伴唱轻量化排版</span>
          <SettingsSwitch id="bms-sw-dl-harmony" v-model="store.desktopLyricHarmonyStyleEnabled" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-dl-shadow">
          <span>多层光学空气柔影</span>
          <SettingsSwitch id="bms-sw-dl-shadow" v-model="store.desktopLyricShadowEnabled" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-dl-trans">
          <span>显示双语译文</span>
          <SettingsSwitch id="bms-sw-dl-trans" v-model="store.desktopLyricShowTranslation" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-dl-phonetic">
          <span>显示外语罗马音/音译</span>
          <SettingsSwitch id="bms-sw-dl-phonetic" v-model="store.desktopLyricShowPhonetic" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-dl-intro">
          <span>显示切歌小标与前奏提示</span>
          <SettingsSwitch id="bms-sw-dl-intro" v-model="store.desktopLyricShowIntro" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-dl-idle">
          <span>暂停空闲自动淡隐</span>
          <SettingsSwitch id="bms-sw-dl-idle" v-model="store.desktopLyricIdleFade" />
        </label>
      </div>
    </SettingsCard>

    <!-- 2. 沉浸播放页歌词默认偏好（与播放页歌词设置工作台 100% 同源同步） -->
    <SettingsCard
      v-else
      key="stage"
      title="沉浸播放页歌词引擎与偏好"
      subtitle="与全屏播放页歌词设置工作台 100% 双向实时同步，覆盖场景预设、四套动效引擎与排版校准"
      icon="lyric-sliders"
      icon-color="var(--primary)"
      setting-id="lyrics-stage-prefs"
    >
      <template #headerRight>
        <button type="button" class="bms-btn sm ghost" @click="handleResetStageLyric">
          <PlumpIcon name="reset-rotate" :size="12" />
          <span>恢复默认</span>
        </button>
      </template>

      <SettingsRow
        title="一键高定场景预设"
        description="快速切换整套动效风格、对齐、字重、辉光与景深黄金配比（不影响自定义主题色与时间偏移）"
        icon="sparkles"
        icon-color="var(--primary)"
        :badge="activeScenePresetBadge"
        :badge-tone="store.activeLyricScenePreset === 'custom' ? 'muted' : 'primary'"
        :highlighted="highlightedId === 'lyrics-stage-prefs'"
        vertical
      >
        <div class="bms-scene-preset-grid" role="radiogroup" aria-label="播放页歌词场景预设">
          <button
            v-for="preset in scenePresetOptions"
            :key="preset.id"
            type="button"
            role="radio"
            class="bms-scene-preset-tile"
            :class="{ active: store.activeLyricScenePreset === preset.id }"
            :aria-checked="store.activeLyricScenePreset === preset.id"
            @click="handleApplyScenePreset(preset.id)"
          >
            <span
              class="bms-scene-dot"
              :style="{ background: preset.accentSwatch }"
              aria-hidden="true"
            ></span>
            <span class="bms-scene-name">{{ preset.label }}</span>
            <span class="bms-scene-tag">{{ preset.tagline }}</span>
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="逐字动效引擎与滚屏质感"
        description="四套纯 GPU 合成逐字演绎引擎搭配三档零抖动滚屏策略，全面适配 60Hz–240Hz 显示器"
        icon="wave"
        icon-color="var(--cyan)"
        vertical
      >
        <div class="bms-motion-engine-grid" role="radiogroup" aria-label="逐字动效引擎">
          <button
            v-for="ms in motionStyleOptions"
            :key="ms.id"
            type="button"
            role="radio"
            class="bms-motion-engine-card"
            :class="{ active: store.lyricMotionStyle === ms.id }"
            :aria-checked="store.lyricMotionStyle === ms.id"
            @click="store.lyricMotionStyle = ms.id"
          >
            <div class="bms-me-top">
              <span class="bms-me-title">{{ ms.label }}</span>
              <span v-if="ms.badge" class="bms-me-badge">{{ ms.badge }}</span>
            </div>
            <span class="bms-me-desc">{{ ms.desc }}</span>
          </button>
        </div>

        <div class="bms-inline-groups">
          <div class="bms-pill-group" role="radiogroup" aria-label="歌词滚屏质感">
            <span class="bms-group-hint">滚屏质感：</span>
            <button
              v-for="sc in scrollCascadeOptions"
              :key="sc.id"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.lyricScrollCascadeMode === sc.id }"
              :aria-checked="store.lyricScrollCascadeMode === sc.id"
              :title="sc.desc"
              @click="store.lyricScrollCascadeMode = sc.id"
            >
              {{ sc.label }}
            </button>
          </div>

          <div class="bms-pill-group" role="radiogroup" aria-label="手动滚动歌词后自动回位延时">
            <span class="bms-group-hint">自动回位：</span>
            <button
              v-for="sec in autoReturnOptions"
              :key="sec"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.lyricAutoReturnSec === sec }"
              :aria-checked="store.lyricAutoReturnSec === sec"
              @click="store.lyricAutoReturnSec = sec"
            >
              {{ sec }} 秒
            </button>
          </div>
        </div>
      </SettingsRow>

      <SettingsRow
        title="字形风格与四档字重"
        :description="
          store.stageLyricFontOverride.familyStack
            ? `当前已由「字体与排版」自定义覆写：${store.stageLyricFontOverride.label}`
            : '与播放页歌词工作台共用四款内置字形与四档真实字重阶梯（亦可在「字体与排版」导入本机字体）'
        "
        icon="font-plus"
        icon-color="var(--primary)"
        :badge="store.stageLyricFontOverride.familyStack ? '已自定义字体' : ''"
        badge-tone="primary"
        vertical
      >
        <div class="bms-inline-groups">
          <button
            type="button"
            class="bms-btn sm"
            @click="store.openSettingsSection('typography')"
          >
            <PlumpIcon name="font-plus" :size="12" />
            <span>前往「字体与排版」自定义电脑 / 导入字体</span>
          </button>
        </div>
        <div class="bms-inline-groups">
          <div class="bms-pill-group" role="radiogroup" aria-label="播放页歌词字体">
            <button
              v-for="f in fontFamilyOptions"
              :key="f.id"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.lyricFontFamily === f.id && store.typographyConfig.stageLyricMode === 'preset' }"
              :aria-checked="store.lyricFontFamily === f.id && store.typographyConfig.stageLyricMode === 'preset'"
              @click="selectStageLyricPresetFamily(f.id)"
            >
              {{ f.label }}
            </button>
          </div>
          <div class="bms-pill-group" role="radiogroup" aria-label="播放页歌词字重">
            <button
              v-for="w in fontWeightOptions"
              :key="w.id"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.lyricFontWeight === w.id }"
              :aria-checked="store.lyricFontWeight === w.id"
              @click="selectStageLyricWeight(w.id)"
            >
              {{ w.label }}
            </button>
          </div>
          <div class="bms-pill-group" role="radiogroup" aria-label="歌词对齐方式">
            <button
              v-for="al in alignmentOptions"
              :key="al.id"
              type="button"
              role="radio"
              class="bms-pill-opt"
              :class="{ active: store.lyricAlignment === al.id }"
              :aria-checked="store.lyricAlignment === al.id"
              @click="store.lyricAlignment = al.id"
            >
              {{ al.label }}
            </button>
          </div>
        </div>
      </SettingsRow>

      <SettingsRow
        title="字号、行距与光学质感"
        description="细腻调节主词尺码、行间呼吸距、焦点放大倍率与逐字辉光强度"
        icon="lyric-layout-left"
        icon-color="var(--cyan)"
        vertical
      >
        <div class="bms-dual-sliders">
          <div class="bms-slider-field">
            <label for="bms-st-fontsize" class="bms-slider-label">主歌词字号</label>
            <div class="bms-slider-wrap">
              <input
                id="bms-st-fontsize"
                v-model.number="store.lyricFontSize"
                type="range"
                min="20"
                max="48"
                step="1"
                class="bms-range"
              />
              <span class="bms-range-val">{{ store.lyricFontSize }} px</span>
            </div>
          </div>

          <div class="bms-slider-field">
            <label for="bms-st-spacing" class="bms-slider-label">行间呼吸距</label>
            <div class="bms-slider-wrap">
              <input
                id="bms-st-spacing"
                v-model.number="store.lyricLineSpacing"
                type="range"
                min="8"
                max="28"
                step="1"
                class="bms-range"
              />
              <span class="bms-range-val">{{ store.lyricLineSpacing }} px</span>
            </div>
          </div>

          <div class="bms-slider-field">
            <label for="bms-st-focus-ratio" class="bms-slider-label">焦点放大倍率</label>
            <div class="bms-slider-wrap">
              <input
                id="bms-st-focus-ratio"
                v-model.number="store.lyricFocusScaleRatio"
                type="range"
                min="1"
                max="1.24"
                step="0.02"
                :disabled="!store.lyricFocusScaleEnabled"
                class="bms-range"
              />
              <span class="bms-range-val">{{ Number(store.lyricFocusScaleRatio).toFixed(2) }}x</span>
            </div>
          </div>

          <div class="bms-slider-field">
            <label for="bms-st-glow" class="bms-slider-label">逐字辉光强度</label>
            <div class="bms-slider-wrap">
              <input
                id="bms-st-glow"
                v-model.number="store.lyricGlowIntensity"
                type="range"
                min="0"
                max="200"
                step="5"
                class="bms-range"
              />
              <span class="bms-range-val">{{ Math.round(store.lyricGlowIntensity) }}%</span>
            </div>
          </div>
        </div>
      </SettingsRow>

      <SettingsRow
        title="全局歌词时间偏移校准"
        description="微调歌词整体提前或延后，对播放页与桌面歌词同时生效"
        icon="clock"
        icon-color="var(--mint)"
        :badge="lyricOffsetText"
        badge-tone="mint"
      >
        <button type="button" class="bms-btn sm" @click="store.adjustLyricOffsetMs(-500)">-0.5s</button>
        <button type="button" class="bms-btn sm" @click="store.adjustLyricOffsetMs(-100)">-0.1s</button>
        <button
          type="button"
          class="bms-btn sm ghost"
          :disabled="store.lyricOffsetMs === 0"
          @click="store.lyricOffsetMs = 0"
        >
          归零
        </button>
        <button type="button" class="bms-btn sm" @click="store.adjustLyricOffsetMs(100)">+0.1s</button>
        <button type="button" class="bms-btn sm" @click="store.adjustLyricOffsetMs(500)">+0.5s</button>
      </SettingsRow>

      <div class="bms-mini-toggle-grid">
        <label class="bms-mini-toggle-card" for="bms-sw-st-krc">
          <span>KRC 逐字演绎</span>
          <SettingsSwitch id="bms-sw-st-krc" v-model="store.lyricWordByWord" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-st-bounce">
          <span>逐字微浮弹性</span>
          <SettingsSwitch
            id="bms-sw-st-bounce"
            v-model="store.lyricWordBounceEnabled"
            :disabled="!store.lyricWordByWord || store.lyricMotionStyle === 'apple_minimal'"
          />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-st-harmony">
          <span>和声/伴唱轻量化排版</span>
          <SettingsSwitch id="bms-sw-st-harmony" v-model="store.lyricHarmonyStyleEnabled" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-st-blur">
          <span>非焦点行景深模糊</span>
          <SettingsSwitch id="bms-sw-st-blur" v-model="store.lyricBlurEnabled" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-st-scale">
          <span>当前焦点行微幅放大</span>
          <SettingsSwitch id="bms-sw-st-scale" v-model="store.lyricFocusScaleEnabled" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-st-dots">
          <span>液态光珠间奏倒计时</span>
          <SettingsSwitch id="bms-sw-st-dots" v-model="store.lyricInterludeDotsEnabled" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-st-seek">
          <span>点击任意歌词跳转进度</span>
          <SettingsSwitch id="bms-sw-st-seek" v-model="store.lyricClickSeekEnabled" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-st-trans">
          <span>显示双语译文</span>
          <SettingsSwitch id="bms-sw-st-trans" v-model="store.showTranslation" />
        </label>
        <label class="bms-mini-toggle-card" for="bms-sw-st-phonetic">
          <span>显示罗马音 / 音译</span>
          <SettingsSwitch id="bms-sw-st-phonetic" v-model="store.showPhonetic" />
        </label>
      </div>
    </SettingsCard>
    </Transition>
  </div>
</template>

<style scoped>
.bms-head-btn-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.bms-scene-preset-grid {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 9px;
  width: 100%;
}

@media (max-width: 1120px) {
  .bms-scene-preset-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

@media (max-width: 640px) {
  .bms-scene-preset-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

.bms-scene-preset-tile {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  padding: 11px 12px;
  border-radius: 14px;
  background: var(--bms-surface-tile);
  color: var(--text-primary);
  text-align: left;
  cursor: pointer;
  transition:
    background 0.18s ease,
    transform 0.2s cubic-bezier(0.22, 1.35, 0.36, 1),
    box-shadow 0.2s ease;
}

.bms-scene-preset-tile:hover {
  background: var(--bms-surface-tile-hover);
  transform: translateY(-1px);
}

.bms-scene-preset-tile.active {
  background: color-mix(in oklab, var(--primary) 16%, var(--bms-surface-tile));
  box-shadow: 0 6px 18px color-mix(in oklab, var(--primary) 16%, transparent);
}

.bms-scene-dot {
  width: 18px;
  height: 6px;
  border-radius: 999px;
  margin-bottom: 2px;
}

.bms-scene-name {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--text-primary);
}

.bms-scene-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 500;
  color: var(--text-muted);
}

.bms-motion-engine-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  width: 100%;
}

@media (max-width: 720px) {
  .bms-motion-engine-grid {
    grid-template-columns: 1fr;
  }
}

.bms-motion-engine-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  padding: 11px 14px;
  border-radius: 14px;
  background: var(--bms-surface-tile);
  text-align: left;
  cursor: pointer;
  transition:
    background 0.18s ease,
    transform 0.2s cubic-bezier(0.22, 1.35, 0.36, 1),
    box-shadow 0.2s ease;
}

.bms-motion-engine-card:hover {
  background: var(--bms-surface-tile-hover);
  transform: translateY(-1px);
}

.bms-motion-engine-card.active {
  background: color-mix(in oklab, var(--primary) 16%, var(--bms-surface-tile));
  box-shadow: 0 6px 18px color-mix(in oklab, var(--primary) 14%, transparent);
}

.bms-me-top {
  display: flex;
  align-items: center;
  gap: 7px;
}

.bms-me-title {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: var(--text-primary);
}

.bms-me-badge {
  font-size: 10.5px;
  font-weight: 800;
  padding: 1px 7px;
  border-radius: 999px;
  background: color-mix(in oklab, var(--primary) 20%, transparent);
  color: var(--primary-text, var(--primary));
}

.bms-me-desc {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-muted);
  line-height: 1.4;
}

.bms-dual-sliders {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(230px, 1fr));
  gap: 12px 18px;
  width: 100%;
}

.bms-slider-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.bms-slider-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--text-secondary);
}

.bms-inline-groups {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.bms-group-hint {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 700;
  color: var(--text-muted);
  padding: 0 4px 0 8px;
}

.bms-mini-toggle-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

@media (max-width: 1120px) {
  .bms-mini-toggle-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 640px) {
  .bms-mini-toggle-grid {
    grid-template-columns: 1fr;
  }
}

.bms-mini-toggle-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 11px 14px;
  border-radius: 13px;
  background: var(--bms-surface-tile);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-primary);
  cursor: pointer;
  transition: background 0.18s ease;
}

.bms-mini-toggle-card:hover {
  background: var(--bms-surface-tile-hover);
}

.bms-dl-swatch-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 7px;
  width: 100%;
}

.bms-dl-swatch-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 29px;
  padding: 0 11px;
  border-radius: 999px;
  background: var(--bms-surface-tile);
  color: var(--text-secondary);
  font-size: var(--bm-fs-micro, 11.5px);
  font-weight: 700;
  cursor: pointer;
  white-space: nowrap;
  transition:
    background 0.16s ease,
    color 0.16s ease,
    box-shadow 0.16s ease;
}

.bms-dl-swatch-chip.compact {
  height: 26px;
  padding: 0 9px;
}

.bms-dl-swatch-chip:hover {
  background: var(--bms-surface-tile-hover);
  color: var(--text-primary);
}

.bms-dl-swatch-chip.active {
  background: color-mix(in oklab, var(--primary) 18%, var(--bms-surface-tile));
  color: var(--primary-text, var(--primary));
  box-shadow: 0 4px 12px color-mix(in oklab, var(--primary) 14%, transparent);
}

.bms-dl-swatch-dot {
  width: 11px;
  height: 11px;
  border-radius: 50%;
  flex-shrink: 0;
}

.bms-dl-color-input {
  width: 14px;
  height: 14px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  cursor: pointer;
}

.bms-dl-color-input::-webkit-color-swatch-wrapper {
  padding: 0;
}

.bms-dl-color-input::-webkit-color-swatch {
  border: none;
  border-radius: 50%;
}
</style>

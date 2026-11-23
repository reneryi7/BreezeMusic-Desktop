<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useAppStore } from "../../stores/appStore";
import { runThemeCircularTransition } from "../../utils/microMotionFx";
import type {
  CoverColorStageMode,
  DarkThemeVariant,
  LightThemeVariant,
  MotionLevel,
  PlayerBackgroundStyle,
  PlayerStageStyle,
  UiDensityMode,
} from "../../types";
import PlumpIcon from "../PlumpIcon.vue";
import SettingsCard from "./SettingsCard.vue";
import SettingsRow from "./SettingsRow.vue";

const props = defineProps<{
  highlightedId?: string;
}>();

const store = useAppStore();
const showPaletteDetails = ref(false);

watch(
  () => props.highlightedId,
  (id) => {
    if (id === "appearance-palette-preview") {
      showPaletteDetails.value = true;
    }
  }
);

interface ThemePresetCard {
  key: string;
  mode: "dark" | "light";
  variant?: LightThemeVariant;
  darkVariant?: DarkThemeVariant;
  badge: string;
  title: string;
  description: string;
}

const themePresetCards: ThemePresetCard[] = [
  {
    key: "d1",
    mode: "dark",
    darkVariant: "obsidian_aurora",
    badge: "D1 · 极夜冷曜",
    title: "黑曜石与暗夜极光",
    description: "冷曜石夜色画布与冰川极光散景，层次清晰、专注通透。",
  },
  {
    key: "d2",
    mode: "dark",
    darkVariant: "velvet_nebula",
    badge: "D2 · 丝绒微醺",
    title: "丝绒暗紫与星云流光",
    description: "苦巧丝绒实体块面与香槟暖字，温润醇厚、久看不累。",
  },
  {
    key: "l1",
    mode: "light",
    variant: "watercolor",
    badge: "L1 · 云宣水彩",
    title: "棉纸水彩浸润",
    description: "云宣瓷白画卷融合樱粉天青湿水彩，轻盈通透。",
  },
  {
    key: "l2",
    mode: "light",
    variant: "nordic_oat",
    badge: "L2 · 燕麦暖纸",
    title: "晨雾燕麦与矿物纸感",
    description: "三阶手抄暖纸与矿物青玉点缀，零彩光柔和护眼。",
  },
];

const motionOptions: ReadonlyArray<{
  id: MotionLevel;
  label: string;
  desc: string;
}> = [
  {
    id: "best",
    label: "最佳效果",
    desc: "完整弹性微交互、FLIP 封面飞行与 60fps 光学舞台",
  },
  {
    id: "standard",
    label: "普通效果",
    desc: "轻盈简洁过渡与节流渲染，兼顾流畅观感与节能",
  },
  {
    id: "none",
    label: "无效果",
    desc: "零过渡即时切换、静态舞台底色，极速响应低占用",
  },
];

const densityOptions: ReadonlyArray<{
  id: UiDensityMode;
  label: string;
  desc: string;
}> = [
  { id: "compact", label: "紧凑", desc: "同屏展示更多曲目与设置项" },
  { id: "standard", label: "标准", desc: "默认平衡留白与阅读效率" },
  { id: "spacious", label: "舒展", desc: "更宽裕的行高与视觉呼吸感" },
];

const accentTokens = computed(() => {
  const isDark = store.isDarkTheme;
  const isVelvet = isDark && store.darkThemeVariant === "velvet_nebula";
  const isOat = !isDark && store.lightThemeVariant === "nordic_oat";
  return [
    {
      role: isVelvet
        ? "主调 · 落日玫瑰"
        : isDark
        ? "主调 · 极光蓝靛"
        : isOat
        ? "主调 · 北欧青玉"
        : "主调 · 晴空鸢尾",
      token: "--primary",
      hex: isVelvet ? "#F43F5E" : isDark ? "#5E6AD2" : isOat ? "#0F766E" : "#4F46E5",
      usage: "导航 · 主按钮 · 进度条",
    },
    {
      role: isVelvet
        ? "点缀 · 暮光暖珊"
        : isOat
        ? "点缀 · 赤陶胭脂"
        : !isDark
        ? "点缀 · 樱花水彩"
        : "点缀 · 霓虹珊瑚",
      token: "--coral",
      hex: isVelvet ? "#FB923C" : isDark ? "#FB7185" : isOat ? "#BE123C" : "#EC4899",
      usage: "喜欢红心 · 今日标 · 热歌榜",
    },
    {
      role: isVelvet
        ? "点缀 · 胆机琥珀"
        : isDark
        ? "点缀 · 极昼冷金"
        : isOat
        ? "点缀 · 赭石琥珀"
        : "点缀 · 暖阳流金",
      token: "--gold",
      hex: isVelvet ? "#F59E0B" : isDark ? "#FACC15" : isOat ? "#B45309" : "#D97706",
      usage: "Hi-Res · VIP 尊享",
    },
    {
      role: isVelvet
        ? "点缀 · 苦艾青玉"
        : isOat
        ? "点缀 · 松林青玉"
        : !isDark
        ? "点缀 · 晨露薄荷"
        : "点缀 · 极光青玉",
      token: "--mint",
      hex: isVelvet ? "#34D399" : isDark ? "#2DD4BF" : isOat ? "#0F766E" : "#0D9488",
      usage: "SQ 无损 · 本地曲库",
    },
    {
      role: isVelvet
        ? "点缀 · 星云冰青"
        : isOat
        ? "点缀 · 石灰钴蓝"
        : !isDark
        ? "点缀 · 晴空天青"
        : "点缀 · 冰川天青",
      token: "--cyan",
      hex: isVelvet ? "#67E8F9" : isDark ? "#38BDF8" : isOat ? "#0369A1" : "#0284C7",
      usage: "新歌速递 · 音波",
    },
    {
      role: isVelvet
        ? "点缀 · 微醺兰紫"
        : isDark
        ? "点缀 · 星际靛紫"
        : isOat
        ? "点缀 · 矿物深靛"
        : "点缀 · 紫藤水彩",
      token: "--violet",
      hex: isVelvet ? "#E879F9" : isDark ? "#818CF8" : isOat ? "#4338CA" : "#7C3AED",
      usage: "专注舱 · 每日推荐",
    },
  ];
});

const coverColorModeOptions: ReadonlyArray<{
  id: CoverColorStageMode;
  label: string;
  desc: string;
}> = [
  { id: "follow", label: "封面 · 随明暗", desc: "深色进暗室，浅色进入 OKLab 宣纸漫染舞台" },
  { id: "dark_lock", label: "封面 · 锁暗室", desc: "无视全局浅色，播放页恒定保持深色剧场" },
  { id: "off", label: "主题配色", desc: "播放页与全部浮窗统一跟随当前全局主题" },
];

const bgStyleOptions: ReadonlyArray<{
  id: PlayerBackgroundStyle;
  title: string;
  tag: string;
  desc: string;
}> = [
  { id: "fluid_aurora", title: "液态丝绸", tag: "Fluid Silk", desc: "多峰 OKLab 流体丝绒随旋律呼吸" },
  { id: "rhythm_fluted", title: "长虹声幕", tag: "Fluted Glass", desc: "竖向长虹玻璃折光与频谱流动" },
  { id: "rhythm_cymatic", title: "声致驻波", tag: "Cymatic Wave", desc: "对称声学驻波呼应鼓点瞬态能量" },
  { id: "starlight_nebula", title: "星尘梦境", tag: "Starlight", desc: "三层视差星尘微光与柔焦云雾" },
];

const stageStyleOptions: ReadonlyArray<{
  id: PlayerStageStyle;
  label: string;
  icon: string;
}> = [
  { id: "vinyl_turntable", label: "黑胶唱机转盘", icon: "disc" },
  { id: "gallery_poster", label: "画廊海报封面", icon: "poster" },
];

function isCardActive(card: ThemePresetCard): boolean {
  if (card.mode === "dark") {
    return (
      store.resolvedColorScheme === "abyssal_dark" &&
      store.darkThemeVariant === (card.darkVariant || "obsidian_aurora")
    );
  }
  return (
    store.resolvedColorScheme === "watercolor_light" &&
    store.lightThemeVariant === card.variant
  );
}

function applyPresetCard(card: ThemePresetCard, event?: MouseEvent) {
  runThemeCircularTransition(event, async () => {
    if (card.mode === "dark") {
      store.setDarkThemeVariant(card.darkVariant || "obsidian_aurora");
      store.notify(`已切换深色方案：「${card.title}」`);
    } else if (card.variant) {
      store.setLightThemeVariant(card.variant);
      store.notify(`已切换浅色方案：「${card.title}」`);
    }
    await nextTick();
  });
}

function setColorMode(
  mode: "abyssal_dark" | "watercolor_light" | "system",
  event?: MouseEvent
) {
  runThemeCircularTransition(event, async () => {
    store.colorScheme = mode;
    if (mode === "system") {
      store.notify("外观模式已设为「跟随系统」");
    } else if (mode === "abyssal_dark") {
      store.notify(`已切换为「${store.darkThemeVariantMeta.name}」`);
    } else {
      store.notify(`已切换为「${store.lightThemeVariantMeta.name}」`);
    }
    await nextTick();
  });
}

function handleMotionChange(level: MotionLevel) {
  store.setMotionLevel(level);
  const label = motionOptions.find((o) => o.id === level)?.label || level;
  store.notify(`界面动效已切换为「${label}」`);
}

function handleDensityChange(density: UiDensityMode) {
  store.setUiDensity(density);
  const label = densityOptions.find((o) => o.id === density)?.label || density;
  store.notify(`界面信息密度已切换为「${label}」`);
}
</script>

<template>
  <div class="bms-panel-stack">
    <!-- 1. 四套高定主题矩阵 -->
    <SettingsCard
      icon="palette-orb"
      icon-color="var(--primary)"
      title="主题氛围"
      subtitle="两套深色与两套浅色无框主题，支持跟随系统自动切换"
    >
      <template #actions>
        <div class="bms-pill-group" role="group" aria-label="明暗主题模式">
          <button
            type="button"
            class="bms-pill-opt"
            :class="{ active: store.colorScheme === 'abyssal_dark' }"
            @click="setColorMode('abyssal_dark', $event)"
          >
            <PlumpIcon name="moon" :size="13" />
            <span>深色</span>
          </button>
          <button
            type="button"
            class="bms-pill-opt"
            :class="{ active: store.colorScheme === 'watercolor_light' }"
            @click="setColorMode('watercolor_light', $event)"
          >
            <PlumpIcon name="sun" :size="13" />
            <span>浅色</span>
          </button>
          <button
            type="button"
            class="bms-pill-opt"
            :class="{ active: store.colorScheme === 'system' }"
            @click="setColorMode('system', $event)"
          >
            <PlumpIcon name="sparkles" :size="13" />
            <span>跟随系统</span>
          </button>
        </div>
      </template>

      <div
        class="ts-presets-grid"
        data-setting-id="appearance-theme-matrix"
        :class="{ 'is-flash-highlight': highlightedId === 'appearance-theme-matrix' }"
      >
        <button
          v-for="card in themePresetCards"
          :key="card.key"
          type="button"
          class="ts-preset-card"
          :class="[`preview-${card.key}`, { 'is-active': isCardActive(card) }]"
          :aria-pressed="isCardActive(card)"
          @click="applyPresetCard(card, $event)"
        >
          <div class="ts-mini-canvas" :class="`canvas-${card.key}`">
            <div class="ts-mini-island">
              <span class="ts-mini-tab-active"></span>
              <span class="ts-mini-tab-dot"></span>
              <span class="ts-mini-tab-dot"></span>
            </div>
            <div class="ts-mini-bento-row">
              <div class="ts-mini-bento-a"><span class="ts-mini-play-dot a"></span></div>
              <div class="ts-mini-bento-b"><span class="ts-mini-play-dot b"></span></div>
            </div>
            <div class="ts-mini-glass-card">
              <div class="ts-mini-badges">
                <span class="ts-demo-tag gold">Hi-Res</span>
                <span class="ts-demo-tag mint">SQ</span>
                <span class="ts-demo-tag rose">今日</span>
              </div>
              <div class="ts-mini-bars">
                <span class="ts-mini-bar w70"></span>
                <span class="ts-mini-bar w45"></span>
              </div>
            </div>
          </div>

          <div class="ts-card-meta">
            <div class="ts-card-badge-row">
              <span class="ts-preset-code">{{ card.badge }}</span>
              <span v-if="isCardActive(card)" class="ts-active-chip">
                <PlumpIcon name="check" :size="11" color="#ffffff" />
                <span>生效中</span>
              </span>
            </div>
            <h3 class="ts-card-title">{{ card.title }}</h3>
            <p class="ts-card-desc">{{ card.description }}</p>
          </div>
        </button>
      </div>

      <!-- 渐进式折叠色彩预览，默认清爽收起 -->
      <div
        class="ts-palette-disclosure-wrap"
        data-setting-id="appearance-palette-preview"
        :class="{
          'is-open': showPaletteDetails,
          'is-flash-highlight': highlightedId === 'appearance-palette-preview',
        }"
      >
        <button
          type="button"
          class="bms-disclosure-btn ts-palette-trigger"
          :class="{ 'is-open': showPaletteDetails }"
          :aria-expanded="showPaletteDetails"
          @click="showPaletteDetails = !showPaletteDetails"
        >
          <div class="ts-palette-summary-left">
            <span class="ts-palette-summary-title">当前主题色彩与徽章预览</span>
            <div
              class="ts-mini-dot-strip"
              :class="{ 'is-dimmed': showPaletteDetails }"
              aria-hidden="true"
            >
              <span
                v-for="sw in accentTokens"
                :key="sw.token"
                class="ts-mini-color-dot"
                :style="{ background: sw.hex }"
              ></span>
            </div>
          </div>
          <span class="ts-disclosure-toggle" :class="{ 'is-open': showPaletteDetails }">
            <span class="ts-disclosure-label">{{ showPaletteDetails ? "收起预览" : "展开预览" }}</span>
            <span class="ts-disclosure-chevron" aria-hidden="true">
              <PlumpIcon name="chevron-down" :size="14" />
            </span>
          </span>
        </button>

        <div
          class="ts-palette-collapse"
          :class="{ 'is-open': showPaletteDetails }"
          :aria-hidden="!showPaletteDetails"
          :inert="!showPaletteDetails"
        >
          <div class="ts-palette-collapse-inner">
            <div class="ts-palette-expanded">
              <div class="ts-swatches-grid">
                <div
                  v-for="(swatch, sIdx) in accentTokens"
                  :key="swatch.token"
                  class="ts-swatch-card"
                  :style="{ '--sw-idx': sIdx }"
                >
                  <div class="ts-swatch-orb" :style="{ background: swatch.hex }"></div>
                  <div class="ts-swatch-info">
                    <div class="ts-swatch-role">{{ swatch.role }}</div>
                    <div class="ts-swatch-usage">{{ swatch.usage }}</div>
                  </div>
                </div>
              </div>
              <div class="ts-jelly-showcase">
                <span class="ts-showcase-label">状态徽章预览：</span>
                <span class="mini-badge hires">Hi-Res</span>
                <span class="mini-badge sq">SQ</span>
                <span class="mini-badge hq">HQ</span>
                <span class="mini-badge vip">VIP</span>
                <span class="ns-song-date-pill is-today">今日</span>
                <span class="ns-song-date-pill">昨日</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </SettingsCard>

    <!-- 2. 界面动效等级 & 界面信息密度（用户核心关注项） -->
    <SettingsCard
      icon="sparkles"
      icon-color="var(--cyan)"
      title="动效与视觉舒适度"
      subtitle="按需平衡流畅动效与硬件功耗，以及列表和卡片的排布松紧"
    >
      <SettingsRow
        setting-id="appearance-motion-density"
        :highlighted="highlightedId === 'appearance-motion-density'"
        vertical
        title="界面动效效果"
        description="统一控制页面切换、封面飞入、弹簧微交互与沉浸播放页 WebGL 光学舞台的渲染帧率"
      >
        <div class="bms-tile-grid cols-3" role="radiogroup" aria-label="界面动效等级">
          <button
            v-for="opt in motionOptions"
            :key="opt.id"
            type="button"
            role="radio"
            class="bms-tile-btn"
            :class="{ 'is-active': store.motionLevel === opt.id }"
            :aria-checked="store.motionLevel === opt.id"
            @click="handleMotionChange(opt.id)"
          >
            <div class="bms-tile-top">
              <span class="bms-tile-name">{{ opt.label }}</span>
              <PlumpIcon
                v-if="store.motionLevel === opt.id"
                name="check"
                :size="14"
                color="var(--primary)"
              />
            </div>
            <span class="bms-tile-desc">{{ opt.desc }}</span>
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        vertical
        title="界面信息密度"
        description="仅作用于歌曲列表行、歌单/专辑卡片栅格、设置行与导航间距，保持顶栏、播控底栏及全屏播放舞台比例不变"
      >
        <div class="bms-tile-grid cols-3" role="radiogroup" aria-label="界面信息密度">
          <button
            v-for="opt in densityOptions"
            :key="opt.id"
            type="button"
            role="radio"
            class="bms-tile-btn"
            :class="{ 'is-active': store.uiDensity === opt.id }"
            :aria-checked="store.uiDensity === opt.id"
            @click="handleDensityChange(opt.id)"
          >
            <div class="bms-tile-top">
              <span class="bms-tile-name">{{ opt.label }}</span>
              <PlumpIcon
                v-if="store.uiDensity === opt.id"
                name="check"
                :size="14"
                color="var(--primary)"
              />
            </div>
            <span class="bms-tile-desc">{{ opt.desc }}</span>
          </button>
        </div>
      </SettingsRow>
    </SettingsCard>

    <!-- 3. 沉浸播放页默认视觉偏好 -->
    <SettingsCard
      title="沉浸播放页视觉偏好"
      subtitle="封面动态取色三态联动、四大光学背景与专辑主视觉形态"
      icon="disc"
      icon-color="var(--violet)"
    >
      <SettingsRow
        title="封面动态取色策略"
        description="决定全屏播放舞台及播放页浮窗如何与歌曲封面双主色联动"
        setting-id="appearance-cover-color-mode"
        :highlighted="highlightedId === 'appearance-cover-color-mode'"
        vertical
      >
        <div class="bms-tile-grid cols-3" role="radiogroup" aria-label="封面动态取色策略">
          <button
            v-for="opt in coverColorModeOptions"
            :key="opt.id"
            type="button"
            role="radio"
            class="bms-tile-btn"
            :class="{ 'is-active': store.coverColorStageMode === opt.id }"
            :aria-checked="store.coverColorStageMode === opt.id"
            @click="store.coverColorStageMode = opt.id"
          >
            <div class="bms-tile-top">
              <span class="bms-tile-name">{{ opt.label }}</span>
              <PlumpIcon
                v-if="store.coverColorStageMode === opt.id"
                name="check"
                :size="14"
                color="var(--primary)"
              />
            </div>
            <span class="bms-tile-desc">{{ opt.desc }}</span>
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="高定光学背景"
        description="由 WebGL2 着色器实时渲染，切歌期间物理轨道零中断"
        setting-id="appearance-player-bg"
        :highlighted="highlightedId === 'appearance-player-bg'"
        vertical
      >
        <div class="bms-tile-grid cols-4" role="radiogroup" aria-label="高定光学背景">
          <button
            v-for="bg in bgStyleOptions"
            :key="bg.id"
            type="button"
            role="radio"
            class="bms-tile-btn"
            :class="{ 'is-active': store.playerBgStyle === bg.id }"
            :aria-checked="store.playerBgStyle === bg.id"
            @click="store.playerBgStyle = bg.id"
          >
            <div class="bms-tile-top">
              <span class="bms-tile-name">{{ bg.title }}</span>
              <span class="bms-tile-en">{{ bg.tag }}</span>
            </div>
            <span class="bms-tile-desc">{{ bg.desc }}</span>
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="默认播放舞台形态"
        description="选择全屏播放页左侧专辑视觉呈现样式"
        setting-id="appearance-player-stage"
        :highlighted="highlightedId === 'appearance-player-stage'"
      >
        <div class="bms-pill-group" role="radiogroup" aria-label="默认播放舞台形态">
          <button
            v-for="st in stageStyleOptions"
            :key="st.id"
            type="button"
            role="radio"
            class="bms-pill-opt"
            :class="{ active: store.playerStageStyle === st.id }"
            :aria-checked="store.playerStageStyle === st.id"
            @click="store.playerStageStyle = st.id"
          >
            <PlumpIcon :name="st.icon" :size="14" />
            <span>{{ st.label }}</span>
          </button>
        </div>
      </SettingsRow>
    </SettingsCard>
  </div>
</template>

<style scoped>
.ts-presets-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 14px;
  border-radius: var(--bm-r-lg, 18px);
}

@container settings-stage (max-width: 960px) {
  .ts-presets-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@container settings-stage (max-width: 560px) {
  .ts-presets-grid {
    grid-template-columns: 1fr;
  }
}

.ts-presets-grid.is-flash-highlight {
  box-shadow: 0 0 30px -2px color-mix(in srgb, var(--primary) 48%, transparent);
}

.ts-preset-card {
  display: flex;
  flex-direction: column;
  text-align: left;
  padding: 13px;
  border: none;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bms-surface-row);
  cursor: pointer;
  transition:
    transform 0.24s var(--ease-fluid),
    box-shadow 0.24s ease,
    background 0.24s ease;
}

.ts-preset-card:hover {
  transform: translateY(-2px);
  background: var(--bms-surface-row-hover);
}

.ts-preset-card.is-active {
  background: color-mix(in srgb, var(--primary) 16%, var(--bms-surface-row));
  box-shadow: 0 14px 28px -12px color-mix(in srgb, var(--primary) 42%, transparent);
}

.ts-mini-canvas {
  width: 100%;
  height: 122px;
  border-radius: var(--bm-r-md, 14px);
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 7px;
  overflow: hidden;
  margin-bottom: 12px;
}

.canvas-d1 {
  background:
    radial-gradient(circle at 18% 18%, rgba(56, 189, 248, 0.28), transparent 55%),
    radial-gradient(circle at 86% 22%, rgba(45, 212, 191, 0.22), transparent 50%),
    radial-gradient(circle at 76% 88%, rgba(94, 106, 210, 0.24), transparent 54%),
    #0C0E14;
}

.canvas-d2 {
  background:
    radial-gradient(circle at 16% 18%, rgba(244, 63, 94, 0.30), transparent 54%),
    radial-gradient(circle at 86% 24%, rgba(245, 158, 11, 0.26), transparent 50%),
    radial-gradient(circle at 78% 86%, rgba(232, 121, 249, 0.18), transparent 52%),
    #1A1116;
}

.canvas-l1 {
  background:
    radial-gradient(circle at 16% 18%, rgba(56, 189, 248, 0.44), transparent 54%),
    radial-gradient(circle at 85% 22%, rgba(244, 114, 182, 0.42), transparent 52%),
    radial-gradient(circle at 22% 86%, rgba(167, 139, 250, 0.34), transparent 54%),
    #F6F8FC;
}

.canvas-l2 {
  background: #E8E0D0;
}

.ts-mini-island {
  align-self: center;
  height: 16px;
  padding: 2px 6px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  background: rgba(255, 255, 255, 0.78);
}

.canvas-l2 .ts-mini-island { background: #FAF6EE; }
.canvas-d1 .ts-mini-island { background: rgba(19, 24, 34, 0.88); }
.canvas-d2 .ts-mini-island { background: rgba(38, 23, 33, 0.90); }

.ts-mini-tab-active {
  width: 22px;
  height: 9px;
  border-radius: 999px;
  background: linear-gradient(135deg, #4f46e5, #0ea5e9, #ec4899);
}

.canvas-l2 .ts-mini-tab-active { background: linear-gradient(135deg, #115e59, #0f766e); }
.canvas-d1 .ts-mini-tab-active { background: linear-gradient(135deg, #5e6ad2, #38bdf8); }
.canvas-d2 .ts-mini-tab-active { background: linear-gradient(135deg, #e11d48, #f43f5e, #f59e0b); }

.ts-mini-tab-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: rgba(120, 113, 108, 0.35);
}

.ts-mini-bento-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
  height: 26px;
}

.ts-mini-bento-a,
.ts-mini-bento-b {
  border-radius: 8px;
  padding: 4px 6px;
  display: flex;
  align-items: center;
  justify-content: flex-end;
}

.canvas-d1 .ts-mini-bento-a { background: linear-gradient(135deg, rgba(18, 28, 64, 0.92), rgba(22, 68, 102, 0.86)); }
.canvas-d1 .ts-mini-bento-b { background: linear-gradient(135deg, rgba(12, 42, 46, 0.92), rgba(24, 92, 104, 0.84)); }
.canvas-d2 .ts-mini-bento-a { background: linear-gradient(135deg, rgba(64, 18, 36, 0.94), rgba(126, 52, 34, 0.86)); }
.canvas-d2 .ts-mini-bento-b { background: linear-gradient(135deg, rgba(54, 26, 18, 0.94), rgba(84, 42, 24, 0.86)); }
.canvas-l1 .ts-mini-bento-a { background: linear-gradient(135deg, rgba(224, 242, 254, 0.88), rgba(252, 231, 243, 0.86)); }
.canvas-l1 .ts-mini-bento-b { background: linear-gradient(135deg, rgba(204, 251, 241, 0.88), rgba(224, 242, 254, 0.84)); }
.canvas-l2 .ts-mini-bento-a { background: linear-gradient(135deg, #DCECE7, #EAE5D9); }
.canvas-l2 .ts-mini-bento-b { background: linear-gradient(135deg, #F5E6D6, #E6EFEA); }

.ts-mini-play-dot { width: 11px; height: 11px; border-radius: 50%; }
.ts-mini-play-dot.a { background: #4f46e5; }
.canvas-l2 .ts-mini-play-dot.a { background: #0f766e; }
.canvas-d1 .ts-mini-play-dot.a { background: #38bdf8; }
.canvas-d2 .ts-mini-play-dot.a { background: #f43f5e; }
.ts-mini-play-dot.b { background: #ec4899; }
.canvas-l2 .ts-mini-play-dot.b { background: #b45309; }
.canvas-d1 .ts-mini-play-dot.b { background: #2dd4bf; }
.canvas-d2 .ts-mini-play-dot.b { background: #f59e0b; }

.ts-mini-glass-card {
  flex: 1;
  border-radius: 10px;
  padding: 6px 8px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  background: rgba(255, 255, 255, 0.62);
}

.canvas-d1 .ts-mini-glass-card { background: rgba(20, 26, 38, 0.80); }
.canvas-d2 .ts-mini-glass-card { background: rgba(44, 27, 38, 0.86); }
.canvas-l2 .ts-mini-glass-card { background: #FAF6EE; }

.ts-mini-badges { display: flex; gap: 4px; }
.ts-demo-tag {
  font-size: 9.5px;
  font-weight: var(--bm-fw-bold, 700);
  padding: 1px 5px;
  border-radius: 5px;
}
.ts-demo-tag.gold { background: rgba(217, 119, 6, 0.18); color: #b45309; }
.ts-demo-tag.mint { background: rgba(13, 148, 136, 0.16); color: #0f766e; }
.ts-demo-tag.rose { background: rgba(225, 29, 72, 0.15); color: #be123c; }
.canvas-d1 .ts-demo-tag.gold, .canvas-d2 .ts-demo-tag.gold { color: #fbbf24; }
.canvas-d1 .ts-demo-tag.mint { color: #2dd4bf; }
.canvas-d2 .ts-demo-tag.mint { color: #34d399; }
.canvas-d1 .ts-demo-tag.rose, .canvas-d2 .ts-demo-tag.rose { color: #fb7185; }

.ts-mini-bars { display: flex; flex-direction: column; gap: 3px; }
.ts-mini-bar { height: 4px; border-radius: 999px; background: rgba(28, 25, 23, 0.16); }
.canvas-d1 .ts-mini-bar, .canvas-d2 .ts-mini-bar { background: rgba(245, 239, 247, 0.18); }
.ts-mini-bar.w70 { width: 72%; }
.ts-mini-bar.w45 { width: 46%; }

.ts-card-meta {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.ts-card-badge-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.ts-preset-code {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--primary-text, var(--primary));
}

.ts-active-chip {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--bms-focus-grad, var(--grad-kugou-primary));
  color: #ffffff;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-bold, 700);
}

.ts-card-title {
  margin: 0;
  font-size: var(--bm-fs-song-title, 15px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
}

.ts-card-desc {
  margin: 0;
  font-size: var(--bm-fs-caption, 12.5px);
  line-height: var(--bm-ui-line-height, 1.48);
  color: var(--text-secondary);
}

/* 可折叠色彩预览条（Grid 固有高度丝滑展开/收起 + 图标连续旋转） */
.ts-palette-disclosure-wrap {
  display: flex;
  flex-direction: column;
}

.ts-palette-trigger {
  transition:
    background-color 0.24s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1)),
    transform 0.2s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1));
}

.ts-palette-trigger:active {
  transform: scale(0.994);
}

.ts-palette-trigger.is-open {
  background: color-mix(in srgb, var(--primary) 10%, var(--bms-surface-row));
}

.ts-palette-summary-left {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.ts-palette-summary-title {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.ts-mini-dot-strip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  transition:
    opacity 0.24s ease,
    transform 0.28s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1));
}

.ts-mini-dot-strip.is-dimmed {
  opacity: 0.28;
  transform: translate3d(-4px, 0, 0) scale(0.92);
}

.ts-mini-color-dot {
  width: 13px;
  height: 13px;
  border-radius: 50%;
}

.ts-disclosure-toggle {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-secondary);
  transition: color 0.22s ease;
}

.ts-disclosure-toggle.is-open {
  color: var(--primary-text, var(--primary));
}

.ts-disclosure-label {
  min-width: 4em;
  text-align: right;
}

.ts-disclosure-chevron {
  width: 24px;
  height: 24px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--bms-neutral-control, color-mix(in srgb, var(--text-primary) 6%, transparent));
  transition:
    transform 0.34s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1)),
    background-color 0.24s ease,
    color 0.24s ease;
}

.ts-disclosure-toggle.is-open .ts-disclosure-chevron {
  transform: rotate(180deg);
  background: color-mix(in srgb, var(--primary) 20%, transparent);
  color: var(--primary-text, var(--primary));
}

/* CSS Grid 0fr -> 1fr 平滑折叠容器（无任何布局突跳或硬边框） */
.ts-palette-collapse {
  display: grid;
  grid-template-rows: 0fr;
  opacity: 0;
  transition:
    grid-template-rows 0.34s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1)),
    opacity 0.22s ease;
}

.ts-palette-collapse.is-open {
  grid-template-rows: 1fr;
  opacity: 1;
  transition:
    grid-template-rows 0.36s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1)),
    opacity 0.26s ease 0.04s;
}

.ts-palette-collapse-inner {
  min-height: 0;
  overflow: hidden;
}

.ts-palette-expanded {
  padding-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  transform: translate3d(0, -8px, 0) scale(0.99);
  transition: transform 0.34s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1));
}

.ts-palette-collapse.is-open .ts-palette-expanded {
  transform: translate3d(0, 0, 0) scale(1);
}

.ts-swatches-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

@container settings-stage (max-width: 720px) {
  .ts-swatches-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

.ts-swatch-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--bms-surface-row);
  opacity: 0;
  transform: translate3d(0, 6px, 0);
  transition:
    opacity 0.18s ease,
    transform 0.24s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1));
}

.ts-palette-collapse.is-open .ts-swatch-card {
  opacity: 1;
  transform: translate3d(0, 0, 0);
  transition-delay: calc(var(--sw-idx, 0) * 24ms + 40ms);
}

[data-motion-level="none"] .ts-palette-collapse,
[data-motion-level="none"] .ts-palette-expanded,
[data-motion-level="none"] .ts-swatch-card,
[data-motion-level="none"] .ts-disclosure-chevron,
[data-motion-level="none"] .ts-mini-dot-strip {
  transition: none !important;
}

@media (prefers-reduced-motion: reduce) {
  .ts-palette-collapse,
  .ts-palette-expanded,
  .ts-swatch-card,
  .ts-disclosure-chevron,
  .ts-mini-dot-strip {
    transition: none !important;
  }
}

.ts-swatch-orb {
  width: 30px;
  height: 30px;
  border-radius: 10px;
  flex-shrink: 0;
}

.ts-swatch-info {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.ts-swatch-role {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.ts-swatch-usage {
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-muted);
}

.ts-jelly-showcase {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  padding: 10px 14px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--bms-surface-row);
}

.ts-showcase-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-secondary);
}

.bms-tile-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.bms-tile-name {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.bms-tile-en {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-muted);
}

.bms-tile-desc {
  font-size: var(--bm-fs-caption, 12.5px);
  line-height: var(--bm-ui-line-height, 1.48);
  color: var(--text-secondary);
}

/* L2 / D2 特化 */
.scheme-watercolor_light.light-variant-nordic_oat .ts-preset-card,
.scheme-watercolor_light.light-variant-nordic_oat .ts-swatch-card,
.scheme-watercolor_light.light-variant-nordic_oat .ts-jelly-showcase,
.scheme-abyssal_dark.dark-variant-velvet_nebula .ts-preset-card,
.scheme-abyssal_dark.dark-variant-velvet_nebula .ts-swatch-card,
.scheme-abyssal_dark.dark-variant-velvet_nebula .ts-jelly-showcase {
  background: var(--bms-surface-row);
}

.scheme-watercolor_light.light-variant-nordic_oat .ts-preset-card.is-active {
  background: color-mix(in srgb, var(--primary) 13%, #FAF6EE);
}
</style>

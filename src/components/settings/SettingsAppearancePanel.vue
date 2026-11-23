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
    title: "黑曜石极光",
    description: "冷曜石夜幕与冰川极光折射，通透专注。",
  },
  {
    key: "d2",
    mode: "dark",
    darkVariant: "velvet_nebula",
    badge: "D2 · 丝绒微醺",
    title: "丝绒星云",
    description: "哑光苦巧实体块面与香槟暖字，温润耐看。",
  },
  {
    key: "l1",
    mode: "light",
    variant: "watercolor",
    badge: "L1 · 云宣水彩",
    title: "棉纸水彩",
    description: "云宣瓷白融合樱粉天青湿水彩，轻盈透亮。",
  },
  {
    key: "l2",
    mode: "light",
    variant: "nordic_oat",
    badge: "L2 · 燕麦暖纸",
    title: "晨雾燕麦",
    description: "三阶手抄暖纸配矿物青玉点缀，柔和护眼。",
  },
];

const motionOptions: ReadonlyArray<{
  id: MotionLevel;
  label: string;
  icon: string;
  desc: string;
}> = [
  {
    id: "best",
    label: "最佳效果",
    icon: "sparkles",
    desc: "完整弹性微交互、FLIP 封面飞行与 60fps 光学舞台",
  },
  {
    id: "standard",
    label: "普通效果",
    icon: "wave",
    desc: "轻盈简洁过渡与节流渲染，兼顾流畅观感与节能",
  },
  {
    id: "none",
    label: "无效果",
    icon: "sliders",
    desc: "零过渡即时切换与静态舞台底色，极速低占用",
  },
];

const densityOptions: ReadonlyArray<{
  id: UiDensityMode;
  label: string;
  desc: string;
}> = [
  { id: "compact", label: "紧凑", desc: "同屏呈现更多曲目与设置项" },
  { id: "standard", label: "标准", desc: "平衡留白呼吸感与阅读效率" },
  { id: "spacious", label: "舒展", desc: "更宽裕的行距与从容节奏" },
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
      icon-color="var(--primary-text, var(--primary))"
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
            <PlumpIcon name="moon" :size="13" color="currentColor" />
            <span>深色</span>
          </button>
          <button
            type="button"
            class="bms-pill-opt"
            :class="{ active: store.colorScheme === 'watercolor_light' }"
            @click="setColorMode('watercolor_light', $event)"
          >
            <PlumpIcon name="sun" :size="13" color="currentColor" />
            <span>浅色</span>
          </button>
          <button
            type="button"
            class="bms-pill-opt"
            :class="{ active: store.colorScheme === 'system' }"
            @click="setColorMode('system', $event)"
          >
            <PlumpIcon name="sparkles" :size="13" color="currentColor" />
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
            <span v-if="isCardActive(card)" class="ts-canvas-check" aria-hidden="true">
              <PlumpIcon name="check" :size="11" color="#ffffff" />
            </span>
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
            <span class="ts-preset-code">{{ card.badge }}</span>
            <h3 class="ts-card-title">{{ card.title }}</h3>
            <p class="ts-card-desc">{{ card.description }}</p>
          </div>
        </button>
      </div>

      <!-- 轻量内联主题色板折叠条（与上方主题卡由柔光渐隐线区隔，默认极简收起） -->
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
            <span class="ts-palette-summary-title">主题色板与音质徽章预览</span>
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
            <span class="ts-disclosure-label">{{ showPaletteDetails ? "收起" : "展开" }}</span>
            <span class="ts-disclosure-chevron" aria-hidden="true">
              <PlumpIcon name="chevron-down" :size="13" color="currentColor" />
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
                <span class="ts-showcase-label">状态徽章</span>
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

    <!-- 2. 界面动效等级 & 界面信息密度 -->
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
              <div class="bms-tile-head-left">
                <PlumpIcon
                  :name="opt.icon"
                  :size="14"
                  :color="store.motionLevel === opt.id ? 'var(--primary-text, var(--primary))' : 'var(--text-muted)'"
                />
                <span class="bms-tile-name">{{ opt.label }}</span>
              </div>
              <span v-if="store.motionLevel === opt.id" class="bms-tile-check" aria-hidden="true">
                <PlumpIcon name="check" :size="10" color="#ffffff" />
              </span>
            </div>
            <span class="bms-tile-desc">{{ opt.desc }}</span>
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        vertical
        title="界面信息密度"
        description="仅作用于歌曲列表行、歌单/专辑卡片栅格、设置行与导航间距，保持顶栏、播控底栏及播放舞台比例不变"
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
              <div class="bms-tile-head-left">
                <span
                  class="ts-density-glyph"
                  :class="[`is-${opt.id}`, { 'is-active': store.uiDensity === opt.id }]"
                  aria-hidden="true"
                >
                  <span></span>
                  <span></span>
                  <span></span>
                </span>
                <span class="bms-tile-name">{{ opt.label }}</span>
              </div>
              <span v-if="store.uiDensity === opt.id" class="bms-tile-check" aria-hidden="true">
                <PlumpIcon name="check" :size="10" color="#ffffff" />
              </span>
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
              <span v-if="store.coverColorStageMode === opt.id" class="bms-tile-check" aria-hidden="true">
                <PlumpIcon name="check" :size="10" color="#ffffff" />
              </span>
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
              <span v-if="store.playerBgStyle === bg.id" class="bms-tile-check" aria-hidden="true">
                <PlumpIcon name="check" :size="10" color="#ffffff" />
              </span>
              <span v-else class="bms-tile-en">{{ bg.tag }}</span>
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
            <PlumpIcon :name="st.icon" :size="14" color="currentColor" />
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
  gap: 11px;
  border-radius: 14px;
}

@container settings-stage (max-width: 820px) {
  .ts-presets-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@container settings-stage (max-width: 520px) {
  .ts-presets-grid {
    grid-template-columns: 1fr;
  }
}

.ts-presets-grid.is-flash-highlight {
  box-shadow: 0 0 30px -2px color-mix(in oklab, var(--primary) 48%, transparent);
}

.ts-preset-card {
  display: flex;
  flex-direction: column;
  text-align: left;
  padding: 10px;
  border: none;
  border-radius: 15px;
  background: var(--bms-surface-tile);
  cursor: pointer;
  transition:
    transform 0.22s var(--ease-fluid),
    box-shadow 0.22s ease,
    background 0.2s ease;
}

.ts-preset-card:hover {
  transform: translateY(-2px);
  background: var(--bms-surface-tile-hover);
}

.ts-preset-card:focus-visible {
  outline: 2px solid var(--primary-text, var(--primary));
  outline-offset: 2px;
}

.ts-preset-card.is-active {
  background: var(--bms-tile-active-bg);
  box-shadow: var(--bms-tile-active-shadow);
}

.ts-mini-canvas {
  position: relative;
  width: 100%;
  height: clamp(102px, 11cqi, 128px);
  border-radius: 11px;
  padding: 9px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow: hidden;
  margin-bottom: 10px;
}

.ts-canvas-check {
  position: absolute;
  top: 7px;
  right: 7px;
  z-index: 2;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--bms-focus-grad, var(--grad-kugou-primary));
  display: inline-flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 4px 10px -2px color-mix(in oklab, var(--primary) 62%, transparent);
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
  height: 14px;
  padding: 2px 6px;
  border-radius: 999px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: rgba(255, 255, 255, 0.78);
}

.canvas-l2 .ts-mini-island { background: #FAF6EE; }
.canvas-d1 .ts-mini-island { background: rgba(19, 24, 34, 0.88); }
.canvas-d2 .ts-mini-island { background: rgba(38, 23, 33, 0.90); }

.ts-mini-tab-active {
  width: 20px;
  height: 8px;
  border-radius: 999px;
  background: linear-gradient(135deg, #4f46e5, #0ea5e9, #ec4899);
}

.canvas-l2 .ts-mini-tab-active { background: linear-gradient(135deg, #115e59, #0f766e); }
.canvas-d1 .ts-mini-tab-active { background: linear-gradient(135deg, #5e6ad2, #38bdf8); }
.canvas-d2 .ts-mini-tab-active { background: linear-gradient(135deg, #e11d48, #f43f5e, #f59e0b); }

.ts-mini-tab-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: rgba(120, 113, 108, 0.35);
}

.ts-mini-bento-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 5px;
  height: 22px;
}

.ts-mini-bento-a,
.ts-mini-bento-b {
  border-radius: 7px;
  padding: 3px 5px;
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

.ts-mini-play-dot { width: 9px; height: 9px; border-radius: 50%; }
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
  border-radius: 8px;
  padding: 5px 7px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  background: rgba(255, 255, 255, 0.66);
}

.canvas-d1 .ts-mini-glass-card { background: rgba(20, 26, 38, 0.82); }
.canvas-d2 .ts-mini-glass-card { background: rgba(44, 27, 38, 0.88); }
.canvas-l2 .ts-mini-glass-card { background: #FAF6EE; }

.ts-mini-badges { display: flex; gap: 4px; }
.ts-demo-tag {
  font-size: 9px;
  font-weight: var(--bm-fw-bold, 700);
  padding: 1px 4px;
  border-radius: 4px;
}
.ts-demo-tag.gold { background: rgba(217, 119, 6, 0.18); color: #b45309; }
.ts-demo-tag.mint { background: rgba(13, 148, 136, 0.16); color: #0f766e; }
.ts-demo-tag.rose { background: rgba(225, 29, 72, 0.15); color: #be123c; }
.canvas-d1 .ts-demo-tag.gold, .canvas-d2 .ts-demo-tag.gold { color: #fbbf24; }
.canvas-d1 .ts-demo-tag.mint { color: #2dd4bf; }
.canvas-d2 .ts-demo-tag.mint { color: #34d399; }
.canvas-d1 .ts-demo-tag.rose, .canvas-d2 .ts-demo-tag.rose { color: #fb7185; }

.ts-mini-bars { display: flex; flex-direction: column; gap: 3px; }
.ts-mini-bar { height: 3.5px; border-radius: 999px; background: rgba(28, 25, 23, 0.16); }
.canvas-d1 .ts-mini-bar, .canvas-d2 .ts-mini-bar { background: rgba(245, 239, 247, 0.18); }
.ts-mini-bar.w70 { width: 70%; }
.ts-mini-bar.w45 { width: 44%; }

.ts-card-meta {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 0 2px 2px;
}

.ts-preset-code {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-muted);
  transition: color 0.18s ease;
}

.ts-preset-card.is-active .ts-preset-code {
  color: var(--primary-text, var(--primary));
}

.ts-card-title {
  margin: 0;
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
  line-height: 1.3;
}

.ts-card-desc {
  margin: 2px 0 0;
  font-size: 12px;
  line-height: 1.44;
  color: var(--text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

/* 轻量内联主题色板折叠区（与上方主题矩阵以柔光渐隐线区隔，绝无笨重灰盒套壳） */
.ts-palette-disclosure-wrap {
  position: relative;
  display: flex;
  flex-direction: column;
  padding-top: 8px;
}

.ts-palette-disclosure-wrap::before {
  content: "";
  position: absolute;
  top: 0;
  left: 4px;
  right: 4px;
  height: 1px;
  background: linear-gradient(
    90deg,
    transparent 0%,
    var(--bms-hairline-soft) 6%,
    var(--bms-hairline-soft) 94%,
    transparent 100%
  );
  pointer-events: none;
}

.ts-palette-trigger {
  padding: 8px 10px;
  border-radius: 12px;
}

.ts-palette-summary-left {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.ts-palette-summary-title {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-secondary);
}

.ts-palette-trigger:hover .ts-palette-summary-title,
.ts-palette-trigger.is-open .ts-palette-summary-title {
  color: var(--text-primary);
}

.ts-mini-dot-strip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  transition:
    opacity 0.22s ease,
    transform 0.26s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1));
}

.ts-mini-dot-strip.is-dimmed {
  opacity: 0.25;
  transform: translate3d(-4px, 0, 0) scale(0.92);
}

.ts-mini-color-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

.ts-disclosure-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: var(--bm-fs-caption, 12px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-muted);
  transition: color 0.18s ease;
}

.ts-palette-trigger:hover .ts-disclosure-toggle,
.ts-disclosure-toggle.is-open {
  color: var(--primary-text, var(--primary));
}

.ts-disclosure-chevron {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: transform 0.32s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1));
}

.ts-disclosure-toggle.is-open .ts-disclosure-chevron {
  transform: rotate(180deg);
}

.ts-palette-collapse {
  display: grid;
  grid-template-rows: 0fr;
  opacity: 0;
  transition:
    grid-template-rows 0.32s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1)),
    opacity 0.2s ease;
}

.ts-palette-collapse.is-open {
  grid-template-rows: 1fr;
  opacity: 1;
  transition:
    grid-template-rows 0.34s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1)),
    opacity 0.24s ease 0.04s;
}

.ts-palette-collapse-inner {
  min-height: 0;
  overflow: hidden;
}

.ts-palette-expanded {
  padding-top: 10px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  transform: translate3d(0, -6px, 0);
  transition: transform 0.32s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1));
}

.ts-palette-collapse.is-open .ts-palette-expanded {
  transform: translate3d(0, 0, 0);
}

.ts-swatches-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 9px;
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
  padding: 9px 11px;
  border-radius: 12px;
  background: var(--bms-surface-tile);
  opacity: 0;
  transform: translate3d(0, 5px, 0);
  transition:
    opacity 0.18s ease,
    transform 0.22s var(--ease-apple-fluid, cubic-bezier(0.22, 1, 0.36, 1));
}

.ts-palette-collapse.is-open .ts-swatch-card {
  opacity: 1;
  transform: translate3d(0, 0, 0);
  transition-delay: calc(var(--sw-idx, 0) * 20ms + 30ms);
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
  width: 26px;
  height: 26px;
  border-radius: 9px;
  flex-shrink: 0;
}

.ts-swatch-info {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.ts-swatch-role {
  font-size: 12px;
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
  padding: 9px 13px;
  border-radius: 12px;
  background: var(--bms-surface-tile);
}

.ts-showcase-label {
  font-size: 12px;
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-muted);
}

/* 选择砖内部排版 */
.bms-tile-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-height: 20px;
}

.bms-tile-head-left {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.bms-tile-name {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.bms-tile-en {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-medium, 500);
  color: var(--text-muted);
}

.bms-tile-desc {
  font-size: 12px;
  line-height: 1.48;
  color: var(--text-secondary);
}

/* 界面信息密度可视化三线示意徽标（纯 CSS，一眼识别紧凑 / 标准 / 舒展） */
.ts-density-glyph {
  width: 15px;
  height: 14px;
  display: inline-flex;
  flex-direction: column;
  justify-content: center;
  flex-shrink: 0;
}

.ts-density-glyph span {
  display: block;
  width: 100%;
  height: 2px;
  border-radius: 999px;
  background: var(--text-muted);
  transition: background 0.18s ease;
}

.ts-density-glyph span:last-child {
  width: 72%;
}

.ts-density-glyph.is-active span {
  background: var(--primary-text, var(--primary));
}

.ts-density-glyph.is-compact { gap: 2px; }
.ts-density-glyph.is-standard { gap: 3.5px; }
.ts-density-glyph.is-spacious { gap: 5px; }
</style>

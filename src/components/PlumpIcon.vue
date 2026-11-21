<script setup lang="ts">
import { computed } from "vue";

/**
 * PlumpIcon.vue — 微风音乐专属「软润饱满 (Plump & Organic)」双色矢量图标引擎
 * 统一对齐播放页 (NowPlayingView.vue) `np-*` 图标美学规范：
 * 1. 24×24 光学视觉网格，预留舒适呼吸边距，几何形态视觉重心居中校准；
 * 2. 统一圆润倒角 (rx / stroke-linecap="round" / stroke-linejoin="round") 与友好粗细笔画 (2.1–2.7)；
 * 3. 双层景深质感：柔和半透明内衬填充 (fill-opacity="0.14–0.22") + 利落外轮廓描边 + 圆润胶囊点缀 (0.68–1)；
 * 4. 当未显式传入 `accent` 时自动回落至 `color`，确保浅色/深色/纯白/反色按钮内和谐统一、不刺眼。
 */
const props = withDefaults(
  defineProps<{
    name: string;
    size?: number;
    color?: string;
    accent?: string;
    filled?: boolean;
  }>(),
  {
    size: 20,
    color: "currentColor",
    accent: undefined,
    filled: true,
  }
);

const resolvedAccent = computed(() => props.accent ?? props.color);
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    class="plump-svg-icon"
    aria-hidden="true"
  >
    <!-- ==================== 1. 播放控制核心 (`np-*` 及全局共用) ==================== -->
    <!-- 播放 (视觉重心向右微移校准的饱满流线大圆角贝塞尔三角) -->
    <template v-if="name === 'play' || name === 'np-play'">
      <path
        d="M8.1 4.2C6.1 3.1 4.0 4.5 4.0 6.7V17.3C4.0 19.5 6.1 20.9 8.1 19.8L19.1 14.0C21.0 13.0 21.0 11.0 19.1 10.0L8.1 4.2Z"
        :fill="color"
      />
    </template>

    <!-- 暂停 (疏密均衡的双圆润超椭圆胶囊柱) -->
    <template v-else-if="name === 'pause' || name === 'np-pause'">
      <rect x="5.2" y="4.2" width="5.0" height="15.6" rx="2.5" :fill="color" />
      <rect x="13.8" y="4.2" width="5.0" height="15.6" rx="2.5" :fill="color" />
    </template>

    <!-- 上一首 (双色质感：圆润胶囊竖柱 + 饱满大圆角快切三角) -->
    <template v-else-if="name === 'prev' || name === 'np-prev'">
      <rect
        x="3.6"
        y="4.8"
        width="3.9"
        height="14.4"
        rx="1.95"
        :fill="resolvedAccent"
        fill-opacity="0.9"
      />
      <path
        d="M17.5 4.9C19.2 3.8 21.2 5.0 21.2 7.0V17.0C21.2 19.0 19.2 20.2 17.5 19.1L9.6 14.1C8.1 13.2 8.1 10.8 9.6 9.9L17.5 4.9Z"
        :fill="color"
      />
    </template>

    <!-- 下一首 (双色质感：饱满大圆角快切三角 + 圆润胶囊竖柱) -->
    <template v-else-if="name === 'next' || name === 'np-next'">
      <path
        d="M6.5 4.9C4.8 3.8 2.8 5.0 2.8 7.0V17.0C2.8 19.0 4.8 20.2 6.5 19.1L14.4 14.1C15.9 13.2 15.9 10.8 14.4 9.9L6.5 4.9Z"
        :fill="color"
      />
      <rect
        x="16.5"
        y="4.8"
        width="3.9"
        height="14.4"
        rx="1.95"
        :fill="resolvedAccent"
        fill-opacity="0.9"
      />
    </template>

    <!-- 软润饱满爱心 (三次贝塞尔丰盈曲线) -->
    <template
      v-else-if="
        name === 'heart' ||
        name === 'heart-filled' ||
        name === 'np-heart' ||
        name === 'np-heart-filled'
      "
    >
      <path
        v-if="name === 'heart-filled' || name === 'np-heart-filled'"
        d="M12 20.7C11.3 20.7 10.7 20.4 10.1 19.9C5.1 16.2 2.1 12.6 2.1 8.3C2.1 4.9 4.7 2.5 8.0 2.5C9.8 2.5 11.2 3.3 12 4.6C12.8 3.3 14.2 2.5 16.0 2.5C19.3 2.5 21.9 4.9 21.9 8.3C21.9 12.6 18.9 16.2 13.9 19.9C13.3 20.4 12.7 20.7 12 20.7Z"
        :fill="color"
      />
      <path
        v-else
        d="M12 19.8C6.4 15.8 3.1 12.3 3.1 8.4C3.1 5.6 5.2 3.6 7.9 3.6C9.6 3.6 11.1 4.4 12 5.8C12.9 4.4 14.4 3.6 16.1 3.6C18.8 3.6 20.9 5.6 20.9 8.4C20.9 12.3 17.6 15.8 12 19.8Z"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- 列表循环 / 单曲循环 / 随机播放 / 顺序播放 -->
    <template
      v-else-if="
        name === 'repeat-all' ||
        name === 'repeat_all' ||
        name === 'repeat' ||
        name === 'np-repeat'
      "
    >
      <path
        d="M4.8 9.2C4.8 6.9 6.7 5.0 9.0 5.0H17.8M17.8 5.0L15.2 2.7M17.8 5.0L15.2 7.3M19.2 14.8C19.2 17.1 17.3 19.0 15.0 19.0H6.2M6.2 19.0L8.8 16.7M6.2 19.0L8.8 21.3"
        :stroke="color"
        stroke-width="2.6"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template
      v-else-if="
        name === 'repeat-one' ||
        name === 'repeat_one' ||
        name === 'np-repeat-one'
      "
    >
      <path
        d="M4.8 9.2C4.8 6.9 6.7 5.0 9.0 5.0H17.8M17.8 5.0L15.4 2.8M17.8 5.0L15.4 7.2M19.2 14.8C19.2 17.1 17.3 19.0 15.0 19.0H6.2M6.2 19.0L8.6 16.8M6.2 19.0L8.6 21.2"
        :stroke="color"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M12.3 9.2V14.8M10.6 10.6L12.3 9.2"
        :stroke="color"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'shuffle' || name === 'np-shuffle'">
      <path
        d="M3.8 7.6C9.5 7.6 11.5 16.4 19.2 16.4M19.2 16.4L16.5 14.0M19.2 16.4L16.5 18.8"
        :stroke="color"
        stroke-width="2.6"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M3.8 16.4C9.5 16.4 11.5 7.6 19.2 7.6M19.2 7.6L16.5 5.2M19.2 7.6L16.5 10.0"
        :stroke="color"
        stroke-opacity="0.72"
        stroke-width="2.6"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'sequence' || name === 'np-sequence'">
      <rect x="3.5" y="6.5" width="11.5" height="3.2" rx="1.6" :fill="color" />
      <rect x="3.5" y="14.3" width="11.5" height="3.2" rx="1.6" :fill="color" fill-opacity="0.68" />
      <path
        d="M16.5 8.5L20 12L16.5 15.5"
        :stroke="color"
        stroke-width="2.8"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- ==================== 2. 播放页专属功能图标 (`np-*` 基准美学) ==================== -->
    <!-- 母带声波 / 律动频谱 -->
    <template v-else-if="name === 'np-wave'">
      <rect x="2.8" y="8.5" width="3.0" height="7" rx="1.5" :fill="color" fill-opacity="0.72" />
      <rect x="6.7" y="5.5" width="3.0" height="13" rx="1.5" :fill="color" fill-opacity="0.9" />
      <rect x="10.5" y="3.0" width="3.0" height="18" rx="1.5" :fill="color" />
      <rect x="14.3" y="6.0" width="3.0" height="12" rx="1.5" :fill="color" fill-opacity="0.9" />
      <rect x="18.2" y="8.8" width="3.0" height="6.4" rx="1.5" :fill="color" fill-opacity="0.72" />
    </template>
    <template v-else-if="name === 'wave' || name === 'waveform'">
      <rect x="2.8" y="8.5" width="3.0" height="7" rx="1.5" :fill="color" fill-opacity="0.7" />
      <rect x="6.7" y="5.5" width="3.0" height="13" rx="1.5" :fill="color" fill-opacity="0.88" />
      <rect x="10.5" y="3.0" width="3.0" height="18" rx="1.5" :fill="resolvedAccent" />
      <rect x="14.3" y="6.0" width="3.0" height="12" rx="1.5" :fill="color" fill-opacity="0.88" />
      <rect x="18.2" y="8.8" width="3.0" height="6.4" rx="1.5" :fill="color" fill-opacity="0.7" />
    </template>

    <!-- 桌面歌词牌 -->
    <template v-else-if="name === 'np-lyric'">
      <rect
        x="2.6"
        y="4.2"
        width="18.8"
        height="15.6"
        rx="5.6"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.3"
      />
      <rect x="6.4" y="8.5" width="11.2" height="2.6" rx="1.3" :fill="color" />
      <rect x="6.4" y="13.0" width="7.4" height="2.6" rx="1.3" :fill="color" fill-opacity="0.68" />
    </template>
    <template v-else-if="name === 'lyric'">
      <rect
        x="2.6"
        y="4.2"
        width="18.8"
        height="15.6"
        rx="5.6"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.3"
      />
      <rect x="6.4" y="8.5" width="11.2" height="2.6" rx="1.3" :fill="color" />
      <rect x="6.4" y="13.0" width="7.4" height="2.6" rx="1.3" :fill="resolvedAccent" fill-opacity="0.76" />
    </template>

    <!-- 歌词海报星芒 -->
    <template v-else-if="name === 'np-poster'">
      <rect
        x="3.2"
        y="3.2"
        width="17.6"
        height="17.6"
        rx="5.8"
        :fill="color"
        fill-opacity="0.15"
        :stroke="color"
        stroke-width="2.3"
      />
      <path
        d="M15.8 6.0L16.5 7.9L18.4 8.6L16.5 9.3L15.8 11.2L15.1 9.3L13.2 8.6L15.1 7.9L15.8 6.0Z"
        :fill="color"
      />
      <rect x="6.6" y="12.2" width="9.2" height="2.5" rx="1.25" :fill="color" />
      <rect x="6.6" y="15.8" width="6.2" height="2.4" rx="1.2" :fill="color" fill-opacity="0.68" />
    </template>
    <template v-else-if="name === 'poster'">
      <rect
        x="3.2"
        y="3.2"
        width="17.6"
        height="17.6"
        rx="5.8"
        :fill="color"
        fill-opacity="0.15"
        :stroke="color"
        stroke-width="2.3"
      />
      <path
        d="M15.8 6.0L16.5 7.9L18.4 8.6L16.5 9.3L15.8 11.2L15.1 9.3L13.2 8.6L15.1 7.9L15.8 6.0Z"
        :fill="resolvedAccent"
      />
      <rect x="6.6" y="12.2" width="9.2" height="2.5" rx="1.25" :fill="color" />
      <rect x="6.6" y="15.8" width="6.2" height="2.4" rx="1.2" :fill="color" fill-opacity="0.68" />
    </template>

    <!-- 添加到歌单 -->
    <template v-else-if="name === 'np-playlist-add' || name === 'np-add-playlist'">
      <rect x="3.2" y="5.0" width="15.0" height="2.8" rx="1.4" :fill="color" />
      <rect x="3.2" y="10.4" width="10.6" height="2.8" rx="1.4" :fill="color" fill-opacity="0.85" />
      <rect x="3.2" y="15.8" width="8.0" height="2.8" rx="1.4" :fill="color" fill-opacity="0.68" />
      <path
        d="M17.4 11.8V19.0M13.8 15.4H21.0"
        :stroke="color"
        stroke-width="2.8"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'playlist-add'">
      <rect x="3.2" y="5.0" width="15.0" height="2.8" rx="1.4" :fill="color" />
      <rect x="3.2" y="10.4" width="10.6" height="2.8" rx="1.4" :fill="color" fill-opacity="0.85" />
      <rect x="3.2" y="15.8" width="8.0" height="2.8" rx="1.4" :fill="color" fill-opacity="0.68" />
      <path
        d="M17.4 11.8V19.0M13.8 15.4H21.0"
        :stroke="resolvedAccent"
        stroke-width="2.7"
        stroke-linecap="round"
      />
    </template>

    <!-- 播放队列 (升级对齐 np-playlist-add / np-wave 圆润双层胶囊 + 右下角饱满在播指示三角) -->
    <template v-else-if="name === 'queue'">
      <rect x="3.2" y="5.0" width="17.6" height="2.8" rx="1.4" :fill="color" />
      <rect x="3.2" y="10.5" width="9.8" height="2.8" rx="1.4" :fill="color" fill-opacity="0.85" />
      <rect x="3.2" y="16.0" width="8.6" height="2.8" rx="1.4" :fill="color" fill-opacity="0.68" />
      <path
        d="M16.5 11.3C15.7 10.85 14.8 11.4 14.8 12.3V17.7C14.8 18.6 15.7 19.15 16.5 18.7L20.8 16.0C21.6 15.5 21.6 14.5 20.8 14.0L16.5 11.3Z"
        :fill="resolvedAccent"
      />
    </template>

    <!-- 评论气泡 -->
    <template v-else-if="name === 'np-comment'">
      <path
        d="M12 3.4C6.9 3.4 2.8 6.8 2.8 11.2C2.8 13.6 4.0 15.7 5.8 17.1L4.8 19.8C4.6 20.5 5.3 21.0 5.9 20.7L9.2 18.9C10.1 19.1 11.0 19.2 12 19.2C17.1 19.2 21.2 15.8 21.2 11.2C21.2 6.8 17.1 3.4 12 3.4Z"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
      <circle cx="8.5" cy="11.3" r="1.5" :fill="color" />
      <circle cx="12.0" cy="11.3" r="1.5" :fill="color" />
      <circle cx="15.5" cy="11.3" r="1.5" :fill="color" />
    </template>
    <template v-else-if="name === 'comment'">
      <path
        d="M12 3.4C6.9 3.4 2.8 6.8 2.8 11.2C2.8 13.6 4.0 15.7 5.8 17.1L4.8 19.8C4.6 20.5 5.3 21.0 5.9 20.7L9.2 18.9C10.1 19.1 11.0 19.2 12 19.2C17.1 19.2 21.2 15.8 21.2 11.2C21.2 6.8 17.1 3.4 12 3.4Z"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
      <circle cx="8.5" cy="11.3" r="1.45" :fill="color" />
      <circle cx="12.0" cy="11.3" r="1.45" :fill="resolvedAccent" />
      <circle cx="15.5" cy="11.3" r="1.45" :fill="color" />
    </template>

    <!-- 更多三圆珠 -->
    <template v-else-if="name === 'np-more'">
      <circle cx="5.5" cy="12" r="2.2" :fill="color" />
      <circle cx="12" cy="12" r="2.2" :fill="color" />
      <circle cx="18.5" cy="12" r="2.2" :fill="color" />
    </template>
    <template v-else-if="name === 'more'">
      <circle cx="5.6" cy="12" r="2.15" :fill="color" />
      <circle cx="12" cy="12" r="2.15" :fill="resolvedAccent" />
      <circle cx="18.4" cy="12" r="2.15" :fill="color" />
    </template>

    <!-- 歌词双语翻译 / 音译音符 / 歌词微调 / 播放页音量 / 下拉收起 -->
    <template v-else-if="name === 'translate-bubble' || name === 'np-translate'">
      <rect x="2.5" y="3.2" width="12.4" height="10.6" rx="4.2" :fill="color" fill-opacity="0.22" :stroke="color" stroke-width="2.35" />
      <path d="M6.4 8.2H11.0M8.7 6.2V8.2M7.2 8.2C7.6 9.8 8.8 11.0 10.4 11.4" :stroke="color" stroke-width="2.1" stroke-linecap="round" />
      <rect x="9.2" y="10.2" width="12.2" height="10.4" rx="4.2" :fill="color" fill-opacity="0.2" :stroke="color" stroke-width="2.35" />
      <path d="M13.2 17.2L15.3 12.6L17.4 17.2M13.8 16.0H16.8" :stroke="color" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" />
    </template>

    <template v-else-if="name === 'phonetic-note' || name === 'np-phonetic'">
      <circle cx="8.0" cy="16.4" r="3.4" :fill="color" />
      <path d="M11.2 16.4V5.2L18.8 7.0V11.2" :stroke="color" stroke-width="2.65" stroke-linecap="round" stroke-linejoin="round" />
      <path d="M15.0 14.6C16.6 13.4 18.6 13.4 20.2 14.6M14.0 17.8C16.2 16.2 19.0 16.2 21.2 17.8" :stroke="color" stroke-width="2.4" stroke-linecap="round" />
    </template>

    <template v-else-if="name === 'lyric-sliders' || name === 'lyric-badge' || name === 'np-lyric-sliders'">
      <rect x="2.8" y="4.0" width="18.4" height="16.0" rx="5.4" :fill="color" fill-opacity="0.2" :stroke="color" stroke-width="2.4" />
      <path d="M7.2 9.4H16.8" :stroke="color" stroke-width="2.65" stroke-linecap="round" />
      <path d="M7.2 14.6H12.6" :stroke="color" stroke-width="2.65" stroke-linecap="round" />
      <circle cx="16.0" cy="14.6" r="2.2" :fill="color" />
    </template>

    <template v-else-if="name === 'np-volume'">
      <path
        d="M4.2 9.6C4.2 8.72 4.92 8.0 5.8 8.0H8.1L12.1 4.65C13.15 3.78 14.75 4.52 14.75 5.88V18.12C14.75 19.48 13.15 20.22 12.1 19.35L8.1 16.0H5.8C4.92 16.0 4.2 15.28 4.2 14.4V9.6Z"
        :fill="color"
      />
      <path
        d="M17.5 9.2C18.5 10.7 18.5 13.3 17.5 14.8M20.2 6.8C22.0 9.8 22.0 14.2 20.2 17.2"
        :stroke="color"
        stroke-width="2.55"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'volume'">
      <path
        d="M4.2 9.6C4.2 8.72 4.92 8.0 5.8 8.0H8.1L12.1 4.65C13.15 3.78 14.75 4.52 14.75 5.88V18.12C14.75 19.48 13.15 20.22 12.1 19.35L8.1 16.0H5.8C4.92 16.0 4.2 15.28 4.2 14.4V9.6Z"
        :fill="color"
      />
      <path
        d="M17.5 9.2C18.5 10.7 18.5 13.3 17.5 14.8M20.2 6.8C22.0 9.8 22.0 14.2 20.2 17.2"
        :stroke="resolvedAccent"
        stroke-width="2.55"
        stroke-linecap="round"
      />
    </template>

    <template v-else-if="name === 'np-volume-mute'">
      <path
        d="M4.2 9.6C4.2 8.72 4.92 8.0 5.8 8.0H8.1L12.1 4.65C13.15 3.78 14.75 4.52 14.75 5.88V18.12C14.75 19.48 13.15 20.22 12.1 19.35L8.1 16.0H5.8C4.92 16.0 4.2 15.28 4.2 14.4V9.6Z"
        :fill="color"
        fill-opacity="0.72"
      />
      <path
        d="M17.4 9.6L21.2 14.4M21.2 9.6L17.4 14.4"
        :stroke="color"
        stroke-width="2.65"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'volume-mute'">
      <path
        d="M4.2 9.6C4.2 8.72 4.92 8.0 5.8 8.0H8.1L12.1 4.65C13.15 3.78 14.75 4.52 14.75 5.88V18.12C14.75 19.48 13.15 20.22 12.1 19.35L8.1 16.0H5.8C4.92 16.0 4.2 15.28 4.2 14.4V9.6Z"
        :fill="color"
        fill-opacity="0.72"
      />
      <path
        d="M17.4 9.6L21.2 14.4M21.2 9.6L17.4 14.4"
        :stroke="resolvedAccent"
        stroke-width="2.65"
        stroke-linecap="round"
      />
    </template>

    <template v-else-if="name === 'np-chevron-down'">
      <path
        d="M5.6 9.2L12 15.4L18.4 9.2"
        :stroke="color"
        stroke-width="2.65"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- ==================== 3. 顶部导航与主功能区 (`nav-*` / 通用双层美学) ==================== -->
    <!-- 发现 / 指南针 -->
    <template v-else-if="name === 'nav-discover' || name === 'compass'">
      <circle
        cx="12"
        cy="12"
        r="9.2"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.35"
      />
      <path
        d="M15.6 8.4L13.7 13.7L8.4 15.6L10.3 10.3L15.6 8.4Z"
        :fill="resolvedAccent"
        fill-opacity="0.26"
        :stroke="resolvedAccent"
        stroke-width="2.1"
        stroke-linejoin="round"
      />
      <circle cx="12" cy="12" r="1.45" :fill="resolvedAccent" />
    </template>

    <!-- 顶部主导航搜索 (带圆润镜片高光弧) -->
    <template v-else-if="name === 'nav-search'">
      <circle
        cx="10.6"
        cy="10.6"
        r="6.7"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.45"
      />
      <path
        d="M8.1 8.6C8.8 7.7 9.9 7.2 11.1 7.3"
        :stroke="resolvedAccent"
        stroke-width="2.1"
        stroke-linecap="round"
      />
      <path
        d="M15.6 15.6L20.0 20.0"
        :stroke="resolvedAccent"
        stroke-width="3.0"
        stroke-linecap="round"
      />
    </template>

    <!-- 通用搜索放大镜 (13–17px 搜索框保持利落干净) -->
    <template v-else-if="name === 'search'">
      <circle
        cx="10.6"
        cy="10.6"
        r="6.6"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.45"
      />
      <path
        d="M15.5 15.5L19.9 19.9"
        :stroke="resolvedAccent"
        stroke-width="2.85"
        stroke-linecap="round"
      />
    </template>

    <!-- 顶部主导航“我的” (双层软润人像剪影 + 饱满红心点缀) -->
    <template v-else-if="name === 'nav-library'">
      <circle
        cx="12"
        cy="6.9"
        r="3.6"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.2"
      />
      <path
        d="M4.8 18.6C4.8 15.3 7.9 13.2 12 13.2C16.1 13.2 19.2 15.3 19.2 18.6C19.2 19.9 18.1 20.8 16.6 20.8H7.4C5.9 20.8 4.8 19.9 4.8 18.6Z"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.2"
        stroke-linejoin="round"
      />
      <path
        d="M12 19.0C10.6 18.0 9.8 17.1 9.8 16.2C9.8 15.5 10.4 15.0 11.0 15.0C11.45 15.0 11.8 15.25 12 15.55C12.2 15.25 12.55 15.0 13.0 15.0C13.6 15.0 14.2 15.5 14.2 16.2C14.2 17.1 13.4 18.0 12 19.0Z"
        :fill="resolvedAccent"
      />
    </template>

    <!-- 我的音乐库 / 歌单架 (三枚等视觉宽度圆润软润书脊胶囊) -->
    <template v-else-if="name === 'library'">
      <rect
        x="3.6"
        y="4.8"
        width="3.6"
        height="14.4"
        rx="1.8"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.05"
      />
      <rect
        x="9.5"
        y="4.2"
        width="3.6"
        height="15.0"
        rx="1.8"
        :fill="color"
        fill-opacity="0.34"
        :stroke="color"
        stroke-width="2.05"
      />
      <rect
        x="15.6"
        y="5.2"
        width="3.6"
        height="13.8"
        rx="1.8"
        transform="rotate(-11 17.4 12.1)"
        :fill="resolvedAccent"
        fill-opacity="0.24"
        :stroke="resolvedAccent"
        stroke-width="2.05"
      />
    </template>

    <!-- 顶部调音台导航 / 声学滑块 (零贯穿断轨 + 双层软润圆形推子钮) -->
    <template v-else-if="name === 'nav-tuner' || name === 'tuning'">
      <rect x="3.2" y="6.3" width="6.2" height="2.6" rx="1.3" :fill="color" />
      <circle
        cx="13.4"
        cy="7.6"
        r="2.9"
        :fill="resolvedAccent"
        fill-opacity="0.22"
        :stroke="resolvedAccent"
        stroke-width="2.2"
      />
      <rect x="17.4" y="6.3" width="3.4" height="2.6" rx="1.3" :fill="color" fill-opacity="0.34" />

      <rect x="3.2" y="15.1" width="3.4" height="2.6" rx="1.3" :fill="color" fill-opacity="0.34" />
      <circle
        cx="10.6"
        cy="16.4"
        r="2.9"
        :fill="color"
        fill-opacity="0.22"
        :stroke="color"
        stroke-width="2.2"
      />
      <rect x="14.6" y="15.1" width="6.2" height="2.6" rx="1.3" :fill="resolvedAccent" />
    </template>

    <!-- 参量均衡推子 / Equalizer (三路垂直胶囊推子) -->
    <template v-else-if="name === 'sliders' || name === 'equalizer'">
      <path d="M5.4 4.2V5.6M5.4 13.4V19.8" :stroke="color" stroke-opacity="0.65" stroke-width="2.25" stroke-linecap="round" />
      <rect x="3.4" y="6.8" width="4.0" height="5.0" rx="2.0" :fill="color" fill-opacity="0.22" :stroke="color" stroke-width="2.1" />

      <path d="M12.0 4.2V10.4M12.0 18.2V19.8" :stroke="resolvedAccent" stroke-opacity="0.72" stroke-width="2.25" stroke-linecap="round" />
      <rect x="10.0" y="11.6" width="4.0" height="5.0" rx="2.0" :fill="resolvedAccent" fill-opacity="0.24" :stroke="resolvedAccent" stroke-width="2.1" />

      <path d="M18.6 4.2V6.8M18.6 14.6V19.8" :stroke="color" stroke-opacity="0.65" stroke-width="2.25" stroke-linecap="round" />
      <rect x="16.6" y="8.0" width="4.0" height="5.0" rx="2.0" :fill="color" fill-opacity="0.22" :stroke="color" stroke-width="2.1" />
    </template>

    <!-- 设置 (统一大尺寸圆润六瓣梅花齿轮 + 柔光中心核) -->
    <template v-else-if="name === 'nav-settings' || name === 'settings'">
      <path
        d="M12 2.9C13.3 2.9 14.3 3.8 14.6 5.0C15.1 5.2 15.6 5.5 16.1 5.9C17.2 5.4 18.6 5.8 19.3 6.9C19.9 8.0 19.6 9.4 18.6 10.1C18.8 10.7 18.9 11.4 18.9 12.0C18.9 12.6 18.8 13.3 18.6 13.9C19.6 14.6 19.9 16.0 19.3 17.1C18.6 18.2 17.2 18.6 16.1 18.1C15.6 18.5 15.1 18.8 14.6 19.0C14.3 20.2 13.3 21.1 12 21.1C10.7 21.1 9.7 20.2 9.4 19.0C8.9 18.8 8.4 18.5 7.9 18.1C6.8 18.6 5.4 18.2 4.7 17.1C4.1 16.0 4.4 14.6 5.4 13.9C5.2 13.3 5.1 12.6 5.1 12.0C5.1 11.4 5.2 10.7 5.4 10.1C4.4 9.4 4.1 8.0 4.7 6.9C5.4 5.8 6.8 5.4 7.9 5.9C8.4 5.5 8.9 5.2 9.4 5.0C9.7 3.8 10.7 2.9 12 2.9Z"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
      <circle cx="12" cy="12" r="3.1" :fill="resolvedAccent" />
    </template>

    <!-- ==================== 4. 窗口三键 & 方向箭头 & 常规微交互 ==================== -->
    <template v-else-if="name === 'minimize'">
      <path d="M5.2 12H18.8" :stroke="color" stroke-width="2.55" stroke-linecap="round" />
    </template>
    <template v-else-if="name === 'maximize'">
      <rect
        x="4.8"
        y="4.8"
        width="14.4"
        height="14.4"
        rx="4.2"
        :fill="color"
        fill-opacity="0.14"
        :stroke="color"
        stroke-width="2.35"
      />
    </template>
    <template v-else-if="name === 'restore'">
      <path
        d="M8.5 6.8V5.6C8.5 4.3 9.5 3.4 10.8 3.4H18.2C19.5 3.4 20.5 4.3 20.5 5.6V13.0C20.5 14.3 19.5 15.2 18.2 15.2H17.0"
        :stroke="color"
        stroke-opacity="0.72"
        stroke-width="2.25"
        stroke-linecap="round"
      />
      <rect
        x="3.6"
        y="7.6"
        width="12.6"
        height="12.6"
        rx="3.8"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.3"
      />
    </template>
    <template v-else-if="name === 'close'">
      <path
        d="M6.5 6.5L17.5 17.5M17.5 6.5L6.5 17.5"
        :stroke="color"
        stroke-width="2.55"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'plus'">
      <path
        d="M12 5.2V18.8M5.2 12H18.8"
        :stroke="color"
        stroke-width="2.65"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'minus'">
      <path
        d="M5.2 12H18.8"
        :stroke="color"
        stroke-width="2.65"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'check'">
      <circle cx="12" cy="12" r="9.4" :fill="resolvedAccent" fill-opacity="0.16" />
      <path
        d="M7.2 12.4L10.5 15.7L17.0 8.8"
        :stroke="resolvedAccent"
        stroke-width="2.8"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <template v-else-if="name === 'back' || name === 'arrow-left'">
      <path
        d="M18.8 12H5.4M5.4 12L10.8 6.6M5.4 12L10.8 17.4"
        :stroke="color"
        stroke-width="2.6"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'arrow-right'">
      <path
        d="M5.2 12H18.6M18.6 12L13.2 6.6M18.6 12L13.2 17.4"
        :stroke="color"
        stroke-width="2.6"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'chevron-right'">
      <path
        d="M9.2 5.8L15.4 12L9.2 18.2"
        :stroke="color"
        stroke-width="2.65"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'chevron-left'">
      <path
        d="M14.8 5.8L8.6 12L14.8 18.2"
        :stroke="color"
        stroke-width="2.65"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'chevron-down'">
      <path
        d="M5.8 9.2L12 15.4L18.2 9.2"
        :stroke="color"
        stroke-width="2.65"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'chevron-up'">
      <path
        d="M5.8 14.8L12 8.6L18.2 14.8"
        :stroke="color"
        stroke-width="2.65"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'top' || name === 'pin-top'">
      <path
        d="M5.6 4.8H18.4"
        :stroke="resolvedAccent"
        stroke-width="2.6"
        stroke-linecap="round"
      />
      <path
        d="M12 19.4V9.0M7.4 13.4L12 8.8L16.6 13.4"
        :stroke="color"
        stroke-width="2.6"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'expand'">
      <path
        d="M14.6 4.6H19.4V9.4M19.0 5.0L13.8 10.2M9.4 19.4H4.6V14.6M5.0 19.0L10.2 13.8"
        :stroke="color"
        stroke-width="2.45"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'collapse-all'">
      <path
        d="M7.0 9.8L12 5.0L17.0 9.8M7.0 18.6L12 13.8L17.0 18.6"
        :stroke="color"
        stroke-width="2.55"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'expand-all'">
      <path
        d="M7.0 5.4L12 10.2L17.0 5.4M7.0 14.2L12 19.0L17.0 14.2"
        :stroke="resolvedAccent"
        stroke-width="2.55"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- ==================== 5. 刷新 / 定位 / 时钟 / 历史 ==================== -->
    <template v-else-if="name === 'refresh'">
      <circle cx="12" cy="12" r="7.6" :fill="color" fill-opacity="0.12" />
      <path
        d="M19.5 13.2A7.6 7.6 0 1 1 18.58 8.2"
        :stroke="color"
        stroke-width="2.45"
        stroke-linecap="round"
      />
      <path
        d="M18.6 4.1V8.2H14.5"
        :stroke="color"
        stroke-width="2.45"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'reset-rotate'">
      <circle cx="12" cy="12" r="7.6" :fill="color" fill-opacity="0.12" />
      <path
        d="M4.5 13.2A7.6 7.6 0 1 0 5.42 8.2"
        :stroke="color"
        stroke-width="2.45"
        stroke-linecap="round"
      />
      <path
        d="M5.4 4.1V8.2H9.5"
        :stroke="color"
        stroke-width="2.45"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- 定位当前在播 (兼容 locate / locate-target) -->
    <template v-else-if="name === 'locate' || name === 'locate-target'">
      <circle
        cx="12"
        cy="12"
        r="7.1"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.3"
      />
      <circle cx="12" cy="12" r="2.9" :fill="resolvedAccent" />
      <path
        d="M12 2.4V4.8M12 19.2V21.6M2.4 12H4.8M19.2 12H21.6"
        :stroke="color"
        stroke-width="2.35"
        stroke-linecap="round"
      />
    </template>

    <!-- 时钟 / 定时 -->
    <template v-else-if="name === 'timer' || name === 'clock'">
      <circle
        cx="12"
        cy="12"
        r="8.8"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.35"
      />
      <path
        d="M12 7.4V12.2L15.2 14.1"
        :stroke="resolvedAccent"
        stroke-width="2.45"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- 最近播放历史栈 (软润表盘 + 回溯逆时针箭头) -->
    <template v-else-if="name === 'history'">
      <circle cx="12" cy="12" r="8.6" :fill="color" fill-opacity="0.14" />
      <path
        d="M4.6 8.3C6.0 5.3 9.0 3.3 12.4 3.3C17.1 3.3 20.8 7.2 20.8 12.0C20.8 16.8 16.9 20.7 12.0 20.7C7.6 20.7 4.0 17.5 3.4 13.3"
        :stroke="color"
        stroke-width="2.3"
        stroke-linecap="round"
      />
      <path
        d="M3.8 4.8V8.5H7.5"
        :stroke="color"
        stroke-width="2.3"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M12 7.8V12.2L15.0 14.0"
        :stroke="resolvedAccent"
        stroke-width="2.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- ==================== 6. 媒体 / 榜单 / 云盘 / 文件夹 / 用户 / 视觉元素 ==================== -->
    <!-- 电台 / 调频台 -->
    <template v-else-if="name === 'radio'">
      <rect
        x="3.0"
        y="7.8"
        width="18.0"
        height="12.6"
        rx="4.6"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.3"
      />
      <path
        d="M7.2 7.8L16.2 3.6"
        :stroke="color"
        stroke-width="2.4"
        stroke-linecap="round"
      />
      <circle cx="8.6" cy="14.1" r="2.7" :fill="resolvedAccent" />
      <rect x="13.6" y="11.5" width="4.4" height="2.3" rx="1.15" :fill="color" />
      <rect x="13.6" y="15.0" width="3.0" height="2.3" rx="1.15" :fill="color" fill-opacity="0.68" />
    </template>

    <!-- 奖杯榜单 -->
    <template v-else-if="name === 'trophy'">
      <path
        d="M6.8 4.4C6.8 3.9 7.2 3.6 7.7 3.6H16.3C16.8 3.6 17.2 3.9 17.2 4.4V9.8C17.2 12.8 14.9 15.2 12.0 15.2C9.1 15.2 6.8 12.8 6.8 9.8V4.4Z"
        :fill="color"
        fill-opacity="0.2"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
      <path
        d="M6.8 6.0H4.5C3.7 6.0 3.1 6.7 3.2 7.5C3.5 9.8 4.9 11.4 6.8 11.8M17.2 6.0H19.5C20.3 6.0 20.9 6.7 20.8 7.5C20.5 9.8 19.1 11.4 17.2 11.8"
        :stroke="color"
        stroke-width="2.3"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path d="M12 15.4V18.4" :stroke="color" stroke-width="2.4" stroke-linecap="round" />
      <rect x="7.8" y="18.2" width="8.4" height="2.8" rx="1.4" :fill="resolvedAccent" />
    </template>

    <!-- 榜单柱状图 -->
    <template v-else-if="name === 'rank' || name === 'chart' || name === 'bar-chart'">
      <rect x="2.4" y="2.8" width="19.2" height="18.4" rx="5.2" :fill="color" fill-opacity="0.14" />
      <rect x="4.6" y="11.4" width="3.8" height="7.4" rx="1.9" :fill="color" fill-opacity="0.72" />
      <rect x="10.1" y="5.2" width="3.8" height="13.6" rx="1.9" :fill="resolvedAccent" />
      <rect x="15.6" y="8.4" width="3.8" height="10.4" rx="1.9" :fill="color" fill-opacity="0.88" />
    </template>

    <!-- 黑胶唱片 (移除硬编码暗色，改为双环通透黑胶美学) -->
    <template v-else-if="name === 'disc' || name === 'vinyl'">
      <circle
        cx="12"
        cy="12"
        r="9.2"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.3"
      />
      <path
        d="M7.1 9.0C7.9 7.6 9.3 6.7 10.9 6.5"
        :stroke="color"
        stroke-opacity="0.52"
        stroke-width="2.0"
        stroke-linecap="round"
      />
      <circle
        cx="12"
        cy="12"
        r="3.4"
        :fill="resolvedAccent"
        fill-opacity="0.28"
        :stroke="resolvedAccent"
        stroke-width="2.15"
      />
      <circle cx="12" cy="12" r="1.15" :fill="resolvedAccent" />
    </template>

    <!-- 音符 -->
    <template v-else-if="name === 'music' || name === 'music-note'">
      <path
        d="M9.6 16.8V6.4C9.6 5.6 10.2 4.9 11.0 4.7L18.4 3.4C19.3 3.2 20.0 3.9 20.0 4.8V14.8"
        :stroke="color"
        stroke-width="2.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M9.6 9.3L20.0 7.5"
        :stroke="color"
        stroke-opacity="0.68"
        stroke-width="2.1"
        stroke-linecap="round"
      />
      <circle
        cx="6.7"
        cy="17.1"
        r="2.9"
        :fill="resolvedAccent"
        fill-opacity="0.24"
        :stroke="resolvedAccent"
        stroke-width="2.3"
      />
      <circle
        cx="17.1"
        cy="15.1"
        r="2.9"
        :fill="color"
        fill-opacity="0.24"
        :stroke="color"
        stroke-width="2.3"
      />
    </template>

    <!-- 耳机 / 冥想专注 -->
    <template v-else-if="name === 'headphones' || name === 'meditate'">
      <path
        d="M4.8 13.4V11.8C4.8 7.5 8.0 4.2 12 4.2C16.0 4.2 19.2 7.5 19.2 11.8V13.4"
        :stroke="color"
        stroke-width="2.45"
        stroke-linecap="round"
      />
      <rect
        x="3.2"
        y="12.0"
        width="4.6"
        height="7.6"
        rx="2.3"
        :fill="color"
        fill-opacity="0.22"
        :stroke="color"
        stroke-width="2.2"
      />
      <rect
        x="16.2"
        y="12.0"
        width="4.6"
        height="7.6"
        rx="2.3"
        :fill="resolvedAccent"
        fill-opacity="0.24"
        :stroke="resolvedAccent"
        stroke-width="2.2"
      />
    </template>

    <!-- 皇冠 VIP -->
    <template v-else-if="name === 'crown'">
      <path
        d="M4.0 16.2L2.8 8.4C2.6 7.1 4.1 6.3 5.1 7.1L8.4 9.8L10.9 5.0C11.4 4.0 12.6 4.0 13.1 5.0L15.6 9.8L18.9 7.1C19.9 6.3 21.4 7.1 21.2 8.4L20.0 16.2C19.8 17.6 18.6 18.6 17.2 18.6H6.8C5.4 18.6 4.2 17.6 4.0 16.2Z"
        :fill="color"
        fill-opacity="0.22"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
      <circle cx="12" cy="14.0" r="1.75" :fill="resolvedAccent" />
    </template>

    <!-- 火焰热度 -->
    <template v-else-if="name === 'flame' || name === 'fire'">
      <path
        d="M12.6 3.2C12.9 5.6 11.4 7.3 10.2 8.7C9.3 9.7 9.6 11.3 10.9 11.6C12.1 11.8 13.1 10.8 13.3 9.4C13.4 8.7 14.3 8.4 14.8 8.9C17.2 11.1 18.5 13.4 18.5 15.7C18.5 19.1 15.7 21.2 12.0 21.2C8.3 21.2 5.5 18.9 5.5 15.3C5.5 11.7 8.1 8.3 11.4 3.6C11.8 3.0 12.5 2.6 12.6 3.2Z"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.25"
        stroke-linejoin="round"
      />
      <path
        d="M12.0 13.6C13.4 14.7 14.2 16.0 14.2 17.2C14.2 18.5 13.2 19.5 12.0 19.5C10.8 19.5 9.8 18.5 9.8 17.2C9.8 16.0 10.6 14.7 12.0 13.6Z"
        :fill="resolvedAccent"
        fill-opacity="0.88"
      />
    </template>

    <!-- 星芒 / 副歌精华 -->
    <template v-else-if="name === 'sparkle' || name === 'climax'">
      <path
        d="M11.6 3.2C12.4 7.8 15.8 11.2 20.4 12.0C15.8 12.8 12.4 16.2 11.6 20.8C10.8 16.2 7.4 12.8 2.8 12.0C7.4 11.2 10.8 7.8 11.6 3.2Z"
        :fill="color"
        fill-opacity="0.22"
        :stroke="color"
        stroke-width="2.2"
        stroke-linejoin="round"
      />
      <circle cx="18.6" cy="5.4" r="1.75" :fill="resolvedAccent" />
    </template>
    <template v-else-if="name === 'sparkles'">
      <path
        d="M9.8 4.4C10.5 8.5 13.3 11.3 17.4 12.0C13.3 12.7 10.5 15.5 9.8 19.6C9.1 15.5 6.3 12.7 2.2 12.0C6.3 11.3 9.1 8.5 9.8 4.4Z"
        :fill="color"
        fill-opacity="0.22"
        :stroke="color"
        stroke-width="2.2"
        stroke-linejoin="round"
      />
      <path
        d="M17.8 3.2C18.2 5.2 19.4 6.4 21.4 6.8C19.4 7.2 18.2 8.4 17.8 10.4C17.4 8.4 16.2 7.2 14.2 6.8C16.2 6.4 17.4 5.2 17.8 3.2Z"
        :fill="resolvedAccent"
      />
      <circle cx="18.2" cy="17.2" r="1.5" :fill="resolvedAccent" fill-opacity="0.78" />
    </template>

    <!-- 收藏星标 -->
    <template v-else-if="name === 'star' || name === 'star-filled'">
      <path
        d="M12 3.3L14.6 8.6L20.4 9.4C21.1 9.5 21.4 10.4 20.9 10.9L16.7 15.0L17.7 20.7C17.8 21.4 17.0 21.9 16.4 21.6L12 19.3L7.6 21.6C7.0 21.9 6.2 21.4 6.3 20.7L7.3 15.0L3.1 10.9C2.6 10.4 2.9 9.5 3.6 9.4L9.4 8.6L12 3.3Z"
        :fill="color"
        :fill-opacity="name === 'star-filled' ? 1 : 0.18"
        :stroke="color"
        stroke-width="2.2"
        stroke-linejoin="round"
      />
    </template>

    <!-- 用户头像 / 多歌手集群 (为 NowPlayingQueuePopover 歌手聚簇内联 SVG 提供标准名称) -->
    <template v-else-if="name === 'user'">
      <circle
        cx="12"
        cy="7.8"
        r="3.8"
        :fill="color"
        fill-opacity="0.2"
        :stroke="color"
        stroke-width="2.3"
      />
      <path
        d="M4.8 19.2C5.2 15.6 8.2 13.6 12 13.6C15.8 13.6 18.8 15.6 19.2 19.2C19.3 20.1 18.5 20.8 17.6 20.8H6.4C5.5 20.8 4.7 20.1 4.8 19.2Z"
        :fill="resolvedAccent"
        fill-opacity="0.18"
        :stroke="resolvedAccent"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'artists' || name === 'users' || name === 'artist-cluster'">
      <circle
        cx="9.2"
        cy="7.8"
        r="3.4"
        :fill="color"
        fill-opacity="0.2"
        :stroke="color"
        stroke-width="2.2"
      />
      <path
        d="M3.2 18.8C3.6 15.5 6.0 13.6 9.2 13.6C12.4 13.6 14.8 15.5 15.2 18.8C15.3 19.6 14.6 20.2 13.8 20.2H4.6C3.8 20.2 3.1 19.6 3.2 18.8Z"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.2"
        stroke-linejoin="round"
      />
      <circle
        cx="16.4"
        cy="8.9"
        r="2.5"
        :fill="resolvedAccent"
        fill-opacity="0.22"
        :stroke="resolvedAccent"
        stroke-width="2.0"
      />
      <path
        d="M16.4 13.9C18.7 14.2 20.4 15.7 20.8 18.4"
        :stroke="resolvedAccent"
        stroke-opacity="0.82"
        stroke-width="2.15"
        stroke-linecap="round"
      />
    </template>

    <!-- 音质宝石 / 钻石 (为 NowPlayingQueuePopover 音质判别内联 SVG 提供标准名称) -->
    <template v-else-if="name === 'quality-gem' || name === 'diamond' || name === 'gem' || name === 'quality'">
      <path
        d="M6.4 4.6H17.6L21.0 9.4L12.0 19.8L3.0 9.4L6.4 4.6Z"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.25"
        stroke-linejoin="round"
      />
      <path
        d="M3.8 9.4H20.2"
        :stroke="resolvedAccent"
        stroke-opacity="0.82"
        stroke-width="2.05"
        stroke-linecap="round"
      />
      <path
        d="M9.0 9.4L12.0 18.8L15.0 9.4"
        :stroke="resolvedAccent"
        stroke-opacity="0.6"
        stroke-width="1.9"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- 文件夹 / 云端 -->
    <template v-else-if="name === 'folder'">
      <path
        d="M3.2 7.2C3.2 5.6 4.5 4.4 6.1 4.4H9.2C10.0 4.4 10.7 4.8 11.2 5.4L12.2 6.8H17.9C19.5 6.8 20.8 8.1 20.8 9.7V16.8C20.8 18.4 19.5 19.6 17.9 19.6H6.1C4.5 19.6 3.2 18.4 3.2 16.8V7.2Z"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
      <path
        d="M7.2 11.2H16.8"
        :stroke="resolvedAccent"
        stroke-opacity="0.65"
        stroke-width="2.1"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'cloud'">
      <path
        d="M7.2 18.4H17.0C19.5 18.4 21.4 16.5 21.4 14.1C21.4 11.9 19.8 10.1 17.6 9.8C17.0 6.7 14.3 4.6 11.2 4.6C7.8 4.6 5.1 7.2 4.9 10.5C3.5 11.1 2.6 12.6 2.6 14.2C2.6 16.6 4.6 18.4 7.2 18.4Z"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
    </template>

    <!-- 删除垃圾桶 -->
    <template v-else-if="name === 'trash'">
      <path
        d="M6.4 7.4L7.3 18.0C7.4 19.4 8.5 20.4 9.9 20.4H14.1C15.5 20.4 16.6 19.4 16.7 18.0L17.6 7.4"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.3"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M4.5 7.4H19.5M9.6 4.3H14.4"
        :stroke="color"
        stroke-width="2.35"
        stroke-linecap="round"
      />
      <path
        d="M10.2 11.2V16.4M13.8 11.2V16.4"
        :stroke="color"
        stroke-opacity="0.65"
        stroke-width="2.05"
        stroke-linecap="round"
      />
    </template>

    <!-- 日间 / 夜间主题 -->
    <template v-else-if="name === 'sun'">
      <circle
        cx="12"
        cy="12"
        r="4.5"
        :fill="resolvedAccent"
        fill-opacity="0.22"
        :stroke="resolvedAccent"
        stroke-width="2.3"
      />
      <path
        d="M12 3.1V4.9M12 19.1V20.9M3.1 12H4.9M19.1 12H20.9M5.7 5.7L7.0 7.0M17.0 17.0L18.3 18.3M18.3 5.7L17.0 7.0M7.0 17.0L5.7 18.3"
        :stroke="color"
        stroke-width="2.35"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'moon'">
      <path
        d="M19.8 14.5C18.7 15.0 17.5 15.2 16.2 15.2C11.4 15.2 7.6 11.4 7.6 6.6C7.6 5.4 7.8 4.2 8.3 3.2C4.9 4.5 2.6 7.8 2.6 11.6C2.6 16.6 6.7 20.7 11.7 20.7C15.5 20.7 18.7 18.2 19.8 14.5Z"
        :fill="color"
        fill-opacity="0.2"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
    </template>

    <!-- ==================== 7. 工具 / 文件 / 分享 / 视图切换 / 筛选排序 ==================== -->
    <template v-else-if="name === 'download'">
      <path
        d="M4.4 14.8V16.6C4.4 18.4 5.8 19.8 7.6 19.8H16.4C18.2 19.8 19.6 18.4 19.6 16.6V14.8"
        :fill="color"
        fill-opacity="0.14"
        :stroke="color"
        stroke-width="2.4"
        stroke-linecap="round"
      />
      <path
        d="M12 4.2V14.6M8.2 11.0L12 14.8L15.8 11.0"
        :stroke="resolvedAccent"
        stroke-width="2.55"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'upload'">
      <path
        d="M4.4 14.8V16.6C4.4 18.4 5.8 19.8 7.6 19.8H16.4C18.2 19.8 19.6 18.4 19.6 16.6V14.8"
        :fill="color"
        fill-opacity="0.14"
        :stroke="color"
        stroke-width="2.4"
        stroke-linecap="round"
      />
      <path
        d="M12 14.8V4.4M8.2 8.0L12 4.2L15.8 8.0"
        :stroke="resolvedAccent"
        stroke-width="2.55"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'import'">
      <path
        d="M4.2 13.8V16.8C4.2 18.5 5.6 19.8 7.3 19.8H16.7C18.4 19.8 19.8 18.5 19.8 16.8V13.8"
        :fill="color"
        fill-opacity="0.14"
        :stroke="color"
        stroke-width="2.4"
        stroke-linecap="round"
      />
      <path
        d="M12 4.2V14.4M8.3 10.8L12 14.6L15.7 10.8"
        :stroke="resolvedAccent"
        stroke-width="2.55"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'share'">
      <path
        d="M8.7 10.6L14.9 7.0M8.7 13.4L14.9 17.0"
        :stroke="color"
        stroke-width="2.3"
        stroke-linecap="round"
      />
      <circle
        cx="17.4"
        cy="5.6"
        r="2.9"
        :fill="resolvedAccent"
        fill-opacity="0.24"
        :stroke="resolvedAccent"
        stroke-width="2.2"
      />
      <circle
        cx="6.2"
        cy="12"
        r="2.9"
        :fill="color"
        fill-opacity="0.2"
        :stroke="color"
        stroke-width="2.2"
      />
      <circle
        cx="17.4"
        cy="18.4"
        r="2.9"
        :fill="color"
        fill-opacity="0.2"
        :stroke="color"
        stroke-width="2.2"
      />
    </template>
    <template v-else-if="name === 'link'">
      <path
        d="M10.0 13.8C11.2 15.0 13.2 15.0 14.4 13.8L17.6 10.6C18.8 9.4 18.8 7.4 17.6 6.2C16.4 5.0 14.4 5.0 13.2 6.2L11.8 7.6"
        :stroke="color"
        stroke-width="2.5"
        stroke-linecap="round"
      />
      <path
        d="M14.0 10.2C12.8 9.0 10.8 9.0 9.6 10.2L6.4 13.4C5.2 14.6 5.2 16.6 6.4 17.8C7.6 19.0 9.6 19.0 10.8 17.8L12.2 16.4"
        :stroke="resolvedAccent"
        stroke-opacity="0.84"
        stroke-width="2.5"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'copy'">
      <rect
        x="8.6"
        y="8.6"
        width="11.4"
        height="11.4"
        rx="3.8"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.25"
      />
      <path
        d="M5.2 15.0H5.0C3.9 15.0 3.0 14.1 3.0 13.0V5.8C3.0 4.7 3.9 3.8 5.0 3.8H12.2C13.3 3.8 14.2 4.7 14.2 5.8V5.6"
        :stroke="resolvedAccent"
        stroke-width="2.3"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'shield'">
      <path
        d="M12 2.8L4.6 6.0V11.6C4.6 16.4 7.8 20.6 12 22.0C16.2 20.6 19.4 16.4 19.4 11.6V6.0L12 2.8Z"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
      <path
        d="M9.0 12.2L11.2 14.4L15.4 9.8"
        :stroke="resolvedAccent"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'lock'">
      <rect
        x="4.6"
        y="10.2"
        width="14.8"
        height="10.2"
        rx="4.2"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.3"
      />
      <path
        d="M8.0 10.2V7.2C8.0 5.0 9.8 3.2 12 3.2C14.2 3.2 16.0 5.0 16.0 7.2V10.2"
        :stroke="resolvedAccent"
        stroke-width="2.4"
        stroke-linecap="round"
      />
      <circle cx="12" cy="15.3" r="1.6" :fill="resolvedAccent" />
    </template>
    <template v-else-if="name === 'unlock'">
      <rect
        x="4.6"
        y="10.2"
        width="14.8"
        height="10.2"
        rx="4.2"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.3"
      />
      <path
        d="M8.0 10.2V7.2C8.0 5.0 9.8 3.2 12 3.2C13.9 3.2 15.5 4.5 15.9 6.2"
        :stroke="resolvedAccent"
        stroke-width="2.4"
        stroke-linecap="round"
      />
      <circle cx="12" cy="15.3" r="1.6" :fill="color" />
    </template>
    <template v-else-if="name === 'image'">
      <rect
        x="3.2"
        y="4.2"
        width="17.6"
        height="15.6"
        rx="4.8"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.3"
      />
      <circle cx="8.6" cy="9.2" r="1.9" :fill="resolvedAccent" />
      <path
        d="M5.0 17.4C7.5 14.0 9.6 12.2 11.4 13.6C12.8 14.6 13.6 15.2 14.8 14.2C16.0 13.2 17.4 14.2 19.0 16.8"
        :stroke="color"
        stroke-width="2.3"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'edit' || name === 'pen'">
      <path
        d="M15.2 4.4C16.2 3.4 17.8 3.4 18.8 4.4L19.6 5.2C20.6 6.2 20.6 7.8 19.6 8.8L9.8 18.6L4.6 19.8L5.8 14.6L15.2 4.4Z"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
      <path
        d="M13.6 6.2L17.8 10.4"
        :stroke="resolvedAccent"
        stroke-width="2.35"
        stroke-linecap="round"
      />
    </template>

    <template v-else-if="name === 'grid' || name === 'grid-view'">
      <rect x="3.5" y="3.5" width="7.2" height="7.2" rx="2.7" :fill="color" fill-opacity="0.2" :stroke="color" stroke-width="2.15" />
      <rect x="13.3" y="3.5" width="7.2" height="7.2" rx="2.7" :fill="resolvedAccent" fill-opacity="0.24" :stroke="resolvedAccent" stroke-width="2.15" />
      <rect x="3.5" y="13.3" width="7.2" height="7.2" rx="2.7" :fill="color" fill-opacity="0.2" :stroke="color" stroke-width="2.15" />
      <rect x="13.3" y="13.3" width="7.2" height="7.2" rx="2.7" :fill="color" fill-opacity="0.2" :stroke="color" stroke-width="2.15" />
    </template>
    <template v-else-if="name === 'list' || name === 'list-view'">
      <circle cx="5.2" cy="6.6" r="1.6" :fill="color" />
      <rect x="9.0" y="5.2" width="11.6" height="2.8" rx="1.4" :fill="color" />
      <circle cx="5.2" cy="12.0" r="1.6" :fill="resolvedAccent" />
      <rect x="9.0" y="10.6" width="11.6" height="2.8" rx="1.4" :fill="resolvedAccent" fill-opacity="0.86" />
      <circle cx="5.2" cy="17.4" r="1.6" :fill="color" fill-opacity="0.68" />
      <rect x="9.0" y="16.0" width="11.6" height="2.8" rx="1.4" :fill="color" fill-opacity="0.68" />
    </template>

    <template v-else-if="name === 'filter'">
      <path
        d="M4.2 5.4C3.6 4.7 4.1 3.8 5.0 3.8H19.0C19.9 3.8 20.4 4.7 19.8 5.4L14.2 12.2V18.0C14.2 18.5 13.9 19.0 13.4 19.3L10.8 20.6C10.2 20.9 9.6 20.4 9.6 19.8V12.2L4.2 5.4Z"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.3"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'sort'">
      <path
        d="M8.0 18.8V5.4M8.0 5.4L4.6 8.8M8.0 5.4L11.4 8.8"
        :stroke="color"
        stroke-width="2.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M16.0 5.2V18.6M16.0 18.6L12.6 15.2M16.0 18.6L19.4 15.2"
        :stroke="resolvedAccent"
        stroke-opacity="0.78"
        stroke-width="2.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'sort-asc'">
      <path
        d="M12 18.8V5.4M12 5.4L7.4 10.0M12 5.4L16.6 10.0"
        :stroke="color"
        stroke-width="2.55"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'sort-desc'">
      <path
        d="M12 5.2V18.6M12 18.6L7.4 14.0M12 18.6L16.6 14.0"
        :stroke="color"
        stroke-width="2.55"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- 侧边栏收起 / 展开 -->
    <template v-else-if="name === 'sidebar-collapse'">
      <rect
        x="2.8"
        y="4.0"
        width="18.4"
        height="16.0"
        rx="4.8"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.25"
      />
      <rect x="5.8" y="7.0" width="3.2" height="10.0" rx="1.6" :fill="resolvedAccent" />
      <path
        d="M15.6 9.2L12.6 12L15.6 14.8"
        :stroke="color"
        stroke-width="2.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'sidebar-expand'">
      <rect
        x="2.8"
        y="4.0"
        width="18.4"
        height="16.0"
        rx="4.8"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.25"
      />
      <rect x="5.8" y="7.0" width="2.6" height="10.0" rx="1.3" :fill="resolvedAccent" />
      <path
        d="M12.4 9.2L15.4 12L12.4 14.8"
        :stroke="resolvedAccent"
        stroke-width="2.45"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- ==================== 8. 播放页黑胶/方图切换 & 桌面歌词排版工具 ==================== -->
    <template v-else-if="name === 'vinyl-switch'">
      <circle
        cx="11"
        cy="12"
        r="8.2"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.35"
      />
      <circle cx="11" cy="12" r="3.1" :fill="resolvedAccent" />
      <path
        d="M18.6 5.2V10.6L16.2 13.0"
        :stroke="resolvedAccent"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <circle cx="18.6" cy="5.2" r="1.8" :fill="color" />
    </template>
    <template v-else-if="name === 'square-switch'">
      <rect
        x="3.4"
        y="3.4"
        width="17.2"
        height="17.2"
        rx="5.4"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.35"
      />
      <circle cx="9.2" cy="9.2" r="2.1" :fill="resolvedAccent" />
      <path
        d="M5.4 17.2L10.4 12.4L13.8 15.2L16.5 12.8L18.8 15.6"
        :stroke="resolvedAccent"
        stroke-width="2.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- 桌面歌词四种排版模式 (为 DesktopLyricWindow 内联 SVG 提供标准名称) -->
    <template v-else-if="name === 'lyric-layout-center'">
      <rect x="2.4" y="3.4" width="19.2" height="17.2" rx="5.4" :fill="color" fill-opacity="0.14" />
      <rect x="4.8" y="6.2" width="14.4" height="2.7" rx="1.35" :fill="color" />
      <rect x="6.7" y="10.65" width="10.6" height="2.7" rx="1.35" :fill="resolvedAccent" />
      <rect x="8.4" y="15.1" width="7.2" height="2.7" rx="1.35" :fill="color" fill-opacity="0.68" />
    </template>
    <template v-else-if="name === 'lyric-layout-left'">
      <rect x="2.4" y="3.4" width="19.2" height="17.2" rx="5.4" :fill="color" fill-opacity="0.14" />
      <rect x="5.2" y="6.2" width="13.6" height="2.7" rx="1.35" :fill="color" />
      <rect x="5.2" y="10.65" width="10.4" height="2.7" rx="1.35" :fill="resolvedAccent" />
      <rect x="5.2" y="15.1" width="7.2" height="2.7" rx="1.35" :fill="color" fill-opacity="0.68" />
    </template>
    <template v-else-if="name === 'lyric-layout-ktv'">
      <rect x="2.4" y="3.4" width="19.2" height="17.2" rx="5.4" :fill="color" fill-opacity="0.14" />
      <rect x="4.6" y="6.4" width="11.4" height="3.0" rx="1.5" :fill="resolvedAccent" />
      <rect x="8.0" y="14.6" width="11.4" height="3.0" rx="1.5" :fill="color" fill-opacity="0.88" />
      <path
        d="M17.8 7.0C19.4 8.7 19.4 11.3 17.8 13.0M6.2 17.0C4.6 15.3 4.6 12.7 6.2 11.0"
        :stroke="resolvedAccent"
        stroke-opacity="0.82"
        stroke-width="2.2"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'lyric-layout-single'">
      <rect
        x="2.6"
        y="5.2"
        width="18.8"
        height="13.6"
        rx="5.4"
        :fill="color"
        fill-opacity="0.16"
        :stroke="color"
        stroke-width="2.25"
      />
      <rect x="6.4" y="10.6" width="11.2" height="2.8" rx="1.4" :fill="resolvedAccent" />
    </template>

    <!-- 桌面歌词宽度缩放 / 字号缩放 / 调色盘 -->
    <template v-else-if="name === 'width-shrink'">
      <rect x="8.6" y="5.5" width="6.8" height="13.0" rx="2.6" :fill="color" fill-opacity="0.22" />
      <path
        d="M2.6 12H7.2M7.2 12L4.8 9.4M7.2 12L4.8 14.6M21.4 12H16.8M16.8 12L19.2 9.4M16.8 12L19.2 14.6"
        :stroke="color"
        stroke-width="2.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'width-expand'">
      <rect x="7.8" y="5.5" width="8.4" height="13.0" rx="2.6" :fill="resolvedAccent" fill-opacity="0.22" />
      <path
        d="M7.0 12H2.5M2.5 12L5.0 9.4M2.5 12L5.0 14.6M17.0 12H21.5M21.5 12L19.0 9.4M21.5 12L19.0 14.6"
        :stroke="color"
        stroke-width="2.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>
    <template v-else-if="name === 'font-minus'">
      <path
        d="M4.2 18.2L9.2 5.8L14.2 18.2M5.8 14.4H12.6"
        :stroke="color"
        stroke-width="2.55"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path d="M15.8 11.5H21.2" :stroke="resolvedAccent" stroke-width="2.6" stroke-linecap="round" />
    </template>
    <template v-else-if="name === 'font-plus'">
      <path
        d="M3.6 18.2L8.6 5.8L13.6 18.2M5.2 14.4H12.0"
        :stroke="color"
        stroke-width="2.55"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M18.2 8.6V14.4M15.3 11.5H21.1"
        :stroke="resolvedAccent"
        stroke-width="2.6"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'palette-orb'">
      <path
        d="M12 3.2C7.0 3.2 3.0 7.1 3.0 12.0C3.0 16.9 7.0 20.8 11.8 20.8C13.4 20.8 14.6 19.6 14.6 18.1C14.6 17.4 14.3 16.8 13.9 16.3C13.6 15.9 13.8 15.2 14.4 15.2H16.2C18.9 15.2 21.0 13.1 21.0 10.4C21.0 6.3 16.9 3.2 12 3.2Z"
        :fill="color"
        fill-opacity="0.2"
        :stroke="color"
        stroke-width="2.35"
      />
      <circle cx="8.2" cy="10.2" r="1.6" :fill="resolvedAccent" />
      <circle cx="11.8" cy="7.6" r="1.6" :fill="color" />
      <circle cx="15.6" cy="9.8" r="1.6" :fill="resolvedAccent" />
    </template>

    <!-- ==================== 9. 批量勾选 / A-B对比 / DSP电源 ==================== -->
    <template v-else-if="name === 'batch-check'">
      <rect
        x="3.2"
        y="3.8"
        width="12.6"
        height="12.6"
        rx="4.0"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.25"
      />
      <path
        d="M6.8 10.2L9.1 12.5L12.8 8.2"
        :stroke="resolvedAccent"
        stroke-width="2.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M19.0 8.0V16.2C19.0 18.5 17.1 20.4 14.8 20.4H7.6"
        :stroke="color"
        stroke-opacity="0.72"
        stroke-width="2.2"
        stroke-linecap="round"
      />
    </template>
    <template v-else-if="name === 'ab-compare'">
      <rect
        x="2.8"
        y="5.2"
        width="8.4"
        height="13.6"
        rx="3.8"
        :fill="color"
        fill-opacity="0.18"
        :stroke="color"
        stroke-width="2.25"
      />
      <rect
        x="12.8"
        y="5.2"
        width="8.4"
        height="13.6"
        rx="3.8"
        :fill="resolvedAccent"
        fill-opacity="0.22"
        :stroke="resolvedAccent"
        stroke-width="2.25"
      />
      <path d="M7.0 9.4V14.6" :stroke="color" stroke-width="2.4" stroke-linecap="round" />
      <path d="M17.0 9.4V14.6" :stroke="resolvedAccent" stroke-width="2.4" stroke-linecap="round" />
    </template>
    <template v-else-if="name === 'power-dsp'">
      <path d="M12 3.4V11.2" :stroke="resolvedAccent" stroke-width="2.7" stroke-linecap="round" />
      <path
        d="M7.2 6.2C4.9 7.8 3.6 10.4 3.8 13.4C4.1 17.6 7.6 20.8 12.0 20.8C16.4 20.8 19.9 17.6 20.2 13.4C20.4 10.4 19.1 7.8 16.8 6.2"
        :stroke="color"
        stroke-width="2.55"
        stroke-linecap="round"
      />
    </template>
    <!-- Mini 灵动岛悬浮小窗切换图标 -->
    <template v-else-if="name === 'mini-island'">
      <rect
        x="3.2"
        y="4.4"
        width="17.6"
        height="15.2"
        rx="4.6"
        :fill="color"
        fill-opacity="0.14"
        :stroke="color"
        stroke-width="2.1"
      />
      <rect
        x="6.6"
        y="7.4"
        width="10.8"
        height="3.4"
        rx="1.7"
        :fill="resolvedAccent"
      />
      <circle cx="8.2" cy="14.6" r="1.5" :fill="color" />
      <rect x="11.2" y="13.5" width="6.2" height="2.2" rx="1.1" :fill="color" fill-opacity="0.78" />
    </template>
    <!-- 收起为边缘胶囊图标 -->
    <template v-else-if="name === 'dock-collapse'">
      <rect
        x="4.2"
        y="4.4"
        width="15.6"
        height="4.4"
        rx="2.2"
        :fill="resolvedAccent"
      />
      <path
        d="M8.4 16.2L12.0 12.4L15.6 16.2"
        :stroke="color"
        stroke-width="2.3"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </template>

    <!-- 兜底圆润双层占位符 -->
    <template v-else>
      <circle cx="12" cy="12" r="8.2" :fill="color" fill-opacity="0.18" :stroke="color" stroke-width="2.2" />
      <circle cx="12" cy="12" r="3.2" :fill="resolvedAccent" />
    </template>
  </svg>
</template>

<style scoped>
.plump-svg-icon {
  display: inline-block;
  vertical-align: middle;
  flex-shrink: 0;
  transition: transform 0.28s cubic-bezier(0.22, 1.35, 0.36, 1),
    filter 0.25s ease;
}
</style>

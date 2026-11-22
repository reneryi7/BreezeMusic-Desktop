<script setup lang="ts">
import { useId } from "vue";

/**
 * BreezeBrandLogo.vue — 微风音乐桌面端专属品牌徽标（方案 C · 微风浮雕极简双色符号）
 * 与系统应用图标 (src-tauri/icons/) 及 Favicon 100% 视觉同源：
 * - 连续曲率超椭圆无框底板（晴空鸢尾靛 × 冰川天青 × 薄荷青玉极光流体渐变）
 * - 柔和单层唱片光晕 + 双道磨砂微风丝带 + 软润饱满负空间黑胶孔风翼音符
 * - 严格遵守 100% Zero-Border 与四主题设计不变量
 */
withDefaults(
  defineProps<{
    size?: number;
  }>(),
  {
    size: 64,
  }
);

const uid = useId().replace(/[^a-zA-Z0-9_-]/g, "");
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 1024 1024"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    class="bm-brand-logo-svg"
    role="img"
    aria-label="BreezeMusic 微风音乐品牌徽标"
  >
    <defs>
      <filter
        :id="`${uid}-ns`"
        x="180"
        y="160"
        width="680"
        height="720"
        filterUnits="userSpaceOnUse"
        color-interpolation-filters="sRGB"
      >
        <feDropShadow
          dx="0"
          dy="22"
          stdDeviation="20"
          flood-color="#1E1B4B"
          flood-opacity="0.30"
        />
        <feDropShadow
          dx="0"
          dy="6"
          stdDeviation="6"
          flood-color="#0C4A6E"
          flood-opacity="0.20"
        />
      </filter>

      <filter
        :id="`${uid}-rs`"
        x="110"
        y="340"
        width="810"
        height="460"
        filterUnits="userSpaceOnUse"
        color-interpolation-filters="sRGB"
      >
        <feDropShadow
          dx="0"
          dy="14"
          stdDeviation="14"
          flood-color="#1E1B4B"
          flood-opacity="0.22"
        />
      </filter>

      <filter :id="`${uid}-blur`" x="-30%" y="-30%" width="160%" height="160%">
        <feGaussianBlur stdDeviation="62" />
      </filter>

      <clipPath :id="`${uid}-sq`">
        <path
          d="M512 40
             C732 40 842 40 913 111
             C984 182 984 292 984 512
             C984 732 984 842 913 913
             C842 984 732 984 512 984
             C292 984 182 984 111 913
             C40 842 40 732 40 512
             C40 292 40 182 111 111
             C182 40 292 40 512 40 Z"
        />
      </clipPath>

      <linearGradient
        :id="`${uid}-bg`"
        x1="64"
        y1="56"
        x2="960"
        y2="968"
        gradientUnits="userSpaceOnUse"
      >
        <stop offset="0%" stop-color="#3B2FCF" />
        <stop offset="38%" stop-color="#535CE0" />
        <stop offset="76%" stop-color="#22A7F0" />
        <stop offset="100%" stop-color="#2DD4BF" />
      </linearGradient>

      <radialGradient
        :id="`${uid}-sheen`"
        cx="275"
        cy="145"
        r="650"
        gradientUnits="userSpaceOnUse"
      >
        <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.34" />
        <stop offset="48%" stop-color="#A5B4FC" stop-opacity="0.09" />
        <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0" />
      </radialGradient>

      <radialGradient
        :id="`${uid}-depth`"
        cx="830"
        cy="880"
        r="560"
        gradientUnits="userSpaceOnUse"
      >
        <stop offset="0%" stop-color="#2DD4BF" stop-opacity="0.42" />
        <stop offset="60%" stop-color="#0EA5E9" stop-opacity="0.06" />
        <stop offset="100%" stop-color="#312E81" stop-opacity="0" />
      </radialGradient>

      <linearGradient
        :id="`${uid}-rib1`"
        x1="126"
        y1="648"
        x2="898"
        y2="416"
        gradientUnits="userSpaceOnUse"
      >
        <stop offset="0%" stop-color="#A5F3FC" stop-opacity="0.22" />
        <stop offset="28%" stop-color="#CFFAFE" stop-opacity="0.74" />
        <stop offset="68%" stop-color="#99F6E4" stop-opacity="0.78" />
        <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0.26" />
      </linearGradient>

      <linearGradient
        :id="`${uid}-rib2`"
        x1="182"
        y1="738"
        x2="866"
        y2="562"
        gradientUnits="userSpaceOnUse"
      >
        <stop offset="0%" stop-color="#67E8F9" stop-opacity="0.12" />
        <stop offset="40%" stop-color="#A7F3D0" stop-opacity="0.50" />
        <stop offset="85%" stop-color="#E0F2FE" stop-opacity="0.46" />
        <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0.10" />
      </linearGradient>

      <linearGradient
        :id="`${uid}-note`"
        x1="330"
        y1="210"
        x2="700"
        y2="800"
        gradientUnits="userSpaceOnUse"
      >
        <stop offset="0%" stop-color="#FFFFFF" />
        <stop offset="56%" stop-color="#F5FAFF" />
        <stop offset="100%" stop-color="#D4ECFF" />
      </linearGradient>

      <mask :id="`${uid}-hole`">
        <rect width="1024" height="1024" fill="white" />
        <circle cx="440" cy="636" r="60" fill="black" />
      </mask>
    </defs>

    <g :clip-path="`url(#${uid}-sq)`">
      <rect x="40" y="40" width="944" height="944" :fill="`url(#${uid}-bg)`" />
      <circle
        cx="245"
        cy="185"
        r="310"
        fill="#818CF8"
        fill-opacity="0.44"
        :filter="`url(#${uid}-blur)`"
      />
      <circle
        cx="825"
        cy="830"
        r="340"
        fill="#2DD4BF"
        fill-opacity="0.50"
        :filter="`url(#${uid}-blur)`"
      />
      <circle
        cx="810"
        cy="220"
        r="250"
        fill="#38BDF8"
        fill-opacity="0.34"
        :filter="`url(#${uid}-blur)`"
      />

      <rect x="40" y="40" width="944" height="944" :fill="`url(#${uid}-sheen)`" />
      <rect x="40" y="40" width="944" height="944" :fill="`url(#${uid}-depth)`" />

      <circle cx="512" cy="520" r="306" fill="#FFFFFF" fill-opacity="0.068" />

      <g :filter="`url(#${uid}-rs)`">
        <path
          d="M116 650
             C242 468, 402 464, 528 540
             C654 614, 780 588, 908 412
             C802 638, 648 686, 514 604
             C388 528, 252 538, 116 650 Z"
          :fill="`url(#${uid}-rib1)`"
        />
        <path
          d="M174 742
             C296 612, 428 610, 548 676
             C662 738, 776 702, 874 560
             C780 750, 650 788, 530 724
             C414 662, 294 668, 174 742 Z"
          :fill="`url(#${uid}-rib2)`"
        />
      </g>

      <g :filter="`url(#${uid}-ns)`" :mask="`url(#${uid}-hole)`">
        <path
          d="M528 232
             C528 209 548 192 572 196
             C674 209 756 269 794 368
             C802 388 788 432 768 439
             C756 444 748 432 746 415
             C734 366 686 344 598 346
             L598 636
             C598 724 527 792 440 792
             C353 792 282 724 282 636
             C282 548 353 480 440 480
             C478 480 508 492 528 512
             Z"
          :fill="`url(#${uid}-note)`"
        />
      </g>
    </g>
  </svg>
</template>

<style scoped>
.bm-brand-logo-svg {
  display: block;
  flex-shrink: 0;
}
</style>

<script setup lang="ts">
import { ref } from "vue";
import { useAppStore } from "../../stores/appStore";
import { APP_VERSION_TAG } from "../../utils/settingsCatalog";
import BreezeBrandLogo from "../BreezeBrandLogo.vue";
import PlumpIcon from "../PlumpIcon.vue";
import SettingsCard from "./SettingsCard.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSwitch from "./SettingsSwitch.vue";

defineProps<{
  highlightedId?: string;
}>();

const store = useAppStore();

const techHighlights = [
  {
    title: "Rust + Tauri 2.0 桌面内核",
    desc: "轻量原生桌面架构、Windows DPAPI 本地凭证金库与低开销多线程异步音频代理。",
    icon: "shield",
    color: "var(--primary)",
  },
  {
    title: "64-bit 风格母带 WebAudio DSP",
    desc: "10/15/31 段参量均衡器、Bauer 耳机交叉馈送、卷积空间混响与跨曲双轨淡变。",
    icon: "wave",
    color: "var(--mint)",
  },
  {
    title: "OKLab 双主色 × 四套高定主题",
    desc: "D1 极夜曜石、D2 丝绒星云、L1 云宣水彩、L2 晨雾燕麦，配合全软件零边框美学与 WebGL2 光学舞台。",
    icon: "palette-orb",
    color: "var(--coral)",
  },
  {
    title: "三窗体桌面协同体验",
    desc: "主画布 + 独立透明穿透桌面歌词 + 四边磁吸 Mini 灵动岛，搭配系统级自定义全局热键。",
    icon: "mini-island",
    color: "var(--cyan)",
  },
];

// 方案 A：隐蔽 5 连击版本号彩蛋（未开启时页面零“下载”字样）
const versionTapCount = ref(0);
let versionTapResetTimer: ReturnType<typeof setTimeout> | null = null;

function handleVersionBadgeClick() {
  if (store.downloadFeatureUnlocked) {
    store.notify(`当前版本 ${APP_VERSION_TAG}（实验性拓展已处于启用状态）`);
    return;
  }

  if (versionTapResetTimer) {
    clearTimeout(versionTapResetTimer);
  }
  versionTapCount.value += 1;

  if (versionTapCount.value >= 5) {
    versionTapCount.value = 0;
    versionTapResetTimer = null;
    store.downloadFeatureUnlocked = true;
    store.notify("已解锁隐藏功能：左侧栏已新增「下载管理」");
    return;
  }

  if (versionTapCount.value >= 3) {
    const remain = 5 - versionTapCount.value;
    store.notify(`再点击 ${remain} 次以开启实验功能`);
  }

  versionTapResetTimer = setTimeout(() => {
    versionTapCount.value = 0;
    versionTapResetTimer = null;
  }, 3000);
}

function handleDisableUnlockedFeature(enabled: boolean) {
  store.downloadFeatureUnlocked = enabled;
  if (!enabled) {
    store.notify("已关闭并重新隐藏「下载管理」入口");
  }
}
</script>

<template>
  <div class="bms-panel-stack">
    <!-- 1. 品牌视觉卡 -->
    <section
      class="bms-about-hero"
      :class="{ 'is-flash-highlight': highlightedId === 'about-brand' }"
      data-setting-id="about-brand"
    >
      <div class="bms-about-brand-left">
        <div class="bms-about-orb">
          <BreezeBrandLogo :size="68" />
        </div>
        <div class="bms-about-copy">
          <div class="bms-about-title-row">
            <h2 class="bms-about-name">BreezeMusic · 微风音乐</h2>
            <button
              type="button"
              class="bms-version-pill"
              aria-label="软件版本号"
              @click="handleVersionBadgeClick"
            >
              {{ APP_VERSION_TAG }}
            </button>
            <span class="bms-chip gold">Windows 桌面版</span>
          </div>
          <p class="bms-about-tagline">
            耳畔微风起，帧享真无损。源自安卓端 BreezeMusic 项目的 Windows 桌面重构版本，基于 Rust + Tauri 2.0 + Vue 3 打造。
          </p>
          <div class="bms-about-badges">
            <span class="bms-chip mint">100% Zero-Border UI</span>
            <span class="bms-chip muted">Tauri 2.0 · Rust · Vue 3</span>
            <span class="bms-chip muted">GPL-3.0 License</span>
          </div>
        </div>
      </div>

      <div class="bms-about-actions">
        <button
          type="button"
          class="bms-btn primary"
          @click="store.openDirectoryInExplorer('data_root')"
        >
          <PlumpIcon name="folder" :size="14" color="#ffffff" />
          <span>打开数据目录</span>
        </button>
      </div>
    </section>

    <!-- 2. 核心引擎架构亮点 -->
    <SettingsCard
      title="核心声学与视觉架构"
      subtitle="面向 Windows 桌面端重新打磨的原生性能与视觉体验"
      icon="sparkles"
      icon-color="var(--primary)"
    >
      <div class="bms-tile-grid">
        <div
          v-for="item in techHighlights"
          :key="item.title"
          class="bms-about-feature"
        >
          <div class="bms-about-feat-icon">
            <PlumpIcon :name="item.icon" :size="18" :color="item.color" />
          </div>
          <div class="bms-about-feat-copy">
            <div class="bms-about-feat-title">{{ item.title }}</div>
            <div class="bms-about-feat-desc">{{ item.desc }}</div>
          </div>
        </div>
      </div>
    </SettingsCard>

    <!-- 3. 项目安全与开源边界 -->
    <SettingsCard
      title="项目说明与本地安全"
      subtitle="透明可控的本地运行机制与个人学习研究声明"
      icon="shield"
      icon-color="var(--mint)"
    >
      <SettingsRow
        title="本地凭证保护 (DPAPI + AES-256 / HMAC)"
        description="所有账号会话凭证均经 Windows DPAPI 及强加密校验后仅存于本机用户数据目录，内存使用完毕即刻清零，绝不上传至任何第三方服务器"
        icon="lock"
        icon-color="var(--mint)"
      >
        <span class="bms-chip mint">本地加密</span>
      </SettingsRow>
      <SettingsRow
        title="开源协议与使用边界"
        description="本项目遵循 GPL-3.0 开源协议，定位为个人技术学习与跨端客户端架构实践之作，不附带也不托管任何受版权保护的音频内容；相关音乐作品版权归原权利人所有"
        icon="shield"
        icon-color="var(--cyan)"
      />

      <!-- 仅在已通过 5 连击解锁后展示，用于随时一键关闭并隐藏 -->
      <SettingsRow
        v-if="store.downloadFeatureUnlocked"
        title="实验性离线拓展模块"
        description="已开启「下载管理」分类与离线标签写入能力；关闭后将立即从左侧导航栏与菜单中隐藏"
        icon="sliders"
        icon-color="var(--gold)"
        badge="已启用"
        badge-tone="gold"
      >
        <button
          type="button"
          class="bms-btn sm"
          @click="store.openSettingsSection('downloads')"
        >
          <span>前往管理</span>
        </button>
        <SettingsSwitch
          :model-value="store.downloadFeatureUnlocked"
          aria-label="开关实验性离线拓展模块"
          @update:model-value="handleDisableUnlockedFeature"
        />
      </SettingsRow>
    </SettingsCard>
  </div>
</template>

<style scoped>
.bms-about-hero {
  border-radius: var(--bm-r-xl, 24px);
  padding: 24px 26px;
  background: var(--bg-card);
  box-shadow: var(--shadow-soft);
  backdrop-filter: blur(20px);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  flex-wrap: wrap;
}

.bms-about-hero.is-flash-highlight {
  box-shadow: 0 0 30px -2px color-mix(in srgb, var(--primary) 50%, transparent);
}

.scheme-watercolor_light.light-variant-nordic_oat .bms-about-hero {
  background: var(--bms-surface-card);
  backdrop-filter: none;
}

.scheme-abyssal_dark.dark-variant-velvet_nebula .bms-about-hero {
  background: var(--bms-surface-card);
  backdrop-filter: none;
}

.bms-about-brand-left {
  display: flex;
  align-items: center;
  gap: 18px;
  flex: 1;
  min-width: 260px;
}

.bms-about-orb {
  width: 68px;
  height: 68px;
  border-radius: 20px;
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  filter: drop-shadow(0 12px 22px color-mix(in srgb, var(--primary) 34%, transparent));
}

.bms-about-copy {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.bms-about-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.bms-about-name {
  margin: 0;
  font-size: var(--bm-fs-title, 20px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
}

.bms-version-pill {
  border: none;
  display: inline-flex;
  align-items: center;
  padding: 2px 10px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-bold, 700);
  background: var(--primary-soft);
  color: var(--primary-text, var(--primary));
  cursor: pointer;
  user-select: none;
  transition: transform 0.14s ease, background 0.18s ease;
}

.bms-version-pill:hover {
  background: color-mix(in srgb, var(--primary) 24%, transparent);
}

.bms-version-pill:active {
  transform: scale(0.94);
}

.bms-about-tagline {
  margin: 0;
  font-size: var(--bm-fs-meta, 13.5px);
  line-height: var(--bm-ui-line-height, 1.48);
  color: var(--text-secondary);
}

.bms-about-badges {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  margin-top: 2px;
}

.bms-about-feature {
  display: flex;
  align-items: flex-start;
  gap: 13px;
  padding: 16px 18px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bms-surface-row);
}

.bms-about-feat-icon {
  width: 38px;
  height: 38px;
  border-radius: 12px;
  background: var(--bms-surface-icon);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.bms-about-feat-copy {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.bms-about-feat-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-primary);
}

.bms-about-feat-desc {
  font-size: var(--bm-fs-caption, 12.5px);
  line-height: var(--bm-ui-line-height, 1.52);
  color: var(--text-secondary);
}

.scheme-watercolor_light.light-variant-nordic_oat .bms-about-feature {
  background: var(--bms-surface-row);
}

.scheme-watercolor_light.light-variant-nordic_oat .bms-about-feat-icon {
  background: var(--bms-surface-icon);
}

.scheme-abyssal_dark.dark-variant-velvet_nebula .bms-about-feature {
  background: var(--bms-surface-row);
}
</style>

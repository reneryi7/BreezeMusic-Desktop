<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useAppStore } from "../../stores/appStore";
import type { StartupHomePage } from "../../types";
import PlumpIcon from "../PlumpIcon.vue";
import SettingsCard from "./SettingsCard.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSwitch from "./SettingsSwitch.vue";

defineProps<{
  highlightedId?: string;
}>();

const store = useAppStore();

const startupHomeOptions: ReadonlyArray<{ id: StartupHomePage; label: string }> = [
  { id: "discover", label: "探索发现" },
  { id: "library", label: "我的音乐库" },
  { id: "local_music", label: "本地音乐" },
  { id: "last", label: "上次浏览" },
];

onMounted(() => {
  store.refreshAutostartState();
});

const lastRefreshText = computed(() => {
  const ts = store.lastFullRefreshAt;
  if (!ts) return "尚未执行后台刷新";
  const d = new Date(ts);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  return `上次同步 ${hh}:${mm}:${ss}`;
});

const vipBadgeText = computed(() => {
  if (!store.isLoggedIn) return "未登录";
  if (store.vipStatus?.svipActive) return "概念 VIP 生效中";
  if (store.vipStatus?.tvipActive) return "畅听 VIP 生效中";
  return "每日免费领 VIP";
});

function handleToggleCloseToTray(val: boolean) {
  store.closeToTray = val;
  store.notify(
    val
      ? "关闭主窗口时将最小化至系统托盘继续播放"
      : "关闭主窗口时将直接退出微风音乐"
  );
}

function handleToggleMiniYieldFullscreen(val: boolean) {
  store.miniYieldFullscreenEnabled = val;
  store.notify(
    val
      ? "检测到同屏全屏视频、游戏或 F11 全屏时将自动隐藏 Mini 灵动岛（不影响普通最大化窗口）"
      : "已关闭全屏自动礼让，Mini 灵动岛将始终保持置顶"
  );
}

async function handleManualRefreshAll() {
  if (store.isBackgroundRefreshing) return;
  store.notify("正在错峰刷新全模块推荐与云端资产...");
  await store.runHourlySilentRefresh();
  store.notify("后台静默全量同步已完成");
}
</script>

<template>
  <div class="bms-panel-stack">
    <!-- 1. 账号与概念 VIP 状态摘要 -->
    <SettingsCard
      title="账号与概念版 VIP 权益"
      subtitle="管理酷狗概念版多账号持久化会话、云端听歌等级与每日签到自动化"
      icon="user"
      icon-color="var(--cyan)"
    >
      <div
        class="bms-account-hero"
        :class="{ 'is-flash-highlight': highlightedId === 'general-account-summary' }"
        data-setting-id="general-account-summary"
      >
        <div class="bms-account-left">
          <div class="bms-account-avatar">
            <img
              v-if="store.isLoggedIn && (store.profile?.avatarUrl || store.session?.avatarUrl)"
              :src="store.profile?.avatarUrl || store.session?.avatarUrl"
              alt="当前用户头像"
            />
            <PlumpIcon v-else name="user" :size="22" color="#ffffff" />
          </div>
          <div class="bms-account-meta">
            <div class="bms-account-name-row">
              <span class="bms-account-name">
                {{ store.isLoggedIn ? (store.profile?.nickname || store.session?.nickname || "微风行者") : "未登录酷狗概念版账号" }}
              </span>
              <span
                class="bms-chip"
                :class="store.vipStatus?.svipActive || store.vipStatus?.tvipActive ? 'gold' : 'muted'"
              >
                <PlumpIcon name="crown" :size="11" />
                <span>{{ vipBadgeText }}</span>
              </span>
              <span v-if="store.isLoggedIn && store.profile?.userGrade" class="bms-chip mint">
                Lv.{{ store.profile.userGrade }}
              </span>
            </div>
            <div class="bms-account-sub">
              <template v-if="store.isLoggedIn">
                累计听歌 {{ store.profile?.listenDurationFormatted || "计算中" }}
                <span v-if="store.profile?.vipExpireText"> · {{ store.profile.vipExpireText }}</span>
                <span v-if="store.savedAccounts.length > 1"> · 已保存 {{ store.savedAccounts.length }} 个账号</span>
              </template>
              <template v-else>
                登录后可同步云端歌单、红心喜欢、云盘曲库，并每日免费领取概念版无损 VIP 权益
              </template>
            </div>
          </div>
        </div>

        <div class="bms-account-actions">
          <button
            v-if="!store.isLoggedIn"
            type="button"
            class="bms-btn primary"
            @click="store.isLoginModalOpen = true"
          >
            <PlumpIcon name="user" :size="14" color="#ffffff" />
            <span>立即登录</span>
          </button>
          <template v-else>
            <button
              type="button"
              class="bms-btn"
              @click="store.isVipCenterOpen = true"
            >
              <PlumpIcon name="crown" :size="14" color="var(--gold)" />
              <span>VIP 中心</span>
            </button>
            <button
              type="button"
              class="bms-btn"
              :disabled="store.isSyncingListeningGrade"
              @click="store.syncListeningGradeCloud(true)"
            >
              <PlumpIcon name="refresh" :size="14" />
              <span>{{ store.isSyncingListeningGrade ? "同步中..." : "同步时长" }}</span>
            </button>
            <button
              type="button"
              class="bms-btn ghost"
              @click="store.isAccountManagerOpen = true"
            >
              <span>管理多账号</span>
            </button>
          </template>
        </div>
      </div>

      <SettingsRow
        title="启动时自动每日签到领概念版 VIP"
        description="打开软件并识别登录态后，自动静默领取当日畅听与概念版无损会员权益"
        icon="crown"
        icon-color="var(--gold)"
        label-for="bms-sw-auto-vip"
        setting-id="general-auto-vip"
        :highlighted="highlightedId === 'general-auto-vip'"
      >
        <SettingsSwitch
          id="bms-sw-auto-vip"
          v-model="store.autoClaimVipOnStartup"
          aria-label="启动时自动每日签到领概念版 VIP"
        />
      </SettingsRow>
    </SettingsCard>

    <!-- 2. 启动与系统窗口行为 -->
    <SettingsCard
      title="启动与窗口行为"
      subtitle="配置冷启动默认首页、托盘驻留与开机自启"
      icon="settings"
      icon-color="var(--primary)"
    >
      <SettingsRow
        title="启动时默认打开页面"
        description="决定冷启动进入微风音乐后第一时间呈现的主界面"
        icon="sparkles"
        icon-color="var(--primary)"
        setting-id="general-startup-home"
        :highlighted="highlightedId === 'general-startup-home'"
      >
        <div class="bms-pill-group" role="radiogroup" aria-label="启动默认首页">
          <button
            v-for="opt in startupHomeOptions"
            :key="opt.id"
            type="button"
            role="radio"
            class="bms-pill-opt"
            :class="{ active: store.startupHomePage === opt.id }"
            :aria-checked="store.startupHomePage === opt.id"
            @click="store.startupHomePage = opt.id"
          >
            {{ opt.label }}
          </button>
        </div>
      </SettingsRow>

      <SettingsRow
        title="关闭主窗口时最小化至系统托盘"
        description="开启后点击右上角「×」隐藏至任务栏右下角托盘继续后台听歌；关闭则直接退出程序"
        icon="minimize"
        icon-color="var(--cyan)"
        :badge="store.closeToTray ? '最小化到托盘' : '直接退出'"
        :badge-tone="store.closeToTray ? 'mint' : 'muted'"
        label-for="bms-sw-close-tray"
        setting-id="general-close-to-tray"
        :highlighted="highlightedId === 'general-close-to-tray'"
      >
        <SettingsSwitch
          id="bms-sw-close-tray"
          :model-value="store.closeToTray"
          aria-label="关闭主窗口时最小化至系统托盘"
          @update:model-value="handleToggleCloseToTray"
        />
      </SettingsRow>

      <SettingsRow
        title="全屏视频/游戏时自动隐藏 Mini 灵动岛"
        description="同屏进入真正的 F11 浏览器全屏、全屏视频、PPT 放映或全屏游戏时自动无痕隐藏灵动岛，退出后瞬时恢复（自动隐藏任务栏与普通最大化窗口不受影响）"
        icon="sparkles"
        icon-color="var(--cyan)"
        :badge="store.miniYieldFullscreenEnabled ? '智能礼让' : '始终置顶'"
        :badge-tone="store.miniYieldFullscreenEnabled ? 'mint' : 'muted'"
        label-for="bms-sw-mini-yield-fs"
        setting-id="general-mini-yield-fullscreen"
        :highlighted="highlightedId === 'general-mini-yield-fullscreen'"
      >
        <SettingsSwitch
          id="bms-sw-mini-yield-fs"
          :model-value="store.miniYieldFullscreenEnabled"
          aria-label="全屏视频或游戏时自动隐藏 Mini 灵动岛"
          @update:model-value="handleToggleMiniYieldFullscreen"
        />
      </SettingsRow>

      <SettingsRow
        title="切回前台自动识别剪贴板酷狗分享链接"
        description="复制官方酷狗音乐或酷狗概念版单曲/歌单分享链接后，切回微风音乐窗口时自动弹出快速播放与查看确认"
        icon="link"
        icon-color="var(--mint)"
        :badge="store.clipboardShareDetectEnabled ? '自动识别' : '已关闭'"
        :badge-tone="store.clipboardShareDetectEnabled ? 'mint' : 'muted'"
        label-for="bms-sw-clipboard-share"
        setting-id="general-clipboard-share"
        :highlighted="highlightedId === 'general-clipboard-share'"
      >
        <SettingsSwitch
          id="bms-sw-clipboard-share"
          v-model="store.clipboardShareDetectEnabled"
          aria-label="切回前台自动识别剪贴板酷狗分享链接"
        />
      </SettingsRow>

      <SettingsRow
        title="Windows 开机自动启动"
        description="登录当前 Windows 用户桌面后自动运行微风音乐（写入当前用户级启动项，无需管理员权限）"
        icon="sparkles"
        icon-color="var(--violet)"
        :badge="store.autostartEnabled ? '已启用' : '未启用'"
        :badge-tone="store.autostartEnabled ? 'mint' : 'muted'"
        label-for="bms-sw-autostart"
        setting-id="general-autostart"
        :highlighted="highlightedId === 'general-autostart'"
      >
        <SettingsSwitch
          id="bms-sw-autostart"
          :model-value="store.autostartEnabled"
          :disabled="store.isUpdatingAutostart"
          aria-label="Windows 开机自动启动"
          @update:model-value="store.setAutostartEnabled($event)"
        />
      </SettingsRow>
    </SettingsCard>

    <!-- 3. 后台同步与桌面小窗 -->
    <SettingsCard
      title="后台静默同步与灵动小窗"
      subtitle="每小时低并发错峰刷新推荐、榜单、歌单与本地曲库（含系统休眠唤醒补偿）"
      icon="refresh"
      icon-color="var(--mint)"
    >
      <SettingsRow
        title="全模块数据自动刷新调度器"
        :description="`${lastRefreshText} · 绝不打断正在播放的音频流，亦不干扰批量编辑`"
        icon="clock"
        icon-color="var(--mint)"
        setting-id="general-auto-refresh"
        :highlighted="highlightedId === 'general-auto-refresh'"
      >
        <button
          type="button"
          class="bms-btn"
          :disabled="store.isBackgroundRefreshing"
          @click="handleManualRefreshAll"
        >
          <PlumpIcon name="refresh" :size="14" />
          <span>{{ store.isBackgroundRefreshing ? "正在静默同步..." : "立即全量同步" }}</span>
        </button>
      </SettingsRow>

      <SettingsRow
        title="Mini 灵动岛桌面悬浮小窗"
        description="收起主窗口，切换为支持四边磁吸停靠与歌词滚动的独立透明置顶迷你播控条"
        icon="mini-island"
        icon-color="var(--primary)"
        :badge="store.globalHotkeys.toggle_mini_mode || 'Ctrl+Alt+M'"
        badge-tone="muted"
      >
        <button
          type="button"
          class="bms-btn primary"
          @click="store.toggleMiniPlayerMode(true)"
        >
          <PlumpIcon name="mini-island" :size="14" color="#ffffff" />
          <span>切入 Mini 小窗</span>
        </button>
      </SettingsRow>
    </SettingsCard>
  </div>
</template>

<style scoped>
.bms-account-hero {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
  padding: 15px 17px;
  border-radius: 15px;
  background: var(--bms-surface-tile);
  transition: box-shadow 0.25s ease;
}

.bms-account-hero.is-flash-highlight {
  box-shadow: 0 0 28px -2px color-mix(in oklab, var(--primary) 52%, transparent);
}

.bms-account-left {
  display: flex;
  align-items: center;
  gap: 14px;
  min-width: 0;
  flex: 1;
}

.bms-account-avatar {
  width: 52px;
  height: 52px;
  border-radius: 16px;
  overflow: hidden;
  background: var(--grad-kugou-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  box-shadow: 0 8px 18px -6px color-mix(in srgb, var(--primary) 42%, transparent);
}

.bms-account-avatar img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.bms-account-meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.bms-account-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.bms-account-name {
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-primary);
}

.bms-account-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
  line-height: 1.45;
}

.bms-account-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
</style>

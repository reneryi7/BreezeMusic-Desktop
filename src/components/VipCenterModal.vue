<script setup lang="ts">
import { computed, ref } from "vue";
import { useAppStore } from "../stores/appStore";
import { triggerVipCelebrate } from "../utils/microMotionFx";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();
const showDeviceDiagnostics = ref(false);
const showPlainCredentials = ref(false);
const isCelebratingClaim = ref(false);

async function handleClaimVipDaily(event: MouseEvent) {
  await store.claimVipDaily(false);
  if (store.vipStatus?.todayClaimed) {
    triggerVipCelebrate(event);
    isCelebratingClaim.value = false;
    window.requestAnimationFrame(() => {
      isCelebratingClaim.value = true;
      window.setTimeout(() => {
        isCelebratingClaim.value = false;
      }, 900);
    });
  }
}

const currentDateObj = computed(() => new Date(store.nowClockTick));
const currentYear = computed(() => currentDateObj.value.getFullYear());
const currentMonth = computed(() => currentDateObj.value.getMonth() + 1);
const todayDay = computed(() => currentDateObj.value.getDate());

const daysInMonth = computed(() => {
  return new Date(currentYear.value, currentMonth.value, 0).getDate();
});

const weekHeaders = ["一", "二", "三", "四", "五", "六", "日"];
const leadingBlanks = computed(() => {
  const firstDayWeek = new Date(currentYear.value, currentMonth.value - 1, 1).getDay(); // 0=Sun..6=Sat
  return firstDayWeek === 0 ? 6 : firstDayWeek - 1;
});

const calendarCells = computed(() => {
  const recs = store.vipStatus?.records || [];
  const cells: {
    day: number;
    dateStr: string;
    claimed: boolean;
    isSvip: boolean;
    isToday: boolean;
  }[] = [];

  for (let d = 1; d <= daysInMonth.value; d++) {
    const mm = String(currentMonth.value).padStart(2, "0");
    const dd = String(d).padStart(2, "0");
    const dateStr = `${currentYear.value}-${mm}-${dd}`;
    const found = recs.find((r) => (r.day === dateStr || r.day.endsWith(`-${mm}-${dd}`)) && r.received);
    const isClaimed = Boolean(found) || (d === todayDay.value && Boolean(store.vipStatus?.todayClaimed));
    const isSvipDay = isClaimed && (found?.vipType === "svip" || (d === todayDay.value && Boolean(store.vipStatus?.svipActive)));
    cells.push({
      day: d,
      dateStr,
      claimed: isClaimed,
      isSvip: isSvipDay,
      isToday: d === todayDay.value,
    });
  }
  return cells;
});

function formatCred(val?: string) {
  if (!val) return "已就绪";
  if (showPlainCredentials.value) return val;
  if (val.length <= 10) return val;
  return `${val.slice(0, 6)}••••••${val.slice(-4)}`;
}

async function copyCred(label: string, val?: string) {
  if (!val) return;
  try {
    await navigator.clipboard.writeText(val);
    store.notify(`已复制 ${label} 至剪贴板`);
  } catch {
    store.notify(`${label}: ${val}`);
  }
}
</script>

<template>
  <transition name="fade">
    <div
      v-if="store.isVipCenterOpen"
      class="modal-backdrop"
      @click.self="store.isVipCenterOpen = false"
    >
      <div class="vip-modal-card vip-dual-modal">
        <div class="modal-header">
          <div class="vip-title-row">
            <div
              class="vip-icon-orb"
              :class="{ 'is-celebrating': isCelebratingClaim }"
            >
              <PlumpIcon name="crown" :size="20" color="#ffd54f" />
            </div>
            <div>
              <h3>概念版 VIP 权益中心</h3>
              <p class="vip-sub-caption">
                每日免费签到领取畅听与概念版会员，解锁完整无损音源
              </p>
            </div>
          </div>
          <button class="icon-close-btn" @click="store.isVipCenterOpen = false">
            <PlumpIcon name="close" :size="16" />
          </button>
        </div>

        <!-- 桌面端双栏黑金布局：左栏会员状态与自动签到 + 右栏月度签到日历与设备指纹诊断 -->
        <div class="vip-dual-grid">
          <!-- 左栏 -->
          <div class="vip-left-col">
            <div class="vip-hero-banner">
              <div class="vip-hero-top">
                <div>
                  <div class="vip-badge-pill">概念版专属权益</div>
                  <h4 class="vip-hero-title">
                    {{
                      store.vipStatus?.svipActive
                        ? "概念版 VIP 会员生效中"
                        : store.vipStatus?.tvipActive
                        ? "畅听 VIP 会员生效中"
                        : "每日签到免费领概念版 VIP"
                    }}
                  </h4>
                  <p class="vip-hero-sub">
                    {{
                      store.vipStatus?.statusMessage ||
                      "登录后每日可免费签到领取 1 天畅听 VIP 并升级概念版权益"
                    }}
                  </p>
                </div>
              </div>

              <div class="vip-benefit-rows">
                <div class="vip-benefit-row">
                  <PlumpIcon name="check" :size="16" accent="#ffd54f" />
                  <span>
                    畅听 VIP：{{
                      store.vipStatus?.tvipActive
                        ? `有效 · 到期 ${store.vipStatus.tvipExpiry}`
                        : "签到立即激活"
                    }}
                  </span>
                </div>
                <div class="vip-benefit-row">
                  <PlumpIcon name="check" :size="16" accent="var(--cyan)" />
                  <span>
                    概念 VIP：{{
                      store.vipStatus?.svipActive
                        ? `有效 · 到期 ${store.vipStatus.svipExpiry}`
                        : "签到后自动触发激励升级"
                    }}
                  </span>
                </div>
              </div>

              <button
                class="vip-gold-claim-cta"
                :disabled="store.isClaimingVip"
                @click="handleClaimVipDaily($event)"
              >
                <PlumpIcon name="crown" :size="17" color="#1a1200" />
                <span>
                  {{
                    store.isClaimingVip
                      ? "正在签到并升级概念版 VIP..."
                      : store.vipStatus?.todayClaimed
                      ? "今日已领 · 再次核验升级状态"
                      : "一键领取今日概念版 VIP"
                  }}
                </span>
              </button>
            </div>

            <!-- 启动自动签到守护卡片（对齐 Android VipCenterBottomSheet） -->
            <div class="vip-auto-guard-card">
              <div class="guard-info">
                <div class="guard-title">打开应用时自动静默领取</div>
                <div class="guard-desc">
                  启动客户端时自动检测并静默签到当日 1 天概念版 VIP 权益
                </div>
              </div>
              <label class="plump-switch">
                <input
                  v-model="store.autoClaimVipOnStartup"
                  type="checkbox"
                />
                <span class="plump-slider"></span>
              </label>
            </div>
          </div>

          <!-- 右栏：当月签到日历矩阵 + 设备标识与音源鉴权凭证 -->
          <div class="vip-right-col">
            <div class="vip-calendar-card">
              <div class="cal-header">
                <span class="cal-title">
                  {{ currentYear }} 年 {{ currentMonth }} 月签到日历
                </span>
                <span class="cal-count-pill">
                  本月已领 {{ store.vipStatus?.monthClaimedDays || (store.vipStatus?.todayClaimed ? 1 : 0) }} /
                  {{ daysInMonth }} 天
                </span>
              </div>

              <div class="cal-grid">
                <div
                  v-for="wh in weekHeaders"
                  :key="'wh_' + wh"
                  class="cal-day-cell cal-weekday-cell"
                >
                  {{ wh }}
                </div>
                <div
                  v-for="b in leadingBlanks"
                  :key="'blank_' + b"
                  class="cal-day-cell cal-blank-cell"
                ></div>
                <div
                  v-for="cell in calendarCells"
                  :key="cell.day"
                  class="cal-day-cell"
                  :class="{
                    claimed: cell.claimed,
                    svip: cell.isSvip,
                    today: cell.isToday,
                    'is-fresh-claim': cell.isToday && isCelebratingClaim,
                  }"
                >
                  {{ cell.day }}
                </div>
              </div>

              <div class="cal-legend">
                <span><i class="dot gold"></i> 概念 VIP</span>
                <span><i class="dot cyan"></i> 畅听 VIP</span>
                <span><i class="dot green"></i> 今日</span>
              </div>
            </div>

            <!-- 可折叠：设备与连接诊断抽屉（对普通听众保持简洁，需要时一键展开） -->
            <div class="vip-device-card" :class="{ 'is-collapsed': !showDeviceDiagnostics }">
              <button
                type="button"
                class="dev-card-head dev-drawer-trigger"
                :aria-expanded="showDeviceDiagnostics"
                @click="showDeviceDiagnostics = !showDeviceDiagnostics"
              >
                <div class="dev-head-left">
                  <PlumpIcon name="shield" :size="16" color="var(--cyan)" />
                  <span>设备与连接诊断</span>
                  <span v-if="store.gatewayDiag" class="dev-latency-pill">
                    {{ store.gatewayDiag.latencyMs }}ms
                  </span>
                </div>
                <span class="dev-drawer-chevron" :class="{ open: showDeviceDiagnostics }">
                  <PlumpIcon name="chevron-down" :size="14" />
                </span>
              </button>

              <div
                class="dev-drawer-collapse"
                :class="{ open: showDeviceDiagnostics }"
                :inert="!showDeviceDiagnostics ? true : undefined"
              >
                <div class="dev-drawer-collapse-inner">
                  <div class="dev-drawer-body">
                    <div class="dev-kv-row">
                      <span>设备指纹 (DFID)</span>
                      <div class="dev-kv-right">
                        <code>{{ formatCred(store.vipStatus?.dfid || store.gatewayDiag?.dfid) }}</code>
                        <button
                          type="button"
                          class="dev-copy-btn"
                          @click="copyCred('DFID', store.vipStatus?.dfid || store.gatewayDiag?.dfid)"
                        >
                          复制
                        </button>
                      </div>
                    </div>

                    <div class="dev-kv-row">
                      <span>会话标识 (GUID)</span>
                      <div class="dev-kv-right">
                        <code>{{ formatCred(store.vipStatus?.guid || store.gatewayDiag?.guid) }}</code>
                        <button
                          type="button"
                          class="dev-copy-btn"
                          @click="copyCred('GUID', store.vipStatus?.guid || store.gatewayDiag?.guid)"
                        >
                          复制
                        </button>
                      </div>
                    </div>

                    <div class="dev-actions-row">
                      <button
                        type="button"
                        class="dev-toggle-plain"
                        @click="showPlainCredentials = !showPlainCredentials"
                      >
                        {{ showPlainCredentials ? "隐藏完整凭证" : "显示完整凭证" }}
                      </button>
                      <button
                        type="button"
                        class="tonal-action-btn icon-only-btn"
                        :disabled="store.isDiagnosingGateway"
                        data-tip="刷新设备连接标识"
                        aria-label="刷新设备连接标识"
                        @click="store.resignDeviceAndRefresh()"
                      >
                        <PlumpIcon name="refresh" :size="15" />
                      </button>
                      <button
                        type="button"
                        class="tonal-action-btn cyan icon-only-btn"
                        :disabled="store.isDiagnosingGateway"
                        :data-tip="
                          store.gatewayDiag
                            ? `音源链路正常 (${store.gatewayDiag.latencyMs}ms)`
                            : '检测音源连接延迟'
                        "
                        aria-label="检测音源连接延迟"
                        @click="store.runGatewayDiagnostic()"
                      >
                        <PlumpIcon name="wave" :size="15" />
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </transition>
</template>

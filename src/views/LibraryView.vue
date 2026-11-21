<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useAppStore } from "../stores/appStore";
import { resolveSongQualityBadge } from "../api/kugou";
import type { FollowedArtist, Playlist, PurchasedAlbumItem, Song } from "../types";
import PlumpIcon from "../components/PlumpIcon.vue";

const store = useAppStore();

const galleryFilterKw = ref("");
const isRefreshingAssets = ref(false);

// 新建歌单、编辑歌单与外部导入弹窗状态
const showCreateModal = ref(false);
const newPlaylistName = ref("");

const showImportModal = ref(false);
const importInputText = ref("");

const showEditModal = ref(false);
const editingPlaylist = ref<Playlist | null>(null);
const editTitle = ref("");
const editIntro = ref("");

const levelNum = computed(() => {
  return store.profile?.userGrade || store.profile?.userLevel || 1;
});

const nextLevelNum = computed(() => {
  const ng = store.profile?.nextGrade;
  return ng && ng > levelNum.value ? ng : levelNum.value + 1;
});

const listenTotalMinutes = computed(() => {
  const sec = store.profile?.listenDurationSec ?? 0;
  return sec > 0
    ? Math.floor(sec / 60)
    : store.profile?.listenDurationMin ?? 0;
});

const listenHoursNumber = computed(() => {
  return Math.floor(listenTotalMinutes.value / 60);
});

const listenRemainderMins = computed(() => {
  return listenTotalMinutes.value % 60;
});

const levelProgressPercent = computed(() => {
  const p = store.profile?.levelProgressPct;
  if (typeof p === "number" && Number.isFinite(p)) {
    return Math.min(100, Math.max(0, p));
  }
  return 0;
});

const gradeSpeedText = computed(() => {
  const sp = store.profile?.gradeSpeed;
  if (typeof sp === "number" && Number.isFinite(sp) && sp > 0) {
    return Number.isInteger(sp) ? `${sp}.0x` : `${sp.toFixed(1)}x`;
  }
  return "1.0x";
});

const nextLevelHintText = computed(() => {
  const cur = store.profile?.userScore || 0;
  const next = store.profile?.nextLevelScore || 0;
  if (cur > 0 && next > 0) {
    return `成长值 ${cur.toLocaleString()} / ${next.toLocaleString()}`;
  }
  const diff = store.profile?.nextLevelRemainMin || 0;
  if (diff > 0) {
    const h = Math.floor(diff / 60);
    const m = diff % 60;
    return h > 0
      ? `距 Lv.${nextLevelNum.value} 需 ${h}h ${m}m`
      : `距 Lv.${nextLevelNum.value} 需 ${m} 分钟`;
  }
  return `云端成长值实时校准`;
});

const gradeDetailTooltip = computed(() => {
  const base = store.profile?.gradeBaseScore || 0;
  const cur = store.profile?.userScore || 0;
  const next = store.profile?.nextLevelScore || 0;
  const remScore =
    store.profile?.nextLevelRemainScore ?? Math.max(0, next - cur);
  if (next > 0) {
    return `Lv.${levelNum.value} 起点 ${base.toLocaleString()} · 当前 ${cur.toLocaleString()} / ${next.toLocaleString()}（差 ${remScore.toLocaleString()} 成长值升级，当前加速 ${gradeSpeedText.value}）`;
  }
  return `云端累计听歌 ${listenTotalMinutes.value.toLocaleString()} 分钟`;
});

const likedTotalCount = computed(() => {
  return Math.max(
    store.likedSongs.length,
    store.likedPlaylistMeta?.trackCount || 0
  );
});

const likedHiResCount = computed(() => {
  return store.likedSongs.filter(
    (s) => s.hasHiRes || s.hasSq || s.isLossless || s.isHiRes
  ).length;
});

const likedDurationText = computed(() => {
  return formatSongsTotalHours(store.likedSongs);
});

function formatSongsTotalHours(songs: Song[]): string {
  if (!songs.length) return "双端云同步";
  const totalSec = songs.reduce((acc, s) => {
    const sec = s.durationSeconds || Math.floor((s.durationMs || 0) / 1000);
    return acc + (Number.isFinite(sec) ? sec : 0);
  }, 0);
  const hours = Math.floor(totalSec / 3600);
  const mins = Math.max(1, Math.floor((totalSec % 3600) / 60));
  return hours > 0 ? `约 ${hours} 小时 ${mins} 分` : `约 ${mins} 分钟`;
}

const assetTabs = computed(() => [
  {
    id: "created" as const,
    label: "自建歌单",
    count: store.createdPlaylists.length,
    color: "var(--primary)",
  },
  {
    id: "collected" as const,
    label: "收藏歌单",
    count: store.collectedPlaylists.length,
    color: "var(--accent-coral)",
  },
  {
    id: "artists" as const,
    label: "关注歌手",
    count: store.followedArtists.length,
    color: "var(--accent-mint)",
  },
  {
    id: "purchased" as const,
    label: "已购内容",
    count:
      store.purchasedAlbums.length + (store.purchasedSongs.length > 0 ? 1 : 0),
    color: "var(--accent-gold)",
  },
]);

const filteredPlaylists = computed<Playlist[]>(() => {
  const base =
    store.libraryAssetTab === "collected"
      ? store.collectedPlaylists
      : store.createdPlaylists;
  const kw = galleryFilterKw.value.trim().toLowerCase();
  if (!kw) return base;
  return base.filter(
    (p) =>
      (p.title || p.name || "").toLowerCase().includes(kw) ||
      (p.description || p.intro || "").toLowerCase().includes(kw) ||
      (p.creator || "").toLowerCase().includes(kw)
  );
});

const filteredArtists = computed<FollowedArtist[]>(() => {
  const kw = galleryFilterKw.value.trim().toLowerCase();
  if (!kw) return store.followedArtists;
  return store.followedArtists.filter((a) =>
    (a.name || "").toLowerCase().includes(kw)
  );
});

const filteredPurchasedAlbums = computed<PurchasedAlbumItem[]>(() => {
  const kw = galleryFilterKw.value.trim().toLowerCase();
  if (!kw) return store.purchasedAlbums;
  return store.purchasedAlbums.filter(
    (a) =>
      (a.albumName || "").toLowerCase().includes(kw) ||
      (a.singerName || a.artistName || "").toLowerCase().includes(kw)
  );
});

const localDirShortName = computed(() => {
  const dir = store.localMusicDir || store.downloadMusicDir || "";
  if (!dir) return "默认下载目录";
  const normalized = dir.replace(/\\/g, "/").replace(/\/+$/, "");
  const parts = normalized.split("/").filter(Boolean);
  return parts[parts.length - 1] || normalized;
});

function handleSelectAssetTab(
  tabId: "created" | "collected" | "artists" | "purchased"
) {
  store.libraryAssetTab = tabId;
  if (tabId === "artists" && store.followedArtists.length === 0) {
    store.loadFollowedArtists();
  } else if (
    tabId === "purchased" &&
    store.purchasedAlbums.length === 0 &&
    store.purchasedSongs.length === 0
  ) {
    store.loadPurchasedAssets();
  }
}

function playAllCloudDisk() {
  if (store.cloudDiskSongs.length > 0) {
    store.playSongFromList(
      store.cloudDiskSongs[0],
      store.cloudDiskSongs,
      "个人音乐云盘"
    );
  } else {
    store.openCloudDrivePage();
  }
}

function playAllLocalMusic() {
  if (store.localMusicSongs.length > 0) {
    store.playSongFromList(
      store.localMusicSongs[0],
      store.localMusicSongs,
      "本地音乐"
    );
  } else {
    store.openLocalMusicPage();
  }
}

async function refreshLibraryAssets() {
  if (!store.isLoggedIn) {
    store.isLoginModalOpen = true;
    return;
  }
  if (isRefreshingAssets.value) return;
  isRefreshingAssets.value = true;
  try {
    await Promise.all([
      store.loadUserPlaylists(),
      store.loadRecentHistory(),
      store.loadFollowedArtists(),
      store.loadPurchasedAssets(),
      store.loadUserCloudDisk(true),
    ]);
    store.notify("已全面刷新我的音乐库与已购资产 ✨");
  } finally {
    isRefreshingAssets.value = false;
  }
}

function playAllLiked() {
  if (store.likedSongs.length > 0) {
    store.playSongFromList(
      store.likedSongs[0],
      store.likedSongs,
      "我喜欢的音乐"
    );
  } else {
    store.openLikedSongsDetail();
  }
}

function shufflePlayLiked() {
  if (store.likedSongs.length > 0) {
    store.playMode = "shuffle";
    const randIdx = Math.floor(Math.random() * store.likedSongs.length);
    store.playSongFromList(
      store.likedSongs[randIdx],
      store.likedSongs,
      "我喜欢的音乐 · 随机漫游"
    );
  } else {
    store.openLikedSongsDetail();
  }
}

function playAllRecent() {
  if (store.recentHistorySongs.length > 0) {
    store.playSongFromList(
      store.recentHistorySongs[0],
      store.recentHistorySongs,
      "最近播放"
    );
  } else {
    store.openRecentHistoryDetail();
  }
}

function playAllPurchasedSingles() {
  const songs = store.purchasedSongs.map((p) => p.song);
  if (songs.length > 0) {
    store.playSongFromList(songs[0], songs, "已购单曲合集");
  } else {
    store.openPurchasedSongsDetail();
  }
}

function openCreateModal() {
  if (!store.isLoggedIn) {
    store.isLoginModalOpen = true;
    return;
  }
  newPlaylistName.value = "";
  showCreateModal.value = true;
}

async function submitCreatePlaylist() {
  const name = newPlaylistName.value.trim();
  if (!name) return;
  showCreateModal.value = false;
  await store.createNewPlaylist(name);
}

function openImportModal() {
  if (!store.isLoggedIn) {
    store.isLoginModalOpen = true;
    return;
  }
  importInputText.value = "";
  showImportModal.value = true;
}

async function submitImportPlaylist() {
  const raw = importInputText.value.trim();
  if (!raw) return;
  showImportModal.value = false;
  await store.importExternalPlaylist(raw);
}

function openEditPlaylist(pl: Playlist) {
  editingPlaylist.value = pl;
  editTitle.value = pl.title || pl.name || "";
  editIntro.value = pl.description || pl.intro || "";
  showEditModal.value = true;
}

async function submitEditPlaylist() {
  if (!editingPlaylist.value || !editingPlaylist.value.listId) return;
  const title = editTitle.value.trim();
  if (!title) return;
  showEditModal.value = false;
  await store.updatePlaylistMeta(
    editingPlaylist.value.listId,
    title,
    editIntro.value.trim()
  );
}

async function confirmDeletePlaylist(pl: Playlist) {
  const ok = await store.askConfirm({
    title: "删除自建歌单",
    message: `确定要删除自建歌单「${pl.title || pl.name}」吗？删除后歌单内的曲目编排将无法恢复。`,
    confirmText: "确认删除",
    cancelText: "取消",
    tone: "danger",
  });
  if (ok) {
    store.deletePlaylist(pl);
  }
}

function formatFansCount(cnt: number): string {
  if (!cnt || cnt <= 0) return "概念版音乐人";
  if (cnt >= 10000) return `${(cnt / 10000).toFixed(1)}w 粉丝`;
  return `${cnt} 粉丝`;
}

function handleMineAvatarClick(event: MouseEvent) {
  if (store.isLoggedIn) {
    const triggerBtn = event.currentTarget as HTMLElement | null;
    const cardEl = (triggerBtn?.closest(".bento-identity-card") as HTMLElement | null) || null;
    store.openAccountProfile(triggerBtn, {
      anchorMode: "card-overlay",
      cardEl,
    });
  } else {
    store.isLoginModalOpen = true;
  }
}

function handleEscapeModal(e: Event) {
  if (showEditModal.value) {
    showEditModal.value = false;
    e.preventDefault();
  } else if (showImportModal.value) {
    showImportModal.value = false;
    e.preventDefault();
  } else if (showCreateModal.value) {
    showCreateModal.value = false;
    e.preventDefault();
  }
}

onMounted(() => {
  window.addEventListener("breeze:escape-modal", handleEscapeModal);
  if (store.isLoggedIn) {
    if (store.userPlaylists.length === 0) {
      store.loadUserPlaylists();
    }
    if (store.followedArtists.length === 0) {
      store.loadFollowedArtists();
    }
    if (
      store.purchasedAlbums.length === 0 &&
      store.purchasedSongs.length === 0
    ) {
      store.loadPurchasedAssets();
    }
    if (store.cloudDiskSongs.length === 0) {
      store.loadUserCloudDisk(false, true);
    }
  }
  if (store.recentHistorySongs.length === 0) {
    store.loadRecentHistory();
  }
  if (store.localMusicSongs.length === 0 && store.localMusicDir) {
    store.scanLocalMusicLibrary(undefined, true);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("breeze:escape-modal", handleEscapeModal);
});
</script>

<template>
  <!-- 方案 B：12 栅格 5:4:3 杂志感不对称 Bento 首屏 + 100% 全宽无界陈列馆 -->
  <div class="library-bento-page" @contextmenu.prevent>
    <!-- ==================== 第一层：12 栅格 5:4:3 杂志感 Bento 首屏 ==================== -->
    <section class="library-bento-hero-grid">
      <!-- 左 5 栅格：个人身份、双水位线听歌等级与内嵌概念版 VIP 权益中心 -->
      <article class="bento-card bento-identity-card">
        <div class="bento-id-top">
          <div class="bento-id-user">
            <button
              type="button"
              class="bento-avatar-wrap"
              :aria-label="
                store.isLoggedIn ? '查看个人音乐档案' : '点击登录酷狗账号'
              "
              :aria-haspopup="store.isLoggedIn ? 'dialog' : undefined"
              :aria-expanded="
                store.isLoggedIn ? store.isAccountProfileOpen : undefined
              "
              :data-tip="
                store.isLoggedIn ? '点击查看个人档案' : '点击登录酷狗账号'
              "
              data-tip-pos="bottom"
              @click="handleMineAvatarClick"
            >
              <img
                v-if="store.profile?.avatarUrl || store.session?.avatarUrl"
                :src="store.profile?.avatarUrl || store.session?.avatarUrl"
                class="bento-avatar-img"
                draggable="false"
                alt="avatar"
              />
              <div v-else class="bento-avatar-fallback">
                {{
                  (
                    store.profile?.nickname ||
                    store.session?.nickname ||
                    "微"
                  ).slice(0, 1)
                }}
              </div>
              <span class="bento-online-dot"></span>
            </button>

            <div class="bento-user-meta">
              <div class="bento-user-name-row">
                <h2 class="bento-user-name">
                  {{
                    store.isLoggedIn
                      ? store.profile?.nickname ||
                        store.session?.nickname ||
                        "微风乐迷"
                      : "登录酷狗概念版"
                  }}
                </h2>
                <span v-if="store.isLoggedIn" class="bento-lv-badge">
                  Lv.{{ levelNum }}
                </span>
                <span
                  v-if="store.isLoggedIn"
                  class="bento-vip-badge"
                  :class="{
                    active:
                      store.vipStatus?.svipActive ||
                      store.vipStatus?.tvipActive ||
                      store.profile?.isVip,
                  }"
                >
                  {{
                    store.vipStatus?.svipActive
                      ? "概念 VIP"
                      : store.vipStatus?.tvipActive || store.profile?.isVip
                      ? "VIP"
                      : "概念版"
                  }}
                </span>
              </div>
              <div
                class="bento-user-sub"
                :title="store.profile?.signature || ''"
              >
                <template v-if="store.isLoggedIn">
                  <span v-if="(store.profile?.musicAgeYears || 0) > 0">
                    乐龄 {{ store.profile?.musicAgeYears }} 年
                  </span>
                  <span
                    v-if="
                      (store.profile?.follows || 0) > 0 ||
                      (store.profile?.fans || 0) > 0
                    "
                  >
                    {{ (store.profile?.musicAgeYears || 0) > 0 ? "· " : "" }}关注
                    {{ store.profile?.follows || 0 }} · 粉丝
                    {{ store.profile?.fans || 0 }}
                  </span>
                  <span
                    v-else-if="!(store.profile?.musicAgeYears || 0)"
                  >
                    {{
                      store.profile?.signature ||
                      (store.savedAccounts.length > 1
                        ? `已保存 ${store.savedAccounts.length} 个账号`
                        : "云端乐库已同步")
                    }}
                  </span>
                </template>
                <template v-else>
                  登录后同步红心曲库、云歌单与 VIP 权益
                </template>
              </div>
            </div>
          </div>

          <div class="bento-id-actions">
            <button
              class="bento-icon-circle"
              :data-tip="store.isLoggedIn ? '账号管理与会话切换' : '立即登录'"
              title="切换账号或管理会话"
              @click="
                store.isLoggedIn
                  ? (store.isAccountManagerOpen = true)
                  : (store.isLoginModalOpen = true)
              "
            >
              <PlumpIcon name="user" :size="15" />
            </button>
            <button
              v-if="store.isLoggedIn"
              class="bento-icon-circle sync-btn"
              :disabled="store.isSyncingListeningGrade"
              data-tip="同步云端听歌时长与等级"
              title="同步云端听歌时长与等级"
              @click="store.syncListeningGradeCloud(true)"
            >
              <PlumpIcon
                name="refresh"
                :size="15"
                :class="{ 'is-spinning': store.isSyncingListeningGrade }"
              />
            </button>
          </div>
        </div>

        <!-- 中段：听歌时长大数字与等级成长值进度条 -->
        <div class="bento-listen-box" :title="gradeDetailTooltip">
          <div class="bento-listen-left">
            <span class="bento-listen-label">累计听歌</span>
            <div class="bento-listen-value">
              <strong>{{ listenHoursNumber }}</strong>
              <span class="unit">小时</span>
              <strong>{{ listenRemainderMins }}</strong>
              <span class="unit">分钟</span>
            </div>
          </div>

          <div class="bento-listen-right">
            <div class="bento-level-meta">
              <span class="bento-lv-route">
                Lv.{{ levelNum }} → Lv.{{ nextLevelNum }}
              </span>
              <span class="hint">{{ nextLevelHintText }}</span>
            </div>
            <progress
              class="bento-level-progress"
              :value="levelProgressPercent"
              max="100"
              :aria-label="`听歌等级进度 Lv.${levelNum} 至 Lv.${nextLevelNum}`"
            ></progress>
          </div>
        </div>

        <!-- 底部内嵌：概念版 VIP 权益与每日签到 -->
        <div
          class="bento-vip-strip"
          @click="store.isVipCenterOpen = true"
        >
          <div class="bento-vip-left">
            <div class="bento-vip-orb">
              <PlumpIcon name="crown" :size="16" color="currentColor" />
            </div>
            <div class="bento-vip-copy">
              <div class="bento-vip-title">概念版 VIP 权益中心</div>
              <div class="bento-vip-sub">
                {{
                  store.vipStatus?.svipExpiry
                    ? `有效期至 ${store.vipStatus.svipExpiry.slice(0, 10)}`
                    : store.vipStatus?.tvipExpiry
                    ? `有效期至 ${store.vipStatus.tvipExpiry.slice(0, 10)}`
                    : "每日签到免费领畅听 VIP 与无损音质"
                }}
              </div>
            </div>
          </div>

          <button
            class="bento-vip-cta"
            :class="{ claimed: store.vipStatus?.todayClaimed }"
            :disabled="store.isClaimingVip"
            @click.stop="
              store.vipStatus?.todayClaimed
                ? (store.isVipCenterOpen = true)
                : store.claimVipDaily(false)
            "
          >
            {{
              store.isClaimingVip
                ? "签到中..."
                : store.vipStatus?.todayClaimed
                ? "今日已签 ›"
                : "签到领 VIP"
            }}
          </button>
        </div>
      </article>

      <!-- 中 4 栅格：我喜欢的音乐 -->
      <article
        class="bento-card bento-liked-card"
        @click="store.openLikedSongsDetail()"
      >
        <div class="bento-liked-top">
          <div class="bento-liked-main">
            <div class="bento-cover-stack">
              <img
                v-if="store.likedSongs[2]?.coverUrl"
                :src="store.likedSongs[2].coverUrl"
                class="stack-img layer-3"
                draggable="false"
                alt=""
              />
              <img
                v-if="store.likedSongs[1]?.coverUrl"
                :src="store.likedSongs[1].coverUrl"
                class="stack-img layer-2"
                draggable="false"
                alt=""
              />
              <div class="stack-img layer-1">
                <img
                  v-if="store.likedSongs[0]?.coverUrl"
                  :src="store.likedSongs[0].coverUrl"
                  draggable="false"
                  alt=""
                />
                <div v-else class="stack-placeholder">
                  <PlumpIcon name="heart-filled" :size="28" color="#fff" />
                </div>
              </div>
            </div>

            <div class="bento-liked-copy">
              <div class="bento-title-row">
                <h3>我喜欢的音乐</h3>
                <span class="bento-count-badge coral">
                  {{ likedTotalCount }} 首
                </span>
              </div>
              <div class="bento-liked-stats">
                <span v-if="likedHiResCount > 0">
                  {{ likedHiResCount }} 首高解析 ·
                </span>
                <span>{{ likedDurationText }}</span>
              </div>
            </div>
          </div>

          <div class="bento-liked-actions" @click.stop>
            <button
              class="bento-icon-circle"
              data-tip="随机播放"
              title="随机播放我喜欢的音乐"
              @click.stop="shufflePlayLiked"
            >
              <PlumpIcon name="shuffle" :size="15" />
            </button>
            <button
              class="bento-play-fab coral"
              data-tip="播放全部"
              title="播放我喜欢的音乐"
              @click.stop="playAllLiked"
            >
              <PlumpIcon name="play" :size="18" color="#fff" />
            </button>
          </div>
        </div>

        <!-- 底部双曲预览胶囊 -->
        <div class="bento-liked-tracks" @click.stop>
          <template v-if="store.likedSongs.length > 0">
            <div
              v-for="song in store.likedSongs.slice(0, 2)"
              :key="song.hash"
              class="bento-mini-track"
              :class="{ playing: store.currentSong?.hash === song.hash }"
              @click.stop="
                store.playSongFromList(song, store.likedSongs, '我喜欢的音乐')
              "
            >
              <img
                v-if="song.coverUrl"
                :src="song.coverUrl"
                class="mini-track-cover"
                draggable="false"
                alt=""
              />
              <div class="mini-track-text">
                <span class="mini-track-title">{{ song.title }}</span>
                <span class="mini-track-artist">{{ song.artist }}</span>
              </div>
              <span
                v-if="resolveSongQualityBadge(song)"
                class="mini-track-tag"
                :class="resolveSongQualityBadge(song)!.tier"
              >
                {{ resolveSongQualityBadge(song)!.label }}
              </span>
              <span v-if="song.isVip" class="mini-track-tag vip">VIP</span>
            </div>
          </template>
          <div
            v-else
            class="bento-liked-empty"
            @click="store.openLikedSongsDetail()"
          >
            <span>进入红心曲库同步你的收藏单曲 ›</span>
          </div>
        </div>
      </article>

      <!-- 右 3 栅格：双层叠放（上：最近播放 / 下：心流专注舱） -->
      <div class="bento-right-stack">
        <!-- 上卡：最近播放 -->
        <article
          class="bento-card bento-recent-card"
          @click="store.openRecentHistoryDetail()"
        >
          <div class="bento-compact-main">
            <div class="bento-compact-cover blue">
              <img
                v-if="store.recentHistorySongs[0]?.coverUrl"
                :src="store.recentHistorySongs[0].coverUrl"
                draggable="false"
                alt=""
              />
              <PlumpIcon v-else name="timer" :size="22" color="#fff" />
            </div>

            <div class="bento-compact-copy">
              <div class="bento-title-row">
                <h4>最近播放</h4>
                <span class="bento-count-badge blue">
                  {{ store.recentHistorySongs.length }}
                </span>
              </div>
              <div class="bento-compact-sub">
                <template v-if="store.recentHistorySongs[0]">
                  {{ store.recentHistorySongs[0].title }} ·
                  {{ store.recentHistorySongs[0].artist }}
                </template>
                <template v-else>
                  云端与本地播放足迹
                </template>
              </div>
            </div>
          </div>

          <button
            class="bento-play-fab blue mini"
            title="播放最近听过"
            @click.stop="playAllRecent"
          >
            <PlumpIcon name="play" :size="15" color="#fff" />
          </button>
        </article>

        <!-- 下卡：心流专注舱 -->
        <article
          class="bento-card bento-focus-card"
          @click="store.isFocusPodOpen = true"
        >
          <div class="bento-compact-main">
            <div class="bento-compact-cover violet">
              <PlumpIcon name="headphones" :size="22" color="#fff" accent="#e9d5ff" />
            </div>

            <div class="bento-compact-copy">
              <div class="bento-title-row">
                <h4>心流专注舱</h4>
                <span
                  v-if="store.focusRunState === 'running' || store.focusRunState === 'paused'"
                  class="bento-count-badge"
                >
                  {{ store.focusFormattedClock }}
                </span>
              </div>
              <div class="bento-compact-sub">
                <template v-if="store.focusRunState === 'running'">
                  正在专注 · {{ store.activeFocusPreset?.title || "沉浸心流" }}
                </template>
                <template v-else-if="store.focusRunState === 'paused'">
                  已暂停 {{ store.focusFormattedClock }} · 点击继续
                </template>
                <template v-else-if="store.focusTodayTotalMinutes > 0">
                  今日已专注 {{ store.focusTodayTotalMinutes }} 分钟 · 白噪音
                </template>
                <template v-else>
                  4 通道白噪音 · 番茄钟专注
                </template>
              </div>
            </div>
          </div>

          <button
            class="bento-play-fab violet mini"
            title="开启心流专注舱"
            @click.stop="store.isFocusPodOpen = true"
          >
            <PlumpIcon name="chevron-right" :size="16" color="#fff" />
          </button>
        </article>
      </div>
    </section>

    <!-- ==================== 第一点五层：音乐云盘 & 本地音乐独立页面入口卡片 ==================== -->
    <section class="library-entrance-dock">
      <article
        class="bento-card bento-cloud-card"
        @click="store.openCloudDrivePage()"
      >
        <div class="bento-compact-main">
          <div class="bento-compact-cover cloud-cyan">
            <PlumpIcon name="music" :size="22" color="#fff" />
          </div>
          <div class="bento-compact-copy">
            <div class="bento-title-row">
              <h4>音乐云盘</h4>
              <span class="bento-count-badge cyan">
                {{ store.cloudDiskTotal || store.cloudDiskSongs.length }}
              </span>
            </div>
            <div class="bento-compact-sub">
              <template v-if="!store.isLoggedIn">
                登录酷狗账号即可跨端同步私藏云盘曲目
              </template>
              <template v-else-if="store.cloudDiskSongs[0]">
                {{ store.cloudDiskSongs[0].title }} · {{ store.cloudDiskSongs[0].artist }}
              </template>
              <template v-else>
                云端私藏曲库 · 点击进入独立云盘页
              </template>
            </div>
          </div>
        </div>
        <div class="bento-entrance-actions">
          <button
            class="bento-play-fab cyan mini"
            title="播放云盘音乐"
            @click.stop="playAllCloudDisk"
          >
            <PlumpIcon name="play" :size="15" color="#fff" />
          </button>
          <span class="bento-entrance-arrow">
            <PlumpIcon name="chevron-right" :size="16" />
          </span>
        </div>
      </article>

      <article
        class="bento-card bento-local-card"
        @click="store.openLocalMusicPage()"
      >
        <div class="bento-compact-main">
          <div class="bento-compact-cover local-jade">
            <PlumpIcon name="folder" :size="22" color="#fff" />
          </div>
          <div class="bento-compact-copy">
            <div class="bento-title-row">
              <h4>本地音乐</h4>
              <span class="bento-count-badge jade">
                {{ store.localMusicSongs.length }}
              </span>
            </div>
            <div class="bento-compact-sub">
              <template v-if="store.localMusicSongs.length > 0">
                目录「{{ localDirShortName }}」· 无损本地直通播放
              </template>
              <template v-else>
                扫描电脑本地音频文件夹 · 支持无损直通解码
              </template>
            </div>
          </div>
        </div>
        <div class="bento-entrance-actions">
          <button
            class="bento-play-fab jade mini"
            title="播放本地音乐"
            @click.stop="playAllLocalMusic"
          >
            <PlumpIcon name="play" :size="15" color="#fff" />
          </button>
          <span class="bento-entrance-arrow">
            <PlumpIcon name="chevron-right" :size="16" />
          </span>
        </div>
      </article>
    </section>

    <!-- ==================== 第二层：100% 全宽无界歌单与资产陈列馆 ==================== -->
    <section class="library-fullwidth-gallery">
      <!-- 顶部无框果冻胶囊导航栏 + 纯图标工具按钮 -->
      <header class="lib-gallery-toolbar">
        <div class="lib-toolbar-left">
          <div class="lib-sliding-tabs jelly-pill-tabs">
            <button
              v-for="tab in assetTabs"
              :key="tab.id"
              class="lib-tab-btn"
              :class="{ active: store.libraryAssetTab === tab.id }"
              @click="handleSelectAssetTab(tab.id)"
            >
              <span class="tab-name">{{ tab.label }}</span>
              <span class="tab-num">{{ tab.count }}</span>
            </button>
          </div>

          <div class="lib-filter-box">
            <PlumpIcon name="search" :size="14" />
            <input
              v-model="galleryFilterKw"
              type="text"
              placeholder="筛选歌单、歌手或已购专辑..."
            />
            <button
              v-if="galleryFilterKw"
              class="filter-clear-btn"
              data-tip="清空筛选"
              @click="galleryFilterKw = ''"
            >
              <PlumpIcon name="close" :size="12" />
            </button>
          </div>
        </div>

        <div class="lib-toolbar-right">
          <button
            class="lib-tool-icon-btn primary"
            data-tip="新建歌单"
            title="新建歌单"
            @click="openCreateModal"
          >
            <PlumpIcon name="plus" :size="16" />
          </button>
          <button
            class="lib-tool-icon-btn"
            data-tip="导入歌单 / 口令"
            title="导入歌单 / 口令"
            @click="openImportModal"
          >
            <PlumpIcon name="import" :size="16" />
          </button>
          <button
            class="lib-tool-icon-btn"
            :disabled="isRefreshingAssets"
            data-tip="刷新云端资产"
            title="刷新云端资产"
            @click="refreshLibraryAssets"
          >
            <PlumpIcon
              name="refresh"
              :size="16"
              :class="{ 'is-spinning': isRefreshingAssets }"
            />
          </button>
          <button
            class="lib-tool-icon-btn"
            :data-tip="
              store.isLibraryGridView ? '切换为双列宽幅列表' : '切换为海报画廊宫格'
            "
            :title="
              store.isLibraryGridView ? '切换为双列宽幅列表' : '切换为海报画廊宫格'
            "
            @click="store.isLibraryGridView = !store.isLibraryGridView"
          >
            <PlumpIcon
              :name="store.isLibraryGridView ? 'list' : 'grid'"
              :size="16"
            />
          </button>
        </div>
      </header>

      <transition name="tag-content-morph" mode="out-in">
      <!-- 1. 自建歌单 / 收藏歌单 画廊 -->
      <div
        v-if="
          store.libraryAssetTab === 'created' ||
          store.libraryAssetTab === 'collected'
        "
        :key="`${store.libraryAssetTab}_${store.isLibraryGridView ? 'grid' : 'list'}`"
        class="lib-gallery-body"
      >
        <div
          v-if="filteredPlaylists.length > 0"
          :class="
            store.isLibraryGridView
              ? 'lib-playlist-poster-grid'
              : 'lib-playlist-wide-list'
          "
        >
          <div
            v-for="(pl, pIdx) in filteredPlaylists"
            :key="pl.id || pl.globalCollectionId || pl.title"
            class="lib-pl-card bm-stagger-item"
            :style="{ '--bm-stagger-idx': Math.min(pIdx, 14) }"
            @click="store.openPlaylistDetail(pl)"
          >
            <div v-tilt="{ maxDeg: 3.5 }" class="lib-pl-cover-wrap">
              <img
                v-if="pl.coverUrl"
                :src="pl.coverUrl"
                class="lib-pl-cover"
                loading="lazy"
                decoding="async"
                draggable="false"
                alt=""
              />
              <div v-else class="lib-pl-cover-fallback">
                <PlumpIcon name="music" :size="30" />
              </div>
            </div>

            <div class="lib-pl-info">
              <div class="lib-pl-title" :title="pl.title || pl.name">
                {{ pl.title || pl.name }}
              </div>
              <div class="lib-pl-sub">
                <span>{{ pl.trackCount || 0 }} 首曲目</span>
                <span v-if="pl.creator"> · {{ pl.creator }}</span>
              </div>
            </div>

            <!-- 自建歌单支持快捷编辑/删除；收藏歌单支持取消收藏 -->
            <div class="lib-pl-item-actions" @click.stop>
              <template v-if="store.libraryAssetTab === 'created' && pl.listId">
                <button
                  class="pl-mini-act-btn"
                  title="编辑歌单标题与简介"
                  @click.stop="openEditPlaylist(pl)"
                >
                  <PlumpIcon name="edit" :size="13" />
                </button>
                <button
                  class="pl-mini-act-btn danger"
                  title="删除歌单"
                  @click.stop="confirmDeletePlaylist(pl)"
                >
                  <PlumpIcon name="trash" :size="13" />
                </button>
              </template>
              <template v-else-if="store.libraryAssetTab === 'collected'">
                <button
                  class="pl-mini-act-btn danger"
                  title="取消收藏此歌单"
                  @click.stop="store.toggleCollectPlaylist(pl)"
                >
                  <PlumpIcon name="trash" :size="13" />
                </button>
              </template>
            </div>
          </div>
        </div>

        <div v-else class="lib-empty-gallery">
          <PlumpIcon name="music" :size="36" />
          <p v-if="!store.isLoggedIn">
            登录酷狗概念版账号后即可查看并管理你的云端歌单
          </p>
          <p v-else-if="store.libraryAssetTab === 'created'">
            暂无自建歌单，点击右上方「+ 新建歌单」或「导入歌单」创建你的专属曲库
          </p>
          <p v-else>
            暂无收藏的外部歌单，可在发现页或搜索页点击红心收藏喜欢的歌单
          </p>
        </div>
      </div>

      <!-- 2. 关注歌手陈列架 -->
      <div
        v-else-if="store.libraryAssetTab === 'artists'"
        :key="`artists_${store.isLibraryGridView ? 'grid' : 'list'}`"
        class="lib-gallery-body"
      >
        <div
          v-if="filteredArtists.length > 0"
          :class="
            store.isLibraryGridView
              ? 'lib-artist-poster-grid'
              : 'lib-playlist-wide-list'
          "
        >
          <div
            v-for="(art, aIdx) in filteredArtists"
            :key="art.artistId || art.singerId || art.name"
            class="lib-artist-card bm-stagger-item"
            :style="{ '--bm-stagger-idx': Math.min(aIdx, 14) }"
            @click="
              store.openArtistDetail(
                String(art.singerId || art.artistId || ''),
                art.name,
                art.avatarUrl
              )
            "
          >
            <div v-tilt="{ maxDeg: 3 }" class="lib-artist-avatar-wrap">
              <img
                v-if="art.avatarUrl"
                :src="art.avatarUrl"
                class="lib-artist-avatar"
                loading="lazy"
                decoding="async"
                draggable="false"
                alt=""
              />
              <div v-else class="lib-artist-avatar fallback">
                {{ (art.name || "歌").slice(0, 1) }}
              </div>
            </div>

            <div class="lib-artist-info">
              <div class="lib-artist-name">{{ art.name }}</div>
              <div class="lib-artist-sub">
                <span v-if="art.songCount">{{ art.songCount }} 首单曲</span>
                <span v-else>{{ formatFansCount(art.fansCount) }}</span>
              </div>
            </div>

            <button
              class="lib-artist-enter-btn"
              @click.stop="
                store.openArtistDetail(
                  String(art.singerId || art.artistId || ''),
                  art.name,
                  art.avatarUrl
                )
              "
            >
              歌手主页 ›
            </button>
          </div>
        </div>

        <div v-else class="lib-empty-gallery">
          <PlumpIcon name="user" :size="36" />
          <p>暂无已关注的歌手，在搜索页或播放页点击歌手即可关注</p>
        </div>
      </div>

      <!-- 3. 已购内容（已购单曲合集 + 已购数字专辑） -->
      <div
        v-else-if="store.libraryAssetTab === 'purchased'"
        :key="`purchased_${store.isLibraryGridView ? 'grid' : 'list'}`"
        class="lib-gallery-body"
      >
        <div
          v-if="
            store.purchasedSongs.length > 0 ||
            filteredPurchasedAlbums.length > 0
          "
          :class="
            store.isLibraryGridView
              ? 'lib-playlist-poster-grid'
              : 'lib-playlist-wide-list'
          "
        >
          <!-- 已购单曲合集入口卡 -->
          <div
            v-if="store.purchasedSongs.length > 0"
            class="lib-pl-card purchased-singles-card"
            @click="store.openPurchasedSongsDetail()"
          >
            <div class="lib-pl-cover-wrap gold-wrap">
              <img
                v-if="store.purchasedSongs[0]?.song?.coverUrl"
                :src="store.purchasedSongs[0].song.coverUrl"
                class="lib-pl-cover"
                draggable="false"
                alt=""
              />
              <div v-else class="lib-pl-cover-fallback gold">
                <PlumpIcon name="crown" :size="30" color="#FFD54F" />
              </div>
              <span class="bento-pl-count-pill gold">
                {{ store.purchasedSongs.length }} 首已购
              </span>

              <div class="lib-pl-hover-mask">
                <button
                  class="lib-pl-play-btn gold"
                  title="播放全部已购单曲"
                  @click.stop="playAllPurchasedSingles"
                >
                  <PlumpIcon name="play" :size="18" color="#1a1300" />
                </button>
              </div>
            </div>

            <div class="lib-pl-info">
              <div class="lib-pl-title">已购单曲合集</div>
              <div class="lib-pl-sub">
                共 {{ store.purchasedSongs.length }} 首已付费数字单曲 · 无损畅听
              </div>
            </div>
          </div>

          <!-- 已购数字专辑卡片 -->
          <div
            v-for="alb in filteredPurchasedAlbums"
            :key="alb.id || alb.albumId || alb.albumName"
            class="lib-pl-card purchased-album-card"
            @click="
              store.openAlbumDetail(
                String(alb.albumId || alb.id || ''),
                alb.albumName,
                alb.singerName || alb.artistName || '',
                alb.coverUrl
              )
            "
          >
            <div class="lib-pl-cover-wrap">
              <img
                v-if="alb.coverUrl"
                :src="alb.coverUrl"
                class="lib-pl-cover"
                loading="lazy"
                decoding="async"
                draggable="false"
                alt=""
              />
              <div v-else class="lib-pl-cover-fallback gold">
                <PlumpIcon name="disc" :size="30" color="#FFD54F" />
              </div>
              <span class="bento-pl-count-pill gold">
                {{ alb.songCount > 0 ? `${alb.songCount} 首` : "数字专辑" }}
              </span>
            </div>

            <div class="lib-pl-info">
              <div class="lib-pl-title" :title="alb.albumName">
                {{ alb.albumName }}
              </div>
              <div class="lib-pl-sub">
                <span>{{
                  alb.singerName || alb.artistName || "数字专辑"
                }}</span>
                <span v-if="alb.buyTime">
                  · {{ alb.buyTime.slice(0, 10) }}
                </span>
              </div>
            </div>
          </div>
        </div>

        <div v-else class="lib-empty-gallery">
          <PlumpIcon name="crown" :size="36" color="#E5A93B" />
          <p>
            当前账号暂无已购数字专辑或单曲记录，点击右上角刷新按钮可重新同步云端已购资产
          </p>
        </div>
      </div>
      </transition>
    </section>

    <!-- 新建歌单弹窗 -->
    <transition name="fade">
      <div
        v-if="showCreateModal"
        class="modal-backdrop"
        @click.self="showCreateModal = false"
      >
        <div class="compact-dialog glass-popover">
          <h3>新建云端自建歌单</h3>
          <p class="dialog-sub">
            歌单将实时同步至你的酷狗概念版云端账号：
          </p>
          <input
            v-model="newPlaylistName"
            type="text"
            class="dialog-input"
            placeholder="请输入歌单名称..."
            @keyup.enter="submitCreatePlaylist"
          />
          <div class="dialog-actions">
            <button class="frameless-hover-btn" @click="showCreateModal = false">
              取消
            </button>
            <button
              class="frameless-hover-btn primary-tint"
              @click="submitCreatePlaylist"
            >
              立即创建
            </button>
          </div>
        </div>
      </div>
    </transition>

    <!-- 编辑歌单弹窗 -->
    <transition name="fade">
      <div
        v-if="showEditModal"
        class="modal-backdrop"
        @click.self="showEditModal = false"
      >
        <div class="compact-dialog glass-popover">
          <h3>编辑歌单信息</h3>
          <p class="dialog-sub">修改歌单标题与简介并同步至酷狗云端：</p>
          <input
            v-model="editTitle"
            type="text"
            class="dialog-input"
            placeholder="歌单名称"
          />
          <textarea
            v-model="editIntro"
            class="dialog-input"
            style="height: 86px; margin-top: 10px; padding: 10px 12px; resize: none"
            placeholder="歌单简介（选填）"
          ></textarea>
          <div class="dialog-actions">
            <button class="frameless-hover-btn" @click="showEditModal = false">
              取消
            </button>
            <button
              class="frameless-hover-btn primary-tint"
              @click="submitEditPlaylist"
            >
              保存修改
            </button>
          </div>
        </div>
      </div>
    </transition>

    <!-- 导入外部歌单弹窗 -->
    <transition name="fade">
      <div
        v-if="showImportModal"
        class="modal-backdrop"
        @click.self="showImportModal = false"
      >
        <div
          class="compact-dialog glass-popover"
          style="width: min(92vw, 520px)"
        >
          <h3>导入外部歌单与分享口令</h3>
          <p class="dialog-sub">
            支持粘贴酷狗分享短链 (t1.kugou.com)、歌单码、global_collection_id、微风口令或「歌名 - 歌手」文本列表：
          </p>
          <textarea
            v-model="importInputText"
            class="dialog-input"
            style="height: 110px; padding: 12px 14px; resize: none; line-height: 1.5"
            placeholder="粘贴酷狗分享链接、口令或多行「歌名 - 歌手」列表..."
          ></textarea>
          <div class="dialog-actions">
            <button class="frameless-hover-btn" @click="showImportModal = false">
              取消
            </button>
            <button
              class="frameless-hover-btn primary-tint"
              @click="submitImportPlaylist"
            >
              智能解析并导入
            </button>
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<style scoped>
.library-bento-page {
  display: flex;
  flex-direction: column;
  gap: 28px;
  padding: 6px 6px 40px;
  min-height: 100%;
  color: var(--text-main);
}

/* ==================== 第一层：12 栅格 5:4:3 杂志感 Bento 首屏 ==================== */
.library-bento-hero-grid {
  display: grid;
  grid-template-columns: repeat(12, minmax(0, 1fr));
  gap: 18px;
  align-items: stretch;
}

.bento-card {
  position: relative;
  border-radius: var(--bm-r-xl, 24px);
  padding: 20px 22px;
  background: var(--bg-card);
  border: none;
  box-shadow: var(--shadow-card);
  transition: transform 0.28s cubic-bezier(0.34, 1.56, 0.64, 1),
    box-shadow 0.28s ease;
  overflow: hidden;
}

/* 左 5 栅格：个人身份 + 听歌等级 + 内嵌 VIP 权益条 */
.bento-identity-card {
  grid-column: span 5;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 14px;
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 14%, transparent) 0%,
    color-mix(in srgb, var(--cyan) 8%, transparent) 52%,
    var(--bg-card) 100%
  );
}

.scheme-watercolor_light .bento-identity-card,
.scheme-light .bento-identity-card {
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 10%, rgba(255, 255, 255, 0.88)) 0%,
    color-mix(in srgb, var(--cyan) 6%, rgba(255, 255, 255, 0.9)) 52%,
    rgba(255, 255, 255, 0.92) 100%
  );
}

.scheme-watercolor_light.light-variant-nordic_oat .bento-identity-card,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .bento-identity-card {
  background: #fcf9f2;
  box-shadow: 0 10px 26px -12px rgba(45, 36, 22, 0.08);
}

.bento-id-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.bento-id-user {
  display: flex;
  align-items: center;
  gap: 14px;
  min-width: 0;
  cursor: default;
}

.bento-avatar-wrap {
  position: relative;
  width: 54px;
  height: 54px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  cursor: pointer;
  transition: transform 0.24s cubic-bezier(0.22, 1, 0.36, 1);
}

.bento-avatar-wrap:hover {
  transform: scale(1.045);
}

.bento-avatar-wrap:active {
  transform: scale(0.96);
}

.bento-avatar-wrap:focus-visible {
  box-shadow: 0 0 12px color-mix(in srgb, var(--primary) 42%, transparent);
}

@media (prefers-reduced-motion: reduce) {
  .bento-avatar-wrap {
    transition: none;
  }

  .bento-avatar-wrap:hover,
  .bento-avatar-wrap:active {
    transform: none;
  }
}

.bento-avatar-img,
.bento-avatar-fallback {
  width: 54px;
  height: 54px;
  border-radius: 50%;
  object-fit: cover;
  display: block;
  border: none;
  box-shadow: 0 8px 18px color-mix(in srgb, var(--primary) 26%, transparent);
}

.bento-avatar-fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--primary-soft);
  color: var(--primary);
  font-size: 20px;
  font-weight: var(--bm-fw-heavy, 800);
}

.bento-online-dot {
  position: absolute;
  right: 1px;
  bottom: 1px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: var(--accent-mint, #10b981);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.28);
}

.bento-user-meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
  cursor: default;
}

.bento-user-name-row {
  display: flex;
  align-items: center;
  gap: 7px;
  flex-wrap: wrap;
}

.bento-user-name {
  margin: 0;
  font-size: 17.5px;
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-main);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 170px;
}

.bento-lv-badge {
  padding: 2px 8px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--accent-mint);
  background: var(--accent-mint-soft);
  border: none;
}

.bento-vip-badge {
  padding: 2px 8px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-sub);
  background: var(--bg-glass-hover);
}

.bento-vip-badge.active {
  color: var(--accent-gold);
  background: var(--accent-gold-soft);
  border: none;
}

.bento-user-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-sub);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bento-id-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.bento-icon-circle.sync-btn {
  color: var(--primary);
  background: var(--primary-soft);
}

/* 听歌时长与等级进度框 */
.bento-listen-box {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 12px 16px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bg-glass);
  border: none;
}

.scheme-watercolor_light .bento-listen-box,
.scheme-light .bento-listen-box {
  background: rgba(255, 255, 255, 0.82);
}

.bento-listen-left {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-right: 12px;
  border: none;
  flex-shrink: 0;
}

.bento-listen-label {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-sub);
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.bento-listen-mins {
  font-style: normal;
  font-weight: var(--bm-fw-semibold, 600);
  font-size: var(--bm-fs-micro, 11px);
  color: var(--text-muted);
}

.bento-listen-value {
  display: flex;
  align-items: baseline;
  gap: 4px;
  color: var(--text-main);
}

.bento-listen-value strong {
  font-size: 22px;
  font-weight: var(--bm-fw-heavy, 800);
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.02em;
  color: var(--primary);
}

.bento-listen-value .unit {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-sub);
  margin-right: 4px;
}

.bento-listen-right {
  flex: 1;
  min-width: 170px;
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.bento-level-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--accent-mint);
}

.bento-lv-route {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  white-space: nowrap;
}

.bento-speed-pill {
  font-style: normal;
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-heavy, 800);
  padding: 2px 6px;
  border-radius: var(--bm-r-pill, 999px);
  color: var(--primary);
  background: var(--primary-soft);
}

.bento-level-meta .hint {
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-sub);
  white-space: nowrap;
}

.bento-level-progress {
  appearance: none;
  border: none;
  overflow: clip;
  width: 100%;
  height: 7px;
  --progress-bar-radius: 999px;
  border-radius: var(--progress-bar-radius);
  background: rgba(120, 145, 185, 0.22);
}

.bento-level-progress::-webkit-progress-bar {
  background: transparent;
}

.bento-level-progress::-webkit-progress-value {
  background: var(--grad-kugou-primary, linear-gradient(90deg, var(--primary) 0%, var(--primary-light, #818cf8) 100%));
  border-radius: var(--progress-bar-radius);
  transition: inline-size 0.3s;
}

.bento-level-progress::-moz-progress-bar {
  background: var(--grad-kugou-primary, linear-gradient(90deg, var(--primary) 0%, var(--primary-light, #818cf8) 100%));
  border-radius: var(--progress-bar-radius);
  transition: inline-size 0.3s;
}

/* 内嵌概念版 VIP 黑金/香槟金权益条 */
.bento-vip-strip {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 14px;
  border-radius: var(--bm-r-lg, 18px);
  background: linear-gradient(
    135deg,
    rgba(255, 213, 79, 0.16) 0%,
    rgba(229, 169, 59, 0.08) 100%
  );
  border: none;
  cursor: pointer;
  transition: transform 0.22s ease, box-shadow 0.22s ease;
}

.scheme-watercolor_light .bento-vip-strip,
.scheme-light .bento-vip-strip {
  background: linear-gradient(135deg, #fef7e0 0%, #fdecc4 100%);
}

.bento-vip-strip:hover {
  transform: translateY(-1px);
  box-shadow: 0 8px 20px rgba(245, 158, 11, 0.15);
}

.bento-vip-left {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.bento-vip-orb {
  width: 32px;
  height: 32px;
  border-radius: var(--bm-r-sm, 10px);
  background: rgba(255, 213, 79, 0.2);
  color: #ffd54f;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.scheme-watercolor_light .bento-vip-orb,
.scheme-light .bento-vip-orb {
  background: rgba(217, 119, 6, 0.16);
  color: #b45309;
}

.bento-vip-copy {
  min-width: 0;
}

.bento-vip-title {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--accent-gold);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.bento-vip-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-sub);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.scheme-watercolor_light .bento-vip-sub,
.scheme-light .bento-vip-sub {
  color: #78350f;
}

.bento-vip-cta {
  padding: 6px 13px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-heavy, 800);
  background: linear-gradient(135deg, #ffd54f, #e5a93b);
  color: #1f1600;
  border: none;
  cursor: pointer;
  flex-shrink: 0;
  transition: transform 0.2s ease;
}

.bento-vip-cta.claimed {
  background: var(--accent-gold-soft);
  color: var(--accent-gold);
  border: none;
}

.scheme-watercolor_light .bento-vip-cta.claimed,
.scheme-light .bento-vip-cta.claimed {
  background: rgba(255, 255, 255, 0.88);
  color: #92400e;
}

/* 中 4 栅格：我喜欢的音乐（珊瑚粉水彩 / 暗夜瑰红渐变主视觉卡） */
.bento-liked-card {
  grid-column: span 4;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 14px;
  cursor: pointer;
  background: linear-gradient(
    145deg,
    rgba(255, 94, 126, 0.2) 0%,
    rgba(236, 72, 153, 0.1) 55%,
    var(--bg-card) 100%
  );
}

.scheme-watercolor_light .bento-liked-card,
.scheme-light .bento-liked-card {
  background: linear-gradient(
    145deg,
    #ffecf1 0%,
    #fce7f3 48%,
    rgba(255, 255, 255, 0.96) 100%
  );
}

.bento-liked-card:hover {
  transform: translateY(-3px);
  box-shadow: 0 18px 40px -12px rgba(244, 63, 94, 0.24);
}

.bento-liked-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.bento-liked-main {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 0;
}

.bento-cover-stack {
  position: relative;
  width: 90px;
  height: 70px;
  flex-shrink: 0;
}

.stack-img {
  position: absolute;
  top: 2px;
  left: 0;
  width: 66px;
  height: 66px;
  border-radius: var(--bm-r-md, 14px);
  object-fit: cover;
  border: none;
  box-shadow: 0 8px 18px rgba(15, 23, 42, 0.22);
  transition: transform 0.28s cubic-bezier(0.22, 1, 0.36, 1);
}

.stack-img.layer-3 {
  z-index: 1;
  transform: translate(22px, 4px) rotate(8deg) scale(0.84);
  opacity: 0.65;
}

.stack-img.layer-2 {
  z-index: 2;
  transform: translate(11px, 2px) rotate(4deg) scale(0.92);
  opacity: 0.85;
}

.stack-img.layer-1 {
  z-index: 3;
  position: relative;
  overflow: hidden;
  background: linear-gradient(135deg, #ff5e7e, #e11d48);
}

.bento-liked-card:hover .stack-img.layer-3 {
  transform: translate(26px, 4px) rotate(11deg) scale(0.85);
}

.bento-liked-card:hover .stack-img.layer-2 {
  transform: translate(13px, 2px) rotate(5.5deg) scale(0.93);
}

.stack-img.layer-1 img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.stack-placeholder {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.stack-heart-badge {
  position: absolute;
  right: 5px;
  bottom: 5px;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: #e11d48;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
}

.bento-liked-copy {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.bento-eyebrow {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-heavy, 800);
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.bento-eyebrow.coral {
  color: var(--accent-coral);
}

.bento-eyebrow.blue {
  color: var(--primary);
}

.bento-eyebrow.violet {
  color: var(--accent-violet);
}

.bento-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.bento-title-row h3 {
  margin: 0;
  font-size: 18.5px;
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-main);
}

.bento-title-row h4 {
  margin: 0;
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-main);
}

.bento-count-badge {
  padding: 2px 8px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-heavy, 800);
  font-variant-numeric: tabular-nums;
}

.bento-count-badge.coral {
  background: var(--accent-coral-soft);
  color: var(--accent-coral);
}

.bento-count-badge.blue {
  background: var(--primary-soft);
  color: var(--primary);
}

.bento-count-badge.violet {
  background: var(--accent-violet-soft);
  color: var(--accent-violet);
}

.bento-liked-stats {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-sub);
}

.bento-liked-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.bento-icon-circle {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: var(--bg-glass);
  border: none;
  color: var(--text-main);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: transform 0.22s cubic-bezier(0.34, 1.56, 0.64, 1),
    background 0.2s ease, color 0.2s ease;
}

.scheme-watercolor_light .bento-icon-circle,
.scheme-light .bento-icon-circle {
  background: rgba(255, 255, 255, 0.88);
}

.bento-icon-circle:hover {
  color: var(--primary);
  background: var(--primary-soft);
  transform: scale(1.06);
}

.bento-play-fab {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: transform 0.24s cubic-bezier(0.34, 1.56, 0.64, 1),
    box-shadow 0.24s ease;
  flex-shrink: 0;
}

.bento-play-fab:hover {
  transform: scale(1.08);
}

.bento-play-fab.coral {
  background: linear-gradient(135deg, #ff5e7e, #e11d48);
  box-shadow: 0 8px 20px rgba(225, 29, 72, 0.34);
}

.bento-play-fab.blue {
  background: var(--grad-kugou-primary, linear-gradient(135deg, var(--primary), var(--primary-light, #818cf8)));
  box-shadow: 0 6px 16px color-mix(in srgb, var(--primary) 32%, transparent);
}

.bento-play-fab.violet {
  background: linear-gradient(135deg, #8b5cf6, #6d28d9);
  box-shadow: 0 6px 16px rgba(109, 40, 217, 0.3);
}

.bento-play-fab.mini {
  width: 38px;
  height: 38px;
}

.bento-liked-tracks {
  display: flex;
  flex-direction: column;
  gap: 7px;
}

.bento-mini-track {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 11px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--bg-glass);
  border: none;
  cursor: pointer;
  transition: all 0.2s ease;
}

.scheme-watercolor_light .bento-mini-track,
.scheme-light .bento-mini-track {
  background: rgba(255, 255, 255, 0.8);
}

.bento-mini-track:hover,
.bento-mini-track.playing {
  background: var(--accent-coral-soft);
}

.mini-track-cover {
  width: 32px;
  height: 32px;
  border-radius: var(--bm-r-sm, 10px);
  object-fit: cover;
  flex-shrink: 0;
}

.mini-track-text {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: baseline;
  gap: 7px;
}

.mini-track-title {
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  color: var(--text-main);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mini-track-artist {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-sub);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.mini-track-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-heavy, 800);
  padding: 2px 7px;
  border-radius: var(--bm-r-xs, 6px);
  flex-shrink: 0;
}

.mini-track-tag.hires {
  background: var(--accent-gold-soft);
  color: var(--accent-gold);
}

.mini-track-tag.sq {
  background: var(--accent-mint-soft);
  color: var(--accent-mint);
}

.bento-liked-empty {
  padding: 14px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--bg-glass);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-sub);
  text-align: center;
}

/* 右 3 栅格：上下叠放双胶囊卡 */
.bento-right-stack {
  grid-column: span 3;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.bento-recent-card,
.bento-focus-card {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 16px 18px;
  cursor: pointer;
}

.bento-recent-card {
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 16%, transparent) 0%,
    color-mix(in srgb, var(--bm-accent-sky, var(--primary-light)) 8%, transparent) 62%,
    var(--bg-card) 100%
  );
}

.scheme-watercolor_light .bento-recent-card,
.scheme-light .bento-recent-card {
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--primary) 12%, var(--bg-card)) 0%,
    color-mix(in srgb, var(--bm-accent-sky, var(--primary-light)) 8%, var(--bg-card)) 55%,
    var(--bg-card) 100%
  );
}

.bento-recent-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 14px 32px -10px color-mix(in srgb, var(--primary) 24%, transparent);
}

.bento-focus-card {
  background: linear-gradient(
    145deg,
    rgba(139, 92, 246, 0.18) 0%,
    rgba(108, 92, 231, 0.09) 62%,
    var(--bg-card) 100%
  );
}

.scheme-watercolor_light .bento-focus-card,
.scheme-light .bento-focus-card {
  background: linear-gradient(
    145deg,
    color-mix(in srgb, var(--accent-violet, #7c3aed) 14%, rgba(255, 253, 249, 0.72)) 0%,
    color-mix(in srgb, var(--accent-cyan, #0284c7) 10%, rgba(255, 253, 249, 0.68)) 55%,
    var(--bg-card, rgba(255, 253, 249, 0.7)) 100%
  );
}

.scheme-watercolor_light.light-variant-nordic_oat .bento-focus-card {
  background: linear-gradient(145deg, #EBE5F3 0%, #F2EDE6 58%, #FCF9F2 100%);
}

.bento-focus-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 14px 32px -10px rgba(139, 92, 246, 0.22);
}

.bento-compact-main {
  display: flex;
  align-items: center;
  gap: 13px;
  min-width: 0;
}

.bento-compact-cover {
  width: 50px;
  height: 50px;
  border-radius: var(--bm-r-md, 14px);
  overflow: hidden;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  box-shadow: 0 6px 14px rgba(15, 23, 42, 0.18);
}

.bento-compact-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.bento-compact-cover.blue {
  background: var(--grad-kugou-primary, linear-gradient(135deg, var(--primary), var(--primary-light, #818cf8)));
}

.bento-compact-cover.violet {
  background: linear-gradient(135deg, #8b5cf6, #6d28d9);
}

.bento-compact-copy {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.bento-compact-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-sub);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ==================== 第二层：100% 全宽无界陈列馆 ==================== */
.library-fullwidth-gallery {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.lib-gallery-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 6px 4px 10px;
  border: none;
  flex-wrap: wrap;
}

.lib-toolbar-left {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
}

.lib-sliding-tabs {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--bg-glass);
}

.scheme-watercolor_light .lib-sliding-tabs,
.scheme-light .lib-sliding-tabs {
  background: rgba(15, 29, 51, 0.055);
  box-shadow: inset 0 1px 2px rgba(15, 29, 51, 0.04);
}

.lib-tab-btn {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 7px 15px;
  border-radius: var(--bm-r-pill, 999px);
  background: transparent;
  border: none;
  color: var(--text-sub);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-bold, 700);
  cursor: pointer;
  transition: color 0.2s cubic-bezier(0.22, 1, 0.36, 1),
    background 0.22s cubic-bezier(0.22, 1, 0.36, 1);
}

.lib-tab-btn:hover {
  color: var(--text-main);
}

.lib-tab-btn.active {
  color: #ffffff;
  font-weight: var(--bm-fw-heavy, 800);
  background: var(--grad-kugou-primary, linear-gradient(135deg, var(--primary) 0%, var(--primary-light, #818cf8) 100%));
  box-shadow: 0 6px 16px -4px color-mix(in srgb, var(--primary) 36%, transparent);
}

.scheme-watercolor_light .lib-tab-btn,
.scheme-light .lib-tab-btn {
  color: #364966;
}

.scheme-watercolor_light .lib-tab-btn.active,
.scheme-light .lib-tab-btn.active {
  color: #ffffff;
  background: linear-gradient(135deg, #0066cc 0%, #0099ff 100%);
  box-shadow: 0 6px 16px -4px rgba(0, 102, 204, 0.36);
}

.lib-tab-btn .tab-num {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-heavy, 800);
  padding: 1px 7px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(128, 150, 190, 0.16);
  border: none;
  color: inherit;
  transition: background 0.22s ease, color 0.22s ease;
}

.scheme-watercolor_light .lib-tab-btn .tab-num,
.scheme-light .lib-tab-btn .tab-num {
  background: rgba(15, 29, 51, 0.08);
  color: #364966;
}

.lib-tab-btn.active .tab-num,
.scheme-watercolor_light .lib-tab-btn.active .tab-num,
.scheme-light .lib-tab-btn.active .tab-num {
  color: #ffffff;
  background: rgba(255, 255, 255, 0.24);
}

.lib-filter-box {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 36px;
  padding: 0 14px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--bg-glass);
  border: none;
  color: var(--text-sub);
  width: 245px;
  transition: background 0.2s ease, box-shadow 0.2s ease;
}

.lib-filter-box:focus-within {
  background: var(--bg-elevated);
  box-shadow: 0 6px 18px -6px color-mix(in srgb, var(--primary) 22%, transparent);
}

.lib-filter-box input {
  border: none;
  background: none;
  outline: none;
  color: var(--text-main);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-semibold, 600);
  width: 100%;
}

.lib-filter-box input::placeholder {
  color: var(--text-muted);
}

.filter-clear-btn {
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
}

.lib-toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.lib-tool-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 34px;
  padding: 0 14px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  background: var(--bg-glass);
  color: var(--text-main);
  border: none;
  cursor: pointer;
  transition: all 0.2s ease;
}

.lib-tool-btn:hover {
  color: var(--primary);
  background: var(--primary-soft);
}

.lib-tool-btn.primary {
  background: var(--grad-kugou-primary);
  color: #fff;
  box-shadow: 0 6px 16px -4px color-mix(in srgb, var(--primary) 34%, transparent);
}

.lib-tool-icon-btn {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: var(--bg-glass);
  border: none;
  color: var(--text-main);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: transform 0.24s cubic-bezier(0.34, 1.56, 0.64, 1),
    background 0.2s ease, color 0.2s ease, box-shadow 0.2s ease;
}

.lib-tool-icon-btn:hover:not(:disabled) {
  color: var(--primary);
  background: var(--primary-soft);
  transform: translateY(-2px);
}

.lib-tool-icon-btn.primary {
  background: var(--grad-kugou-primary);
  color: #ffffff;
  box-shadow: 0 6px 18px -4px color-mix(in srgb, var(--primary) 36%, transparent);
}

.lib-tool-icon-btn.primary:hover:not(:disabled) {
  color: #ffffff;
  background: var(--grad-kugou-primary);
  box-shadow: 0 10px 24px -4px color-mix(in srgb, var(--primary) 46%, transparent);
}

/* 海报宫格（100% 无框去卡片化呼吸排版）与双列宽幅列表 */
.lib-playlist-poster-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(195px, 1fr));
  gap: 24px 20px;
}

.lib-playlist-wide-list {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 12px;
}

.lib-pl-card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 4px;
  border-radius: var(--bm-r-lg, 18px);
  background: transparent;
  border: none;
  box-shadow: none;
  cursor: pointer;
  transition: transform 0.24s cubic-bezier(0.22, 1, 0.36, 1),
    background 0.22s ease;
}

.lib-playlist-wide-list .lib-pl-card {
  flex-direction: row;
  align-items: center;
  gap: 14px;
  padding: 8px 12px;
  border-radius: var(--bm-r-lg, 18px);
}

.lib-playlist-wide-list .lib-pl-card:hover {
  background: var(--song-hover-mist);
  transform: translateX(2px);
}

.lib-playlist-poster-grid .lib-pl-card:hover {
  transform: translateY(-4px);
}

.lib-pl-cover-wrap {
  position: relative;
  width: 100%;
  aspect-ratio: 1 / 1;
  border-radius: var(--bm-r-lg, 18px);
  overflow: hidden;
  background: var(--bg-glass);
  flex-shrink: 0;
  box-shadow: 0 12px 28px -8px rgba(0, 0, 0, 0.14);
  transition: box-shadow 0.28s ease;
}

.lib-playlist-poster-grid .lib-pl-card:hover .lib-pl-cover-wrap {
  box-shadow: 0 20px 38px -10px color-mix(in srgb, var(--primary) 24%, transparent);
}

.lib-playlist-wide-list .lib-pl-cover-wrap {
  width: 64px;
  height: 64px;
  border-radius: var(--bm-r-md, 14px);
}

.lib-pl-cover {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.35s ease;
}

.lib-pl-card:hover .lib-pl-cover {
  transform: scale(1.05);
}

.lib-pl-cover-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--primary);
  background: var(--primary-soft);
}

.lib-pl-cover-fallback.gold {
  background: var(--accent-gold-soft);
}

/* 专属防冲突右上角曲目数胶囊 */
.bento-pl-count-pill {
  position: absolute;
  top: 9px;
  right: 9px;
  bottom: auto;
  left: auto;
  width: auto;
  height: auto;
  display: inline-flex;
  align-items: center;
  padding: 3px 9px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-micro, 11px);
  font-weight: var(--bm-fw-heavy, 800);
  line-height: 1.25;
  color: #ffffff;
  background: rgba(10, 15, 26, 0.68);
  border: none;
  backdrop-filter: blur(8px);
  z-index: 2;
  pointer-events: none;
}

.lib-playlist-wide-list .bento-pl-count-pill {
  display: none;
}

.bento-pl-count-pill.gold {
  color: #ffd54f;
}

.lib-pl-hover-mask {
  position: absolute;
  inset: 0;
  background: linear-gradient(
    180deg,
    rgba(10, 14, 23, 0.02) 40%,
    rgba(10, 14, 23, 0.38) 100%
  );
  display: flex;
  align-items: flex-end;
  justify-content: flex-end;
  padding: 10px;
  opacity: 0;
  transition: opacity 0.22s ease;
  z-index: 3;
}

.lib-pl-card:hover .lib-pl-hover-mask {
  opacity: 1;
}

.lib-pl-play-btn {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  border: none;
  background: var(--grad-kugou-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  box-shadow: 0 8px 18px color-mix(in srgb, var(--primary) 36%, transparent);
  transition: transform 0.22s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.lib-pl-play-btn:hover {
  transform: scale(1.1);
}

.lib-pl-play-btn.gold {
  background: linear-gradient(135deg, #ffd54f, #e5a93b);
}

.lib-pl-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 4px;
  padding: 0 2px;
}

.lib-pl-title {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: var(--bm-fw-heavy, 800);
  line-height: 1.38;
  color: var(--text-main);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}

.lib-playlist-wide-list .lib-pl-title {
  -webkit-line-clamp: 1;
}

.lib-pl-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-sub);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.lib-pl-item-actions {
  position: absolute;
  top: 13px;
  left: 13px;
  display: flex;
  gap: 6px;
  opacity: 0;
  transition: opacity 0.2s ease;
  z-index: 4;
}

.lib-playlist-wide-list .lib-pl-item-actions {
  position: static;
}

.lib-pl-card:hover .lib-pl-item-actions {
  opacity: 1;
}

.pl-mini-act-btn {
  width: 29px;
  height: 29px;
  border-radius: 50%;
  border: none;
  background: rgba(10, 15, 26, 0.72);
  color: #fff;
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.18s ease;
}

.pl-mini-act-btn:hover {
  background: var(--primary);
}

.pl-mini-act-btn.danger:hover {
  background: var(--accent-coral);
}

/* 关注歌手（100% 去卡片化圆形悬浮陈列） */
.lib-artist-poster-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
  gap: 24px 18px;
}

.lib-artist-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 10px;
  padding: 12px 8px;
  border-radius: var(--bm-r-lg, 18px);
  background: transparent;
  border: none;
  box-shadow: none;
  cursor: pointer;
  transition: transform 0.26s cubic-bezier(0.34, 1.56, 0.64, 1),
    background 0.2s ease;
}

.lib-playlist-wide-list .lib-artist-card {
  flex-direction: row;
  text-align: left;
  padding: 10px 14px;
}

.lib-artist-card:hover {
  transform: translateY(-5px);
}

.lib-playlist-wide-list .lib-artist-card:hover {
  background: var(--song-hover-mist);
  transform: translateX(3px);
}

.lib-artist-avatar-wrap {
  width: 96px;
  height: 96px;
  border-radius: 50%;
  overflow: hidden;
  border: none;
  box-shadow: 0 12px 26px -6px rgba(0, 0, 0, 0.15);
  flex-shrink: 0;
  transition: box-shadow 0.26s ease, transform 0.26s ease;
}

.lib-artist-card:hover .lib-artist-avatar-wrap {
  box-shadow: 0 16px 32px -6px color-mix(in srgb, var(--primary) 26%, transparent);
}

.lib-playlist-wide-list .lib-artist-avatar-wrap {
  width: 56px;
  height: 56px;
}

.lib-artist-avatar {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.lib-artist-avatar.fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--accent-mint-soft);
  color: var(--accent-mint);
  font-size: 24px;
  font-weight: var(--bm-fw-heavy, 800);
}

.lib-artist-info {
  flex: 1;
  min-width: 0;
}

.lib-artist-name {
  font-size: var(--bm-fs-song-title, 15px);
  font-weight: var(--bm-fw-heavy, 800);
  color: var(--text-main);
}

.lib-artist-sub {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-semibold, 600);
  color: var(--text-sub);
  margin-top: 2px;
}

.lib-artist-enter-btn {
  padding: 5px 13px;
  border-radius: var(--bm-r-pill, 999px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: var(--bm-fw-bold, 700);
  background: var(--accent-mint-soft);
  color: var(--accent-mint);
  border: none;
  cursor: pointer;
}

.lib-empty-gallery {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  padding: 56px 20px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bg-glass);
  border: none;
  color: var(--text-sub);
  font-size: var(--bm-fs-meta, 13.5px);
  font-weight: var(--bm-fw-semibold, 600);
}

.spin-anim {
  animation: libSpin 0.9s linear infinite;
}

@keyframes libSpin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 1360px) {
  .bento-identity-card {
    grid-column: span 7;
  }
  .bento-liked-card {
    grid-column: span 5;
  }
  .bento-right-stack {
    grid-column: span 12;
    flex-direction: row;
  }
}

@media (max-width: 960px) {
  .bento-identity-card {
    grid-column: span 12;
  }
  .bento-liked-card {
    grid-column: span 12;
  }
  .bento-right-stack {
    grid-column: span 12;
    flex-direction: row;
  }
}

@media (max-width: 680px) {
  .bento-right-stack {
    flex-direction: column;
  }
}

/* ==================== 音乐云盘 & 本地音乐独立页面入口 Bento 栏 ==================== */
.library-entrance-dock {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}

.bento-cloud-card,
.bento-local-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 15px 18px;
  cursor: pointer;
}

.bento-cloud-card {
  background: linear-gradient(
    135deg,
    rgba(6, 182, 212, 0.16) 0%,
    rgba(56, 189, 248, 0.08) 62%,
    var(--bg-card) 100%
  );
}

.bento-local-card {
  background: linear-gradient(
    135deg,
    rgba(16, 185, 129, 0.16) 0%,
    rgba(56, 189, 248, 0.08) 62%,
    var(--bg-card) 100%
  );
}

.bento-cloud-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 14px 32px -10px rgba(6, 182, 212, 0.24);
}

.bento-local-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 14px 32px -10px rgba(16, 185, 129, 0.24);
}

.bento-compact-cover.cloud-cyan {
  background: linear-gradient(135deg, #06b6d4, #0284c7);
}

.bento-compact-cover.local-jade {
  background: linear-gradient(135deg, #10b981, #059669);
}

.bento-count-badge.cyan {
  background: rgba(6, 182, 212, 0.18);
  color: #38bdf8;
}

.bento-count-badge.jade {
  background: rgba(16, 185, 129, 0.18);
  color: #34d399;
}

.bento-entrance-actions {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.bento-play-fab.cyan {
  background: linear-gradient(135deg, #06b6d4, #0284c7);
  box-shadow: 0 6px 16px -4px rgba(6, 182, 212, 0.45);
}

.bento-play-fab.jade {
  background: linear-gradient(135deg, #10b981, #059669);
  box-shadow: 0 6px 16px -4px rgba(16, 185, 129, 0.45);
}

.bento-entrance-arrow {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  transition: transform 0.2s ease, color 0.2s ease;
}

.bento-cloud-card:hover .bento-entrance-arrow,
.bento-local-card:hover .bento-entrance-arrow {
  transform: translateX(3px);
  color: var(--text-main);
}

.scheme-watercolor_light .bento-cloud-card,
.scheme-light .bento-cloud-card {
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--accent-cyan, #0284c7) 15%, rgba(255, 253, 249, 0.74)) 0%,
    color-mix(in srgb, var(--accent-cyan, #0284c7) 8%, rgba(255, 253, 249, 0.7)) 58%,
    var(--bg-card, rgba(255, 253, 249, 0.72)) 100%
  );
  border: none;
}

.scheme-watercolor_light.light-variant-nordic_oat .bento-cloud-card {
  background: linear-gradient(135deg, #E2ECF2 0%, #EEF2F0 58%, #FCF9F2 100%);
}

.scheme-watercolor_light .bento-local-card,
.scheme-light .bento-local-card {
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--accent-mint, #0d9488) 15%, rgba(255, 253, 249, 0.74)) 0%,
    color-mix(in srgb, var(--accent-mint, #0d9488) 8%, rgba(255, 253, 249, 0.7)) 58%,
    var(--bg-card, rgba(255, 253, 249, 0.72)) 100%
  );
  border: none;
}

.scheme-watercolor_light.light-variant-nordic_oat .bento-local-card {
  background: linear-gradient(135deg, #E1EFE9 0%, #EEF2EC 58%, #FCF9F2 100%);
}

.scheme-watercolor_light .bento-count-badge.cyan,
.scheme-light .bento-count-badge.cyan {
  background: var(--accent-cyan-soft);
  color: var(--accent-cyan);
}

.scheme-watercolor_light .bento-count-badge.jade,
.scheme-light .bento-count-badge.jade {
  background: var(--accent-mint-soft);
  color: var(--accent-mint);
}

.scheme-watercolor_light .lib-filter-box,
.scheme-light .lib-filter-box,
.scheme-watercolor_light .lib-tool-icon-btn:not(.primary),
.scheme-light .lib-tool-icon-btn:not(.primary) {
  background: var(--bg-pill, rgba(255, 253, 249, 0.78));
  border: none;
  color: var(--text-primary);
}

.scheme-watercolor_light.light-variant-nordic_oat .lib-filter-box,
.scheme-watercolor_light.light-variant-nordic_oat .lib-tool-icon-btn:not(.primary) {
  background: #E4DDD0;
}

.scheme-watercolor_light .lib-empty-gallery,
.scheme-light .lib-empty-gallery {
  background: var(--bg-card, rgba(255, 253, 249, 0.72));
  border: none;
  color: var(--text-secondary);
}

.scheme-watercolor_light.light-variant-nordic_oat .lib-empty-gallery {
  background: #FCF9F2;
}

@media (max-width: 820px) {
  .library-entrance-dock {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>

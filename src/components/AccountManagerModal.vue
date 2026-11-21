<script setup lang="ts">
import { computed, ref } from "vue";
import { useAppStore } from "../stores/appStore";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();

const currentUid = computed(() => store.session?.userId || store.profile?.userId || "");
const isRefreshingSession = ref(false);

function openAddAccount() {
  store.isAccountManagerOpen = false;
  store.isLoginModalOpen = true;
}

async function handleRefreshSession() {
  if (!store.isLoggedIn || isRefreshingSession.value) return;
  isRefreshingSession.value = true;
  try {
    await store.refreshAccountAndVip(true);
  } finally {
    isRefreshingSession.value = false;
  }
}

async function handleLogout() {
  await store.logout();
  store.isAccountManagerOpen = false;
}
</script>

<template>
  <transition name="fade">
    <div
      v-if="store.isAccountManagerOpen"
      class="modal-backdrop"
      @click.self="store.isAccountManagerOpen = false"
    >
      <div class="vip-modal-card account-mgr-modal">
        <div class="modal-header">
          <div class="vip-title-row">
            <div class="acct-mgr-orb">
              <PlumpIcon name="user" :size="20" color="var(--bm-accent-azure, #38bdf8)" />
            </div>
            <div>
              <h3 class="acct-modal-title">酷狗概念版 · 账号与会话管理</h3>
              <p class="vip-sub-caption acct-modal-sub">
                多账号持久化会话切换、添加新账号、刷新 Token 凭证与退出登录
              </p>
            </div>
          </div>
          <button
            class="icon-close-btn acct-close-btn"
            aria-label="关闭账号管理"
            @click="store.isAccountManagerOpen = false"
          >
            <PlumpIcon name="close" :size="16" />
          </button>
        </div>

        <!-- 已保存的多账号会话列表 -->
        <div class="acct-saved-section">
          <div class="acct-section-head">
            <span>本地已保存的酷狗账号 ({{ store.savedAccounts.length }})</span>
          </div>

          <div v-if="store.savedAccounts.length > 0" class="acct-saved-list">
            <div
              v-for="acc in store.savedAccounts"
              :key="acc.userId"
              class="acct-saved-item"
              :class="{ current: acc.userId === currentUid }"
            >
              <div class="acct-saved-left">
                <img
                  v-if="acc.avatarUrl"
                  :src="acc.avatarUrl"
                  class="acct-saved-avatar"
                  alt=""
                />
                <div v-else class="acct-saved-avatar acct-avatar-fallback">
                  {{ (acc.nickname || "酷").slice(0, 1) }}
                </div>
                <div class="acct-saved-meta">
                  <div class="acct-saved-name-row">
                    <span class="acct-saved-name">{{ acc.nickname || `用户 ${acc.userId}` }}</span>
                    <span v-if="acc.userId === currentUid" class="acct-active-tag">在线使用中</span>
                  </div>
                  <div class="acct-saved-uid">UID: {{ acc.userId }}</div>
                </div>
              </div>

              <div class="acct-saved-btns">
                <button
                  v-if="acc.userId !== currentUid"
                  class="acct-switch-btn"
                  @click="store.switchAccount(acc.userId)"
                >
                  切换账号
                </button>
                <button
                  v-else
                  class="acct-refresh-btn"
                  :disabled="isRefreshingSession"
                  @click="handleRefreshSession"
                >
                  <PlumpIcon
                    name="refresh"
                    :size="13"
                    :class="{ 'is-spinning': isRefreshingSession }"
                  />
                  <span>{{ isRefreshingSession ? "刷新中..." : "刷新 Token" }}</span>
                </button>
                <button
                  class="acct-remove-btn"
                  title="从本地移除此账号凭证"
                  @click="store.removeSavedAccount(acc.userId)"
                >
                  移除
                </button>
              </div>
            </div>
          </div>
          <div v-else class="acct-empty-tip">
            当前暂无已保存的账号会话，点击下方按钮即可扫码或手机号添加酷狗账号。
          </div>
        </div>

        <!-- 底部操作栏 -->
        <div class="acct-modal-footer">
          <div class="acct-footer-left">
            <button class="acct-footer-btn add-btn" @click="openAddAccount">
              <PlumpIcon name="plus" :size="15" />
              <span>扫码 / 手机号登录新账号</span>
            </button>
          </div>
          <button
            v-if="store.isLoggedIn"
            class="acct-footer-btn danger-btn"
            @click="handleLogout"
          >
            <span>退出当前登录</span>
          </button>
        </div>
      </div>
    </div>
  </transition>
</template>

<style scoped>
.account-mgr-modal {
  width: min(620px, 92vw);
  max-height: 84vh;
  display: flex;
  flex-direction: column;
  gap: 18px;
  padding: 24px 26px;
  background: rgba(22, 22, 28, 0.98);
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  border: none;
  border-radius: var(--bm-r-xl, 24px);
  color: #f8fafc;
  box-shadow: 0 28px 64px rgba(0, 0, 0, 0.56);
}

[data-theme="abyssal_dark"][data-dark-variant="velvet_nebula"] .account-mgr-modal {
  background: rgba(26, 20, 32, 0.98);
}

[data-theme="watercolor_light"] .account-mgr-modal,
.scheme-watercolor_light .account-mgr-modal {
  background: rgba(255, 255, 255, 0.98);
  border: none;
  color: #181a20;
  box-shadow: 0 24px 60px rgba(24, 26, 32, 0.14);
}

[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .account-mgr-modal {
  background: #fcf9f2;
  color: #221f1a;
  box-shadow: 0 24px 60px -12px rgba(45, 38, 28, 0.16);
}

.acct-modal-title {
  margin: 0;
  font-size: var(--bm-fs-subhead, 16.5px);
  font-weight: 800;
  color: #f8fafc;
}

[data-theme="watercolor_light"] .acct-modal-title,
.scheme-watercolor_light .acct-modal-title {
  color: var(--text-primary);
}

.acct-modal-sub {
  margin: 2px 0 0;
  font-size: var(--bm-fs-caption, 12.5px);
  color: rgba(226, 232, 240, 0.76);
}

[data-theme="watercolor_light"] .acct-modal-sub,
.scheme-watercolor_light .acct-modal-sub {
  color: #475569;
}

.acct-mgr-orb {
  width: 40px;
  height: 40px;
  border-radius: var(--bm-r-md, 14px);
  background: rgba(56, 189, 248, 0.14);
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

[data-theme="watercolor_light"] .acct-mgr-orb,
.scheme-watercolor_light .acct-mgr-orb {
  background: rgba(2, 132, 199, 0.1);
  border: none;
}

.acct-close-btn {
  border: none;
  background: rgba(255, 255, 255, 0.06);
  color: #e2e8f0;
}

[data-theme="watercolor_light"] .acct-close-btn,
.scheme-watercolor_light .acct-close-btn {
  background: #f1f5f9;
  border: none;
  color: #334155;
}

.acct-avatar-fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(56, 189, 248, 0.18);
  color: #38bdf8;
  font-weight: 800;
  font-size: var(--bm-fs-subhead, 16.5px);
}

[data-theme="watercolor_light"] .acct-avatar-fallback,
.scheme-watercolor_light .acct-avatar-fallback {
  background: #e0f2fe;
  color: #0284c7;
}

.acct-saved-section {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.acct-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: rgba(226, 232, 240, 0.82);
}

[data-theme="watercolor_light"] .acct-section-head,
.scheme-watercolor_light .acct-section-head {
  color: #334155;
}

.acct-saved-list {
  display: flex;
  flex-direction: column;
  gap: 9px;
  max-height: 290px;
  overflow-y: auto;
}

.acct-saved-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px;
  border-radius: var(--bm-r-md, 14px);
  background: rgba(255, 255, 255, 0.045);
  border: none;
  transition: all 0.2s ease;
}

.acct-saved-item.current {
  background: linear-gradient(
    135deg,
    rgba(56, 189, 248, 0.14),
    rgba(45, 212, 191, 0.08)
  );
  border: none;
}

[data-theme="watercolor_light"] .acct-saved-item,
.scheme-watercolor_light .acct-saved-item {
  background: #f8fafc;
  border: none;
}

[data-theme="watercolor_light"] .acct-saved-item.current,
.scheme-watercolor_light .acct-saved-item.current {
  background: linear-gradient(135deg, #f0f9ff 0%, #f0fdfa 100%);
  border: none;
}

.acct-saved-left {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.acct-saved-avatar {
  width: 40px;
  height: 40px;
  border-radius: 50%;
  object-fit: cover;
  flex-shrink: 0;
}

.acct-saved-meta {
  min-width: 0;
}

.acct-saved-name-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.acct-saved-name {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: 700;
  color: #f8fafc;
}

[data-theme="watercolor_light"] .acct-saved-name,
.scheme-watercolor_light .acct-saved-name {
  color: var(--text-primary);
}

.acct-active-tag {
  font-size: var(--bm-fs-micro, 11px);
  font-weight: 800;
  padding: 2px 8px;
  border-radius: var(--bm-r-pill, 999px);
  background: rgba(45, 212, 191, 0.16);
  color: #2dd4bf;
  border: none;
}

[data-theme="watercolor_light"] .acct-active-tag,
.scheme-watercolor_light .acct-active-tag {
  background: rgba(13, 148, 136, 0.12);
  color: #0f766e;
  border: none;
}

.acct-saved-uid {
  font-size: var(--bm-fs-caption, 12.5px);
  color: rgba(148, 163, 184, 0.85);
  margin-top: 2px;
}

[data-theme="watercolor_light"] .acct-saved-uid,
.scheme-watercolor_light .acct-saved-uid {
  color: #64748b;
}

.acct-saved-btns {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}

.acct-switch-btn {
  padding: 6px 13px;
  border-radius: var(--bm-r-sm, 10px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  background: #0284c7;
  color: #ffffff;
  border: none;
  cursor: pointer;
  transition: transform 0.16s ease, background 0.16s ease;
}

.acct-switch-btn:hover {
  background: #0369a1;
  transform: translateY(-1px);
}

.acct-refresh-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 6px 11px;
  border-radius: var(--bm-r-sm, 10px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  background: rgba(56, 189, 248, 0.14);
  color: #38bdf8;
  border: none;
  cursor: pointer;
  transition: background 0.16s ease;
}

.acct-refresh-btn:hover:not(:disabled) {
  background: rgba(56, 189, 248, 0.24);
}

.acct-refresh-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

[data-theme="watercolor_light"] .acct-refresh-btn,
.scheme-watercolor_light .acct-refresh-btn {
  background: rgba(2, 132, 199, 0.1);
  color: #0369a1;
  border: none;
}

.acct-remove-btn {
  padding: 6px 11px;
  border-radius: var(--bm-r-sm, 10px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 600;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(226, 232, 240, 0.78);
  border: none;
  cursor: pointer;
  transition: all 0.16s ease;
}

[data-theme="watercolor_light"] .acct-remove-btn,
.scheme-watercolor_light .acct-remove-btn {
  background: #ffffff;
  color: #475569;
  border: none;
}

.acct-remove-btn:hover {
  color: #fb7185;
  background: rgba(244, 63, 94, 0.14);
  border: none;
}

[data-theme="watercolor_light"] .acct-remove-btn:hover,
.scheme-watercolor_light .acct-remove-btn:hover {
  color: #be123c;
  background: rgba(244, 63, 94, 0.1);
  border: none;
}

.acct-empty-tip {
  padding: 24px;
  text-align: center;
  font-size: var(--bm-fs-meta, 13.5px);
  color: rgba(148, 163, 184, 0.85);
  border-radius: var(--bm-r-md, 14px);
  background: rgba(255, 255, 255, 0.04);
  border: none;
}

[data-theme="watercolor_light"] .acct-empty-tip,
.scheme-watercolor_light .acct-empty-tip {
  background: #f8fafc;
  border: none;
  color: #475569;
}

.acct-modal-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
  padding-top: 4px;
}

.acct-footer-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.acct-footer-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  padding: 9px 15px;
  border-radius: var(--bm-r-sm, 10px);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  cursor: pointer;
  border: none;
  background: rgba(255, 255, 255, 0.06);
  color: #f8fafc;
  transition: all 0.18s ease;
}

.acct-footer-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

[data-theme="watercolor_light"] .acct-footer-btn,
.scheme-watercolor_light .acct-footer-btn {
  background: #f8fafc;
  border: none;
  color: var(--text-primary);
}

.acct-footer-btn.add-btn:hover {
  background: rgba(56, 189, 248, 0.18);
  border: none;
  color: #38bdf8;
}

[data-theme="watercolor_light"] .acct-footer-btn.add-btn:hover,
.scheme-watercolor_light .acct-footer-btn.add-btn:hover {
  background: #e0f2fe;
  border: none;
  color: #0369a1;
}

.acct-footer-btn.danger-btn {
  color: #fda4af;
  background: rgba(244, 63, 94, 0.14);
  border: none;
}

.acct-footer-btn.danger-btn:hover {
  background: var(--coral);
  color: #ffffff;
}

[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .acct-saved-item,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .acct-empty-tip,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .acct-footer-btn:not(.danger-btn),
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .acct-close-btn,
[data-theme="watercolor_light"][data-light-variant="nordic_oat"] .acct-remove-btn {
  background: #e4ddd0;
  color: #221f1a;
}

[data-theme="watercolor_light"] .acct-footer-btn.danger-btn,
.scheme-watercolor_light .acct-footer-btn.danger-btn {
  color: #be123c;
  background: rgba(244, 63, 94, 0.09);
  border: none;
}

[data-theme="watercolor_light"] .acct-footer-btn.danger-btn:hover,
.scheme-watercolor_light .acct-footer-btn.danger-btn:hover {
  background: #e11d48;
  color: #ffffff;
}
</style>

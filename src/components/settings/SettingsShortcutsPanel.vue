<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import { useAppStore } from "../../stores/appStore";
import type {
  GlobalShortcutActionId,
  InAppShortcutActionId,
} from "../../types";
import {
  GLOBAL_SHORTCUT_ITEMS,
  IN_APP_SHORTCUT_ITEMS,
  formatAcceleratorKeycaps,
  recordKeyboardEventToAccelerator,
} from "../../utils/shortcuts";
import PlumpIcon from "../PlumpIcon.vue";
import SettingsCard from "./SettingsCard.vue";
import SettingsRow from "./SettingsRow.vue";
import SettingsSwitch from "./SettingsSwitch.vue";

const props = defineProps<{
  highlightedId?: string;
}>();

const store = useAppStore();
const activeTab = ref<"global" | "inApp">("global");

watch(
  () => props.highlightedId,
  (id) => {
    if (id === "shortcuts-inapp-list") activeTab.value = "inApp";
    else if (id === "shortcuts-global-toggle" || id === "shortcuts-global-list") {
      activeTab.value = "global";
    }
  },
  { immediate: true }
);

type RecordingTarget =
  | { scope: "global"; action: GlobalShortcutActionId }
  | { scope: "inApp"; action: InAppShortcutActionId }
  | null;

const recordingTarget = ref<RecordingTarget>(null);
const recordingHint = ref<string>("");

function isRecording(scope: "global" | "inApp", action: string): boolean {
  return (
    recordingTarget.value?.scope === scope &&
    recordingTarget.value?.action === action
  );
}

function stopRecording() {
  recordingTarget.value = null;
  recordingHint.value = "";
  store.isRecordingShortcut = false;
  window.removeEventListener("keydown", handleRecorderKeyDown, true);
}

function startRecording(
  scope: "global" | "inApp",
  action: GlobalShortcutActionId | InAppShortcutActionId
) {
  if (isRecording(scope, action)) {
    stopRecording();
    return;
  }
  stopRecording();
  recordingTarget.value =
    scope === "global"
      ? { scope: "global", action: action as GlobalShortcutActionId }
      : { scope: "inApp", action: action as InAppShortcutActionId };
  recordingHint.value = "请按下新组合键（Esc 取消 · Backspace 清空）";
  store.isRecordingShortcut = true;
  window.addEventListener("keydown", handleRecorderKeyDown, true);
}

async function handleRecorderKeyDown(e: KeyboardEvent) {
  const target = recordingTarget.value;
  if (!target) return;

  e.preventDefault();
  e.stopPropagation();

  // Esc 取消本次录制
  if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) {
    stopRecording();
    return;
  }

  // 单独按下 Backspace / Delete 清空解绑
  if (
    (e.key === "Backspace" || e.key === "Delete") &&
    !e.ctrlKey &&
    !e.altKey &&
    !e.shiftKey &&
    !e.metaKey
  ) {
    if (target.scope === "global") {
      await store.setGlobalHotkey(target.action, "");
    } else {
      store.setInAppHotkey(target.action, "");
    }
    stopRecording();
    return;
  }

  const res = recordKeyboardEventToAccelerator(e, {
    isGlobal: target.scope === "global",
  });
  if (res === null) {
    // 仍在等待主键（仅按下了修饰键）
    return;
  }
  if (!res.ok) {
    recordingHint.value = res.reason;
    return;
  }

  if (target.scope === "global") {
    await store.setGlobalHotkey(target.action, res.accelerator);
  } else {
    store.setInAppHotkey(target.action, res.accelerator);
  }
  stopRecording();
}

async function clearSingleBinding(
  scope: "global" | "inApp",
  action: GlobalShortcutActionId | InAppShortcutActionId
) {
  if (isRecording(scope, action)) {
    stopRecording();
  }
  if (scope === "global") {
    await store.setGlobalHotkey(action as GlobalShortcutActionId, "");
  } else {
    store.setInAppHotkey(action as InAppShortcutActionId, "");
  }
}

async function handleResetAll() {
  stopRecording();
  const ok = await store.askConfirm({
    title: "恢复全部默认快捷键",
    message: "将系统级全局热键与应用内快捷键全部恢复为微风音乐出厂默认键位，是否继续？",
    confirmText: "恢复默认",
    cancelText: "取消",
  });
  if (ok) {
    await store.resetAllHotkeys();
  }
}

onBeforeUnmount(() => {
  stopRecording();
});
</script>

<template>
  <div class="bms-panel-stack">
    <div class="bms-subtabs" role="tablist" aria-label="快捷键分区">
      <button
        type="button"
        role="tab"
        class="bms-subtab-btn"
        :class="{ active: activeTab === 'global' }"
        :aria-selected="activeTab === 'global'"
        @click="activeTab = 'global'"
      >
        <PlumpIcon name="sliders" :size="14" :color="activeTab === 'global' ? '#ffffff' : 'var(--coral)'" />
        <span>系统全局热键</span>
        <span v-if="store.osGlobalHotkeysEnabled" class="bms-subtab-count">已启用</span>
      </button>
      <button
        type="button"
        role="tab"
        class="bms-subtab-btn"
        :class="{ active: activeTab === 'inApp' }"
        :aria-selected="activeTab === 'inApp'"
        @click="activeTab = 'inApp'"
      >
        <PlumpIcon name="sparkles" :size="14" :color="activeTab === 'inApp' ? '#ffffff' : 'var(--primary)'" />
        <span>应用内快捷键</span>
      </button>
    </div>

    <!-- 1. 系统级全局热键总控 -->
    <SettingsCard
      v-if="activeTab === 'global'"
      title="系统级后台全局热键"
      subtitle="基于 Windows 原生 RegisterHotKey 驱动，应用最小化至托盘或全屏游戏时依然随时响应"
      icon="sliders"
      icon-color="var(--coral)"
    >
      <template #headerRight>
        <button type="button" class="bms-btn sm ghost" @click="handleResetAll">
          <PlumpIcon name="reset-rotate" :size="12" />
          <span>恢复全部默认</span>
        </button>
      </template>

      <SettingsRow
        title="启用系统级全局热键与硬件媒体键"
        description="关闭后仅保留窗口聚焦时的应用内快捷键，自动释放全部 Windows 系统热键占用"
        icon="power-dsp"
        icon-color="var(--coral)"
        :badge="store.osGlobalHotkeysEnabled ? '后台监听中' : '已暂停'"
        :badge-tone="store.osGlobalHotkeysEnabled ? 'mint' : 'muted'"
        label-for="bms-sw-global-hk"
        setting-id="shortcuts-global-toggle"
        :highlighted="highlightedId === 'shortcuts-global-toggle'"
      >
        <SettingsSwitch
          id="bms-sw-global-hk"
          v-model="store.osGlobalHotkeysEnabled"
          aria-label="启用系统级全局热键"
        />
      </SettingsRow>

      <!-- 录制态全局引导横幅 -->
      <div v-if="recordingTarget" class="bms-recording-banner" role="status" aria-live="polite">
        <span class="bms-rec-pulse" aria-hidden="true"></span>
        <span>{{ recordingHint }}</span>
        <button type="button" class="bms-btn sm ghost" @click="stopRecording">
          取消录制 (Esc)
        </button>
      </div>

      <!-- 自定义全局热键列表 -->
      <div
        class="bms-hk-list"
        :class="{
          'is-dimmed': !store.osGlobalHotkeysEnabled,
          'is-flash-highlight': highlightedId === 'shortcuts-global-list',
        }"
        data-setting-id="shortcuts-global-list"
      >
        <div
          v-for="item in GLOBAL_SHORTCUT_ITEMS"
          :key="item.action"
          class="bms-hk-row"
          :class="{
            'is-recording': isRecording('global', item.action),
            'is-conflict':
              store.osGlobalHotkeysEnabled &&
              store.globalHotkeyFailedActions.includes(item.action),
          }"
        >
          <div class="bms-hk-copy">
            <div class="bms-hk-title-line">
              <span class="bms-hk-label">{{ item.label }}</span>
              <span
                v-if="
                  store.osGlobalHotkeysEnabled &&
                  store.globalHotkeyFailedActions.includes(item.action)
                "
                class="bms-chip coral"
              >
                已被其他软件占用
              </span>
            </div>
            <span class="bms-hk-desc">{{ item.description }}</span>
          </div>

          <div class="bms-hk-actions">
            <button
              type="button"
              class="bms-keycap-recorder"
              :class="{
                'is-recording': isRecording('global', item.action),
                'is-unbound': !store.globalHotkeys[item.action],
              }"
              :aria-label="`设置全局热键：${item.label}`"
              @click="startRecording('global', item.action)"
            >
              <template v-if="isRecording('global', item.action)">
                <span class="bms-rec-text">请按下组合键…</span>
              </template>
              <template v-else-if="store.globalHotkeys[item.action]">
                <kbd
                  v-for="(cap, idx) in formatAcceleratorKeycaps(store.globalHotkeys[item.action])"
                  :key="`${item.action}-${idx}`"
                  class="bms-kbd"
                >
                  {{ cap }}
                </kbd>
              </template>
              <template v-else>
                <span class="bms-unbound-text">点击录制</span>
              </template>
            </button>

            <button
              v-if="store.globalHotkeys[item.action]"
              type="button"
              class="bms-hk-clear-btn"
              :aria-label="`清空全局热键：${item.label}`"
              data-tip="解除绑定"
              @click="clearSingleBinding('global', item.action)"
            >
              <PlumpIcon name="close" :size="12" />
            </button>
          </div>
        </div>
      </div>

      <div class="bms-media-keys-note">
        <PlumpIcon name="headphones" :size="15" color="var(--mint)" />
        <span>
          <strong>硬件多媒体键自动支持：</strong>
          启用全局热键后，键盘与蓝牙耳机自带的「播放/暂停」「上一曲」「下一曲」多媒体实体键会自动接管，无需手动绑定。
        </span>
      </div>
    </SettingsCard>

    <!-- 2. 应用内快捷键自定义 -->
    <SettingsCard
      v-if="activeTab === 'inApp'"
      title="应用内快捷键"
      subtitle="在微风音乐主窗口获得焦点时生效（文本输入框内自动避让，不影响打字）；点击键帽即可改键"
      icon="sparkles"
      icon-color="var(--primary)"
    >
      <template #headerRight>
        <button type="button" class="bms-btn sm ghost" @click="store.resetInAppHotkeys()">
          <PlumpIcon name="reset-rotate" :size="12" />
          <span>仅重置应用内</span>
        </button>
      </template>

      <div
        class="bms-hk-list"
        :class="{ 'is-flash-highlight': highlightedId === 'shortcuts-inapp-list' }"
        data-setting-id="shortcuts-inapp-list"
      >
        <div
          v-for="item in IN_APP_SHORTCUT_ITEMS"
          :key="item.action"
          class="bms-hk-row"
          :class="{ 'is-recording': isRecording('inApp', item.action) }"
        >
          <div class="bms-hk-copy">
            <span class="bms-hk-label">{{ item.label }}</span>
            <span class="bms-hk-desc">{{ item.description }}</span>
          </div>

          <div class="bms-hk-actions">
            <button
              type="button"
              class="bms-keycap-recorder"
              :class="{
                'is-recording': isRecording('inApp', item.action),
                'is-unbound': !store.inAppHotkeys[item.action],
              }"
              :aria-label="`设置应用内快捷键：${item.label}`"
              @click="startRecording('inApp', item.action)"
            >
              <template v-if="isRecording('inApp', item.action)">
                <span class="bms-rec-text">请按下快捷键…</span>
              </template>
              <template v-else-if="store.inAppHotkeys[item.action]">
                <kbd
                  v-for="(cap, idx) in formatAcceleratorKeycaps(store.inAppHotkeys[item.action])"
                  :key="`${item.action}-${idx}`"
                  class="bms-kbd"
                >
                  {{ cap }}
                </kbd>
              </template>
              <template v-else>
                <span class="bms-unbound-text">点击录制</span>
              </template>
            </button>

            <button
              v-if="store.inAppHotkeys[item.action]"
              type="button"
              class="bms-hk-clear-btn"
              :aria-label="`清空应用内快捷键：${item.label}`"
              data-tip="解除绑定"
              @click="clearSingleBinding('inApp', item.action)"
            >
              <PlumpIcon name="close" :size="12" />
            </button>
          </div>
        </div>
      </div>

      <div class="bms-media-keys-note">
        <PlumpIcon name="back" :size="15" color="var(--cyan)" />
        <span>
          <strong>内置固定交互：</strong>
          <kbd class="bms-kbd inline">Esc</kbd> 按优先级逐层收起弹窗与全屏播放页；鼠标侧键（后退键）随时触发返回上一页。
        </span>
      </div>
    </SettingsCard>
  </div>
</template>

<style scoped>
.bms-recording-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 11px 15px;
  border-radius: var(--bm-r-md, 14px);
  background: color-mix(in srgb, var(--primary) 16%, var(--bg-surface));
  color: var(--text-primary);
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
}

.bms-recording-banner > span:nth-child(2) {
  flex: 1;
}

.bms-rec-pulse {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  background: var(--coral);
  box-shadow: 0 0 10px var(--coral);
  animation: bmsRecPulse 1.1s ease-in-out infinite;
}

@keyframes bmsRecPulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.45; transform: scale(0.75); }
}

.bms-hk-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  border-radius: var(--bm-r-lg, 18px);
  transition: opacity 0.2s ease, box-shadow 0.25s ease;
}

.bms-hk-list.is-dimmed {
  opacity: 0.58;
}

.bms-hk-list.is-flash-highlight {
  box-shadow: 0 0 28px -2px color-mix(in srgb, var(--primary) 48%, transparent);
}

.bms-hk-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  padding: 12px 15px;
  border-radius: var(--bm-r-lg, 18px);
  background: var(--bg-surface);
  transition: background 0.2s ease, box-shadow 0.2s ease;
}

.bms-hk-row.is-recording {
  background: color-mix(in srgb, var(--primary) 14%, var(--bg-surface));
  box-shadow: 0 10px 24px -10px color-mix(in srgb, var(--primary) 42%, transparent);
}

.bms-hk-row.is-conflict {
  background: color-mix(in srgb, var(--coral) 10%, var(--bg-surface));
}

.bms-hk-copy {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.bms-hk-title-line {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.bms-hk-label {
  font-size: var(--bm-fs-body, 14.5px);
  font-weight: 800;
  color: var(--text-primary);
}

.bms-hk-desc {
  font-size: var(--bm-fs-caption, 12.5px);
  color: var(--text-secondary);
}

.bms-hk-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.bms-keycap-recorder {
  min-width: 132px;
  min-height: 36px;
  padding: 5px 12px;
  border: none;
  border-radius: var(--bm-r-md, 14px);
  background: var(--bg-card);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  cursor: pointer;
  transition: background 0.2s ease, transform 0.2s ease, box-shadow 0.2s ease;
}

.bms-keycap-recorder:hover {
  background: var(--bg-card-hover);
  transform: translateY(-1px);
}

.bms-keycap-recorder:focus-visible,
.bms-hk-clear-btn:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}

.bms-keycap-recorder.is-recording {
  background: var(--grad-kugou-primary);
  color: #ffffff;
  box-shadow: 0 8px 20px -6px color-mix(in srgb, var(--primary) 54%, transparent);
}

.bms-rec-text {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 800;
  color: #ffffff;
}

.bms-unbound-text {
  font-size: var(--bm-fs-caption, 12.5px);
  font-weight: 700;
  color: var(--text-muted);
}

.bms-kbd {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 26px;
  height: 24px;
  padding: 0 8px;
  border-radius: 7px;
  background: var(--primary-soft);
  color: var(--primary);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
  font-weight: 800;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.12);
}

.bms-kbd.inline {
  height: 20px;
  padding: 0 6px;
  margin: 0 2px;
  vertical-align: middle;
}

.scheme-abyssal_dark .bms-kbd {
  color: var(--primary-light);
  background: color-mix(in srgb, var(--primary) 24%, transparent);
}

.bms-hk-clear-btn {
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 50%;
  background: var(--bg-pill);
  color: var(--text-secondary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.bms-hk-clear-btn:hover {
  background: var(--accent-coral-soft);
  color: var(--coral);
}

.bms-media-keys-note {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 15px;
  border-radius: var(--bm-r-md, 14px);
  background: var(--bg-surface);
  font-size: var(--bm-fs-caption, 12.5px);
  line-height: 1.55;
  color: var(--text-secondary);
}

.bms-media-keys-note strong {
  color: var(--text-primary);
}

.scheme-watercolor_light.light-variant-nordic_oat .bms-hk-row,
.scheme-watercolor_light.light-variant-nordic_oat .bms-media-keys-note {
  background: #F2ECE1;
}

.scheme-watercolor_light.light-variant-nordic_oat .bms-keycap-recorder {
  background: #FAF6EE;
}

.scheme-abyssal_dark.dark-variant-velvet_nebula .bms-hk-row,
.scheme-abyssal_dark.dark-variant-velvet_nebula .bms-media-keys-note {
  background: rgba(34, 20, 29, 0.82);
}

@media (max-width: 680px) {
  .bms-hk-row {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>

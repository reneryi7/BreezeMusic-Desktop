<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from "vue";
import { useAppStore } from "../stores/appStore";
import {
  INPUT_CONTEXT_MENU_EVENT,
  type InputContextMenuDetail,
} from "../utils/desktopNativeGuard";

const store = useAppStore();

const state = reactive<{
  visible: boolean;
  x: number;
  y: number;
  target: HTMLInputElement | HTMLTextAreaElement | HTMLElement | null;
  hasSelection: boolean;
  isReadOnly: boolean;
  hasText: boolean;
}>({
  visible: false,
  x: 0,
  y: 0,
  target: null,
  hasSelection: false,
  isReadOnly: false,
  hasText: false,
});

const menuRef = ref<HTMLElement | null>(null);

const menuStyle = computed(() => {
  const vw = typeof window !== "undefined" ? window.innerWidth : 1280;
  const vh = typeof window !== "undefined" ? window.innerHeight : 800;
  const menuW = 164;
  const menuH = 164;
  const flipRight = state.x > vw - menuW - 8;
  const flipBottom = state.y > vh - menuH - 8;
  const left = Math.max(8, Math.min(state.x, vw - menuW - 8));
  const top = Math.max(8, Math.min(state.y, vh - menuH - 8));
  return {
    left: `${left}px`,
    top: `${top}px`,
    transformOrigin: `${flipBottom ? "bottom" : "top"} ${flipRight ? "right" : "left"}`,
  };
});

function closeMenu() {
  state.visible = false;
  state.target = null;
}

function getSelectedText(el: HTMLInputElement | HTMLTextAreaElement | HTMLElement | null): string {
  if (!el) return "";
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    const start = el.selectionStart ?? 0;
    const end = el.selectionEnd ?? 0;
    return end > start ? el.value.slice(start, end) : "";
  }
  return window.getSelection()?.toString() || "";
}

function onOpenInputContextMenu(e: Event) {
  const detail = (e as CustomEvent<InputContextMenuDetail>).detail;
  if (!detail?.target) return;
  const el = detail.target;
  el.focus();

  const isTextInput =
    el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement;
  const selected = getSelectedText(el);
  const fullVal = isTextInput ? el.value : el.textContent || "";

  state.target = el;
  state.x = detail.x;
  state.y = detail.y;
  state.hasSelection = selected.length > 0;
  state.hasText = fullVal.length > 0;
  state.isReadOnly = isTextInput ? Boolean(el.readOnly || el.disabled) : !el.isContentEditable;
  state.visible = true;
}

async function handleCopy() {
  const text = getSelectedText(state.target);
  if (text) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      document.execCommand("copy");
    }
  }
  closeMenu();
}

async function handleCut() {
  const el = state.target;
  if (!el || state.isReadOnly) {
    closeMenu();
    return;
  }
  const text = getSelectedText(el);
  if (text) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      document.execCommand("cut");
      closeMenu();
      return;
    }
    if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
      const start = el.selectionStart ?? 0;
      const end = el.selectionEnd ?? 0;
      el.setRangeText("", start, end, "end");
      el.dispatchEvent(new Event("input", { bubbles: true }));
    } else {
      document.execCommand("delete");
    }
  }
  closeMenu();
}

async function handlePaste() {
  const el = state.target;
  if (!el || state.isReadOnly) {
    closeMenu();
    return;
  }
  el.focus();
  try {
    const clip = await navigator.clipboard.readText();
    if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
      const start = el.selectionStart ?? el.value.length;
      const end = el.selectionEnd ?? el.value.length;
      el.setRangeText(clip, start, end, "end");
      el.dispatchEvent(new Event("input", { bubbles: true }));
    } else {
      document.execCommand("insertText", false, clip);
    }
  } catch {
    document.execCommand("paste");
  }
  closeMenu();
}

function handleSelectAll() {
  const el = state.target;
  if (!el) {
    closeMenu();
    return;
  }
  el.focus();
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    el.select();
  } else {
    document.execCommand("selectAll");
  }
  closeMenu();
}

function onGlobalPointerDown(e: MouseEvent) {
  if (!state.visible) return;
  if (menuRef.value && menuRef.value.contains(e.target as Node)) return;
  closeMenu();
}

function onGlobalKeyDown(e: KeyboardEvent) {
  if (state.visible && e.key === "Escape") {
    closeMenu();
  }
}

onMounted(() => {
  window.addEventListener(INPUT_CONTEXT_MENU_EVENT, onOpenInputContextMenu);
  window.addEventListener("pointerdown", onGlobalPointerDown, true);
  window.addEventListener("keydown", onGlobalKeyDown, true);
  window.addEventListener("blur", closeMenu);
});

onBeforeUnmount(() => {
  window.removeEventListener(INPUT_CONTEXT_MENU_EVENT, onOpenInputContextMenu);
  window.removeEventListener("pointerdown", onGlobalPointerDown, true);
  window.removeEventListener("keydown", onGlobalKeyDown, true);
  window.removeEventListener("blur", closeMenu);
});
</script>

<template>
  <Teleport to="body">
    <transition name="bm-ctx-spring">
      <div
        v-if="state.visible"
        ref="menuRef"
        class="bm-input-context-menu"
        :class="[
          `scheme-${store.resolvedColorScheme}`,
          store.resolvedColorScheme === 'watercolor_light'
            ? `light-variant-${store.lightThemeVariant}`
            : `dark-variant-${store.darkThemeVariant}`,
        ]"
        :style="menuStyle"
        role="menu"
        @contextmenu.prevent
      >
        <button
          type="button"
          class="bm-icm-item"
          role="menuitem"
          :disabled="!state.hasSelection || state.isReadOnly"
          @mousedown.prevent="handleCut"
        >
          <span>剪切</span>
          <kbd>Ctrl+X</kbd>
        </button>
        <button
          type="button"
          class="bm-icm-item"
          role="menuitem"
          :disabled="!state.hasSelection"
          @mousedown.prevent="handleCopy"
        >
          <span>复制</span>
          <kbd>Ctrl+C</kbd>
        </button>
        <button
          type="button"
          class="bm-icm-item"
          role="menuitem"
          :disabled="state.isReadOnly"
          @mousedown.prevent="handlePaste"
        >
          <span>粘贴</span>
          <kbd>Ctrl+V</kbd>
        </button>
        <button
          type="button"
          class="bm-icm-item"
          role="menuitem"
          :disabled="!state.hasText"
          @mousedown.prevent="handleSelectAll"
        >
          <span>全选</span>
          <kbd>Ctrl+A</kbd>
        </button>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.bm-input-context-menu {
  position: fixed;
  z-index: 12000;
  width: 158px;
  padding: 6px;
  border: none;
  border-radius: 14px;
  background: color-mix(in srgb, var(--bg-elevated, #141924) 94%, transparent);
  backdrop-filter: blur(24px) saturate(160%);
  -webkit-backdrop-filter: blur(24px) saturate(160%);
  box-shadow:
    0 16px 40px rgba(0, 0, 0, 0.38),
    0 3px 10px rgba(0, 0, 0, 0.2);
  display: flex;
  flex-direction: column;
  gap: 2px;
  user-select: none;
}

.bm-input-context-menu.scheme-abyssal_dark.dark-variant-velvet_nebula {
  background: #23181f;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
}

.bm-input-context-menu.scheme-watercolor_light {
  background: rgba(252, 253, 255, 0.96);
  box-shadow:
    0 16px 36px rgba(15, 23, 42, 0.14),
    0 3px 10px rgba(15, 23, 42, 0.06);
}

.bm-input-context-menu.scheme-watercolor_light.light-variant-nordic_oat {
  background: #faf6ee;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow:
    0 16px 36px rgba(68, 56, 38, 0.16),
    0 3px 10px rgba(68, 56, 38, 0.07);
}

.bm-icm-item {
  border: none;
  background: transparent;
  color: var(--text-primary);
  border-radius: 9px;
  padding: 7px 10px;
  font-size: 13px;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  transition: background 0.15s ease, color 0.15s ease;
}

.bm-icm-item kbd {
  font-family: inherit;
  font-size: 11px;
  font-weight: 500;
  color: var(--text-muted);
}

.bm-icm-item:hover:not(:disabled) {
  background: color-mix(in srgb, var(--primary) 16%, transparent);
  color: var(--primary);
}

.bm-icm-item:hover:not(:disabled) kbd {
  color: var(--primary);
}

.bm-icm-item:disabled {
  opacity: 0.38;
  cursor: default;
}

.bm-icm-pop-enter-active,
.bm-icm-pop-leave-active {
  transition: opacity 0.14s ease, transform 0.16s cubic-bezier(0.22, 1, 0.36, 1);
  transform-origin: top left;
}

.bm-icm-pop-enter-from,
.bm-icm-pop-leave-to {
  opacity: 0;
  transform: scale(0.94) translateY(-4px);
}
</style>

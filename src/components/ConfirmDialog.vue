<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { useAppStore } from "../stores/appStore";
import PlumpIcon from "./PlumpIcon.vue";

const store = useAppStore();
const dialogRef = ref<HTMLDialogElement | null>(null);
const cancelBtnRef = ref<HTMLButtonElement | null>(null);
const confirmBtnRef = ref<HTMLButtonElement | null>(null);

watch(
  () => store.confirmDialog.open,
  async (isOpen) => {
    await nextTick();
    const el = dialogRef.value;
    if (!el) return;
    if (isOpen) {
      if (!el.open) {
        el.showModal();
      }
      // 破坏性操作默认将焦点落在安全的「取消」按钮，普通确认则落在确认按钮
      const focusTarget =
        store.confirmDialog.tone === "danger"
          ? cancelBtnRef.value
          : confirmBtnRef.value;
      focusTarget?.focus();
    } else if (el.open) {
      el.close();
    }
  }
);

function handleCancelEvent(e: Event) {
  e.preventDefault();
  store.resolveConfirmDialog(false);
}

function handleBackdropClick(e: MouseEvent) {
  if (e.target === dialogRef.value) {
    store.resolveConfirmDialog(false);
  }
}
</script>

<template>
  <dialog
    ref="dialogRef"
    class="bm-confirm-dialog"
    :class="[
      `scheme-${store.resolvedColorScheme}`,
      store.resolvedColorScheme === 'watercolor_light'
        ? `light-variant-${store.lightThemeVariant}`
        : `dark-variant-${store.darkThemeVariant}`,
      { 'is-danger': store.confirmDialog.tone === 'danger' },
    ]"
    aria-labelledby="bm-confirm-title"
    aria-describedby="bm-confirm-desc"
    @cancel="handleCancelEvent"
    @click="handleBackdropClick"
  >
    <div class="bm-confirm-surface" @click.stop>
      <div class="bm-confirm-head">
        <span
          class="bm-confirm-icon"
          :class="{ danger: store.confirmDialog.tone === 'danger' }"
        >
          <PlumpIcon
            :name="store.confirmDialog.tone === 'danger' ? 'trash' : 'info'"
            :size="18"
          />
        </span>
        <div class="bm-confirm-copy">
          <h3 id="bm-confirm-title" class="bm-confirm-title">
            {{ store.confirmDialog.title }}
          </h3>
          <p id="bm-confirm-desc" class="bm-confirm-message">
            {{ store.confirmDialog.message }}
          </p>
        </div>
      </div>

      <div class="bm-confirm-actions">
        <button
          ref="cancelBtnRef"
          type="button"
          class="bm-confirm-btn ghost"
          @click="store.resolveConfirmDialog(false)"
        >
          {{ store.confirmDialog.cancelText || "取消" }}
        </button>
        <button
          ref="confirmBtnRef"
          type="button"
          class="bm-confirm-btn"
          :class="store.confirmDialog.tone === 'danger' ? 'danger' : 'primary'"
          @click="store.resolveConfirmDialog(true)"
        >
          {{ store.confirmDialog.confirmText || "确定" }}
        </button>
      </div>
    </div>
  </dialog>
</template>

<style scoped>
.bm-confirm-dialog {
  margin: auto;
  padding: 0;
  border: none;
  background: transparent;
  max-width: min(440px, 90vw);
  width: 100%;
  overflow: visible;
  color: var(--text-primary);
  opacity: 0;
  transform: translateY(10px) scale(0.95);
  transition:
    opacity 0.22s cubic-bezier(0.22, 1, 0.36, 1),
    transform 0.24s cubic-bezier(0.22, 1, 0.36, 1),
    overlay 0.24s allow-discrete,
    display 0.24s allow-discrete;
}

.bm-confirm-dialog[open] {
  opacity: 1;
  transform: translateY(0) scale(1);
}

@starting-style {
  .bm-confirm-dialog[open] {
    opacity: 0;
    transform: translateY(10px) scale(0.95);
  }
}

.bm-confirm-dialog:modal {
  overscroll-behavior: contain;
}

.bm-confirm-dialog::backdrop {
  background: transparent;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  overflow: hidden;
  overscroll-behavior: contain;
}

.bm-confirm-surface {
  border: none;
  border-radius: 22px;
  padding: 22px 24px 20px;
  background: color-mix(in srgb, var(--bg-elevated, #141924) 94%, transparent);
  backdrop-filter: blur(28px) saturate(150%);
  -webkit-backdrop-filter: blur(28px) saturate(150%);
  box-shadow:
    0 24px 56px rgba(0, 0, 0, 0.42),
    0 4px 16px rgba(0, 0, 0, 0.22);
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.bm-confirm-dialog.scheme-abyssal_dark.dark-variant-velvet_nebula .bm-confirm-surface {
  background: #23181f;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow:
    0 26px 56px rgba(10, 4, 8, 0.56),
    0 6px 18px rgba(0, 0, 0, 0.3);
}

.bm-confirm-dialog.scheme-watercolor_light .bm-confirm-surface {
  background: rgba(252, 253, 255, 0.94);
  box-shadow:
    0 20px 48px rgba(15, 23, 42, 0.16),
    0 4px 14px rgba(15, 23, 42, 0.08);
}

.bm-confirm-dialog.scheme-watercolor_light.light-variant-nordic_oat .bm-confirm-surface {
  background: #faf6ee;
  backdrop-filter: none;
  -webkit-backdrop-filter: none;
  box-shadow:
    0 20px 46px rgba(68, 56, 38, 0.16),
    0 4px 14px rgba(68, 56, 38, 0.08);
}

.bm-confirm-head {
  display: flex;
  align-items: flex-start;
  gap: 14px;
}

.bm-confirm-icon {
  flex: 0 0 40px;
  width: 40px;
  height: 40px;
  border-radius: 14px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--primary) 16%, transparent);
  color: var(--primary);
}

.bm-confirm-icon.danger {
  background: color-mix(in srgb, var(--coral, #fb7185) 18%, transparent);
  color: var(--coral, #fb7185);
}

.bm-confirm-copy {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.bm-confirm-title {
  margin: 0;
  font-size: 16px;
  font-weight: 800;
  line-height: 1.35;
  color: var(--text-primary);
}

.bm-confirm-message {
  margin: 0;
  font-size: 13.5px;
  line-height: 1.6;
  color: var(--text-secondary);
  word-break: break-word;
}

.bm-confirm-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
}

.bm-confirm-btn {
  border: none;
  border-radius: 999px;
  padding: 9px 20px;
  font-size: 13.5px;
  font-weight: 700;
  cursor: pointer;
  transition: transform 0.2s cubic-bezier(0.22, 1, 0.36, 1), background 0.18s ease, box-shadow 0.18s ease;
}

.bm-confirm-btn:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 2px;
}

.bm-confirm-btn:active {
  transform: scale(0.96);
}

.bm-confirm-btn.ghost {
  background: var(--bg-pill);
  color: var(--text-primary);
}

.bm-confirm-btn.ghost:hover {
  background: var(--bg-pill-hover);
}

.bm-confirm-btn.primary {
  background: var(--grad-kugou-blue, var(--primary));
  color: #ffffff;
  box-shadow: 0 8px 20px color-mix(in srgb, var(--primary) 34%, transparent);
}

.bm-confirm-btn.primary:hover {
  transform: translateY(-1px);
  box-shadow: 0 10px 24px color-mix(in srgb, var(--primary) 42%, transparent);
}

.bm-confirm-btn.danger {
  background: linear-gradient(135deg, #e11d48 0%, #f43f5e 100%);
  color: #ffffff;
  box-shadow: 0 8px 20px rgba(244, 63, 94, 0.32);
}

.bm-confirm-btn.danger:hover {
  transform: translateY(-1px);
  box-shadow: 0 10px 24px rgba(244, 63, 94, 0.42);
}

@media (prefers-reduced-motion: reduce) {
  .bm-confirm-dialog,
  .bm-confirm-dialog::backdrop {
    transition: none;
  }
}
</style>

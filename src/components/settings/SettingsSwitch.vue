<script setup lang="ts">
import { useId } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue: boolean;
    id?: string;
    disabled?: boolean;
    ariaLabel?: string;
  }>(),
  {
    id: undefined,
    disabled: false,
    ariaLabel: undefined,
  }
);

const emit = defineEmits<{
  (e: "update:modelValue", value: boolean): void;
  (e: "change", value: boolean): void;
}>();

const fallbackId = useId();
const controlId = props.id || `bms-switch-${fallbackId}`;

function onChange(e: Event) {
  if (props.disabled) return;
  const checked = (e.target as HTMLInputElement).checked;
  emit("update:modelValue", checked);
  emit("change", checked);
}
</script>

<template>
  <label
    :for="controlId"
    class="bms-switch-shell"
    :class="{
      'is-checked': modelValue,
      'is-disabled': disabled,
    }"
  >
    <input
      :id="controlId"
      type="checkbox"
      role="switch"
      class="bms-switch-native"
      :checked="modelValue"
      :aria-checked="modelValue ? 'true' : 'false'"
      :aria-label="ariaLabel"
      :disabled="disabled"
      @change="onChange"
    />
    <span class="bms-switch-track" aria-hidden="true">
      <span class="bms-switch-thumb"></span>
    </span>
  </label>
</template>

<style scoped>
.bms-switch-shell {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 26px;
  cursor: pointer;
  flex-shrink: 0;
  user-select: none;
}

.bms-switch-shell.is-disabled {
  opacity: 0.48;
  cursor: not-allowed;
}

.bms-switch-native {
  position: absolute;
  clip-path: inset(50%);
  overflow: hidden;
  width: 1px;
  height: 1px;
  margin: -1px;
  padding: 0;
  border: 0;
  white-space: nowrap;
}

.bms-switch-track {
  position: relative;
  width: 42px;
  height: 24px;
  border-radius: var(--bm-r-pill, 999px);
  background: var(--bms-surface-tile-hover, rgba(128, 128, 128, 0.28));
  transition: background 0.24s cubic-bezier(0.22, 1, 0.36, 1), box-shadow 0.24s ease;
}

.scheme-watercolor_light .bms-switch-track,
.scheme-light .bms-switch-track {
  background: rgba(24, 24, 27, 0.16);
}

.scheme-watercolor_light.light-variant-nordic_oat .bms-switch-track {
  background: #D6CBB8;
}

.bms-switch-shell.is-checked .bms-switch-track {
  background: var(--bms-focus-grad, var(--grad-kugou-primary));
  box-shadow: 0 5px 14px -4px color-mix(in oklab, var(--primary) 52%, transparent);
}

.bms-switch-native:focus-visible + .bms-switch-track {
  outline: 2px solid var(--primary-text, var(--primary));
  outline-offset: 2px;
}

.bms-switch-thumb {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.92);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.26);
  transition:
    transform 0.32s var(--ease-spring-tactile, cubic-bezier(0.22, 1.28, 0.36, 1)),
    width 0.16s ease,
    background 0.2s ease;
}

.bms-switch-shell.is-checked .bms-switch-thumb {
  transform: translateX(18px);
  background: #ffffff;
}

.bms-switch-shell:active:not(.is-disabled) .bms-switch-thumb {
  width: 22px;
}

.bms-switch-shell.is-checked:active:not(.is-disabled) .bms-switch-thumb {
  transform: translateX(14px);
}
</style>

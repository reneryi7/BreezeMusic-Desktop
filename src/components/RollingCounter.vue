<script setup lang="ts">
/**
 * RollingCounter.vue
 * 纵轴里程表数字翻滚微组件（用于评论点赞计数、队列数量变化等微触感反馈）
 */
import { computed, ref, watch } from "vue";
import { getMotionRuntimeProfile } from "../utils/motionRuntime";

const props = withDefaults(
  defineProps<{
    value: number | string;
    displayText?: string;
  }>(),
  {
    displayText: undefined,
  }
);

const direction = ref<"up" | "down">("up");

function toComparableNumber(val: number | string): number {
  if (typeof val === "number") return val;
  const parsed = Number.parseFloat(String(val).replace(/[^0-9.-]/g, ""));
  return Number.isFinite(parsed) ? parsed : 0;
}

watch(
  () => props.value,
  (next, prev) => {
    const n = toComparableNumber(next);
    const p = toComparableNumber(prev);
    direction.value = n >= p ? "up" : "down";
  }
);

const renderedText = computed(() =>
  props.displayText !== undefined ? props.displayText : String(props.value)
);

const transitionName = computed(() => {
  const profile = getMotionRuntimeProfile();
  if (profile.effectiveLevel === "none") return "";
  return direction.value === "up" ? "bm-roll-up" : "bm-roll-down";
});
</script>

<template>
  <span class="bm-rolling-counter">
    <Transition v-if="transitionName" :name="transitionName" mode="out-in">
      <span :key="renderedText" class="bm-rolling-counter-val">
        {{ renderedText }}
      </span>
    </Transition>
    <span v-else class="bm-rolling-counter-val">{{ renderedText }}</span>
  </span>
</template>

<style scoped>
.bm-rolling-counter {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  vertical-align: middle;
  line-height: 1.15;
  font-variant-numeric: tabular-nums;
}

.bm-rolling-counter-val {
  display: inline-block;
  will-change: transform, opacity;
}

.bm-roll-up-enter-active,
.bm-roll-down-enter-active {
  transition:
    transform 0.24s var(--ease-spring-tactile, cubic-bezier(0.22, 1.28, 0.36, 1)),
    opacity 0.16s ease-out;
}

.bm-roll-up-leave-active,
.bm-roll-down-leave-active {
  transition:
    transform 0.14s cubic-bezier(0.4, 0, 1, 1),
    opacity 0.12s ease-in;
}

.bm-roll-up-enter-from {
  transform: translateY(72%);
  opacity: 0;
}
.bm-roll-up-leave-to {
  transform: translateY(-68%);
  opacity: 0;
}

.bm-roll-down-enter-from {
  transform: translateY(-72%);
  opacity: 0;
}
.bm-roll-down-leave-to {
  transform: translateY(68%);
  opacity: 0;
}
</style>

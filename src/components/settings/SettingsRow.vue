<script setup lang="ts">
import PlumpIcon from "../PlumpIcon.vue";

withDefaults(
  defineProps<{
    title: string;
    description?: string;
    icon?: string;
    iconColor?: string;
    badge?: string;
    badgeTone?: "primary" | "mint" | "gold" | "coral" | "muted";
    vertical?: boolean;
    settingId?: string;
    labelFor?: string;
    highlighted?: boolean;
  }>(),
  {
    description: "",
    icon: "",
    iconColor: "var(--primary)",
    badge: "",
    badgeTone: "primary",
    vertical: false,
    settingId: undefined,
    labelFor: undefined,
    highlighted: false,
  }
);
</script>

<template>
  <div
    class="bms-row"
    :class="{
      'is-vertical': vertical,
      'is-flash-highlight': highlighted,
    }"
    :data-setting-id="settingId"
  >
    <div class="bms-row-info">
      <div v-if="icon" class="bms-row-icon">
        <PlumpIcon :name="icon" :size="17" :color="iconColor" />
      </div>
      <div class="bms-row-copy">
        <div class="bms-row-title-line">
          <label
            v-if="labelFor"
            :for="labelFor"
            class="bms-row-title bms-row-label"
          >
            {{ title }}
          </label>
          <span v-else class="bms-row-title">{{ title }}</span>
          <span
            v-if="badge"
            class="bms-chip"
            :class="badgeTone === 'primary' ? '' : badgeTone"
          >
            {{ badge }}
          </span>
        </div>
        <div v-if="description" class="bms-row-desc">{{ description }}</div>
      </div>
    </div>
    <div class="bms-row-control">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.bms-row-label {
  cursor: pointer;
}
</style>

<script setup lang="ts">
import PlumpIcon from "../PlumpIcon.vue";

withDefaults(
  defineProps<{
    title: string;
    subtitle?: string;
    icon?: string;
    iconColor?: string;
    settingId?: string;
  }>(),
  {
    subtitle: "",
    icon: "",
    iconColor: "var(--primary)",
    settingId: undefined,
  }
);
</script>

<template>
  <section class="bms-section" :data-setting-id="settingId">
    <header class="bms-section-head">
      <div class="bms-section-head-left">
        <div class="bms-section-title-row">
          <span v-if="icon" class="bms-section-icon" aria-hidden="true">
            <PlumpIcon :name="icon" :size="15" :color="iconColor" />
          </span>
          <h2 class="bms-section-title">{{ title }}</h2>
        </div>
        <p v-if="subtitle" class="bms-section-sub">{{ subtitle }}</p>
      </div>
      <div
        v-if="$slots.headerRight || $slots.actions"
        class="bms-section-head-right"
      >
        <slot name="headerRight" />
        <slot name="actions" />
      </div>
    </header>
    <div class="bms-panel">
      <slot />
    </div>
  </section>
</template>

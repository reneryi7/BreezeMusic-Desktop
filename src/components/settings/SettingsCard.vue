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
  <section class="bms-card" :data-setting-id="settingId">
    <header class="bms-card-head">
      <div class="bms-card-head-left">
        <div v-if="icon" class="bms-card-icon">
          <PlumpIcon :name="icon" :size="18" :color="iconColor" />
        </div>
        <div>
          <h2 class="bms-card-title">{{ title }}</h2>
          <p v-if="subtitle" class="bms-card-sub">{{ subtitle }}</p>
        </div>
      </div>
      <div
        v-if="$slots.headerRight || $slots.actions"
        class="bms-card-head-right"
      >
        <slot name="headerRight" />
        <slot name="actions" />
      </div>
    </header>
    <div class="bms-card-body">
      <slot />
    </div>
  </section>
</template>

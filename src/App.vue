<script setup lang="ts">
import PowerCurve from "./components/PowerCurve.vue";
import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const isReady = ref(false);
const error = ref<string | null>(null);

onMounted(async () => {
  try {
    // Smoke test the Rust backend
    const version = await invoke<string>("get_app_version");
    console.log("[BatteryShower] backend version:", version);
    isReady.value = true;
  } catch (e) {
    error.value = String(e);
  }
});
</script>

<template>
  <main class="app">
    <header class="hero">
      <h1>BatteryShower</h1>
      <p class="subtitle">
        A lightweight Windows taskbar battery power monitor.
      </p>
    </header>

    <section v-if="error" class="alert error">
      <strong>Backend error:</strong> {{ error }}
    </section>

    <section v-else-if="!isReady" class="alert info">
      Initializing backend...
    </section>

    <section v-else>
      <PowerCurve />
    </section>
  </main>
</template>

<style scoped>
.app {
  max-width: 960px;
  margin: 0 auto;
  padding: 24px;
  font-family:
    "Inter",
    -apple-system,
    BlinkMacSystemFont,
    "Segoe UI",
    sans-serif;
  color: #1a1a1a;
}

.hero {
  margin-bottom: 24px;
}

.hero h1 {
  margin: 0 0 4px;
  font-size: 24px;
  font-weight: 600;
  letter-spacing: -0.02em;
}

.subtitle {
  margin: 0;
  color: #6b7280;
  font-size: 14px;
}

.alert {
  padding: 12px 16px;
  border-radius: 8px;
  font-size: 14px;
}

.alert.error {
  background: #fee2e2;
  color: #991b1b;
}

.alert.info {
  background: #f3f4f6;
  color: #4b5563;
}
</style>

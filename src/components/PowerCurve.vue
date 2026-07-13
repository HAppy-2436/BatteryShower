<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import * as echarts from "echarts/core";
import { LineChart } from "echarts/charts";
import {
  GridComponent,
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  DataZoomComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { PowerSession } from "../types";

echarts.use([
  LineChart,
  GridComponent,
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  DataZoomComponent,
  CanvasRenderer,
]);

const chartEl = ref<HTMLDivElement | null>(null);
const chargeSession = ref<PowerSession | null>(null);
const dischargeSession = ref<PowerSession | null>(null);
const activeTab = ref<"charge" | "discharge">("charge");
const loadError = ref<string | null>(null);
const isLoading = ref(false);
let chart: echarts.ECharts | null = null;
let pollTimer: number | null = null;

function formatSession(s: PowerSession | null) {
  if (!s) return { points: [] as [number, number][], avg: 0, energy: 0, duration: 0 };
  const t0 = s.samples[0]?.timestamp ?? 0;
  const points = s.samples.map((p) => [p.timestamp - t0, p.power_watts]);
  const powerSum = s.samples.reduce((a, b) => a + b.power_watts, 0);
  const avg = powerSum / Math.max(1, s.samples.length);
  let energy = 0;
  for (let i = 1; i < s.samples.length; i++) {
    const dt = s.samples[i].timestamp - s.samples[i - 1].timestamp;
    energy += (s.samples[i].power_watts + s.samples[i - 1].power_watts) * 0.5 * dt;
  }
  energy /= 3600;
  const duration =
    s.samples.length > 1
      ? s.samples[samples_safe_last(s)].timestamp - s.samples[0].timestamp
      : 0;
  return { points, avg, energy, duration };
}

function samples_safe_last(s: PowerSession): number {
  return s.samples.length - 1;
}

function renderChart() {
  if (!chart) return;
  const session =
    activeTab.value === "charge" ? chargeSession.value : dischargeSession.value;
  const { points, avg, energy, duration } = formatSession(session);
  const hasData = points.length > 0;
  chart.setOption(
    {
      backgroundColor: "transparent",
      textStyle: { color: "#e5e7eb" },
      grid: { left: 50, right: 20, top: 30, bottom: 40 },
      tooltip: hasData
        ? {
            trigger: "axis",
            backgroundColor: "#1f2937",
            borderColor: "#374151",
            textStyle: { color: "#e5e7eb" },
            formatter: (params: any) => {
              const p = params[0];
              return `${(p.value[0] / 60).toFixed(1)} min<br/><b>${p.value[1].toFixed(0)} W</b>`;
            },
          }
        : { show: false },
      xAxis: {
        type: "value",
        name: "min",
        nameLocation: "middle",
        nameGap: 24,
        nameTextStyle: { color: "#9ca3af" },
        axisLine: { lineStyle: { color: "#374151" } },
        axisLabel: {
          color: "#9ca3af",
          formatter: (v: number) => (v / 60).toFixed(0),
        },
        splitLine: { lineStyle: { color: "#1f2937" } },
      },
      yAxis: {
        type: "value",
        name: "W",
        nameLocation: "middle",
        nameGap: 40,
        nameTextStyle: { color: "#9ca3af" },
        axisLine: { lineStyle: { color: "#374151" } },
        axisLabel: { color: "#9ca3af" },
        splitLine: { lineStyle: { color: "#1f2937" } },
      },
      series: [
        {
          type: "line",
          smooth: true,
          showSymbol: false,
          data: points,
          areaStyle: { opacity: 0.2, color: "#10b981" },
          lineStyle: { width: 2, color: "#10b981" },
          itemStyle: { color: "#10b981" },
        },
      ],
    },
    true,
  );
  stats.value = {
    avg,
    energy,
    duration,
    count: session?.samples.length ?? 0,
  };
}

const stats = ref({ avg: 0, energy: 0, duration: 0, count: 0 });

async function load() {
  isLoading.value = true;
  try {
    const [c, d] = await Promise.all([
      invoke<PowerSession | null>("get_latest_charge_session"),
      invoke<PowerSession | null>("get_latest_discharge_session"),
    ]);
    chargeSession.value = c;
    dischargeSession.value = d;
    loadError.value = null;
    renderChart();
  } catch (e) {
    loadError.value = String(e);
    console.error("load session failed:", e);
  } finally {
    isLoading.value = false;
  }
}

onMounted(() => {
  if (chartEl.value) {
    chart = echarts.init(chartEl.value);
  }
  load();
  // Auto-refresh every 3 s so an in-progress session shows up live
  // (matches the 1 Hz backend sampling rate and feels real-time without
  // hammering the SQLite query).
  pollTimer = window.setInterval(load, 3000);
});

onUnmounted(() => {
  if (pollTimer !== null) {
    window.clearInterval(pollTimer);
    pollTimer = null;
  }
  if (chart) {
    chart.dispose();
    chart = null;
  }
});
</script>

<template>
  <div class="curve">
    <div class="tabs">
      <button
        :class="{ active: activeTab === 'charge' }"
        @click="activeTab = 'charge'; renderChart()"
      >
        Charge
      </button>
      <button
        :class="{ active: activeTab === 'discharge' }"
        @click="activeTab = 'discharge'; renderChart()"
      >
        Discharge
      </button>
      <span class="refresh-hint" v-if="isLoading">refreshing…</span>
    </div>

    <div v-if="loadError" class="alert error">
      <strong>Backend error:</strong> {{ loadError }}
    </div>

    <div
      v-else-if="stats.count === 0"
      class="alert info"
    >
      No samples yet for this session. Plug / unplug the charger to start
      one (samples flow in at 1 Hz).
    </div>

    <div class="stats">
      <div class="stat">
        <div class="label">Samples</div>
        <div class="value">{{ stats.count }}</div>
      </div>
      <div class="stat">
        <div class="label">Avg power</div>
        <div class="value">{{ Math.round(stats.avg) }} W</div>
      </div>
      <div class="stat">
        <div class="label">Duration</div>
        <div class="value">
          {{ Math.floor(stats.duration / 60) }}m
          {{ Math.floor(stats.duration % 60) }}s
        </div>
      </div>
      <div class="stat">
        <div class="label">Energy</div>
        <div class="value">{{ stats.energy.toFixed(1) }} Wh</div>
      </div>
    </div>

    <div ref="chartEl" class="chart"></div>
  </div>
</template>

<style scoped>
.curve {
  background: #1f2937;
  border-radius: 12px;
  padding: 20px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  color: #e5e7eb;
}

.tabs {
  display: flex;
  gap: 4px;
  align-items: center;
  margin-bottom: 12px;
}

.tabs button {
  border: 1px solid #374151;
  background: #111827;
  color: #9ca3af;
  padding: 6px 14px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  transition: all 0.15s;
}

.tabs button:hover {
  background: #1f2937;
  color: #e5e7eb;
}

.tabs button.active {
  background: #10b981;
  color: #0a0a0a;
  border-color: #10b981;
}

.refresh-hint {
  margin-left: auto;
  color: #6b7280;
  font-size: 12px;
  font-style: italic;
}

.alert {
  padding: 12px 16px;
  border-radius: 8px;
  font-size: 14px;
  margin-bottom: 12px;
}

.alert.error {
  background: #7f1d1d;
  color: #fecaca;
  border: 1px solid #991b1b;
}

.alert.info {
  background: #1f2937;
  color: #d1d5db;
  border: 1px solid #374151;
}

.stats {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  margin-bottom: 16px;
}

.stat {
  background: #111827;
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid #374151;
}

.label {
  font-size: 11px;
  color: #9ca3af;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.value {
  font-size: 18px;
  font-weight: 600;
  margin-top: 2px;
  color: #e5e7eb;
  font-variant-numeric: tabular-nums;
}

.chart {
  width: 100%;
  height: 380px;
}
</style>

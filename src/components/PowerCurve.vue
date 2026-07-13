<script setup lang="ts">
import { onMounted, ref } from "vue";
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
let chart: echarts.ECharts | null = null;

function formatSession(s: PowerSession | null) {
  if (!s) return { points: [], avg: 0, energy: 0, duration: 0 };
  const t0 = s.samples[0]?.timestamp ?? 0;
  const points = s.samples.map((p) => [p.timestamp - t0, p.power_watts]);
  const powerSum = s.samples.reduce((a, b) => a + b.power_watts, 0);
  const avg = powerSum / Math.max(1, s.samples.length);
  // crude trapezoid integration of power over time -> Wh
  let energy = 0;
  for (let i = 1; i < s.samples.length; i++) {
    const dt = s.samples[i].timestamp - s.samples[i - 1].timestamp; // seconds
    energy += (s.samples[i].power_watts + s.samples[i - 1].power_watts) * 0.5 * dt;
  }
  energy /= 3600; // J -> Wh
  const duration =
    s.samples.length > 1
      ? s.samples[s.samples.length - 1].timestamp - s.samples[0].timestamp
      : 0;
  return { points, avg, energy, duration };
}

function renderChart() {
  if (!chart) return;
  const session =
    activeTab.value === "charge" ? chargeSession.value : dischargeSession.value;
  const { points, avg, energy, duration } = formatSession(session);
  chart.setOption(
    {
      grid: { left: 50, right: 20, top: 30, bottom: 40 },
      tooltip: {
        trigger: "axis",
        formatter: (params: any) => {
          const p = params[0];
          return `${(p.value[0] / 60).toFixed(1)} min<br/><b>${p.value[1].toFixed(2)} W</b>`;
        },
      },
      xAxis: {
        type: "value",
        name: "min",
        nameLocation: "middle",
        nameGap: 24,
        axisLabel: {
          formatter: (v: number) => (v / 60).toFixed(0),
        },
      },
      yAxis: {
        type: "value",
        name: "W",
        nameLocation: "middle",
        nameGap: 40,
      },
      series: [
        {
          type: "line",
          smooth: true,
          showSymbol: false,
          data: points,
          areaStyle: { opacity: 0.2 },
          lineStyle: { width: 2 },
        },
      ],
    },
    true,
  );
  stats.value = { avg, energy, duration, count: session?.samples.length ?? 0 };
}

const stats = ref({ avg: 0, energy: 0, duration: 0, count: 0 });

async function load() {
  try {
    const [c, d] = await Promise.all([
      invoke<PowerSession | null>("get_latest_charge_session"),
      invoke<PowerSession | null>("get_latest_discharge_session"),
    ]);
    chargeSession.value = c;
    dischargeSession.value = d;
    renderChart();
  } catch (e) {
    console.error("load session failed:", e);
  }
}

onMounted(() => {
  if (chartEl.value) {
    chart = echarts.init(chartEl.value);
  }
  load();
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
    </div>

    <div class="stats">
      <div class="stat">
        <div class="label">Samples</div>
        <div class="value">{{ stats.count }}</div>
      </div>
      <div class="stat">
        <div class="label">Avg power</div>
        <div class="value">{{ stats.avg.toFixed(2) }} W</div>
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
        <div class="value">{{ stats.energy.toFixed(2) }} Wh</div>
      </div>
    </div>

    <div ref="chartEl" class="chart"></div>
  </div>
</template>

<style scoped>
.curve {
  background: #fff;
  border-radius: 12px;
  padding: 16px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.04);
}

.tabs {
  display: flex;
  gap: 4px;
  margin-bottom: 12px;
}

.tabs button {
  border: 1px solid #e5e7eb;
  background: #f9fafb;
  color: #4b5563;
  padding: 6px 14px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  transition: all 0.15s;
}

.tabs button.active {
  background: #1f2937;
  color: white;
  border-color: #1f2937;
}

.stats {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  margin-bottom: 16px;
}

.stat {
  background: #f9fafb;
  padding: 10px 12px;
  border-radius: 8px;
}

.label {
  font-size: 11px;
  color: #6b7280;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.value {
  font-size: 18px;
  font-weight: 600;
  margin-top: 2px;
  font-variant-numeric: tabular-nums;
}

.chart {
  width: 100%;
  height: 380px;
}
</style>

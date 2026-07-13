/* Shared types between Rust backend and Vue frontend */

export interface PowerSample {
  timestamp: number; // Unix seconds
  power_watts: number;
  percentage: number;
}

export interface PowerSession {
  state: "charging" | "discharging";
  start_time: number;
  end_time: number | null;
  samples: PowerSample[];
}

# BatteryShower

> A lightweight Windows taskbar battery power monitor with an anti-aliased
> monospace numeric overlay. Inspired by the need to see real-time charge /
> discharge watts at a glance — without the visual noise of generic system
> monitors.

![Taskbar preview — charging in green, discharging in red, full in white](docs/preview.png)

## Why

Most "battery widgets" either:

- show only the **percentage** (Windows already does this — useless),
- use icons or thin fonts that get truncated at 16×16,
- are heavy and bring in features you don't want (CPU power caps, etc.),
- or read the noisy built-in "remaining time" field that jumps every second.

BatteryShower is the opposite:

- **just the watts**, drawn in a clean monospace glyph that fits the tray exactly,
- a **color gradient** that telegraphs state: full = white, mid = mid-saturation
  green or red, low = pure green or red,
- **average-power remaining-time** — uses a 5-minute sliding window instead
  of the instantaneous value,
- **only the most recent charge and discharge curves** are kept in history,
  so SQLite never bloats.

## Features

| # | Feature | Detail |
|---|---|---|
| 1 | Live charge / discharge power in the taskbar | Updated every 1 second |
| 1.1 | Color gradient by state + percentage | Full → white, charging → green, discharging → red; saturation rises as battery level drops (white at 100% → full color at 0%) |
| 2 | Hover tooltip with % + remaining time | "充电中 \| 78% \| 剩余 1h 23m" or "放电中 \| 23% \| 剩余 38m" |
| 2.1 | Full-state tooltip is empty | Nothing to show when the battery is at 100% |
| 3 | Anti-aliased monospace glyph | Renders 1-, 2-, and 3-digit values at different sizes; always centred in the 32×32 icon |
| 4 | 1 Hz sampling | Driven by a Rust tokio task |
| 5 | Left-click does **nothing** | By design — no accidental popups |
| 6 | Right-click menu | "View Power Curve" / "Quit" |
| 7 | Single-session history | Latest charge & latest discharge, each replaced on the next start |
| 8 | Pluggable architecture | `sensors::State` + `Store` + `color::icon_color` are all isolated; adding new metrics = drop a new `sensors/*` module |
| 9 | Author-tested on AMD Ryzen AI 9 H365 + iGPU | Designed around this hardware first, but the battery path is vendor-neutral (Windows IOCTL `IOCTL_BATTERY_QUERY_STATUS`) |

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                     Windows Tray                    │
│            (32×32 RGBA8 icon + tooltip)              │
└──────────────────────────▲──────────────────────────┘
                           │ every 1 s
┌──────────────────────────┴──────────────────────────┐
│                    Rust backend                     │
│  ┌──────────┐  ┌───────────┐  ┌──────────────────┐  │
│  │  battery  │→│ color +   │→│ tray + tooltip   │  │
│  │   IOCTL   │  │ render    │  │   updates        │  │
│  └─────┬────┘  └───────────┘  └──────────────────┘  │
│        │                                             │
│        ▼                                             │
│  ┌──────────┐  ┌───────────┐  ┌──────────────────┐  │
│  │   Store  │  │ estimator │  │ Tauri commands   │  │
│  │ SQLite   │  │ avg + ETA │  │  → Vue frontend  │  │
│  └──────────┘  └───────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────┘
                           ▲
                           │ invoke()
┌──────────────────────────┴──────────────────────────┐
│            Vue 3 + Vite + TypeScript                │
│   PowerCurve.vue — ECharts line chart of the        │
│   most recent charge or discharge session           │
└─────────────────────────────────────────────────────┘
```

## Project layout

```
BatteryShower/
├── src/                      # Vue 3 frontend
│   ├── App.vue
│   ├── components/
│   │   └── PowerCurve.vue    # ECharts line chart
│   └── types.ts              # shared types (Rust ⇄ TS)
├── src-tauri/                # Rust backend
│   ├── src/
│   │   ├── main.rs           # 4-line entry → lib::run()
│   │   ├── lib.rs            # Tauri setup, sampling loop, commands
│   │   ├── tray.rs           # tray icon, left-click noop, right menu
│   │   ├── render.rs         # 32×32 RGBA8 glyph via imageproc + rusttype
│   │   ├── color.rs          # gradient rules
│   │   ├── estimator.rs      # sliding-window avg, remaining-time
│   │   ├── store.rs          # SQLite session/sample store
│   │   └── sensors/
│   │       ├── mod.rs
│   │       └── battery.rs    # Windows IOCTL on \\.\BATTERY0
│   ├── assets/Consola.ttf    # embedded monospace font (Win2k+)
│   ├── icons/                # Tauri-required icon set
│   ├── capabilities/default.json
│   ├── tauri.conf.json
│   ├── Cargo.toml
│   └── build.rs
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html
├── LICENSE                   # MIT
└── .gitignore
```

## Requirements

| | |
|---|---|
| **OS** | Windows 10 1809+ / Windows 11 |
| **Runtime** | Microsoft Edge **WebView2** (preinstalled on Win11; [download](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) for older systems) |
| **Battery** | Any Windows-managed battery (single-cell or multi-cell) |
| **For dev** | Node 18+, Rust 1.77+ (rustup `default stable`), `tauri-cli` |

## Development

```powershell
# 1. install JS deps
npm install

# 2. install tauri-cli (if not present)
cargo install tauri-cli --version "^2.0" --locked

# 3. dev build — opens the system tray and a debug curve window
npm run tauri:dev

# 4. release build — produces a .msi / .exe installer in src-tauri/target/release/bundle
npm run tauri:build
```

## How the gradient works

`color::icon_color(state, percentage)` returns an `(R, G, B)` triple:

```rust
let t = 1.0 - (percentage as f64 / 100.0);  // 0 at 100%, 1 at 0%
let t = t.powf(0.85);                       // slight ease-in

match state {
    Charging    => (255 - 255*t, 255,         255 - 255*t),  // white → green
    Discharging => (255,         255 - 255*t, 255 - 255*t),  // white → red
    Full        => (255, 255, 255),                          // always white
}
```

That gives you a perceptually smooth ramp from a near-white tint at 99 % to a
saturated red or green at 0 %.

## Roadmap (next versions, not yet implemented)

- [ ] Configurable sampling rate (default 1 Hz)
- [ ] Optional CPU + GPU + system-total power channels (currently battery-only)
- [ ] CSV export of a session
- [ ] Custom gradient curves (HSL / piecewise)
- [ ] Per-monitor multi-instance tray icon

## License

[MIT](LICENSE) © 2026 HAppy-2436

## Acknowledgements

Inspired by [topabomb/BatteryMaster](https://github.com/topabomb/BatteryMaster) —
this project is a from-scratch reimplementation, **no code was copied from that
project** (which is unlicensed).

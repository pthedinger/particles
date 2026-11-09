# Measured performance

Measured 2026-11-09 on this development machine: macOS / Apple Silicon (aarch64), Rust 1.90.0, Bevy 0.14.2. Results describe this host under its current load. The application uses a release build, a 1/120-second fixed step and two constraint iterations. The default window is 1360 × 900 logical pixels; the captured Retina render is 2720 × 1800.

## Default application

The final 1,800-frame scripted windowed run used **3,000 particles** and exercised rotation, shaking and pause/resume. After startup warm-up, its 1,650 nonpaused timing samples measured:

- Median frame: **8.319 ms / 120.2 FPS**.
- 95th-percentile frame: **10.216 ms**.
- Median physics time per displayed frame: **2.858 ms**. This run often displayed 120 frames/second; do not compare this number directly with the two-step 60 Hz solver benchmark below.
- Simulated time: 14.858 seconds. Cumulative lost simulation time: 0.058 seconds, including initial stalls; deliberately paused time is excluded by the clock.
- All particles finite and contained, with the original 3,000 count preserved.

The default meets the proposed 60 FPS target in this run. Live timing now uses Bevy's **real** clock; its capped virtual clock would hide part of a long stall. The real-clock integration regression covers a full one-second frame.

A second 900-frame windowed run at **6,000 particles** measured 750 nonpaused samples: median **16.946 ms / 59.0 FPS**, p95 **23.789 ms**, physics median **13.181 ms** per displayed frame, and 0.050 seconds lost simulation time. It stayed finite and contained. This is a borderline setting on this host, with visible frame-time variation; it is not the 60 FPS default.

## Headless scaling

Each row measures **two fixed steps**, equivalent to 1/60 second of simulated time, without rendering. Each workload starts fresh and warms up for 120 frame-equivalents. 1,500 / 3,000 / 6,000 measure 600 frame-equivalents per workload; 12,000 is a shorter 120-frame scaling probe. These were run sequentially, after the app test.

| Particles | Workload | Median ms | p95 ms | Max ms |
| ---: | :--- | ---: | ---: | ---: |
| 1500 | settling | 2.345 | 2.674 | 3.150 |
| 1500 | rotation | 2.551 | 2.742 | 2.991 |
| 1500 | shaking | 2.624 | 2.953 | 4.156 |
| 3000 | settling | 5.825 | 6.297 | 7.137 |
| 3000 | rotation | 5.497 | 5.764 | 6.187 |
| 3000 | shaking | 5.894 | 6.476 | 11.306 |
| 6000 | settling | 13.495 | 14.139 | 14.770 |
| 6000 | rotation | 12.387 | 13.106 | 21.932 |
| 6000 | shaking | 13.531 | 14.469 | 15.539 |
| 12000 | settling | 32.051 | 33.404 | 34.909 |
| 12000 | rotation | 27.718 | 29.075 | 32.604 |
| 12000 | shaking | 30.786 | 32.503 | 33.703 |

At **3,000**, all three workloads meet the proposed median <8 ms and p95 <12 ms physics budgets. **6,000** takes roughly 12.4–13.5 ms of physics per 60 Hz frame-equivalent and leaves a narrow rendering budget. **12,000** takes 27.7–32.1 ms before drawing and is not a real-time 60 Hz setting on this CPU implementation. 24,000 is selectable for exploration but was not benchmarked here; no frame-rate claim is made for it.

Particle count and mixture remain unchanged when the app slows down. At most four fixed steps run per displayed frame; the app reports lost simulation time when that cannot maintain wall-clock progression. Color-field texture resolution and draw batching stay bounded; the CPU neighbor/constraint solve is the present scaling limit. Parallel/GPU physics is the next substantial scaling step, rather than adding one ECS entity per particle.

## Checks and limitations

- 22 behavior tests passed: recipe validation/area allocation, seed reproducibility, containment/count conservation, density contrast and dense settling, frictional settling, zero-distance contacts, pause/catch-up timing, offset/scaled coordinate mapping, transactional Apply/Reset, exact bounded half-turn braking, real-clock stall reporting and visible gas color.
- Formatting, full release tests and all-target compilation passed.
- An independent review found the clock, turn and gas-color issues; each has a regression observed failing before its fix and the final suite is green.
- The rendered final frame was visually inspected for coherent liquid regions, visible grains and frame orientation. The scripted run drives the same bounded rotation/shake/clock paths as the controls.
- Native mouse-event automation and live window resizing were not exercised. Geometry mapping is covered in unit tests; visual inspection used the normal desktop window size.
- This is an approximate 2D physical toy, not a calibrated multiphase CFD model. Bounded correction/acceleration clamps intentionally trade exact conservation and incompressibility for interactive stability.

Raw results are in [benchmarks](benchmarks/). The reproducible commands and controls are in the [README](../README.md).

![Final rendered vessel](screenshots/sand-art.png)

# Material lab implementation plan

**Goal:** User-controlled composition, material properties and temperature on the existing realtime GPU simulator.
**Spec:** docs/superpowers/specs/2026-11-10-material-lab-design.md
**Execution:** Inline, with meaningful model tests before implementation and one fresh final review.

- [x] Test exact percentages/solo/zero, presets, property derivation, temperature reversibility and invalid inputs. Implement MaterialProperties, presets, percentage operations and effective material definitions.
- [x] Parameterize CPU/GPU cohesion, compression, mixing and sample-size density targets; add deterministic thermal diffusion. Test real-GPU thermal response and unchanged particle identity. Keep existing regression suites green.
- [x] Build percentage/numeric controls, Solo, presets/custom definitions, property editors/effective readouts and global temperature. Keep structural changes explicit; update dynamic properties live. Test state application and derived properties.
- [x] Check all targets/format; run CPU+real GPU tests, inspect windowed controls and100k timing; document model/ranges and review/fix important findings; commit scoped changes and launch updated app.

Validation: 39 ordinary tests and 16 explicitly invoked real-GPU tests passed; all-target check, formatting and whitespace checks passed. The 900-frame 100k run measured 59.2 FPS median, simulation/wall ratio 0.996, with GPU state finite/contained and identities preserved. UI screenshot inspected and real egui Solo → Apply checked. One independent read-only final review found no Critical, Important or substantiated Minor issue. Its exclusions (calibrated thermodynamics, phase transitions, all-configuration performance, native input/resizing/accessibility) are outside this increment or explicitly documented as unverified. Parent independently read all test and performance outputs. No merge or push; continue existing branch and preserve the user's lockfile/scratch files.

# Implementation decisions and review

The implementation follows the approved [sand-art design](superpowers/specs/2026-11-09-sand-art-design.md). Work is on `codex/particle-sand-art` in the original checkout; no merge or remote push was performed.

## Decisions recorded during execution

- Proceed inline after design approval without another approval round. The user explicitly said to start. If implementation choices differ from their intent, those choices can be adjusted in the current checkout.
- Use a feature branch in the requested checkout rather than moving to a separate worktree, so the user's existing source and dependency edits remain present. The cost is that concurrent edits in this checkout require care.
- Preserve the previous simulator as `examples/legacy.rs`, including the user's pixel-size setting. This keeps it runnable and retains the existing dependencies; removing the old stack is a separate cleanup.
- Test a partially filled granular bed for downward settling. A near-full all-solid vessel cannot achieve the centroid of a small open pile. The revised fixture keeps a strict downward-centroid assertion with room to settle.
- Use two constraint iterations at the fixed 1/120 step after profiling the corrected dense mixture. This recovers the default CPU budget while passing the containment, settling and force checks. The tradeoff is less constraint convergence than three iterations, appropriate to this approximate physical toy.
- Keep the gas model without adding a speculative distance floor. A 3,000-particle diagnostic over 20 simulated seconds found around 24,000 local pairs and a maximum 25–29 neighbors, with no pathological pocket collapse in that case. Adversarial dense/high-count recipes could still increase neighborhood cost; this remains a scaling risk to profile.

## Review and fixes

An independent read-only review covered `f608ee7..a827bc8`, the design, implementation plan, and behavioral tests. It found capped virtual-time accounting (Important), half-turn overshoot and an ineffective gas-color control (both initially graded Minor). The latter two were treated as Important because the UI promises those user-visible behaviors.

All three were reproduced and fixed:

- A real Bevy one-second frame previously reported only 0.217 seconds of lost simulation time. The simulation and timing reports now consume `Time<Real>`; an integration test checks >0.96 seconds of lost time with bounded catch-up.
- The old half-turn ended around 191–192 degrees from rest. Braking now reserves acceleration for discrete stopping and handles initially opposing motion; the regression verifies 180 degrees within 0.002 radians and the acceleration bound.
- Gas previously rendered as transparent regardless of its color. The phase field now includes gas RGB and tint; a pixel-output test checks visible color and a live color change.

The final full suite passes 22 tests. Formatting and all-target compilation also pass. No unresolved actionable review finding remains.

The reviewer declined quantitative CFD accuracy/conservation after deliberate clamps (outside this toy's scope), arbitrary invalid external mutation of public state (outside supported app input paths), and final performance acceptance while measurements were pending. These were resolved by documenting approximations and supported inputs, and by the final [performance measurements](performance.md). No claim of laboratory accuracy or arbitrary-mutated-state robustness is made.

## Existing user changes

Only the newly required direct `glam` dependency entry was staged from Cargo.lock. The user's pre-existing lockfile updates remain uncommitted. The original main source is preserved in the legacy example. The untracked `a.out` and `tmp.c` files were left untouched.

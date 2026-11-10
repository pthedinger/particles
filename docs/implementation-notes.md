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


## Grain-sticking follow-up

The user's observation that solids stuck together led to isolated behavior tests. The old near-contact friction removed roughly two thirds of sideways relative speed even across a visible gap or with negligible compressive load. A separate solid-solid viscosity path coupled dry, noncontact grains. Finally, a single under-filled liquid parcel pulled two noncontacting, equal-density grains inward without viscosity or explicit surface tension.

The solver now records actual grain-contact positional corrections and uses their normal load to cap friction, preserving pair momentum and preventing reversal of tangential slip. Viscosity skips solid-solid pairs. Density constraints resist compression only; explicit fluid/gas cohesion stays separate from pressure. No defaults were weakened to hide the defects. Independent review approved the contact accounting and confirmed the residual tensile pressure mechanism before its one-line correction.

All 27 tests pass, including the existing settling and dense-mixture checks. The updated windowed test still measures about 120 FPS at the default 3,000 particles.


## Pressure-driven aggregation follow-up

The user still observed open grain networks in liquid after the isolated friction/attraction fixes. Twelve controlled temporary solver variants showed that disabling friction, viscosity or explicit phase cohesion did not remove this behavior. Removing direct grain displacement by the fluid pressure lattice did. The coarse fluid samples were also acting as finite rigid beads: one sample between two noncontacting grains changed their separation from 0.1795 to 0.2612 in one step.

The new coupling treats grains as immersed boundaries with zero inverse mass in the **fluid pressure projection only**. Physical grain inverse masses remain active for grain contacts, drag and force reactions. Grain volumes still contribute to density and displace fluid samples; fluid exclusion uses the actual grain radius and moves the fluid point only. The pressure denominator consistently uses these projection weights. Explicit Archimedes buoyancy now supplies solid hydrostatic response, replacing the old supplemental density-contrast term; equal-density submerged grains receive zero net gravity. Fluid-phase coupling remains unchanged.

The original neutral-density force-pass test assumed the former positional pressure lift and was replaced by the physical zero-net-gravity regression. New full-recipe settling and narrow-gap no-wedging cases were observed failing before the change. In the actual deterministic 1,200-particle balanced-mixture check, the upper/open region held **128/446 grains before** and **9/446 afterward** after a stir and ten simulated seconds. The regression allows a modest transient remainder, rather than requiring every grain to settle identically.

An independent focused review approved the approximate coupling and independently passed 12 quick physics checks. Its minor documentation issue was corrected; the gap test was renamed to match its measured no-wedging assertion. The complete release suite passes **29 tests**, including the four longer scenes, and all-target compilation passes. The final screenshot shows the actual application with a settled bed instead of the suspended scaffold.

The approximation deliberately does not exchange exact positional pressure impulses between fluid and grains; buoyancy and viscous drag retain equal/opposite reactions. This tradeoff addresses under-resolved fluid/grain coupling at interactive particle counts and is documented in the README. It should not be represented as fully conservative rigid/fluid pressure dynamics.


## GPU solver for 100,000 particles (2026-11-10)

GPU mode is now the default; `--cpu` retains the previous 3,000-particle reference. The existing UI and material recipes are shared. Initial seeding skips CPU relaxation in GPU mode, and resets publish a new revision/seed. Physics and instanced rendering live in the render world. Only seeds and small frame/material commands cross from the CPU; explicit smoke diagnostics read back actual particles and pixels after measurement.

The neighbour grid uses counting, exclusive scans and scatter to physically reorder the two particle buffers. This was essential: merely copying the CPU's indirect neighbour layout measured roughly 52 ms per two steps in dense settling; spatial ordering brought the two-iteration variant near17 ms. Stable IDs keep scratch and material identity attached to particles. The grid has no fixed cell capacity. Its block scan handles512 blocks with 256 lanes, covering the full valid100k/radius range. A maximum-size recipe regression caught the earlier256-block limit.

Two solver iterations delivered high frame rates but collapsed high-resolution liquid into a cap (pure-water mean y about-0.61 after 5 seconds). Eight iterations brought the regression above-0.35 (about-0.29); the final six-pass setting also passes at about-0.336 with less work. Friction moved to one final gather after pressure/contact correction; repeating it during eight position iterations injected energy and left grains suspended. Symmetric max(A_i,A_j,1) normalization makes the accumulated parallel viscosity/friction update bounded at both endpoints. These changes retain unilateral grain contacts and the approximate immersed-boundary coupling described above.

A fresh independent review found that the former radius/dt speed cap could not simply be replaced with a world-space cap: fast small grains could cross between endpoint-only contact checks. Swept grain contacts now query swept AABBs against conservative per-cell motion bounds, apply the earliest impact correction, and then run the position solve. A fast head-on pair at 100k-scale radius was observed swapping sides before the fix and stopping at impact after it, including a slightly overlapping starting pair. This is a visual collision model, not an exact many-body impact solver.

The blank first UI image was a lifetime issue: render-only Image extraction emits an asset removal event, which makes bevy_egui unregister the texture. Retaining the small Image asset in both worlds preserves registration; its CPU pixels are not updated or copied each frame. Real GPU pixel tests verify drawing/live colour changes; actual window and vessel screenshots verify the full handoff.

Review: no Critical findings; the Important tunneling finding is covered by a real-GPU regression and fixed. One Minor remains in diagnostic reporting: resetting/remixing/applying after smoke warmup mixes timing epochs. Run each measured recipe in a fresh process. Normal interactive resets recreate GPU state correctly. No code was merged or pushed.

Validation includes 29 ordinary CPU/app tests and 13 explicit real-GPU tests. GPU cases cover driven100k containment and stable identity,600 particles in one crowded cell, loaded/gap contacts, no fluid-sample wedging, full-mixture settling, pure-water volume, drawing/live colour, swept collisions, and the maximum valid material-size grid. The final measured performance and limitations are in performance.md. Bitwise GPU trajectories are not promised because parallel bin ordering changes summation order.


Final collision/performance choice: swept queries run for grains whose movement exceeds0.85 times the smallest grain radius. Slower nonoverlapping pairs cannot jump a complete grain diameter and retain ordinary endpoint projection. Each fast grain treats predicted neighbours as swept obstacles and limits its own motion. Exact high-speed impact momentum is sacrificed; normal loaded contacts retain their mass-weighted projection and bounded friction. This avoids parallel impact corrections amplifying a packed bed. The last position grid is reused for the next force pass because final damping changes only velocities.

The major late performance bottleneck was cross-phase pressure, rather than the spatial index itself. Smoke-only occupancy diagnostics found559 gas samples in one cell, while isolated gas stayed resolved (peak 8). A driven100k mixture regression reproduced collapse at 192 per cell. The GPU solver now keeps gas's own compressible pressure separate from liquid incompressibility: liquid still sees gas volume, but has zero gas pressure weight, and gas projection excludes liquid constraints. Buoyancy and symmetric bounded drag still exchange motion. The unchanged regression now has peak 7 gas samples/cell. Neighbour gathers explicitly reject samples outside the kernel support before expensive material/gradient work, and grain projection queries its actual contact-radius box rather than the larger fluid support box. This is an immersed-phase visual approximation; it does not conserve exact liquid/gas pressure impulses or implement a thermodynamic gas law.


The dense-bed regression also checks centroid/occupied volume: nominal 72%grain area cannot pack into a cap with mean y below roughly-0.24 without substantial overlap. An initial12k bed reached-0.314. Bounded cached contact support (decay 0.95, acceleration cap 3.6) reuses the preceding normal reaction while contact persists, and improved the centroid to about-0.246. Current contact detection clears it on separation; a real-GPU test compares a separated running pair against a fresh pair at the same state. Disabling clearing was observed to fail with residual velocity0.027. This cache uses unused scratch space and adds no solver passes. Global displacement remains capped at 2.5*dt, so swept neighbour bounds remain valid.

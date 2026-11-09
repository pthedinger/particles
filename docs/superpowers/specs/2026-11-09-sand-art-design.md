# Interactive particle-based sand art

Date: 2026-11-09
Project: /Users/peterh/sandboxes/playpen/particles
Status: Proposed design, awaiting user approval

## Intent and agreed direction

Turn the existing Rust/Bevy particle experiment into an interactive, sealed circular sand art vessel. The user wants a configurable mixture of liquid, gases, and different solid particles, with rotation and shaking followed by visible settling in real time. The user selected a particle-based physics solver over extending the existing cellular grid, conditional on real-time performance.

This is a substantial change to the simulation, rendering, and interaction. Keep Rust and Bevy; build an approachable physical toy with plausible, responsive motion. Quantitative laboratory accuracy is outside the first version. Performance targets below are acceptance criteria to measure, not existing results or guarantees.

## Existing project and constraints

The current application uses Bevy 0.14.2, bevy_pixel_buffer 0.8.1, rand 0.8.5, and bevy-inspector-egui 0.27.0. It stores one material per cell, updates using vertical density comparisons, renders a rectangular pixel buffer, and flips by rearranging cells. These motion and rendering paths need replacing for freely moving particles.

The checkout already has user changes in Cargo.lock and src/main.rs, plus untracked a.out and tmp.c. Preserve these changes and files. Inspect repository instructions again before implementation. The project is outside this chat's writable roots; request normal filesystem authorization when writing there. Avoid an engine upgrade unless a demonstrated blocker requires it. Use cached APIs and dependency sources to establish compatibility.

## Approaches considered

1. Extend the grid: cheapest route to many grains and familiar behavior, but lattice movement and uniform cell sizes limit inertia, grain sizes, and rotation.
2. Freely moving particles: more implementation and stability tuning, but allows momentum, contacts, different grain sizes, and continuous interaction. This is the user's selection, provided it meets the real-time target.

Implementation proposal: a CPU position-based solver with spatial neighbor search, fluid density constraints, grain contacts, viscosity, and an explicit approximation for buoyancy between phases. A GPU solver is a possible later optimization, not a dependency of the first version. Validate the CPU budget early, before polishing the interface.

## Vessel, phases, and composition

Model a shallow, two-dimensional circular vessel with impermeable walls and no particle sources or sinks. Depth is represented through tunable glass drag, not a third simulation dimension.

Provide an initial palette with water-like liquid, oil-like liquid, gas, and three colored granular solids. Each material has a stable identity and editable color, density, and viscosity or drag; solid materials additionally have grain radius and friction. Offer bounded, meaningful ranges rather than arbitrary values that routinely destabilize the solver. Liquid identity persists; different liquids need not mix into a new material. Gas is a low-density, more compressible phase, with simple same-phase cohesion to form visible pockets. No combustion, chemical reaction, dissolved gas, thermal effects, or phase changes.

Composition controls set relative occupied-area shares, normalized to 100%. They are not mass percentages or molecule counts. Liquid and gas parcels have a fixed reference radius; solid radii affect their number for a given area budget. Particle-count quality settings express an approximate resolution budget: initialization allocates shares and radii together so every enabled material is represented when feasible. Report the actual count and realized area shares. Reject an all-zero mixture and combinations that cannot fit rather than silently losing material. Gas occupies the remaining represented phase volume; do not unintentionally introduce a second invisible air phase.

Editing a recipe, radius, or resolution is a pending change until Apply/Reset rebuilds the vessel. Color, density, viscosity/drag, and friction may update existing particles live, without adding or deleting material. The reset action creates a reproducible distributed mixture with a known seed, after which it settles. Supply at least a balanced sand-art preset and a liquid/gas demonstration preset. Keep total represented fill and the circular boundary consistent across recipes; do not initialize severe overlaps.

## Physics and rotation

Store continuous positions, previous positions or velocities, phase identity, mass/inverse mass, and radius in compact buffers. Separate the pure simulation module from Bevy so correctness and timing can be checked headlessly.

Use a fixed simulation step, initially 1/120 second, with a bounded number of constraint iterations. A uniform spatial hash or equivalent bounded cell list restricts contact and fluid operations to nearby particles. Reuse allocated buffers. Do not compare every particle with every other particle or truncate neighbor lists silently.

Each step integrates gravity and applied vessel motion, predicts positions, resolves circular containment and grain contacts, relaxes liquid density/pressure and gas compression, reconstructs velocities, and applies bounded viscosity, contact friction, and damping. Rebuild neighbor information often enough that position corrections cannot invalidate contact coverage. Handle zero-distance neighbors deterministically. Radius, timestep, and maximum input acceleration must prevent tunneling during supported interactions.

Solids displace fluid and exchange momentum with it; fluid resistance slows their descent. Heavy grains should sink, lighter solids may float, and gas should rise and collect into pockets. A practical phase-buoyancy approximation is acceptable, but it must depend on the surrounding phase and density contrast, exchange reaction momentum with neighbors, and avoid making an unsupported grain rise in a gas pocket merely because it is lighter than water. Distinguish granular friction from fluid viscosity, so grains can form persistent piles while liquid levels relax.

Keep world gravity downward. Rotation changes the vessel's orientation and the velocity of its walls and front/back glass. Include finite glass drag toward the vessel's local tangential velocity, with stronger coupling for contacting grains and tunable liquid coupling. A circular radial wall alone is rotationally symmetric and cannot reliably carry a sand layer when turned. Glass coupling is a deliberate shallow-vessel approximation; the builder chooses a bounded law and documents its coefficients. Particle positions must not teleport with angle changes, and constant rotation must not inject unbounded energy.

Dragging a rim rotates the vessel continuously. Provide left/right rotation controls and a half-turn action, with a visible orientation marker. Clamp angular speed and acceleration, and animate requested turns through physical vessel motion. Shake applies a finite, bounded translation/acceleration pulse to the vessel and its contents; holding the control repeats pulses at a capped rate. Releasing controls leaves the contents to settle. Pause freezes simulation time and vessel motion; pending recipe changes remain editable. Reset restores a known initial state and angle.

## Presentation and controls

The vessel is the main visual focus. Render colored grains as smooth circles; draw liquid as a coherent translucent region or overlapping softened parcels, with distinguishable gas pockets. Avoid presenting the liquid as rigid colored marbles. Use Bevy's existing rendering facilities and batching; choose a modest texture pass or mesh only if profiling supports it. The visible orientation marker communicates frame motion even when the rim itself is symmetric.

Expose recipe shares, material properties, quality, pause/resume, reset/apply, shake, and rotation without hidden keyboard-only requirements. Add keyboard shortcuts and document them. Show actual particle count, frame time/FPS, physics time, and any simulation lag. Resize and high-DPI coordinate mapping must preserve the circular aspect and accurate rim hit-testing. UI input must not also rotate or shake the vessel. This design leaves detailed panel layout, typography, and palette to the builder.

Legacy painting, fire, source emitters, and image import are outside this focused mode. Document the new controls and the changed application purpose. Preserve the original behavior in git history rather than maintaining two independent simulation engines.

## Real-time budget and quality

Target 60 displayed FPS and simulation advancing at wall-clock speed in an optimized release build on this machine at a normal desktop window size. Start with an approximately 3,000-particle balanced preset and offer lower/higher quality presets around 1,500 and 6,000 particles. These counts are proposals to tune from measurements, including grain-radius effects.

For the chosen default, target aggregate physics cost per 1/60 second of simulated time below 8 ms median and 12 ms at the 95th percentile, leaving time for rendering/UI. Measure at least 600 frames after warm-up across settling, sustained rotation, and shaking. Record hardware/runtime facts that can be read and distinguish headless solver time from application frame time. Use a deterministic headless benchmark to make repeatable comparisons; it does not by itself establish 60 displayed FPS.

At low frame rates cap accumulated catch-up work, initially four fixed steps per rendered frame, to keep controls responsive; report any dropped simulation time instead of claiming real-time physics. Never delete particles, change recipe shares, or lower resolution silently. If the initial default misses the budget, optimize neighbor search and rendering, then explicitly select and report a lower tested default. Higher quality may be slower and must be labeled accordingly. If no useful particle count meets the target, report that result before expanding scope into a GPU rewrite.

## Verification and completion

Add meaningful deterministic solver checks for circular containment, finite state under repeated rotation and shaking, conservation of particle count and identities, repeatable initialization and recipe allocation, and pause behavior. Validate density contrast in simple scenes (heavy solid sinks in liquid, buoyant solid rises in liquid, neither receives water buoyancy when surrounded only by gas). Check supported grain radii for overlap stability and meaningful frictional settling. These checks should assert behavior over multiple simulation steps, not mirror implementation details.

Run Rust formatting, compilation, and relevant tests. Run the release benchmark and record median/p95 timing and actual counts for each tested quality. Visually verify the running application for liquid appearance, gas pockets, distinct grain motion, rotation and shake, material editing/apply, pause/reset, resizing, and input capture. The completion report distinguishes automated checks, measured performance, and visual observations; it explicitly names any verification that could not run.

## Decisions left to the builder

Choose module boundaries, compatible UI integration, fluid kernel, density constraint formulation, gas cohesion and interface tuning, exact drag/friction laws, batching strategy, valid property ranges, and final default resolution based on tests and profiling. Keep approximations explicit. A target of 60 FPS is agreed as the proposed interpretation of real time, pending user approval of this document; no specific particle count has been promised.

## Technical basis

- Macklin and Müller, Position Based Fluids: https://matthias-research.github.io/pages/publications/pbf_sig_preprint.pdf
- Bender, Müller, and Macklin, A Survey on Position Based Dynamics: https://matthias-research.github.io/pages/publications/PBDTutorial2017-CourseNotes.pdf

These support the interactive constraint-solver approach, not a claim that this project's proposed multiphase model or performance has already been validated.

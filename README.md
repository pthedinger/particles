# Sand Art

A shallow circular vessel of liquid, gas pockets and colored grains, simulated with freely moving particles in Rust/Bevy. Turn the frame, shake the mixture, and watch it settle. The initial recipe uses water, oil, air, golden sand, terracotta and obsidian.

## Run

```sh
cargo run --release --bin particles
```

Use a release build for real-time simulation. The default is 3,000 particles; choose 1,500, 3,000, 6,000, 12,000 or 24,000 in the controls and press **Apply mixture**. Higher settings may run slower than real time. The application never deletes particles or changes your mixture to maintain frame rate.

```sh
cargo run --release --bin particles -- --particles 6000
```

## Controls

- Drag the rim to rotate the glass continuously. Hold **Left** or **Right** buttons or the arrow keys for steady rotation.
- **Half-turn** / **F** requests a smooth half-turn; the white marker shows the frame's orientation.
- Hold **Shake** or **S** to agitate the vessel.
- **Pause / Resume** or **Space** freezes and resumes physical time, including frame motion.
- **Reset** or **R** reproduces the current recipe and seed. **Remix** changes the seed.
- Adjust material shares and grain sizes, then **Apply mixture** to rebuild the vessel. Preset buttons prepare a recipe; Apply activates it.
- Open each material's **Properties** to change density, viscosity/drag, friction and color live. Relative densities are measured against water at 1.0; lower-density grains can float. Grain sizes apply on rebuild.

Mixture shares are relative **occupied areas**, not masses or particle counts. Doubling a grain's radius gives it four times the area, so fewer grains represent the same share. The controls show normalized requested shares and the actual realized area share. Very small positive shares receive at least one particle; finite resolution can therefore round the exact percentage. Invalid recipes leave the running vessel intact and display an error.

## Physics and rendering

The headless core uses a fixed 1/120-second step, reusable uniform spatial bins and position-based fluid/contact constraints. Materials have separate identities, density, viscosity, radius and friction. Liquids resist compression, gas parcels are more compressible and cohesive, and grains resolve contacts with load-limited friction. Solid grains have no viscous connection to each other. Phase cohesion is separate from compression pressure.

Grains are **immersed boundaries** for the liquid/gas pressure solve: their volumes contribute to fluid density, while pressure projection moves fluid samples around the grains. Fluid samples are quadrature points, so their sampling radius does not wedge grains apart across narrow gaps. Grain motion comes from gravity, grain contacts, glass drag, and two-way fluid drag and Archimedes buoyancy. Equal-density submerged grains have zero net gravity. This avoids mechanically bracing grains in the coarse fluid sampling lattice, which previously left open grain networks suspended in the liquid.

This low-speed coupling approximates hydrodynamic pressure with buoyancy and drag. Their neighbor reactions exchange equal/opposite momentum before existing clamps, but the fluid positional projection does **not** transfer an exact pressure impulse back to grains. It is intended for interactive settling rather than quantitative fluid/rigid-body dynamics.

Rotation drives bounded wall/glass drag; a smooth circular boundary alone would barely stir its contents. Shaking applies bounded translational inertial acceleration in the vessel's frame. This is a visually plausible 2D physical toy: it approximates phase interfaces, gas pressure, buoyancy and shallow glass coupling. It is not a calibrated CFD or thermodynamic model; no combustion, heat, dissolved gas or phase changes are modeled. Same-phase cohesion and unlike-liquid repulsion are local approximations, not a quantitative surface-tension model.

A fixed-resolution soft field reconstructs liquid regions and gas openings; colored grains are drawn as one batched mesh. There is no Bevy entity per particle. The solver is independent of Bevy/UI, so later neighbor-search parallelism, a GPU solver and larger counts can be developed behind the same state boundary. The current CPU implementation remains the first scaling limit.

The frame meter shows actual frame, physics and surface-drawing time. At most four fixed steps run per displayed frame. If catch-up would exceed this, the clock reports lost simulation time rather than hiding lag. Brief startup/shader compilation stalls can contribute to this cumulative value. Resizing preserves the circular vessel and pointer coordinates; the desktop window has a minimum size to keep controls usable.

## Verification and performance

```sh
cargo test --release
cargo check --all-targets
cargo fmt --all -- --check
cargo run --release --bin benchmark -- 3000 600
```

The benchmark warms up 120 frame-equivalents, then independently measures settling, rotation and shaking. Each measured frame-equivalent advances two fixed steps (1/60 second). It reports median, 95th percentile and maximum solver time and checks finite state, containment and particle count. It does not measure rendering. Run different sizes separately to avoid contention:

```sh
cargo run --release --bin benchmark -- 1500 600
cargo run --release --bin benchmark -- 6000 600
cargo run --release --bin benchmark -- 12000 120
```

A windowed scripted test exercises rotation, shaking and pause/resume, captures the app's own rendered output, and records frame timing:

```sh
cargo run --release --bin particles -- --smoke-frames 1800 --scripted --screenshot /tmp/sand-art.png --report /tmp/sand-art.txt
```

Measured results for this development machine are recorded in [docs/performance.md](docs/performance.md). They are machine- and workload-specific, not a guarantee for other systems. The target is 60 displayed FPS at the default resolution; higher particle counts are an explicit future scaling goal.

## Original experiment

The previous cellular simulator, including the checkout's pre-existing pixel-size edit, is preserved as a runnable example:

```sh
cargo run --release --example legacy
```

Its original painting, sources, image import and fire modes are separate from the sand-art application. Your pre-existing lockfile updates and untracked scratch files are preserved.

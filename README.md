# Sand Art

A shallow circular vessel of liquid, gas pockets and colored grains, simulated with freely moving particles in Rust/Bevy. Turn the frame, shake the mixture, and watch it settle. The initial recipe uses water, oil, air, golden sand, terracotta and obsidian.

## Run

```sh
cargo run --release --bin particles
```

Use a release build. The default is **100,000 particles simulated and drawn on the GPU**. Choose 12,000, 24,000, 50,000 or 100,000 in the controls and press **Apply mixture**. Particle state stays on the GPU; the CPU sends controls and material properties. No particle count is reduced automatically.

```sh
cargo run --release --bin particles -- --particles 50000
cargo run --release --bin particles -- --cpu
```

`--cpu` runs the retained CPU reference at 3,000 particles by default. GPU mode needs compute and vertex storage-buffer support (tested with Metal on an Apple M4 Pro). Bevy/wgpu handles the graphics API.

## Controls

- Drag the rim to rotate the glass continuously. Hold **Left** or **Right** buttons or the arrow keys for steady rotation.
- **Half-turn** / **F** requests a smooth half-turn; the white marker shows the frame's orientation.
- Hold **Shake** or **S** to agitate the vessel.
- **Pause / Resume** or **Space** freezes and resumes physical time, including frame motion.
- **Reset** or **R** reproduces the current recipe and seed. **Remix** changes the seed.
- Set each material to **0–100%** with the slider or numeric percentage; **Solo** selects a pure material. **Apply mixture** rebuilds the vessel with the edited percentages, phase, size and particle count.
- Choose **Sand art**, **Dry grains**, **Liquids + bubbles**, **Viscous slurry**, **Float + sink** or **Pure water**, then **Load** to start that experiment immediately. Loading preserves particle count, seed and temperature.
- Open each material's **properties** to change its name, density, viscosity/drag, solid friction, fluid surface cohesion, compression resistance, liquid mixing resistance and colour. These update live. **+ Material** adds a custom definition (up to 16); choose its phase and properties, give it a share, then Apply.
- Move **Temperature** from −20 to 150°C. The per-material **Temperature response** controls expansion, thinning and thermal motion. Effective density and viscosity are displayed under the properties.

Shares describe **occupied area**, rather than mass or particle count. Editing a percentage keeps that value and proportionally rebalances the other active types to total 100%. Zero disables a type completely. If you reduce the only active type, the first other definition gets the remainder. Doubling radius gives four times the area, so fewer particles represent the same share. Each positive share receives at least one particle; the **in vessel** readout shows rounding from finite particle resolution. Invalid recipes leave the running vessel intact and display an error.

Size (0.5–2×), phase, percentages and count are draft changes until **Apply mixture**. All other properties and temperature change the current scene without reseeding or moving particles. Reference density is relative to water at 1.0; lower-density grains can float. Property values use relative solver units. Increasing compression resistance reduces fluid compression, increasing cohesion gathers separated same-material samples, and increasing mixing resistance separates different liquids. Solids have contact friction but no attractive cohesion.

Properties are defined at **20°C** and kept separate from derived values, so returning to 20°C restores them exactly. Heating usually lowers liquid density and viscosity; the signs and strengths can be adjusted per material. Gas target concentration decreases with absolute temperature. Thermal motion adds bounded, zero-mean velocity diffusion with strength proportional to the square root of absolute temperature; set its strength to zero to disable it. Temperature is a uniform external bath, with no heat transport, latent heat, freezing, melting or boiling. These are phenomenological responses for visual experiments, not calibrated thermodynamics; particle identity, count and geometric size remain fixed while temperature changes.

## Physics and rendering

The headless core uses a fixed 1/120-second step, reusable uniform spatial bins and position-based fluid/contact constraints. Materials have separate identities, density, viscosity, radius and friction. Liquids resist compression, gas parcels are more compressible and cohesive, and grains resolve contacts with load-limited friction. Solid grains have no viscous connection to each other. Phase cohesion is separate from compression pressure.

Grains are **immersed boundaries** for the liquid/gas pressure solve: their volumes contribute to fluid density, while pressure projection moves fluid samples around the grains. Fluid samples are quadrature points, so their sampling radius does not wedge grains apart across narrow gaps. Grain motion comes from gravity, grain contacts, glass drag, and two-way fluid drag and Archimedes buoyancy. Equal-density submerged grains have zero net gravity. This avoids mechanically bracing grains in the coarse fluid sampling lattice, which previously left open grain networks suspended in the liquid.

Gas has a separate compressible pressure solve: liquid sees gas parcel volume as an immersed boundary, without forcing low-mass gas samples through the liquid incompressibility projection. This prevents artificial gas collapse and dense neighbour-search hotspots. Gas remains coupled through buoyancy and drag, rather than a calibrated pressure/volume law.

This low-speed coupling approximates hydrodynamic pressure with buoyancy and drag. Their neighbor reactions exchange equal/opposite momentum before existing clamps, but the fluid positional projection does **not** transfer an exact pressure impulse back to grains. It is intended for interactive settling rather than quantitative fluid/rigid-body dynamics.

Rotation drives bounded wall/glass drag; a smooth circular boundary alone would barely stir its contents. Shaking applies bounded translational inertial acceleration in the vessel's frame. This is a visually plausible 2D physical toy: it approximates phase interfaces, gas pressure, buoyancy and shallow glass coupling. It is not a calibrated CFD or thermodynamic model; no combustion, heat transport, dissolved gas or phase changes are modeled. Same-phase cohesion and unlike-liquid repulsion are local approximations, not a quantitative surface-tension model.

The GPU solver uses WGSL compute shaders, read-only input snapshots and a separate output buffer. Counting, two-level prefix scans and scatter spatially reorder particles before neighbour gathers. There is no fixed cell capacity or all-pairs search. Stable IDs preserve material identity and solver history through reordering. Six pressure/contact iterations maintain bulk volume at high resolution; bounded contact support is reused while grains touch and cleared on separation. Friction runs once after position correction, with bounded symmetric neighbour damping. A conservative swept-motion guard catches fast crossings between timesteps, while a world-space speed bound avoids slowing flow simply because grains are smaller. This rare high-speed guard treats predicted neighbours as obstacles and does not conserve exact impact momentum; ordinary contact projection remains mass-weighted. GPU trajectories are not bitwise deterministic because bin insertion and summation order can vary.

Liquid/gas splats accumulate into a 1024-square GPU texture; grains use instanced quads with circular shading. egui samples the vessel texture directly. No CPU mesh or per-particle Bevy entities are created, and ordinary frames do not read particle state back to the CPU. The CPU reference renderer retains its soft field and batched grain mesh.

The frame meter shows displayed frame time and CPU command-submission time in GPU mode. Submission time is **not GPU execution time**. At most four 1/120-second steps run per displayed frame; the clock reports lost time if it cannot keep up. Startup/shader compilation stalls can contribute to this cumulative value. Resizing preserves the circular vessel and pointer coordinates; the desktop window has a minimum size to keep controls usable.

## Verification and performance

```sh
cargo test --release
cargo check --all-targets
cargo fmt --all -- --check
cargo run --release --bin benchmark -- 3000 600
cargo test --release --test gpu -- --ignored --test-threads=1
cargo run --release --bin gpu_benchmark -- 100000 300
```

The CPU benchmark warms up 120 frame-equivalents, then independently measures settling, rotation and shaking. Each measured frame-equivalent advances two fixed steps (1/60 second). It reports median, 95th percentile and maximum solver time and checks finite state, containment and particle count. It does not measure rendering. Run different sizes separately to avoid contention:

```sh
cargo run --release --bin benchmark -- 1500 600
cargo run --release --bin benchmark -- 6000 600
cargo run --release --bin benchmark -- 12000 120
```

The GPU benchmark waits for GPU completion for each sample and measures two 1/120-second steps. It excludes drawing. GPU tests are explicit/ignored in the ordinary suite because they require a compute-capable adapter; they fail if it is unavailable.

A windowed scripted test exercises rotation, shaking and pause/resume, captures the app's own rendered output, and records frame timing, active wall versus simulated time, and actual GPU containment/material identity. Particle and vessel-pixel readback happens **after** the measured window. A separate `.vessel.png` image accompanies the screenshot:

```sh
cargo run --release --bin particles -- --smoke-frames 1800 --scripted --screenshot /tmp/sand-art.png --report /tmp/sand-art.txt
```

Measured results for this development machine are recorded in [docs/performance.md](docs/performance.md). They are machine- and workload-specific, not a guarantee for other systems. The latest scripted run at 100,000 particles measured 59.2 FPS median and a 0.996 simulation/wall-time ratio, with 0.033 seconds dropped after warmup. See the performance report for the tested workload and limitations; one million particles remains future work.

## Original experiment

The previous cellular simulator, including the checkout's pre-existing pixel-size edit, is preserved as a runnable example:

```sh
cargo run --release --example legacy
```

Its original painting, sources, image import and fire modes are separate from the sand-art application. Your pre-existing lockfile updates and untracked scratch files are preserved.

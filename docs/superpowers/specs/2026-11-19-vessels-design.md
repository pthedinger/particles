# Configurable vessels for Sand Art

Status: user approved ready-made vessels first on 2026-11-19. A fresh builder check found no other important planning gap.

## Intent

Add distinct vessels that change how the existing liquid, gas and granular mixtures flow, settle and layer. The user specifically suggested a sand timer, square box, and irregular shapes containing fixed blockages or shelves. Preserve material experimentation and the 100,000-particle GPU target.

## Proposed scope

Start with a library of five vessels, selectable independently of material presets:

- **Circle:** retain the existing vessel and its initial composition/behaviour.
- **Square:** a box with softened corners, useful for corner piling and overturning layers.
- **Hourglass:** two chambers joined by a narrow, open throat. Start the mixture in its upper chamber so it visibly pours into the lower chamber. Particle size and properties may cause physical jamming; do not delete particles to clear it.
- **Cascade:** a broad chamber containing alternating, rounded shelves, with connected gaps at alternating ends. Turning or shaking creates falls and catches material on shelves.
- **Rock garden:** an irregular convex outer chamber with a reproducible arrangement of rounded fixed obstacles. A New layout action changes the layout; Reset reproduces it, and Remix changes particle placement without moving the obstacles.

The user selected this ready-made-vessel increment. A drawing tool, movable obstacles, open containers and resizing geometry around particles already in motion are deferred. Rock garden uses a randomized convex hull and rounded pegs; Hourglass supplies a concave outline. This is an implementation choice to keep neighbour visibility affordable.

## Approaches

1. **Shared boundary map (recommended):** define the vessel and its obstacles once, then build a signed-distance/normal map for fast GPU collision queries and matching drawing. Keep the current particle solver and neighbour index. This handles concave outlines and internal solid structures through one representation; map resolution and sampling require verification.
2. **Analytical shapes in each solver:** implement separate polygon/primitive collision loops in Rust and WGSL. This avoids map discretisation but duplicates geometry rules and adds work proportional to the number of wall segments at every particle projection.

The boundary-map approach is used for the approved ready-made-vessel increment. Circle can retain its analytical fast path to preserve existing behaviour and cost. GPU texture format/resolution and caching are implementation choices to settle with measurements, rather than adding a new graphics dependency by default.

## Behaviour and controls

A vessel dropdown and Load vessel action rebuild the scene using the chosen vessel and retain the current applied mixture, temperature, particle count and seed. Label that loading resets particle placement. Draft material edits stay explicit: vessel loading must not silently discard or apply them. Material preset loading retains the selected vessel; particle Remix and Reset also retain it. A separate layout seed is stored for Rock garden.

Outer walls, shelves and blockages form one rigid assembly. Existing rotation/half-turn controls rotate the actual geometry about the centre while gravity stays downward in world coordinates. Shaking continues to use the existing bounded inertial acceleration. Pausing freezes geometry and particles. Keep a visible outer rotation guide so the existing drag gesture remains usable for every outline; geometric walls may also act as drag handles when practical.

The selected vessel, walls, fluid field and solid particles must use the same pose. Rendering must clip fluid splats at walls and obstacles so liquid cannot appear to fill blocked regions. CPU reference drawing also respects the selected shape. The default layout, expanded properties and small-window scrolling remain usable after adding controls.

## Physics and initialization

Keep world-space particle positions, existing material definitions, GPU read-only neighbour gathers and separate output buffers. Add vessel geometry to Recipe/Simulation/GPU Seed, with Circle defaults for existing callers. A per-step vessel pose supplies rotation to boundary queries. Rebuild immutable geometry resources only on vessel/layout changes; normal frames do not rebuild the map or read back particles.

Use radius-aware clearance to project particle centres out of walls, with a consistent wall normal for velocity response and existing material friction relative to the moving wall. Collision handling must prevent particles crossing shelves or container walls at supported speeds, including rotation and repeated pressure corrections. Test narrow passages and obstacle ends. Avoid false fluid coupling across solid barriers, either through clearance-aware neighbour rejection or sufficient separation plus tested rules.

Scale new vessel geometry to fit inside the existing simulation bounds at every angle. Smaller particles in compact shapes can exceed the existing two-level grid scan capacity; keep grid dimensions bounded independently of physical kernel radius, or extend the scan, and verify the smallest supported all-large-material recipe at 100,000 particles. Do not silently alter physical kernel radius to fit the grid. Scale particle radii from available area to keep packing sensible across vessels. Initial particles must lie fully in free space; use deterministic, distributed placement rather than projecting a circular cloud onto walls. Hourglass uses the upper chamber's area/placement region to leave room below. Respect the configured particle count and all nonzero material shares. Unsupported low resolutions or particles too large for a chosen initialization region produce a useful error and preserve the running scene.

## Verification and acceptance

- Shared geometry tests: free-space signs, wall normals/projection, rounded obstacles, reproducible layouts, rotations and connected passages.
- Initialization: all vessel types, exact particle/material counts, no particles inside walls, same seeds reproduce positions and layouts.
- CPU and real-GPU behaviour: containment through turning/shaking; grains rest on shelves; walls prevent crossings; an hourglass population moves through the throat; no cross-wall fluid coupling; identity/count conservation.
- App state and editor tests: vessel selection independent of material presets, transactional errors, Reset/Remix/layout seed semantics, and paused behaviour.
- Rendering: inspect actual hourglass, shelves and irregular vessel captures; verify clipped fluid/solid pixels and matching rotated geometry.
- Keep all existing CPU/app and GPU tests green. Run formatting and all-target checks.
- Measure the 100,000-particle application with Circle and at least one obstacle-heavy vessel. Report displayed timing and simulated/wall-clock progression honestly; do not reduce count or simulation speed to meet a display target.

## Constraints and non-goals

Continue the existing Rust/Bevy/wgpu project and branch. Preserve the user's a.out and tmp.c, and the now-committed lockfile. No new worktree, merge or push. No phase changes or calibrated hourglass timing. This remains the existing approximate 2D physical toy. Low-resolution granular jamming and workload-dependent performance should be explained rather than concealed.

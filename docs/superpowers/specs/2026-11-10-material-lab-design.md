# Material and temperature experiments

User request: exact 0–100% composition control for every material, useful presets, editable shared physical properties, and a global temperature. User chose material-property changes and thermal motion first; no phase transitions.

Use exact area percentages: editing one holds its requested percentage and proportionally rebalances other active entries. Solo makes a pure material. Zero means no particles of that type. If no other type is active, the first other definition receives the remainder. Keep count allocation by area/radius² and total particle budget. Add custom definitions up to the existing16-material limit.

Separate MaterialProperties from identity/phase/share. Expose density, sampling/grain size, viscosity, friction, surface cohesion, mixing resistance, compression resistance, and thermal response coefficients. Density, viscosity, applicable interaction coefficients, colour and temperature update live; composition, phase and size require Apply/reseed. Presets load full coherent definitions and proportions while retaining budget, seed and environment temperature.

Temperature is a phenomenological bath from-20 to150°C with20°C reference values. Effective properties are derived from immutable reference definitions, never compounded: density changes through a bounded expansion response, viscosity through a bounded exponential response, gas target concentration through absolute temperature, and thermal velocity diffusion through sqrt(Kelvin ratio). Per-material thermal strength controls bounded zero-mean motion; phases stay fixed. Display effective density/viscosity. No calibrated thermodynamic, phase-change or exact mass-conservation claim.

Keep100k GPU default and CPU reference. Shared material derivation feeds both solvers; GPU struct layouts and shaders stay consistent. Parameterize formerly hard-coded cohesion/compression/mixing. Make fluid density targets account for self sample volume so changing liquid/gas size does not create an impossible target below the sample's own contribution.

Validate percentage endpoints, presets/custom definitions, pure phases, temperature reversibility/no compounding, effective properties, thermal motion on real GPU, material edits without position reset, varied sample size, existing loose-grain regressions, and100k realtime. Preserve user-owned lockfile changes/a.out/tmp.c. Continue existing branch in place, stage outside sandbox in /private/tmp and sync with approved writes.

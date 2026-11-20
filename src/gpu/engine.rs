use super::*;
use std::num::NonZeroU64;
use wgpu::util::DeviceExt;
const MAX_STEPS: usize = 8;
// Two iterations let 100k liquid collapse; validate six against the bulk-volume regression.
const SOLVER_ITERATIONS: usize = 6;
const INFO_SIZE: u64 = 64;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    n: u32,
    side: u32,
    h: f32,
    radius: f32,
    dt: f32,
    angular: f32,
    shake: [f32; 2],
    cells: u32,
    blocks: u32,
    max_radius: f32,
    min_radius: f32,
    thermal_tick: u32,
    angle: f32,
    cell_size: f32,
    vessel_kind: u32,
    pose: [f32; 2],
    previous_pose: [f32; 2],
}
struct Boundary {
    vessel: std::sync::Arc<crate::vessel::Vessel>,
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}
/// GPU allocations are owned for the lifetime of one mixture.
pub struct Engine {
    pub count: u32,
    pub side: u32,
    h: f32,
    cell_size: f32,
    pub angle: f32,
    pub(super) vessel_kind: u32,
    radius: f32,
    max_radius: f32,
    min_radius: f32,
    has_grid: bool,
    thermal_tick: u32,
    boundary: std::sync::Arc<Boundary>,
    state: [wgpu::Buffer; 2],
    pub(super) active: usize,
    pub(super) drawing: super::render::Drawing,
    material_buffer: wgpu::Buffer,
    params: wgpu::Buffer,
    groups: [wgpu::BindGroup; 2],
    pipelines: Vec<wgpu::ComputePipeline>,
}
impl Engine {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, seed: &Seed) -> Self {
        Self::create(device, queue, seed, None)
    }
    /// Replace particle state, retaining the uploaded boundary when geometry is unchanged.
    /// Returns whether the immutable GPU boundary was reused.
    pub fn reseed(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, seed: &Seed) -> bool {
        let reuse = std::sync::Arc::ptr_eq(&self.boundary.vessel, &seed.vessel);
        let cached = if reuse {
            Some(self.boundary.clone())
        } else {
            None
        };
        *self = Self::create(device, queue, seed, cached);
        reuse
    }
    fn create(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        seed: &Seed,
        cached: Option<std::sync::Arc<Boundary>>,
    ) -> Self {
        assert!(!seed.particles.is_empty() && seed.particles.len() <= 100_000);
        assert!(!seed.materials.is_empty() && seed.materials.len() <= 16);
        let count = seed.particles.len() as u32;
        let radius = seed.base_radius;
        let max_radius = seed
            .particles
            .iter()
            .map(|p| p.radius)
            .fold(radius, f32::max);
        let min_radius = seed
            .particles
            .iter()
            .map(|p| p.radius)
            .fold(f32::INFINITY, f32::min);
        let h = (radius * 4.4).max(max_radius * 2.05);
        let cell_size = h.max(2.0 / 357.0);
        let side = (2.0 / cell_size).ceil() as u32 + 3;
        let cells = side * side;
        let blocks = cells.div_ceil(256);
        assert!(blocks <= 512, "grid exceeds two-level scan capacity");
        let mut initial = seed.particles.clone();
        for (i, p) in initial.iter_mut().enumerate() {
            p.id = i as u32;
        }
        let state = std::array::from_fn(|_| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("GPU particle state"),
                contents: bytemuck::cast_slice(&initial),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            })
        });
        let alloc = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let material_buffer = alloc(
            "GPU materials",
            16 * 64,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        queue.write_buffer(&material_buffer, 0, bytemuck::cast_slice(&seed.materials));
        let params = alloc(
            "step parameters",
            256 * MAX_STEPS as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let info = alloc(
            "solver scratch",
            u64::from(count) * INFO_SIZE,
            wgpu::BufferUsages::STORAGE,
        );
        let counts = alloc(
            "grid counts",
            u64::from(cells * 2) * 4,
            wgpu::BufferUsages::STORAGE,
        );
        let offsets = alloc(
            "grid prefixes",
            u64::from(cells + blocks) * 4,
            wgpu::BufferUsages::STORAGE,
        );
        let boundary = cached.unwrap_or_else(|| {
            let map_size = if seed.vessel.kind == crate::vessel::VesselKind::Circle {
                1
            } else {
                crate::vessel::MAP_SIZE as u32
            };
            let boundary_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("vessel clearance and normal"),
                size: wgpu::Extent3d {
                    width: map_size,
                    height: map_size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let packed_map: Vec<u16> = seed
                .vessel
                .map
                .iter()
                .flat_map(|pixel| pixel.map(|v| half::f16::from_f32(v).to_bits()))
                .collect();
            queue.write_texture(
                boundary_texture.as_image_copy(),
                bytemuck::cast_slice(&packed_map),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(map_size * 8),
                    rows_per_image: Some(map_size),
                },
                wgpu::Extent3d {
                    width: map_size,
                    height: map_size,
                    depth_or_array_layers: 1,
                },
            );
            let boundary_view = boundary_texture.create_view(&Default::default());
            let boundary_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("filtered vessel map"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            std::sync::Arc::new(Boundary {
                vessel: seed.vessel.clone(),
                _texture: boundary_texture,
                view: boundary_view,
                sampler: boundary_sampler,
            })
        });
        let boundary_view = &boundary.view;
        let boundary_sampler = &boundary.sampler;
        let mut entries: Vec<_> = (0..7)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 0 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: binding == 1 || binding == 3,
                        }
                    },
                    has_dynamic_offset: binding == 0,
                    min_binding_size: if binding == 0 {
                        NonZeroU64::new(80)
                    } else {
                        None
                    },
                },
                count: None,
            })
            .collect();
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 7,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 8,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle compute layout"),
            entries: &entries,
        });
        let groups = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("particle compute bindings"),
                layout: &layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &params,
                            offset: 0,
                            size: NonZeroU64::new(80),
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: state[i].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: state[1 - i].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: material_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: info.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: counts.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: offsets.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::TextureView(&boundary_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: wgpu::BindingResource::Sampler(&boundary_sampler),
                    },
                ],
            })
        });
        let (visibility_source, needs_visibility) = visibility_shader(&seed.vessel, h, min_radius);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sand art solver"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "const VESSEL_KIND:u32={}u;\nconst NEEDS_VISIBILITY:bool={};\nconst CACHE_VISIBILITY:bool={};\n{}\n{}\n{}",
                    seed.vessel.kind as u32,
                    needs_visibility,
                    needs_visibility && seed.vessel.kind!=crate::vessel::VesselKind::Hourglass,
                    include_str!("solver.wgsl"),
                    include_str!("vessel.wgsl"),
                    visibility_source
                )
                .into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipelines = [
            "clear",
            "histogram",
            "scan_cells",
            "scan_blocks",
            "scatter",
            "environment",
            "predict",
            "density",
            "project",
            "finish",
            "damping_weights",
            "swept_contacts",
            "motion_bounds",
        ]
        .iter()
        .map(|name| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(name),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: name,
                compilation_options: Default::default(),
            })
        })
        .collect();
        let drawing = super::render::Drawing::new(
            device,
            &state,
            &material_buffer,
            &boundary_view,
            &boundary_sampler,
        );
        Self {
            count,
            side,
            h,
            cell_size,
            angle: seed.angle,
            vessel_kind: seed.vessel.kind as u32,
            radius,
            max_radius,
            min_radius,
            has_grid: false,
            thermal_tick: 0,
            boundary,
            state,
            active: 0,
            drawing,
            material_buffer,
            params,
            groups,
            pipelines,
        }
    }
    pub fn grid_cell_size(&self) -> f32 {
        self.cell_size
    }
    pub fn upload_materials(&self, queue: &wgpu::Queue, materials: &[GpuMaterial]) {
        assert!(materials.len() <= 16);
        queue.write_buffer(&self.material_buffer, 0, bytemuck::cast_slice(materials));
    }
    fn dispatch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        kernel: usize,
        slot: usize,
        groups: u32,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("sand art compute"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipelines[kernel]);
        pass.set_bind_group(0, &self.groups[self.active], &[(slot * 256) as u32]);
        pass.dispatch_workgroups(groups, 1, 1);
    }
    fn grid(&mut self, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        let cells = self.side * self.side;
        self.dispatch(encoder, 0, slot, cells.div_ceil(256));
        self.dispatch(encoder, 1, slot, self.count.div_ceil(256));
        self.dispatch(encoder, 2, slot, cells.div_ceil(256));
        self.dispatch(encoder, 3, slot, 1);
        self.dispatch(encoder, 4, slot, self.count.div_ceil(256));
        self.active = 1 - self.active;
    }
    pub fn advance(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, steps: &[Step]) {
        for chunk in steps.chunks(MAX_STEPS) {
            let mut packed = vec![0u8; chunk.len() * 256];
            for (i, s) in chunk.iter().enumerate() {
                assert!(s.dt.is_finite() && s.dt > 0. && s.dt <= 1. / 60.);
                let cells = self.side * self.side;
                let motion = crate::controls::bounded_motion(
                    s.angular_speed,
                    glam::Vec2::from_array(s.shake),
                );
                self.thermal_tick = self.thermal_tick.wrapping_add(1);
                let previous_angle = self.angle;
                self.angle =
                    (self.angle + motion.angular_velocity * s.dt).rem_euclid(std::f32::consts::TAU);
                let p = Params {
                    n: self.count,
                    side: self.side,
                    h: self.h,
                    radius: self.radius,
                    dt: s.dt,
                    angular: motion.angular_velocity,
                    shake: motion.shake.to_array(),
                    cells,
                    blocks: cells.div_ceil(256),
                    max_radius: self.max_radius,
                    min_radius: self.min_radius,
                    thermal_tick: self.thermal_tick,
                    angle: self.angle,
                    cell_size: self.cell_size,
                    vessel_kind: self.vessel_kind,
                    pose: [self.angle.cos(), self.angle.sin()],
                    previous_pose: [previous_angle.cos(), previous_angle.sin()],
                };
                packed[i * 256..i * 256 + 80].copy_from_slice(bytemuck::bytes_of(&p));
            }
            queue.write_buffer(&self.params, 0, &packed);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("GPU physics steps"),
            });
            for slot in 0..chunk.len() {
                if !self.has_grid {
                    self.grid(&mut encoder, slot);
                }
                self.dispatch(&mut encoder, 5, slot, self.count.div_ceil(128));
                self.dispatch(&mut encoder, 6, slot, self.count.div_ceil(128));
                self.active = 1 - self.active;
                self.dispatch(&mut encoder, 12, slot, self.count.div_ceil(128));
                self.dispatch(&mut encoder, 11, slot, self.count.div_ceil(128));
                self.active = 1 - self.active;
                for _ in 0..SOLVER_ITERATIONS {
                    self.grid(&mut encoder, slot);
                    self.dispatch(&mut encoder, 7, slot, self.count.div_ceil(128));
                    self.dispatch(&mut encoder, 8, slot, self.count.div_ceil(128));
                    self.active = 1 - self.active;
                }
                self.grid(&mut encoder, slot);
                self.dispatch(&mut encoder, 10, slot, self.count.div_ceil(128));
                self.dispatch(&mut encoder, 9, slot, self.count.div_ceil(128));
                self.active = 1 - self.active;
                self.has_grid = true;
            }
            queue.submit([encoder.finish()]);
        }
    }
    pub fn read_particles(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<GpuParticle> {
        let state = &self.state[self.active];
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("explicit particle readback"),
            size: state.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_buffer_to_buffer(state, 0, &buffer, 0, state.size());
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();
        let data = buffer.slice(..).get_mapped_range();
        let mut result: Vec<GpuParticle> = bytemuck::cast_slice(&data).to_vec();
        result.sort_unstable_by_key(|p| p.id);
        drop(data);
        buffer.unmap();
        result
    }
}

// Fixed geometry is compiled into uniform branches; no per-neighbour dynamic
// storage-buffer loop or trigonometry is needed for visibility.

fn rounded_visibility_needed(radius: f32, h: f32, min_radius: f32) -> bool {
    // Bilinear interpolation of distance-to-capsule is bounded by its Hessian.
    // Subtract interpolation + half-float error from the guaranteed free radius.
    let cell = 2. * crate::vessel::MAP_EXTENT / (crate::vessel::MAP_SIZE - 1) as f32;
    let error = cell * cell / (4. * (radius - cell * 2f32.sqrt()).max(0.000001))
        + min_radius * 0.001
        + 0.00002;
    let free_radius = (min_radius - error).max(0.);
    // The shortest chord through a rounded obstacle joins its offset surface
    // at two points separated by 2*sqrt((R+r)^2-R^2).
    h * h >= 4. * free_radius * (2. * radius + free_radius)
}
fn visibility_shader(vessel: &crate::vessel::Vessel, h: f32, min_radius: f32) -> (String, bool) {
    let vector = |x: f32, y: f32| format!("vec2<f32>({x:?},{y:?})");
    if vessel.kind == crate::vessel::VesselKind::Hourglass {
        // Width is affine between outline vertices. Only the two re-entrant
        // neck planes can be crossed by a segment joining interior points.
        let width = vessel
            .barriers
            .iter()
            .filter(|w| w[5] == 1.)
            .map(|w| w[0].abs())
            .fold(f32::INFINITY, f32::min);
        let mut planes: Vec<f32> = vessel
            .barriers
            .iter()
            .filter(|w| w[5] == 1. && (w[0].abs() - width).abs() < 1e-6)
            .map(|w| w[1])
            .collect();
        planes.sort_by(f32::total_cmp);
        planes.dedup();
        let mut source=String::from("fn clear_segment(world_a:vec2<f32>,world_b:vec2<f32>,pose:vec2<f32>)->bool{let a=vec2<f32>(dot(world_a,pose),dot(world_a,vec2<f32>(-pose.y,pose.x)));let b=vec2<f32>(dot(world_b,pose),dot(world_b,vec2<f32>(-pose.y,pose.x)));let dy=b.y-a.y; if abs(dy)>1e-12{");
        for y in planes {
            source+=&format!("if (a.y-({y:?}))*(b.y-({y:?}))<=0.0{{let x=a.x+(b.x-a.x)*({y:?}-a.y)/dy;if abs(x)>{width:?}{{return false;}}}}");
        }
        source += "}return true;}";
        return (source, true);
    }
    // The other outer outlines are convex. A segment joining free points
    // cannot leave them; only the rounded shelves/pegs need consideration.
    let active: Vec<_> = vessel
        .barriers
        .iter()
        .filter(|b| b[5] != 1. && rounded_visibility_needed(b[4], h, min_radius))
        .collect();
    if active.is_empty() {
        return (
            String::from(
                "fn clear_segment(a:vec2<f32>,b:vec2<f32>,pose:vec2<f32>)->bool{return true;}",
            ),
            false,
        );
    }
    let mut source=String::from("fn clear_segment(world_a:vec2<f32>,world_b:vec2<f32>,pose:vec2<f32>)->bool {let cs=pose.x;let sn=pose.y;let a=vec2<f32>(cs*world_a.x+sn*world_a.y,-sn*world_a.x+cs*world_a.y);let b=vec2<f32>(cs*world_b.x+sn*world_b.y,-sn*world_b.x+cs*world_b.y);let lo=min(a,b);let hi=max(a,b);\n");
    for wall in active {
        let c = glam::Vec2::new(wall[0], wall[1]);
        let d = glam::Vec2::new(wall[2], wall[3]);
        let radius = wall[4];
        let low = c.min(d) - glam::Vec2::splat(radius);
        let high = c.max(d) + glam::Vec2::splat(radius);
        source += &format!(
            "if all(hi>={})&&all(lo<={}) {{let c={};let d={};",
            vector(low.x, low.y),
            vector(high.x, high.y),
            vector(c.x, c.y),
            vector(d.x, d.y)
        );
        if wall[5] == 1. {
            source += "if segments_intersect(a,b,c,d){return false;}";
        } else if wall[5] == 3. {
            source += &format!(
                "if point_segment_squared(c,a,b)<={:?}{{return false;}}",
                radius * radius
            );
        } else {
            source+=&format!("if segments_intersect(a,b,c,d)||min(min(point_segment_squared(a,c,d),point_segment_squared(b,c,d)),min(point_segment_squared(c,a,b),point_segment_squared(d,a,b)))<={:?}{{return false;}}",radius*radius);
        }
        source += "}\n";
    }
    source += "return true;}";
    (source, true)
}

#[cfg(test)]
mod visibility_tests {
    use super::*;
    #[test]
    fn rounded_short_chord_certificate_preserves_shelf_cap_visibility() {
        let vessel = crate::vessel::Vessel::new(crate::vessel::VesselKind::Cascade, 17);
        for radius in [0.0005_f32, 0.001, 0.002, 0.005, 0.01] {
            let h = 2. * (2. * 0.045 * radius + radius * radius).sqrt() * 0.8;
            if rounded_visibility_needed(0.045, h, radius) {
                continue;
            }
            let points: Vec<_> = (0..128)
                .map(|i| {
                    glam::Vec2::new(0.21, 0.38)
                        + glam::Vec2::from_angle(i as f32 * std::f32::consts::TAU / 128.)
                            * (0.045 + radius)
                })
                .filter(|p| vessel.clearance(*p, 0.) >= radius)
                .collect();
            for a in &points {
                for b in &points {
                    if a.distance(*b) < h {
                        assert!(
                            vessel.segment_clear(*a, *b, 0.),
                            "short chord crossed cap: {a}/{b}"
                        );
                    }
                }
            }
        }
    }
}

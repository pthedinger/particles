use super::*;
use std::num::NonZeroU64;
use wgpu::util::DeviceExt;
const MAX_STEPS: usize = 8;
// Two iterations let 100k liquid collapse; validate six against the bulk-volume regression.
const SOLVER_ITERATIONS: usize = 6;
const INFO_SIZE: u64 = 48;
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
}
/// GPU allocations are owned for the lifetime of one mixture.
pub struct Engine {
    pub count: u32,
    pub side: u32,
    h: f32,
    radius: f32,
    max_radius: f32,
    min_radius: f32,
    has_grid: bool,
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
        let side = (2.0 / h).ceil() as u32 + 3;
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
            16 * 32,
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
        let entries: Vec<_> = (0..7)
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
                        NonZeroU64::new(48)
                    } else {
                        None
                    },
                },
                count: None,
            })
            .collect();
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
                            size: NonZeroU64::new(48),
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
                ],
            })
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sand art solver"),
            source: wgpu::ShaderSource::Wgsl(include_str!("solver.wgsl").into()),
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
        let drawing = super::render::Drawing::new(device, &state, &material_buffer);
        Self {
            count,
            side,
            h,
            radius,
            max_radius,
            min_radius,
            has_grid: false,
            state,
            active: 0,
            drawing,
            material_buffer,
            params,
            groups,
            pipelines,
        }
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
                };
                packed[i * 256..i * 256 + 48].copy_from_slice(bytemuck::bytes_of(&p));
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

//! Small frame commands cross into Bevy's render world; particle state stays there.
use crate::{simulate, smoke_test, Art};
use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::{RenderAssetUsages, RenderAssets},
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
        Render, RenderApp, RenderSet,
    },
};
use bevy_inspector_egui::bevy_egui::EguiUserTextures;
use particles::gpu::{Engine, GpuMaterial, Seed, Step};
use std::sync::{Arc, Mutex};
#[derive(Default, Clone)]
pub struct Feedback {
    pub checked: bool,
    pub finite_contained: bool,
    pub identities_preserved: bool,
    pub grain_mean_y: f32,
    pub suspended: usize,
    pub grains: usize,
    pub simulated: f32,
    pub submission_ms: f32,
    pub visible_pixels: usize,
}
#[derive(Resource, Clone, ExtractResource, Default)]
pub struct GpuFrame {
    seed: Option<Arc<Seed>>,
    revision: u64,
    steps: Vec<Step>,
    materials: Vec<GpuMaterial>,
    image: Handle<Image>,
    pub check: bool,
    pub capture: Option<String>,
    pub feedback: Arc<Mutex<Feedback>>,
}
#[derive(Resource, Default)]
struct Runtime {
    engine: Option<Engine>,
    revision: u64,
    simulated: f32,
}
pub struct GpuArtPlugin;
impl Plugin for GpuArtPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GpuFrame>()
            .add_plugins(ExtractResourcePlugin::<GpuFrame>::default())
            .add_systems(Startup, setup)
            .add_systems(Update, publish.after(simulate).before(smoke_test));
        app.sub_app_mut(RenderApp)
            .init_resource::<Runtime>()
            .add_systems(Render, run.in_set(RenderSet::PrepareResources));
    }
}
fn setup(
    mut images: ResMut<Assets<Image>>,
    mut textures: ResMut<EguiUserTextures>,
    mut frame: ResMut<GpuFrame>,
    mut art: ResMut<Art>,
) {
    let mut image = Image::new_fill(
        wgpu::Extent3d {
            width: 1024,
            height: 1024,
            depth_or_array_layers: 1,
        },
        wgpu::TextureDimension::D2,
        &[0, 0, 0, 0],
        wgpu::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage = wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::COPY_SRC;
    let handle = images.add(image);
    art.gpu_texture = Some(textures.add_image(handle.clone()));
    frame.image = handle;
}
fn publish(mut art: ResMut<Art>, mut frame: ResMut<GpuFrame>) {
    if frame.seed.is_none() || frame.revision != art.revision {
        frame.seed = Some(Arc::new(Seed::from(&art.simulation)));
        frame.revision = art.revision;
        frame.check = false;
        *frame.feedback.lock().unwrap() = Feedback::default();
    }
    frame.capture = art
        .options
        .screenshot
        .as_ref()
        .map(|p| format!("{p}.vessel.png"));
    frame.steps = std::mem::take(&mut art.gpu_steps);
    frame.materials = art.simulation.materials.iter().map(Into::into).collect();
    art.physics_ms = frame.feedback.lock().unwrap().submission_ms;
}
fn run(
    frame: Res<GpuFrame>,
    mut runtime: ResMut<Runtime>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    images: Res<RenderAssets<GpuImage>>,
) {
    let Some(seed) = &frame.seed else {
        return;
    };
    let Some(image) = images.get(&frame.image) else {
        return;
    };
    let d = device.wgpu_device();
    if runtime.engine.is_none() || runtime.revision != frame.revision {
        runtime.engine = Some(Engine::new(d, &queue, seed));
        runtime.revision = frame.revision;
        runtime.simulated = 0.;
    }
    let start = std::time::Instant::now();
    let engine = runtime.engine.as_mut().unwrap();
    engine.upload_materials(&queue, &frame.materials);
    engine.advance(d, &queue, &frame.steps);
    engine.draw(d, &queue, &image.texture_view);
    let ms = start.elapsed().as_secs_f32() * 1000.;
    if frame.check && !frame.feedback.lock().unwrap().checked {
        let out = engine.read_particles(d, &queue);
        // Explicit smoke diagnostics only: identify clustering that can make
        // neighbour work nonlinear even though the index has no overflow.
        let h = (seed.base_radius * 4.4).max(
            seed.particles
                .iter()
                .map(|p| p.radius * 2.05)
                .fold(0., f32::max),
        );
        let side = engine.side as usize;
        let mut bins = vec![[0usize; 3]; side * side];
        let bin = |p: &particles::gpu::GpuParticle| {
            let x = (((p.position[0] + 1.) / h).floor() as isize + 1).clamp(0, side as isize - 1)
                as usize;
            let y = (((p.position[1] + 1.) / h).floor() as isize + 1).clamp(0, side as isize - 1)
                as usize;
            (x, y)
        };
        for p in &out {
            let (x, y) = bin(p);
            bins[y * side + x][frame.materials[p.material as usize].phase as usize] += 1;
        }
        let mut candidates = [0usize; 3];
        let mut totals = [0usize; 3];
        for p in &out {
            let (x, y) = bin(p);
            let phase = frame.materials[p.material as usize].phase as usize;
            totals[phase] += 1;
            for cy in y.saturating_sub(1)..=(y + 1).min(side - 1) {
                for cx in x.saturating_sub(1)..=(x + 1).min(side - 1) {
                    candidates[phase] += bins[cy * side + cx].iter().sum::<usize>();
                }
            }
        }
        eprintln!(
            "Grid max cell occupancy liquid/gas/grain {:?}; mean neighbour candidates {:?}",
            std::array::from_fn::<_, 3, _>(|i| bins.iter().map(|b| b[i]).max().unwrap_or(0)),
            std::array::from_fn::<_, 3, _>(|i| candidates[i] as f32 / totals[i].max(1) as f32)
        );

        let buffer = d.create_buffer(&wgpu::BufferDescriptor {
            label: Some("smoke vessel pixels"),
            size: 1024 * 4096,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = d.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            image.texture.as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4096),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        d.poll(wgpu::Maintain::Wait);
        rx.recv().unwrap().unwrap();
        let pixels = buffer.slice(..).get_mapped_range();
        let visible = pixels
            .chunks_exact(4)
            .filter(|p| p[0] > 45 || p[1] > 55 || p[2] > 65)
            .count();
        if let Some(path) = &frame.capture {
            if let Err(error) =
                image::save_buffer(path, &pixels, 1024, 1024, image::ColorType::Rgba8)
            {
                error!("Vessel capture failed: {error}");
            }
        }
        drop(pixels);
        buffer.unmap();
        let mut f = frame.feedback.lock().unwrap();
        f.checked = true;
        f.visible_pixels = visible;
        f.identities_preserved = out.len() == seed.particles.len()
            && out
                .iter()
                .zip(&seed.particles)
                .enumerate()
                .all(|(i, (a, b))| {
                    a.id == i as u32 && a.material == b.material && a.radius == b.radius
                });
        f.finite_contained = out.iter().all(|p| {
            p.position.iter().chain(&p.velocity).all(|v| v.is_finite())
                && p.position[0].hypot(p.position[1]) + p.radius <= 1.001
        });
        for p in &out {
            if frame.materials[p.material as usize].phase == 2 {
                f.grains += 1;
                f.grain_mean_y += p.position[1];
                if p.position[1] > -0.3 && p.position[0].hypot(p.position[1]) < 0.9 {
                    f.suspended += 1;
                }
            }
        }
        f.grain_mean_y /= f.grains.max(1) as f32;
    }
    runtime.simulated += frame.steps.iter().map(|s| s.dt).sum::<f32>();
    let mut f = frame.feedback.lock().unwrap();
    f.simulated = runtime.simulated;
    f.submission_ms = ms;
}

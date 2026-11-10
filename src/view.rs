use crate::{measured_ms, Art};
use bevy::prelude::*;
use bevy_inspector_egui::{
    bevy_egui::EguiContexts,
    egui::{self, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke},
};
use particles::{
    controls::view_to_world,
    material::{Phase, Recipe},
};
use std::time::Instant;

const CREAM: Color32 = Color32::from_rgb(231, 225, 207);
const MUTED: Color32 = Color32::from_rgb(139, 157, 161);
const GOLD: Color32 = Color32::from_rgb(216, 183, 120);
const PANEL: Color32 = Color32::from_rgb(21, 32, 39);
const BACKGROUND: Color32 = Color32::from_rgb(12, 23, 30);
const FIELD_SIZE: usize = 320;

pub struct FluidField {
    texture: Option<egui::TextureHandle>,
    dirty: bool,
    weights: Vec<[f32; 8]>,
    image: egui::ColorImage,
}
impl FluidField {
    pub fn new() -> Self {
        Self {
            texture: None,
            dirty: true,
            weights: vec![[0.0; 8]; FIELD_SIZE * FIELD_SIZE],
            image: egui::ColorImage::new([FIELD_SIZE, FIELD_SIZE], Color32::TRANSPARENT),
        }
    }

    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    fn update(&mut self, ctx: &egui::Context, sim: &particles::simulation::Simulation) {
        self.dirty = false;
        self.weights.fill([0.0; 8]);
        let scale = (FIELD_SIZE as f32 - 1.0) * 0.5;
        for p in &sim.particles {
            let material = &sim.materials[p.material];
            if material.phase == Phase::Solid {
                continue;
            }
            let cx = (p.position.x + 1.0) * scale;
            let cy = (1.0 - p.position.y) * scale;
            let radius = p.radius * scale * 2.4;
            let r2 = radius * radius;
            let x0 = (cx - radius).max(0.0) as usize;
            let x1 = ((cx + radius).ceil() as usize).min(FIELD_SIZE - 1);
            let y0 = (cy - radius).max(0.0) as usize;
            let y1 = ((cy + radius).ceil() as usize).min(FIELD_SIZE - 1);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let q =
                        (1.0 - ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)) / r2).max(0.0);
                    let weight = q * q;
                    let field = &mut self.weights[y * FIELD_SIZE + x];
                    if material.phase == Phase::Gas {
                        field[4] += weight;
                        for c in 0..3 {
                            field[5 + c] += material.color[c] as f32 * weight;
                        }
                    } else {
                        for (c, sum) in field[..3].iter_mut().enumerate() {
                            *sum += material.color[c] as f32 * weight;
                        }
                        field[3] += weight;
                    }
                }
            }
        }
        for (i, field) in self.weights.iter().enumerate() {
            let x = (i % FIELD_SIZE) as f32 / scale - 1.0;
            let y = 1.0 - (i / FIELD_SIZE) as f32 / scale;
            if x * x + y * y > 1.0 {
                self.image.pixels[i] = Color32::TRANSPARENT;
                continue;
            }
            let liquid = field[3];
            let gas = field[4];
            let liquid_coverage = ((liquid - 0.035) / 0.36).clamp(0.0, 1.0);
            let gas_coverage = ((gas - 0.035) / 0.36).clamp(0.0, 1.0);
            let bubble = (gas / (liquid + gas).max(0.001) * 1.5 - 0.2).clamp(0.0, 1.0);
            let liquid_alpha = liquid_coverage * (1.0 - bubble);
            let gas_alpha = gas_coverage * bubble * 0.7;
            let alpha = (liquid_alpha + gas_alpha).min(1.0);
            if alpha < 0.001 {
                self.image.pixels[i] = Color32::TRANSPARENT;
                continue;
            }
            let light = 0.72 + 0.20 * (y + 1.0) * 0.5;
            let channel = |c: usize| {
                ((field[c] / liquid.max(0.001) * liquid_alpha * light
                    + field[5 + c] / gas.max(0.001) * gas_alpha)
                    / alpha)
                    .clamp(0.0, 255.0) as u8
            };
            self.image.pixels[i] = Color32::from_rgba_unmultiplied(
                channel(0),
                channel(1),
                channel(2),
                (alpha * 255.0) as u8,
            );
        }
        if let Some(texture) = &mut self.texture {
            texture.set(self.image.clone(), egui::TextureOptions::LINEAR);
        } else {
            self.texture = Some(ctx.load_texture(
                "liquid surface",
                self.image.clone(),
                egui::TextureOptions::LINEAR,
            ));
        }
    }
}

pub fn run_ui(
    mut contexts: EguiContexts,
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut art: ResMut<Art>,
) {
    let ctx = contexts.ctx_mut();
    if art.frames == 0 {
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.override_text_color = Some(CREAM);
        style.visuals.selection.bg_fill = Color32::from_rgb(73, 103, 103);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(35, 51, 59);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(53, 73, 77);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(74, 96, 95);
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 9.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.0));
        ctx.set_style(style);
    }
    let state = &mut *art;
    let elapsed = time.delta_seconds();

    if !ctx.wants_keyboard_input() {
        if keys.just_pressed(KeyCode::Space) {
            state.paused = !state.paused;
        }
        if keys.just_pressed(KeyCode::KeyR) {
            state.apply(state.recipe.clone());
        }
        if keys.just_pressed(KeyCode::KeyF) {
            state.turn_remaining = std::f32::consts::PI;
        }
    }
    sidebar(ctx, state);
    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(BACKGROUND)
                .inner_margin(egui::Margin::same(24.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "THE VESSEL   /   {:03.0}°",
                        state.simulation.angle.to_degrees()
                    ))
                    .size(11.0)
                    .color(MUTED)
                    .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(if state.paused { "PAUSED" } else { "SETTLING" })
                            .color(if state.paused {
                                GOLD
                            } else {
                                Color32::from_rgb(128, 188, 163)
                            })
                            .size(11.0),
                    );
                });
            });
            let mut rect = ui.available_rect_before_wrap();
            rect.max.y -= 40.0;
            let radius = ((rect.width().min(rect.height()) - 70.0) * 0.5).max(10.0);
            let center = rect.center();
            let response = ui.allocate_rect(rect, Sense::click_and_drag());
            let mut desired_speed = state.button_turn;
            if response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let p = view_to_world(
                        glam::Vec2::new(pos.x, pos.y),
                        glam::Vec2::new(center.x, center.y),
                        radius,
                    );
                    if (0.82..=1.13).contains(&p.length()) {
                        state.drag_angle = Some(p.y.atan2(p.x));
                    }
                }
            }
            if response.dragged() && state.drag_angle.is_some() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let p = view_to_world(
                        glam::Vec2::new(pos.x, pos.y),
                        glam::Vec2::new(center.x, center.y),
                        radius,
                    );
                    let angle = p.y.atan2(p.x);
                    let previous = state.drag_angle.unwrap();
                    let delta = (angle - previous + std::f32::consts::PI)
                        .rem_euclid(std::f32::consts::TAU)
                        - std::f32::consts::PI;
                    desired_speed = (delta / elapsed.max(0.001)).clamp(-3.0, 3.0);
                    state.drag_angle = Some(angle);
                }
            }
            if response.drag_stopped() {
                state.drag_angle = None;
            }
            if !ctx.wants_keyboard_input() {
                if keys.pressed(KeyCode::ArrowLeft) {
                    desired_speed = 1.6;
                }
                if keys.pressed(KeyCode::ArrowRight) {
                    desired_speed = -1.6;
                }
                if keys.pressed(KeyCode::KeyS) && !state.paused {
                    state.shake_remaining = 0.4;
                }
            }
            // Button hold and half-turn requests use the same bounded physical input path.
            if state.options.scripted {
                if (80..220).contains(&state.frames) {
                    desired_speed = 1.6;
                }
                if (230..360).contains(&state.frames) {
                    state.shake_remaining = 0.4;
                }
                state.paused = (370..400).contains(&state.frames);
            }
            state.desired_speed = desired_speed;
            let draw_start = Instant::now();
            draw_vessel(ui, ctx, state, center, radius);
            state.drawing_ms = measured_ms(draw_start);
            let footer = Pos2::new(center.x, rect.max.y + 18.0);
            ui.painter().text(
                footer,
                egui::Align2::CENTER_CENTER,
                "Drag the rim to turn  ·  Left / Right rotate  ·  S shake  ·  Space pause",
                FontId::proportional(13.0),
                MUTED,
            );
        });
}

fn sidebar(ctx: &egui::Context, state: &mut Art) {
    state.button_turn = 0.0;
    egui::SidePanel::left("controls")
        .exact_width(318.0)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(PANEL)
                .inner_margin(egui::Margin::same(24.0)),
        )
        .show(ctx, |ui| {
            ui.label(
                RichText::new("S A N D   /   0 1")
                    .size(11.0)
                    .color(GOLD)
                    .strong(),
            );
            ui.add_space(12.0);
            ui.label(RichText::new("Matter in\nmotion.").size(37.0).color(CREAM));
            ui.label(
                RichText::new("A small world. An endless rearrangement.")
                    .size(12.0)
                    .color(MUTED),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui
                    .add_sized(
                        [121.0, 36.0],
                        egui::Button::new(if state.paused { "Resume" } else { "Pause" }),
                    )
                    .clicked()
                {
                    state.paused = !state.paused;
                }
                if ui
                    .add_sized([121.0, 36.0], egui::Button::new("Reset"))
                    .clicked()
                {
                    state.apply(state.recipe.clone());
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!state.paused, egui::Button::new("Left"))
                    .is_pointer_button_down_on()
                {
                    state.button_turn = 1.6;
                }
                if ui
                    .add_enabled(!state.paused, egui::Button::new("Right"))
                    .is_pointer_button_down_on()
                {
                    state.button_turn = -1.6;
                }
                if ui
                    .add_enabled(!state.paused, egui::Button::new("Half-turn"))
                    .clicked()
                {
                    state.turn_remaining = std::f32::consts::PI;
                }
                if ui
                    .add_enabled(!state.paused, egui::Button::new("Shake"))
                    .is_pointer_button_down_on()
                {
                    state.shake_remaining = 0.4;
                }
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("THE MIXTURE").size(11.0).color(GOLD).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("shares by area").size(11.0).color(MUTED));
                });
            });
            ui.horizontal(|ui| {
                if ui.small_button("Sand art").clicked() {
                    let budget = state.pending.budget;
                    state.pending = Recipe::balanced();
                    state.pending.budget = budget;
                }
                if ui.small_button("Liquid + bubbles").clicked() {
                    for (i, m) in state.pending.materials.iter_mut().enumerate() {
                        m.share = match i {
                            0 => 70.0,
                            1 => 10.0,
                            2 => 20.0,
                            _ => 0.0,
                        };
                    }
                }
                if ui.small_button("Remix").clicked() {
                    let mut recipe = state.recipe.clone();
                    recipe.seed = recipe.seed.wrapping_add(1);
                    state.apply(recipe);
                }
            });
            let total_share: f32 = state.pending.materials.iter().map(|m| m.share).sum();
            let mut actual_area = vec![0.0; state.simulation.materials.len()];
            for p in &state.simulation.particles {
                actual_area[p.material] += p.radius * p.radius;
            }
            let area: f32 = actual_area.iter().sum();
            egui::ScrollArea::vertical()
                .max_height((ui.available_height() - 185.0).max(100.0))
                .show(ui, |ui| {
                    for (i, m) in state.pending.materials.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(8.0, 8.0), Sense::hover());
                            ui.painter().circle_filled(
                                rect.center(),
                                4.0,
                                Color32::from_rgb(m.color[0], m.color[1], m.color[2]),
                            );
                            ui.label(RichText::new(&m.name).size(13.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "{:.0}%",
                                            100.0 * m.share / total_share.max(0.001)
                                        ))
                                        .size(12.0)
                                        .color(MUTED),
                                    );
                                },
                            );
                        });
                        ui.add(egui::Slider::new(&mut m.share, 0.0..=100.0).show_value(false));
                        egui::CollapsingHeader::new(
                            RichText::new(format!(
                                "Properties  ·  {:.1}% in vessel",
                                actual_area[i] / area.max(0.001) * 100.0
                            ))
                            .size(10.0)
                            .color(MUTED),
                        )
                        .id_salt(i)
                        .show(ui, |ui| {
                            ui.add(
                                egui::Slider::new(&mut m.density, 0.02..=10.0)
                                    .logarithmic(true)
                                    .text("Density"),
                            );
                            ui.add(
                                egui::Slider::new(&mut m.viscosity, 0.0..=8.0)
                                    .text("Viscosity / drag"),
                            );
                            if m.phase == Phase::Solid {
                                ui.add(
                                    egui::Slider::new(&mut m.radius, 0.5..=2.0).text("Grain size"),
                                );
                                ui.add(
                                    egui::Slider::new(&mut m.friction, 0.0..=1.0).text("Friction"),
                                );
                            }
                            ui.horizontal(|ui| {
                                ui.label("Colour");
                                ui.color_edit_button_srgb(&mut m.color);
                            });
                        });
                        let live = &mut state.simulation.materials[i];
                        if live.color != m.color {
                            state.field.dirty = true;
                        }
                        live.density = m.density;
                        live.viscosity = m.viscosity;
                        live.friction = m.friction;
                        live.color = m.color;
                        let saved = &mut state.recipe.materials[i];
                        saved.density = m.density;
                        saved.viscosity = m.viscosity;
                        saved.friction = m.friction;
                        saved.color = m.color;
                        ui.add_space(3.0);
                    }
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Particles").color(MUTED));
                egui::ComboBox::from_id_salt("resolution")
                    .selected_text(format!("{}", state.pending.budget))
                    .show_ui(ui, |ui| {
                        for n in if state.gpu {
                            vec![12000, 24000, 50000, 100000]
                        } else {
                            vec![1500, 3000, 6000, 12000, 24000]
                        } {
                            ui.selectable_value(&mut state.pending.budget, n, format!("{n}"));
                        }
                    });
            });
            if ui
                .add_sized(
                    [ui.available_width(), 36.0],
                    egui::Button::new(RichText::new("Apply mixture").color(PANEL)).fill(GOLD),
                )
                .clicked()
            {
                state.apply(state.pending.clone());
            }
            ui.label(
                RichText::new(
                    "Shares, size & resolution need Apply mixture.\nOther properties change live.",
                )
                .size(10.0)
                .color(MUTED),
            );
            if let Some(error) = &state.error {
                ui.colored_label(Color32::LIGHT_RED, error);
            }
            ui.add_space(8.0);
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "{} particles   ·   {:.0} FPS",
                    state.simulation.particles.len(),
                    1000.0 / state.frame_ms.max(0.1)
                ))
                .size(12.0)
                .color(CREAM),
            );
            ui.label(
                RichText::new(if state.gpu {
                    format!(
                        "GPU · frame {:.1} ms / submit {:.1} ms",
                        state.frame_ms, state.physics_ms
                    )
                } else {
                    format!(
                        "Frame {:.1} ms / physics {:.1} ms / draw {:.1} ms",
                        state.frame_ms, state.physics_ms, state.drawing_ms
                    )
                })
                .size(10.0)
                .color(MUTED),
            );
            if state.clock.dropped_time > 0.1 {
                ui.label(
                    RichText::new(format!(
                        "Lost simulation time: {:.2}s total",
                        state.clock.dropped_time
                    ))
                    .size(10.0)
                    .color(GOLD),
                );
            }
        });
}

fn draw_vessel(ui: &egui::Ui, ctx: &egui::Context, state: &mut Art, center: Pos2, radius: f32) {
    let painter = ui.painter();
    painter.circle_filled(
        center + egui::vec2(0.0, 12.0),
        radius + 22.0,
        Color32::from_black_alpha(50),
    );
    painter.circle_filled(center, radius + 17.0, Color32::from_rgb(43, 54, 57));
    painter.circle_stroke(
        center,
        radius + 17.0,
        Stroke::new(1.0, Color32::from_rgb(87, 98, 96)),
    );
    painter.circle_stroke(
        center,
        radius + 10.0,
        Stroke::new(8.0, Color32::from_rgb(137, 123, 92)),
    );
    painter.circle_stroke(
        center,
        radius + 10.0,
        Stroke::new(1.0, Color32::from_rgb(201, 181, 132)),
    );
    painter.circle_filled(center, radius + 4.0, Color32::from_rgb(7, 17, 23));
    painter.circle_filled(center, radius, Color32::from_rgb(14, 31, 40));
    if let Some(texture) = state.gpu_texture {
        painter.image(
            texture,
            Rect::from_center_size(center, egui::Vec2::splat(radius * 2.0)),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        if !state.paused || state.field.texture.is_none() || state.field.dirty {
            state.field.update(ctx, &state.simulation);
        }
        if let Some(texture) = &state.field.texture {
            painter.image(
                texture.id(),
                Rect::from_center_size(center, egui::Vec2::splat(radius * 2.0)),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let mut mesh = egui::Mesh::default();
        mesh.reserve_vertices(state.simulation.particles.len() * 9);
        mesh.reserve_triangles(state.simulation.particles.len() * 8);
        for (i, p) in state.simulation.particles.iter().enumerate() {
            let material = &state.simulation.materials[p.material];
            if material.phase != Phase::Solid {
                continue;
            }
            let pos = center + egui::vec2(p.position.x * radius, -p.position.y * radius);
            let r = (p.radius * radius * 0.96).max(0.5);
            let variation = 0.9 + (i % 19) as f32 * 0.011;
            let color = |shade: f32| {
                Color32::from_rgb(
                    (material.color[0] as f32 * shade).min(255.0) as u8,
                    (material.color[1] as f32 * shade).min(255.0) as u8,
                    (material.color[2] as f32 * shade).min(255.0) as u8,
                )
            };
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(
                pos - egui::vec2(r * 0.15, r * 0.15),
                color(variation * 1.08),
            );
            for side in 0..8 {
                let a = side as f32 * std::f32::consts::TAU / 8.0;
                mesh.colored_vertex(
                    pos + egui::vec2(a.cos() * r, a.sin() * r),
                    color(variation * (0.75 - 0.1 * a.sin())),
                );
            }
            for side in 0..8 {
                mesh.add_triangle(base, base + 1 + side, base + 1 + (side + 1) % 8);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
    }
    painter.circle_stroke(
        center,
        radius + 1.0,
        Stroke::new(2.0, Color32::from_rgb(5, 15, 21)),
    );
    for i in 0..60 {
        let a = state.simulation.angle + i as f32 * std::f32::consts::TAU / 60.0;
        let d = egui::vec2(a.cos(), -a.sin());
        let length = if i % 5 == 0 { 5.0 } else { 2.0 };
        painter.line_segment(
            [
                center + d * (radius + 19.0),
                center + d * (radius + 19.0 + length),
            ],
            Stroke::new(1.0, Color32::from_rgb(85, 99, 101)),
        );
    }
    let a = state.simulation.angle + std::f32::consts::FRAC_PI_2;
    let marker = center + egui::vec2(a.cos(), -a.sin()) * (radius + 10.0);
    painter.circle_filled(marker, 4.0, CREAM);
    painter.circle_stroke(marker, 6.0, Stroke::new(1.0, GOLD));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gas_colour_changes_visible_surface_pixels() {
        let mut recipe = Recipe::balanced();
        recipe.budget = 40;
        let mut sim = particles::simulation::Simulation::new(&recipe).unwrap();
        sim.particles.truncate(1);
        sim.particles[0].material = 2;
        sim.particles[0].position = glam::Vec2::ZERO;
        sim.particles[0].radius = 0.15;
        sim.materials[2].color = [220, 40, 60];
        let mut field = FluidField::new();
        let ctx = egui::Context::default();
        field.update(&ctx, &sim);
        let before = field.image.pixels[FIELD_SIZE / 2 * FIELD_SIZE + FIELD_SIZE / 2];
        assert!(
            before.a() > 0 && before.r() > before.g() + 80,
            "gas tint was invisible: {before:?}"
        );
        sim.materials[2].color = [20, 200, 80];
        field.update(&ctx, &sim);
        let after = field.image.pixels[FIELD_SIZE / 2 * FIELD_SIZE + FIELD_SIZE / 2];
        assert!(after.g() > after.r() + 80);
    }
}

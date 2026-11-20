//! Immutable local-space geometry shared by physics and drawing.
use glam::Vec2;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::sync::Arc;

pub const MAP_SIZE: usize = 512;
pub const MAP_EXTENT: f32 = 1.1;
pub const DEFAULT_NECK_WIDTH_PERCENT: u16 = 100;
pub const HOURGLASS_NECK_WIDTH_RANGE: std::ops::RangeInclusive<u16> = 5..=250;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum VesselKind {
    #[default]
    Circle,
    Square,
    Hourglass,
    Cascade,
    RockGarden,
}
impl VesselKind {
    pub const ALL: [Self; 5] = [
        Self::Circle,
        Self::Square,
        Self::Hourglass,
        Self::Cascade,
        Self::RockGarden,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Circle => "Circle",
            Self::Square => "Square",
            Self::Hourglass => "Hourglass",
            Self::Cascade => "Cascade shelves",
            Self::RockGarden => "Rock garden",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Circle => "circle",
            Self::Square => "square",
            Self::Hourglass => "hourglass",
            Self::Cascade => "cascade",
            Self::RockGarden => "garden",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.key() == key)
    }
}
#[derive(Clone, Debug)]
pub struct Vessel {
    pub kind: VesselKind,
    /// Signed clearance, inward normal x/y, reserved. Positive means free space.
    pub map: Vec<[f32; 4]>,
    /// Segment endpoints, then radius/kind/reserved, also uploaded for exact visibility.
    pub barriers: Vec<[f32; 8]>,
    map_size: usize,
    area: f32,
    upper_area: f32,
}
fn rotate(p: Vec2, angle: f32) -> Vec2 {
    Vec2::from_angle(angle).rotate(p)
}
fn rounded_box(p: Vec2, half: Vec2, round: f32) -> f32 {
    let q = p.abs() - half + Vec2::splat(round);
    -(q.max(Vec2::ZERO).length() + q.max_element().min(0.) - round)
}
fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let v = b - a;
    (p - a - v * ((p - a).dot(v) / v.length_squared()).clamp(0., 1.)).length()
}
fn polygon(p: Vec2, points: &[Vec2]) -> f32 {
    let mut inside = false;
    let mut distance = f32::INFINITY;
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        distance = distance.min(segment_distance(p, *a, *b));
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    if inside {
        distance
    } else {
        -distance
    }
}
fn convex_outline(mut points: Vec<Vec2>) -> Vec<Vec2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let mut hull: Vec<Vec2> = Vec::new();
    for p in &points {
        while hull.len() >= 2
            && (hull[hull.len() - 1] - hull[hull.len() - 2]).perp_dot(*p - hull[hull.len() - 1])
                <= 0.
        {
            hull.pop();
        }
        hull.push(*p);
    }
    let lower = hull.len();
    for p in points.iter().rev().skip(1) {
        while hull.len() > lower
            && (hull[hull.len() - 1] - hull[hull.len() - 2]).perp_dot(*p - hull[hull.len() - 1])
                <= 0.
        {
            hull.pop();
        }
        hull.push(*p);
    }
    hull.pop();
    hull
}
fn point_segment_squared(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let v = b - a;
    (p - a - v * ((p - a).dot(v) / v.length_squared().max(1e-12)).clamp(0., 1.)).length_squared()
}
fn segments_intersect(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let r = b - a;
    let s = d - c;
    let denominator = r.perp_dot(s);
    if denominator.abs() < 1e-12 {
        return false;
    }
    let t = (c - a).perp_dot(s) / denominator;
    let u = (c - a).perp_dot(r) / denominator;
    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}
impl Vessel {
    pub fn new(kind: VesselKind, seed: u64) -> Arc<Self> {
        Self::with_neck_width(kind, seed, DEFAULT_NECK_WIDTH_PERCENT)
    }

    /// Set the Hourglass opening as a percentage of its original width, clamped
    /// to the supported range. Other vessel shapes ignore this setting.
    pub fn with_neck_width(kind: VesselKind, seed: u64, neck_width_percent: u16) -> Arc<Self> {
        type Cache = std::collections::HashMap<(VesselKind, u64, u16), std::sync::Weak<Vessel>>;
        static CACHE: std::sync::OnceLock<std::sync::Mutex<Cache>> = std::sync::OnceLock::new();
        let seed = if kind == VesselKind::RockGarden {
            seed
        } else {
            0
        };
        let neck_width_percent = if kind == VesselKind::Hourglass {
            neck_width_percent.clamp(
                *HOURGLASS_NECK_WIDTH_RANGE.start(),
                *HOURGLASS_NECK_WIDTH_RANGE.end(),
            )
        } else {
            DEFAULT_NECK_WIDTH_PERCENT
        };
        let key = (kind, seed, neck_width_percent);
        let cache = CACHE.get_or_init(Default::default);
        if let Some(v) = cache
            .lock()
            .unwrap()
            .get(&key)
            .and_then(std::sync::Weak::upgrade)
        {
            return v;
        }
        let vessel = Self::build(kind, seed, neck_width_percent);
        let mut entries = cache.lock().unwrap();
        if let Some(existing) = entries.get(&key).and_then(std::sync::Weak::upgrade) {
            return existing;
        }
        entries.retain(|_, v| v.strong_count() > 0);
        entries.insert(key, Arc::downgrade(&vessel));
        vessel
    }
    fn build(kind: VesselKind, seed: u64, neck_width_percent: u16) -> Arc<Self> {
        if kind == VesselKind::Circle {
            return Arc::new(Self {
                kind,
                map: vec![[0.; 4]],
                barriers: vec![],
                map_size: 1,
                area: std::f32::consts::PI,
                upper_area: std::f32::consts::PI * 0.5,
            });
        }
        let mut rng = StdRng::seed_from_u64(seed);
        let outline: Vec<Vec2> = if kind == VesselKind::Hourglass {
            let neck = 0.06 * (f32::from(neck_width_percent) / 100.);
            [
                (-0.58, 0.80),
                (0.58, 0.80),
                (0.58, 0.62),
                (neck, 0.08),
                (neck, -0.08),
                (0.58, -0.62),
                (0.58, -0.80),
                (-0.58, -0.80),
                (-0.58, -0.62),
                (-neck, -0.08),
                (-neck, 0.08),
                (-0.58, 0.62),
            ]
            .into_iter()
            .map(|(x, y)| Vec2::new(x, y))
            .collect()
        } else if kind == VesselKind::RockGarden {
            (0..12)
                .map(|i| {
                    Vec2::from_angle(i as f32 * std::f32::consts::TAU / 12.)
                        * rng.gen_range(0.84..0.95)
                })
                .collect()
        } else {
            vec![]
        };
        let outline = if kind == VesselKind::RockGarden {
            convex_outline(outline)
        } else {
            outline
        };
        let pegs: Vec<(Vec2, f32)> = [
            (-0.32, 0.40, 0.10),
            (0.31, 0.25, 0.12),
            (-0.28, -0.18, 0.10),
            (0.27, -0.42, 0.13),
        ]
        .into_iter()
        .map(|(x, y, r)| {
            (
                Vec2::new(
                    x + rng.gen_range(-0.035..0.035),
                    y + rng.gen_range(-0.035..0.035),
                ),
                r,
            )
        })
        .collect();
        let shelves = [
            (Vec2::new(-0.66, 0.38), Vec2::new(0.21, 0.38)),
            (Vec2::new(-0.21, -0.02), Vec2::new(0.66, -0.02)),
            (Vec2::new(-0.66, -0.42), Vec2::new(0.16, -0.42)),
        ];
        let mut barriers = Vec::new();
        for (a, b) in outline
            .iter()
            .zip(outline.iter().cycle().skip(1))
            .take(outline.len())
        {
            barriers.push([a.x, a.y, b.x, b.y, 0., 1., 0., 0.]);
        }
        if kind == VesselKind::Cascade {
            for (a, b) in shelves {
                barriers.push([a.x, a.y, b.x, b.y, 0.045, 2., 0., 0.]);
            }
        }
        if kind == VesselKind::RockGarden {
            for (c, r) in &pegs {
                barriers.push([c.x, c.y, c.x, c.y, *r, 3., 0., 0.]);
            }
        }
        let distance = |p: Vec2| {
            let mut d = match kind {
                VesselKind::Square => rounded_box(p, Vec2::splat(0.70), 0.07),
                VesselKind::Cascade => rounded_box(p, Vec2::new(0.64, 0.75), 0.10),
                _ => polygon(p, &outline),
            };
            if kind == VesselKind::Cascade {
                for (a, b) in shelves {
                    d = d.min(segment_distance(p, a, b) - 0.045);
                }
            } else if kind == VesselKind::RockGarden {
                for (c, r) in &pegs {
                    d = d.min(p.distance(*c) - r);
                }
            }
            d
        };
        // Sample the centerline of narrow openings directly. With an even grid,
        // interpolation between its two central columns understates clearance.
        let map_size = if kind == VesselKind::Hourglass && neck_width_percent < 50 {
            MAP_SIZE + 1
        } else {
            MAP_SIZE
        };
        let mut map = Vec::with_capacity(map_size * map_size);
        let mut area = 0.;
        let mut upper_area = 0.;
        let cell = 2. * MAP_EXTENT / (map_size - 1) as f32;
        for y in 0..map_size {
            for x in 0..map_size {
                let p = Vec2::new(x as f32, y as f32) * cell - Vec2::splat(MAP_EXTENT);
                let d = distance(p);
                let eps = 0.0005;
                let n = Vec2::new(
                    distance(p + Vec2::X * eps) - distance(p - Vec2::X * eps),
                    distance(p + Vec2::Y * eps) - distance(p - Vec2::Y * eps),
                )
                .normalize_or_zero();
                map.push([d, n.x, n.y, 0.]);
                if d > 0. {
                    area += cell * cell;
                    if p.y > 0.10 {
                        upper_area += cell * cell;
                    }
                }
            }
        }
        Arc::new(Self {
            kind,
            map,
            barriers,
            map_size,
            area,
            upper_area,
        })
    }
    pub(crate) fn map_size(&self) -> usize {
        self.map_size
    }
    pub fn area(&self, upper_only: bool) -> f32 {
        if upper_only {
            self.upper_area
        } else {
            self.area
        }
    }
    pub fn sample(&self, p: Vec2, angle: f32) -> (f32, Vec2) {
        if self.kind == VesselKind::Circle {
            return (1. - p.length(), -p.normalize_or(Vec2::Y));
        }
        let local = rotate(p, -angle);
        let size = self.map_size;
        let uv = ((local / (2. * MAP_EXTENT) + Vec2::splat(0.5)) * (size - 1) as f32)
            .clamp(Vec2::ZERO, Vec2::splat((size - 1) as f32));
        let x = (uv.x as usize).min(size - 2);
        let y = (uv.y as usize).min(size - 2);
        let t = uv - Vec2::new(x as f32, y as f32);
        let mut sample = [0.; 3];
        for (c, value) in sample.iter_mut().enumerate() {
            let a = self.map[y * size + x][c] * (1. - t.x) + self.map[y * size + x + 1][c] * t.x;
            let b = self.map[(y + 1) * size + x][c] * (1. - t.x)
                + self.map[(y + 1) * size + x + 1][c] * t.x;
            *value = a * (1. - t.y) + b * t.y;
        }
        (
            sample[0],
            rotate(Vec2::new(sample[1], sample[2]).normalize_or(Vec2::Y), angle),
        )
    }
    pub fn clearance(&self, p: Vec2, angle: f32) -> f32 {
        self.sample(p, angle).0
    }
    /// Check against the same defining edges/rounded primitives as the distance map.
    pub fn segment_clear(&self, a: Vec2, b: Vec2, angle: f32) -> bool {
        let a = rotate(a, -angle);
        let b = rotate(b, -angle);
        let lo = a.min(b);
        let hi = a.max(b);
        for wall in &self.barriers {
            let c = Vec2::new(wall[0], wall[1]);
            let d = Vec2::new(wall[2], wall[3]);
            let radius = wall[4];
            if (hi.cmplt(c.min(d) - Vec2::splat(radius))).any()
                || (lo.cmpgt(c.max(d) + Vec2::splat(radius))).any()
            {
                continue;
            }
            let intersects = segments_intersect(a, b, c, d);
            if wall[5] == 1. {
                if intersects {
                    return false;
                }
            } else if wall[5] == 3. {
                if point_segment_squared(c, a, b) <= radius * radius {
                    return false;
                }
            } else if intersects
                || point_segment_squared(a, c, d)
                    .min(point_segment_squared(b, c, d))
                    .min(point_segment_squared(c, a, b))
                    .min(point_segment_squared(d, a, b))
                    <= radius * radius
            {
                return false;
            }
        }
        true
    }
    pub fn project(&self, mut p: Vec2, radius: f32, angle: f32) -> Vec2 {
        if self.kind == VesselKind::Circle {
            return p.clamp_length_max(1. - radius);
        }
        for _ in 0..12 {
            let (d, n) = self.sample(p, angle);
            if d >= radius {
                break;
            }
            p += n * (radius - d + 0.00001);
        }
        p
    }
}

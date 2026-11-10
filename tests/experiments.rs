use particles::material::{Material, MaterialProperties, Phase, Preset, Recipe};
#[test]
fn percentages_are_exact_zero_and_one_hundred_are_real_endpoints() {
    for i in 0..6 {
        let mut r = Recipe::balanced();
        r.set_percentage(i, 100.).unwrap();
        assert_eq!(r.materials[i].share, 100.);
        let counts = r.counts().unwrap();
        assert_eq!(counts[i], r.budget);
        assert_eq!(counts.iter().sum::<usize>(), r.budget);
        for (j, m) in r.materials.iter().enumerate() {
            if j != i {
                assert_eq!(m.share, 0.);
                assert_eq!(counts[j], 0);
            }
        }
        r.set_percentage(i, 0.).unwrap();
        assert_eq!(r.counts().unwrap()[i], 0);
        assert!((r.materials.iter().map(|m| m.share).sum::<f32>() - 100.).abs() < 0.001);
    }
}
#[test]
fn percentage_rebalance_preserves_disabled_types_and_requested_value() {
    let mut r = Recipe::balanced();
    r.set_percentage(1, 0.).unwrap();
    r.set_percentage(0, 37.5).unwrap();
    assert_eq!(r.materials[0].share, 37.5);
    assert_eq!(r.materials[1].share, 0.);
    assert!((r.materials.iter().map(|m| m.share).sum::<f32>() - 100.).abs() < 0.001);
    let old = r.materials.clone();
    assert!(r.set_percentage(0, f32::NAN).is_err());
    assert_eq!(r.materials, old);
}
#[test]
fn thermal_properties_derive_from_reference_without_compounding() {
    let mut r = Recipe::balanced();
    let reference = r.materials.clone();
    r.temperature_c = 100.;
    let hot = r.effective_materials();
    assert!(hot[0].viscosity < reference[0].viscosity);
    assert!(hot[0].density < reference[0].density);
    assert!(hot[2].neighbor_target() < 1.65);
    assert!(hot[2].thermal_motion > reference[2].thermal_motion);
    assert_eq!(hot, r.effective_materials());
    assert_eq!(reference, r.materials);
    r.temperature_c = 20.;
    assert_eq!(r.effective_materials(), reference);
    r.temperature_c = -20.;
    assert!(r.effective_materials()[0].viscosity > reference[0].viscosity);
}
#[test]
fn presets_are_valid_and_cover_distinct_experiments() {
    for p in Preset::ALL {
        let r = p.recipe();
        r.validate().unwrap();
        assert!((r.materials.iter().map(|m| m.share).sum::<f32>() - 100.).abs() < 0.001);
        assert_eq!(r.counts().unwrap().iter().sum::<usize>(), r.budget);
    }
    let dry = Preset::DryGrains.recipe();
    assert!(dry
        .materials
        .iter()
        .filter(|m| m.share > 0.)
        .all(|m| m.phase == Phase::Solid));
    let pure = Preset::PureWater.recipe();
    assert_eq!(pure.counts().unwrap()[0], pure.budget);
}
#[test]
fn invalid_temperature_or_physical_properties_are_rejected() {
    let mut r = Recipe::balanced();
    r.temperature_c = f32::NAN;
    assert!(r.validate().is_err());
    r.temperature_c = 200.;
    assert!(r.validate().is_err());
    r.temperature_c = 20.;
    r.materials[0].compression_resistance = 0.;
    assert!(r.validate().is_err());
    r.materials[0].compression_resistance = 1.;
    r.materials[0].thermal_motion = f32::INFINITY;
    assert!(r.validate().is_err());
}
#[test]
fn custom_material_properties_are_independent() {
    let mut r = Recipe::balanced();
    r.materials.push(Material::new(
        "Custom",
        Phase::Liquid,
        0.,
        MaterialProperties::for_phase(Phase::Liquid),
    ));
    let i = r.materials.len() - 1;
    r.set_percentage(i, 100.).unwrap();
    assert_eq!(r.counts().unwrap()[i], r.budget);
    r.materials[i].viscosity = 5.;
    assert_ne!(r.materials[i].viscosity, r.materials[0].viscosity);
}
#[test]
fn thermal_noise_is_zero_mean_unit_variance_and_reproducible() {
    let mut mean = glam::Vec2::ZERO;
    let mut square = glam::Vec2::ZERO;
    for i in 0..10000 {
        let n = particles::thermal::noise(i, 17);
        assert_eq!(n, particles::thermal::noise(i, 17));
        mean += n;
        square += n * n;
    }
    mean /= 10000.;
    square /= 10000.;
    assert!(mean.abs().max_element() < 0.03);
    assert!((square - glam::Vec2::ONE).abs().max_element() < 0.05);
}

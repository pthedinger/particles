use particles::material::{Phase, Recipe};

#[test]
fn rejects_empty_or_nonfinite_mixtures() {
    let mut recipe = Recipe::balanced();
    for m in &mut recipe.materials {
        m.share = 0.0;
    }
    assert!(recipe.validate().is_err());
    recipe.materials[0].share = f32::NAN;
    assert!(recipe.validate().is_err());
}

#[test]
fn area_shares_account_for_different_grain_sizes() {
    let mut recipe = Recipe::balanced();
    recipe.materials.truncate(2);
    recipe.budget = 1000;
    for m in &mut recipe.materials {
        m.phase = Phase::Solid;
        m.share = 1.0;
    }
    recipe.materials[0].radius = 1.0;
    recipe.materials[1].radius = 2.0;
    assert_eq!(recipe.counts().unwrap(), vec![800, 200]);
}

#[test]
fn includes_small_positive_shares_and_conserves_budget() {
    let mut recipe = Recipe::balanced();
    recipe.budget = 100;
    recipe.materials[5].share = 0.00001;
    let counts = recipe.counts().unwrap();
    assert_eq!(counts.iter().sum::<usize>(), 100);
    assert!(counts.iter().all(|n| *n >= 1));
    assert_eq!(counts, recipe.counts().unwrap());
}

#[test]
fn rejects_insufficient_resolution_and_invalid_properties() {
    let mut recipe = Recipe::balanced();
    recipe.budget = 2;
    assert!(recipe.counts().is_err());
    recipe.budget = 100;
    recipe.materials[3].density = 0.0;
    assert!(recipe.validate().is_err());
    recipe.materials[3].density = 2.0;
    recipe.materials[3].radius = f32::INFINITY;
    assert!(recipe.validate().is_err());
}

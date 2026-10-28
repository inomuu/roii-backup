#[test]
fn a_save_with_the_retired_band_clears_key_still_loads() {
    let mut s = sim_core::state::GameState::new(1, 8, 8);
    s.set_gil_for_test(77);
    let mut v: serde_json::Value = serde_json::to_value(&s).unwrap();
    // Simulate a pre-0.0.291 save that still carries the removed map.
    v.as_object_mut().unwrap().insert("band_clears".into(), serde_json::json!({"5": 2}));
    let back: sim_core::state::GameState =
        serde_json::from_value(v).expect("an old save with the retired key must still load");
    assert_eq!(back.gil(), 77);
}

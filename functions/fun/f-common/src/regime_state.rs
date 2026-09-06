pub fn f_regime_state(z_rate: f64, z_frag: f64, total_depth: f64, regime_onset: bool) -> i64 {
    if z_rate > 2.5 && z_frag < -3.5 && total_depth > 5.0 {
        return 10;
    }

    if z_rate > 2.0 && z_frag < -3.0 && total_depth > 5.0 {
        return 20;
    }

    if z_rate < 1.5 && z_frag < -4.0 && total_depth < 1.0 {
        return 30;
    }

    if regime_onset == true {
        return 1;
    }

    0
}

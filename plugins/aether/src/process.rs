//! Aether process path split for profile/isolation.
//!
//! ponytail: same-crate module split only — no behavior change.

use aura::prelude::*;
use aura_dsp::fx::FtzDazGuard;

use crate::{AetherDspState, AetherParams, CF_DELAY_MAX, NUM_BANDS};

/// Steal a scratch Vec from DSP state (capacity stays), grow if the host block is bigger.
fn take_grow(slot: &mut Vec<f32>, n: usize) -> Vec<f32> {
    let mut v = std::mem::take(slot);
    if v.len() < n {
        v.resize(n, 0.0);
    }
    v
}

pub(crate) fn run(
    state: &mut AetherDspState,
    params: &AetherParams,
    buffer: &mut AudioBuffer<'_, f32>,
) -> ProcessStatus {
    let _ftz = FtzDazGuard::new();

    if buffer.num_inputs() < 2 || buffer.num_outputs() < 2 {
        return ProcessStatus::Continue;
    }
    // ponytail: never process with empty delay (would panic on % 0)
    if state.cf_delay_l.len() < 2 || state.cf_delay_r.len() < 2 {
        if state.cf_delay_l.len() != CF_DELAY_MAX {
            state.cf_delay_l.resize(CF_DELAY_MAX, 0.0);
        }
        if state.cf_delay_r.len() != CF_DELAY_MAX {
            state.cf_delay_r.resize(CF_DELAY_MAX, 0.0);
        }
        state.cf_delay_pos = 0;
    }
    let delay_len = state.cf_delay_l.len();
    let sr = state.sample_rate.max(1.0);
    let num_samples = buffer.num_samples();
    if num_samples == 0 {
        return ProcessStatus::Continue;
    }

    let mut in0 = take_grow(&mut state.in0, num_samples);
    let mut in1 = take_grow(&mut state.in1, num_samples);
    let mut out0 = take_grow(&mut state.out0, num_samples);
    let mut out1 = take_grow(&mut state.out1, num_samples);
    in0[..num_samples].copy_from_slice(buffer.input(0));
    in1[..num_samples].copy_from_slice(buffer.input(1));

    let mut in_peak = 0.0f32;
    for i in 0..num_samples {
        in_peak = in_peak.max(in0[i].abs()).max(in1[i].abs());
    }
    let in_db = if in_peak < 1e-9 {
        -90.0
    } else {
        20.0 * in_peak.log10()
    };
    params
        .shared
        .input_peak
        .store(in_db, std::sync::atomic::Ordering::Release);

    let bypass = params.bypass.value();
    if bypass != state.was_bypass {
        if state.was_bypass && !bypass {
            for b in state.eq_l.iter_mut().chain(state.eq_r.iter_mut()) {
                b.reset();
            }
            state.cf_delay_l.fill(0.0);
            state.cf_delay_r.fill(0.0);
            state.cf_lp_l = 0.0;
            state.cf_lp_r = 0.0;
            state.cf_delay_pos = 0;
        }
        state.was_bypass = bypass;
    }

    if bypass {
        buffer.output(0).copy_from_slice(&in0[..num_samples]);
        buffer.output(1).copy_from_slice(&in1[..num_samples]);
        state.in0 = in0;
        state.in1 = in1;
        state.out0 = out0;
        state.out1 = out1;
        return ProcessStatus::Continue;
    }

    state.update_eq_coeffs(params);
    let blend = params.blend.raw_target() as f32 / 100.0;
    let (itd_ms, cut_mul, feed_mul) = match params.cf_realism.value_i32() {
        1 => (0.32, 0.85, 1.05),
        2 => (0.45, 0.70, 1.15),
        _ => (0.22, 1.00, 1.00),
    };
    let cf_mix = ((params.cf_amount.raw_target() as f32 / 100.0) * 0.5 * feed_mul).min(0.75);
    let cf_norm = ((params.cf_angle.raw_target() as f32 - 30.0) / 45.0).clamp(0.0, 1.0);
    let cf_fc = (700.0 + cf_norm * 1300.0) * cut_mul;
    let cf_a = 1.0 - (-2.0 * std::f32::consts::PI * cf_fc / sr).exp();
    let max_delay = delay_len - 1;
    let delay_samples = ((itd_ms * 0.001 * sr).round() as usize).min(max_delay);
    if state.cf_delay_pos >= delay_len {
        state.cf_delay_pos = 0;
    }
    let g = 10.0_f32.powf(params.gain.value() / 20.0);

    for i in 0..num_samples {
        let in_l = in0[i];
        let in_r = in1[i];

        let mut eq_l = in_l;
        let mut eq_r = in_r;
        for b in 0..NUM_BANDS {
            if state.eq_on[b] {
                eq_l = state.eq_l[b].process(eq_l);
                eq_r = state.eq_r[b].process(eq_r);
            }
        }
        let h_l = in_l + (eq_l - in_l) * blend;
        let h_r = in_r + (eq_r - in_r) * blend;

        let wp = state.cf_delay_pos;
        state.cf_delay_l[wp] = h_l;
        state.cf_delay_r[wp] = h_r;
        let rp = (wp + delay_len - delay_samples) % delay_len;
        let del_l = state.cf_delay_l[rp];
        let del_r = state.cf_delay_r[rp];
        state.cf_delay_pos = (wp + 1) % delay_len;

        state.cf_lp_l += cf_a * (del_r - state.cf_lp_l);
        state.cf_lp_r += cf_a * (del_l - state.cf_lp_r);
        let cf_l = h_l + state.cf_lp_l * cf_mix;
        let cf_r = h_r + state.cf_lp_r * cf_mix;

        out0[i] = cf_l * g;
        out1[i] = cf_r * g;
    }

    buffer.output(0).copy_from_slice(&out0[..num_samples]);
    buffer.output(1).copy_from_slice(&out1[..num_samples]);
    state.in0 = in0;
    state.in1 = in1;
    state.out0 = out0;
    state.out1 = out1;

    ProcessStatus::Continue
}

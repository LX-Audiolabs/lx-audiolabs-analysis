//! Lucent Relay process path split for profile/isolation.
//!
//! ponytail: same-crate module split only — no behavior change.

use aura::prelude::*;
use aura_dsp::analysis::*;
use aura_dsp::fx::FtzDazGuard;
use lx_shm::*;

use crate::{FFT_SIZE, LucentRelayDspState, LucentRelayParams, try_read_persisted};

fn take_grow(slot: &mut Vec<f32>, n: usize) -> Vec<f32> {
    let mut v = std::mem::take(slot);
    if v.len() < n {
        v.resize(n, 0.0);
    }
    v
}

pub(crate) fn run(
    state: &mut LucentRelayDspState,
    params: &LucentRelayParams,
    buffer: &mut AudioBuffer<'_, f32>,
) -> ProcessStatus {
    let _ftz = FtzDazGuard::new();

    let now_ms = now_ms();
    if state.claimed_slot.is_none() {
        state.claim_slot();
    }

    let mut target_changed = false;
    if let Some((n, t)) = try_read_persisted(params) {
        if n != state.cached_name {
            state.cached_name = n;
        }
        target_changed = t != state.cached_target;
        if target_changed {
            state.cached_target = t;
        }
    }
    state.resolve_target(params, now_ms, target_changed);

    let channels = buffer.num_inputs().min(buffer.num_outputs());
    let n_samples = buffer.num_samples();
    if channels == 0 || n_samples == 0 {
        return ProcessStatus::Continue;
    }
    let mut in0 = take_grow(&mut state.in0, n_samples);
    let mut in1 = take_grow(&mut state.in1, n_samples);
    in0[..n_samples].copy_from_slice(buffer.input(0));
    if channels > 1 {
        in1[..n_samples].copy_from_slice(buffer.input(1));
    } else {
        in1[..n_samples].copy_from_slice(&in0[..n_samples]);
    }
    buffer.output(0).copy_from_slice(&in0[..n_samples]);
    if channels > 1 {
        buffer.output(1).copy_from_slice(&in1[..n_samples]);
    }

    {
        for i in 0..n_samples {
            let l = in0[i];
            let r = in1[i];
            state.fft_input[state.fft_write_pos] = (l + r) * 0.5;
            state.fft_write_pos += 1;

            if state.fft_write_pos >= FFT_SIZE {
                state.fft_write_pos = 0;
                for (d, (s, w)) in state
                    .fft_windowed
                    .iter_mut()
                    .zip(state.fft_input.iter().zip(state.fft_hann.iter()))
                {
                    *d = s * w;
                }
                if state
                    .fft_fwd
                    .process(&mut state.fft_windowed, &mut state.fft_output)
                    .is_ok()
                {
                    compute_spectrum_bins(
                        &state.fft_output,
                        &mut state.fft_bins,
                        FFT_SIZE,
                        state.sample_rate,
                    );
                    state.publish_fft(now_ms);
                }
            }
        }
    }
    state.in0 = in0;
    state.in1 = in1;
    ProcessStatus::Continue
}

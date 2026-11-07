//! A radix-2 FFT for the spectrogram, written here rather than taken as a
//! dependency (none is approved).
//!
//! Only IEEE-754 basic operations (add, multiply), never `sin`, `cos` or
//! `log`, whose results vary in the last bit between platforms' math
//! libraries: the twiddle table is built by rotating by one step whose cosine
//! and sine are literals, so the same input gives the same bits everywhere.

/// The transform size.
pub(crate) const N: usize = 1024;
/// cos and sin of 2π/1024, to the nearest f64 (Python's `math`, checked by a
/// test against Rust's).
const STEP_COS: f64 = 0.999_981_175_282_601_1;
const STEP_SIN: f64 = 0.006_135_884_649_154_475;

/// cos(2πk/N) and sin(2πk/N) for k in 0..N.
pub(crate) struct Twiddles {
    pub cos: Vec<f64>,
    pub sin: Vec<f64>,
}

impl Twiddles {
    pub fn new() -> Twiddles {
        let (mut cos, mut sin) = (Vec::with_capacity(N), Vec::with_capacity(N));
        let (mut c, mut s) = (1.0f64, 0.0f64);
        for _ in 0..N {
            cos.push(c);
            sin.push(s);
            (c, s) = (c * STEP_COS - s * STEP_SIN, s * STEP_COS + c * STEP_SIN);
        }
        Twiddles { cos, sin }
    }

    /// The Hann window's weight for sample `n` of `N`.
    pub fn hann(&self, n: usize) -> f64 {
        0.5 - 0.5 * self.cos[n]
    }
}

/// The squared magnitude of bins 0..N/2 of `input`'s transform (N samples).
pub(crate) fn power_spectrum(t: &Twiddles, input: &[f64]) -> Vec<f64> {
    assert_eq!(input.len(), N);
    let bits = N.trailing_zeros();
    let mut re = vec![0.0; N];
    let mut im = vec![0.0; N];
    for (i, &x) in input.iter().enumerate() {
        re[i.reverse_bits() >> (usize::BITS - bits)] = x;
    }
    let mut size = 2;
    while size <= N {
        let half = size / 2;
        let stride = N / size;
        for start in (0..N).step_by(size) {
            for k in 0..half {
                // e^(-2πik/size) = cos - i·sin at index k·stride of the table.
                let (wr, wi) = (t.cos[k * stride], -t.sin[k * stride]);
                let (a, b) = (start + k, start + k + half);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        size *= 2;
    }
    (0..N / 2).map(|k| re[k] * re[k] + im[k] * im[k]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_step_literals_are_rusts_cos_and_sin_of_the_step() {
        let angle = 2.0 * std::f64::consts::PI / N as f64;
        assert!((angle.cos() - STEP_COS).abs() < 1e-16);
        assert!((angle.sin() - STEP_SIN).abs() < 1e-16);
    }

    #[test]
    fn the_twiddle_table_stays_on_the_circle() {
        let t = Twiddles::new();
        for k in [1, 256, 511, 1023] {
            let angle = 2.0 * std::f64::consts::PI * k as f64 / N as f64;
            assert!((t.cos[k] - angle.cos()).abs() < 1e-12, "cos {k}");
            assert!((t.sin[k] - angle.sin()).abs() < 1e-12, "sin {k}");
        }
    }

    #[test]
    fn a_cosine_on_bin_k_puts_its_power_in_bin_k() {
        let t = Twiddles::new();
        let k = 37;
        let input: Vec<f64> = (0..N).map(|n| t.cos[(k * n) % N]).collect();
        let power = power_spectrum(&t, &input);
        let peak = (0..N / 2)
            .max_by(|&a, &b| power[a].total_cmp(&power[b]))
            .unwrap();
        assert_eq!(peak, k);
        // A unit cosine on a bin has magnitude N/2 there.
        assert!((power[k].sqrt() - N as f64 / 2.0).abs() < 1e-9);
        assert!(power[k + 3] < 1e-12);
    }
}

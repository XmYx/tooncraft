//! In-place iterative radix-2 FFT (Cooley–Tukey) for power-of-two lengths.

pub(crate) fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    if n < 2 || !n.is_power_of_two() || im.len() != n {
        return;
    }
    // bit reversal
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f32::consts::TAU / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
                let t = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = t;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_tone_lands_in_its_bin() {
        let n = 256;
        let mut re: Vec<f32> = (0..n).map(|i| (std::f32::consts::TAU * 10.0 * i as f32 / n as f32).cos()).collect();
        let mut im = vec![0.0; n];
        super::fft(&mut re, &mut im);
        let mag: Vec<f32> = (0..n / 2).map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt()).collect();
        let best = mag.iter().enumerate().fold((0, 0.0f32), |b, (i, v)| if *v > b.1 { (i, *v) } else { b }).0;
        assert_eq!(best, 10);
        assert!((mag[10] - n as f32 / 2.0).abs() < 0.5);
    }
}

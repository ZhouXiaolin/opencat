// 1:1 port of faac/libfaac/fft.c
//
// Twiddle (cos / -sin) and bit-reversal tables, indexed by logm in
// [0, MAXLOGM]. The original lazily initializes per-slot; we eagerly
// compute all tables in new() since MAXLOGM = 9 makes the cost negligible.

const MAXLOGM: usize = 9;
const MAXLOGR: usize = 8;

pub struct FftTables {
    costbl: [Vec<f64>; MAXLOGM + 1],
    negsintbl: [Vec<f64>; MAXLOGM + 1],
    reordertbl: [Vec<u16>; MAXLOGM + 1],
}

impl FftTables {
    pub fn new() -> Self {
        let mut costbl = [(); MAXLOGM + 1].map(|_| Vec::new());
        let mut negsintbl = [(); MAXLOGM + 1].map(|_| Vec::new());
        let mut reordertbl = [(); MAXLOGM + 1].map(|_| Vec::new());
        for logm in 0..=MAXLOGM {
            let size = 1usize << logm;
            let mut ct = vec![0.0f64; size / 2];
            let mut st = vec![0.0f64; size / 2];
            for i in 0..(size >> 1) {
                let theta = 2.0 * std::f64::consts::PI * (i as f64) / (size as f64);
                ct[i] = theta.cos();
                st[i] = -theta.sin();
            }
            costbl[logm] = ct;
            negsintbl[logm] = st;

            let mut tbl = vec![0u16; size];
            for i in 0..size {
                let mut reversed = 0u32;
                let mut tmp = i as u32;
                for _ in 0..logm {
                    reversed = (reversed << 1) | (tmp & 1);
                    tmp >>= 1;
                }
                tbl[i] = reversed as u16;
            }
            reordertbl[logm] = tbl;
        }
        Self {
            costbl,
            negsintbl,
            reordertbl,
        }
    }

    fn reorder2(&self, xr: &mut [f64], xi: &mut [f64], logm: usize) {
        let size = 1usize << logm;
        let r = &self.reordertbl[logm];
        for i in 0..size {
            let j = r[i] as usize;
            if j <= i {
                continue;
            }
            xr.swap(i, j);
            xi.swap(i, j);
        }
    }

    pub fn fft(&self, xr: &mut [f64], xi: &mut [f64], logm: i32) {
        if logm > MAXLOGM as i32 {
            eprintln!("fft size too big ({})", logm);
            return;
        }
        if logm < 1 {
            return;
        }
        let logm = logm as usize;
        self.reorder2(xr, xi, logm);
        fft_proc(xr, xi, &self.costbl[logm], &self.negsintbl[logm], 1usize << logm);
    }

    // rfft(): real-input FFT. Caller passes one buffer of length 1<<logm
    // containing the real signal; output stores the real part in the first
    // half and the imaginary part in the second half.
    pub fn rfft(&self, x: &mut [f64], logm: i32) {
        if logm > MAXLOGR as i32 {
            eprintln!("rfft size too big ({})", logm);
            return;
        }
        let logm_u = logm as usize;
        let size = 1usize << logm_u;
        let mut xi = vec![0.0f64; size];
        self.fft(x, &mut xi, logm);
        let half = size >> 1;
        x[half..size].copy_from_slice(&xi[..half]);
    }
}

impl Default for FftTables {
    fn default() -> Self {
        Self::new()
    }
}

// fft_proc(): in-place radix-2 butterfly.
//   - stage 1 (step=1): twiddle = (1, 0), so multiplications are elided.
//   - stage 2 (step=2): twiddles are (1,0) and (0,-1); also unrolled.
//   - stages >= 3: standard loop using table-lookup twiddles.
fn fft_proc(xr: &mut [f64], xi: &mut [f64], refac: &[f64], imfac: &[f64], size: usize) {
    // First stage: step = 1
    {
        let mut pos = 0;
        while pos < size {
            let x1 = pos;
            let x2 = pos + 1;
            let v2r = xr[x2];
            let v2i = xi[x2];
            xr[x2] = xr[x1] - v2r;
            xr[x1] += v2r;
            xi[x2] = xi[x1] - v2i;
            xi[x1] += v2i;
            pos += 2;
        }
    }

    // Second stage: step = 2 (only if size >= 4)
    if size >= 4 {
        let mut pos = 0;
        while pos < size {
            // shift = 0: twiddle (1, 0)
            {
                let x1 = pos;
                let x2 = pos + 2;
                let v2r = xr[x2];
                let v2i = xi[x2];
                xr[x2] = xr[x1] - v2r;
                xr[x1] += v2r;
                xi[x2] = xi[x1] - v2i;
                xi[x1] += v2i;
            }
            // shift = 1: twiddle (0, -1)
            {
                let x1 = pos + 1;
                let x2 = pos + 3;
                let v2r = xi[x2];
                let v2i = -xr[x2];
                xr[x2] = xr[x1] - v2r;
                xr[x1] += v2r;
                xi[x2] = xi[x1] - v2i;
                xi[x1] += v2i;
            }
            pos += 4;
        }
    }

    // Remaining stages from step = 4. The C version uses running x1/x2
    // pointers that advance by `step` inside the shift loop and roll over to
    // the next butterfly group on the next pos; this is equivalent to taking
    // `x1_start = pos`.
    let mut estep = size >> 2;
    let mut step = 4usize;
    while step < size {
        estep >>= 1;
        let mut pos = 0;
        while pos < size {
            let x1_start = pos;
            let x2_start = pos + step;
            let mut exp = 0usize;
            for shift in 0..step {
                let x1 = x1_start + shift;
                let x2 = x2_start + shift;
                let v2r = xr[x2] * refac[exp] - xi[x2] * imfac[exp];
                let v2i = xr[x2] * imfac[exp] + xi[x2] * refac[exp];
                xr[x2] = xr[x1] - v2r;
                xr[x1] += v2r;
                xi[x2] = xi[x1] - v2i;
                xi[x1] += v2i;
                exp += estep;
            }
            pos += 2 * step;
        }
        step *= 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive_dft(xr: &[f64], xi: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let n = xr.len();
        let mut yr = vec![0.0f64; n];
        let mut yi = vec![0.0f64; n];
        for k in 0..n {
            for j in 0..n {
                let theta = -2.0 * std::f64::consts::PI * (k as f64) * (j as f64) / (n as f64);
                let c = theta.cos();
                let s = theta.sin();
                yr[k] += xr[j] * c - xi[j] * s;
                yi[k] += xr[j] * s + xi[j] * c;
            }
        }
        (yr, yi)
    }

    #[test]
    fn fft_matches_naive_dft() {
        // Compare against the textbook DFT for a few small sizes.
        for &logm in &[1i32, 2, 3, 4, 5, 6] {
            let n = 1usize << (logm as usize);
            let mut xr: Vec<f64> = (0..n).map(|i| (i as f64).sin()).collect();
            let mut xi: Vec<f64> = (0..n).map(|i| (i as f64 * 0.3).cos()).collect();
            let (rr, ri) = naive_dft(&xr, &xi);

            let tables = FftTables::new();
            tables.fft(&mut xr, &mut xi, logm);

            for i in 0..n {
                assert!(
                    (xr[i] - rr[i]).abs() < 1e-9,
                    "logm={}, i={}, xr={} vs {}",
                    logm,
                    i,
                    xr[i],
                    rr[i]
                );
                assert!(
                    (xi[i] - ri[i]).abs() < 1e-9,
                    "logm={}, i={}, xi={} vs {}",
                    logm,
                    i,
                    xi[i],
                    ri[i]
                );
            }
        }
    }
}

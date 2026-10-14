//! Reference implementation of one pseudo-spectral 2D Navier-Stokes step,
//! written so that it can be compiled three ways from the SAME source:
//!
//!   * native x86-64  (benchnative binary)
//!   * wasm32         (cdylib -> benchcore.wasm)
//!
//! and mirrored line-for-line by the plain-JS version in js-bench.html.
//! The algorithm deliberately matches C:/Users/kevin/sim/src/solver.rs:
//! vorticity form, radix-2 FFT with a transpose, Heun step, exponential
//! viscosity, 2/3 dealiasing.

use std::f64::consts::PI;

// ---------------------------------------------------------------- 1D / 2D FFT

pub struct Plan {
    pub n: usize,
    rev: Vec<u32>,
    cos: Vec<f64>,
    sin: Vec<f64>,
}

impl Plan {
    pub fn new(n: usize) -> Plan {
        let bits = n.trailing_zeros();
        let mut rev = vec![0u32; n];
        for i in 0..n {
            rev[i] = (i as u32).reverse_bits() >> (32 - bits);
        }
        let half = (n / 2).max(1);
        let mut cos = vec![0.0; half];
        let mut sin = vec![0.0; half];
        for i in 0..half {
            let a = -2.0 * PI * (i as f64) / (n as f64);
            cos[i] = a.cos();
            sin[i] = a.sin();
        }
        Plan { n, rev, cos, sin }
    }

    #[inline]
    pub fn fft1(&self, re: &mut [f64], im: &mut [f64], off: usize, inverse: bool) {
        let n = self.n;
        for i in 0..n {
            let j = self.rev[i] as usize;
            if j > i {
                re.swap(off + i, off + j);
                im.swap(off + i, off + j);
            }
        }
        let mut len = 2usize;
        while len <= n {
            let half = len >> 1;
            let step = n / len;
            let mut i = 0usize;
            while i < n {
                for k in 0..half {
                    let t = k * step;
                    let wr = self.cos[t];
                    let wi = if inverse { -self.sin[t] } else { self.sin[t] };
                    let a = off + i + k;
                    let b = a + half;
                    let xr = re[b] * wr - im[b] * wi;
                    let xi = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - xr;
                    im[b] = im[a] - xi;
                    re[a] += xr;
                    im[a] += xi;
                }
                i += len;
            }
            len <<= 1;
        }
    }
}

/// 2D transform: rows, transpose, rows, transpose back. Dimensions are powers
/// of two. `tre`/`tim` must be distinct buffers of n*n.
#[inline]
pub fn fft2(
    dre: &mut [f64],
    dim: &mut [f64],
    tre: &mut [f64],
    tim: &mut [f64],
    plan: &Plan,
    inverse: bool,
) {
    let n = plan.n;
    for y in 0..n {
        plan.fft1(dre, dim, y * n, inverse);
    }
    for y in 0..n {
        for x in 0..n {
            tre[x * n + y] = dre[y * n + x];
            tim[x * n + y] = dim[y * n + x];
        }
    }
    for y in 0..n {
        plan.fft1(tre, tim, y * n, inverse);
    }
    for y in 0..n {
        for x in 0..n {
            dre[y * n + x] = tre[x * n + y];
            dim[y * n + x] = tim[x * n + y];
        }
    }
    if inverse {
        let inv = 1.0 / ((n * n) as f64);
        for i in 0..n * n {
            dre[i] *= inv;
            dim[i] *= inv;
        }
    }
}

// -------------------------------------------------------------------- solver

pub struct Sim {
    pub n: usize,
    k2: Vec<f64>,
    kx: Vec<f64>,
    ky: Vec<f64>,
    mask: Vec<bool>,

    // state spectrum
    sre: Vec<f64>,
    sim: Vec<f64>,
    // Heun buffers
    kre: Vec<f64>,
    kim: Vec<f64>,
    mre: Vec<f64>,
    mim: Vec<f64>,
    bre: Vec<f64>,
    bim: Vec<f64>,
    // spectral work + transpose scratch
    wr: Vec<f64>,
    wi: Vec<f64>,
    tre: Vec<f64>,
    tim: Vec<f64>,
    // physical scratch
    u: Vec<f64>,
    v: Vec<f64>,
    w: Vec<f64>,
    dxw: Vec<f64>,
    dyw: Vec<f64>,

    plan: Plan,
    pub dt: f64,
    pub nu: f64,
    pub steps: u64,
}

impl Sim {
    pub fn new(n: usize) -> Sim {
        let nn = n * n;
        let kf = |i: usize| -> f64 {
            if i <= n / 2 { i as f64 } else { i as f64 - n as f64 }
        };
        let mut k2 = vec![0.0; nn];
        let mut kx = vec![0.0; nn];
        let mut ky = vec![0.0; nn];
        let mut mask = vec![false; nn];
        let keep = (2.0 / 3.0) * (n as f64) / 2.0;
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                let a = kf(x);
                let b = kf(y);
                kx[i] = a;
                ky[i] = b;
                k2[i] = (a * a + b * b).max(1e-12);
                mask[i] = a.abs() > keep || b.abs() > keep;
            }
        }
        k2[0] = 1.0;

        let mut s = Sim {
            n,
            k2, kx, ky, mask,
            sre: vec![0.0; nn], sim: vec![0.0; nn],
            kre: vec![0.0; nn], kim: vec![0.0; nn],
            mre: vec![0.0; nn], mim: vec![0.0; nn],
            bre: vec![0.0; nn], bim: vec![0.0; nn],
            wr: vec![0.0; nn], wi: vec![0.0; nn],
            tre: vec![0.0; nn], tim: vec![0.0; nn],
            u: vec![0.0; nn], v: vec![0.0; nn], w: vec![0.0; nn],
            dxw: vec![0.0; nn], dyw: vec![0.0; nn],
            plan: Plan::new(n),
            dt: 0.002,
            nu: 1e-4,
            steps: 0,
        };
        s.seed();
        s
    }

    /// Deterministic Hermitian seed so native, wasm and JS start identically.
    fn seed(&mut self) {
        let n = self.n;
        let idx = |a: i64, b: i64| -> usize {
            (((b + n as i64) % n as i64) as usize) * n + (((a + n as i64) % n as i64) as usize)
        };
        let set = |sre: &mut Vec<f64>, sim: &mut Vec<f64>, a: i64, b: i64, re: f64, im: f64| {
            let i = idx(a, b);
            sre[i] = re;
            sim[i] = im;
        };
        set(&mut self.sre, &mut self.sim, 1, 1, 2.0, 0.0);
        set(&mut self.sre, &mut self.sim, -1, -1, 2.0, -0.0);
        set(&mut self.sre, &mut self.sim, 2, 3, 1.0, 0.5);
        set(&mut self.sre, &mut self.sim, -2, -3, 1.0, -0.5);
        set(&mut self.sre, &mut self.sim, 3, -2, 0.8, -0.3);
        set(&mut self.sre, &mut self.sim, -3, 2, 0.8, 0.3);
    }

    /// rhs(src) = -NL(src), 2/3 dealiased, into dst.
    fn rhs(&mut self, src_re: &[f64], src_im: &[f64], dst_re: &mut [f64], dst_im: &mut [f64]) {
        let n = self.n;
        let nn = n * n;

        // --- physical u   (uh = -i ky psi)
        for i in 0..nn {
            let (pr, pi) = if i == 0 {
                (0.0, 0.0)
            } else {
                (src_re[i] / self.k2[i], src_im[i] / self.k2[i])
            };
            let b = self.ky[i];
            self.wr[i] = pi * b;
            self.wi[i] = -pr * b;
        }
        fft2(&mut self.wr, &mut self.wi, &mut self.tre, &mut self.tim, &self.plan, true);
        self.u.copy_from_slice(&self.wr);

        // --- physical v   (vh = +i kx psi)
        for i in 0..nn {
            let (pr, pi) = if i == 0 {
                (0.0, 0.0)
            } else {
                (src_re[i] / self.k2[i], src_im[i] / self.k2[i])
            };
            let a = self.kx[i];
            self.wr[i] = -pi * a;
            self.wi[i] = pr * a;
        }
        fft2(&mut self.wr, &mut self.wi, &mut self.tre, &mut self.tim, &self.plan, true);
        self.v.copy_from_slice(&self.wr);

        // --- physical omega
        self.wr.copy_from_slice(src_re);
        self.wi.copy_from_slice(src_im);
        fft2(&mut self.wr, &mut self.wi, &mut self.tre, &mut self.tim, &self.plan, true);
        self.w.copy_from_slice(&self.wr);

        // --- dx omega
        for i in 0..nn {
            let a = self.kx[i];
            self.wr[i] = -a * src_im[i];
            self.wi[i] = a * src_re[i];
        }
        fft2(&mut self.wr, &mut self.wi, &mut self.tre, &mut self.tim, &self.plan, true);
        self.dxw.copy_from_slice(&self.wr);

        // --- dy omega
        for i in 0..nn {
            let b = self.ky[i];
            self.wr[i] = -b * src_im[i];
            self.wi[i] = b * src_re[i];
        }
        fft2(&mut self.wr, &mut self.wi, &mut self.tre, &mut self.tim, &self.plan, true);
        self.dyw.copy_from_slice(&self.wr);

        // --- NL = u dx(omega) + v dy(omega)
        for i in 0..nn {
            self.dxw[i] = self.u[i] * self.dxw[i] + self.v[i] * self.dyw[i];
        }
        self.wr.copy_from_slice(&self.dxw);
        for i in 0..nn {
            self.wi[i] = 0.0;
        }
        fft2(&mut self.wr, &mut self.wi, &mut self.tre, &mut self.tim, &self.plan, false);

        for i in 0..nn {
            if self.mask[i] {
                dst_re[i] = 0.0;
                dst_im[i] = 0.0;
            } else {
                dst_re[i] = -self.wr[i];
                dst_im[i] = -self.wi[i];
            }
        }
    }

    pub fn step(&mut self) {
        let dt = self.dt;
        let nn = self.n * self.n;

        let mut sre = std::mem::take(&mut self.sre);
        let mut sim = std::mem::take(&mut self.sim);
        let mut kre = std::mem::take(&mut self.kre);
        let mut kim = std::mem::take(&mut self.kim);
        let mut mre = std::mem::take(&mut self.mre);
        let mut mim = std::mem::take(&mut self.mim);
        let mut bre = std::mem::take(&mut self.bre);
        let mut bim = std::mem::take(&mut self.bim);

        self.rhs(&sre, &sim, &mut kre, &mut kim);
        for i in 0..nn {
            mre[i] = sre[i] + kre[i] * dt;
            mim[i] = sim[i] + kim[i] * dt;
        }
        self.rhs(&mre, &mim, &mut bre, &mut bim);
        for i in 0..nn {
            sre[i] += 0.5 * dt * (kre[i] + bre[i]);
            sim[i] += 0.5 * dt * (kim[i] + bim[i]);
        }
        for i in 1..nn {
            let f = (-self.nu * self.k2[i] * dt).exp();
            sre[i] *= f;
            sim[i] *= f;
        }
        sre[0] = 0.0;
        sim[0] = 0.0;

        self.sre = sre;
        self.sim = sim;
        self.kre = kre;
        self.kim = kim;
        self.mre = mre;
        self.mim = mim;
        self.bre = bre;
        self.bim = bim;
        self.steps += 1;
    }

    /// Deterministic summary of the state, for cross-language comparison.
    pub fn checksum(&self) -> f64 {
        let n = self.n;
        let inv = 1.0 / ((n * n) as f64);
        let mut acc = 0.0f64;
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                let w = ((y * 7919 + x * 104729) % 997) as f64;
                acc += (self.sre[i] + self.sim[i]) * w;
            }
        }
        acc * inv
    }
}

#[no_mangle]
pub extern "C" fn bench_run(n: usize, steps: usize) -> f64 {
    let mut s = Sim::new(n);
    for _ in 0..steps {
        s.step();
    }
    s.checksum()
}

/// Allocate a solver the caller keeps across calls, so timing can exclude setup.
#[no_mangle]
pub extern "C" fn sim_new(n: usize) -> *mut Sim {
    Box::into_raw(Box::new(Sim::new(n)))
}

#[no_mangle]
pub extern "C" fn sim_step(p: *mut Sim, steps: usize) {
    let s = unsafe { &mut *p };
    for _ in 0..steps {
        s.step();
    }
}

#[no_mangle]
pub extern "C" fn sim_check(p: *const Sim) -> f64 {
    unsafe { (*p).checksum() }
}

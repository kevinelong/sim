//! Pseudo-spectral 2D incompressible Navier-Stokes on a doubly periodic
//! domain [0, 2pi)^2, in vorticity form, plus the extras the six scenes need:
//!
//!   * oriented force dipoles (stresslets) - a static one is the "rod", many
//!     moving ones are the brine-shrimp swimmers of the paper;
//!   * a steady banded body force  f_x = A cos(k y)  - the magnet-driven
//!     counter-rotating channel of the Pittsburgh experiment;
//!   * a beta-plane term  -beta * v  - for the zonal-jet / Great Red Spot
//!     analogue;
//!   * a passive scalar advected by the flow - the dye that shows a transport
//!     barrier being broken.
//!
//! State is the vorticity spectrum in unnormalized-DFT order.  All the extras
//! are assembled in physical space into `src_phys`, transformed once per right
//! hand side evaluation, dealiased, and added.
//!
//! Diagnostics: E(k) and the spectral energy flux Pi(k) = -sum_{q<=k} T(q),
//! with T the transfer spectrum of the advective term.  Pi < 0 below the
//! forcing band is the inverse energy cascade.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

const TWO_THIRDS: f64 = 2.0 / 3.0;

fn fft2(
    data: &mut [Complex<f64>],
    tmp: &mut [Complex<f64>],
    scratch: &mut [Complex<f64>],
    plan: &dyn Fft<f64>,
    n: usize,
    inverse: bool,
) {
    for y in 0..n {
        plan.process_with_scratch(&mut data[y * n..y * n + n], scratch);
    }
    for y in 0..n {
        for x in 0..n {
            tmp[x * n + y] = data[y * n + x];
        }
    }
    for y in 0..n {
        plan.process_with_scratch(&mut tmp[y * n..y * n + n], scratch);
    }
    for y in 0..n {
        for x in 0..n {
            data[y * n + x] = tmp[x * n + y];
        }
    }
    if inverse {
        let inv = 1.0 / ((n * n) as f64);
        for c in data.iter_mut() {
            *c *= inv;
        }
    }
}

/// An oriented force dipole: position, orientation, strength multiplier.
#[derive(Clone, Copy)]
pub struct Dipole {
    pub x: f64,
    pub y: f64,
    pub th: f64,
    pub amp: f64,
}

pub struct Solver {
    pub n: usize,
    pub dx: f64,

    // core turbulence parameters
    pub nu: f64,
    pub drag: f64,
    pub drag_k: f64,
    pub dt: f64,
    pub dt_max: f64,
    pub cfl: f64,
    pub kf: f64,
    pub force_amp: f64,

    // dipoles (rod / swimmers / barrier perturbation)
    pub dipoles: Vec<Dipole>,
    pub dipole_on: bool,
    pub dipole_sigma: f64,
    pub swimmers_move: bool,
    pub swim_speed: f64,
    pub swim_noise: f64,

    // banded (MHD channel) body force
    pub band_on: bool,
    pub band_amp: f64,
    pub band_k: f64,

    // beta-plane
    pub beta: f64,

    // passive scalar (dye)
    pub scalar_on: bool,
    pub kappa: f64,

    pub probe: (f64, f64),

    pub t: f64,
    pub steps: u64,
    pub umax: f64,

    kxa: Vec<f64>,
    kya: Vec<f64>,
    k2: Vec<f64>,
    kmag: Vec<f64>,
    mask: Vec<bool>,

    state: Vec<Complex<f64>>,
    k1: Vec<Complex<f64>>,
    k2v: Vec<Complex<f64>>,
    mid: Vec<Complex<f64>>,
    rhs_buf: Vec<Complex<f64>>,
    scalar: Vec<Complex<f64>>,

    uh: Vec<Complex<f64>>,
    vh: Vec<Complex<f64>>,
    spec_u: Vec<Complex<f64>>,
    spec_v: Vec<Complex<f64>>,
    tmp_s: Vec<Complex<f64>>,
    tmp_s2: Vec<Complex<f64>>,
    tmp_t: Vec<Complex<f64>>,
    sc_a: Vec<Complex<f64>>,
    sc_b: Vec<Complex<f64>>,

    u: Vec<f64>,
    v: Vec<f64>,
    dux: Vec<f64>,
    duy: Vec<f64>,
    dvx: Vec<f64>,
    dvy: Vec<f64>,
    nlx: Vec<f64>,
    nly: Vec<f64>,
    om: Vec<f64>,
    src_phys: Vec<f64>,
    pub scalar_phys: Vec<f64>,

    force_modes: Vec<usize>,

    plan_fwd: Arc<dyn Fft<f64>>,
    plan_inv: Arc<dyn Fft<f64>>,
    scratch: Vec<Complex<f64>>,

    rng: u64,
    pub rng2: u64,

    pub shell_k: Vec<f64>,
    pub shell_e: Vec<f64>,
    pub shell_flux: Vec<f64>,
    pub avg_k: Vec<f64>,
    pub avg_e: Vec<f64>,
    pub avg_flux: Vec<f64>,
    acc_e: Vec<f64>,
    acc_tr: Vec<f64>,
    pub n_diag: u64,
    pub strain_axis: f64,
}

fn lcg_unit(s: &mut u64) -> f64 {
    let mut x = *s;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *s = x;
    let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
    ((v >> 11) as f64) / ((1u64 << 53) as f64)
}

fn gauss(s: &mut u64) -> f64 {
    let u1 = lcg_unit(s).max(1e-12);
    let u2 = lcg_unit(s);
    (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
}

impl Solver {
    pub fn new(n: usize) -> Self {
        let dx = TAU / n as f64;
        let kf_of = |i: usize| -> f64 {
            if i <= n / 2 {
                i as f64
            } else {
                i as f64 - n as f64
            }
        };
        let nn = n * n;
        let mut k2 = vec![0.0; nn];
        let mut kmag = vec![0.0; nn];
        let mut kxa = vec![0.0; nn];
        let mut kya = vec![0.0; nn];
        let mut mask = vec![false; nn];
        let kmax_keep = TWO_THIRDS * (n as f64) / 2.0;
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                let kxv = kf_of(x);
                let kyv = kf_of(y);
                k2[i] = (kxv * kxv + kyv * kyv).max(1e-12);
                kmag[i] = (kxv * kxv + kyv * kyv).sqrt();
                kxa[i] = kxv;
                kya[i] = kyv;
                mask[i] = kxv.abs() > kmax_keep || kyv.abs() > kmax_keep;
            }
        }
        k2[0] = 1.0;

        let mut planner = FftPlanner::<f64>::new();
        let plan_fwd = planner.plan_fft_forward(n);
        let plan_inv = planner.plan_fft_inverse(n);
        let scratch_len = plan_fwd
            .get_inplace_scratch_len()
            .max(plan_inv.get_inplace_scratch_len())
            .max(n);

        let z = || vec![Complex::new(0.0, 0.0); nn];
        let mut s = Self {
            n,
            dx,
            nu: 2.0e-5,
            drag: 0.05,
            drag_k: 2.0,
            dt: 0.004,
            dt_max: 0.01,
            cfl: 0.5,
            kf: 8.0,
            force_amp: 20.0,
            dipoles: Vec::new(),
            dipole_on: false,
            dipole_sigma: 0.10,
            swimmers_move: false,
            swim_speed: 0.0,
            swim_noise: 0.0,
            band_on: false,
            band_amp: 0.0,
            band_k: 1.0,
            beta: 0.0,
            scalar_on: false,
            kappa: 1e-4,
            probe: (PI, PI),
            t: 0.0,
            steps: 0,
            umax: 0.0,
            kxa,
            kya,
            k2,
            kmag,
            mask,
            state: z(),
            k1: z(),
            k2v: z(),
            mid: z(),
            rhs_buf: z(),
            scalar: z(),
            uh: z(),
            vh: z(),
            spec_u: z(),
            spec_v: z(),
            tmp_s: z(),
            tmp_s2: z(),
            tmp_t: z(),
            sc_a: z(),
            sc_b: z(),
            u: vec![0.0; nn],
            v: vec![0.0; nn],
            dux: vec![0.0; nn],
            duy: vec![0.0; nn],
            dvx: vec![0.0; nn],
            dvy: vec![0.0; nn],
            nlx: vec![0.0; nn],
            nly: vec![0.0; nn],
            om: vec![0.0; nn],
            src_phys: vec![0.0; nn],
            scalar_phys: vec![0.0; nn],
            force_modes: Vec::new(),
            plan_fwd,
            plan_inv,
            scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            rng: 0x9E37_79B9_7F4A_7C15,
            rng2: 0x1234_5678_9ABC_DEF0,
            shell_k: Vec::new(),
            shell_e: Vec::new(),
            shell_flux: Vec::new(),
            avg_k: Vec::new(),
            avg_e: Vec::new(),
            avg_flux: Vec::new(),
            acc_e: vec![0.0; n / 2 + 2],
            acc_tr: vec![0.0; n / 2 + 2],
            n_diag: 0,
            strain_axis: 0.0,
        };
        s.seed_perturbation();
        s.rebuild_forcing();
        s
    }

    fn seed_perturbation(&mut self) {
        let n = self.n;
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                let kk = self.kmag[i];
                if kk > 3.0 && kk < 20.0 && !self.mask[i] {
                    let amp = 1e-3;
                    let a = (lcg_unit(&mut self.rng) - 0.5) * amp;
                    let b = (lcg_unit(&mut self.rng) - 0.5) * amp;
                    self.state[i] = Complex::new(a, b);
                }
            }
        }
    }

    pub fn rebuild_forcing(&mut self) {
        self.force_modes.clear();
        for i in 0..self.n * self.n {
            let kk = self.kmag[i];
            if (kk - self.kf).abs() < 0.75 && !self.mask[i] {
                self.force_modes.push(i);
            }
        }
    }

    /// Two halves of dye separated by a horizontal interface at y = pi.
    pub fn init_scalar(&mut self, width: f64) {
        let n = self.n;
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                let yp = TAU * y as f64 / n as f64;
                self.sc_a[i] = Complex::new(((yp - PI) / width).tanh(), 0.0);
            }
        }
        fft2(&mut self.sc_a, &mut self.tmp_t, &mut self.scratch, &*self.plan_fwd, n, false);
        self.scalar.copy_from_slice(&self.sc_a);
    }

    /// rhs(src) = -NL(src) + all scene sources, written into self.rhs_buf.
    fn rhs_into(&mut self, src: &[Complex<f64>]) {
        let n = self.n;
        let nn = n * n;

        for i in 0..nn {
            let psi = if i == 0 {
                Complex::new(0.0, 0.0)
            } else {
                src[i] / self.k2[i]
            };
            let ky = self.kya[i];
            let kx = self.kxa[i];
            self.uh[i] = Complex::new(psi.im * ky, -psi.re * ky);
            self.vh[i] = Complex::new(-psi.im * kx, psi.re * kx);
        }
        fft2(&mut self.uh, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        fft2(&mut self.vh, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);

        self.tmp_s2.copy_from_slice(src);
        fft2(&mut self.tmp_s2, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);

        let mut umax: f64 = 0.0;
        for i in 0..nn {
            let u = self.uh[i].re;
            let v = self.vh[i].re;
            self.u[i] = u;
            self.v[i] = v;
            self.om[i] = self.tmp_s2[i].re;
            let sp = (u * u + v * v).sqrt();
            if sp > umax {
                umax = sp;
            }
        }
        self.umax = umax;

        for i in 0..nn {
            self.tmp_s2[i] = Complex::new(0.0, self.kxa[i]) * src[i];
        }
        fft2(&mut self.tmp_s2, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        for i in 0..nn {
            self.dux[i] = self.tmp_s2[i].re;
        }
        for i in 0..nn {
            self.tmp_s2[i] = Complex::new(0.0, self.kya[i]) * src[i];
        }
        fft2(&mut self.tmp_s2, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        for i in 0..nn {
            self.duy[i] = self.tmp_s2[i].re;
        }

        for i in 0..nn {
            self.nlx[i] = self.u[i] * self.dux[i] + self.v[i] * self.duy[i];
        }
        for i in 0..nn {
            self.tmp_s2[i] = Complex::new(self.nlx[i], 0.0);
        }
        fft2(&mut self.tmp_s2, &mut self.tmp_t, &mut self.scratch, &*self.plan_fwd, n, false);
        for i in 0..nn {
            self.rhs_buf[i] = if self.mask[i] {
                Complex::new(0.0, 0.0)
            } else {
                -self.tmp_s2[i]
            };
        }

        // ---- scene sources, assembled in physical space ----
        for v in self.src_phys.iter_mut() {
            *v = 0.0;
        }
        let mut any = false;
        if self.dipole_on && !self.dipoles.is_empty() {
            any = true;
            self.add_dipoles();
        }
        if self.band_on {
            any = true;
            let a = self.band_amp * self.band_k;
            for y in 0..n {
                let yy = TAU * y as f64 / n as f64;
                let val = a * (self.band_k * yy).sin();
                for x in 0..n {
                    self.src_phys[y * n + x] += val;
                }
            }
        }
        if any {
            for i in 0..nn {
                self.tmp_s[i] = Complex::new(self.src_phys[i], 0.0);
            }
            fft2(&mut self.tmp_s, &mut self.tmp_t, &mut self.scratch, &*self.plan_fwd, n, false);
            for i in 0..nn {
                if !self.mask[i] {
                    self.rhs_buf[i] += self.tmp_s[i];
                }
            }
        }
    }

    /// Sum the stresslet of every dipole into src_phys (already zeroed).
    fn add_dipoles(&mut self) {
        let n = self.n;
        let dx = self.dx;
        let s2 = self.dipole_sigma * self.dipole_sigma;
        let s4i = 1.0 / (s2 * s2);
        for d in 0..self.dipoles.len() {
            let (x0, y0) = (self.dipoles[d].x, self.dipoles[d].y);
            let nx = self.dipoles[d].th.cos();
            let ny = self.dipoles[d].th.sin();
            let a = self.dipoles[d].amp;
            for y in 0..n {
                let mut eta = dx * y as f64 - y0;
                if eta > PI {
                    eta -= TAU;
                } else if eta < -PI {
                    eta += TAU;
                }
                let base = y * n;
                for x in 0..n {
                    let mut xi = dx * x as f64 - x0;
                    if xi > PI {
                        xi -= TAU;
                    } else if xi < -PI {
                        xi += TAU;
                    }
                    let r2 = xi * xi + eta * eta;
                    if r2 > 9.0 * s2 {
                        continue;
                    }
                    let g = (-0.5 * r2 / s2).exp();
                    let bracket = nx * ny * (xi * xi - eta * eta) * s4i
                        + (ny * ny - nx * nx) * xi * eta * s4i;
                    self.src_phys[base + x] += a * g * bracket;
                }
            }
        }
    }

    /// Exact integration of the beta term over `half_dt`: the linear term
    /// d(w)/dt = -beta * v  is a pure phase rotation, -i * beta * kx/k^2.
    /// Integrating it explicitly is only marginally stable and drifts unstable
    /// over thousands of steps, so we do it exactly.
    fn rossby_rotate(&self, state: &mut [Complex<f64>], half_dt: f64) {
        if self.beta == 0.0 {
            return;
        }
        for i in 1..state.len() {
            let omega = self.beta * self.kxa[i] / self.k2[i];
            state[i] *= Complex::new(0.0, -omega * half_dt).exp();
        }
    }

    pub fn step(&mut self) {
        if self.steps % 4 == 0 {
            self.update_dt();
        }
        let dt = self.dt;
        let nn = self.n * self.n;

        let mut state = std::mem::take(&mut self.state);
        let mut k1 = std::mem::take(&mut self.k1);
        let mut k2v = std::mem::take(&mut self.k2v);
        let mut mid = std::mem::take(&mut self.mid);

        self.rossby_rotate(&mut state, 0.5 * dt);
        self.rhs_into(&state);
        k1.copy_from_slice(&self.rhs_buf);
        for i in 0..nn {
            mid[i] = state[i] + k1[i] * dt;
        }
        self.rhs_into(&mid);
        k2v.copy_from_slice(&self.rhs_buf);
        for i in 0..nn {
            state[i] += (k1[i] + k2v[i]) * (0.5 * dt);
        }
        for i in 1..nn {
            state[i] *= (-self.nu * self.k2[i] * dt).exp();
        }
        if self.drag > 0.0 {
            let f = (-self.drag * dt).exp();
            for i in 1..nn {
                if self.kmag[i] <= self.drag_k {
                    state[i] *= f;
                }
            }
        }
        self.rossby_rotate(&mut state, 0.5 * dt);
        state[0] = Complex::new(0.0, 0.0);

        if self.force_amp > 0.0 {
            let f0 = self.force_amp * (self.n as f64) / 2.0;
            let sq = dt.sqrt();
            for j in 0..self.force_modes.len() {
                let i = self.force_modes[j];
                let a = gauss(&mut self.rng) * f0 * sq;
                let b = gauss(&mut self.rng) * f0 * sq;
                state[i] += Complex::new(a, b);
            }
        }

        self.state = state;
        self.k1 = k1;
        self.k2v = k2v;
        self.mid = mid;

        // swimmers ride and rotate with the flow (uses mid-step u, v, omega)
        if self.swimmers_move && !self.dipoles.is_empty() {
            let mut loc = Vec::with_capacity(self.dipoles.len());
            for d in &self.dipoles {
                let (u, v) = self.vel_at(d.x, d.y);
                let w = self.om_at(d.x, d.y);
                loc.push((u, v, w));
            }
            let noise = self.swim_noise;
            let speed = self.swim_speed;
            for (k, d) in self.dipoles.iter_mut().enumerate() {
                let (u, v, w) = loc[k];
                d.th += (0.5 * w + gauss(&mut self.rng) * noise) * dt;
                d.x += (u + speed * d.th.cos()) * dt;
                d.y += (v + speed * d.th.sin()) * dt;
                d.x = d.x.rem_euclid(TAU);
                d.y = d.y.rem_euclid(TAU);
            }
        }

        if self.scalar_on {
            self.step_scalar(dt);
        }

        self.t += dt;
        self.steps += 1;
    }

    fn step_scalar(&mut self, dt: f64) {
        let n = self.n;
        let nn = n * n;
        let kappa = self.kappa;
        // c in physical space
        self.sc_a.copy_from_slice(&self.scalar);
        fft2(&mut self.sc_a, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        for i in 0..nn {
            self.scalar_phys[i] = self.sc_a[i].re;
        }
        // dx c, dy c
        for i in 0..nn {
            self.sc_a[i] = Complex::new(0.0, self.kxa[i]) * self.scalar[i];
        }
        fft2(&mut self.sc_a, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        for i in 0..nn {
            self.dux[i] = self.sc_a[i].re;
        }
        for i in 0..nn {
            self.sc_a[i] = Complex::new(0.0, self.kya[i]) * self.scalar[i];
        }
        fft2(&mut self.sc_a, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        for i in 0..nn {
            self.duy[i] = self.sc_a[i].re;
        }
        for i in 0..nn {
            self.nlx[i] = self.u[i] * self.dux[i] + self.v[i] * self.duy[i];
        }
        for i in 0..nn {
            self.sc_b[i] = Complex::new(self.nlx[i], 0.0);
        }
        fft2(&mut self.sc_b, &mut self.tmp_t, &mut self.scratch, &*self.plan_fwd, n, false);
        for i in 0..nn {
            let mut c = self.scalar[i] - self.sc_b[i] * dt;
            if i != 0 {
                c *= (-kappa * self.k2[i] * dt).exp();
            }
            self.scalar[i] = c;
        }
    }

    fn update_dt(&mut self) {
        let u = self.umax.max(1e-6);
        self.dt = (self.cfl * self.dx / u).min(self.dt_max).max(1e-5);
    }

    fn vel_at(&self, x: f64, y: f64) -> (f64, f64) {
        let (i00, i10, i01, i11, tx, ty) = self.bilerp(x, y);
        let f = |grid: &Vec<f64>| {
            grid[i00] * (1.0 - tx) * (1.0 - ty)
                + grid[i10] * tx * (1.0 - ty)
                + grid[i01] * (1.0 - tx) * ty
                + grid[i11] * tx * ty
        };
        (f(&self.u), f(&self.v))
    }

    fn om_at(&self, x: f64, y: f64) -> f64 {
        let (i00, i10, i01, i11, tx, ty) = self.bilerp(x, y);
        self.om[i00] * (1.0 - tx) * (1.0 - ty)
            + self.om[i10] * tx * (1.0 - ty)
            + self.om[i01] * (1.0 - tx) * ty
            + self.om[i11] * tx * ty
    }

    #[allow(clippy::type_complexity)]
    fn bilerp(&self, x: f64, y: f64) -> (usize, usize, usize, usize, f64, f64) {
        let n = self.n;
        let nf = n as f64;
        let fx = (x / self.dx).rem_euclid(nf);
        let fy = (y / self.dx).rem_euclid(nf);
        let x0 = fx.floor() as usize % n;
        let y0 = fy.floor() as usize % n;
        let x1 = (x0 + 1) % n;
        let y1 = (y0 + 1) % n;
        let tx = fx - fx.floor();
        let ty = fy - fy.floor();
        (y0 * n + x0, y0 * n + x1, y1 * n + x0, y1 * n + x1, tx, ty)
    }

    pub fn vorticity(&mut self) -> &[f64] {
        let n = self.n;
        self.tmp_s2.copy_from_slice(&self.state);
        fft2(&mut self.tmp_s2, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        self.om
            .iter_mut()
            .zip(self.tmp_s2.iter())
            .for_each(|(o, c)| *o = c.re);
        &self.om
    }

    pub fn scalar_field(&mut self) -> &[f64] {
        let n = self.n;
        self.sc_a.copy_from_slice(&self.scalar);
        fft2(&mut self.sc_a, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        self.scalar_phys
            .iter_mut()
            .zip(self.sc_a.iter())
            .for_each(|(o, c)| *o = c.re);
        &self.scalar_phys
    }

    pub fn diagnostics(&mut self) {
        let n = self.n;
        let nn = n * n;
        let s = 1.0 / (nn as f64);

        for i in 0..nn {
            let psi = if i == 0 {
                Complex::new(0.0, 0.0)
            } else {
                self.state[i] / self.k2[i]
            };
            self.uh[i] = Complex::new(psi.im * self.kya[i], -psi.re * self.kya[i]);
            self.vh[i] = Complex::new(-psi.im * self.kxa[i], psi.re * self.kxa[i]);
        }
        self.spec_u.copy_from_slice(&self.uh);
        self.spec_v.copy_from_slice(&self.vh);

        for i in 0..nn {
            self.tmp_s[i] = Complex::new(0.0, self.kxa[i]) * self.spec_u[i];
        }
        fft2(&mut self.tmp_s, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        for i in 0..nn {
            self.dux[i] = self.tmp_s[i].re;
        }
        for i in 0..nn {
            self.tmp_s[i] = Complex::new(0.0, self.kya[i]) * self.spec_u[i];
        }
        fft2(&mut self.tmp_s, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        for i in 0..nn {
            self.duy[i] = self.tmp_s[i].re;
        }
        for i in 0..nn {
            self.tmp_s[i] = Complex::new(0.0, self.kxa[i]) * self.spec_v[i];
        }
        fft2(&mut self.tmp_s, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        for i in 0..nn {
            self.dvx[i] = self.tmp_s[i].re;
        }
        for i in 0..nn {
            self.tmp_s[i] = Complex::new(0.0, self.kya[i]) * self.spec_v[i];
        }
        fft2(&mut self.tmp_s, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        for i in 0..nn {
            self.dvy[i] = self.tmp_s[i].re;
        }

        self.uh.copy_from_slice(&self.spec_u);
        self.vh.copy_from_slice(&self.spec_v);
        fft2(&mut self.uh, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        fft2(&mut self.vh, &mut self.tmp_t, &mut self.scratch, &*self.plan_inv, n, true);
        for i in 0..nn {
            self.u[i] = self.uh[i].re;
            self.v[i] = self.vh[i].re;
        }

        let (rx, ry) = self.probe_index();
        let idx = ry * n + rx;
        let sxx = self.dux[idx];
        let syy = self.dvy[idx];
        let sxy = 0.5 * (self.duy[idx] + self.dvx[idx]);
        self.strain_axis = 0.5 * (2.0 * sxy).atan2(sxx - syy);

        for i in 0..nn {
            self.nlx[i] = self.u[i] * self.dux[i] + self.v[i] * self.duy[i];
            self.nly[i] = self.u[i] * self.dvx[i] + self.v[i] * self.dvy[i];
        }
        for i in 0..nn {
            self.tmp_s[i] = Complex::new(self.nlx[i], 0.0);
        }
        fft2(&mut self.tmp_s, &mut self.tmp_t, &mut self.scratch, &*self.plan_fwd, n, false);
        for i in 0..nn {
            self.tmp_s[i] *= s;
        }
        for i in 0..nn {
            self.tmp_s2[i] = Complex::new(self.nly[i], 0.0);
        }
        fft2(&mut self.tmp_s2, &mut self.tmp_t, &mut self.scratch, &*self.plan_fwd, n, false);
        for i in 0..nn {
            self.tmp_s2[i] *= s;
        }

        let kmax = n / 2;
        let mut e = vec![0.0f64; kmax + 2];
        let mut tr = vec![0.0f64; kmax + 2];
        for i in 1..nn {
            let b = self.kmag[i].round() as usize;
            if b > kmax {
                continue;
            }
            let us = self.spec_u[i] * s;
            let vs = self.spec_v[i] * s;
            e[b] += 0.5 * (us.norm_sqr() + vs.norm_sqr());
            let nx = self.tmp_s[i];
            let ny = self.tmp_s2[i];
            tr[b] -= (us.conj() * nx + vs.conj() * ny).re;
        }
        let mut flux = vec![0.0f64; kmax + 1];
        let mut acc = 0.0;
        for kb in 1..=kmax {
            acc += tr[kb];
            flux[kb] = -acc;
        }
        self.shell_k = (1..=kmax).map(|k| k as f64).collect();
        self.shell_e = (1..=kmax).map(|k| e[k]).collect();
        self.shell_flux = (1..=kmax).map(|k| flux[k]).collect();

        for b in 1..=kmax {
            self.acc_e[b] += e[b];
            self.acc_tr[b] += tr[b];
        }
        self.n_diag += 1;
        let inv = 1.0 / self.n_diag as f64;
        let mut aa = 0.0;
        let mut aflux = Vec::with_capacity(kmax);
        for kb in 1..=kmax {
            aa += self.acc_tr[kb] * inv;
            aflux.push(-aa);
        }
        self.avg_k = (1..=kmax).map(|k| k as f64).collect();
        self.avg_e = (1..=kmax).map(|k| self.acc_e[k] * inv).collect();
        self.avg_flux = aflux;
    }

    pub fn reset_average(&mut self) {
        for v in self.acc_e.iter_mut() {
            *v = 0.0;
        }
        for v in self.acc_tr.iter_mut() {
            *v = 0.0;
        }
        self.n_diag = 0;
    }

    fn probe_index(&self) -> (usize, usize) {
        let n = self.n as f64;
        let ix = ((self.probe.0 / TAU) * n).rem_euclid(n) as usize;
        let iy = ((self.probe.1 / TAU) * n).rem_euclid(n) as usize;
        (ix.min(self.n - 1), iy.min(self.n - 1))
    }

    pub fn total_energy(&self) -> f64 {
        let s = 1.0 / ((self.n * self.n) as f64);
        let mut e = 0.0;
        for i in 1..self.n * self.n {
            let psi = self.state[i] / self.k2[i];
            let uh = Complex::new(psi.im * self.kya[i], -psi.re * self.kya[i]);
            let vh = Complex::new(-psi.im * self.kxa[i], psi.re * self.kxa[i]);
            e += 0.5 * (uh.norm_sqr() + vh.norm_sqr()) * s * s;
        }
        e
    }

    /// RMS of the two velocity components, from the spectrum (Parseval).
    pub fn uv_rms(&self) -> (f64, f64) {
        let s = 1.0 / ((self.n * self.n) as f64);
        let (mut su, mut sv) = (0.0f64, 0.0f64);
        for i in 1..self.n * self.n {
            let psi = self.state[i] / self.k2[i];
            let uh = Complex::new(psi.im * self.kya[i], -psi.re * self.kya[i]);
            let vh = Complex::new(-psi.im * self.kxa[i], psi.re * self.kxa[i]);
            su += uh.norm_sqr() * s * s;
            sv += vh.norm_sqr() * s * s;
        }
        (su.sqrt(), sv.sqrt())
    }

    /// Fraction of kinetic energy carried by modes with kx = 0, i.e. flow that
    /// varies only across y - the signature of the MHD channel's bands.
    pub fn banded_fraction(&self) -> f64 {
        let (mut tot, mut band) = (0.0f64, 0.0f64);
        for i in 1..self.n * self.n {
            let psi = self.state[i] / self.k2[i];
            let uh = Complex::new(psi.im * self.kya[i], -psi.re * self.kya[i]);
            let vh = Complex::new(-psi.im * self.kxa[i], psi.re * self.kxa[i]);
            let e = uh.norm_sqr() + vh.norm_sqr();
            tot += e;
            if self.kxa[i] == 0.0 {
                band += e;
            }
        }
        if tot > 0.0 { band / tot } else { 0.0 }
    }
}

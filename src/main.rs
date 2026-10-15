//! Six switchable 2D-turbulence scenes inspired by Si & Fang,
//! "Sea Monkeys Show Scientists How To Rewrite a Rule of Turbulence"
//! (Quanta, 2026-10-02).

mod solver;

use eframe::egui;
use egui::{Color32, ColorImage, TextureHandle, TextureOptions};
use solver::{Dipole, Solver};
use std::f64::consts::{PI, TAU};

// ---------------------------------------------------------------- colormaps

const STOPS: [(f64, [u8; 3]); 5] = [
    (0.00, [8, 12, 40]),
    (0.25, [40, 90, 200]),
    (0.50, [235, 238, 245]),
    (0.75, [230, 120, 30]),
    (1.00, [90, 8, 8]),
];
const DYE: [(f64, [u8; 3]); 4] = [
    (0.00, [10, 20, 60]),
    (0.40, [20, 120, 150]),
    (0.70, [170, 230, 200]),
    (1.00, [255, 250, 210]),
];

fn lerp_stops(stops: &[(f64, [u8; 3])], t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        let (a, ca) = w[0];
        let (b, cb) = w[1];
        if t <= b {
            let f = ((t - a) / (b - a)).clamp(0.0, 1.0);
            let mut o = [0u8; 3];
            for i in 0..3 {
                o[i] = (ca[i] as f64 + (cb[i] as f64 - ca[i] as f64) * f) as u8;
            }
            return o;
        }
    }
    stops[stops.len() - 1].1
}

fn colormap(t: f64) -> [u8; 3] {
    lerp_stops(&STOPS, t)
}
fn dye_colormap(t: f64) -> [u8; 3] {
    lerp_stops(&DYE, t)
}

// -------------------------------------------------------------------- scenes

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scene {
    Cascade,
    Rod,
    Swimmers,
    Mhd,
    Jets,
    Barrier,
}

impl Scene {
    const ALL: [Scene; 6] = [
        Scene::Cascade,
        Scene::Rod,
        Scene::Swimmers,
        Scene::Mhd,
        Scene::Jets,
        Scene::Barrier,
    ];

    fn title(&self) -> &'static str {
        match self {
            Scene::Cascade => "Dual cascade (bare turbulence)",
            Scene::Rod => "Rod / 45-deg flux switch",
            Scene::Swimmers => "Brine-shrimp swarm (active matter)",
            Scene::Mhd => "MHD channel (counter-rotating bands)",
            Scene::Jets => "Beta-plane zonal jets (Great Red Spot)",
            Scene::Barrier => "Transport barrier (two-fluid mixing)",
        }
    }
}

fn spawn_swimmers(s: &mut Solver, count: usize) {
    s.dipoles.clear();
    for _ in 0..count {
        let x = rnd(&mut s.rng2, 0.0, TAU);
        let y = rnd(&mut s.rng2, 0.0, TAU);
        let th = rnd(&mut s.rng2, 0.0, TAU);
        s.dipoles.push(Dipole { x, y, th, amp: 1.5 });
    }
}

fn rnd(state: &mut u64, lo: f64, hi: f64) -> f64 {
    let mut x = *state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *state = x;
    let v = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
    let u = ((v >> 11) as f64) / ((1u64 << 53) as f64);
    lo + (hi - lo) * u
}

fn configure(s: &mut Solver, scene: Scene) {
    s.nu = 2.0e-5;
    s.drag = 0.05;
    s.drag_k = 2.0;
    s.kf = 8.0;
    s.force_amp = 20.0;
    s.dipole_on = false;
    s.dipoles.clear();
    s.dipole_sigma = 0.10;
    s.swimmers_move = false;
    s.swim_speed = 0.0;
    s.swim_noise = 0.0;
    s.band_on = false;
    s.band_amp = 0.0;
    s.band_k = 1.0;
    s.beta = 0.0;
    s.scalar_on = false;
    s.kappa = 1e-4;
    s.probe = (PI, PI);
    s.rng2 = 0x1234_5678_9ABC_DEF0;

    match scene {
        Scene::Cascade => {}
        Scene::Rod => {
            s.dipoles.push(Dipole { x: PI, y: PI, th: 0.0, amp: 1.5 });
            s.dipole_on = true;
        }
        Scene::Swimmers => {
            s.force_amp = 12.0;
            s.dipole_sigma = 0.055;
            s.dipole_on = true;
            s.swimmers_move = true;
            s.swim_speed = 0.45;
            s.swim_noise = 0.15;
            spawn_swimmers(s, 24);
        }
        Scene::Mhd => {
            s.nu = 0.03;
            s.band_on = true;
            s.band_amp = 0.10;
            s.band_k = 1.0;
            s.force_amp = 0.0;
            s.drag = 0.10;
            s.dipoles.push(Dipole { x: PI, y: PI, th: 0.0, amp: 1.5 });
            s.dipole_on = false;
        }
        Scene::Jets => {
            s.beta = 50.0;
            s.force_amp = 5.0;
            s.drag = 0.05;
            s.drag_k = 1.0;
            s.kf = 12.0;
        }
        Scene::Barrier => {
            s.scalar_on = true;
            s.kappa = 1e-4;
            s.nu = 0.01;
            s.force_amp = 0.0;
            s.band_on = true;
            s.band_amp = 0.10;
            s.band_k = 1.0;
            s.drag = 0.10;
            s.dipoles.push(Dipole { x: PI, y: PI + 0.35, th: 0.0, amp: 2.0 });
            s.dipole_on = false;
            s.init_scalar(0.30);
        }
    }
}

// ---------------------------------------------------------------- field guide

const VORT_DISPLAY: &str = "The image is the vorticity field w = curl(u) of a \
genuine 2D incompressible flow on a periodic box. Colour: deep blue = \
clockwise spin, orange/red = counter-clockwise, near-white = little rotation. \
Bright filaments are shear layers; round blobs are vortices. There is no third \
dimension - every eddy is a vertical column seen from above.";

const DYE_DISPLAY: &str = "The image is a passive scalar (dye) carried by the \
flow: navy = one fluid, pale yellow = the other, with a sharp interface \
between them across the middle. The flow does not create or destroy dye, it \
only stirs it, so the picture shows exactly where mixing has happened. Once \
the interface breaks into filaments the two fluids are mixed and cannot be \
separated again.";

const CASCADES: &str = "3D turbulence breaks big eddies into smaller ones \
until viscosity turns the motion into heat: a forward cascade (Richardson, \
Kolmogorov). 2D turbulence is different - enstrophy still goes forward, but \
ENERGY cascades backward, climbing to the largest scales in the box \
(Kraichnan & Batchelor). That inverse cascade is what feeds Jupiter's Great \
Red Spot from the small eddies around its rim.\n\n\
The flux plot is the direct measurement. Pi(k) = -sum_{q<=k} T(q), T the \
transfer spectrum of the advective term:\n\
  Pi < 0 -> energy moving to LARGER scales (inverse)\n\
  Pi > 0 -> energy moving to SMALLER scales (forward)\n\
Below the peak you should see Pi negative; above it, positive.";

const PLOTS: &str = "E(k) is energy per wavenumber, log-log. Wavenumber is \
inverse length, so small k = big structures. The peak sits at the forcing \
wavenumber kf. Straight stretches on a log-log plot are power laws; treat the \
slope as illustrative at this resolution rather than a measured exponent.\n\n\
Both curves are time-averaged (see 'samples' in the left panel); the \
instantaneous flux is very noisy. Use 'reset average' after changing \
anything.";

const ROD_RULE: &str = "The paper's claim: energy flows from small scales to \
large when the swimmer's long axis sits at less than 45 degrees to the \
direction of maximum stretching, and reverses above 45 degrees.\n\n\
Green line = direction of maximum stretching (principal axis of the \
strain-rate tensor S = (grad u + grad u^T)/2). Grey lines = the two 45-degree \
boundaries. Orange = the rod, modelled as the exact stresslet (force dipole) \
the researchers used. The readout prints theta between the rod and the \
stretching axis and flags it against the rule.";

const ROD_CAVEAT: &str = "Honest caveat, with measurements. This scene does NOT \
reproduce the paper's clean 45-degree flux reversal, and the reasons are measured \
rather than assumed.\n\n\
First, this build puts the rod at (pi,pi), a strain NODE, so 'the direction of \
maximum stretching' is meaningless at the default position: in the clean laminar \
banded shear the node reads |S| = 0.0000 there against 5.0011 at (pi,pi/2), with \
the ambient strain axis at exactly 45.00 deg. turbulence.html moves the rod to \
(pi,pi/2), where the imposed strain actually is. (An older note here quoted \
'|S| = 0.067 against 12.0'; that pair does not reproduce under any recorded \
configuration. README.md carries the recipe that does.)\n\n\
Second, and deeper: placed correctly in a steady imposed shear (ambient axis a \
clean 45.00 deg, rod-off flux ~1e-17, so the rod is the only source), the rod's \
own stresslet still dominates the local strain, and unless viscosity is high \
enough for the rod's scale to equilibrate the flux is not stationary - \
window-to-window scatter is 334% at nu=1e-4 and 6% at nu=5e-3.\n\n\
In the stationary regime the flux varies smoothly by 22x with the rod's ABSOLUTE \
angle but never changes sign, and two orientations that pure-shear symmetry says \
must be equivalent differ by 22x. The stresslet-strain coupling predicts zero at \
both 0 and 90 deg, yet those differ 22x - so the effect here is not the alignment \
mechanism the paper reports. Treat the rod as a probe, not a validated result.";

const SWIM_ACTIVE: &str = "Active matter: objects that move by themselves and \
inject energy into their surroundings. Each dot is one brine-shrimp analogue - \
a small force dipole (stresslet) that pushes fluid one way at its head and the \
opposite way at its tail, so it swims. The dots are advected by the flow, \
rotate with the local vorticity, and wander a little on their own.\n\n\
The question the paper started from: in a large body of water, do billions of \
tiny swimmers matter for mixing? Turning the swarm on and watching the \
small-scale structure thicken is the qualitative answer. The swimmers are \
energy sources, so with forcing set to 0 the flow is driven entirely by them.";

const MHD_APPARATUS: &str = "The Pittsburgh experiment in its simplest form. \
A thin film of conductive salt water sits on a sheet of glass with magnets \
underneath; a current through the fluid produces a Lorentz force j x B that \
drives the water one way along one side of the channel and the opposite way \
along the other. The velocity gradient between the two bands is the shear that \
the shrimp swim in.\n\n\
Here that drive is a steady banded body force f_x = A cos(k y), which produces \
the same counter-rotating bands. The rod is the obstacle the paper substitutes \
for the shrimp. The shallow film also has real bottom friction, which is why \
large-scale friction is not zero by default.";

const JETS_BETA: &str = "Adding a Coriolis gradient (the beta effect) to 2D \
turbulence changes where the inverse cascade can go. Instead of piling into a \
single giant vortex, energy is arrested at the Rhines wavenumber \
k_b = sqrt(beta / 2U), and the flow develops alternating east-west (zonal) \
bands - the mechanism behind Jupiter's banded atmosphere and its Great Red \
Spot. Raise beta and the arrest scale moves to smaller scales.\n\n\
Stated honestly: in this box (2*pi across) the zonal signature is real but \
modest. Measured, the zonal energy fraction climbs to roughly 2 to 4 times the \
isotropic baseline and u_rms/v_rms to about 1.6, and the effect is stable for \
beta between about 20 and 120. Beta too large (say 300) kills it again, \
because the Rossby waves become faster than the eddy turnover. Striking, \
many-banded jets need a domain many jet-widths across - a bigger box than we \
simulate here. The two readouts in the left panel are the measurement: watch \
the zonal fraction rise above its isotropic value.";

const BARRIER_CLAIM: &str = "Fang's theory concerns the transport barrier: the \
boundary between two fluids that resist mixing. Keeping such a boundary intact \
costs energy; the claim is that disrupting it locally uses less than 1 percent \
of that cost. Here the interface is the dye boundary across the middle. With \
the perturbation off it holds (only slow diffusion). Switch the perturbation \
on - the dipole sitting on the interface - and watch vortices tear the \
boundary into filaments, which is mixing.";

const NATIVE_FROZEN: &str = "This native build is FROZEN as the reference \
implementation: the fast local tool, the same algorithm as turbulence.html at \
roughly 3x the browser speed, and the build whose numbers README.md publishes. \
turbulence.html is the lab that is still moving.\n\n\
Ported here in the 2026-10-15 pass, because a published claim depended on them: \
the 2/3 dealiasing on the dye advection (the browser scalar path always had it, \
this one did not, so the barrier dye was aliased here) and the measured rod \
caveat below.\n\n\
Deliberately NOT ported: named presets, the runtime resolution selector, \
Save PNG, the dark-neutral colormap, and the barrier's perturbation spin. So \
this build keeps the bright blue/orange vorticity map, its barrier scene starts \
with the perturbation OFF (the browser starts it on and spinning), and its rod \
scene keeps the original rod-at-(pi,pi) default - inside the strain node the \
caveat below describes. For the barrier's torn-interface look and for scene \
presets, use turbulence.html. RESUME.md lists the divergences and the \
re-verify commands.";

fn notes(scene: Scene) -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&'static str, &'static str)> = Vec::new();
    v.push(("State of this build", NATIVE_FROZEN));
    match scene {
        Scene::Barrier => v.push(("What you are seeing", DYE_DISPLAY)),
        _ => v.push(("What you are seeing", VORT_DISPLAY)),
    }
    match scene {
        Scene::Cascade => {
            v.push((
                "The bare experiment",
                "No rod, no swimmers: just forced 2D turbulence. This is the \
baseline every other scene perturbs. The web of filaments and vortices is \
what the inverse cascade looks like from above.",
            ));
        }
        Scene::Rod => {
            v.push(("The 45-degree rule", ROD_RULE));
            v.push(("Limitation", ROD_CAVEAT));
        }
        Scene::Swimmers => {
            v.push(("Active matter", SWIM_ACTIVE));
        }
        Scene::Mhd => {
            v.push(("The apparatus", MHD_APPARATUS));
        }
        Scene::Jets => {
            v.push(("Beta plane and zonal jets", JETS_BETA));
        }
        Scene::Barrier => {
            v.push(("The transport barrier", BARRIER_CLAIM));
        }
    }
    if scene != Scene::Barrier {
        v.push(("The two cascades", CASCADES));
    }
    v.push(("Reading the two plots", PLOTS));
    v.push((
        "Knobs and what they illustrate",
        "forcing = strength of the random small-scale stirring (the energy \
source).\n\
kf = the wavenumber it is injected at, i.e. the size of the energy source.\n\
viscosity = damps the smallest scales at rate nu*k^2, setting where the \
forward cascade dies.\n\
large-scale friction = drains the biggest scales; without it energy piles up \
in a 'condensate' at k = 1 and the flux never settles. It is physical: the \
paper's film has real bottom friction.\n\
rod strength / rod size = how hard the stresslet pushes and at what scale \
(smaller rod = smaller scales).\n\
swarm strength / swim speed / noise = how hard each swimmer pushes, how fast \
it swims, and how much it wanders.\n\
band drive / band wavenumber = the MHD force amplitude and how many \
counter-rotating bands fit across the box.\n\
beta = Coriolis gradient; sets the jet spacing.\n\
dye diffusivity = how fast the interface blurs on its own, without flow.\n\
steps/frame = simulation speed only, not physics.",
    ));
    if scene == Scene::Barrier {
        v.push((
            "Things worth trying",
            "1. Run it with the perturbation off: the interface stays sharp, \
only slowly diffusing.\n\
2. Switch the perturbation on and watch the boundary break into filaments.\n\
3. Sweep the perturbation angle and see how the tear pattern changes.\n\
4. Raise the dye diffusivity and watch the interface blur without any help \
from the flow.",
        ));
    } else {
        v.push((
            "Things worth trying",
            "1. Watch the inverse cascade in the bare flow: rod off, Pi(k) \
negative across everything below kf.\n\
2. Turn the rod or swarm on and watch small-scale structure appear.\n\
3. Set large-scale friction to 0 and watch the condensate form at k = 1.\n\
4. Turn forcing to 0 and watch the flow decay (or, in the swarm scene, keep \
it alive on swimmers alone).\n\
5. Raise viscosity and watch the forward range of E(k) fall away sooner.",
        ));
    }
    v
}

// ---------------------------------------------------------------------- plot

fn plot(
    ui: &mut egui::Ui,
    height: f32,
    xs: &[f64],
    ys: &[f64],
    xlog: bool,
    ylog: bool,
    color: Color32,
    zero_line: bool,
) {
    let w = ui.available_width().max(80.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, height), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, Color32::from_rgb(18, 18, 22));

    let xf = |v: f64| if xlog { v.max(1e-12).ln() } else { v };
    let yf = |v: f64| if ylog { v.max(1e-30).ln() } else { v };
    let mut xmin = f64::INFINITY;
    let mut xmax = f64::NEG_INFINITY;
    let mut ymin = f64::INFINITY;
    let mut ymax = f64::NEG_INFINITY;
    for i in 0..xs.len().min(ys.len()) {
        let x = xf(xs[i]);
        let y = yf(ys[i]);
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        xmin = xmin.min(x);
        xmax = xmax.max(x);
        ymin = ymin.min(y);
        ymax = ymax.max(y);
    }
    if !xmin.is_finite() || xmax <= xmin || ymax <= ymin {
        return;
    }
    let mapx = |x: f64| rect.min.x + ((x - xmin) / (xmax - xmin)) as f32 * rect.width();
    let mapy = |y: f64| rect.max.y - ((y - ymin) / (ymax - ymin)) as f32 * rect.height();

    if zero_line && ymin < 0.0 && ymax > 0.0 {
        let y0 = mapy(0.0);
        p.line_segment(
            [egui::pos2(rect.min.x, y0), egui::pos2(rect.max.x, y0)],
            egui::Stroke::new(1.0, Color32::from_rgb(90, 90, 100)),
        );
    }
    let mut pts = Vec::with_capacity(xs.len());
    for i in 0..xs.len().min(ys.len()) {
        let x = xf(xs[i]);
        let y = yf(ys[i]);
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        pts.push(egui::pos2(mapx(x), mapy(y)));
    }
    if pts.len() >= 2 {
        p.add(egui::Shape::line(pts, egui::Stroke::new(1.5, color)));
    }
}

fn arrow(p: &egui::Painter, from: egui::Pos2, to: egui::Pos2, col: Color32, w: f32) {
    p.line_segment([from, to], egui::Stroke::new(w, col));
    let d = to - from;
    let l = d.length();
    if l < 1.0 {
        return;
    }
    let u = d / l;
    let n = egui::vec2(-u.y, u.x);
    let head = 9.0;
    p.line_segment([to, to - u * head + n * head * 0.5], egui::Stroke::new(w, col));
    p.line_segment([to, to - u * head - n * head * 0.5], egui::Stroke::new(w, col));
}

// ----------------------------------------------------------------------- app

struct App {
    scene: Scene,
    s: Solver,
    tex: Option<TextureHandle>,
    img: ColorImage,
    vmax: f64,
    steps_per_frame: u32,
    frames_left: Option<u32>,
    running: bool,
}

impl App {
    fn new(n: usize, frames_left: Option<u32>, scene0: Scene) -> Self {
        let scene = scene0;
        let mut s = Solver::new(n);
        configure(&mut s, scene);
        Self {
            scene,
            s,
            tex: None,
            img: ColorImage::new([n, n], vec![Color32::BLACK; n * n]),
            vmax: 1.0,
            steps_per_frame: 2,
            frames_left,
            running: true,
        }
    }

    fn switch(&mut self, scene: Scene) {
        let n = self.s.n;
        self.s = Solver::new(n);
        configure(&mut self.s, scene);
        self.scene = scene;
        self.vmax = 1.0;
    }

    fn render_field(&mut self) {
        let n = self.s.n;
        match self.scene {
            Scene::Barrier => {
                let f = self.s.scalar_field();
                for (px, &c) in self.img.pixels.iter_mut().zip(f.iter()) {
                    let t = 0.5 * (1.0 + c.clamp(-1.0, 1.0));
                    let rgb = dye_colormap(t);
                    *px = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                }
            }
            _ => {
                let om = self.s.vorticity();
                let mut curmax: f64 = 1e-9;
                for &w in om {
                    let a = w.abs();
                    if a > curmax {
                        curmax = a;
                    }
                }
                self.vmax = (self.vmax * 0.92).max(curmax * 1.05).max(1e-6);
                let inv = 1.0 / self.vmax;
                for (px, &w) in self.img.pixels.iter_mut().zip(om.iter()) {
                    let t = 0.5 * (1.0 + (w * inv).clamp(-1.0, 1.0));
                    let c = colormap(t);
                    *px = Color32::from_rgb(c[0], c[1], c[2]);
                }
            }
        }
        let _ = n;
    }

    fn draw_overlay(&self, p: &egui::Painter, rect: egui::Rect) {
        let map = |x: f64, y: f64| {
            egui::pos2(
                rect.min.x + (x / TAU) as f32 * rect.width(),
                rect.max.y - (y / TAU) as f32 * rect.height(),
            )
        };
        let scale = rect.width() as f64 / TAU;
        let dipoles = self.s.dipoles.clone();

        match self.scene {
            Scene::Rod => {
                let c = map(self.s.probe.0, self.s.probe.1);
                let sa = self.s.strain_axis;
                let r = 0.9 * scale;
                let ax = egui::vec2((sa.cos() * r) as f32, -(sa.sin() * r) as f32);
                p.line_segment([c - ax, c + ax], egui::Stroke::new(2.0, Color32::from_rgb(90, 230, 120)));
                for sgn in [-1.0f64, 1.0] {
                    let a = sa + sgn * std::f64::consts::FRAC_PI_4;
                    let v = egui::vec2((a.cos() * r) as f32, -(a.sin() * r) as f32);
                    p.line_segment([c, c + v], egui::Stroke::new(1.0, Color32::from_rgb(130, 130, 140)));
                }
            }
            Scene::Swimmers => {
                for d in &dipoles {
                    let c = map(d.x, d.y);
                    let r = 0.30 * scale;
                    let v = egui::vec2((d.th.cos() * r) as f32, -(d.th.sin() * r) as f32);
                    p.line_segment([c - v, c + v], egui::Stroke::new(2.0, Color32::from_rgb(255, 200, 60)));
                }
            }
            Scene::Mhd => {
                let x0 = map(0.15 * TAU, 0.30 * TAU).x;
                let x1 = map(0.85 * TAU, 0.30 * TAU).x;
                let ytop = map(0.0, 0.30 * TAU).y;
                arrow(p, egui::pos2(x0, ytop), egui::pos2(x1, ytop), Color32::from_rgb(120, 220, 255), 2.0);
                let ybot = map(0.0, 0.70 * TAU).y;
                arrow(p, egui::pos2(x1, ybot), egui::pos2(x0, ybot), Color32::from_rgb(255, 160, 120), 2.0);
                if self.s.dipole_on {
                    let c = map(self.s.probe.0, self.s.probe.1);
                    let rl = 0.6 * scale;
                    let rv = egui::vec2((dipoles[0].th.cos() * rl) as f32, -(dipoles[0].th.sin() * rl) as f32);
                    p.line_segment([c - rv, c + rv], egui::Stroke::new(4.0, Color32::from_rgb(255, 170, 40)));
                }
            }
            Scene::Jets => {
                for k in 1..6 {
                    let y = map(0.0, k as f64 / 6.0 * TAU).y;
                    p.line_segment(
                        [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                        egui::Stroke::new(0.7, Color32::from_rgba_unmultiplied(255, 255, 255, 45)),
                    );
                }
            }
            Scene::Barrier => {
                let y = map(0.0, PI + 0.35).y;
                p.line_segment(
                    [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
                    egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 90)),
                );
                if self.s.dipole_on {
                    let d = dipoles[0];
                    let c = map(d.x, d.y);
                    let rl = 0.7 * scale;
                    let rv = egui::vec2((d.th.cos() * rl) as f32, -(d.th.sin() * rl) as f32);
                    p.line_segment([c - rv, c + rv], egui::Stroke::new(4.0, Color32::from_rgb(255, 90, 90)));
                }
            }
            Scene::Cascade => {}
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let scene = self.scene;
        if self.running {
            for _ in 0..self.steps_per_frame {
                self.s.step();
            }
        }
        self.s.diagnostics();
        self.render_field();

        let opts = TextureOptions::LINEAR;
        if self.tex.is_none() {
            self.tex = Some(ui.ctx().load_texture("field", self.img.clone(), opts));
        }
        if let Some(t) = self.tex.as_mut() {
            t.set(self.img.clone(), opts);
        }
        let tex_id = self.tex.as_ref().map(|t| t.id());

        let mut new_scene: Option<Scene> = None;

        egui::Panel::left("hud")
            .resizable(false)
            .default_size(330.0)
            .show(ui, |ui| {
                ui.heading("2D turbulence lab");
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label("scene");
                        egui::ComboBox::from_label("")
                            .selected_text(scene.title())
                            .show_ui(ui, |ui| {
                                for sc in Scene::ALL {
                                    if ui.selectable_label(self.scene == sc, sc.title()).clicked() {
                                        new_scene = Some(sc);
                                    }
                                }
                            });
                        ui.separator();
                        match scene {
                            Scene::Cascade => {
                                ui.add(egui::Slider::new(&mut self.s.force_amp, 0.0..=30.0).text("forcing"));
                                ui.add(egui::Slider::new(&mut self.s.kf, 2.0..=20.0).text("kf (forcing scale)"));
                            }
                            Scene::Rod => {
                                ui.checkbox(&mut self.s.dipole_on, "rod active");
                                let mut deg = self.s.dipoles[0].th.to_degrees();
                                if ui.add(egui::Slider::new(&mut deg, 0.0..=180.0).text("rod angle (deg)")).changed() {
                                    self.s.dipoles[0].th = deg.to_radians();
                                }
                                let mut strength = self.s.dipoles[0].amp;
                                if ui.add(egui::Slider::new(&mut strength, 0.0..=10.0).text("rod strength")).changed() {
                                    for d in self.s.dipoles.iter_mut() {
                                        d.amp = strength;
                                    }
                                }
                                ui.add(egui::Slider::new(&mut self.s.dipole_sigma, 0.03..=0.3).text("rod size"));
                                ui.add(egui::Slider::new(&mut self.s.force_amp, 0.0..=30.0).text("forcing"));
                                ui.add(egui::Slider::new(&mut self.s.kf, 2.0..=20.0).text("kf"));
                                let mut band = self.s.band_on;
                                if ui.checkbox(&mut band, "banded drive on").changed() {
                                    self.s.band_on = band;
                                }
                                if self.s.band_on {
                                    ui.add(egui::Slider::new(&mut self.s.band_amp, 0.0..=1.0).text("band drive"));
                                }
                            }
                            Scene::Swimmers => {
                                ui.checkbox(&mut self.s.dipole_on, "swarm active");
                                ui.checkbox(&mut self.s.swimmers_move, "swimmers self-propel");
                                let mut count = self.s.dipoles.len();
                                if ui.add(egui::Slider::new(&mut count, 0..=64).text("swimmers")).changed() {
                                    spawn_swimmers(&mut self.s, count);
                                }
                                let mut strength = self.s.dipoles.first().map(|d| d.amp).unwrap_or(1.5);
                                if ui.add(egui::Slider::new(&mut strength, 0.0..=12.0).text("swim strength")).changed() {
                                    for d in self.s.dipoles.iter_mut() {
                                        d.amp = strength;
                                    }
                                }
                                ui.add(egui::Slider::new(&mut self.s.dipole_sigma, 0.02..=0.2).text("swimmer size"));
                                ui.add(egui::Slider::new(&mut self.s.swim_speed, 0.0..=1.5).text("swim speed"));
                                ui.add(egui::Slider::new(&mut self.s.swim_noise, 0.0..=0.6).text("orientation noise"));
                                ui.add(egui::Slider::new(&mut self.s.force_amp, 0.0..=30.0).text("forcing"));
                            }
                            Scene::Mhd => {
                                ui.add(egui::Slider::new(&mut self.s.band_amp, 0.0..=1.0).text("band drive"));
                                ui.add(egui::Slider::new(&mut self.s.band_k, 1.0..=8.0).text("band wavenumber"));
                                ui.checkbox(&mut self.s.band_on, "drive on");
                                ui.add(egui::Slider::new(&mut self.s.force_amp, 0.0..=10.0).text("turbulent forcing"));
                                ui.separator();
                                ui.checkbox(&mut self.s.dipole_on, "rod / obstacle active");
                                let mut deg = self.s.dipoles[0].th.to_degrees();
                                if ui.add(egui::Slider::new(&mut deg, 0.0..=180.0).text("rod angle (deg)")).changed() {
                                    self.s.dipoles[0].th = deg.to_radians();
                                }
                                let mut strength = self.s.dipoles[0].amp;
                                if ui.add(egui::Slider::new(&mut strength, 0.0..=10.0).text("rod strength")).changed() {
                                    for d in self.s.dipoles.iter_mut() {
                                        d.amp = strength;
                                    }
                                }
                                ui.add(egui::Slider::new(&mut self.s.dipole_sigma, 0.03..=0.3).text("rod size"));
                                ui.label(format!(
                                    "banded energy fraction {:.3}  [1.0 = pure counter-flow]",
                                    self.s.banded_fraction()
                                ));
                            }
                            Scene::Jets => {
                                ui.add(egui::Slider::new(&mut self.s.beta, 0.0..=300.0).text("beta (Coriolis gradient)"));
                                ui.add(egui::Slider::new(&mut self.s.force_amp, 0.0..=30.0).text("forcing"));
                                ui.add(egui::Slider::new(&mut self.s.kf, 2.0..=20.0).text("kf"));
                                let (ur, vr) = self.s.uv_rms();
                                ui.label(format!("u_rms {:.3}   v_rms {:.3}", ur, vr));
                                let zf = self.s.banded_fraction();
                                let col = if zf > 0.08 {
                                    Color32::from_rgb(120, 220, 160)
                                } else {
                                    Color32::from_rgb(200, 200, 200)
                                };
                                ui.colored_label(
                                    col,
                                    format!("zonal (east-west) energy fraction {:.3}  [~0.01 = isotropic]", zf),
                                );
                                ui.label("Isotropic baseline is about 1/sqrt(n) = 0.01. Watch it rise.");
                            }
                            Scene::Barrier => {
                                ui.checkbox(&mut self.s.scalar_on, "dye active");
                                ui.checkbox(&mut self.s.dipole_on, "perturbation active");
                                let mut deg = self.s.dipoles[0].th.to_degrees();
                                if ui.add(egui::Slider::new(&mut deg, 0.0..=180.0).text("perturbation angle (deg)")).changed() {
                                    self.s.dipoles[0].th = deg.to_radians();
                                }
                                let mut strength = self.s.dipoles[0].amp;
                                if ui.add(egui::Slider::new(&mut strength, 0.0..=10.0).text("perturbation strength")).changed() {
                                    for d in self.s.dipoles.iter_mut() {
                                        d.amp = strength;
                                    }
                                }
                                ui.add(egui::Slider::new(&mut self.s.dipole_sigma, 0.05..=0.4).text("perturbation size"));
                                ui.add(egui::Slider::new(&mut self.s.kappa, 1e-6..=1e-2).logarithmic(true).text("dye diffusivity"));
                                ui.add(egui::Slider::new(&mut self.s.band_amp, 0.0..=1.0).text("shear drive"));
                                if ui.button("reset dye").clicked() {
                                    let n = self.s.n;
                                    let (k, b, bk) = (self.s.kappa, self.s.band_on, self.s.band_k);
                                    self.s = Solver::new(n);
                                    configure(&mut self.s, Scene::Barrier);
                                    self.s.kappa = k;
                                    self.s.band_on = b;
                                    self.s.band_k = bk;
                                }
                            }
                        }
                        ui.separator();
                        ui.add(egui::Slider::new(&mut self.s.nu, 1e-6..=1e-1).logarithmic(true).text("viscosity"));
                        ui.add(egui::Slider::new(&mut self.s.drag, 0.0..=0.3).text("large-scale friction"));
                        ui.add(egui::Slider::new(&mut self.s.drag_k, 1.0..=12.0).text("friction below k"));
                        ui.checkbox(&mut self.running, "running");
                        ui.add(egui::Slider::new(&mut self.steps_per_frame, 1..=8).text("steps/frame"));
                        if ui.button("restart scene").clicked() {
                            let n = self.s.n;
                            self.s = Solver::new(n);
                            configure(&mut self.s, scene);
                            self.vmax = 1.0;
                        }
                        ui.separator();
                        let eff = {
                            let a = (self.s.dipoles.first().map(|d| d.th).unwrap_or(0.0)
                                - self.s.strain_axis)
                                .rem_euclid(PI);
                            a.min(PI - a).to_degrees()
                        };
                        if scene == Scene::Rod {
                            let col = if eff < 45.0 {
                                Color32::from_rgb(120, 190, 255)
                            } else {
                                Color32::from_rgb(255, 160, 90)
                            };
                            ui.label(format!("theta vs max-stretch: {:.1} deg", eff));
                            ui.colored_label(
                                col,
                                if eff < 45.0 {
                                    "inverse (energy -> large scales)"
                                } else {
                                    "forward (energy -> small scales)"
                                },
                            );
                            ui.label(format!("strain axis {:.1} deg", self.s.strain_axis.to_degrees()));
                        }
                        ui.label(format!(
                            "t={:.2}  steps={}  dt={:.4}  umax={:.2}  E={:.3e}",
                            self.s.t,
                            self.s.steps,
                            self.s.dt,
                            self.s.umax,
                            self.s.total_energy()
                        ));
                        ui.separator();
                        ui.label("energy flux Pi(k), time-averaged (sign = direction)");
                        let k: Vec<f64> = self.s.avg_k.clone();
                        let flux: Vec<f64> = self.s.avg_flux.clone();
                        plot(ui, 120.0, &k, &flux, true, false, Color32::from_rgb(120, 220, 160), true);
                        ui.label("energy spectrum E(k), time-averaged");
                        let e: Vec<f64> = self.s.avg_e.clone();
                        plot(ui, 120.0, &k, &e, true, true, Color32::from_rgb(220, 190, 120), false);
                        ui.horizontal(|ui| {
                            ui.label(format!("samples {}", self.s.n_diag));
                            if ui.button("reset average").clicked() {
                                self.s.reset_average();
                            }
                        });
                    });
            });

        egui::Panel::right("notes")
            .resizable(true)
            .default_size(400.0)
            .show(ui, |ui| {
                ui.heading("Field guide");
                ui.label(scene.title());
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, (title, body)) in notes(scene).iter().enumerate() {
                            egui::CollapsingHeader::new(*title)
                                .default_open(i < 2)
                                .show(ui, |ui| {
                                    ui.add(egui::Label::new(*body).wrap());
                                });
                        }
                    });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            let avail = ui.available_size();
            let side = avail.x.min(avail.y).max(64.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
            let p = ui.painter_at(rect);
            if let Some(id) = tex_id {
                p.image(
                    id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            self.draw_overlay(&p, rect);
            let caption = match scene {
                Scene::Rod => "green = max stretching   grey = 45 deg boundary   orange = rod",
                Scene::Swimmers => "orange ticks = self-propelled stresslets",
                Scene::Mhd => "arrows = the two counter-flowing bands of the channel",
                Scene::Jets => "guides = latitude bands; expect zonal (east-west) jets",
                Scene::Barrier => "white line = dye interface   red = perturbation",
                Scene::Cascade => "free 2D turbulence: the inverse cascade in the raw",
            };
            p.text(
                rect.min + egui::vec2(6.0, 6.0),
                egui::Align2::LEFT_TOP,
                caption,
                egui::FontId::proportional(12.0),
                Color32::from_rgb(225, 225, 235),
            );
        });

        if let Some(sc) = new_scene {
            self.switch(sc);
        }

        ui.ctx().request_repaint();
        if let Some(f) = self.frames_left.as_mut() {
            *f = f.saturating_sub(1);
            if *f == 0 {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

// ------------------------------------------------------------------- headless

fn run_avg(s: &mut Solver, steps: u64, diag_every: u64) {
    let mut i = 0u64;
    while i < steps {
        let m = diag_every.min(steps - i);
        for _ in 0..m {
            s.step();
        }
        s.diagnostics();
        i += m;
    }
}

fn inverse_band(s: &Solver) -> f64 {
    let mut sum = 0.0;
    let mut c = 0;
    for (i, &k) in s.avg_k.iter().enumerate() {
        if k <= s.kf - 2.0 {
            sum += s.avg_flux[i];
            c += 1;
        }
    }
    if c > 0 { sum / c as f64 } else { 0.0 }
}

fn selftest(n: usize, steps: u64) {
    let mut s = Solver::new(n);
    configure(&mut s, Scene::Cascade);
    run_avg(&mut s, steps, 20);
    s.reset_average();
    run_avg(&mut s, steps * 4, 10);
    println!(
        "n={} t={:.2} steps={} dt={:.5} umax={:.3} E={:.5e} samples={}",
        n, s.t, s.steps, s.dt, s.umax, s.total_energy(), s.n_diag
    );
    println!("{:>4} {:>14} {:>14}", "k", "E(k)_avg", "Pi(k)_avg");
    for i in 0..s.avg_k.len() {
        println!("{:>4.0} {:>14.5e} {:>14.5e}", s.avg_k[i], s.avg_e[i], s.avg_flux[i]);
    }
}

fn scenecheck(name: &str, scene: Scene, n: usize, steps: u64, beta_override: Option<f64>) {
    let mut s = Solver::new(n);
    configure(&mut s, scene);
    if let Some(b) = beta_override {
        if scene == Scene::Jets {
            s.beta = b;
        }
    }
    run_avg(&mut s, steps, 20);
    s.reset_average();
    run_avg(&mut s, steps, 10);
    let finite = s.total_energy().is_finite()
        && s.avg_e.iter().all(|v| v.is_finite())
        && s.avg_flux.iter().all(|v| v.is_finite());
    let (ur, vr) = s.uv_rms();
    let aniso = if vr > 0.0 { (ur / vr).min(999.0) } else { 999.0 };
    println!(
        "{:<9} t={:7.2} umax={:7.3} E={:10.4e} Pi_inv={:+.4e} u/v={:6.2} band={:5.3} finite={}",
        name,
        s.t,
        s.umax,
        s.total_energy(),
        inverse_band(&s),
        aniso,
        s.banded_fraction(),
        finite
    );
}

fn scene_by_name(name: &str) -> Scene {
    match name {
        "rod" => Scene::Rod,
        "swimmers" => Scene::Swimmers,
        "mhd" => Scene::Mhd,
        "jets" => Scene::Jets,
        "barrier" => Scene::Barrier,
        _ => Scene::Cascade,
    }
}

fn all_scenes(n: usize, steps: u64) {
    for (name, sc) in [
        ("cascade", Scene::Cascade),
        ("rod", Scene::Rod),
        ("swimmers", Scene::Swimmers),
        ("mhd", Scene::Mhd),
        ("jets", Scene::Jets),
        ("barrier", Scene::Barrier),
    ] {
        scenecheck(name, sc, n, steps, None);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(p) = args.iter().position(|a| a == "--one") {
        let name = args.get(p + 1).map(|s| s.as_str()).unwrap_or("cascade");
        let steps: u64 = args.get(p + 2).and_then(|v| v.parse().ok()).unwrap_or(4000);
        let n: usize = args.get(p + 3).and_then(|v| v.parse().ok()).unwrap_or(128);
        let beta: Option<f64> = args.get(p + 4).and_then(|v| v.parse().ok());
        scenecheck(name, scene_by_name(name), n, steps, beta);
        return;
    }
    if let Some(p) = args.iter().position(|a| a == "--scenes") {
        let steps: u64 = args.get(p + 1).and_then(|v| v.parse().ok()).unwrap_or(3000);
        let n: usize = args.get(p + 2).and_then(|v| v.parse().ok()).unwrap_or(128);
        all_scenes(n, steps);
        return;
    }
    if let Some(p) = args.iter().position(|a| a == "--selftest") {
        let steps: u64 = args.get(p + 1).and_then(|v| v.parse().ok()).unwrap_or(2000);
        let n: usize = args.get(p + 2).and_then(|v| v.parse().ok()).unwrap_or(128);
        selftest(n, steps);
        return;
    }
    let frames: Option<u32> = args
        .iter()
        .position(|a| a == "--frames")
        .and_then(|p| args.get(p + 1))
        .and_then(|v| v.parse().ok());
    let n: usize = args
        .iter()
        .position(|a| a == "--n")
        .and_then(|p| args.get(p + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(256);
    let scene0 = args
        .iter()
        .position(|a| a == "--scene")
        .and_then(|p| args.get(p + 1))
        .map(|s| scene_by_name(s))
        .unwrap_or(Scene::Cascade);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1560.0, 900.0])
            .with_title("2D Turbulence Lab"),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "turb2d",
        options,
        Box::new(move |_cc| Ok(Box::new(App::new(n, frames, scene0)))),
    );
}

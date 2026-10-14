# turb2d - a 2D turbulence lab

> Session state, open items, verified numbers and measurement caveats live in
> `RESUME.md` - read that first when picking this up again.

## Two views of the same paper

| file | what it is |
|---|---|
| `turbulence.html` | the six-scene **flow** view, in the browser: one self-contained HTML file, plain JS, 128^2 spectral solver |
| `brine-shrimp.html` | the **organism** view: the brine shrimp itself, metachronal legs, schooling, dye, in one self-contained HTML file |
| `src/` (`turb2d`) | the original native Rust/Windows app, same six scenes at ~3x the browser speed |

The two HTML files are a matched pair and cross-link to each other. No
dependencies, no build step, no server: double-click either one.

Six switchable Rust/Windows simulations inspired by
"Sea Monkeys Show Scientists How To Rewrite a Rule of Turbulence"
(Quanta Magazine, 2026-10-02), on Si & Fang's work at the University of
Pittsburgh (PNAS Nexus 2024; Science Advances 2025).

Every scene runs the same real, forced 2D incompressible turbulent flow and
reports the spectral energy flux `Pi(k)` live. What changes is the physics
layered on top, the knobs in the left panel, and the field guide in the right
panel.

## Scenes

| Scene | Physics added | Verified signature |
|---|---|---|
| **Dual cascade** | none - bare forced turbulence | `Pi(k) < 0` below the forcing band: the inverse cascade |
| **Rod / 45-deg switch** | one static force dipole (the paper's stresslet for a brine shrimp) | inverse cascade persists with the rod as a probe |
| **Brine-shrimp swarm** | 24 moving force dipoles that swim, ride the flow and rotate with the local vorticity | flow sustained by swimmers alone when forcing is set to 0 |
| **MHD channel** | steady banded body force `f_x = A cos(k y)`, the Lorentz drive of the Pittsburgh apparatus | banded energy fraction = 1.000, i.e. a pure counter-flowing channel |
| **Beta-plane zonal jets** | Coriolis gradient term `-beta * v` | zonal energy fraction 2-4x the isotropic baseline, `u_rms/v_rms` about 1.6 |
| **Transport barrier** | passive scalar (dye) advected by the flow, plus an optional perturbation dipole on the interface | a stable banded shear (banded fraction 1.000) with a sharp, slow-diffusing interface |

Left panel: scene selector, per-scene knobs, shared knobs (viscosity,
large-scale friction and its scale, running, steps/frame, restart scene), live
readouts, and the two time-averaged plots. Right panel: the field guide, which
changes with the scene and explains what you are seeing and what each knob
illustrates.

## Numerics

- Pseudo-spectral 2D Navier-Stokes in vorticity form, doubly periodic
  `[0, 2pi)^2`, 2/3-rule dealiasing, Heun step with exponential viscosity,
  adaptive CFL-limited `dt`.
- All scene sources (dipoles, banded drive) are assembled in physical space,
  transformed once per RHS evaluation, dealiased, and added.
- The beta term `-beta * v` is linear and oscillatory. Integrating it
  explicitly is *slowly unstable* (amplification 1 + (beta*dt)^4/4 per step),
  which compounds over thousands of steps and blows up; it is therefore applied
  as an exact per-mode phase rotation, Strang-split around the explicit step,
  exactly as viscosity is. This was a real bug found by the beta sweep.
- Stochastic forcing in a ring around `kf`; low-`k` Rayleigh friction, which
  drains the large-scale condensate so the inverse cascade reaches a steady
  state. The friction scale is a knob (`friction below k`).
- The dye is a passive scalar with spectral advection and diffusion.

## Diagnostics

`E(k)` and `Pi(k)`, both time-averaged: `Pi(k) = -sum_{q<=k} T(q)` with `T` the
transfer spectrum of the advective term (Pope's convention).

```
Pi < 0  ->  energy moving to LARGER scales  (inverse cascade)
Pi > 0  ->  energy moving to SMALLER scales (forward cascade)
```

Two scene-specific scalar readouts are printed live in the left panel so the
claims above are measurable, not asserted: the **banded energy fraction**
(energy in `kx = 0` modes; 1.0 = pure counter-flow) and the **zonal energy
fraction** with `u_rms/v_rms` (isotropic baseline is about `1/n`).

## Run

```
cargo run --release                          # interactive, 256^2
cargo run --release -- --n 192
cargo run --release -- --scene jets          # start in a given scene
cargo run --release -- --frames 60 --n 256   # smoke run, exits after 60 frames
cargo run --release -- --selftest 6000 128   # time-averaged E(k), Pi(k), bare scene
cargo run --release -- --scenes 6000 128     # sanity/stability run of all six scenes
cargo run --release -- --one jets 15000 96 50   # one scene, optional beta override
```

## Verified

`--scenes 6000 128` (all finite):

```
cascade   Pi_inv=-2.32e-3  band=0.041
rod       Pi_inv=-2.18e-3  band=0.036
swimmers  Pi_inv=-7.27e-4  umax=1.45
mhd       Pi_inv=~0        band=1.000
jets      u/v=1.23         band=0.036
barrier   Pi_inv=~0        band=1.000
```

`--selftest 6000 128` reproduces the textbook dual cascade: `Pi(k)` negative
and monotone through `k = 1..7` (inverse) and positive for `k >= 9` (forward),
with the energy peak at the forcing wavenumber 8.

Per-scene UI panels: each of the six scenes starts, renders and exits cleanly.

## Rendering: exact, not interpolated

The field is band-limited and the solver holds its exact Fourier coefficients, so
the image is **synthesized at the display resolution** (zero-pad the spectrum,
inverse transform) rather than letting the browser bilinear-upscale the coarse
raster. Verified exact: resampling the synthesized field back at the coarse
points reproduces the original bit for bit.

That matters because bilinear upscaling is not neutral. Grouped by position
within a coarse cell, the Laplacian of a bilinearly upscaled field is **exactly
zero in every cell interior and non-zero only on the cell-boundary lines** - a
literal square lattice at the upscale period (8.3 device px at 128^2), which is
plainly visible. The exact synthesis gives a uniform Laplacian (0.011 across all
sub-positions, ratio 1.00x) and no lattice.

| pipeline | error vs true field | cost per frame |
|---|---|---|
| direct bilinear 128 -> display | 14.1% | ~0 ms |
| exact to 256, then bilinear 4x | **3.8%** | 3.5 ms, every other frame |
| exact to 512, then bilinear 2x | 0.8% | 20.7 ms |
| exact to 1024 (export path only) | 0.0% | one-off |

Against a 7.7 ms solver step, the live path uses **512**: the best
accuracy-per-cost point among the synthesis sizes. `Save PNG` uses the exact path
at up to 1024 and writes the field alone, so the export has no browser resample at
all. Note the colormap is applied to the *values* at full resolution, rather than
colormapping the coarse grid and interpolating RGB (which smears the sharp band
the dark-neutral map has at zero).

The synthesis target is clamped so it can never fall *below* the solver grid -
synthesizing below `n` folds high modes onto low ones. (That clamp was wrong for
a while: at grid 512 the target was 256, silently corrupting the render.)

All three knobs are reachable from the console: `window.LAB.synthMax` (256 halves
the render cost), `synthEvery`, and `exactScale`.

### Is the residual 4x resample worth removing?

There is a `exactScale` flag (`window.LAB.exactScale = true`) that makes the field
square exactly the synthesis size, so the blit is 1:1 with no resample at all.
Measured, 128^2 grid, 2 steps/frame, 1750x1125 canvas:

| mode | synthesis | square shown | resample | fps |
|---|---|---|---|---|
| default | 256 | 1057 px (full) | 4.1x | **51** |
| 1:1 | 512 | 512 px (48%) | none | 41 |
| 1:1 | 1024 | 1024 px (97%) | none | **16.5** |

Removing the last resample costs **3x the frame time** and buys interpolation
error 3.8% -> 0.8%. It adds no information: the field is band-limited by the
solver grid, so a finer synthesis samples the same field more finely, it does not
resolve anything new. It also constrains the geometry (the square must be a power
of two device pixels, so it cannot fill the frame, and 2048 would be ~90 ms).
The residual tail is the *cheapest* step - the browser's own filter, ~0 ms of JS -
so the default keeps it, and `Save PNG` covers the exact case where it matters.

## Known limitations

**Rod scene.** It does not reproduce the paper's 45-degree flux reversal, and the
reason is now measured rather than assumed:

- The default rod position sits in a strain *node*: |S| = 0.067 at (pi,pi) vs
  12.0 at (pi,pi/2), a factor of 180.
- Even placed correctly in a steady imposed shear (ambient strain axis a clean
  45.00 deg, rod-off flux ~1e-17 so the rod is the only source), the rod's own
  stresslet dominates the local strain, and the flux is not stationary unless
  viscosity is high enough for the rod's scale to equilibrate (window-to-window
  scatter 334% at nu=1e-4, 6% at nu=5e-3).
- In the stationary regime the flux varies smoothly by 22x with the rod's
  *absolute* angle but never changes sign, and two orientations that pure-shear
  symmetry requires to be equivalent differ by 22x. The stresslet-strain
  coupling predicts zero at both 0 and 90 deg, yet those differ 22x.

So the mechanism operating here is not the alignment coupling the paper
reports. Treat the rod as a probe, not a validated result.

**Jets scene.** The zonal signature is real but modest at this box size (2*pi
across): zonal fraction 2-4x the isotropic baseline, `u_rms/v_rms` about 1.6,
stable for beta between about 20 and 120 (beta ~ 300 suppresses it again,
because the Rossby waves outrun the eddy turnover). Visually striking
many-banded jets need a domain many jet-widths across - a larger box than is
simulated here.

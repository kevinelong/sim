# sim - resume note

Last worked: 2026-10-05. Topic: simulations inspired by the Quanta article
"Sea Monkeys Show Scientists How To Rewrite a Rule of Turbulence" (2026-10-02),
on Si & Fang's University of Pittsburgh work.

Nothing of mine is left running (no HTTP server, no `turb2d.exe`). All browser
test tabs were closed.

## What exists

| path | what | state |
|---|---|---|
| `turbulence.html` | 6-scene 2D turbulence lab, plain JS, single file, no deps | works, verified |
| `brine-shrimp.html` | brine shrimp organism sim, plain JS, single file, no deps | works, verified |
| `src/` (`turb2d`) | native Rust/Windows app, same 6 scenes | works, but **now behind** the HTML version |
| `bench/` | Rust crate compiling the solver step to native + wasm32 | works |
| `js-bench.html` | JS / wasm / native comparison harness | works |
| `README.md` | documents all of the above | current |

The two HTML files are a matched pair and cross-link to each other. Neither has
any external reference (verified: 0 matches for http/src=/@import/cdn).

## Open items

1. **Rust app is behind the HTML lab.** Missing: presets, resolution selector,
   Save PNG, the inverted dark-neutral colormap, scalar dealiasing, the barrier
   dye `wobble`, and the updated rod caveat wording. Decision needed: port across,
   or freeze it as the native reference and say so. My recommendation was to
   freeze (it exists to be the fast local tool) - **not done**.
2. **Rod 45-deg switch: not reproduced.** Documented as a measured negative
   (see below). Would need a domain many jet-widths larger than 128^2.
3. **Jets: zonal signature real but modest** at this box size. Documented.

## Verified numbers to trust

Physics (both Rust and JS, independent runs):
- Cascade scene: Pi(k) < 0 for k=1..7 (inverse), > 0 for k>=9 (forward), energy
  peak at kf=8. JS at t=75: -2.6e-4, -1.2e-4, -1.5e-3, -2.7e-3, -3.3e-3,
  -2.7e-3, -2.1e-3; Rust at t=300 agrees in sign and magnitude.

Costs, 128^2 grid, measured in the browser:
- solver step: **7.7 ms** (Bun/JSC does the same in 4.1 ms)
- one field synthesis: 1 ms (m=128), 3.5 ms (m=256), 20.7 ms (m=512)
- canvas draw calls (`fillRect`, `drawImage`, `strokeRect`): ~0 ms, they are queued

Rendering accuracy vs the true band-limited field (measured):
- direct bilinear upscale: 14.1% of field rms
- exact to 256 then 4x tail: 3.8%
- exact to 512 then 2x tail: **0.8%**  <- current live default
- exact to 1024: 0.0%  <- export path only

The bilinear lattice, which is what motivated all of the above: grouped by
position within a coarse cell, the Laplacian of a bilinearly upscaled field is
**exactly zero in every cell interior** and non-zero only on the cell-boundary
lines. Exact synthesis gives a uniform Laplacian (ratio 1.00x).

Browser vs native (same algorithm, bit-identical checksums):
- plain JS ~= wasm (1.05-1.16x)
- both ~2.9x slower than native Rust; SIMD128 buys only 1.05-1.21x
- canvas blit is 0.4% of a step, so WebGL-for-rendering buys nothing

Rod experiment (negative result, with controls):
- ambient strain axis with the banded drive: exactly 45.00 deg, stable
- rod-off flux ~1e-17, so the rod is the only source
- strain NODE at (pi,pi): |S| = 0.028 vs 12-15 at (pi,pi/2)
- flux is not stationary at nu=1e-4 (window-to-window scatter 334%); stationary
  at nu=5e-3 (6%)
- in the stationary regime the flux varies smoothly by 22x with the rod's
  ABSOLUTE angle but never changes sign, and two orientations that pure-shear
  symmetry requires to be equivalent differ by 22x. The stresslet-strain
  coupling predicts zero at both 0 and 90 deg yet those differ 22x. So the
  effect is not the alignment mechanism the paper reports.

## Bugs found and fixed this session (do not reintroduce)

- `bandedFraction()` used `ky` without destructuring it - crashed the MHD/Jets readouts.
- the source buffer was only zeroed inside `addDipoles()`, so in MHD/barrier
  (dipole off) the banded force accumulated every RHS call: umax 41 instead of 0.67.
- the resolution selector offered 96 and 192, which are **not powers of two** -
  the radix-2 FFT silently produced NaN. Now 64/128/256/512 plus a constructor
  guard that throws.
- the synthesis clamp was `min(max(m,n), synthMax)`, which at grid 512 resolved
  to m=256 < n and folded high modes onto low ones.
- preset changes did not reset the time-averaged diagnostics, so the Pi(k) plot
  blended two regimes. Presets now restart the scene (fresh Solver, zeroed averages).
- the rod sat in a strain node, and the Aligned/Across presets measured the axis
  at the rod, which the rod's own stresslet dominates.
- `diagnostics()` reused `dux`/`duy` as scratch after computing the strain (latent).
- a steady 2D perturbation only stretches dye along streamfunction contours
  (integrable), so the barrier scene looked artificial; added perturbation spin.

## Tests

    node --test

Ten tests, stdlib only (no dependencies, no browser), about 95 s. Nine run the real
`Solver` lifted out of `turbulence.html`; one pins the native binary.

- `test/turbulence.test.mjs` extracts the solver the way this file documents - slice the
  HTML between the section markers and evaluate it, with a small DOM stub because
  `configure` lives in "scene setup" and `synthesize` in "plumbing", outside the solver
  section. It pins: the power-of-two guard (96 and 192 throw, 64/128/256/512 do not), the
  grid-size selector offering only powers of two, `forceModes` staying **empty until
  `configure()` fills it** (the trap at the end of this file), the cascade's inverse-band
  sign (Pi < 0 for k=1..7) with the energy peak at the forcing scale, `synthesize`'s
  exactness (the coarse lattice comes back **bit for bit**, residual 0), the Laplacian
  property the whole synthesis path exists for (bilinear: exactly 0 in every cell interior
  and non-zero only on the cell boundaries; exact synthesis: uniform, ratio ~1.00), and the
  accuracy ladder's ordering with the live default under 1%.
- `test/rust-turb2d.test.mjs` runs `target/release/turb2d.exe --selftest 6000 128` (~45 s)
  and pins README.md's "Verified" block: Pi < 0 for k=1..7, Pi > 0 for k >= 9, energy peak
  at k=8. It **skips with a message** naming the build command when the binary is absent, so
  a missing exe is visible rather than silently green.

The accuracy ladder's absolute percentages are deliberately NOT asserted: they are
convention- and frame-dependent (measured here: 2.4% / 0.7% / 0.19% / 0.00% for
128 / 256 / 512 / 1024, against the README's 14.1 / 3.8 / 0.8 / 0.0 - a consistent ~5x,
from the reference and the snapshot the in-browser measurement used). The order and the
smallness of the live default are what the test holds.

Each guard was falsified before it was trusted: removing the power-of-two throw,
commenting `s.rebuildForce()` out of `configure`, forcing `synthesize`'s scale to 1, and
flipping the flux sign each turn the matching test red.

## How to re-verify

```
# Rust app: six-scene sanity, with E, umax, inverse-band flux, bandedness
./target/release/turb2d.exe --scenes 6000 128

# Rust physics check (inverse cascade)
./target/release/turb2d.exe --selftest 6000 128

# performance harness (native; wasm build target is wasm32-unknown-unknown)
cd bench && cargo run --release --bin benchnative -- 256 300

# browser: open either HTML file directly (file://). No server needed.
#   turbulence.html exposes window.LAB for verification:
#     LAB.solver, LAB.renderM, LAB.renderField, LAB.synthMax, LAB.exactScale,
#     LAB.switchScene, LAB.setRes
```

The fastest way to run solver experiments is to extract the solver out of the
HTML rather than drive the browser, which has a watchdog that kills long
synchronous loops:

```js
const html = await Bun.file("C:/Users/kevin/sim/turbulence.html").text();
const M1 = "// ---------------------------------------------------------------------- rng";
const M2 = "// -------------------------------------------------------------- scene setup";
const { Solver } = new Function(html.slice(html.indexOf(M1), html.indexOf(M2)) + "\nreturn { Solver, TAU };")();
```

Note a bare `new Solver(n)` has **empty `forceModes`** - `configure()` is what
calls `rebuildForce()`. Forgetting it gives a plausible, slowly decaying, unforced
flow that looks like a bug in the physics.

## Measurement caveats

- Headless Chromium rasterises in software, so its canvas *paint* cost (~10 ms)
  is an artifact of the environment and much cheaper in a GPU-composited browser.
  Trust the JS-side numbers (step, synthesis) and the ratios, not absolute fps.
- rAF is vsync-capped at 60 fps, so any frame under 16.7 ms reads as exactly 60.
- Absolute timings drift within a long benchmarking session; compare within a
  run, not across runs.
- Screenshots from the browser tool are re-encoded (WebP), which adds an 8-pixel
  block artifact of its own - 0.996 blockiness on the app canvas vs 1.116 after
  encoding. Use `Save PNG` in the app for image comparisons.

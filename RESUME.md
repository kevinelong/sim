# sim - resume note

Last worked: 2026-10-15. Topic: simulations inspired by the Quanta article
"Sea Monkeys Show Scientists How To Rewrite a Rule of Turbulence" (2026-10-02),
on Si & Fang's University of Pittsburgh work.

Nothing of mine is left running: no HTTP server, no `turb2d.exe`, and the one
headless browser tab used for the `window.LAB` check was closed.

## What exists

| path | what | state |
|---|---|---|
| `turbulence.html` | 6-scene 2D turbulence lab, plain JS, single file, no deps | works, verified |
| `brine-shrimp.html` | brine shrimp organism sim, plain JS, single file, no deps | works, verified |
| `src/` (`turb2d`) | native Rust/Windows app, same 6 scenes | **FROZEN native reference** (see below) |
| `bench/` | Rust crate compiling the solver step to native + wasm32 | works |
| `js-bench.html` | JS / wasm / native comparison harness | works |
| `test/` | 10 stdlib tests, run with `node --test` | 10/10 pass, 105.9 s |
| `README.md` | documents all of the above | current |

The two HTML files are a matched pair and cross-link to each other
(`grep -c brine-shrimp.html turbulence.html` = 2, the reverse = 1 - the forward
link lives inside a JS string, so a plain `href="` grep misses it). Both still
have 0 external references (`grep -c -E 'https?://|src="|@import|cdn'` = 0 for
each).

## Decided 2026-10-15: the native app is FROZEN (open item 1, closed)

Item 1 asked for a decision - port the browser lab's newer features into `src/`,
or freeze it as the native reference. **Frozen**: it exists to be the fast local
tool and the build the published numbers come from. The decision is recorded
where a reader meets it, not only here:

- `README.md`, section "`src/` is frozen";
- the binary's own field guide, entry "State of this build" (`NATIVE_FROZEN` in
  `src/main.rs`), so nobody runs the exe believing it is the current lab.

Ported in the same pass, because a published claim rested on each:

- **scalar 2/3 dealiasing** (`src/solver.rs`, `step_scalar`). `turbulence.html`
  has always applied the 2/3 mask to the dye's advective term; the native path
  did not, so the barrier dye was aliased here and clean there. The velocity
  readouts cannot move (the dye is passive - only `self.scalar` is masked), and
  the unchanged `--scenes` rows confirm it.
- **the measured rod caveat** (`ROD_CAVEAT`). The old text asserted only that
  "the reversal is not guaranteed"; the new text names the measurements, so the
  frozen binary stops being vaguer than the docs.

Deliberately NOT ported (a freeze, not drift): named scene presets, the runtime
resolution selector, `Save PNG`, the dark-neutral colormap, and the barrier's
perturbation spin. Consequences worth knowing: this build keeps the bright
blue/orange vorticity map, its barrier scene starts with the perturbation OFF
(the browser starts it on and spinning), and its rod scene keeps the original
rod-at-`(pi,pi)` default - inside the strain node the rod caveat describes,
where the browser lab moved the rod to `(pi,pi/2)`.

## Open items

1. **CLOSED 2026-10-15** - frozen, above.
2. **Rod 45-deg switch: still not reproduced.** Documented as a measured
   negative (see "Verified numbers"). Settling it needs a *bigger physical box*,
   not just more steps: the solver is fixed to a doubly periodic `[0,2pi)^2`, so
   a domain many jet-widths across needs a box-length knob (a `L` in the solver,
   or all wavenumbers and length scales rescaled), and that is a code change
   first. Then the CPU. Measured cost on this box: 512^2 native ~= 99 ms/step
   against ~2.1 ms at 128^2, so a 512^2 run of 40k steps is ~1 h single-core and
   a rod-angle x viscosity sweep of the ~10-12 configurations the original
   negative result used is ~10-15 h; at 1024^2 (needed to keep 128^2-equivalent
   resolution in a 4x box) expect ~4x that per run. **Not started** - this
   machine is shared today. If it is ever run, the stationary-regime lesson
   applies: nu = 5e-3 had 6% window-to-window scatter, nu = 1e-4 had 334%.
3. **Jets: zonal signature real but modest** at this box size. Same box-length
   limitation as item 2, same cost profile. Documented; nothing to re-measure.
4. **NEW: the rod strain-node numbers are not reproducible as written.**
   `README.md` said `|S| = 0.067` at `(pi,pi)` against `12.0` at `(pi,pi/2)`;
   this note used to say `0.028` against `12-15`. Those two disagree with each
   other, and neither names the configuration it was measured in. Measured
   fresh (JS extraction, 128^2, rod scene, diagnostics every 25 steps, 6000
   steps), the developed turbulent state has **no node at all**:

   | configuration | `|S|` at (pi,pi) | `|S|` at (pi,pi/2) | ratio | axis |
   |---|---|---|---|---|
   | rod on, no band (shipped rod scene), t=44.2 | 20.02 | 11.75 | 0.59 | 26.88 deg |
   | rod off, banded drive 0.25, t=24.5 | 23.16 | 50.45 | 2.18 | 19.10 deg |
   | rod on, banded drive 0.25, t=23.5 | 44.29 | 41.31 | 0.93 | -44.20 deg |

   The **clean node needs the laminar limit**, and there it is exact - rod
   scene, rod off, banded drive `bandAmp = 1.0`, `nu = 0.03`, `drag = 0.10`,
   4000 steps at 128^2: `|S| = 0.0000` at (pi,pi) against `5.0011` at (pi,pi/2)
   (ratio ~4e9, not 180) with the ambient axis at **45.00 deg**. That is the
   recipe a reader needs, and it is now in README's rod bullet. Note the rod
   scene's own settings (`nu = 2e-5`, `drag = 0.05`) do not have a clean node to
   measure: the banded state goes unstable (`umax` 7.73 by t = 21, `|S|` ~34 at
   both points). So the qualitative claim survives and is stronger than the old
   numbers made it look, but **the `0.067`/`12.0` pair should be treated as
   unverified** until someone re-derives it with a recorded configuration.
5. **NEW: two README "Verified"-block errors, corrected there.** The `jets` row
   printed `u/v=1.23 band=0.036`; at the scene's default beta = 50 the run gives
   `u/v=1.55 band=0.016`, which agrees with the README's own prose ("about 1.6")
   and not with the table. Also "monotone through k = 1..7" and "positive for
   k >= 9" were both too strong: `Pi(7) = -1.96e-3` sits above `Pi(6) =
   -2.31e-3`, and the positive band ends near k = 26, past which `|Pi| < 2e-4`
   and the sign flips bin to bin.

## The lab has no public URL (Kevin's standing question, answered)

**No.** Checked 2026-10-15 from the repo and the account:

- `git remote -v` is empty - both commits (`a8bd642`, `5d24d17`) are local-only;
  `git branch -a` shows only `master`.
- No `.github/`, no workflow, no `CNAME`, no `docs/`, no `gh-pages` branch, no
  Pages configuration of any kind in the repo.
- Nothing of it is published elsewhere either: `gh api repos/kevinelong/{sim,
  turb2d,turbulence,sim-lab}` all 404, and a code search scoped to the account
  returns `total_count = 0` for both `filename:turbulence.html` and `turb2d`.
  (`brine-shrimp.html` has 0 matches on public GitHub overall.)

So there is no live URL to link to, and no published single-file HTML anywhere.

**Cheapest honest publication, if it is wanted:** the two HTML files are single
files with zero external references, so they already run from `file://` and need
no build, no bundler, no server and no workflow. Dropping `turbulence.html` and
`brine-shrimp.html` into one directory of an already Pages-served public repo -
`kevinelong.github.io` is public with Pages enabled - serves them at
`https://kevinelong.github.io/<dir>/turbulence.html` in a single commit. They
must go in the *same* directory: they cross-link by relative path. Required
before anything is made public: `LICENSE.md` = FSL-1.1-MIT (FSL is
source-available, NOT OSI open source - never describe it as open source).
Publishing is a deploy and a publicity decision, so it waits for an explicit
go-ahead; nothing was published or made public here.

## Verified numbers (observed 2026-10-15, this box)

### Native app - the note's own commands

`--scenes 6000 128` (**148.2 s wall** with nothing else running; 203.9 s in the
first, contended run; all rows `finite=true`). Five of the six rows reproduce the
previously published block to every printed digit; the `jets` row did not and is
corrected in README:

```
cascade   t=109.64 umax=2.049  E=4.8842e-01 Pi_inv=-2.3205e-03 u/v=1.03 band=0.041
rod       t= 77.67 umax=3.351  E=7.9579e-01 Pi_inv=-2.1783e-03 u/v=1.02 band=0.036
swimmers  t=120.00 umax=1.446  E=2.1858e-01 Pi_inv=-7.2670e-04 u/v=0.99 band=0.022
mhd       t=120.00 umax=0.770  E=1.4774e-01 Pi_inv=+2.6e-31     u/v=999.0 band=1.000
jets      t=120.00 umax=0.688  E=4.0907e-02 Pi_inv=-5.3559e-05 u/v=1.55 band=0.016
barrier   t=120.00 umax=0.910  E=2.0638e-01 Pi_inv=-2.5e-29     u/v=999.0 band=1.000
```

`--selftest 6000 128` (**50.3 s** uncontended, 64.0 s contended; `steps=30000` =
6000 burn-in + 4x6000 averaged, `samples=2400`; the table in README): `Pi(k) < 0`
for k = 1..7 (inverse), `Pi(k) > 0` for k = 9..26 (forward), energy peak at
k = 8. Rows are in README. Both runs above were repeated against the final
committed binary and came back bit-identical, which is the seeded-RNG
determinism the note relies on.
`--selftest 1500 128` (12.8 s, end t = 68.27) already shows the same signs, so
the forward band is established well before the 6000-step run's window.

**Not re-run today, deliberately:** the interactive eframe path (the README's
`cargo run --release -- --frames 60 --n 256` smoke run and the per-scene UI
claim). It opens a window on a shared machine, which would take focus from
Kevin's other work. The rebuild changed field-guide *text* only (a new entry and
a rewritten caveat string), so nothing on that path can have broken; re-run it
at the keyboard if the UI matters.

`--one cascade 200 512`: 19.8 s for 200 steps, i.e. **~99 ms/step at 512^2**
against ~2.1 ms at 128^2 (the 30000-step `--selftest` runs at 2.1 ms/step) - the
basis for the cost estimates in items 2 and 3. Re-measured with nothing else
running: 19.8 s against 19.1 s under contention, so contention is not the
dominant effect here.

### Browser and JS

`window.LAB`, headless Chromium, `file:///C:/Users/kevin/sim/turbulence.html`:

```
title "Turbulence Lab - six 2D scenes", scene cascade, n 128, renderM 512,
synthMax 512, synthEvery 2, exactScale false, external refs 0
300 solver steps: 9.34 ms/step  (the session note records 7.7 ms - see caveats)
```

JS extraction (the documented trick, run under **node**, since bun is not
installed on this box), 128^2 cascade. Three runs, because the *averaging
window* turns out to decide the forward-band sign:

```
averaged FROM t=0, 6000 steps (t=53.6):   Pi(1..7) all < 0
    -4.06e-4 -2.46e-3 -3.50e-3 -3.85e-3 -3.73e-3 -3.58e-3 -3.27e-3
    Pi(9..12) -1.46e-3 -1.66e-3 -1.78e-3 -1.93e-3        <- still negative
    E(k) peaks at k = 8

averaged FROM t=0, 15000 steps (t=137.7): Pi(1..7) all < 0
    -5.78e-4 -1.78e-3 -3.68e-3 -4.05e-3 -3.18e-3 -2.49e-3 -2.05e-3
    Pi(9..16) -2.59e-4 -1.77e-4 -5.18e-4 -7.30e-4        <- still negative
    E(k) peaks at k = 8

LATE window, burn to t=90.9 then average to t=184.6 - the shape --selftest
uses (burn in, reset, average a long tail):
    Pi(1..7) -2.41e-5 +1.97e-4 -1.13e-3 -1.89e-3 -1.10e-3 -5.44e-4 -1.62e-4
    Pi(8..16) +1.46e-3 +1.55e-3 +1.32e-3 +8.98e-4 +9.36e-4   <- positive
    E(k) peaks at k = 8
```

Two things to take from that. First, the JS forward band **is** positive once
the average excludes the start-up transient (the Rust `--selftest 1500 128`,
ending at t=68.3, is already positive at k = 9..26), so the earlier
"JS disagrees with Rust" reading was an averaging-window artifact, not a
physics difference: averaged from t=0 the transient dominates and the band reads
negative.

Second, in a late window `k = 1` and `k = 2` sit at the noise floor
(`|Pi| < 2e-4` there, against ~2e-3 at k = 4) and `k = 2` can read slightly
positive. The inverse band k = 3..7 stays solidly negative in every window;
`test/turbulence.test.mjs` averages **from t = 0** (`cascadeRun` resets the
average before stepping), which is the window where all of `k = 1..7` is
negative, and that is what its assertion covers.

The extraction is what `test/turbulence.test.mjs` uses as a fixture, so the JS
side is pinned by `node --test`. Each of these runs is ~1-2.5 min of node at
128^2, single core.

### Rod experiment (negative result, with controls) - unchanged

- ambient strain axis with the banded drive: 45.00 deg (readout 44.0 deg after
  400 steps, converging), stable
- rod-off flux ~1e-17, so the rod is the only source
- strain NODE at `(pi,pi)`: the `|S|` question is **item 4** - the numbers in
  the older note are not reproducible without a recipe
- flux is not stationary at nu=1e-4 (window-to-window scatter 334%); stationary
  at nu=5e-3 (6%)
- in the stationary regime the flux varies smoothly by 22x with the rod's
  ABSOLUTE angle but never changes sign, and two orientations that pure-shear
  symmetry requires to be equivalent differ by 22x. The stresslet-strain
  coupling predicts zero at both 0 and 90 deg yet those differ 22x. So the
  effect is not the alignment mechanism the paper reports.

### Other numbers still to trust (from the 2026-10-05 session, not re-run)

Costs at 128^2 in the browser: one field synthesis 1 ms (m=128), 3.5 ms
(m=256), 20.7 ms (m=512); canvas draw calls ~0 ms (queued). Rendering accuracy
vs the true band-limited field: bilinear 14.1%, exact-256 3.8%, exact-512 0.8%
(live default), exact-1024 0.0% (export only). Browser vs native: JS ~= wasm
(1.05-1.16x), both ~2.9x slower than native, SIMD128 buys 1.05-1.21x, and a
canvas blit is 0.4% of a step.

## Bugs found and fixed (do not reintroduce)

2026-10-15: the native scalar advection was not 2/3-dealiased (see the freeze
section) - the barrier dye was aliased in `turb2d.exe` and clean in the browser.

2026-10-05 list:

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

Observed 2026-10-15: **10 tests, 10 pass, 0 fail, 0 skipped, 105.9 s** (that run
overlapped another job, so treat it as an upper bound; the native-binary test
inside it took 71.3 s against 50.3 s for the same selftest on its own). The
native-binary test ran rather than skipping, i.e. `target/release/turb2d.exe`
was present and the published cascade signature still holds. Ten tests, stdlib
only (no dependencies, no browser). Nine run the real `Solver` lifted out of
`turbulence.html`; one pins the native binary.

- `test/turbulence.test.mjs` extracts the solver the way this file documents -
  slice the HTML between the section markers and evaluate it, with a small DOM
  stub because `configure` lives in "scene setup" and `synthesize` in
  "plumbing", outside the solver section. It pins: the power-of-two guard (96
  and 192 throw, 64/128/256/512 do not), the grid-size selector offering only
  powers of two, `forceModes` staying **empty until `configure()` fills it**
  (the trap at the end of this file), the cascade's inverse-band sign (Pi < 0
  for k=1..7) with the energy peak at the forcing scale, `synthesize`'s
  exactness (the coarse lattice comes back **bit for bit**, residual 0), the
  Laplacian property the whole synthesis path exists for (bilinear: exactly 0 in
  every cell interior and non-zero only on the cell boundaries; exact synthesis:
  uniform, ratio ~1.00), and the accuracy ladder's ordering with the live
  default under 1%. Its cascade run averages **from t = 0**, which is the window
  where `Pi < 0` holds across all of `k = 1..7`; in a late window `k = 1..2` sit
  at the noise floor (measured 2026-10-15), so the assertion is about that
  documented run shape.
- `test/rust-turb2d.test.mjs` runs `target/release/turb2d.exe --selftest 6000
  128` (~45 s) and pins README.md's "Verified" block: Pi < 0 for k=1..7,
  Pi > 0 for k >= 9, energy peak at k=8. It **skips with a message** naming the
  build command when the binary is absent, so a missing exe is visible rather
  than silently green.

The accuracy ladder's absolute percentages are deliberately NOT asserted: they
are convention- and frame-dependent (measured here: 2.4% / 0.7% / 0.19% / 0.00%
for 128 / 256 / 512 / 1024, against the README's 14.1 / 3.8 / 0.8 / 0.0 - a
consistent ~5x, from the reference and the snapshot the in-browser measurement
used). The order and the smallness of the live default are what the test holds.

Each guard was falsified before it was trusted: removing the power-of-two throw,
commenting `s.rebuildForce()` out of `configure`, forcing `synthesize`'s scale
to 1, and flipping the flux sign each turn the matching test red.

## How to re-verify

The native exe is rebuilt with `cargo build --release` (~19 s incremental).
Shell note for this box: a bare relative `./target/release/turb2d.exe` can fail
with a spurious "command not found", and `bash -c 'cd /c/Users/kevin/sim && ...'`
fails because that `bash` is WSL (where the path is `/mnt/c/...`). Use the
absolute path, or run it from Python with `cwd=` set:

```
python -c "import subprocess; subprocess.run(['C:/Users/kevin/sim/target/release/turb2d.exe','--scenes','6000','128'])"

# Rust app: six-scene sanity, with E, umax, inverse-band flux, bandedness
C:/Users/kevin/sim/target/release/turb2d.exe --scenes 6000 128     # 204 s, rows above

# Rust physics check (inverse cascade) - 64 s
C:/Users/kevin/sim/target/release/turb2d.exe --selftest 6000 128

# one scene, one grid: also the cheapest way to time a grid size
C:/Users/kevin/sim/target/release/turb2d.exe --one cascade 200 512   # ~99 ms/step

# performance harness (native; wasm build target is wasm32-unknown-unknown)
cd bench && cargo run --release --bin benchnative -- 256 300

# the whole JS/native contract, 10 tests, 106 s
node --test

# browser: open either HTML file directly (file://). No server needed.
#   turbulence.html exposes window.LAB for verification:
#     LAB.solver, LAB.renderM, LAB.renderField, LAB.synthMax, LAB.exactScale,
#     LAB.switchScene, LAB.setRes
```

The browser has a watchdog that kills long synchronous loops, so keep any
in-page loop to a few hundred steps; drive it through `window.LAB` for contract
checks and short timings only.

The fastest way to run solver experiments is to extract the solver out of the
HTML. **bun is not installed on this box** - the same slice runs under node
(the form `test/turbulence.test.mjs` uses as its fixture, which is the version
to copy):

```js
import { readFileSync } from "node:fs";
const html = readFileSync("C:/Users/kevin/sim/turbulence.html", "utf8");
const M1 = "// ---------------------------------------------------------------------- rng";
const M2 = "// -------------------------------------------------------------- scene setup";
const { Solver, TAU } = new Function(html.slice(html.indexOf(M1), html.indexOf(M2)) + "\nreturn { Solver, TAU };")();
```

Note a bare `new Solver(n)` has **empty `forceModes`** - `configure()` is what
calls `rebuildForce()`. Forgetting it gives a plausible, slowly decaying,
unforced flow that looks like a bug in the physics.

## Measurement caveats

- Headless Chromium rasterises in software, so its canvas *paint* cost (~10 ms)
  is an artifact of the environment and much cheaper in a GPU-composited browser.
  Trust the JS-side numbers (step, synthesis) and the ratios, not absolute fps.
- rAF is vsync-capped at 60 fps, so any frame under 16.7 ms reads as exactly 60.
- Absolute timings drift within a long benchmarking session; compare within a
  run, not across runs. Today's browser step cost was 9.34 ms against the 7.7 ms
  recorded on 2026-10-05 - the same code, so that difference is the machine, not
  a change.
- Runs today overlapped each other on a shared machine (the probe and the Rust
  runs). The printed physics values are deterministic and unaffected (the RNG is
  seeded - five of six `--scenes` rows reproduce a five-day-old block exactly);
  every wall-clock number above is an upper bound under contention.
- Screenshots from the browser tool are re-encoded (WebP), which adds an 8-pixel
  block artifact of its own - 0.996 blockiness on the app canvas vs 1.116 after
  encoding. Use `Save PNG` in the app for image comparisons.

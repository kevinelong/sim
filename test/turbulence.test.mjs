// Tests for the turb2d lab, run with the stdlib runner only:
//
//   node --test
//
// They pin the numbers RESUME.md records, by extracting the real Solver out of
// turbulence.html and running it - not a copy. The extraction is the one
// RESUME.md documents: slice the HTML between the section markers and evaluate
// it. Two sections are needed, because `configure` lives in "scene setup" and
// `synthesize` in "plumbing", both outside the solver section the note slices:
//
//   [colormaps .. field guide]  -> TAU, rng, fft, Solver, SCENES, configure
//   [plumbing  .. knobs]        -> synthMax/exactScale, pow2ceil, synthesize
//
// "plumbing" opens by grabbing canvases off the DOM, so a tiny stub is provided
// (the app's rendering and boot sections are deliberately NOT evaluated).
//
// Verified against turbulence.html at 1256 lines. If a section marker is
// renamed these tests fail loudly rather than silently testing nothing.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const HTML = readFileSync(join(here, '..', 'turbulence.html'), 'utf8');

const CTX = `{ setTransform(){}, fillRect(){}, clearRect(){}, createImageData:(m)=>({data:new Uint8ClampedArray(m*m*4)}), putImageData(){}, getImageData:(x,y,w,h)=>({data:new Uint8ClampedArray(w*h*4)}), drawImage(){}, save(){}, restore(){}, beginPath(){}, closePath(){}, moveTo(){}, lineTo(){}, stroke(){}, fill(){}, arc(){}, fillText(){}, measureText:()=>({width:0}), setLineDash(){}, fillStyle:'', strokeStyle:'', lineWidth:1, font:'', textAlign:'', textBaseline:'', globalAlpha:1 }`;
const DOM_STUB = `
const __ctx = ${CTX};
const __el = () => ({ getContext: () => __ctx, width:0, height:0, style:{}, clientWidth:270, clientHeight:92,
  textContent:'', innerHTML:'', value:'', appendChild(){}, addEventListener(){}, setAttribute(){},
  getBoundingClientRect:()=>({left:0,top:0,width:0,height:0}), classList:{add(){},remove(){},toggle(){}}, options:[], checked:false });
const document = { getElementById:()=>__el(), createElement:()=>__el(), querySelector:()=>__el(),
  querySelectorAll:()=>[], addEventListener(){}, body:__el(), documentElement:__el() };
const window = { devicePixelRatio:1, innerWidth:1600, innerHeight:900, addEventListener(){}, removeEventListener(){} };
const requestAnimationFrame = () => 0;
`;

const markerIndex = (label) => {
  const m = HTML.match(new RegExp('^// -+ ' + label + '$', 'm'));
  assert.ok(m, `section marker "${label}" is gone from turbulence.html - the extraction below needs updating`);
  return HTML.indexOf(m[0]);
};

const LAB = new Function(
  DOM_STUB +
  HTML.slice(markerIndex('colormaps'), markerIndex('field guide')) + '\n' +
  HTML.slice(markerIndex('plumbing'), markerIndex('knobs')) + '\n' +
  'return { Solver, TAU, SCENES, configure, synthesize, mkPlan, pow2ceil, synthMax, synthEvery, exactScale };'
)();

const N = 128;

/** A cascade run: n=128, the scene the docs quote, diagnosed every 25 steps. */
function cascadeRun(steps) {
  const s = new LAB.Solver(N);
  LAB.configure(s, 'cascade');
  s.resetAverage();
  for (let i = 0; i < steps; i++) {
    s.step();
    if (i % 25 === 0) s.diagnostics();
  }
  return s;
}

/** Zero-pad the spectrum to m x m and inverse transform - the app's own path. */
function synth(solver, m) {
  const re = new Float64Array(m * m), im = new Float64Array(m * m);
  const t1 = new Float64Array(m * m), t2 = new Float64Array(m * m);
  LAB.synthesize(solver.sre, solver.sim, N, m, re, im, t1, t2, LAB.mkPlan(m));
  return re;
}

const bilerp = (f, m, u, v) => {
  const X = u * m, Y = v * m;
  const x0 = Math.max(0, Math.min(m - 1, Math.floor(X))), y0 = Math.max(0, Math.min(m - 1, Math.floor(Y)));
  const x1 = Math.min(m - 1, x0 + 1), y1 = Math.min(m - 1, y0 + 1);
  const fx = X - x0, fy = Y - y0;
  return f[y0 * m + x0] * (1 - fx) * (1 - fy) + f[y0 * m + x1] * fx * (1 - fy)
       + f[y1 * m + x0] * (1 - fx) * fy + f[y1 * m + x1] * fx * fy;
};

const lap = (f, m, i, j) =>
  f[(j + 1) * m + i] + f[(j - 1) * m + i] + f[j * m + i + 1] + f[j * m + i - 1] - 4 * f[j * m + i];

// ---------------------------------------------------------------------------
// The solver the page ships

test('the page ships a Solver and TAU = 2*pi', () => {
  assert.equal(typeof LAB.Solver, 'function');
  assert.equal(LAB.TAU, Math.PI * 2);
  assert.deepEqual(Object.keys(LAB.SCENES), ['cascade', 'rod', 'swimmers', 'mhd', 'jets', 'barrier']);
});

test('the grid-size guard rejects non powers of two', () => {
  // RESUME.md: the resolution selector once offered 96 and 192, "which are not
  // powers of two - the radix-2 FFT silently produced NaN". The guard is what
  // turned that into a loud error, so it is pinned for exactly those two.
  for (const bad of [96, 192, 100, 130]) {
    assert.throws(() => new LAB.Solver(bad), /grid size must be a power of two \(radix-2 FFT\): got \d+/,
      `Solver(${bad}) must throw`);
  }
  for (const ok of [64, 128, 256, 512]) {
    assert.doesNotThrow(() => new LAB.Solver(ok), `Solver(${ok}) must be accepted`);
  }
});

test('the resolution selector offers only powers of two', () => {
  // The guard above is only a backstop; the UI must not offer the bad sizes at
  // all. Read them out of the page rather than trusting the guard alone.
  const row = HTML.match(/grid size<\/label><select id="k_res">([\s\S]*?)<\/select>/);
  assert.ok(row, 'the grid-size selector is gone from turbulence.html');
  const sizes = [...row[1].matchAll(/<option value="(\d+)"/g)].map((m) => Number(m[1]));
  assert.deepEqual(sizes, [64, 128, 256, 512]);
  for (const s of sizes) assert.equal(s & (s - 1), 0, `${s} is not a power of two`);
});

test('configure() is what fills forceModes (the documented trap)', () => {
  const s = new LAB.Solver(N);
  assert.equal(s.forceModes.length, 0, 'a bare Solver has no forcing modes');
  LAB.configure(s, 'cascade');
  assert.ok(s.forceModes.length > 0, 'configure() must call rebuildForce()');
  for (const i of s.forceModes) {
    assert.ok(Math.abs(s.kmag[i] - s.kf) < 0.75, 'forcing modes sit in a shell around kf');
    assert.equal(s.mask[i], 0, 'forcing modes must not be dealiased away');
  }
});

// ---------------------------------------------------------------------------
// The physics RESUME.md quotes

test('cascade: the inverse band has the documented sign', () => {
  // RESUME.md, "Verified numbers to trust": the cascade scene has Pi(k) < 0 for
  // k=1..7 (the inverse cascade) with the energy peak at the forcing
  // wavenumber kf=8. t here is ~70, the time RESUME.md's JS row was read at.
  const s = cascadeRun(7500);
  assert.ok(s.t > 60 && s.t < 90, `unexpected elapsed time t=${s.t}`);
  for (let k = 1; k <= 7; k++) {
    assert.ok(s.avgFlux[k] < 0,
      `Pi(${k}) must be negative below the forcing band, got ${s.avgFlux[k].toExponential(2)}`);
  }
  let peak = 1;
  for (let k = 2; k <= N / 2; k++) if (s.avgE[k] > s.avgE[peak]) peak = k;
  assert.ok(peak >= 6 && peak <= 10, `energy must peak at the forcing scale (kf=8), got k=${peak}`);
});

test('cascade: the flow does not decay away (the unforced trap)', () => {
  // RESUME.md: "a bare `new Solver(n)` has empty forceModes ... Forgetting it
  // gives a plausible, slowly decaying, unforced flow that looks like a bug in
  // the physics." So: configured, the flow is sustained; unconfigured, it decays.
  const forced = cascadeRun(1500);
  assert.ok(forced.umax > 0.2, `forced flow should be sustained, umax=${forced.umax}`);
  assert.ok(forced.energy() > 0);

  const bare = new LAB.Solver(N);
  for (let i = 0; i < 1500; i++) bare.step();
  assert.ok(bare.energy() < forced.energy(),
    'an unconfigured solver must end up weaker than a forced one');
});

// ---------------------------------------------------------------------------
// The synthesis path, and why it exists

test('synthesize is exact: the coarse lattice is reproduced bit for bit', () => {
  // RESUME.md: "resampling the synthesized field back at the coarse points
  // reproduces the original bit for bit". Exactly - no tolerance.
  const s = new LAB.Solver(N);
  LAB.configure(s, 'cascade');
  for (let i = 0; i < 400; i++) s.step();

  const coarse = synth(s, N);
  for (const m of [256, 512, 1024]) {
    const f = synth(s, m), step = m / N;
    let worst = 0;
    for (let y = 0; y < N; y++) for (let x = 0; x < N; x++) {
      worst = Math.max(worst, Math.abs(f[(y * step) * m + x * step] - coarse[y * N + x]));
    }
    assert.equal(worst, 0, `m=${m} must reproduce the coarse field exactly, worst residual ${worst}`);
  }
});

test('exact synthesis has no bilinear lattice: its Laplacian is uniform', () => {
  // This is the property the whole synthesis path exists for (RESUME.md and the
  // plumbing comment). Bilinear upscaling of an n x n raster is locally affine
  // inside each coarse cell, so its Laplacian is EXACTLY zero there and non-zero
  // only on the cell-boundary lines - a visible square lattice. The exact
  // synthesis must instead give a uniform Laplacian (ratio 1.00x).
  const s = new LAB.Solver(N);
  LAB.configure(s, 'cascade');
  for (let i = 0; i < 400; i++) s.step();

  const M = 512, c = M / N;
  const coarse = synth(s, N);
  const bil = new Float64Array(M * M);
  for (let j = 0; j < M; j++) for (let i = 0; i < M; i++) bil[j * M + i] = bilerp(coarse, N, i / M, j / M);

  const means = (f) => {
    let int = 0, nInt = 0, bnd = 0, nBnd = 0;
    for (let j = 1; j < M - 1; j++) for (let i = 1; i < M - 1; i++) {
      const L = Math.abs(lap(f, M, i, j));
      if (i % c === 0 || j % c === 0) { bnd += L; nBnd++; } else { int += L; nInt++; }
    }
    return { int: int / nInt, bnd: bnd / nBnd };
  };

  const b = means(bil);
  assert.ok(b.int < 1e-9, `bilinear Laplacian must be 0 in cell interiors, got ${b.int}`);
  assert.ok(b.bnd > 1e-3, `bilinear Laplacian must be non-zero on cell boundaries, got ${b.bnd}`);

  const x = means(synth(s, M));
  assert.ok(x.int > 1e-3, `exact synthesis must NOT be flat, got ${x.int}`);
  const ratio = x.int / x.bnd;
  assert.ok(ratio > 0.9 && ratio < 1.1,
    `exact synthesis Laplacian must be uniform (ratio ~1.0), got ${ratio.toFixed(4)}`);
});

test('the synthesis accuracy ladder is monotone and the live default is tight', () => {
  // README/RESUME ladder. The live default is exact-512 then a cheap tail.
  // Absolute percentages depend on the frame and on the sampling convention
  // (the documented 14.1 / 3.8 / 0.8 were measured in-browser at one moment);
  // what must hold is the ORDER and the smallness of the default, so those are
  // what is asserted here.
  const s = new LAB.Solver(N);
  LAB.configure(s, 'cascade');
  for (let i = 0; i < 400; i++) s.step();

  const D = 1057;                        // the display side RESUME.md's table used
  const truth = synth(s, 1024);
  const truthAt = (i, j) => bilerp(truth, 1024, i / (D - 1), j / (D - 1));
  let ref = 0;
  for (let j = 0; j < D; j++) for (let i = 0; i < D; i++) { const v = truthAt(i, j); ref += v * v; }
  const refRms = Math.sqrt(ref / (D * D));

  const err = (m) => {
    const f = synth(s, m);
    let e = 0;
    for (let j = 0; j < D; j++) for (let i = 0; i < D; i++) {
      const v = bilerp(f, m, i / (D - 1), j / (D - 1)) - truthAt(i, j);
      e += v * v;
    }
    return 100 * Math.sqrt(e / (D * D)) / refRms;
  };

  const direct = err(N), e256 = err(256), e512 = err(512), e1024 = err(1024);
  assert.ok(direct > e256, `direct bilinear (${direct.toFixed(2)}%) must be worse than 256 (${e256.toFixed(2)}%)`);
  assert.ok(e256 > e512, `256 (${e256.toFixed(2)}%) must be worse than 512 (${e512.toFixed(2)}%)`);
  assert.ok(e512 > e1024, `512 (${e512.toFixed(2)}%) must be worse than 1024 (${e1024.toFixed(2)}%)`);
  assert.ok(e512 < 1, `the live default (exact 512) must be under 1%, got ${e512.toFixed(2)}%`);
  assert.ok(e1024 < 0.05, `the export path (exact 1024) must be essentially exact, got ${e1024.toFixed(2)}%`);
  assert.ok(LAB.synthMax >= 512, `synthMax must allow the measured sweet spot, got ${LAB.synthMax}`);
});

test('the injection spectrum agrees with the physical psi*S integral', () => {
  // The per-shell injection is Re(conj(psi)*S) summed over shells, with psi = -omega/k^2. That derivation
  // carries a normalisation constant that is easy to get wrong by a factor of nn, so it is measured against
  // an independent path: inverse-FFT psi, multiply by the source in physical space, average over the grid.
  const s = new LAB.Solver(32);
  LAB.configure(s, 'mhd');   // the banded Lorentz drive - a source with a known, non-zero injection
  s.step();
  s.step();
  s.diagnostics();
  const { spectral, physical } = s.injectionCheck();
  assert.ok(Math.abs(physical) > 0, 'the banded source must inject something for this to test anything');
  const ratio = spectral / physical;
  assert.ok(Math.abs(ratio - 1) < 1e-6,
    `spectral/physical = ${ratio} (expected 1): the injection scaling is off`);
});

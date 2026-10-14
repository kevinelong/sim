// Pins the native turb2d binary against the numbers README.md publishes.
//
//   node --test
//
// The binary is a build artifact, so this test SKIPS when it is absent rather
// than failing - but it is skipped loudly, naming the build command, so a
// missing exe is visible in the output instead of silently green.
//
// README.md's "Verified" block is `--selftest 6000 128`, and that is what runs
// here (~45 s, so this file is the slow one).
//
// The native app and the two HTML files are supposed to be the same algorithm;
// this is the only automated check that the native side still shows the
// published cascade signature.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const EXE = join(here, '..', 'target', 'release', 'turb2d.exe');

const STEPS = 6000, GRID = 128;

test('turb2d --selftest reproduces the published dual cascade', {
  skip: existsSync(EXE)
    ? false
    : `target/release/turb2d.exe is not built - run: cargo build --release (in ${join(here, '..')})`,
}, () => {
  const run = spawnSync(EXE, [`--selftest`, String(STEPS), String(GRID)], { encoding: 'utf8', timeout: 15 * 60 * 1000 });
  assert.equal(run.status, 0,
    `turb2d exited ${run.status}${run.error ? ` (${run.error.code}: ${run.error.message})` : ''}\n${run.stderr}`);
  const out = `${run.stdout || ''}${run.stderr || ''}`;

  const header = out.match(/^n=(\d+) t=([\d.]+) steps=(\d+) dt=([\d.eE+-]+) umax=([\d.eE+-]+) E=([\d.eE+-]+)/m);
  assert.ok(header, `no selftest header in the output:\n${out.slice(0, 400)}`);
  assert.equal(Number(header[1]), GRID);
  assert.ok(Number(header[3]) >= 1, 'the run must actually have stepped');

  const rows = [...out.matchAll(/^\s*(\d+)\s+([\d.eE+-]+)\s+([\d.eE+-]+)\s*$/gm)]
    .map((m) => ({ k: Number(m[1]), E: Number(m[2]), Pi: Number(m[3]) }));
  assert.ok(rows.length >= 20, `expected the E(k)/Pi(k) table, parsed ${rows.length} rows`);
  for (const r of rows) {
    assert.ok(Number.isFinite(r.E) && Number.isFinite(r.Pi), `k=${r.k} produced a non-finite value`);
  }

  const at = (k) => rows.find((r) => r.k === k);
  // README.md, "Verified": Pi(k) < 0 below the forcing band (the inverse
  // cascade), positive for k >= 9 (the forward cascade), energy peak at kf=8.
  for (let k = 1; k <= 7; k++) {
    assert.ok(at(k).Pi < 0, `Pi(${k}) must be negative in the inverse band, got ${at(k).Pi}`);
  }
  for (let k = 9; k <= 20; k++) {
    assert.ok(at(k).Pi > 0, `Pi(${k}) must be positive in the forward band, got ${at(k).Pi}`);
  }
  let peak = rows[0];
  for (const r of rows) if (r.E > peak.E) peak = r;
  assert.equal(peak.k, 8, `the energy must peak at the forcing wavenumber 8, got k=${peak.k}`);
});

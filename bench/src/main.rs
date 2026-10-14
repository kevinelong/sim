use benchcore::Sim;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let n: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(256);
    let steps: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(200);

    let mut s = Sim::new(n);
    for _ in 0..5 {
        s.step(); // warm up caches
    }
    let t0 = Instant::now();
    for _ in 0..steps {
        s.step();
    }
    let el = t0.elapsed().as_secs_f64();

    println!(
        "native  n={:<4} steps={:<5} ms_per_step={:8.4}  steps_per_s={:8.1}  checksum={:.10e}",
        n,
        steps,
        el * 1000.0 / steps as f64,
        steps as f64 / el,
        s.checksum()
    );
}

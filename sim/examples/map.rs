use std::path::Path;
use std::process::ExitCode;

use sim::{config::Cfg, grid::Grid, rng::Rngs};

/// Seed used when none is given on the command line
const DEFAULT_SEED: u64 = 42;

/// Generate a map and print it as ASCII, overlaying the nest and the piles.
///
/// Usage: `cargo run -p sim --example map -- [seed] [config_path]`
fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);

    let seed = match args.next() {
        Some(raw) => match raw.parse::<u64>() {
            Ok(seed) => seed,
            Err(_) => {
                eprintln!("invalid seed: {raw:?} (expected an integer)");
                return ExitCode::FAILURE;
            }
        },
        None => DEFAULT_SEED,
    };

    let config_path = args.next();
    let cfg = match &config_path {
        Some(path) => match Cfg::load(Path::new(path)) {
            Ok(cfg) => cfg,
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::FAILURE;
            }
        },
        None => Cfg::embedded(),
    };

    let dims = cfg.grid.dims();
    let mut rngs = Rngs::new(seed);
    let out = Grid::generate(&mut rngs, dims, &cfg.grid.generator);

    let config_label = config_path.as_deref().unwrap_or("<embedded>");
    println!(
        "map {}x{}  seed={seed}  config={config_label}",
        dims.w(),
        dims.h()
    );
    println!();
    print_map(&out.grid, out.nest, &out.piles);
    println!();
    println!("legend: . soil   # rock   ~ water   N nest   P pile");
    println!(
        "report: attempts={} fallback={} rock={} water={} reachable_soil={} draws={}",
        out.report.attempts,
        out.report.used_fallback,
        out.report.rock_cells,
        out.report.water_cells,
        out.report.reachable_soil,
        out.report.draws,
    );

    ExitCode::SUCCESS
}

/// Print the terrain top-down (`y = h - 1` first), with `N` on the nest and
/// `P` on every pile.
fn print_map(grid: &Grid, nest: (i32, i32), piles: &[(i32, i32)]) {
    let dims = grid.dims();
    let mut line = String::with_capacity(dims.w() as usize);
    for y in (0..dims.h()).rev() {
        line.clear();
        for x in 0..dims.w() {
            let glyph = if (x, y) == nest {
                'N'
            } else if piles.contains(&(x, y)) {
                'P'
            } else {
                grid.at(x, y).glyph() as char
            };
            line.push(glyph);
        }
        println!("{line}");
    }
}

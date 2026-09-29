use std::collections::VecDeque;

use crate::{
    config::GenCfg,
    dims::Dims,
    rng::{Flow, PRNGEngine, Rngs},
    terrain::Terrain,
};

/// 8-connected neighbourhood: N, NE, E, SE, S, SO, O, NO (y grows upward)
const DIR8: [(i32, i32); 8] = [
    (0, 1),
    (1, 1),
    (1, 0),
    (1, -1),
    (0, -1),
    (-1, -1),
    (-1, 0),
    (-1, 1),
];

/// 4-connected neighbourhood used for reachability: N, E, S, O (y grows upward)
const DIR4: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

/// Sentinel stored in `dist_water` for cells never reached by the water BFS
const WATER_UNREACHED: u8 = 255;

/// Largest depth the water BFS is allowed to store; farther cells stay `WATER_UNREACHED`
const WATER_MAX_DEPTH: u8 = 254;

/// Paint a filled disc of radius `radius` centered on `(cx, cy)`
fn paint_disc(cells: &mut [Terrain], dims: Dims, cx: i32, cy: i32, radius: u8, t: Terrain) {
    let r = radius as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r * r {
                continue;
            }
            let (x, y) = (cx + dx, cy + dy);
            if dims.in_bounds(x, y) {
                cells[dims.idx(x, y)] = t;
            }
        }
    }
}

/// Count the in-bounds 8-neighbours of `(x, y)` that are made of rock
fn rock_neighbours(cells: &[Terrain], dims: Dims, x: i32, y: i32) -> u32 {
    let mut count = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (x + dx, y + dy);
            if dims.in_bounds(nx, ny) && cells[dims.idx(nx, ny)] == Terrain::Rock {
                count += 1;
            }
        }
    }
    count
}

/// Build the terrain field for one attempt: rocks (fill, smooth, cleanup) then water
fn build_terrain(rng: &mut PRNGEngine, dims: Dims, opts: &GenCfg) -> Vec<Terrain> {
    let mut cells = vec![Terrain::Soil; dims.len()];

    // Rocks
    let rock_threshold = (opts.rock_fill_pct as u64 * u32::MAX as u64 / 100) as u32;
    for cell in cells.iter_mut() {
        *cell = if rng.next_u32() < rock_threshold {
            Terrain::Rock
        } else {
            Terrain::Soil
        };
    }

    // Smooth rocks
    let birth = ((opts.rock_fill_pct as u32 * 8).div_ceil(100) + 1).min(8);
    for _ in 0..opts.rock_smooth_iters {
        let mut new_cells = vec![Terrain::Soil; dims.len()];
        for y in 0..dims.h() {
            for x in 0..dims.w() {
                let n = rock_neighbours(&cells, dims, x, y);
                let i = dims.idx(x, y);
                new_cells[i] = if n >= birth {
                    Terrain::Rock
                } else {
                    Terrain::Soil
                };
            }
        }
        cells = new_cells;
    }

    // Remove isolated rocks
    for y in 0..dims.h() {
        for x in 0..dims.w() {
            let i = dims.idx(x, y);
            if cells[i] == Terrain::Rock
                && rock_neighbours(&cells, dims, x, y) < opts.rock_isolated_min as u32
            {
                cells[i] = Terrain::Soil;
            }
        }
    }

    // Water
    for _ in 0..opts.water_walkers {
        let mut x = rng.span(dims.w() as u32) as i32;
        let mut y = rng.span(dims.h() as u32) as i32;
        for _ in 0..opts.water_steps {
            let d = rng.next_u32() % 8;
            let (dx, dy) = DIR8[d as usize];
            let (nx, ny) = (x + dx, y + dy);
            if dims.in_bounds(nx, ny) {
                x = nx;
                y = ny;
            }
            paint_disc(
                &mut cells,
                dims,
                x,
                y,
                opts.water_brush_radius,
                Terrain::Water,
            );
        }
    }

    cells
}

/// Multi-source 4-neighbour BFS from every water cell, blocked by rock.
///
/// Returns the number of steps to the nearest water for each cell, or
/// [`WATER_UNREACHED`] when no water can be reached.
fn water_dist(cells: &[Terrain], dims: Dims) -> Vec<u8> {
    let mut dist = vec![WATER_UNREACHED; dims.len()];
    let mut queue = VecDeque::new();

    for y in 0..dims.h() {
        for x in 0..dims.w() {
            let i = dims.idx(x, y);
            if cells[i] == Terrain::Water {
                dist[i] = 0;
                queue.push_back((x, y));
            }
        }
    }

    while let Some((x, y)) = queue.pop_front() {
        let d = dist[dims.idx(x, y)];
        if d >= WATER_MAX_DEPTH {
            continue;
        }
        for (dx, dy) in DIR4 {
            let (nx, ny) = (x + dx, y + dy);
            if !dims.in_bounds(nx, ny) {
                continue;
            }
            let ni = dims.idx(nx, ny);
            if cells[ni] == Terrain::Rock || dist[ni] != WATER_UNREACHED {
                continue;
            }
            dist[ni] = d + 1;
            queue.push_back((nx, ny));
        }
    }

    dist
}

/// Squared distance between two cells
fn dist2(a: (i32, i32), b: (i32, i32)) -> i64 {
    let dx = (a.0 - b.0) as i64;
    let dy = (a.1 - b.1) as i64;
    dx * dx + dy * dy
}

/// Squared distance from `(x, y)` to the grid center
fn center_dist2(dims: Dims, x: i32, y: i32) -> i64 {
    let dx = (x - dims.w() / 2) as i64;
    let dy = (y - dims.h() / 2) as i64;
    dx * dx + dy * dy
}

/// Pick the nest: the water-reachable soil cell maximizing `(dist_water, -center_dist)`.
///
/// Ties are broken towards the smallest index (major-row order). Soil cells that
/// never reach water are excluded, so the nest is never sealed in a dry pocket.
fn pick_nest(cells: &[Terrain], dims: Dims, dist: &[u8]) -> Option<(i32, i32)> {
    let mut best: Option<(i32, i32, u8, i64)> = None;
    for i in 0..dims.len() {
        if cells[i] != Terrain::Soil || dist[i] == WATER_UNREACHED {
            continue;
        }
        let (x, y) = dims.xy(i);
        let cd = center_dist2(dims, x, y);
        let better = match best {
            None => true,
            Some((_, _, bd, bcd)) => dist[i] > bd || (dist[i] == bd && cd < bcd),
        };
        if better {
            best = Some((x, y, dist[i], cd));
        }
    }
    best.map(|(x, y, _, _)| (x, y))
}

/// First soil cell in major-row order, or `(0, 0)` if the map has no soil
fn first_soil(cells: &[Terrain], dims: Dims) -> (i32, i32) {
    for y in 0..dims.h() {
        for x in 0..dims.w() {
            if cells[dims.idx(x, y)] == Terrain::Soil {
                return (x, y);
            }
        }
    }
    (0, 0)
}

/// Try to place `opts.piles` piles with a bounded number of random attempts each.
///
/// Returns `None` as soon as a pile cannot be placed, which rejects the map.
fn try_place_piles(
    cells: &[Terrain],
    dims: Dims,
    dist: &[u8],
    nest: (i32, i32),
    opts: &GenCfg,
    rng: &mut PRNGEngine,
) -> Option<Vec<(i32, i32)>> {
    let min_water = opts.pile_min_dist_water;
    let min_nest = opts.pile_min_dist_nest as i64;
    let min_dist = opts.pile_min_dist as i64;
    let mut piles: Vec<(i32, i32)> = Vec::with_capacity(opts.piles as usize);

    for _ in 0..opts.piles {
        let mut placed = false;
        for _ in 0..opts.piles_max_tries {
            let x = rng.span(dims.w() as u32) as i32;
            let y = rng.span(dims.h() as u32) as i32;
            let i = dims.idx(x, y);
            if cells[i] != Terrain::Soil || dist[i] < min_water {
                continue;
            }
            if dist2((x, y), nest) < min_nest * min_nest {
                continue;
            }
            if piles
                .iter()
                .any(|&p| dist2((x, y), p) < min_dist * min_dist)
            {
                continue;
            }
            piles.push((x, y));
            placed = true;
            break;
        }
        if !placed {
            return None;
        }
    }

    Some(piles)
}

/// 4-neighbour BFS from `start` over soil only
fn reachable_soil(cells: &[Terrain], dims: Dims, start: (i32, i32)) -> Vec<bool> {
    let mut seen = vec![false; dims.len()];
    let start_i = dims.idx(start.0, start.1);
    if cells[start_i] != Terrain::Soil {
        return seen;
    }

    let mut queue = VecDeque::new();
    seen[start_i] = true;
    queue.push_back(start);

    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in DIR4 {
            let (nx, ny) = (x + dx, y + dy);
            if !dims.in_bounds(nx, ny) {
                continue;
            }
            let ni = dims.idx(nx, ny);
            if seen[ni] || cells[ni] != Terrain::Soil {
                continue;
            }
            seen[ni] = true;
            queue.push_back((nx, ny));
        }
    }

    seen
}

/// Count cells made of `t`
fn count_terrain(cells: &[Terrain], t: Terrain) -> u32 {
    cells.iter().filter(|c| **c == t).count() as u32
}

/// Check every acceptance rule for a candidate map
fn validate(
    cells: &[Terrain],
    dims: Dims,
    seen: &[bool],
    piles: &[(i32, i32)],
    opts: &GenCfg,
) -> bool {
    if !piles.iter().all(|&(x, y)| seen[dims.idx(x, y)]) {
        return false;
    }

    let total = dims.len() as u32;
    let total_soil = count_terrain(cells, Terrain::Soil);
    let reachable = seen.iter().filter(|s| **s).count() as u32;
    if reachable * 100 < opts.reachable_soil_min_pct as u32 * total_soil {
        return false;
    }

    let rock = count_terrain(cells, Terrain::Rock);
    if rock * 100 < opts.rock_cover_min_pct as u32 * total
        || rock * 100 > opts.rock_cover_max_pct as u32 * total
    {
        return false;
    }

    let water = count_terrain(cells, Terrain::Water);
    if water * 100 > opts.water_cover_max_pct as u32 * total {
        return false;
    }

    true
}

/// Deterministic fallback: place piles on the highest-`dist_water` soil cells.
///
/// Candidates are walked in decreasing `dist_water` then major-row order. Three
/// passes progressively relax the constraints (nest + inter-pile, nest only,
/// none) so the required number of piles is always reached when soil allows it.
fn fallback_place(
    cells: &[Terrain],
    dims: Dims,
    dist: &[u8],
    nest: (i32, i32),
    opts: &GenCfg,
) -> Vec<(i32, i32)> {
    let target = opts.piles as usize;
    let min_nest = opts.pile_min_dist_nest as i64;
    let min_dist = opts.pile_min_dist as i64;

    let mut candidates: Vec<usize> = (0..dims.len())
        .filter(|&i| cells[i] == Terrain::Soil)
        .collect();
    candidates.sort_by_key(|&i| (std::cmp::Reverse(dist[i]), i));

    let mut piles: Vec<(i32, i32)> = Vec::with_capacity(target);
    for stage in 0..3 {
        for &i in &candidates {
            if piles.len() >= target {
                break;
            }
            let (x, y) = dims.xy(i);
            if piles.contains(&(x, y)) {
                continue;
            }
            if stage < 2 && dist2((x, y), nest) < min_nest * min_nest {
                continue;
            }
            if stage < 1
                && piles
                    .iter()
                    .any(|&p| dist2((x, y), p) < min_dist * min_dist)
            {
                continue;
            }
            piles.push((x, y));
        }
        if piles.len() >= target {
            break;
        }
    }

    piles
}

/// A generated terrain grid, along with the water depth field used by flooding
#[derive(Debug, PartialEq, Eq)]
pub struct Grid {
    dims: Dims,
    cells: Vec<Terrain>,
    dist_water: Vec<u8>,
    water_level: u8,
}

/// Output of a map generation: the grid plus the named cells and a report
#[derive(Debug, PartialEq, Eq)]
pub struct GenOut {
    pub grid: Grid,
    pub nest: (i32, i32),
    pub piles: Vec<(i32, i32)>,
    pub report: GenReport,
}

/// Diagnostics for a generation run
#[derive(Debug, PartialEq, Eq)]
pub struct GenReport {
    /// How many whole maps were drawn before one was accepted
    pub attempts: u8,
    /// `true` when pile placement fell back to the deterministic mode
    pub used_fallback: bool,
    pub rock_cells: u32,
    pub water_cells: u32,
    pub reachable_soil: u32,
    /// `word_pos(Flow::Terrain)` after generation, for golden tests
    pub draws: u128,
}

impl Grid {
    /// Generate a map.
    ///
    /// Draws whole maps until one passes every rule, then falls back to a
    /// deterministic placement after `cfg.map_max_attempts` rejections. The
    /// fallback cannot fail while the map holds enough soil.
    pub fn generate(rngs: &mut Rngs, dims: Dims, cfg: &GenCfg) -> GenOut {
        let rng = rngs.get(Flow::Terrain);
        let mut last: Option<(Vec<Terrain>, Vec<u8>)> = None;

        for attempt in 1..=cfg.map_max_attempts {
            let cells = build_terrain(rng, dims, cfg);
            let dist = water_dist(&cells, dims);

            if let Some(nest) = pick_nest(&cells, dims, &dist) {
                if let Some(piles) = try_place_piles(&cells, dims, &dist, nest, cfg, rng) {
                    let seen = reachable_soil(&cells, dims, nest);
                    if validate(&cells, dims, &seen, &piles, cfg) {
                        let report = GenReport {
                            attempts: attempt,
                            used_fallback: false,
                            rock_cells: count_terrain(&cells, Terrain::Rock),
                            water_cells: count_terrain(&cells, Terrain::Water),
                            reachable_soil: seen.iter().filter(|s| **s).count() as u32,
                            draws: rng.word_pos(),
                        };
                        let grid = Grid {
                            dims,
                            cells,
                            dist_water: dist,
                            water_level: 0,
                        };
                        return GenOut {
                            grid,
                            nest,
                            piles,
                            report,
                        };
                    }
                }
            }

            last = Some((cells, dist));
        }

        // Fallback: deterministic, still zero draws.
        let (cells, dist) = last.expect("map_max_attempts is validated to be >= 1");
        let nest = pick_nest(&cells, dims, &dist).unwrap_or_else(|| first_soil(&cells, dims));
        let piles = fallback_place(&cells, dims, &dist, nest, cfg);
        let seen = reachable_soil(&cells, dims, nest);
        let report = GenReport {
            attempts: cfg.map_max_attempts,
            used_fallback: true,
            rock_cells: count_terrain(&cells, Terrain::Rock),
            water_cells: count_terrain(&cells, Terrain::Water),
            reachable_soil: seen.iter().filter(|s| **s).count() as u32,
            draws: rng.word_pos(),
        };
        let grid = Grid {
            dims,
            cells,
            dist_water: dist,
            water_level: 0,
        };
        GenOut {
            grid,
            nest,
            piles,
            report,
        }
    }

    /// Grid dimensions
    pub fn dims(&self) -> Dims {
        self.dims
    }

    /// Terrain cells in major-row order, for rendering and fingerprinting
    pub fn cells(&self) -> &[Terrain] {
        &self.cells
    }

    /// Current flooding level
    pub fn water_level(&self) -> u8 {
        self.water_level
    }

    /// Raise the flooding level. It can never decrease.
    pub fn set_water_level(&mut self, level: u8) {
        debug_assert!(
            level >= self.water_level,
            "water level must not decrease ({} -> {level})",
            self.water_level
        );
        self.water_level = level;
    }

    /// Terrain at `(x, y)`, or `Rock` when out of bounds
    #[inline]
    pub fn at(&self, x: i32, y: i32) -> Terrain {
        if self.dims.in_bounds(x, y) {
            self.cells[self.dims.idx(x, y)]
        } else {
            Terrain::Rock
        }
    }

    /// Terrain at `(x, y)`, or `None` when out of bounds
    #[inline]
    pub fn try_at(&self, x: i32, y: i32) -> Option<Terrain> {
        if self.dims.in_bounds(x, y) {
            Some(self.cells[self.dims.idx(x, y)])
        } else {
            None
        }
    }

    /// Whether `(x, y)` is flooded at the current water level
    #[inline]
    pub fn underwater(&self, x: i32, y: i32) -> bool {
        self.dist_water[self.dims.idx(x, y)] <= self.water_level
    }

    /// Whether `(x, y)` is dry soil
    #[inline]
    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        self.at(x, y).is_walkable() && !self.underwater(x, y)
    }

    /// Debug rendering, one line per row, top row (`y = h - 1`) first
    pub fn to_ascii(&self) -> String {
        let mut out = String::with_capacity((self.dims.w() as usize + 1) * self.dims.h() as usize);
        for y in (0..self.dims.h()).rev() {
            for x in 0..self.dims.w() {
                out.push(self.at(x, y).glyph() as char);
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Cfg;

    fn grid_of(dims: Dims, cells: Vec<Terrain>, dist: Vec<u8>, level: u8) -> Grid {
        Grid {
            dims,
            cells,
            dist_water: dist,
            water_level: level,
        }
    }

    #[test]
    fn at_and_try_at_out_of_bounds() {
        let dims = Dims::new(3, 2);
        let cells = vec![Terrain::Soil; dims.len()];
        let dist = vec![WATER_UNREACHED; dims.len()];
        let grid = grid_of(dims, cells, dist, 0);

        assert_eq!(grid.at(0, 0), Terrain::Soil);
        assert_eq!(grid.at(-1, 0), Terrain::Rock);
        assert_eq!(grid.at(0, 2), Terrain::Rock);
        assert_eq!(grid.try_at(2, 1), Some(Terrain::Soil));
        assert_eq!(grid.try_at(-1, 0), None);
    }

    #[test]
    fn underwater_and_walkable_follow_water_level() {
        let dims = Dims::new(2, 1);
        let cells = vec![Terrain::Soil, Terrain::Rock];
        let dist = vec![2, 0];
        let mut grid = grid_of(dims, cells, dist, 0);

        assert!(!grid.underwater(0, 0));
        assert!(grid.is_walkable(0, 0));
        assert!(!grid.is_walkable(1, 0));

        grid.set_water_level(2);
        assert!(grid.underwater(0, 0));
        assert!(!grid.is_walkable(0, 0));
    }

    #[test]
    #[should_panic]
    fn water_level_cannot_decrease() {
        let dims = Dims::new(1, 1);
        let mut grid = grid_of(dims, vec![Terrain::Soil], vec![WATER_UNREACHED], 2);
        grid.set_water_level(1);
    }

    #[test]
    fn to_ascii_is_top_down() {
        let dims = Dims::new(2, 2);
        let cells = vec![Terrain::Soil, Terrain::Water, Terrain::Rock, Terrain::Soil];
        let dist = vec![WATER_UNREACHED; dims.len()];
        let grid = grid_of(dims, cells, dist, 0);

        assert_eq!(grid.to_ascii(), "#.\n.~\n");
    }

    #[test]
    fn generate_is_deterministic() {
        let cfg = Cfg::embedded();
        let dims = cfg.grid.dims();
        let opts = &cfg.grid.generator;

        let first = Grid::generate(&mut Rngs::new(42), dims, opts);
        let second = Grid::generate(&mut Rngs::new(42), dims, opts);

        assert_eq!(first, second);
        assert!(
            !first.report.used_fallback,
            "the embedded config should accept a map"
        );
        assert_eq!(first.piles.len(), opts.piles as usize);
        assert!(dims.in_bounds(first.nest.0, first.nest.1));
    }

    #[test]
    fn fallback_place_fills_all_piles() {
        let cfg = Cfg::embedded();
        let opts = &cfg.grid.generator;
        let dims = Dims::new(64, 64);
        let cells = vec![Terrain::Soil; dims.len()];
        let dist = vec![10; dims.len()];

        let piles = fallback_place(&cells, dims, &dist, (0, 0), opts);

        assert_eq!(piles.len(), opts.piles as usize);
        for i in 0..piles.len() {
            for j in i + 1..piles.len() {
                assert_ne!(piles[i], piles[j]);
            }
        }
    }
}

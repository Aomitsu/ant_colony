use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{config::error::ConfigError, dims::Dims};

pub mod error;

/// Embedded constant file
const EMBEDDED_RON: &str = include_str!("../../constantes.ron");

/// Internal path for embedded constant file
const INTERNAL_PATH: &str = "<embedded>";

/// Root configuration, parsed from the embedded or on-disk constant file
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cfg {
    /// Grid configuration
    pub grid: GridCfg,
}

impl Cfg {
    /// Parse the embedded constant file, panicking if it is invalid
    pub fn embedded() -> Cfg {
        Self::embedded_result().expect("embedded constantes.ron invalid")
    }

    /// Parse the embedded constant file into a `Cfg`
    pub fn embedded_result() -> Result<Cfg, ConfigError> {
        Self::parse(EMBEDDED_RON, Path::new(INTERNAL_PATH))
    }

    /// Read and parse a config file from disk
    pub fn load(path: &Path) -> Result<Cfg, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text, path)
    }
    /// Parse RON text and validate the resulting config
    pub fn parse(text: &str, path: &Path) -> Result<Cfg, ConfigError> {
        let cfg: Cfg = ron::from_str(text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        cfg.validate(path)?;
        Ok(cfg)
    }
    /// Reject configs that break grid invariants
    fn validate(&self, path: &Path) -> Result<(), ConfigError> {
        let invalid = |message: String| ConfigError::Invariant {
            path: path.to_path_buf(),
            message,
        };

        if self.grid.w == 0 || self.grid.h == 0 {
            return Err(invalid(format!(
                "grid.w & grid.h needs to be >= 1 (actually {}x{})",
                self.grid.w, self.grid.h
            )));
        }
        if self.grid.generator.map_max_attempts == 0 {
            return Err(invalid(
                "grid.generator.map_max_attempts needs to be >= 1".to_string(),
            ));
        }
        if self.grid.generator.piles_max_tries == 0 {
            return Err(invalid(
                "grid.generator.piles_max_tries needs to be >= 1".to_string(),
            ));
        }

        let generator = &self.grid.generator;
        let percent_fields = [
            ("rock_fill_pct", generator.rock_fill_pct as u16),
            ("water_cover_max_pct", generator.water_cover_max_pct as u16),
            ("rock_cover_min_pct", generator.rock_cover_min_pct as u16),
            ("rock_cover_max_pct", generator.rock_cover_max_pct as u16),
            ("reachable_soil_min_pct", generator.reachable_soil_min_pct),
        ];
        for (name, value) in percent_fields {
            if value > 100 {
                return Err(invalid(format!(
                    "{name} needs to be <= 100 (actually {value})"
                )));
            }
        }
        if generator.rock_cover_min_pct > generator.rock_cover_max_pct {
            return Err(invalid(format!(
                "rock_cover_min_pct needs to be <= rock_cover_max_pct (actually {} > {})",
                generator.rock_cover_min_pct, generator.rock_cover_max_pct
            )));
        }
        Ok(())
    }
}

/// Grid dimensions and map generator settings
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GridCfg {
    /// Grid width in cells
    pub w: u16,
    /// Grid height in cells
    pub h: u16,
    /// Map generation settings
    pub generator: GenCfg,
}

impl GridCfg {
    /// Build the runtime dimensions from the grid config
    pub fn dims(&self) -> Dims {
        Dims::new(self.w, self.h)
    }
}

/// Map generation settings
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenCfg {
    /// Maximum number of whole-map draws before falling back
    pub map_max_attempts: u8,
    /// Target rock density used while generating
    pub rock_fill_pct: u8,
    /// Number of smoothing passes applied to the rock field
    pub rock_smooth_iters: u8,
    /// Minimum rock neighbours for a rock to survive cleanup
    pub rock_isolated_min: u8,
    /// Lower bound of the accepted final rock coverage, in percent
    pub rock_cover_min_pct: u8,
    /// Upper bound of the accepted final rock coverage, in percent
    pub rock_cover_max_pct: u8,
    /// Number of water walkers
    pub water_walkers: u8,
    /// Number of steps taken by each water walker
    pub water_steps: u16,
    /// Radius of the disc painted by a water walker
    pub water_brush_radius: u8,
    /// Maximum accepted final water coverage, in percent
    pub water_cover_max_pct: u8,
    /// Number of piles to place
    pub piles: u8,
    /// Radius of a pile, used for rendering only
    pub pile_radius: u8,
    /// Minimum distance between two piles
    pub pile_min_dist: u8,
    /// Minimum distance from a pile to the nearest water
    pub pile_min_dist_water: u8,
    /// Minimum distance from a pile to the nest
    pub pile_min_dist_nest: u8,
    /// Maximum number of placement attempts per pile
    pub piles_max_tries: u16,
    /// Minimum share of soil reachable from the nest, in percent
    pub reachable_soil_min_pct: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_config_is_valid() {
        let cfg = Cfg::embedded();
        assert!(cfg.grid.w > 0 && cfg.grid.h > 0);
        assert_eq!(cfg.grid.dims().w(), cfg.grid.w as i32);
        assert_eq!(cfg.grid.dims().len(), 128 * 96);
    }
}

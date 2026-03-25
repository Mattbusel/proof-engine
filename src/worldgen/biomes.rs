//! Biome classification from climate data.
//!
//! Maps (temperature, precipitation, elevation) to biome types using a
//! Whittaker-style classification scheme.

use super::Grid2D;

/// Biome types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Biome {
    Ocean,
    DeepOcean,
    CoralReef,
    Beach,
    TropicalRainforest,
    TropicalDryForest,
    Savanna,
    Desert,
    HotSteppe,
    Mediterranean,
    TemperateForest,
    TemperateRainforest,
    Grassland,
    ColdSteppe,
    BorealForest,   // Taiga
    Tundra,
    IceCap,
    Marsh,
    Mangrove,
    Alpine,
    VolcanicWaste,
    River,
    Lake,
}

impl Biome {
    /// Base color (RGBA) for map rendering.
    pub fn color(self) -> [f32; 4] {
        match self {
            Self::Ocean            => [0.1, 0.2, 0.6, 1.0],
            Self::DeepOcean        => [0.05, 0.1, 0.4, 1.0],
            Self::CoralReef        => [0.2, 0.5, 0.6, 1.0],
            Self::Beach            => [0.9, 0.85, 0.6, 1.0],
            Self::TropicalRainforest => [0.05, 0.5, 0.1, 1.0],
            Self::TropicalDryForest  => [0.3, 0.5, 0.15, 1.0],
            Self::Savanna          => [0.6, 0.65, 0.2, 1.0],
            Self::Desert           => [0.85, 0.75, 0.45, 1.0],
            Self::HotSteppe        => [0.7, 0.6, 0.3, 1.0],
            Self::Mediterranean    => [0.5, 0.6, 0.25, 1.0],
            Self::TemperateForest  => [0.15, 0.55, 0.15, 1.0],
            Self::TemperateRainforest => [0.1, 0.45, 0.2, 1.0],
            Self::Grassland        => [0.45, 0.6, 0.2, 1.0],
            Self::ColdSteppe       => [0.55, 0.55, 0.35, 1.0],
            Self::BorealForest     => [0.15, 0.35, 0.2, 1.0],
            Self::Tundra           => [0.6, 0.65, 0.6, 1.0],
            Self::IceCap           => [0.9, 0.92, 0.95, 1.0],
            Self::Marsh            => [0.3, 0.4, 0.25, 1.0],
            Self::Mangrove         => [0.2, 0.4, 0.15, 1.0],
            Self::Alpine           => [0.5, 0.5, 0.5, 1.0],
            Self::VolcanicWaste    => [0.3, 0.2, 0.2, 1.0],
            Self::River            => [0.15, 0.3, 0.7, 1.0],
            Self::Lake             => [0.2, 0.35, 0.65, 1.0],
        }
    }

    /// Whether this biome is water.
    pub fn is_water(self) -> bool {
        matches!(self, Self::Ocean | Self::DeepOcean | Self::CoralReef | Self::River | Self::Lake)
    }

    /// Habitability score for settlement placement (0-1).
    pub fn habitability(self) -> f32 {
        match self {
            Self::TemperateForest | Self::Grassland | Self::Mediterranean => 0.9,
            Self::TemperateRainforest | Self::Savanna | Self::TropicalDryForest => 0.7,
            Self::TropicalRainforest | Self::Marsh => 0.5,
            Self::BorealForest | Self::ColdSteppe | Self::HotSteppe => 0.4,
            Self::Beach | Self::Mangrove => 0.3,
            Self::Desert | Self::Tundra | Self::Alpine => 0.15,
            Self::IceCap | Self::VolcanicWaste => 0.02,
            _ => 0.0, // water
        }
    }

    /// Resource richness.
    pub fn resources(self) -> f32 {
        match self {
            Self::TropicalRainforest | Self::TemperateRainforest => 0.9,
            Self::TemperateForest | Self::BorealForest => 0.7,
            Self::Grassland | Self::Savanna => 0.5,
            Self::Mediterranean | Self::Marsh => 0.6,
            Self::Desert | Self::Tundra => 0.1,
            Self::Alpine | Self::VolcanicWaste => 0.2,
            _ => 0.3,
        }
    }
}

/// The biome assignment map.
#[derive(Debug, Clone)]
pub struct BiomeMap {
    pub width: usize,
    pub height: usize,
    pub biomes: Vec<Biome>,
}

impl BiomeMap {
    pub fn biome_at(&self, x: usize, y: usize) -> Biome {
        self.biomes[y * self.width + x]
    }

    /// Count cells of each biome type.
    pub fn distribution(&self) -> std::collections::HashMap<Biome, usize> {
        let mut map = std::collections::HashMap::new();
        for &b in &self.biomes {
            *map.entry(b).or_insert(0) += 1;
        }
        map
    }

    /// Find all cells of a given biome.
    pub fn find_biome(&self, target: Biome) -> Vec<(usize, usize)> {
        let mut cells = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                if self.biome_at(x, y) == target {
                    cells.push((x, y));
                }
            }
        }
        cells
    }
}

/// Classify biomes from heightmap, temperature, and precipitation.
pub fn classify(
    heightmap: &Grid2D,
    temperature: &Grid2D,
    precipitation: &Grid2D,
    sea_level: f32,
) -> BiomeMap {
    let w = heightmap.width;
    let h = heightmap.height;
    let mut biomes = Vec::with_capacity(w * h);

    for y in 0..h {
        for x in 0..w {
            let elev = heightmap.get(x, y);
            let temp = temperature.get(x, y);
            let precip = precipitation.get(x, y);

            let biome = if elev < sea_level - 0.15 {
                Biome::DeepOcean
            } else if elev < sea_level - 0.02 {
                if temp > 22.0 && precip > 0.3 {
                    Biome::CoralReef
                } else {
                    Biome::Ocean
                }
            } else if elev < sea_level + 0.02 {
                if temp > 20.0 && precip > 0.5 {
                    Biome::Mangrove
                } else {
                    Biome::Beach
                }
            } else if elev > 0.85 {
                if temp < -5.0 {
                    Biome::IceCap
                } else {
                    Biome::Alpine
                }
            } else {
                // Whittaker classification
                whittaker_classify(temp, precip)
            };

            biomes.push(biome);
        }
    }

    BiomeMap { width: w, height: h, biomes }
}

/// Whittaker biome diagram classification.
fn whittaker_classify(temp: f32, precip: f32) -> Biome {
    if temp < -10.0 {
        if precip > 0.3 { Biome::IceCap } else { Biome::Tundra }
    } else if temp < 0.0 {
        if precip > 0.4 { Biome::BorealForest } else { Biome::Tundra }
    } else if temp < 5.0 {
        if precip > 0.5 { Biome::BorealForest } else { Biome::ColdSteppe }
    } else if temp < 15.0 {
        if precip > 0.7 { Biome::TemperateRainforest }
        else if precip > 0.4 { Biome::TemperateForest }
        else if precip > 0.2 { Biome::Grassland }
        else { Biome::ColdSteppe }
    } else if temp < 22.0 {
        if precip > 0.6 { Biome::TemperateForest }
        else if precip > 0.3 { Biome::Mediterranean }
        else if precip > 0.15 { Biome::HotSteppe }
        else { Biome::Desert }
    } else {
        // Hot
        if precip > 0.7 { Biome::TropicalRainforest }
        else if precip > 0.4 { Biome::TropicalDryForest }
        else if precip > 0.2 { Biome::Savanna }
        else if precip > 0.1 { Biome::HotSteppe }
        else { Biome::Desert }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whittaker_hot_wet() {
        assert_eq!(whittaker_classify(28.0, 0.8), Biome::TropicalRainforest);
    }

    #[test]
    fn test_whittaker_cold_dry() {
        assert_eq!(whittaker_classify(-15.0, 0.1), Biome::Tundra);
    }

    #[test]
    fn test_whittaker_hot_dry() {
        assert_eq!(whittaker_classify(25.0, 0.05), Biome::Desert);
    }

    #[test]
    fn test_classify_ocean() {
        let hm = Grid2D::filled(4, 4, 0.1); // all ocean
        let temp = Grid2D::filled(4, 4, 20.0);
        let precip = Grid2D::filled(4, 4, 0.5);
        let bm = classify(&hm, &temp, &precip, 0.4);
        assert!(bm.biome_at(0, 0).is_water());
    }

    #[test]
    fn test_biome_habitability() {
        assert!(Biome::TemperateForest.habitability() > Biome::Desert.habitability());
        assert!(Biome::Ocean.habitability() == 0.0);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Terrain {
    Soil = 0,
    Rock = 1,
    Water = 2,
}

impl Terrain {
    pub fn is_walkable(self) -> bool {
        matches!(self, Terrain::Soil)
    }
    pub fn glyph(self) -> u8 {
        match self {
            Terrain::Soil => b'.',
            Terrain::Rock => b'#',
            Terrain::Water => b'~',
        }
    }
}

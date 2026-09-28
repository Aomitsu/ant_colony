#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dims {
    width: u16,
    height: u16,
}

impl Dims {
    pub fn new(width: u16, height: u16) -> Self {
        assert!(width > 0 && height > 0);
        Self { width, height }
    }
    pub fn w(self) -> i32 {
        self.width as i32
    }
    pub fn h(self) -> i32 {
        self.height as i32
    }
    pub fn len(self) -> usize {
        self.width as usize * self.height as usize
    }

    pub fn in_bounds(self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w() && y < self.h()
    }

    pub fn idx(self, x: i32, y: i32) -> usize {
        debug_assert!(self.in_bounds(x, y), "idx out of bounds: {x},{y}");
        (y as usize) * (self.width as usize) + (x as usize)
    }

    pub fn xy(self, i: usize) -> (i32, i32) {
        (
            (i % self.width as usize) as i32,
            (i / self.width as usize) as i32,
        )
    }
}

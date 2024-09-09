use std::fmt::{Display, Formatter};
use std::io;
use std::str::FromStr;
use std::sync::LazyLock;

// TODO: This palette isn't very accurate - should be easy to improve if a good resource can be found
const ANSI_PALETTE: &[((u8, u8), Color)] = &[
    ((0, 30), Color(0, 0, 0)),
    ((0, 31), Color(200, 0, 0)),
    ((0, 32), Color(0, 200, 0)),
    ((0, 33), Color(200, 200, 0)),
    ((0, 34), Color(0, 0, 200)),
    ((0, 35), Color(200, 0, 200)),
    ((0, 36), Color(0, 200, 200)),
    ((0, 37), Color(255, 255, 255)),
];


/// Color index -> ansi color lookup table. Generated at runtime.
static COLOR_LOOKUP: LazyLock<Vec<(u8, u8)>> = LazyLock::new(|| {
    let mut lookup = vec![(0, 0); 256*256*256];
    for r in 0..=255u8 {
        for g in 0..=255u8 {
            for b in 0..=255u8 {
                let this_col = Color(r, g, b);
                let index = this_col.lookup_index();

                lookup[index] = ANSI_PALETTE.iter().min_by_key(|(_, c)| {
                    c.dist2(this_col)
                }).expect("Palette is non-empty").0
            }
        }
    }

    lookup
});


/// A single rbg24 color
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(u8, u8, u8);


impl Color {
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self(r, g, b)
    }

    const fn lookup_index(self) -> usize {
        (self.0 as usize * 256 + (self.1 as usize)) * 256 + (self.2 as usize)
    }

    fn dist2(self, other: Color) -> u32 {
        let dr = (self.0 as u32).abs_diff(other.0 as u32);
        let dg = (self.1 as u32).abs_diff(other.1 as u32);
        let db = (self.2 as u32).abs_diff(other.2 as u32);

        dr*dr + dg*dg + db*db
    }

    pub fn rgb_interpolate(self, Color(or, og, ob): Self, alpha: f32) -> Self {
        let Color(tr, tg, tb) = self;

        // Basic linear interpolation in rgb, using fixed point arithmetic
        // (x*(255-255a) + y*255a) => 255(x*(1-a) + y*a)
        let beta = (alpha * 255f32) as u16;
        let gamma = 255 - beta;

        Color(
            (((tr as u16) * gamma + (or as u16) * beta) >> 8) as u8,
            (((tg as u16) * gamma + (og as u16) * beta) >> 8) as u8,
            (((tb as u16) * gamma + (ob as u16) * beta) >> 8) as u8,
        )
    }

    pub fn write_as_24bit_ansi<O>(self, mut output: O) -> io::Result<()>
        where O: io::Write {
        let Color(r, g, b) = self;
        write!(output, "\u{001B}[38;2;{r};{g};{b}m")
    }

    pub fn write_as_paletted_ansi<O>(self, mut output: O) -> io::Result<()>
        where O: io::Write {
        // Find closest
        let (a, b) = *COLOR_LOOKUP.get(self.lookup_index())
            .expect("All colors have corresponding palette value");

        write!(output, "\u{001B}[{a};{b}m")
    }
}


impl Default for Color {
    fn default() -> Self {
        Self::from_rgb(0, 0, 0)
    }
}


impl FromStr for Color {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 6 {
            Err("Color must have length 6")
        } else if let Ok(int) = u32::from_str_radix(value, 16) {
            let b = (int % 256) as u8;
            let g = ((int / 256) % 256) as u8;
            let r = ((int / 256) / 256) as u8;
            Ok(Self::from_rgb(r, g, b))
        } else {
            Err("Invalid hexadecimal")
        }
    }
}


impl Display for Color {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:>02X}{:>02X}{:>02X}", self.0, self.1, self.2)
    }
}

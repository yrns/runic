// If the shape is completely filled it never needs to be rotated...

// Need to store slices or make it generic over something? Or all
// shapes are static?

// pub const SHAPE1X1: Shape = Shape {
//     size: Vec2 { x: 1, y: 1 },
//     fill: bits![1],
// };

pub use glam::U16Vec2 as Vec2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub size: Vec2,
    pub fill: Vec<bool>,
}

#[allow(unused)]
const MAX_SLOT: u32 = (u16::MAX as u32).pow(2) - 1;
const _: () = assert!(MAX_SLOT <= u32::MAX);

impl Shape {
    pub fn new(size: impl Into<Vec2>, fill: bool) -> Self {
        let size = size.into();
        assert!(size.x > 0, "width greater than zero");
        assert!(size.y > 0, "height greater than zero");
        Self {
            size,
            fill: vec![fill; size.x as usize * size.y as usize],
        }
    }

    // TODO check size is appropriate for bits, e.g. first/last
    // row/col are empty
    // pub fn from_bits(width: usize, bytes: &[u8]) -> Self {
    //     assert!(bytes.len() % width == 0, "is rect");
    //     let fill = BitVec::from_bytes(bytes);
    //     Self {
    //         size: (width, fill.len() / width).into(),
    //         fill,
    //     }
    // }

    pub fn from_ones(width: usize, ones: impl IntoIterator<Item = u8>) -> Self {
        Self::from_width_slice(width, ones.into_iter().map(|a| a == 1))
    }

    pub fn from_width_slice(width: usize, fill: impl IntoIterator<Item = bool>) -> Self {
        let fill: Vec<_> = fill.into_iter().collect();
        assert!(fill.len() % width == 0, "is rect");
        Self {
            size: [width as u16, (fill.len() / width) as u16].into(),
            fill,
        }
    }

    pub fn width(&self) -> usize {
        self.size.x as usize
    }

    pub fn height(&self) -> usize {
        self.size.y as usize
    }

    pub fn area(&self) -> usize {
        self.size.element_product() as usize
    }

    pub fn contains(&self, pt: Vec2) -> bool {
        pt.x <= self.size.x && pt.y <= self.size.y
    }

    fn overlay_range(&self, other: &Shape, slot: usize) -> Option<std::ops::RangeInclusive<usize>> {
        let p1 = self.pos(slot);
        let p2 = p1 + other.size;
        (self.contains(p1) && self.contains(p2)).then(|| slot..=self.slot(p2 - Vec2::ONE))
    }

    pub fn overlay_mut(&mut self, other: &Shape, slot: usize, f: impl Fn(&mut bool, &bool)) {
        if let Some(r) = self.overlay_range(other, slot) {
            let w = self.width();
            let w2 = other.width();
            self.fill[r]
                .chunks_mut(w)
                .map(|row| &mut row[..w2])
                .zip(other.fill.chunks(w2))
                .for_each(|(r1, r2)| r1.iter_mut().zip(r2.iter()).for_each(|(a, b)| f(a, b)))
        } else {
            tracing::error!("overlay_mut range is empty!")
        }
    }

    pub fn paint(&mut self, other: &Shape, slot: usize) {
        // print!("{}+\n{}=\n", &self, other);
        assert!(slot <= self.fill.len(), "paint slot {slot} in range");
        self.overlay_mut(other, slot, |a, b| *a = *a || *b);
        // println!("{}", &self);
    }

    pub fn unpaint(&mut self, other: &Shape, slot: usize) {
        // print!("{}-\n{}=\n", &self, other);
        assert!(slot <= self.fill.len(), "unpaint slot {slot} in range");
        self.overlay_mut(other, slot, |a, b| *a = *a && !*b);
        // println!("{}", &self);
    }

    pub fn fits(&self, other: &Shape, slot: usize) -> bool {
        if let Some(r) = self.overlay_range(other, slot) {
            let w = other.width();
            self.fill[r]
                .chunks(self.width())
                .map(|row| &row[..w])
                .zip(other.fill.chunks(w))
                // Check that a is empty or b is empty for every cell
                // pair in the row.
                .all(|(r1, r2)| r1.iter().zip(r2.iter()).all(|(a, b)| !b || !a))
        } else {
            false
        }
    }

    /// Return slot for position.
    #[inline]
    pub fn slot(&self, pt: impl Into<Vec2>) -> usize {
        let pt = pt.into();
        pt.x as usize + pt.y as usize * self.width()
    }

    /// Return position for slot.
    #[inline]
    pub fn pos(&self, slot: usize) -> Vec2 {
        Vec2::new((slot % self.width()) as u16, (slot / self.width()) as u16)
    }

    /// Returns an iterator over filled slots.
    pub fn slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.fill
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.then_some(i))
    }

    pub fn rows(&self) -> impl Iterator<Item = &[bool]> + '_ {
        self.fill.as_slice().chunks(self.width())
    }

    // These are adapted from the image crate: https://github.com/image-rs/image/blob/master/src/imageops/affine.rs.

    // TODO: These can be done in half the operations in place w/ swapping.

    pub fn rotate90(&self) -> Self {
        let Vec2 { x: w, y: h } = self.size;
        let mut dest = Shape::new((h, w), false);
        let slice = &mut dest.fill;
        for y in 0..h {
            for x in 0..w {
                let b = self.fill[self.slot((x, y))];
                // dest.slot(h - y - 1, x)
                let slot = h - y - 1 + x * h;
                slice[slot as usize] = b;
            }
        }
        dest
    }

    pub fn rotate180(&self) -> Self {
        let Vec2 { x: w, y: h } = self.size;
        let mut dest = Shape::new((w, h), false);
        for y in 0..h {
            for x in 0..w {
                let b = self.fill[self.slot((x, y))];
                // dest.slot(w - x - 1, h - y - 1)
                let slot = w - x - 1 + (h - y - 1) * w;
                dest.fill[slot as usize] = b;
            }
        }
        dest
    }

    pub fn rotate270(&self) -> Self {
        let Vec2 { x: w, y: h } = self.size;
        let mut dest = Shape::new((h, w), false);
        for y in 0..h {
            for x in 0..w {
                let b = self.fill[self.slot((x, y))];
                // dest.slot(y, w - x - 1)
                let slot = y + (w - x - 1) * h;
                dest.fill[slot as usize] = b;
            }
        }
        dest
    }
}

impl std::fmt::Display for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.rows()
            .map(|r| {
                r.iter()
                    .map(|b| if *b { "■" } else { "□" })
                    .chain(std::iter::once("\n"))
                    .map(|x| write!(f, "{}", x))
                    .collect()
            })
            .collect()
    }
}

impl From<Vec2> for Shape {
    fn from(size: Vec2) -> Self {
        Shape::new(size, true)
    }
}

impl From<(usize, usize)> for Shape {
    fn from((w, h): (usize, usize)) -> Self {
        Shape::new(Vec2::new(w as u16, h as u16), true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bits() {
        let shape = Shape::from_ones(2, [1, 1, 1, 1]);
        assert!(shape.height() == 2);
    }

    #[test]
    #[should_panic]
    fn from_ones_non_rect() {
        let _ = Shape::from_ones(2, [1, 1, 1, 1, 1]);
    }

    #[test]
    fn fits() {
        let a = Shape::from_ones(4, [1, 1, 0, 0, 1, 1, 0, 0]);
        let b = Shape::from_ones(2, [1, 1, 1, 1]);
        assert!(a.fits(&b, a.slot((0, 0))) == false);
        assert!(a.fits(&b, a.slot((1, 0))) == false);
        assert!(a.fits(&b, a.slot((2, 0))) == true);
        assert!(a.fits(&b, a.slot((3, 0))) == false); // outside
    }

    #[test]
    fn paint() {
        let mut a = Shape::from_ones(4, [0, 0, 0, 0]);
        let b = Shape::from_ones(2, [1, 1]);
        a.paint(&b, 0);
        assert_eq!(a, Shape::from_ones(4, [1, 1, 0, 0]));
    }

    #[test]
    fn unpaint() {
        let mut a = Shape::from_ones(4, [0, 1, 1, 1]);
        let b = Shape::from_ones(2, [1, 1]);
        let res = Shape::from_ones(4, [0, 0, 1, 1]);
        a.unpaint(&b, 0);
        assert_eq!(a, res);
        a.unpaint(&b, 0);
        assert_eq!(a, res);
    }

    #[test]
    fn slots_iter() {
        let a = Shape::from_ones(2, [1, 1, 1, 1]);
        itertools::assert_equal(a.slots(), [0, 1, 2, 3]);
    }

    #[test]
    fn rotate() {
        let a = Shape::from_ones(2, [1, 0, 0, 0]);
        assert_eq!(a.rotate90(), Shape::from_ones(2, [0, 1, 0, 0]));
        assert_eq!(a.rotate180(), Shape::from_ones(2, [0, 0, 0, 1]));
        assert_eq!(a.rotate270(), Shape::from_ones(2, [0, 0, 1, 0]));

        let a = Shape::from_ones(3, [1, 0, 0]);
        assert_eq!(a.rotate90(), Shape::from_ones(1, [1, 0, 0]));
        assert_eq!(a.rotate180(), Shape::from_ones(3, [0, 0, 1]));
        assert_eq!(a.rotate270(), Shape::from_ones(1, [0, 0, 1]));
    }
}

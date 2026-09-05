// If the shape is completely filled it never needs to be rotated...

// Need to store slices or make it generic over something? Or all
// shapes are static?

// pub const SHAPE1X1: Shape = Shape {
//     size: Vec2 { x: 1, y: 1 },
//     fill: bits![1],
// };

// use bevy_ecs::template::*;
use bevy_math::UVec2;
use bevy_reflect::Reflect;

pub use bevy_math::UVec2 as Size;

#[derive(Clone, Debug, PartialEq, Eq, Reflect)]
pub struct Shape {
    pub(crate) size: Size,
    pub(crate) fill: Vec<bool>, // bit-vec is in the lockfile already.
}

/*
#[derive(Debug)]
pub struct ShapeTemplate {
    pub(crate) size: Size,
    pub(crate) fill: Vec<bool>,
}

impl Default for ShapeTemplate {
    fn default() -> Self {
        let Shape { size, fill } = Shape::from([[1]]);
        Self { size, fill }
    }
}

impl Template for ShapeTemplate {
    type Output = Shape;

    fn build_template(
        &self,
        _context: &mut TemplateContext,
    ) -> bevy_ecs::error::Result<Self::Output> {
        Ok(Shape {
            size: self.size.clone(),
            fill: self.fill.clone(),
        })
    }

    fn clone_template(&self) -> Self {
        ShapeTemplate {
            size: self.size.clone(),
            fill: self.fill.clone(),
        }
    }
}

impl<const X: usize, const Y: usize> From<[[u8; X]; Y]> for ShapeTemplate {
    fn from(value: [[u8; X]; Y]) -> Self {
        let Shape { size, fill } = value.into();
        Self { size, fill }
    }
}

impl From<(u32, u32)> for ShapeTemplate {
    fn from(value: (u32, u32)) -> Self {
        let Shape { size, fill } = value.into();
        Self { size, fill }
    }
}

impl FromTemplate for Shape {
    type Template = ShapeTemplate;
}
 */

// NOTE: Default only works because the fields are private.
impl Default for Shape {
    fn default() -> Self {
        Self::from([[1]])
    }
}

// impl From<(u32, u32)> for Shape {
//     fn from((x, y): (u32, u32)) -> Self {
//         Self {
//             size: UVec2 { x, y },
//             // Filled would be for items, empty for containers? Containers are never being populated via From? Maybe this is reversed?
//             fill: vec![false; (x * y) as usize],
//         }
//     }
// }

impl<const X: usize, const Y: usize> From<[[u8; X]; Y]> for Shape {
    fn from(fill: [[u8; X]; Y]) -> Self {
        const {
            assert!(X > 0);
            assert!(Y > 0);
        }

        Self {
            size: Size::new(u32::try_from(X).unwrap(), u32::try_from(Y).unwrap()),
            fill: fill.iter().flatten().map(|f| *f > 0).collect(),
        }
    }
}

// This is only useful if we change the type.
// const MAX_SLOT: u32 = (u16::MAX as u32).pow(2) - 1;
// const _: () = assert!(MAX_SLOT <= u32::MAX);

impl Shape {
    pub fn new(size: impl Into<Size>, fill: bool) -> Self {
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
            size: Size::new(width as u32, (fill.len() / width) as u32),
            fill,
        }
    }

    pub fn width(&self) -> usize {
        self.size.x as usize
    }

    pub fn height(&self) -> usize {
        self.size.y as usize
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn area(&self) -> usize {
        self.size.element_product() as usize
    }

    pub fn is_square(&self) -> bool {
        self.size.x == self.size.y
    }

    pub fn contains(&self, pt: UVec2) -> bool {
        pt.x <= self.size.x && pt.y <= self.size.y
    }

    // TODO: std::range::*?
    fn overlay_range(
        &self,
        other: &Shape,
        index: usize,
    ) -> Option<std::ops::RangeInclusive<usize>> {
        let p1 = self.slot(index);
        let p2 = p1 + other.size;
        (self.contains(p1) && self.contains(p2)).then(|| index..=self.index(p2 - UVec2::ONE))
    }

    pub fn overlay_mut(&mut self, other: &Shape, index: usize, f: impl Fn(&mut bool, &bool)) {
        if let Some(r) = self.overlay_range(other, index) {
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

    pub fn paint(&mut self, other: &Shape, index: usize) {
        // print!("{}+\n{}=\n", &self, other);
        assert!(index <= self.fill.len(), "paint slot {index} in range");
        self.overlay_mut(other, index, |a, b| *a = *a || *b);
        // println!("{}", &self);
    }

    pub fn unpaint(&mut self, other: &Shape, index: usize) {
        // print!("{}-\n{}=\n", &self, other);
        assert!(index <= self.fill.len(), "unpaint slot {index} in range");
        self.overlay_mut(other, index, |a, b| *a = *a && !*b);
        // println!("{}", &self);
    }

    pub fn fits(&self, other: &Shape, index: usize) -> bool {
        if !other.size.cmple(self.size).all() {
            return false;
        }

        if let Some(r) = self.overlay_range(other, index) {
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

    /// Return index for `slot` coords/position.
    pub fn index(&self, slot: UVec2) -> usize {
        (slot.x + slot.y * self.size.x) as usize
    }

    /// Return slot for `index`.
    pub fn slot(&self, index: usize) -> UVec2 {
        assert!(index < self.fill.len());
        let index = index as u32;
        let w = self.size.x;
        UVec2::new(index % w, index / w)
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
        let Size { x: w, y: h } = self.size;
        let mut dest = Shape::new((h, w), false);
        let slice = &mut dest.fill;
        for y in 0..h {
            for x in 0..w {
                let b = self.fill[self.index(UVec2::new(x, y))];
                // dest.slot(h - y - 1, x)
                let slot = h - y - 1 + x * h;
                slice[slot as usize] = b;
            }
        }
        dest
    }

    pub fn rotate180(&self) -> Self {
        let Size { x: w, y: h } = self.size;
        let mut dest = Shape::new((w, h), false);
        for y in 0..h {
            for x in 0..w {
                let b = self.fill[self.index(UVec2::new(x, y))];
                // dest.slot(w - x - 1, h - y - 1)
                let slot = w - x - 1 + (h - y - 1) * w;
                dest.fill[slot as usize] = b;
            }
        }
        dest
    }

    pub fn rotate270(&self) -> Self {
        let Size { x: w, y: h } = self.size;
        let mut dest = Shape::new((h, w), false);
        for y in 0..h {
            for x in 0..w {
                let b = self.fill[self.index(UVec2::new(x, y))];
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
        self.rows().try_for_each(|r| {
            r.iter()
                .map(|b| if *b { "■" } else { "□" })
                .chain(std::iter::once("\n"))
                .try_for_each(|x| write!(f, "{}", x))
        })
    }
}

impl From<Size> for Shape {
    fn from(size: Size) -> Self {
        Shape::new(size, false)
    }
}

impl From<(u32, u32)> for Shape {
    fn from((x, y): (u32, u32)) -> Self {
        Shape::new(Size::new(x, y), false)
    }
}

impl std::ops::Not for Shape {
    type Output = Self;

    fn not(mut self) -> Self::Output {
        self.fill.iter_mut().for_each(|slot| *slot = !*slot);
        self
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
        assert!(a.fits(&b, a.index(UVec2::new(0, 0))) == false);
        assert!(a.fits(&b, a.index(UVec2::new(1, 0))) == false);
        assert!(a.fits(&b, a.index(UVec2::new(2, 0))) == true);
        assert!(a.fits(&b, a.index(UVec2::new(3, 0))) == false); // outside
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

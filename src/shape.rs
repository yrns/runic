use std::ops::{Add, Sub};

use bitvec::prelude::*;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Vec2 {
    pub x: u8,
    pub y: u8,
}

impl Vec2 {
    pub fn len(&self) -> usize {
        self.x as usize * self.y as usize
    }
}

impl Add for Vec2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vec2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x.saturating_sub(rhs.x),
            y: self.y.saturating_sub(rhs.y),
        }
    }
}

impl From<(usize, usize)> for Vec2 {
    fn from(from: (usize, usize)) -> Self {
        Self {
            x: from.0 as u8,
            y: from.1 as u8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    size: Vec2,
    fill: BitVec,
}

impl Shape {
    pub fn new(size: impl Into<Vec2>, fill: bool) -> Self {
        let size = size.into();
        assert!(size.x > 0, "width greater than zero");
        assert!(size.y > 0, "height greater than zero");
        let len = size.len();
        Self {
            size,
            fill: BitVec::repeat(fill, len),
        }
    }

    // TODO check size is appropriate for bits, e.g. first/last
    // row/col are empty
    pub fn from_bits(width: usize, bits: &BitSlice) -> Self {
        assert!(bits.len() % width == 0, "is rect");
        let fill = BitVec::from_bitslice(bits);
        Self {
            size: (width, fill.len() / width).into(),
            fill,
        }
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.size.x as usize
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.size.y as usize
    }

    pub fn contains(&self, pt: impl Into<Vec2>) -> bool {
        let pt = pt.into();
        pt.x <= self.size.x && pt.y <= self.size.y
    }

    pub fn paint(&mut self, other: &Shape, pt: impl Into<Vec2>) {
        let pt = pt.into();
        let pt2 = pt + other.size;
        if self.contains(pt) && self.contains(pt2) {
            let start = self.slot(pt);
            let end = self.slot(pt2 - (1, 1).into());
            let w = self.width();
            self.fill[start..=end]
                .chunks_mut(w)
                .map(|row| &mut row[..(other.width())])
                .zip(other.fill.chunks(other.width()))
                .for_each(|(r1, r2)| {
                    r1.iter_mut()
                        .zip(r2.iter())
                        .for_each(|(mut a, b)| *a = *a || *b)
                })
        }
    }

    pub fn fits(&self, other: &Shape, pt: impl Into<Vec2>) -> bool {
        let pt = pt.into();
        let pt2 = pt + other.size;
        if self.contains(pt) && self.contains(pt2) {
            let start = self.slot(pt);
            let end = self.slot(pt2 - (1, 1).into());
            self.fill[start..=end]
                .chunks(self.width())
                .map(|row| &row[..(other.width())])
                .zip(other.fill.chunks(other.width()))
                // Check that a is empty or b is empty for every cell
                // pair in the row.
                .all(|(r1, r2)| r1.iter().zip(r2.iter()).all(|(a, b)| !b || !a))
        } else {
            false
        }
    }

    pub fn slot(&self, pt: Vec2) -> usize {
        pt.x as usize + pt.y as usize * self.width()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bits() {
        let shape = Shape::from_bits(2, bits![1, 1, 1, 1]);
        assert!(shape.height() == 2);
    }

    #[test]
    #[should_panic]
    fn from_bits_non_rect() {
        let _ = Shape::from_bits(2, bits![1, 1, 1, 1, 1]);
    }

    #[test]
    fn fits() {
        let a = Shape::from_bits(4, bits![1, 1, 0, 0, 1, 1, 0, 0]);
        let b = Shape::from_bits(2, bits![1, 1, 1, 1]);
        assert!(a.fits(&b, (0, 0)) == false);
        assert!(a.fits(&b, (1, 0)) == false);
        assert!(a.fits(&b, (2, 0)) == true);
        assert!(a.fits(&b, (3, 0)) == false); // outside
    }

    #[test]
    fn paint() {
        let mut a = Shape::from_bits(4, bits![0, 0, 0, 0]);
        let b = Shape::from_bits(2, bits![1, 1]);
        a.paint(&b, (0, 0));
        assert_eq!(a, Shape::from_bits(4, bits![1, 1, 0, 0]));
    }
}

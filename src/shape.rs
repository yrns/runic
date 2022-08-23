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

    fn overlay_range(&self, other: &Shape, p1: Vec2) -> Option<(usize, usize)> {
        let p2 = p1 + other.size;
        (self.contains(p1) && self.contains(p2))
            .then(|| (self.slot(p1), self.slot(p2 - (1, 1).into())))
    }

    pub fn overlay_mut(&mut self, other: &Shape, pt: Vec2, f: impl Fn(&mut bool, &bool)) {
        if let Some((start, end)) = self.overlay_range(other, pt) {
            let w = self.width();
            self.fill[start..=end]
                .chunks_mut(w)
                .map(|row| &mut row[..(other.width())])
                .zip(other.fill.chunks(other.width()))
                .for_each(|(r1, r2)| {
                    r1.iter_mut()
                        .zip(r2.iter())
                        .for_each(|(mut a, b)| f(&mut *a, &*b))
                })
        }
    }

    pub fn paint(&mut self, other: &Shape, pt: impl Into<Vec2>) {
        self.overlay_mut(other, pt.into(), |a, b| *a = *a || *b)
    }

    pub fn unpaint(&mut self, other: &Shape, pt: impl Into<Vec2>) {
        self.overlay_mut(other, pt.into(), |a, b| *a = *a && !*b)
    }

    pub fn fits(&self, other: &Shape, pt: impl Into<Vec2>) -> bool {
        if let Some((start, end)) = self.overlay_range(other, pt.into()) {
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

    pub fn pos(&self, slot: usize) -> Vec2 {
        (slot % self.width(), slot / self.width()).into()
    }

    pub fn pos_f32(&self, slot: usize, scale: f32) -> (f32, f32) {
        (
            (slot % self.width()) as f32 * scale,
            (slot / self.width()) as f32 * scale,
        )
    }

    /// Returns an iterator over filled slots.
    pub fn slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.fill
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.then_some(i))
    }
}

impl std::fmt::Display for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fill
            .as_bitslice()
            .chunks(self.width())
            .map(|r| {
                r.iter()
                    .map(|b| if *b { "\u{25A0}" } else { "\u{25A1}" })
                    .chain(std::iter::once("\n"))
                    .map(|x| write!(f, "{}", x))
                    .collect()
            })
            .collect()
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

    #[test]
    fn unpaint() {
        let mut a = Shape::from_bits(4, bits![0, 1, 1, 1]);
        let b = Shape::from_bits(2, bits![1, 1]);
        let res = Shape::from_bits(4, bits![0, 0, 1, 1]);
        a.unpaint(&b, (0, 0));
        assert_eq!(a, res);
        a.unpaint(&b, (0, 0));
        assert_eq!(a, res);
    }

    #[test]
    fn slots_iter() {
        let a = Shape::from_bits(2, bits![1, 1, 1, 1]);
        assert_eq!(a.slots().collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    }
}

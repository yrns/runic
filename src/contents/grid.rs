use super::*;

/// Contains items in a 2D grid.
// We want the shape to be required, but there is no way to do so (<https://github.com/bevyengine/bevy/issues/24739>).
#[derive(Component, Debug, Clone)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Debug))]
pub struct GridContents<const N: usize = 48> {
    /// If true, this grid only holds one item, but the size of that item can be any up to the maximum size.
    pub expands: bool,
    /// If true, show inline contents for the contained item.
    pub inline: bool,
    pub header: Option<String>, // Use Name?
    /// The shape describes the dimensions of the container and which slots are filled.
    pub shape: Shape,
}

// I hate this but we need a default for now.
impl Default for GridContents {
    fn default() -> Self {
        Self {
            expands: false,
            inline: false,
            header: None,
            shape: Shape::new((2, 2), false),
        }
    }
}

impl<const N: usize> GridContents<N> {
    pub fn new(size: impl Into<Size>) -> Self {
        Self {
            expands: false,
            inline: false,
            header: None,
            shape: Shape::new(size.into(), false),
        }
    }

    pub fn with_expands(mut self, expands: bool) -> Self {
        self.expands = expands;
        self
    }

    pub fn with_inline(mut self, inline: bool) -> Self {
        self.inline = inline;
        self
    }

    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.header = Some(header.into());
        self
    }

    /// Returns the size of the section. For expanding sections `Self::size` is the maximum item size and the size returned here is always 1x1.
    pub fn size(&self) -> UVec2 {
        if self.expands {
            UVec2::ONE
        } else {
            self.shape.size()
        }
    }

    /// Returns number of slots. Expanding section only have one slot.
    pub fn slots(&self) -> usize {
        if self.expands { 1 } else { self.shape.area() }
    }

    /// Paint item shape.
    pub fn insert(&mut self, slot: Slot, item: &Item) {
        let index = self.index(slot);
        assert!(index < self.slots(), "slot in contents length");
        self.shape.paint(&item.shape, index);
    }

    /// Unpaint item shape.
    pub fn remove(&mut self, slot: Slot, item: &Item) {
        let index = self.index(slot);
        assert!(index < self.slots(), "slot in contents length");
        self.shape.unpaint(&item.shape, index);
    }

    /// Returns an index for `slot`. Always returns 0 for expanding sections.
    pub fn index(&self, Slot(slot): Slot) -> usize {
        // Expanding only ever has one.
        if self.expands {
            0
        } else {
            self.shape.index(slot)
        }
    }

    // This got moved to find_section_slot?
    // fn fits(&self, id: Entity, item: &Item, index: usize, source: &DragSource) -> bool {
    //     // Check if the shape fits here. When moving within
    //     // one container, use the cached shape with the
    //     // dragged item (and original rotation) unpainted.
    //     let shape = match source {
    //         Some((source_id, _, shape)) if id == *source_id => shape,
    //         _ => &self.shape,
    //     };

    //     shape.fits(&item.shape, index)
    // }
}

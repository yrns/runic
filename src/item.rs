use egui::{
    emath::Rot2, CursorIcon, Id, Image, InnerResponse, Pos2, Rect, Rgba, Sense, TextureId, Ui, Vec2,
};

use crate::*;
use bevy_ecs::prelude::*;

#[derive(Component, Clone, Debug)]
pub struct Item<T> {
    pub rotation: ItemRotation,
    pub shape: shape::Shape,
    pub icon: TextureId,
    pub flags: T,
}

#[derive(Debug)]
pub enum ItemResponse<T> {
    DragToItem((usize, Entity)),
    NewDrag(DragItem<T>),
}

impl<T: Clone> Item<T> {
    // Flags are required since the empty (default) flags allow the item to fit any container
    // regardless of the container's flags.
    pub fn new(flags: T) -> Self {
        Self {
            rotation: Default::default(),
            shape: Shape::new([1, 1], true),
            // TODO better default texture
            icon: Default::default(),
            flags,
        }
    }

    pub fn with_icon(mut self, icon: TextureId) -> Self {
        self.icon = icon;
        self
    }

    /// Set the item shape and unset its rotation.
    pub fn with_shape(mut self, shape: impl Into<Shape>) -> Self {
        self.shape = shape.into();
        self.rotation = ItemRotation::None;
        self
    }

    pub fn with_flags(mut self, flags: impl Into<T>) -> Self {
        self.flags = flags.into();
        self
    }

    /// Set the item's rotation and apply it to its shape.
    pub fn with_rotation(mut self, r: ItemRotation) -> Self {
        self.rotation = r;
        self.rotate();
        self
    }

    /// Size in pixels.
    pub fn size(&self) -> Vec2 {
        (self.shape.size.as_vec2() * SLOT_SIZE).as_ref().into()
    }

    /// The width of the shape (in slots).
    pub fn width(&self) -> usize {
        self.shape.width()
    }

    /// Return slot for offset in pixels.
    pub fn slot(&self, offset: Vec2) -> usize {
        self.shape.slot(to_size(offset / SLOT_SIZE))
    }

    const PIVOT: Vec2 = Vec2::splat(0.5);

    pub fn body(
        &self,
        id: Entity,
        drag_item: &Option<DragItem<T>>,
        ui: &mut Ui,
    ) -> InnerResponse<Vec2> {
        let eid = Id::new(id);

        // let (dragging, item) = match drag_item.as_ref() {
        //     Some(drag) if drag.item.id == self.id => (true, &drag.item),
        //     _ => (false, self),
        // };
        let dragging = drag_item.as_ref().is_some_and(|d| d.id == id);

        // Allocate the original size so the contents draws
        // consistenly when the dragged item is scaled.
        let size = self.size();
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());

        // Scale down slightly even when not dragging in lieu of baking a border into every item
        // icon. TODO This needs to be configurable.
        let drag_scale = ui
            .ctx()
            // ui.id() is diff while dragging...
            .animate_bool(eid.with("scale"), dragging);

        if ui.is_rect_visible(rect) {
            // This size is a hint and isn't used since the image is always(?) already loaded.
            let image = Image::new((self.icon, size));
            let image = if dragging {
                image.tint(Rgba::from_rgba_premultiplied(1.0, 1.0, 1.0, 0.8))
            } else {
                image
            };

            // Scale down if dragging from center.
            let rect = Rect::from_center_size(
                rect.center(),
                rect.size() * egui::lerp(1.0..=0.88, drag_scale),
            );

            // For non-square shapes, we need to un-rotate the paint_at rect. This seems like a bug
            // in egui...
            match self.rotation {
                ItemRotation::None => image.paint_at(ui, rect),
                r @ ItemRotation::R180 => image.rotate(r.angle(), Self::PIVOT).paint_at(ui, rect),
                r @ _ => image
                    .rotate(r.angle(), Self::PIVOT)
                    .paint_at(ui, Rect::from_center_size(rect.center(), rect.size().yx())),
            };
        }

        InnerResponse::new(size, response)
    }

    /// `slot` is the slot we occupy in the container.
    pub fn ui(
        &self,
        slot: usize,
        id: Entity,
        name: &str,
        drag_item: &Option<DragItem<T>>,
        ui: &mut Ui,
    ) -> Option<ItemResponse<T>> {
        let eid = ui.id().with(id);
        let p = ui.ctx().pointer_latest_pos();

        // This was a bug: "being dragged" is false on the frame in which we release the button. This means that if we dragged the item onto itself, it would return a hover and prevent a move.
        // let drag = ui.ctx().is_being_dragged(id);

        match drag_item.as_ref() {
            // This item is being dragged. We never return an item response.
            Some(drag) if drag.id == id => {
                // Half of these cursors do not work in X11. See about using custom cursors in bevy and sharing that w/ bevy_egui. See also: https://github.com/mvlabat/bevy_egui/issues/229
                ui.output_mut(|o| o.cursor_icon = CursorIcon::Grab);

                // Draw the dragged item in a new area so it does not affect the size of the contents, which could occur with a large item rotated outside the bounds of the contents.
                if let Some(p) = p {
                    // from egui::containers::show_tooltip_area_dyn
                    egui::containers::Area::new(eid)
                        .fixed_pos(p - drag.offset.1)
                        .interactable(false)
                        // TODO Restrict to ContainerSpace?
                        //.constrain(true) // this is wrong
                        .show(ui.ctx(), |ui| self.body(id, drag_item, ui));
                }

                None
            }
            // This item is not being dragged (but maybe something else is).
            _ => {
                let response = self.body(id, drag_item, ui).response;

                // Figure out what slot we're in, see if it's filled, don't sense drag if not.
                p.filter(|p| response.rect.contains(*p))
                    .map(|p| p - response.rect.min)
                    .map(|offset| (self.slot(offset), offset))
                    .filter(|(slot, _)| {
                        self.shape.fill.get(*slot).map(|b| *b).unwrap_or_else(|| {
                            // FIX This occurs somewhere on drag/mouseover.
                            tracing::error!(
                                "point {:?} slot {} out of shape fill {}",
                                p,
                                slot,
                                self.shape
                            );
                            false
                        })
                    })
                    .map(|offset| {
                        if drag_item.is_some() {
                            Some(ItemResponse::DragToItem((slot, id)))
                        } else {
                            ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
                            let response = ui.interact(response.rect, eid, Sense::drag());
                            let response = response.on_hover_text_at_pointer(name);
                            response.drag_started().then(|| {
                                ItemResponse::NewDrag(DragItem {
                                    id,
                                    item: self.clone(),
                                    // Contents::ui sets this.
                                    source: None,
                                    offset,
                                })
                            })
                        }
                    })
                    .flatten()
            }
        }
    }

    // Apply rotation to shape.
    fn rotate(&mut self) {
        match self.rotation {
            ItemRotation::None => (),
            ItemRotation::R90 => self.shape = self.shape.rotate90(),
            ItemRotation::R180 => self.shape = self.shape.rotate180(),
            ItemRotation::R270 => self.shape = self.shape.rotate270(),
        };
    }
}

// impl std::fmt::Display for Item {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.write_str(&self.name)?;
//         f.write_str(" ")?;

//         // FIX: require Display for flags?
//         //f.write_fmt(format_args!("{}", self.flags))

//         f.debug_list()
//             .entries(self.flags.iter_names().map(|f| f.0)) // format_args!("{}", f.0)
//             .finish()
//     }
// }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ItemRotation {
    #[default]
    None,
    R90,
    R180,
    R270,
}

impl ItemRotation {
    pub const R0_UVS: [Pos2; 4] = [
        egui::pos2(0.0, 0.0),
        egui::pos2(1.0, 0.0),
        egui::pos2(0.0, 1.0),
        egui::pos2(1.0, 1.0),
    ];

    pub const R90_UVS: [Pos2; 4] = [
        egui::pos2(0.0, 1.0),
        egui::pos2(0.0, 0.0),
        egui::pos2(1.0, 1.0),
        egui::pos2(1.0, 0.0),
    ];

    pub const R180_UVS: [Pos2; 4] = [
        egui::pos2(1.0, 1.0),
        egui::pos2(0.0, 1.0),
        egui::pos2(1.0, 0.0),
        egui::pos2(0.0, 0.0),
    ];

    pub const R270_UVS: [Pos2; 4] = [
        egui::pos2(1.0, 0.0),
        egui::pos2(1.0, 1.0),
        egui::pos2(0.0, 0.0),
        egui::pos2(0.0, 1.0),
    ];

    pub fn increment(&self) -> Self {
        match self {
            Self::None => Self::R90,
            Self::R90 => Self::R180,
            Self::R180 => Self::R270,
            _ => Self::None,
        }
    }

    pub fn angle(&self) -> f32 {
        match *self {
            Self::None => 0.0,
            Self::R90 => 90.0_f32.to_radians(),
            Self::R180 => 180.0_f32.to_radians(),
            Self::R270 => 270.0_f32.to_radians(),
        }
    }

    pub fn rot2(&self) -> Rot2 {
        Rot2::from_angle(self.angle())
    }

    pub fn uvs(&self) -> &[Pos2; 4] {
        match *self {
            ItemRotation::None => &Self::R0_UVS,
            ItemRotation::R90 => &Self::R90_UVS,
            ItemRotation::R180 => &Self::R180_UVS,
            ItemRotation::R270 => &Self::R270_UVS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 1234, rotate x times, swap the last two to match the quad uvs:
    fn gen_uvs(r: usize) -> [egui::Pos2; 4] {
        let mut uvs = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        uvs.rotate_right(r); // not const yet
        uvs.swap(2, 3); // not const yet
        uvs.map(|(x, y)| egui::pos2(x, y)) // never?
    }

    #[test]
    fn uvs() {
        assert_eq!(gen_uvs(0), ItemRotation::R0_UVS);
        assert_eq!(gen_uvs(1), ItemRotation::R90_UVS);
        assert_eq!(gen_uvs(2), ItemRotation::R180_UVS);
        assert_eq!(gen_uvs(3), ItemRotation::R270_UVS);
    }
}

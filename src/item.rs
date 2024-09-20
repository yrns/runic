use bevy_ecs::prelude::*;
use bevy_egui::egui::{
    self, emath::Rot2, text::LayoutJob, Align, CursorIcon, FontSelection, Id, InnerResponse,
    Modifiers, Pos2, Rect, Rgba, RichText, Sense, Style, TextureId, Ui, Vec2,
};
use bevy_reflect::prelude::*;

use crate::*;

/// An item.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct Item<T> {
    pub rotation: ItemRotation,
    /// The shape represents this items dimensions (and filled "slots" in case it is not rectangular).
    pub shape: Shape,
    pub flags: T,
}

impl<T> Item<T> {
    // Flags are required since the empty (default) flags allow the item to fit any container
    // regardless of the container's flags.
    pub fn new(flags: T) -> Self {
        Self {
            rotation: Default::default(),
            shape: Shape::new([1, 1], true),
            flags,
        }
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
    pub fn size(&self, slot_dim: f32) -> Vec2 {
        (self.shape.size.as_vec2() * slot_dim).as_ref().into()
    }

    /// The width of the shape (in slots).
    pub fn width(&self) -> usize {
        self.shape.width()
    }

    /// Return slot for offset in pixels.
    pub fn slot(&self, offset: Vec2) -> usize {
        self.shape.slot(to_size(offset))
    }

    const PIVOT: Vec2 = Vec2::splat(0.5);

    /// Show item body (icon, etc.).
    pub fn body(
        &self,
        id: Entity,
        drag: Option<&DragItem<T>>,
        icon: TextureId,
        slot_dim: f32,
        ui: &mut Ui,
    ) -> InnerResponse<Vec2> {
        let eid = Id::new(id);

        // let (dragging, item) = match drag_item.as_ref() {
        //     Some(drag) if drag.item.id == self.id => (true, &drag.item),
        //     _ => (false, self),
        // };
        let dragging = drag.is_some_and(|d| d.id == id);

        // Allocate the original size so the contents draws consistenly when the dragged item is scaled.
        let size = self.size(slot_dim);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());

        // Scale down slightly even when not dragging in lieu of baking a border into every item icon. TODO Configurable?
        let drag_scale = ui
            .ctx()
            // ui.id() is diff while dragging...
            .animate_bool(eid.with("scale"), dragging);

        if ui.is_rect_visible(rect) {
            // This size is a hint and isn't used since the image is always(?) already loaded.
            let image = egui::Image::new((icon, size));
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

            // For non-square shapes, we need to un-rotate the paint_at rect. This seems like a bug in egui...
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

    /// Show item. `slot` is the slot we occupy in the container.
    pub fn ui(
        &self,
        slot: usize,
        id: Entity,
        name: &str,
        drag: Option<&DragItem<T>>,
        icon: TextureId,
        slot_dim: f32,
        ui: &mut Ui,
    ) -> Option<ContentsResponse<T>>
    where
        T: Clone + std::fmt::Display,
    {
        let eid = ui.id().with(id);
        let p = ui.ctx().pointer_latest_pos();

        // This was a bug: "being dragged" is false on the frame in which we release the button. This means that if we dragged the item onto itself, it would return a hover and prevent a move.
        // let drag = ui.ctx().is_being_dragged(id);

        match drag {
            // This item is being dragged. We never return an item response.
            Some(drag) if drag.id == id => {
                // Half of these cursors do not work in X11. See about using custom cursors in bevy and sharing that w/ bevy_egui. See also: https://github.com/mvlabat/bevy_egui/issues/229
                ui.output_mut(|o| o.cursor_icon = CursorIcon::Grab);

                // Draw the dragged item in a new area so it does not affect the size of the contents, which could occur with a large item rotated outside the bounds of the contents. We always draw the dragged item using the outer offset so that the pointer is never inside the area. That way we can reliably use egui's hit detection for widgets under the pointer.
                if let Some(p) = p {
                    egui::containers::Area::new(eid)
                        // TODO: The offset needs to account for the drag scaling.
                        .fixed_pos(p - drag.outer_offset)
                        // .order(egui::Order::Tooltip)
                        .interactable(false)
                        // TODO Restrict to ContainerSpace?
                        //.constrain(true) // this is wrong
                        .show(ui.ctx(), |ui| self.body(id, Some(drag), icon, slot_dim, ui));
                }

                None
            }
            // This item is not being dragged (but maybe something else is).
            _ => {
                let response = self.body(id, drag, icon, slot_dim, ui).response;

                // Figure out what slot we're in, see if it's filled, don't sense drag if not.
                p.filter(|_| response.contains_pointer())
                    .map(|p| p - response.rect.min)
                    .map(|offset| (self.slot(offset / slot_dim), offset))
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
                    .map(|(offset_slot, offset)| {
                        if drag.is_some() {
                            Some(ContentsResponse::NewTarget(id, slot))
                        } else {
                            ui.output_mut(|o| o.cursor_icon = CursorIcon::PointingHand);
                            let response = ui.interact(response.rect, eid, Sense::click_and_drag());

                            let response = response
                                .on_hover_text_at_pointer(self.hover_text(name, ui.style()));

                            if response.double_clicked() {
                                Some(ContentsResponse::Open(id))
                            } else if response.clicked()
                                && ui.input(|i| i.modifiers.contains(Modifiers::CTRL))
                            {
                                Some(ContentsResponse::SendItem(DragItem::new(id, self.clone())))
                            } else if response.drag_started() {
                                Some(ContentsResponse::NewDrag(DragItem {
                                    id,
                                    item: self.clone(),
                                    // Contents::body sets the source.
                                    source: None,
                                    target: None,
                                    offset,
                                    outer_offset: outer_offset(
                                        offset,
                                        response.rect.size(),
                                        OUTER_DISTANCE,
                                    ),
                                    offset_slot,
                                }))
                            } else {
                                None
                            }
                        }
                    })
                    .flatten()
            }
        }
    }

    fn hover_text(&self, name: &str, style: &Style) -> LayoutJob
    where
        T: std::fmt::Display,
    {
        let mut job = LayoutJob::default();
        RichText::new(name)
            .color(style.visuals.text_color())
            .append_to(&mut job, style, FontSelection::Default, Align::Center);

        RichText::new(format!("\n{}", self.flags))
            .small()
            .color(style.visuals.text_color())
            .append_to(&mut job, style, FontSelection::Default, Align::Center);
        job
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

// Finds the closest edge to the point and extends the point outside the edge by some distance.
// TODO This treats the item as a rectangle and does not take into account empty slots. See boomerang. This should probably extend a line from the center through the point, to a point outside the shape.
fn outer_offset(Vec2 { x, y }: Vec2, size: Vec2, d: f32) -> Vec2 {
    // left/right/top/bottom
    [
        // (distance to edge, new point)
        (x, (-d, y)),
        (size.x - x, (size.x + d, y)),
        (y, (x, -d)),
        (size.y - y, (x, size.y + d)),
    ]
    .iter()
    .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
    .map(|a| a.1.into())
    .unwrap()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
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

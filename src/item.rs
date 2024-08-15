use egui::TextureId;

use crate::*;
use bevy_ecs::prelude::*;

#[derive(Component, Clone, Debug)]
pub struct Item {
    pub rotation: ItemRotation,
    pub shape: shape::Shape,
    pub icon: TextureId,
    pub flags: ItemFlags,
}

#[derive(Debug)]
pub enum ItemResponse {
    // Rename DragToItem,
    SetTarget((usize, Entity)),
    NewDrag(DragItem),
}

impl Item {
    // Flags are required since the empty (default) flags allow the item to fit any container
    // regardless of the container's flags.
    pub fn new(flags: ItemFlags) -> Self {
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

    pub fn with_flags(mut self, flags: impl Into<ItemFlags>) -> Self {
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
    pub fn size(&self) -> egui::Vec2 {
        (self.shape.size.as_vec2() * SLOT_SIZE).as_ref().into()
    }

    /// The width of the shape (in slots).
    pub fn width(&self) -> usize {
        self.shape.width()
    }

    /// Return slot for offset in pixels.
    pub fn slot(&self, offset: egui::Vec2) -> usize {
        self.shape.slot(to_size(offset / SLOT_SIZE))
    }

    const PIVOT: egui::Vec2 = egui::Vec2::splat(0.5);

    pub fn body(
        &self,
        id: Entity,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> InnerResponse<egui::Vec2> {
        let eid = egui::Id::new(id);

        // let (dragging, item) = match drag_item.as_ref() {
        //     Some(drag) if drag.item.id == self.id => (true, &drag.item),
        //     _ => (false, self),
        // };
        let dragging = drag_item.as_ref().is_some_and(|d| d.id == id);

        // Allocate the original size so the contents draws
        // consistenly when the dragged item is scaled.
        let size = self.size();
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());

        // Scale down slightly even when not dragging in lieu of baking a border into every item
        // icon. TODO This needs to be configurable.
        let drag_scale = ui
            .ctx()
            // ui.id() is diff while dragging...
            .animate_bool(eid.with("scale"), dragging);

        if ui.is_rect_visible(rect) {
            // This size is a hint and isn't used since the image is always(?) already loaded.
            let image = egui::Image::new((self.icon, size));
            let image = if dragging {
                image.tint(egui::Rgba::from_rgba_premultiplied(1.0, 1.0, 1.0, 0.8))
            } else {
                image
            };

            // Scale down if dragging from center.
            let rect = egui::Rect::from_center_size(
                rect.center(),
                rect.size() * egui::lerp(0.98..=0.88, drag_scale),
            );

            // For non-square shapes, we need to un-rotate the paint_at rect. This seems like a bug
            // in egui...
            match self.rotation {
                ItemRotation::None => image.paint_at(ui, rect),
                r @ ItemRotation::R180 => image.rotate(r.angle(), Self::PIVOT).paint_at(ui, rect),
                r @ _ => image.rotate(r.angle(), Self::PIVOT).paint_at(
                    ui,
                    egui::Rect::from_center_size(rect.center(), rect.size().yx()),
                ),
            };
        }

        InnerResponse::new(size, response)
    }

    pub fn ui(
        &self,
        slot: usize,
        id: Entity,
        name: &str,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> Option<ItemResponse> {
        let eid = egui::Id::new(id);

        // This was a bug: "being dragged" is false on the frame in which we release the button.
        // This means that if we dragged the item onto itself, it would return a hover and prevent a
        // move.
        // let drag = ui.ctx().is_being_dragged(id);

        let drag = drag_item.as_ref().is_some_and(|d| d.id == id);

        if !drag {
            let response = self.body(id, drag_item, ui).response;

            // Figure out what slot we're in, see if it's filled, don't sense drag if not.
            let filled = ui
                .ctx()
                .pointer_interact_pos()
                .filter(|p| response.rect.contains(*p))
                .map(|p| {
                    let slot = self.slot(p - response.rect.min);
                    self.shape.fill.get(slot).map(|b| *b).unwrap_or_else(|| {
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
                .unwrap_or_default();

            if filled {
                let response = ui.interact(response.rect, eid, egui::Sense::drag());
                let response = response.on_hover_text_at_pointer(name);
                if response.drag_started() {
                    return Some(ItemResponse::NewDrag(DragItem {
                        id,
                        item: self.clone(),
                        // Contents::ui sets this.
                        source: None,
                    }));
                }
                if response.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                }
                drag_item
                    .as_ref()
                    .map(|_| ItemResponse::SetTarget((slot, id)))
            } else {
                None
            }
        } else {
            // Half of these cursors do not work in X11. See about using custom cursors in bevy and
            // sharing that w/ bevy_egui. See also: https://github.com/mvlabat/bevy_egui/issues/229
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);

            // pos - pos = vec
            // pos + pos = error
            // pos +/- vec = pos
            // vec +/- pos = error

            // Draw the dragged item in a new area so it does not
            // affect the size of the contents, which could occur with
            // a large item rotated outside the bounds of the contents.
            match ui.ctx().pointer_interact_pos() {
                Some(p) => {
                    // from egui::containers::show_tooltip_area_dyn
                    let _resp = egui::containers::Area::new(eid)
                        // .order(egui::Order::Tooltip)
                        // The cursor is placing the first slot (upper
                        // left) when dragging, so draw the dragged
                        // item in roughly the same place.
                        .fixed_pos(p - slot_size() * 0.2)
                        .interactable(false)
                        .movable(false)
                        // Restrict to ContainerSpace?
                        //.constrain(true) // FIX this is wrong
                        //.default_size(self.size_rotated())
                        .show(ui.ctx(), |ui| self.body(id, drag_item, ui));

                    // Still allocate the original size for expanding contents. The response size
                    // can be rotated (since it's being dragged), so use our (rotated) size.

                    // We no longer know the undragged item size, so this is broken. FIX:
                    // ui.allocate_exact_size(self.size_rotated(), egui::Sense::hover());

                    // This only works because we're not drawing the original item...
                    ui.allocate_exact_size(slot_size(), egui::Sense::hover());
                }
                _ => tracing::error!("no interact position for drag?"),
            }

            // make sure there is no existing drag_item or it matches
            // our id
            // assert!(
            //     drag_item.is_none() || drag_item.as_ref().map(|drag| drag.item.id) == Some(self.id)
            // );

            None
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
    pub const R0_UVS: [egui::Pos2; 4] = [
        egui::pos2(0.0, 0.0),
        egui::pos2(1.0, 0.0),
        egui::pos2(0.0, 1.0),
        egui::pos2(1.0, 1.0),
    ];

    pub const R90_UVS: [egui::Pos2; 4] = [
        egui::pos2(0.0, 1.0),
        egui::pos2(0.0, 0.0),
        egui::pos2(1.0, 1.0),
        egui::pos2(1.0, 0.0),
    ];

    pub const R180_UVS: [egui::Pos2; 4] = [
        egui::pos2(1.0, 1.0),
        egui::pos2(0.0, 1.0),
        egui::pos2(1.0, 0.0),
        egui::pos2(0.0, 0.0),
    ];

    pub const R270_UVS: [egui::Pos2; 4] = [
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

    pub fn rot2(&self) -> egui::emath::Rot2 {
        egui::emath::Rot2::from_angle(self.angle())
    }

    pub fn uvs(&self) -> &[egui::Pos2; 4] {
        match *self {
            ItemRotation::None => &Self::R0_UVS,
            ItemRotation::R90 => &Self::R90_UVS,
            ItemRotation::R180 => &Self::R180_UVS,
            ItemRotation::R270 => &Self::R270_UVS,
        }
    }
}

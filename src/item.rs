use egui::TextureId;

use crate::*;

#[derive(Clone, Debug)]
pub struct Item {
    pub id: usize,
    pub rotation: ItemRotation,
    pub shape: shape::Shape,
    pub icon: TextureId,
    pub flags: FlagSet<ItemFlags>,
    pub name: String, // WidgetText?
}

// pub fn item(
//     id: usize,
//     icon: TextureId,
//     shape: shape::Shape,
//     drag_item: &Option<DragItem>,
// ) -> impl egui::Widget + '_ {
//     // Widget will never work since we need to return things other
//     // than a response.
//     move |ui: &mut egui::Ui| ui.horizontal(|ui| Item::new(id, icon, shape).ui(drag_item, ui))
// }

#[derive(Debug)]
pub enum ItemResponse {
    Hover((usize, Item)),
    NewDrag(Item),
    Drag(DragItem),
}

impl Item {
    pub fn new(id: usize, icon: TextureId, shape: impl Into<Shape>) -> Self {
        Self {
            id,
            rotation: Default::default(),
            shape: shape.into(),
            icon,
            flags: FlagSet::default(),
            name: Default::default(),
        }
    }

    pub fn with_id(mut self, id: usize) -> Self {
        self.id = id;
        self
    }

    pub fn with_flags(mut self, flags: impl Into<FlagSet<ItemFlags>>) -> Self {
        self.flags = flags.into();
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn with_rotation(mut self, r: ItemRotation) -> Self {
        self.rotation = r;
        self
    }

    /// Returns an egui id based on the item id.
    pub fn eid(&self) -> egui::Id {
        egui::Id::new(self.id)
    }

    /// Size of the (unrotated?) item in pixels.
    // TODO check uses of this and make sure the rotation is right
    pub fn size(&self) -> egui::Vec2 {
        egui::Vec2::new(
            self.shape.width() as f32 * ITEM_SIZE,
            self.shape.height() as f32 * ITEM_SIZE,
        )
    }

    // DragItem is already rotated, so this should never be used in
    // that case. It would be nice to enforce this via type.
    pub fn size_rotated(&self) -> egui::Vec2 {
        match (self.size(), self.rotation) {
            (egui::Vec2 { x, y }, ItemRotation::R90 | ItemRotation::R270) => egui::vec2(y, x),
            (size, _) => size,
        }
    }

    // The width of the item with rotation.
    pub fn width(&self) -> usize {
        match self.rotation {
            ItemRotation::R90 | ItemRotation::R270 => self.shape.height(),
            _ => self.shape.width(),
        }
    }

    pub fn body(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> egui::Vec2 {
        // the demo adds a context menu here for removing items
        // check the response id is the item id?
        //ui.add(egui::Label::new(format!("item {}", self.id)).sense(egui::Sense::click()))

        let (dragging, rot) = match drag_item.as_ref() {
            Some(drag) if drag.item.id == self.id => (true, drag.item.rotation),
            _ => (false, self.rotation),
        };

        // let image = if dragging {
        //     image.tint(egui::Rgba::from_rgba_premultiplied(1.0, 1.0, 1.0, 0.5))
        // } else {
        //     image
        // };

        // Allocate the original size so the contents draws
        // consistenly when the dragged item is scaled.
        let size = rot.size(self.size());
        let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());

        // Image::rotate was problematic for non-square images. Rather
        // than rotate the mesh, reassign uvs.
        if ui.is_rect_visible(rect) {
            let mut mesh = egui::Mesh::with_texture(self.icon);
            // Scale down slightly when dragged to see the
            // background. Animate this and the drag-shift to cursor?
            let drag_scale = if dragging { 0.8 } else { 1.0 };

            mesh.add_rect_with_uv(
                egui::Rect::from_min_size(rect.min, size * drag_scale),
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            for (v, uv) in mesh.vertices.iter_mut().zip(rot.uvs().iter()) {
                v.uv = *uv;
            }

            ui.painter().add(egui::Shape::mesh(mesh));
        }

        size
    }

    pub fn ui(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> Option<ItemResponse> {
        let id = self.eid();
        let drag = ui.ctx().is_being_dragged(id);
        if !drag {
            // This does not work.
            // ui.push_id(self.id, |ui| self.body(drag_item, ui))
            //     .response
            //     .interact(egui::Sense::drag())
            //     .on_hover_cursor(egui::CursorIcon::Grab);

            let response = ui.scope(|ui| self.body(drag_item, ui)).response;

            // Figure out what slot we're in, see if it's filled,
            // don't sense drag if not.
            let filled = ui
                .ctx()
                .pointer_interact_pos()
                .filter(|p| response.rect.contains(*p))
                .map(|p| {
                    // This is roughly <GridContents as Contents>::slot?
                    let p = (p - response.rect.min) / ITEM_SIZE;
                    let slot = p.x as usize + p.y as usize * self.width();
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
                let response = ui.interact(response.rect, id, egui::Sense::drag());
                let response = response.on_hover_text_at_pointer(format!("{}", self));
                if response.hovered() {
                    ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                }
                drag_item
                    .as_ref()
                    .map(|_| ItemResponse::Hover((0, self.clone())))
            } else {
                None
            }
        } else {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);

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
                    let _resp = egui::containers::Area::new(id)
                        .order(egui::Order::Tooltip)
                        // The cursor is placing the first slot (upper
                        // left) when dragging, so draw the dragged
                        // item in roughly the same place.
                        .fixed_pos(p - item_size() * 0.25)
                        .interactable(false)
                        // Restrict to ContainerShape?
                        .constrain_to(egui::Rect::EVERYTHING)
                        .show(ui.ctx(), |ui| self.body(drag_item, ui));

                    // Still allocate the original size for expanding
                    // contents. The response size can be rotated
                    // (since it's being dragged), so use our
                    // (rotated) size.
                    ui.allocate_exact_size(self.rotation.size(self.size()), egui::Sense::hover());
                }
                _ => tracing::error!("no interact position for drag?"),
            }

            // make sure there is no existing drag_item or it matches
            // our id
            assert!(
                drag_item.is_none() || drag_item.as_ref().map(|drag| drag.item.id) == Some(self.id)
            );

            // Only send back a clone if this is a new drag (drag_item
            // is empty):
            drag_item
                .is_none()
                // This clones the shape twice...
                .then(|| ItemResponse::NewDrag(self.clone().rotate()))
        }
    }

    // This returns a clone every time, even if not rotated.
    fn shape(&self) -> shape::Shape {
        match self.rotation {
            ItemRotation::None => self.shape.clone(),
            ItemRotation::R90 => self.shape.rotate90(),
            ItemRotation::R180 => self.shape.rotate180(),
            ItemRotation::R270 => self.shape.rotate270(),
        }
    }

    // Rotate the (dragged) shape to match the item's rotation.
    fn rotate(mut self) -> Self {
        self.shape = self.shape();
        self
    }
}

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)?;
        f.write_str(" ")?;
        f.debug_list().entries(self.flags.into_iter()).finish()
    }
}

#[derive(Clone, Copy, Debug, Default)]
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

    // Move this into item? TODO
    pub fn size(&self, size: egui::Vec2) -> egui::Vec2 {
        match *self {
            ItemRotation::R90 | ItemRotation::R270 => egui::Vec2::new(size.y, size.x),
            _ => size,
        }
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

// This might be useful to get generic over item iterators, but it
// would require adding another parameter to `Contents::ui`.
pub trait SlotItem {
    fn slot_item(&self) -> (usize, &Item);
}

impl SlotItem for (usize, Item) {
    fn slot_item(&self) -> (usize, &Item) {
        (self.0, &self.1)
    }
}

impl SlotItem for (usize, &Item) {
    fn slot_item(&self) -> (usize, &Item) {
        *self
    }
}

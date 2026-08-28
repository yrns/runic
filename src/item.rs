use bevy_ecs::prelude::*;
use bevy_egui::egui::{
    self, text::LayoutJob, Align, CursorIcon, FontSelection, Id, InnerResponse, Modifiers, Pos2,
    Rect, Rgba, RichText, Sense, Style, TextureId, Ui, Vec2,
};
use bevy_math::Rot2;
use bevy_reflect::prelude::*;
use bevy_ui::{widget::*, *};

use crate::*;

/// An item.
#[derive(Component, Clone, Debug, Reflect, FromTemplate)]
#[reflect(Component)]
#[require(ItemRotation)]
pub struct Item {
    /// The shape represents this items dimensions (and filled "slots" in case it is not rectangular).
    pub shape: Shape,
}

/// Update shape and transform when the item is rotated.
pub fn insert_rotation(
    mut commands: Commands,
    mut items: Query<(Entity, &mut Item, &ItemRotation), Changed<ItemRotation>>,
) {
    for (id, _item, rotation) in &mut items {
        // transform.rotation = rotation.rot2();
        // commands
        //     .entity(id)
        //     .insert(UiTransform::from_rotation(rotation.rot2()));
        // We no longer mutate the item shape.
        // item.rotate(*rotation);
    }
}

// The slot can change (move inside a container).
// The slot and parent can change.
// The rotation can change.
// Or all three!
// So we can't easily use component changes, and rather use item events.

pub fn update_node(
    Slot(slot): Slot,
    item: &Item,
    rotation: ItemRotation,
    node: &mut Node,
    transform: &mut UiTransform,
) {
    use bevy_math::Vec2Swizzles;

    let size = item.shape.size;
    // let size = match rotation {
    //     ItemRotation::R90 | ItemRotation::R270 => size.yx(),
    //     _ => size,
    // };
    node.grid_row = GridPlacement::start_span((slot.y + 1) as i16, size.y as u16);
    node.grid_column = GridPlacement::start_span((slot.x + 1) as i16, size.x as u16);
    // Grid tracks are fixed.

    let size = size.as_vec2() * 48.0;

    // The icons don't stretch in Bevy by default. Specifying the fixed grid tracks isn't enough to get the border at the right size, the cells weirdly take up more room when the neighboring cells aren't filled...
    node.width = px(size.x);
    node.height = px(size.y);
    // node.max_width = px(size.x as f32 * 48.0);
    // node.max_height = px(size.y as f32 * 48.0);
    // node.min_width = node.max_width;
    // node.min_height = node.max_height;

    // Bevy rotates from the center. We're not just rotating it, we're trying to maintain the item's upper left corner in the current slot. So non-square items will need to be offset.
    transform.rotation = rotation.rot2();
    transform.translation = match rotation {
        ItemRotation::R90 | ItemRotation::R270 => {
            let offset = (size.yx() - size) * 0.5;
            Val2::px(offset.x, offset.y)
        }
        // Center pivot is fine. Clear translation.
        _ => Val2::default(),
    };
}

// TODO: SystemParam?
pub fn on_item_insert(
    event: On<ItemInsert>,
    mut items: Query<(&Slot, &Item, &ItemRotation, &mut Node, &mut UiTransform)>,
) -> Result {
    let (slot, item, rotation, mut node, mut transform) = items.get_mut(event.item)?;
    update_node(*slot, item, *rotation, &mut *node, &mut *transform);
    Ok(())
}

pub fn on_item_move(
    event: On<ItemMove>,
    mut items: Query<(&Slot, &Item, &ItemRotation, &mut Node, &mut UiTransform)>,
) -> Result {
    let (slot, item, rotation, mut node, mut transform) = items.get_mut(event.item)?;
    update_node(*slot, item, *rotation, &mut *node, &mut *transform);
    Ok(())
}

// If it's being dragged, it's not on the grid...
pub fn on_item_rotate(_event: On<ItemDragRotate>) {}

pub fn insert_nodes(
    mut commands: Commands,
    icons: Query<(Entity, &Icon), Changed<Icon>>,
    mut items: Query<
        (
            Entity,
            &Slot,
            &Item,
            &ItemRotation,
            &Icon,
            Option<&mut Node>,
        ),
        Or<(Changed<Slot>, Changed<Item>)>,
    >,
) {
    // for (id, icon) in &icons {
    //     commands.entity(id).insert(ImageNode::new(icon.0.clone()));
    // }

    for (id, slot, item, rotation, icon, node) in &mut items {
        match node {
            Some(_node) => {
                // transform.rotation = todo!();
            }
            _ => {
                let mut node = Node {
                    // TEMP styling?
                    // We don't want to border around the items because of irregular shapes. But it's useful for debugging.
                    border: px(1.).all(),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    // overflow: Overflow::clip(),
                    ..Default::default()
                };
                let mut transform = UiTransform::IDENTITY;
                update_node(*slot, item, *rotation, &mut node, &mut transform);
                commands.entity(id).insert((
                    node,
                    transform,
                    ImageNode::new(icon.0.clone()).with_mode(NodeImageMode::Auto),
                ));
                // .with_children(|p| {
                //     p.spawn((
                //         Node {
                //             width: Val::Percent(100.0),
                //             height: Val::Percent(100.0),
                //             ..Default::default()
                //         },
                //         // ImageNode::new(icon.0.clone()).with_mode(NodeImageMode::Stretch),
                //         // UiTransform::from_rotation(rotation.rot2()),
                //     ));
                // });
            }
        }
    }
}

impl Item {
    // Flags are required since the empty (default) flags allow the item to fit any container
    // regardless of the container's flags.
    pub fn new() -> Self {
        Self {
            shape: Shape::new([1, 1], true),
        }
    }

    // /// Set the item shape and unset its rotation.
    // pub fn with_shape(mut self, shape: impl Into<Shape>) -> Self {
    //     self.shape = shape.into();
    //     self.rotation = ItemRotation::None;
    //     self
    // }

    // /// Set the item's rotation and apply it to its shape.
    // pub fn with_rotation(mut self, r: ItemRotation) -> Self {
    //     self.rotation = r;
    //     self.rotate();
    //     self
    // }

    /// Size in pixels.
    pub fn size(&self, slot_dim: f32) -> Vec2 {
        (self.shape.size().as_vec2() * slot_dim).as_ref().into()
    }

    /// The width of the shape (in slots).
    pub fn width(&self) -> usize {
        self.shape.width()
    }

    /// Return index for offset in pixels.
    pub fn index(&self, offset: Vec2) -> usize {
        self.shape.index(to_size(offset))
    }

    const PIVOT: Vec2 = Vec2::splat(0.5);

    /// Show item body (icon, etc.).
    pub fn body(
        &self,
        _id: Entity,
        drag_scale: f32,
        icon: TextureId,
        // The item's shape is already rotated. This is for the icon.
        rotation: ItemRotation,
        slot_dim: f32,
        ui: &mut Ui,
    ) -> InnerResponse<Vec2> {
        // Allocate the original size so the contents draws consistenly when the dragged item is scaled.
        let size = self.size(slot_dim);
        let (rect, response) = ui.allocate_exact_size(size, Sense::hover());

        if ui.is_rect_visible(rect) {
            // This size is a hint and isn't used since the image is always(?) already loaded.
            let image = egui::Image::new((icon, size));
            let image = image.tint(Rgba::from_rgba_premultiplied(
                1.0,
                1.0,
                1.0,
                egui::lerp(1.0..=0.8, drag_scale),
            ));

            // Scale down if dragging from center. The offset is not scaled, so for a really large item, the distance from the item to the pointer could be relatively large, which might look bad.
            let rect = Rect::from_center_size(
                rect.center(),
                rect.size() * egui::lerp(1.0..=0.88, drag_scale),
            );

            // For non-square shapes, we need to un-rotate the paint_at rect. This seems like a bug in egui...
            match rotation {
                ItemRotation::None => image.paint_at(ui, rect),
                r @ ItemRotation::R180 => image.rotate(r.angle(), Self::PIVOT).paint_at(ui, rect),
                r => image
                    .rotate(r.angle(), Self::PIVOT)
                    .paint_at(ui, Rect::from_center_size(rect.center(), rect.size().yx())),
            };
        }

        InnerResponse::new(size, response)
    }

    /// Show item. `slot` is the slot we occupy in the container.
    #[allow(clippy::too_many_arguments)]
    pub fn ui<T: Copy>(
        &self,
        slot: Slot,
        id: Entity,
        flags: &Flags<T>,
        name: &str,
        drag: Option<&DragItem<T>>,
        icon: TextureId,
        rotation: ItemRotation,
        slot_dim: f32,
        ui: &mut Ui,
    ) -> Option<ContentsResponse<T>> {
        let eid = Id::new(id);
        let p = ui.ctx().pointer_latest_pos();

        // This was a bug: "being dragged" is false on the frame in which we release the button. This means that if we dragged the item onto itself, it would return a hover and prevent a move.
        // let drag = ui.ctx().is_being_dragged(id);

        // Scale down slightly while dragging so more of the shadow is visible. It also offsets the offset.
        let drag_id = drag.filter(|d| d.id == id);
        let drag_scale = ui.ctx().animate_bool(eid.with("scale"), drag_id.is_some());

        match drag_id {
            // This item is being dragged. We never return an item response.
            Some(drag) => {
                // Half of these cursors do not work in X11. See about using custom cursors in bevy and sharing that w/ bevy_egui. See also: https://github.com/mvlabat/bevy_egui/issues/229
                ui.output_mut(|o| o.cursor_icon = CursorIcon::Grab);

                // Draw the dragged item in a new area so it does not affect the size of the contents, which could occur with a large item rotated outside the bounds of the contents. We always draw the dragged item using the outer offset so that the pointer is never inside the area. That way we can reliably use egui's hit detection for widgets under the pointer.
                if let Some(p) = p {
                    egui::containers::Area::new(eid)
                        // Animate from the origin to the offset position.
                        .fixed_pos(drag.origin.lerp(p - drag.outer_offset, drag_scale))
                        // .order(egui::Order::Tooltip)
                        .interactable(false)
                        // TODO Restrict to ContainerSpace?
                        //.constrain(true) // this is wrong
                        .show(ui.ctx(), |ui| {
                            // We already have the drag rotation?
                            self.body(id, drag_scale, icon, drag.rotation, slot_dim, ui)
                        });
                }

                None
            }
            // This item is not being dragged (but maybe something else is).
            _ => {
                let response = self
                    .body(id, drag_scale, icon, rotation, slot_dim, ui)
                    .response;

                // Figure out what slot we're in, see if it's filled, don't sense drag if not.
                p.filter(|_| response.contains_pointer())
                    .map(|p| p - response.rect.min)
                    .map(|offset| (self.index(offset / slot_dim), offset))
                    .filter(|(index, _)| {
                        self.shape.fill.get(*index).copied().unwrap_or_else(|| {
                            // This occurs somewhere on drag/mouseover. Not anymore?
                            tracing::error!(
                                "point {:?} index {} out of shape fill {}",
                                p,
                                index,
                                self.shape
                            );
                            false
                        })
                    })
                    .and_then(|(offset_slot, offset)| {
                        // Dragging a different item? Drag to item.
                        if drag.is_some() {
                            // Why is this being spammed? Why is this not calculated in a mouse move event?

                            // This slot is the slot the target item is in in its container, which gets rewritten outside in contents.
                            Some(ContentsResponse::NewTarget((id, slot, ui.id())))
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
                                Some(ContentsResponse::SendItem(DragItem::new(
                                    id,
                                    self.clone(),
                                    rotation,
                                    flags.clone(),
                                )))
                            } else if response.drag_started() {
                                // Contents::body sets the source.
                                Some(ContentsResponse::NewDrag(DragItem {
                                    offset,
                                    outer_offset: outer_offset(
                                        offset,
                                        response.rect.size(),
                                        OUTER_DISTANCE,
                                    ),
                                    origin: response.rect.min,
                                    offset_slot,

                                    ..DragItem::new(id, self.clone(), rotation, *flags)
                                }))
                            } else {
                                None
                            }
                        }
                    })
            }
        }
    }

    fn hover_text(&self, name: &str, style: &Style) -> LayoutJob {
        let mut job = LayoutJob::default();
        RichText::new(name)
            .color(style.visuals.text_color())
            .append_to(&mut job, style, FontSelection::Default, Align::Center);

        // FIX post-egui
        // RichText::new(format!("\n{}", self.flags))
        //     .small()
        //     .color(style.visuals.text_color())
        //     .append_to(&mut job, style, FontSelection::Default, Align::Center);

        job
    }

    // Apply rotation to shape. This should really only be used for temporary items. The actual item entities are stored unrotated and the rotation is applied when we need to check to see if it'll fit somewhere, or other operations where the rotation is pertinent.
    pub fn with_rotation(mut self, rotation: ItemRotation) -> Self {
        match rotation {
            ItemRotation::None => (),
            ItemRotation::R90 => self.shape = self.shape.rotate90(),
            ItemRotation::R180 => self.shape = self.shape.rotate180(),
            ItemRotation::R270 => self.shape = self.shape.rotate270(),
        }
        self
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

// TODO: rename?
/// Clockwise rotation.
#[derive(Component, Copy, Clone, Debug, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
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

    /// Returns a `Rot2` for the current rotation. `ItemRotation` is clockwise relative to the screen and user (+Y is down), so the rotation this returns is inverted.
    /// ```
    /// # use runic::ItemRotation;
    /// # use bevy_math::Rot2;
    /// assert_eq!(ItemRotation::R90.rot2().as_degrees(), 90.0);
    /// assert_eq!(ItemRotation::R180.rot2().as_degrees(), 180.0);
    /// assert_eq!(ItemRotation::R270.rot2().as_degrees(), -90.0);
    /// ```
    pub const fn rot2(&self) -> Rot2 {
        match self {
            Self::None => Rot2::IDENTITY,
            Self::R90 => Rot2::FRAC_PI_2,
            Self::R180 => Rot2::PI,
            Self::R270 => Rot2::FRAC_PI_2.inverse(),
        }
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

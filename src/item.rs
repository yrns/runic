use bevy_ecs::{prelude::*, query::Spawned};
use bevy_input::{keyboard::KeyCode, *};
use bevy_math::*;
use bevy_picking::prelude::*;
use bevy_reflect::prelude::*;
use bevy_ui::{widget::*, *};
use tracing::*;

use crate::*;

/// An item.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
#[require(ItemRotation)]
pub struct Item {
    /// The shape represents this items dimensions (and filled "slots" in case it is not rectangular).
    pub shape: Shape,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            shape: Shape::new([1, 1], true),
        }
    }
}

impl Item {
    /// Apply `rotation` to shape.
    // This should really only be used for temporary items. The actual item entities are stored unrotated and the rotation is applied when we need to check to see if it'll fit somewhere, or other operations where the rotation is pertinent.
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

/// Update the dragged item's transform when rotated.
pub fn update_drag_rotation(
    // mut commands: Commands,
    mut items: Query<(&Item, &DragRotation, &mut UiTransform), Changed<DragRotation>>,
) {
    for (Item { shape }, DragRotation(drag_rotation, offset), mut transform) in &mut items {
        transform.rotation = drag_rotation.rot2();
        let Vec2 { x, y } = drag_rotation.offset(shape.size.as_vec2() * 48.0) + offset;
        transform.translation = Val2::px(x, y);
    }
}

// The slot can change (move inside a container).
// The slot and parent can change.
// The rotation can change.
// Or all three!
// So we can't easily use component changes, and rather use item events.

/// Update the node and transform when an item's size changes (via rotation) or its slot changes.
pub fn update_nodes(
    mut items: Query<
        (
            NameOrEntity,
            &Item,
            &ItemRotation,
            &Slot,
            &mut Node,
            &mut UiTransform,
        ),
        Or<(Changed<ItemRotation>, Changed<Slot>)>, // ChildOf?
    >,
) {
    for (_, Item { shape }, rotation, Slot(slot), mut node, mut transform) in &mut items {
        let size = shape.size;

        // We don't need to apply the rotation to the size because the transform does.
        // let size = match rotation {
        //     ItemRotation::R90 | ItemRotation::R270 => size.yx(),
        //     _ => size,
        // };

        node.grid_row = GridPlacement::start_span((slot.y + 1) as i16, size.y as u16);
        node.grid_column = GridPlacement::start_span((slot.x + 1) as i16, size.x as u16);

        // Grid tracks are fixed, is this needed?
        let size = size.as_vec2() * 48.0;

        // The icons don't stretch in Bevy by default. Specifying the fixed grid tracks isn't enough to get the border at the right size, the cells weirdly take up more room when the neighboring cells aren't filled...
        node.width = px(size.x);
        node.height = px(size.y);
        // node.max_width = px(size.x as f32 * 48.0);
        // node.max_height = px(size.y as f32 * 48.0);
        // node.min_width = node.max_width;
        // node.min_height = node.max_height;

        // Update transform. What about scale?
        transform.rotation = rotation.rot2();
        let Vec2 { x, y } = rotation.offset(size);
        transform.translation = Val2::px(x, y);
    }
}

// If it's being dragged, it's not on the grid...
pub fn on_item_rotate(_event: On<ItemDragRotate>) {}

pub fn on_item_drag_start(
    event: On<Pointer<DragStart>>,
    mut commands: Commands,
    contents: Query<&GridContents>,
    items: Query<(&Item, &ItemRotation, &Slot, &ChildOf)>,
) {
    let id = event.event_target();
    if let Ok((item, rotation, Slot(slot), ChildOf(container))) = items.get(id) {
        if let Ok(contents) = contents.get(*container) {
            let item = item.clone().with_rotation(*rotation);
            let mut shape = contents.shape.clone();
            shape.unpaint(&item.shape, shape.index(*slot));
            commands.entity(*container).insert(DragShape(shape));
            commands.entity(id).insert((
                GlobalZIndex(1),
                Pickable::IGNORE,
                DragRotation(*rotation, Vec2::ZERO),
            ));
        }
    }
}

pub fn on_item_drag(
    event: On<Pointer<Drag>>,
    mut items: Query<(&Item, &mut DragRotation, &mut UiTransform)>,
) {
    if let Ok((item, mut drag, mut transform)) = items.get_mut(event.event_target()) {
        drag.1 = event.distance;
        transform.rotation = drag.0.rot2();
        let d = drag.1 + drag.0.offset(item.shape.size.as_vec2() * 48.0);
        transform.translation = Val2::px(d.x, d.y);
    }
}

fn pointer_slot(
    position: Vec2,
    section: &GridContents,
    transform: &UiGlobalTransform,
    node: &ComputedNode,
) -> UVec2 {
    let p = transform.affine().inverse().transform_point2(position) / node.size + Vec2::splat(0.5);
    (section.shape.size().as_vec2() * p).as_uvec2()
}

pub fn on_item_drag_enter<T: Accepts>(
    mut event: On<Pointer<DragEnter>>,
    mut commands: Commands,
    items: Query<(NameOrEntity, &Flags<T>), With<Item>>,
    sections: Query<(NameOrEntity, &Flags<T>), With<GridContents>>,
) {
    if let Ok((item, item_flags)) = items.get(event.dragged) {
        if let Ok((target, ..)) = items.get(event.event_target()) {
            info!("drag enter item: {item} -> {target}");
            // TODO hit check
            // All items overlap the section that they're in. And we don't want to be inserting a drag slot in the parent section.
            event.propagate(false);
        } else if let Ok((target, section_flags)) = sections.get(event.event_target()) {
            if section_flags.accepts(item_flags) {
                commands.entity(target.entity).insert(DragSlot(None));
                event.propagate(false);
                info!("drag enter: {item} -> {target}");
            }
        }
    }
}

/// Sets the `DragSlot` for the currently hovered section.
pub fn on_item_drag_over<T>(
    event: On<Pointer<DragOver>>,
    items: Query<(NameOrEntity, &Item, &ItemRotation, Option<&DragRotation>)>,
    mut sections: Query<(
        NameOrEntity,
        &GridContents,
        Option<&DragShape>,
        &UiGlobalTransform,
        &ComputedNode,
        &mut DragSlot,
    )>,
) {
    // Fetch the item we are dragging.
    if let Ok((item_id, item, rotation, drag_rotation)) = items.get(event.dragged) {
        // Fetch the section we are hovering.
        if let Ok((id, section, drag_shape, transform, node, mut drag_slot)) =
            sections.get_mut(event.event_target())
        {
            // Apply (drag) rotation.
            let item = item
                .clone()
                .with_rotation(drag_rotation.map_or(*rotation, |r| r.0));

            // Use the cached shape with the item unpainted when moving within the same container.
            let section_shape = drag_shape.map_or(&section.shape, |DragShape(s)| &s);
            let slot = pointer_slot(event.pointer_location.position, section, transform, node);
            let slot = DragSlot(
                section_shape
                    .fits(&item.shape, section_shape.index(slot))
                    .then(|| Slot(slot)),
            );
            if drag_slot.replace_if_neq(slot).is_some() {
                info!("drag over: {item_id} -> {id} slot: {slot}");
            } //  else {
              //     warn!("does not fit: {slot}\n{}", &drag_shape.unwrap().0);
              // }
        }
    }
}

/// If dropped on an item, we attempt to find a section and slot for the item. If dropped on a suitable section slot we move it there.
pub fn on_item_drag_drop<T: Accepts>(
    event: On<Pointer<DragDrop>>,
    mut items: Items<T>,
    drag_slot: Query<(Entity, &DragSlot)>,
    mut contents: ContentsStorage<T>,
) {
    // We only care about the original target? What if someone spawns something (text/icon?) inside the item? Then they'd have to be unpickable.
    let t = event.original_event_target();
    if t != event.event_target() {
        return;
    }

    // Fetch the dragged item.
    if let Ok((id, slot, item, item_rotation, drag_rotation, child_of, flags)) =
        items.get_mut(event.dropped)
    {
        // We need to check if this is an item we're dropping onto or contents.
        if let Some(target) = if let Ok((id, DragSlot(slot))) = drag_slot.get(t) {
            // If the slot is None the item won't fit.
            slot.map(|slot| (id, slot))
        } else {
            // Item. Find a target.
            contents.find_section_slot(event.event_target(), item, flags)
        } {
            contents.resolve_drag(
                target,
                id,
                slot,
                item,
                item_rotation,
                drag_rotation,
                child_of,
            );
        }
    }
}

// TODO: move key input to example only?
/// Send item to target container.
pub fn on_item_ctrl_click(
    event: On<Pointer<Click>>,
    input: Res<ButtonInput<KeyCode>>,
    items: Query<(&Name, &Item, &Slot, &ItemRotation, &Children, &ChildOf)>,
    // mut contents: Query<(&mut GridContents)>,
) {
    match event.button {
        PointerButton::Primary => {
            if input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
                if let Ok((name, ..)) = items.get(event.event_target()) {
                    dbg!(name);
                }
            }
        }
        _ => (),
    }
}

pub fn on_item_drag_end(
    event: On<Pointer<DragEnd>>,
    mut commands: Commands,
    items: Query<NameOrEntity, With<Item>>,
) {
    if let Ok(item) = items.get(event.event_target()) {
        info!("drag end: {item}");
        commands
            .entity(item.entity)
            .insert((GlobalZIndex::default(), Pickable::default()))
            .remove::<DragRotation>();
    }
}

pub fn on_item_drag_leave(
    event: On<Pointer<DragLeave>>,
    mut commands: Commands,
    // items: Query<NameOrEntity, With<Item>>,
    sections: Query<NameOrEntity, With<DragSlot>>,
) {
    // if let Ok(id) = items.get(event.event_target()) {
    //     info!("drag leave item: {id}");
    //     // event.propagate(false);
    // } else
    if let Ok(id) = sections.get(event.event_target()) {
        info!("drag leave: {id}");
        commands.entity(id.entity).remove::<(DragShape, DragSlot)>();
        // event.propagate(false);
    }
}

// How do we trigger this?
pub fn on_item_drag_cancel(event: On<Pointer<Cancel>>) {
    warn!("cancel! {}", event.event_target());
}

pub fn insert_nodes(
    mut commands: Commands,
    mut items: Query<(Entity, &Icon, Option<&mut Node>), Spawned>,
) {
    for (id, icon, node) in &mut items {
        match node {
            Some(mut node) => {
                node.border = px(1.).all(); // TEMP
                node.align_items = AlignItems::Center;
                node.justify_content = JustifyContent::Center;
            }
            _ => {
                commands.entity(id).insert(Node {
                    // TEMP styling?
                    // We don't want to border around the items because of irregular shapes. But it's useful for debugging.
                    border: px(1.).all(),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                });
            }
        }

        commands.entity(id).insert((
            ImageNode::new(icon.0.clone()).with_mode(NodeImageMode::Auto),
            // This is also default behavior and is only needed when dragging?
            Pickable::default(),
            // Remove? This is only needed when dragging?
            GlobalZIndex::default(),
        ));
    }
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

    /// Bevy rotates from the center. We're not just rotating the item, we're trying to maintain its upper left corner in the current slot. So non-square items will need to be offset. This function returns that offset.
    fn offset(&self, size: Vec2) -> Vec2 {
        use bevy_math::Vec2Swizzles;

        match self {
            ItemRotation::R90 | ItemRotation::R270 => (size.yx() - size) * 0.5,
            // Center pivot is fine.
            _ => Vec2::ZERO,
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
// }

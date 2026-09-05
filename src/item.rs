use bevy_ecs::prelude::*;
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

pub fn on_item_drag_start(
    event: On<Pointer<DragStart>>,
    mut commands: Commands,
    contents: Query<&GridContents>,
    items: Query<(&Item, &Slot, &ChildOf)>,
) {
    let id = event.event_target();
    if let Ok((item, Slot(slot), ChildOf(container))) = items.get(id) {
        if let Ok(contents) = contents.get(*container) {
            let mut shape = contents.shape.clone();
            shape.unpaint(&item.shape, shape.index(*slot));
            commands.entity(*container).insert(DragShape(shape));
            commands
                .entity(id)
                .insert((GlobalZIndex(1), Pickable::IGNORE));
        }
    }
}

pub fn on_item_drag(event: On<Pointer<Drag>>, mut items: Query<&mut UiTransform, With<Item>>) {
    if let Ok(mut transform) = items.get_mut(event.event_target()) {
        // We can't use distance because the item may already have a translation (from rotation)
        let bevy_math::Vec2 { x: dx, y: dy } = event.delta;

        match &mut transform.translation {
            Val2 {
                x: Val::Px(x),
                y: Val::Px(y),
            } => {
                *x += dx;
                *y += dy;
            }
            _ => (),
        }
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
    items: Query<(NameOrEntity, &Item, &Flags<T>)>,
    sections: Query<(
        NameOrEntity,
        &GridContents,
        &Flags<T>,
        &UiGlobalTransform,
        &ComputedNode,
    )>,
) {
    if let Ok((item, _item, item_flags)) = items.get(event.dragged) {
        if let Ok((target, _, _)) = items.get(event.event_target()) {
            info!("drag enter item: {item} -> {target}");
            // All items overlap the section that they're in. And we don't want to be inserting a drag slot in the parent section.
            event.propagate(false);
        } else if let Ok((target, section, section_flags, transform, node)) =
            sections.get(event.event_target())
        {
            if section_flags.accepts(item_flags) {
                let slot = pointer_slot(event.pointer_location.position, section, transform, node);
                info!("drag enter: {item} -> {target}");
                commands.entity(target.entity).insert(DragSlot(Slot(slot)));
                event.propagate(false);
            }
        }
    }
}

// We need to determine the slot when dragging over contents.
pub fn on_item_drag_over<T>(
    event: On<Pointer<DragOver>>,
    mut sections: Query<(
        NameOrEntity,
        &GridContents,
        &UiGlobalTransform,
        &ComputedNode,
        &mut DragSlot,
    )>,
) {
    if let Ok((id, section, transform, node, mut drag_slot)) =
        sections.get_mut(event.event_target())
    {
        let slot = pointer_slot(event.pointer_location.position, section, transform, node);
        if drag_slot.replace_if_neq(DragSlot(Slot(slot))).is_some() {
            info!("drag over: {id} slot: {slot}");
        }
    }
}

pub fn on_item_drag_drop<T: Accepts>(
    mut event: On<Pointer<DragDrop>>,
    mut items: Items<T>,
    drag_slot: Query<(Entity, &DragSlot)>,
    mut contents: ContentsStorage<T>,
) {
    // Fetch the dragged item.
    if let Ok((id, slot, item, item_rotation, drag_rotation, child_of, flags)) =
        items.get_mut(event.dropped)
    {
        // We need to check if this is an item we're dropping onto or contents.
        if let Some(target) = drag_slot
            .get(event.event_target())
            .ok()
            .map(|(target, DragSlot(slot))| (target, *slot))
            .or_else(|| contents.find_section_slot(event.event_target(), item, flags))
        {
            contents.resolve_drag(
                target,
                id,
                slot,
                item,
                item_rotation,
                drag_rotation,
                child_of,
            );
            event.propagate(false);
        }
    }
}

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
    items: Query<(NameOrEntity, &Item)>,
) {
    if let Ok((item, _item)) = items.get(event.event_target()) {
        commands
            .entity(item.entity)
            .insert((GlobalZIndex::default(), Pickable::default()));
        info!("drag end: {item}");
    }
}

pub fn on_item_drag_leave(
    mut event: On<Pointer<DragLeave>>,
    mut commands: Commands,
    items: Query<NameOrEntity, With<Item>>,
    sections: Query<NameOrEntity, With<DragSlot>>,
) {
    if let Ok(id) = items.get(event.event_target()) {
        info!("drag leave item: {id}");
        event.propagate(false);
    } else if let Ok(id) = sections.get(event.event_target()) {
        info!("drag leave: {id}");
        commands.entity(id.entity).remove::<DragSlot>();
        event.propagate(false);
    }
}

// How do we trigger this?
pub fn on_item_drag_cancel(event: On<Pointer<Cancel>>) {
    warn!("cancel! {}", event.event_target());
}

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
                    Pickable::default(),
                    GlobalZIndex::default(),
                ))
                // .observe(|event: On<Pointer<DragStart>>| {
                //     dbg!("drag start", event);
                // })
;
            }
        }
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
}

// #[cfg(test)]
// mod tests {
//     use super::*;
// }

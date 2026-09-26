use crate::Slot;
use bevy_ecs::prelude::{Entity, EntityEvent};

// TODO: consider adding root container and section information

/// Item `item` inserted into target container at `slot`.
#[derive(EntityEvent, Debug)]
pub struct ItemInsert {
    pub entity: Entity,
    pub slot: Slot,
    pub item: Entity,
    // pub container: Entity,
}

/// Item `item` removed from target container at `slot`.
#[derive(EntityEvent, Debug)]
pub struct ItemRemove {
    pub entity: Entity,
    pub slot: Slot,
    pub item: Entity,
    // pub container: Entity,
}

/// Item `item` moved within target container from `old_slot` to `new_slot`.
#[derive(EntityEvent, Debug)]
pub struct ItemMove {
    pub entity: Entity,
    pub old_slot: Slot,
    pub new_slot: Slot,
    pub item: Entity,
    // pub container: Entity,
}

/// Item started dragging from `section` at `slot`.
#[derive(EntityEvent, Debug)]
pub struct ItemDragStart {
    pub entity: Entity,
    pub section: Entity,
    pub slot: Slot,
    // pub container: Entity,
}

/// Item drag ended at target section and slot, if any.
#[derive(EntityEvent, Debug)]
pub struct ItemDragEnd {
    pub entity: Entity,
    pub target: Option<(Entity, Slot)>,
    // pub container: Entity,
}

/// Item was dragged over `section` at `slot`.
// We removed the drag to item variant here.
#[derive(EntityEvent, Debug)]
pub struct ItemDragOver {
    /// Dragged item.
    pub entity: Entity,
    /// Target section.
    pub section: Entity,
    /// Target slot.
    pub slot: Slot,
    // pub container: Entity,
}

/// Dragged item was rotated.
#[derive(EntityEvent, Debug)]
pub struct ItemDragRotate(pub Entity);

/// This container was just opened.
#[derive(EntityEvent, Debug)]
pub struct ContainerOpened(pub Entity);

/// This container was just closed.
#[derive(EntityEvent, Debug)]
pub struct ContainerClosed(pub Entity);

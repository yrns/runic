use bevy_ecs::prelude::{Entity, Event};

// TODO: consider adding root container and section information

/// Item `item` inserted into target container at `slot`.
#[derive(Event, Debug)]
pub struct ItemInsert {
    pub slot: usize,
    pub item: Entity,
    // pub container: Entity,
}

/// Item `item` removed from target container at `slot`.
#[derive(Event, Debug)]
pub struct ItemRemove {
    pub slot: usize,
    pub item: Entity,
    // pub container: Entity,
}

/// Item `item` moved within target container from `old_slot` to `new_slot`.
#[derive(Event, Debug)]
pub struct ItemMove {
    pub old_slot: usize,
    pub new_slot: usize,
    pub item: Entity,
    // pub container: Entity,
}

/// Item `item` started dragging from target container at `slot`.
#[derive(Event, Debug)]
pub struct ItemDragStart {
    pub slot: usize,
    pub item: Entity,
    // pub container: Entity,
}

/// Item `item` drag ended at target container at `slot`. If slot is `None` the item is being dragged over another item, not an empty slot. No event is fired if the drag is not released over a target item or container
// TODO: add "drag canceled"?
#[derive(Event, Debug)]
pub struct ItemDragEnd {
    pub slot: Option<usize>,
    pub item: Entity,
    // pub container: Entity,
}

/// Item `item` dragged over target container at `slot`. If slot is `None` the item is being dragged over another item, not an empty slot.
#[derive(Event, Debug)]
pub struct ItemDragOver {
    pub slot: Option<usize>,
    pub item: Entity,
    // pub container: Entity,
}

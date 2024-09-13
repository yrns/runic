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

/// Item `item` drag ended at target container at `slot`. No event is fired if the drag is not released over a target item or container.
// TODO: add "drag canceled"? or make slot/item an option
#[derive(Event, Debug)]
pub struct ItemDragEnd {
    pub slot: usize,
    pub item: Entity,
    // pub container: Entity,
}

/// Item `item` dragged over target container at `slot`.
#[derive(Event, Debug)]
pub struct ItemDragOver {
    /// If we are dragging over an item that's a container, and it accepts the dragged item, then the target and slot will be accurate even if the contents are not visible. If the container does not accept the dragged item the target and slot will be of the occupied item.
    // TODO: this is confusing; distinguish "drag to item"?
    pub slot: usize,
    pub item: Entity,
    // pub container: Entity,
}

mod grid;

use bevy_ecs::{name::NameOrEntityItem, prelude::*, query::Spawned, system::SystemParam};
use bevy_math::UVec2;
use bevy_reflect::{Reflect, ReflectDeserialize, ReflectSerialize};
use bevy_ui::{widget::*, *};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tracing::*;

use crate::*;
pub use grid::*;

/// The slot this item occupies in its parent container.
// This really shouldn't derive `Default`, but it has to for BSN?
#[derive(Component, Copy, Clone, PartialEq, Eq, Reflect, FromTemplate)]
#[reflect(Component)]
pub struct Slot(pub UVec2);

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Slot")
            .field(&self.0.x)
            .field(&self.0.y)
            .finish()
    }
}

impl Slot {
    /// Returns a shape index for this slot based on a shape's width.
    pub fn index(&self, width: usize) -> usize {
        self.0.x as usize + self.0.y as usize * width
    }
}

/// NOTE: This paints the contents' shape when items are initially spawned inside it. This requires a slot.
// TODO: Possibly we could insert slot via `find_slot`? Or issue a warning here?
pub fn insert_item(
    items: Query<(&Item, &ItemRotation, &Slot, &ChildOf), Added<ChildOf>>,
    mut contents: Query<&mut GridContents>,
) {
    for (item, rotation, slot, parent) in &items {
        if let Ok(mut contents) = contents.get_mut(parent.0) {
            contents.insert(*slot, &item.clone().with_rotation(*rotation));
        }
    }
}

// fn on_discard_item(
//     event: On<Discard, ChildOf>,
//     items: Query<(&Name, &Item, &ItemRotation, &Slot, &ChildOf)>,
//     mut contents: Query<(&Name, &mut GridContents)>,
// ) {
//     if let Ok((name, item, rotation, slot, parent)) = items.get(event.entity) {
//         dbg!("discard", event.entity, slot);
//     }
// }

pub fn update_node(contents: &GridContents, node: &mut Node) {
    let UVec2 { x, y } = contents.shape.size;

    node.display = Display::Grid;
    // TEMP styling remove
    node.border = px(1.).all();
    node.margin = px(2.).all();
    node.padding = px(2.).all();
    node.width = px(x * 48);
    node.height = px(y * 48);
    node.grid_template_columns = RepeatedGridTrack::px(x as u16, 48.0);
    node.grid_template_rows = RepeatedGridTrack::px(y as u16, 48.0);
    node.align_items = AlignItems::Center;
    node.justify_content = JustifyContent::Center;
}

// Paint shape and set slots here?
pub fn contents_spawned(
    mut commands: Commands,
    mut contents: Query<(Entity, &GridContents, Option<&mut Node>), Spawned>,
) {
    for (id, contents, node) in &mut contents {
        match node {
            Some(mut node) => update_node(contents, &mut *node),
            _ => {
                let mut node = Node::default();
                update_node(contents, &mut node);
                _ = commands.entity(id).insert(node);
            }
        }
    }
}

// fn rotate90(&mut self) {
//     self.rotation = self.rotation.increment();
//     self.item.shape = self.item.shape.rotate90();

//     // This is close but not quite right. This also leaves the slot incorrect...
//     if !self.item.shape.is_square() {
//         self.offset = self.offset.yx();
//         // We need the slot dimensions to recalculate. This works?
//         self.outer_offset = self.outer_offset.yx();
//     }
// }

/// `Accepts` must be `Clone` because items are cloned.
// TODO Indicate textually why something does't accept another?
pub trait Accepts: Copy + Clone + Default + std::fmt::Display + Send + Sync + 'static {
    fn accepts(&self, other: &Self) -> bool;
}

impl<T> Accepts for T
where
    T: bitflags::Flags + Copy + Clone + Default + std::fmt::Display + Send + Sync + 'static,
{
    fn accepts(&self, other: &Self) -> bool {
        self.contains(*other)
    }
}

/// Options.
#[derive(Clone, Debug, Default, Resource)]
pub struct Options {}

/// Bit flags used to determine compatibility between containers and items.
#[derive(Component, Copy, Clone, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Flags<T>(pub T);

impl<T: Accepts> Flags<T> {
    pub fn accepts(&self, flags: &Flags<T>) -> bool {
        self.0.accepts(&flags.0)
    }
}

#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
pub struct DragShape(pub Shape);

#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
pub struct DragRotation(pub ItemRotation);

/// Current target slot in section contents when an item is being dragged.
#[derive(Component, Debug, PartialEq, Eq)]
#[component(storage = "SparseSet")]
pub struct DragSlot(pub Slot);

pub type Items<'w, 's, T> = Query<
    'w,
    's,
    (
        NameOrEntity,
        &'static mut Slot,
        &'static Item,
        &'static mut ItemRotation,
        Option<&'static DragRotation>,
        &'static ChildOf,
        &'static Flags<T>,
    ),
>;

/// Contents storage.
// TODO: There is no more `Contents` trait, so we can rename this. GridSection?
#[derive(SystemParam)]
pub struct ContentsStorage<'w, 's, T: Send + Sync + 'static> {
    pub commands: Commands<'w, 's>,
    pub contents: Query<
        'w,
        's,
        (
            NameOrEntity,
            &'static mut GridContents,
            Option<&'static DragShape>,
            &'static Flags<T>,
        ),
    >,
    /// A container must have children (sections), but is not necessarily an item for "fixed" containers.
    pub children: Query<'w, 's, &'static Children>,
    pub options: Res<'w, Options>,
}

impl<'w, 's, T: Accepts> ContentsStorage<'w, 's, T> {
    pub fn update(&mut self) {
        // if let Some(drag) = self.drag.as_mut() {
        // Rotate the dragged item.
        //if ctx.input(|i| i.key_pressed(egui::Key::R)) {
        // drag.rotate90();
        // TODO: self.commands.trigger(ItemDragRotate { entity: drag.id });
        // }
        // }
    }

    // Some(ContentsResponse::NewTarget((id, slot, _))) => {
    //     // Overwrite the egui id. The original is effectively unused.
    //     self.set_drag_target(Some((id, slot, ui.id())))
    // }
    // Some(ContentsResponse::NewDrag(new_drag)) => {
    //     *self.drag = Some(new_drag);

    //     if let Some(DragItem {
    //         id: item,
    //         source: Some((id, slot, _)),
    //         ..
    //     }) = &*self.drag
    //     {
    //         self.commands.trigger(ItemDragStart {
    //             // source contents
    //             entity: *id,
    //             slot: *slot,
    //             item: *item,
    //         });
    //     }
    // }
    // Some(ContentsResponse::SendItem(mut item)) => {
    //     item.target = self.target.and_then(|t| {
    //         self.find_section_slot(t, &item.item, &item.flags, &item.source)
    //             .map(|(id, slot)| (id, slot, ui.id()))
    //     });
    //     self.resolve_drag(item);
    // }
    // Some(ContentsResponse::Open(item)) => {
    //     if self.is_container(item) {
    //         self.commands.trigger(ContainerOpen(item));
    //     }
    // }

    // Containers and items are now always separate entities (each with separate flags). And the contents entities are contained in the section entity of the item. This means every item that's a container is always two entities...
    // FIX: Items may contain other children besides contents.
    // Only for items, though?
    pub fn is_container(&self, id: Entity) -> bool {
        if let Ok(c) = self.children.get(id) {
            self.contents.iter_many(c).next().is_some()
        } else {
            false
        }
    }

    /// Returns true if the sections of container `a` contains item `b`.
    pub fn contains(&self, a: Entity, b: Entity) -> bool {
        self.children.iter_descendants(a).contains(&b)
    }

    // The different ways to move items:
    // 1. Send to target, control-click.
    // 2. Drag to item, (find section and slot).
    // 3. Drag to slot.
    // 1+2 are the same? Send to target can be a container or section?

    // If the item is more than one slot we can skip some indices...
    // TODO test multiple rotations (if non-square) and return it?
    pub fn find_slot(
        &self,
        section: &GridContents,
        drag_shape: Option<&DragShape>,
        item: &Item,
    ) -> Option<Slot> {
        let shape = drag_shape.map_or(&section.shape, |s| &s.0);
        (0..section.slots())
            .find(|&index| shape.fits(&item.shape, index))
            .map(|i| Slot(shape.slot(i)))
    }

    /// Search all sections of container `id` for an available slot.
    // Cache this over the span of many frames on drag over? This is used every pointer move when dragging?
    pub fn find_section_slot(
        &self,
        target: Entity,
        item: &Item,
        flags: &Flags<T>,
        // source: &DragSource,
    ) -> Option<(Entity, Slot)> {
        // Pass in sections since we're probably already fetching it?
        // Consider layout in the order?
        self.contents
            .iter_many(self.children.get(target).ok()?)
            .filter(|(.., f)| f.accepts(&flags))
            .find_map(|(id, section, drag_shape, _)| {
                self.find_slot(section, drag_shape, item)
                    .map(|slot| (id.entity, slot))
            })
    }

    pub fn resolve_drag(
        &mut self,
        (target_id, target_slot): (Entity, Slot),
        id: NameOrEntityItem,
        mut slot: Mut<Slot>,
        item: &Item,
        mut rotation: Mut<ItemRotation>,
        drag_rotation: Option<&DragRotation>,
        child_of: &ChildOf,
        // flags: &Flags<T>,
    ) {
        // Is the target the same as or inside the item being moved already?
        if id.entity == target_id || self.contains(id.entity, target_id) {
            return warn!("cannot move item {id} inside itself");
        }

        let container_id = child_of.parent();

        // We can't fetch the source and destination container mutably if they're the same.
        let ((_, mut contents, _flags, _items), dest) = if container_id == target_id {
            (
                self.contents
                    .get_mut(container_id)
                    .expect("src container exists"),
                None,
            )
        } else if let Ok([src, dest]) = self.contents.get_many_mut([container_id, target_id]) {
            (src, Some(dest))
        } else {
            return error!(
                "no contents for source ({}) or destination ({})",
                container_id, target_id,
            );
        };

        // Remove from source container with the original rotation applied.
        contents.remove(*slot, &item.clone().with_rotation(*rotation));

        // Copy rotation from the dragged item.
        if let Some(r) = drag_rotation {
            rotation.set_if_neq(r.0);
        }

        // Set target slot.
        // self.commands.entity(id.entity).insert(target_slot);
        slot.set_if_neq(target_slot);

        // Insert into destination container (or source if same).
        {
            let mut contents = match dest {
                Some((_, c, ..)) => c,
                None => contents,
            };
            self.commands.entity(target_id).add_child(id.entity);
            contents.insert(
                target_slot,
                &item
                    .clone()
                    .with_rotation(drag_rotation.map_or(*rotation, |r| r.0)),
            );
        }

        // Fire events.
        let item = id.entity;
        if container_id == target_id {
            self.commands.trigger(ItemMove {
                entity: container_id,
                old_slot: *slot,
                new_slot: target_slot,
                item,
            });
        } else {
            self.commands.trigger(ItemRemove {
                entity: container_id,
                slot: *slot,
                item,
            });

            self.commands.trigger(ItemInsert {
                entity: target_id,
                slot: target_slot,
                item,
            });
        }
    }
}

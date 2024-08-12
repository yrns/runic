pub mod grid;

use crate::*;
pub use grid::*;

use bevy_core::Name;
use bevy_ecs::{prelude::*, system::SystemParam};
use egui::ecolor::{tint_color_towards, Color32};

pub type BoxedContents = Box<dyn Contents + Send + Sync + 'static>;

#[derive(Component)]
pub struct ContentsLayout(pub BoxedContents);

#[derive(Component, Default)]
pub struct ContentsItems(pub Vec<(usize, Entity)>);

// TODO layout per contents?
#[derive(Component)]
pub struct Sections(pub egui::Layout, pub Vec<Entity>);

#[derive(SystemParam)]
pub struct ContentsStorage<'w, 's> {
    pub contents: Query<'w, 's, (&'static mut ContentsLayout, &'static mut ContentsItems)>,
    pub items: Query<'w, 's, (&'static Name, &'static mut Item)>,
    pub sections: Query<'w, 's, &'static Sections>,
}

pub type Items<'a> = &'a [((usize, Entity), (&'a Name, &'a Item))];

impl<'w, 's> ContentsStorage<'w, 's> {
    pub fn show_contents(
        &self,
        id: Entity,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> Option<InnerResponse<MoveData>> {
        let (layout, items) = self.get(id)?;
        Some(layout.0.ui(&id.into(), self, drag_item, &items, ui))
    }

    pub fn get(
        &self,
        id: Entity,
    ) -> Option<(&ContentsLayout, Vec<((usize, Entity), (&Name, &Item))>)> {
        let (layout, items) = self.contents.get(id).ok()?;
        let q_items = self.items.iter_many(items.0.iter().map(|i| i.1));
        // FIX: remove collect
        let items: Vec<((usize, Entity), (&Name, &Item))> =
            items.0.iter().copied().zip(q_items).collect();
        Some((layout, items))
    }

    // Check sections first or last? Last is less recursion.
    pub fn find_slot(
        &self,
        id: Entity,
        // TODO remove?
        _ctx: &Context,
        drag_item: &DragItem,
    ) -> Option<(Entity, usize, egui::Id)> {
        let find_slot = |id| {
            self.contents.get(id).ok().and_then(|(contents, _items)| {
                let ctx = Context::from(id);
                contents.0.find_slot(&ctx, drag_item)
            })
        };

        find_slot(id).or_else(|| {
            self.sections
                .get(id)
                .ok()
                .and_then(|s| s.1.iter().find_map(|id| find_slot(*id)))
        })
    }

    pub fn resolve_move(&mut self, data: MoveData) {
        {
            let MoveData {
                drag:
                    Some(DragItem {
                        id,
                        item: Item { rotation, .. },
                        container: (container_id, container_slot, ..),
                        ..
                    }),
                target: Some((target_id, slot, ..)),
                ..
            } = data
            else {
                return;
            };

            // TODO better check for cycles?
            assert_ne!(
                id, container_id,
                "cannot move an item inside itself: {}",
                id
            );

            let (name, mut item) = self.items.get_mut(id).expect("item exists");

            // We can't fetch the source and destination container mutably if they're the same.
            let (mut src, dest) = if container_id == target_id {
                (
                    self.contents
                        .get_mut(container_id)
                        .expect("src container exists"),
                    None,
                )
            } else {
                let [src, dest] = self.contents.many_mut([container_id, target_id]);
                (src, Some(dest))
            };

            // Remove from source container.
            src.1.remove(container_slot, id);
            (src.0).0.remove(container_slot, item.as_ref());

            // Insert into destination container (or source if same).
            let (mut dest_layout, mut dest) = dest.unwrap_or(src);

            assert!(
                slot < dest_layout.0.len(),
                "destination slot in container range"
            );

            // TODO: put slot_item back on error?
            dest.insert(slot, id);
            dest_layout.0.insert(slot, item.as_ref());

            // Separate components?
            if item.rotation != rotation {
                item.rotation = rotation;
            }

            tracing::info!(
                "moved item {name} {id} {rotation:?} -> container {target_id} slot {slot}"
            );
        }
    }
}

impl ContentsItems {
    pub fn insert(&mut self, slot: usize, id: Entity) {
        let i = self
            .0
            .binary_search_by(|(k, _)| k.cmp(&slot))
            .expect_err("item slot free");
        self.0.insert(i, (slot, id));
    }

    pub fn remove(&mut self, slot: usize, id: Entity) {
        self.0
            .iter()
            .position(|slot_item| *slot_item == (slot, id))
            //.position(|(_, item)| item == id)
            .map(|i| self.0.remove(i))
            .expect("item exists");
    }
}

/// A widget to display the contents of a container.
pub trait Contents {
    fn boxed(self) -> Box<dyn Contents + Send + Sync>
    where
        Self: Sized + Send + Sync + 'static,
    {
        Box::new(self)
    }

    /// Number of slots this container holds.
    fn len(&self) -> usize;

    fn insert(&mut self, slot: usize, item: &Item);

    fn remove(&mut self, slot: usize, item: &Item);

    /// Returns a position for a given slot relative to the contents' origin.
    fn pos(&self, slot: usize) -> egui::Vec2;

    /// Returns a container slot for a given offset. May return
    /// invalid results if the offset is outside the container.
    // This always returns a local slot.
    fn slot(&self, offset: egui::Vec2) -> usize;

    fn accepts(&self, item: &Item) -> bool;

    /// Returns true if the dragged item will fit at the specified slot.
    fn fits(&self, ctx: &Context, item: &DragItem, slot: usize) -> bool;

    /// Finds the first available slot for the dragged item.
    fn find_slot(&self, ctx: &Context, item: &DragItem) -> Option<(Entity, usize, egui::Id)>;

    fn shadow_color(&self, accepts: bool, fits: bool, ui: &egui::Ui) -> egui::Color32 {
        let color = if !accepts {
            Color32::GRAY
        } else if fits {
            Color32::GREEN
        } else {
            Color32::RED
        };
        tint_color_towards(color, ui.visuals().window_fill())
    }

    // Draw contents.
    fn body(
        &self,
        _ctx: &Context,
        _drag_item: &Option<DragItem>,
        _items: Items,
        ui: &mut egui::Ui,
    ) -> InnerResponse<Option<ItemResponse>> {
        // Never used: header, inline, sectioned contents don't call body.
        InnerResponse::new(None, ui.label("❓"))
    }

    fn ui(
        &self,
        ctx: &Context,
        q: &ContentsStorage,
        drag_item: &Option<DragItem>,
        items: Items<'_>,
        ui: &mut egui::Ui,
    ) -> InnerResponse<MoveData>;
}

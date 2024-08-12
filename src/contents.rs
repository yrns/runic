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
    pub contents: Query<'w, 's, (&'static ContentsLayout, &'static mut ContentsItems)>,
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
    // TODO remove contents in return
    pub fn find_slot(
        &self,
        id: Entity,
        // TODO remove?
        _ctx: &Context,
        egui_ctx: &egui::Context,
        drag_item: &DragItem,
    ) -> Option<(&ContentsLayout, (Entity, usize, egui::Id))> {
        let find_slot = |id| {
            self.get(id).and_then(|(contents, items)| {
                // We only need the items to generate shapes, which should go away. TODO
                let ctx = Context::from(id);
                contents
                    .0
                    .find_slot(&ctx, egui_ctx, drag_item, &items)
                    .map(|t| (contents, t))
            })
        };

        find_slot(id).or_else(|| {
            self.sections
                .get(id)
                .ok()
                .and_then(|s| s.1.iter().find_map(|id| find_slot(*id)))
        })
    }

    pub fn resolve_move(&mut self, data: MoveData, ctx: &egui::Context) {
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
            } = &data
            else {
                return;
            };

            // TODO better check for cycles?
            assert_ne!(
                id, container_id,
                "cannot move an item inside itself: {}",
                id
            );

            let (name, mut item) = self.items.get_mut(*id).expect("item exists");

            // We can't fetch the source and destination container mutably if they're the same.
            let (mut src, dest) = if container_id == target_id {
                (
                    self.contents
                        .get_mut(*container_id)
                        .expect("src container exists"),
                    None,
                )
            } else {
                let [src, dest] = self.contents.many_mut([*container_id, *target_id]);
                (src, Some(dest))
            };

            // Remove from source container. Items must be ordered by slot in order for section
            // contents to work.
            let _slot_item = (src.1)
                .0
                .iter()
                .position(|slot_item| *slot_item == (*container_slot, *id))
                //.position(|(_, item)| item == id)
                .map(|i| (src.1).0.remove(i))
                .expect("src exists");

            // Insert into destination container (or source if same).
            let (dest_layout, mut dest) = dest.unwrap_or(src);
            let i = dest
                .0
                .binary_search_by(|(k, _)| k.cmp(slot))
                .expect_err("dest item slot free");

            // TODO: put slot_item back on error?

            assert!(
                *slot < dest_layout.0.len(),
                "destination slot in container range"
            );

            dest.0.insert(i, (*slot, *id));

            // Separate components?
            if item.rotation != *rotation {
                item.rotation = *rotation;
            }

            tracing::info!(
                "moved item {name} {id} {rotation:?} -> container {target_id} slot {slot}"
            );

            data.resolve(ctx);
        }
    }
}

/// A widget to display the contents of a container.
pub trait Contents {
    /// Returns an egui id based on the contents id. Unused, except
    /// for loading state.
    // fn eid(&self, id: usize) -> egui::Id {
    //     // Containers are items, so we need a unique id for the contents.
    //     egui::Id::new("contents").with(id)
    // }

    fn boxed(self) -> Box<dyn Contents + Send + Sync>
    where
        Self: Sized + Send + Sync + 'static,
    {
        Box::new(self)
    }

    /// Number of slots this container holds.
    fn len(&self) -> usize;

    /// Creates a thunk that is resolved after a move when an item is
    /// added. The contents won't exist after a move so we use this to
    /// update internal state in lieu of a normal trait method. `slot`
    /// is used for sectioned contents only. SectionContents needs to
    /// be updated...
    fn add(&self, _ctx: &Context, _slot: usize) -> Option<ResolveFn> {
        None
    }

    /// Returns a thunk that is resolved after a move when an item is removed.
    fn remove(&self, _ctx: &Context, _slot: usize, _shape: shape::Shape) -> Option<ResolveFn> {
        None
    }

    /// Returns a position for a given slot relative to the contents' origin.
    fn pos(&self, slot: usize) -> egui::Vec2;

    /// Returns a container slot for a given offset. May return
    /// invalid results if the offset is outside the container.
    // This always returns a local slot.
    fn slot(&self, offset: egui::Vec2) -> usize;

    fn accepts(&self, item: &Item) -> bool;

    /// Returns true if the dragged item will fit at the specified slot.
    fn fits(&self, ctx: &Context, egui_ctx: &egui::Context, item: &DragItem, slot: usize) -> bool;

    /// Finds the first available slot for the dragged item.
    #[allow(unused)]
    fn find_slot(
        &self,
        ctx: &Context,
        egui_ctx: &egui::Context,
        item: &DragItem,
        items: Items,
    ) -> Option<(Entity, usize, egui::Id)> {
        // Expanding, header, inline contents never call this. This is only called on drag to
        // (container) item. TODO: This is only a trait method since it uses the grid contents
        // shape?
        unimplemented!();
        // find_slot_default(self, ctx, egui_ctx, item, items)
    }

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

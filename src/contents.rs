pub mod expanding;
pub mod grid;
pub mod header;
pub mod inline;
pub mod section;

use crate::*;
pub use expanding::*;
pub use grid::*;
pub use header::*;
pub use inline::*;
pub use section::*;

use bevy_core::Name;
use bevy_ecs::{prelude::*, system::SystemParam};
use egui::ecolor::{tint_color_towards, Color32};

pub type BoxedContents = Box<dyn Contents + Send + Sync + 'static>;

#[derive(Component)]
pub struct ContentsLayout(pub BoxedContents);

#[derive(Component)]
pub struct ContentsItems(pub Vec<(usize, Entity)>);

#[derive(SystemParam)]
pub struct ContentsStorage<'w, 's> {
    pub contents: Query<'w, 's, (&'static ContentsLayout, &'static mut ContentsItems)>, // Option<&'static Sections> ?
    pub items: Query<'w, 's, (&'static Name, &'static mut Item)>,
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

// TODO Eliminate the Contents trait:
// Everything boils down to a grid contents.
// Expanding is a special case one by one grid.
// Header, inline, and potentially section contents are just layout concerns.
// Sectioned contents can be made into a a collection of contents, and their layout.

/// Local slot (slot - offset).
#[derive(Copy, Clone, Debug)]
pub struct LocalSlot(pub usize);

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
    fn add(&self, _ctx: &Context, _slot: LocalSlot) -> Option<ResolveFn> {
        None
    }

    /// Returns a thunk that is resolved after a move when an item is removed.
    fn remove(&self, _ctx: &Context, _slot: LocalSlot, _shape: shape::Shape) -> Option<ResolveFn> {
        None
    }

    /// Returns a position for a given slot relative to the contents' origin.
    fn pos(&self, slot: LocalSlot) -> egui::Vec2;

    /// Returns a container slot for a given offset. May return
    /// invalid results if the offset is outside the container.
    // This always returns a local slot.
    fn slot(&self, offset: egui::Vec2) -> LocalSlot;

    fn accepts(&self, item: &Item) -> bool;

    /// Returns true if the dragged item will fit at the specified slot.
    fn fits(
        &self,
        ctx: &Context,
        egui_ctx: &egui::Context,
        item: &DragItem,
        slot: LocalSlot,
    ) -> bool;

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

    // Default impl should handle everything including
    // grid/sectioned/expanding containers. Iterator type changed to
    // (usize, &Item) so section contents can rewrite slots.
    fn ui(
        &self,
        ctx: &Context,
        q: &ContentsStorage,
        drag_item: &Option<DragItem>,
        // This used to be an option but we're generally starting with
        // show_contents at the root which implies items. (You can't
        // have items w/o a layout or vice-versa).
        items: Items<'_>,
        ui: &mut egui::Ui,
    ) -> InnerResponse<MoveData> {
        // This no longer works because `drag_item` is a frame behind `dragged_id`. In other words, the
        // dragged_id will be unset before drag_item for one frame.

        // match drag_item.as_ref().map(|d| d.item.eid()) {
        //     Some(id) => {
        //         assert_eq!(ui.ctx().dragged_id(), Some(id));
        //         // if ui.ctx().dragged_id() != Some(id) {
        //         //     tracing::warn!(
        //         //         "drag_item eid {:?} != dragged_id {:?}",
        //         //         id,
        //         //         ui.ctx().dragged_id()
        //         //     )
        //         // }
        //     }
        //     _ => (), // we could be dragging something else
        // }

        // Go back to with_bg/min_frame since egui::Frame takes up all available space.
        crate::min_frame::min_frame(ui, |style, ui| {
            // Reserve shape for the dragged item's shadow.
            let shadow = ui.painter().add(egui::Shape::Noop);

            let InnerResponse { inner, response } = self.body(ctx, drag_item, items, ui);
            let min_rect = response.rect;

            // TODO move everything into the match

            // If we are dragging onto another item, check to see if
            // the dragged item will fit anywhere within its contents.
            match (drag_item, inner.as_ref()) {
                (Some(drag), Some(ItemResponse::Hover((slot, id, item)))) => {
                    if let Some((contents, items)) = q.get(*id) {
                        let ctx = Context::from(*id);
                        let target = contents.0.find_slot(&ctx, ui.ctx(), drag, &items);

                        // The item shadow becomes the target item, not the dragged item, for
                        // drag-to-item. TODO just use rect
                        let color = self.shadow_color(true, target.is_some(), ui);
                        let mut mesh = egui::Mesh::default();
                        mesh.add_colored_rect(
                            egui::Rect::from_min_size(
                                min_rect.min + self.pos(*slot),
                                item.size_rotated(),
                            ),
                            color,
                        );
                        ui.painter().set(shadow, mesh);

                        return InnerResponse::new(
                            MoveData {
                                drag: None,
                                target, //: (id, slot, eid),
                                add_fn: target.and_then(|(_, slot, ..)| {
                                    contents.0.add(&ctx, ctx.local_slot(slot))
                                }),
                            },
                            response,
                        );
                    }
                }
                _ => (),
            }

            // tarkov also checks if containers are full, even if not
            // hovering -- maybe track min size free? TODO just do
            // accepts, and only check fits for hover
            let dragging = drag_item.is_some();

            let mut move_data = MoveData {
                drag: match inner {
                    // TODO NewDrag?
                    Some(ItemResponse::Drag(drag)) => Some(drag),
                    _ => None,
                },
                ..Default::default()
            };

            if !dragging {
                return InnerResponse::new(move_data, response);
            }

            let accepts = drag_item
                .as_ref()
                .map(|drag| self.accepts(&drag.item))
                .unwrap_or_default();

            // Highlight the contents border if we can accept the dragged item.
            if accepts {
                // TODO move this to settings?
                style.bg_stroke = ui.visuals().widgets.hovered.bg_stroke;
            }

            // `contains_pointer` does not work for the target because only the dragged items'
            // response will contain the pointer.
            let slot = ui
                .ctx()
                .pointer_latest_pos()
                // the hover includes the outer_rect?
                .filter(|p| min_rect.contains(*p))
                .map(|p| self.slot(p - min_rect.min));

            let Context {
                container_id: id,
                container_eid: eid,
                ..
            } = *ctx;

            let fits = drag_item
                .as_ref()
                .zip(slot)
                .map(|(item, slot)| self.fits(ctx, ui.ctx(), item, slot))
                .unwrap_or_default();

            // Paint the dragged item's shadow, showing which slots will
            // be filled.
            if let Some((drag, slot)) = drag_item.as_ref().zip(slot) {
                let color = self.shadow_color(accepts, fits, ui);
                // Use the rotated shape.
                let shape = drag.item.shape();
                let mesh = shape_mesh(&shape, min_rect, self.pos(slot), color, SLOT_SIZE);
                ui.painter().set(shadow, mesh);
            }

            // Only send target on release?
            let released = ui.input(|i| i.pointer.any_released());
            if released && fits && !accepts {
                tracing::info!(
                    "container {:?} does not accept item {:?}!",
                    id,
                    drag_item.as_ref().map(|drag| drag.item.flags)
                );
            }

            // accepts ⇒ dragging, fits ⇒ dragging, fits ⇒ slot

            match slot {
                Some(slot) if accepts && fits => {
                    // The target eid is unused?
                    // dbg!(slot.0, ctx.slot_offset);
                    move_data.target = Some((id, slot.0 + ctx.slot_offset, eid));
                    move_data.add_fn = self.add(ctx, slot);
                }
                _ => (),
            }
            InnerResponse::new(move_data, response)
        })
    }
}

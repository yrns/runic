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

use egui::ecolor::*;

/// A widget to display the contents of a container.
pub trait Contents {
    /// Returns an egui id based on the contents id. Unused, except
    /// for loading state.
    // fn eid(&self, id: usize) -> egui::Id {
    //     // Containers are items, so we need a unique id for the contents.
    //     egui::Id::new("contents").with(id)
    // }

    fn boxed(self) -> Box<dyn Contents>
    where
        Self: Sized + 'static,
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
    fn add(&self, _ctx: Context, _slot: usize) -> Option<ResolveFn> {
        None
    }

    /// Returns a thunk that is resolved after a move when an item is removed.
    fn remove(&self, _ctx: Context, _slot: usize, _shape: shape::Shape) -> Option<ResolveFn> {
        None
    }

    /// Returns a position for a given slot relative to the contents' origin.
    fn pos(&self, slot: usize) -> egui::Vec2;

    /// Returns a container slot for a given offset. May return
    /// invalid results if the offset is outside the container.
    fn slot(&self, offset: egui::Vec2) -> usize;

    fn accepts(&self, item: &Item) -> bool;

    /// Returns true if the dragged item will fit at the specified slot.
    fn fits(&self, ctx: Context, egui_ctx: &egui::Context, item: &DragItem, slot: usize) -> bool;

    /// Finds the first available slot for the dragged item.
    fn find_slot(
        &self,
        ctx: Context,
        egui_ctx: &egui::Context,
        item: &DragItem,
        items: &[(usize, Item)],
    ) -> Option<(usize, usize, egui::Id)> {
        find_slot_default(self, ctx, egui_ctx, item, items)
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
        _ctx: Context,
        _drag_item: &Option<DragItem>,
        _items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<ItemResponse>> {
        // never used
        InnerResponse::new(None, ui.label("❓"))
    }

    // Default impl should handle everything including
    // grid/sectioned/expanding containers. Iterator type changed to
    // (usize, &Item) so section contents can rewrite slots.
    fn ui(
        &self,
        ctx: Context,
        q: &ContentsStorage,
        drag_item: &Option<DragItem>,
        // This used to be an option but we're generally starting with
        // show_contents at the root which implies items. (You can't
        // have items w/o a layout or vice-versa).
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData> {
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

        // We have all the information we need to set the style from
        // the MoveData/drag_item, so we could do that internal to
        // with_bg... ItemResponse::Item would need to be
        // preserved. Also, with_item_shadow()?
        with_bg(ui, |style, mut ui| {
            // The item shadow becomes the target item, not the dragged
            // item, for drag-to-item?

            // Reserve shape for the dragged item's shadow.
            let shadow = ui.painter().add(egui::Shape::Noop);

            let egui::InnerResponse { inner, response } = self.body(ctx, drag_item, items, &mut ui);

            let min_rect = ui.min_rect();

            // If we are dragging onto another item, check to see if
            // the dragged item will fit anywhere within its contents.
            match (drag_item, inner.as_ref()) {
                // hover ⇒ dragging
                (Some(drag), Some(ItemResponse::Hover((slot, item)))) => {
                    if let Some((contents, items)) = q.get(&item.id) {
                        let ctx = item.id.into_ctx();
                        let target = contents.find_slot(ctx, ui.ctx(), drag, items.as_slice());

                        // TODO fits && !accepts?
                        let color = self.shadow_color(true, target.is_some(), ui);
                        let mut mesh = egui::Mesh::default();
                        mesh.add_colored_rect(
                            // TODO make sure slot is correct for sections
                            egui::Rect::from_min_size(
                                min_rect.min + self.pos(*slot),
                                item.size_rotated(),
                            ),
                            color,
                        );
                        ui.painter().set(shadow, mesh);

                        return MoveData {
                            drag: None,
                            target, //: (id, slot, eid),
                            add_fn: target.and_then(|t| contents.add(ctx, t.1)),
                        };
                    }
                }
                _ => (),
            }

            // tarkov also checks if containers are full, even if not
            // hovering -- maybe track min size free? TODO just do
            // accepts, and only check fits for hover
            let dragging = drag_item.is_some();

            // `contains_pointer` does not work for the target because only the dragged items'
            // response will contain the pointer.
            let slot = // response.contains_pointer()
                // .then_some(())
                // .and_then(|_| ui.ctx().pointer_latest_pos())
                ui.ctx().pointer_latest_pos()
                // the hover includes the outer_rect?
                .filter(|p| min_rect.contains(*p))
                .map(|p| self.slot(p - min_rect.min));

            let accepts = drag_item
                .as_ref()
                .map(|drag| self.accepts(&drag.item))
                .unwrap_or_default();

            let (id, eid, _) = ctx;

            let fits = drag_item
                .as_ref()
                .zip(slot)
                .map(|(item, slot)| self.fits(ctx, ui.ctx(), item, slot))
                .unwrap_or_default();

            // Paint the dragged item's shadow, showing which slots will
            // be filled.
            if let Some(drag) = drag_item {
                if let Some(slot) = slot {
                    let color = self.shadow_color(accepts, fits, ui);
                    ui.painter().set(
                        shadow,
                        shape_mesh(&drag.item.shape, min_rect, self.pos(slot), color, ITEM_SIZE),
                    );
                }
            }

            // Is response.hovered() ever true when dragging?
            if !(dragging && accepts && response.hovered()) {
                *style = ui.visuals().widgets.inactive;
            };

            if dragging && accepts {
                // gray out:
                style.bg_fill = tint_color_towards(style.bg_fill, ui.visuals().window_fill());
                style.bg_stroke.color =
                    tint_color_towards(style.bg_stroke.color, ui.visuals().window_fill());
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

            // if released {
            //     dbg!(dragging, accepts, slot, fits);
            // }

            // accepts ⇒ dragging, fits ⇒ dragging, fits ⇒ slot

            MoveData {
                drag: match inner {
                    Some(ItemResponse::Drag(drag)) => Some(drag),
                    _ => None,
                },
                // The target eid is unused..? dragging implied...
                target: (dragging && accepts && fits).then(|| (id, slot.unwrap(), eid)),
                add_fn: (accepts && fits)
                    .then(|| self.add(ctx, slot.unwrap()))
                    .flatten(),
            }
        })
    }
}

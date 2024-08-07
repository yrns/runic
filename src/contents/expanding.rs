use crate::*;

// An expanding container fits only one item but it can be any size up
// to a maximum size. This is useful for equipment slots where only
// one item can go and the size varies.
#[derive(Clone, Debug)]
pub struct ExpandingContents {
    // TODO this needs an "empty size" as well
    pub max_size: shape::Vec2,
    pub flags: ItemFlags,
}

impl ExpandingContents {
    pub fn new(max_size: impl Into<shape::Vec2>) -> Self {
        Self {
            max_size: max_size.into(),
            flags: ItemFlags::all(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<ItemFlags>) -> Self {
        self.flags = flags.into();
        self
    }
}

// Indicates this expanding slot is filled.
#[derive(Copy, Clone, Default)]
struct Filled(bool);

impl Contents for ExpandingContents {
    fn len(&self) -> usize {
        1
    }

    // We don't need these since it's reset in body and only used after...

    // fn add(&self, _slot: usize) {
    //     // assert!(slot == 0);
    //     // ctx.data().insert_temp(self.eid(), true);
    // }

    // fn remove(&self, _slot: usize) {
    //     // assert!(slot == 0);
    //     // ctx.data().insert_temp(self.eid(), false);
    // }

    // Expanding contents only ever has one slot.
    fn pos(&self, _slot: usize) -> egui::Vec2 {
        egui::Vec2::ZERO
    }

    // Expanding contents only ever has one slot.
    fn slot(&self, _offset: egui::Vec2) -> usize {
        0
    }

    fn accepts(&self, item: &Item) -> bool {
        self.flags.contains(item.flags)
    }

    // How do we visually show if the item is too big? What if the
    // item is rotated and oblong, and only fits one way?
    fn fits(
        &self,
        (_id, eid, _): Context,
        ctx: &egui::Context,
        drag: &DragItem,
        slot: usize,
    ) -> bool {
        // Allow rotating in place.
        let current_item = eid == drag.container.2;
        let filled = !current_item
            && ctx
                .data(|d| d.get_temp::<Filled>(eid).unwrap_or_default())
                .0;
        let size = drag.item.shape_size();
        slot == 0 && !filled && size.cmple(self.max_size).all()
    }

    fn body(
        &self,
        ctx: Context,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<ItemResponse>> {
        let item = items.first();

        let (id, eid, offset) = ctx;
        ui.data_mut(|d| d.insert_temp(eid, Filled(item.is_some())));

        assert!(items.len() <= 1);

        // is_rect_visible? TODO
        let (new_drag, response) = match item {
            Some((slot, item)) => {
                assert_eq!(*slot, offset);

                let (dragged, item) = drag::item!(drag_item, item);

                // This is kind of a hack. We don't want to blow out the container if it doesn't
                // fit. We only want to expand if it fits. We've already checked if it does so, so
                // maybe we should cache that in drag_item?
                let size = if !dragged || self.fits(ctx, ui.ctx(), drag_item.as_ref().unwrap(), 0) {
                    item.size_rotated()
                } else {
                    slot_size()
                };

                let InnerResponse { inner, response } =
                    ui.allocate_ui(size, |ui| item.ui(drag_item, ui));
                (
                    inner.map(|item| match item {
                        ItemResponse::NewDrag(item) => ItemResponse::Drag(DragItem {
                            item,
                            container: (id, *slot, eid),
                            cshape: None,
                            remove_fn: None,
                        }),
                        // We don't need to update ItemResponse::Hover(...)
                        // since the default slot is 0.
                        _ => item,
                    }),
                    response,
                )
            }
            _ => (
                None,
                // Should the empty size be some minimum value? Or the max?
                ui.allocate_exact_size(slot_size(), egui::Sense::hover()).1,
            ),
        };

        InnerResponse::new(new_drag, response)
    }
}

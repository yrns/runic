use crate::*;

// An expanding container fits only one item but it can be any size up
// to a maximum size. This is useful for equipment slots where only
// one item can go and the size varies.
#[derive(Clone, Debug)]
pub struct ExpandingContents {
    pub max_size: shape::Vec2,
    pub flags: FlagSet<ItemFlags>,
}

impl ExpandingContents {
    pub fn new(max_size: impl Into<shape::Vec2>) -> Self {
        Self {
            max_size: max_size.into(),
            flags: Default::default(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<FlagSet<ItemFlags>>) -> Self {
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

    fn pos(&self, _slot: usize) -> egui::Vec2 {
        egui::Vec2::ZERO
    }

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
        slot == 0 && !filled && drag.item.shape.size.cmple(self.max_size).all()
    }

    fn body(
        &self,
        (id, eid, offset): Context,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<ItemResponse>> {
        let item = items.first();

        ui.data_mut(|d| d.insert_temp(eid, Filled(item.is_some())));

        assert!(items.len() <= 1);

        // is_rect_visible?
        let (new_drag, response) = match item {
            Some((slot, item)) => {
                assert_eq!(*slot, offset);

                let InnerResponse { inner, response } =
                    // item.size() isn't rotated... TODO: test
                    // non-square containers, review item.size() everywhere
                    ui.allocate_ui(item.size(), |ui| item.ui(drag_item, ui));
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
                ui.allocate_exact_size(item_size(), egui::Sense::hover()).1,
            ),
        };

        InnerResponse::new(new_drag, response)
    }
}

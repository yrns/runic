use crate::*;

// A container for a single item (or "slot") that, when containing
// another container, the interior contents are displayed inline.
#[derive(Clone, Debug)]
pub struct InlineContents(ExpandingContents);

impl InlineContents {
    pub fn new(contents: ExpandingContents) -> Self {
        Self(contents)
    }
}

impl Contents for InlineContents {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn pos(&self, slot: usize) -> egui::Vec2 {
        self.0.pos(slot)
    }

    fn slot(&self, offset: egui::Vec2) -> usize {
        self.0.slot(offset)
    }

    fn accepts(&self, item: &Item) -> bool {
        self.0.accepts(item)
    }

    fn fits(&self, ctx: &Context, egui_ctx: &egui::Context, item: &DragItem, slot: usize) -> bool {
        self.0.fits(ctx, egui_ctx, item, slot)
    }

    fn ui(
        &self,
        ctx: &Context,
        q: &ContentsStorage,
        drag_item: &Option<DragItem>,
        items: Items,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData> {
        // get the layout and contents of the contained item (if any)
        let inline_id = items.first().map(|((_, id), ..)| *id);

        // TODO: InlineLayout?
        ui.horizontal_top(|ui| {
            let data = self.0.ui(ctx, q, drag_item, items, ui).inner;

            // ui.label("Inline contents:");

            if let Some(id) = inline_id {
                // Don't add contents if the container is being dragged.
                if !drag_item.as_ref().is_some_and(|d| d.id == id) {
                    if let Some(inline_data) = q.show_contents(id, drag_item, ui) {
                        return data.merge(inline_data.inner);
                    }
                }
            }

            data
        })
    }
}

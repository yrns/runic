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

    fn fits(&self, ctx: Context, egui_ctx: &egui::Context, item: &DragItem, slot: usize) -> bool {
        self.0.fits(ctx, egui_ctx, item, slot)
    }

    fn ui(
        &self,
        ctx: Context,
        q: &ContentsStorage,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData> {
        // get the layout and contents of the contained item (if any)
        let inline_id = items.first().map(|(_, item)| item.id);

        // TODO: InlineLayout?
        ui.horizontal(|ui| {
            let data = self.0.ui(ctx, q, drag_item, items, ui).inner;

            if let Some(id) = inline_id {
                // Don't add contents if the container is being dragged.
                if !drag_item.as_ref().is_some_and(|d| d.item.id == id) {
                    if let Some((contents, items)) = q.get(&id) {
                        return data
                            .merge(contents.ui(id.into_ctx(), q, drag_item, items, ui).inner);
                    }
                }
            }

            data
        })
    }
}

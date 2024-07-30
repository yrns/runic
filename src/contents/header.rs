use crate::*;

#[derive(Clone, Debug)]
pub struct HeaderContents<T> {
    // Box<dyn Fn(...)> ? Not clonable.
    pub header: String,
    pub contents: T,
}

impl<T> HeaderContents<T> {
    pub fn new(header: impl Into<String>, contents: T) -> Self {
        Self {
            header: header.into(),
            contents,
        }
    }
}

pub type ContentsStorage = HashMap<
    usize,
    (
        Box<dyn Contents + Send + Sync + 'static>,
        Vec<(usize, Item)>,
    ),
>;

impl<T> Contents for HeaderContents<T>
where
    T: Contents,
{
    fn len(&self) -> usize {
        self.contents.len()
    }

    fn pos(&self, slot: usize) -> egui::Vec2 {
        self.contents.pos(slot)
    }

    fn slot(&self, offset: egui::Vec2) -> usize {
        self.contents.slot(offset)
    }

    fn accepts(&self, item: &Item) -> bool {
        self.contents.accepts(item)
    }

    fn fits(&self, ctx: Context, egui_ctx: &egui::Context, item: &DragItem, slot: usize) -> bool {
        self.contents.fits(ctx, egui_ctx, item, slot)
    }

    fn ui(
        &self,
        ctx: Context,
        q: &ContentsStorage,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> InnerResponse<MoveData> {
        // Is InnerResponse really useful?
        let InnerResponse { inner, response } = ui.vertical(|ui| {
            ui.label(&self.header);
            self.contents.ui(ctx, q, drag_item, items, ui)
        });
        InnerResponse::new(inner.inner, response)
    }
}

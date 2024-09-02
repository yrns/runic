use bevy_egui::egui::{self, style::WidgetVisuals, InnerResponse, Margin, Rect, Sense, Shape, Ui};

/// This is similar to Frame::group except, critically, it takes up the minimum space, not the
/// maximum. The style is mutable so it can be modified based on the contents.
pub fn min_frame<R>(
    ui: &mut Ui,
    add_contents: impl FnOnce(&mut WidgetVisuals, &mut Ui) -> InnerResponse<R>,
) -> InnerResponse<R> {
    let margin = Margin::same(4.0);
    let outer_rect = ui.available_rect_before_wrap();
    let inner_rect = outer_rect - margin;

    // Reserve a shape for the background so it draws first.
    let bg = ui.painter().add(Shape::Noop);
    let mut content_ui = ui.child_ui(inner_rect, *ui.layout(), None);

    // Draw contents.
    let mut style = ui.visuals().widgets.noninteractive;
    let inner = add_contents(&mut style, &mut content_ui);

    // let outer_rect: Rect = content_ui.min_rect() + margin;
    let outer_rect: Rect = inner.response.rect + margin;
    let (rect, response) = ui.allocate_exact_size(outer_rect.size(), Sense::hover());

    // ui.ctx()
    //     .debug_painter()
    //     .debug_rect(rect, Color32::LIGHT_BLUE, "*");

    ui.painter().set(
        bg,
        egui::epaint::RectShape::new(rect, style.rounding, style.bg_fill, style.bg_stroke),
    );

    InnerResponse::new(inner.inner, response)
}

use eframe::egui;
use egui::InnerResponse;

fn main() {
    tracing_subscriber::fmt::init();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "runic",
        options,
        Box::new(|_cc| {
            // this should pull from a data source only when it changes,
            // and only for open containers?
            Box::new(Runic {
                container: Container {
                    id: 0,
                    // remove?
                    drag_item: None,
                    items: vec![Item::new(1), Item::new(2)],
                },
            })
        }),
    )
}

struct Runic {
    // nothing yet
    container: Container,
}

// item/container + widget id? tuple
type ItemData = egui::Id;
type ContainerData = usize; //egui::Id;

/// source item -> target container
type State = (Option<ItemData>, Option<ContainerData>);

impl eframe::App for Runic {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // mult containers?
            ui.add(ContainerSpace::new(|ui| self.container.ui(ui)))
        });
    }
}

struct ContainerSpace<F> {
    add_contents: Box<F>,
}

impl<F> ContainerSpace<F>
where
    F: FnOnce(&mut egui::Ui) -> egui::InnerResponse<State>,
{
    fn new(add_contents: F) -> Self {
        Self {
            add_contents: add_contents.into(),
        }
    }
}

impl<F> egui::widgets::Widget for ContainerSpace<F>
where
    F: FnOnce(&mut egui::Ui) -> egui::InnerResponse<State>,
{
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let Self { add_contents } = self;
        let egui::InnerResponse { inner, response } = add_contents(ui);
        // do something w/ inner state, i.e. move items
        if let (Some(from), Some(to)) = inner {
            if ui.input().pointer.any_released() {
                tracing::info!("moving item {:?} -> container {:?}", from, to);
            }
        }
        response
    }
}

// this is a struct because it'll eventually be a trait
struct Container {
    id: usize,
    drag_item: Option<ItemData>,
    items: Vec<Item>,
    //size: ?
    //mask: ?
    //slot/type: ?
}

impl Container {
    fn body(&self, ui: &mut egui::Ui) -> egui::InnerResponse<Option<ItemData>> {
        // make a grid, use ui.put for manual layout
        ui.horizontal(|ui| {
            let mut drag_item = None;
            for item in self.items.iter() {
                if let Some(id) = item.ui(ui) {
                    drag_item = Some(id)
                }
            }
            drag_item
        })
    }

    // this is drop_target
    fn ui(&self, ui: &mut egui::Ui) -> egui::InnerResponse<State> {
        let can_accept_what_is_being_dragged = true;

        let dragging = ui.memory().is_anything_being_dragged();
        let accept_move = dragging && can_accept_what_is_being_dragged;

        let margin = egui::Vec2::splat(4.0);

        let outer_rect_bounds = ui.available_rect_before_wrap();
        let inner_rect = outer_rect_bounds.shrink2(margin);
        let where_to_put_background = ui.painter().add(egui::Shape::Noop);
        let mut content_ui = ui.child_ui(inner_rect, *ui.layout());

        let egui::InnerResponse { inner, response: _ } = self.body(&mut content_ui);

        let outer_rect =
            egui::Rect::from_min_max(outer_rect_bounds.min, content_ui.min_rect().max + margin);
        let (rect, response) = ui.allocate_at_least(outer_rect.size(), egui::Sense::hover());

        let style = if accept_move && response.hovered() {
            ui.visuals().widgets.active
        } else {
            ui.visuals().widgets.inactive
        };

        let mut fill = style.bg_fill;
        let mut stroke = style.bg_stroke;
        if accept_move {
            // gray out:
            fill = egui::color::tint_color_towards(fill, ui.visuals().window_fill());
            stroke.color =
                egui::color::tint_color_towards(stroke.color, ui.visuals().window_fill());
        }

        ui.painter().set(
            where_to_put_background,
            eframe::epaint::RectShape {
                rounding: style.rounding,
                fill,
                stroke,
                rect,
            },
        );

        InnerResponse::new((inner, accept_move.then_some(self.id)), response)
    }
}

struct Item {
    id: usize,
    rotation: ItemRotation,
    // lieu of size?
    //mask: ItemMask,
    // width/height in units
    size: (u8, u8),
}

impl Item {
    fn new(id: usize) -> Self {
        Self {
            id,
            rotation: Default::default(),
            size: (1, 1),
        }
    }

    fn body(&self, ui: &mut egui::Ui) -> egui::Response {
        // the demo adds a context menu here for removing items
        // check the response id is the item id?
        ui.add(egui::Label::new(format!("item {}", self.id)).sense(egui::Sense::click()))
    }

    // this combines drag_source and the body, need to separate again
    fn ui(&self, ui: &mut egui::Ui) -> Option<ItemData> {
        // egui::InnerResponse<ItemData> {
        let id = egui::Id::new(self.id);
        let drag = ui.memory().is_being_dragged(id);
        if !drag {
            let response = ui.scope(|ui| self.body(ui)).response;
            let response = ui.interact(response.rect, id, egui::Sense::drag());
            if response.hovered() {
                ui.output().cursor_icon = egui::CursorIcon::Grab;
            }
            None
        } else {
            ui.output().cursor_icon = egui::CursorIcon::Grabbing;

            let layer_id = egui::LayerId::new(egui::Order::Tooltip, id);
            let response = ui.with_layer_id(layer_id, |ui| self.body(ui)).response;

            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let delta = pointer_pos - response.rect.center();
                ui.ctx().translate_layer(layer_id, delta);
            }
            Some(id)
        }
    }
}

#[derive(Default)]
enum ItemRotation {
    #[default]
    Up,
    Left,
    Down,
    Right,
}

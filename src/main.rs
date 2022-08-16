use eframe::egui;
use egui::InnerResponse;

mod shape;

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
                container1: Container {
                    id: 1,
                    items: vec![Item::new(2), Item::new(3)],
                },
                container2: Container {
                    id: 4,
                    items: vec![Item::new(5), Item::new(6)],
                },
            })
        }),
    )
}

struct Runic {
    container1: Container,
    container2: Container,
}

// item/container + widget id? tuple
type ItemId = usize;
type ContainerId = usize;
type ItemData = (ItemId, ContainerId);
type ContainerData = ContainerId; //egui::Id;

/// source item -> target container
struct MoveData {
    item: Option<ItemData>,
    container: Option<ContainerData>,
}

impl MoveData {
    fn merge(self, other: Self) -> Self {
        if self.item.and(other.item).is_some() {
            tracing::error!("multiple items! ({:?} and {:?})", self.item, other.item);
        }
        if self.container.and(other.container).is_some() {
            tracing::error!(
                "multiple containers! ({:?} and {:?})",
                self.container,
                other.container
            );
        }
        Self {
            item: self.item.or(other.item),
            container: self.container.or(other.container),
        }
    }

    fn data(&self) -> Option<(ItemData, ContainerData)> {
        match self {
            Self {
                item: Some(item),
                container: Some(container),
            } => Some((*item, *container)),
            _ => None,
        }
    }
}

impl eframe::App for Runic {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some((item, container)) = ContainerSpace::default().show(ui, |ui| {
                // need to resolve how these results are merged with a
                // hierarchy of containers and items
                self.container1
                    .ui(ui)
                    .inner
                    .merge(self.container2.ui(ui).inner)
            }) {
                tracing::info!("moving item {:?} -> container {:?}", item, container);

                let (id, prev) = item;

                if let Some(item) = match prev {
                    1 => self.container1.remove(id),
                    4 => self.container2.remove(id),
                    _ => None,
                } {
                    match container {
                        1 => self.container1.items.push(item),
                        4 => self.container2.items.push(item),
                        _ => (),
                    }
                }
            }
        });
    }
}

#[derive(Default)]
struct ContainerSpace {
    // nothing yet
}

impl ContainerSpace {
    // not a widget since it doesn't return a Response
    fn show(
        self,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&mut egui::Ui) -> MoveData,
    ) -> Option<(ItemData, ContainerData)> {
        // what about a handler for the container that had an item
        // removed?
        // include old container id?
        // do something w/ inner state, i.e. move items
        let data = add_contents(ui).data();
        match ui.input().pointer.any_released() {
            true => data,
            _ => None,
        }
    }
}

// this is a struct because it'll eventually be a trait
struct Container {
    id: usize,
    // returned from item
    //drag_item: Option<ItemData>,
    items: Vec<Item>,
    //size: ?
    //mask: ?
    //slot/type: ?
}

impl Container {
    fn remove(&mut self, id: ItemId) -> Option<Item> {
        let idx = self.items.iter().position(|item| item.id == id);
        idx.map(|i| self.items.remove(i))
    }

    fn body(&self, ui: &mut egui::Ui) -> egui::InnerResponse<Option<ItemData>> {
        // make a grid, use ui.put for manual layout
        ui.horizontal(|ui| {
            let mut drag_item = None;
            for item in self.items.iter() {
                if let Some(id) = item.ui(ui) {
                    drag_item = Some((id, self.id))
                }
            }
            drag_item
        })
    }

    // this is drop_target
    fn ui(&self, ui: &mut egui::Ui) -> egui::InnerResponse<MoveData> {
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

        InnerResponse::new(
            MoveData {
                item: inner,
                container: (accept_move && response.hovered()).then_some(self.id),
            },
            response,
        )
    }
}

struct Item {
    id: usize,
    rotation: ItemRotation,
    // lieu of size?
    //mask: ItemMask,
    // width/height in units
    shape: shape::Shape,
}

impl Item {
    fn new(id: usize) -> Self {
        Self {
            id,
            rotation: Default::default(),
            shape: shape::Shape::new((1, 1), true),
        }
    }

    fn body(&self, ui: &mut egui::Ui) -> egui::Response {
        // the demo adds a context menu here for removing items
        // check the response id is the item id?
        ui.add(egui::Label::new(format!("item {}", self.id)).sense(egui::Sense::click()))
    }

    // this combines drag_source and the body, need to separate again
    fn ui(&self, ui: &mut egui::Ui) -> Option<ItemId> {
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
            Some(self.id)
        }
    }
}

#[derive(Default)]
enum ItemRotation {
    #[default]
    Up,
    //Left,
    //Down,
    //Right,
}

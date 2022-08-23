use eframe::egui;
use egui::{InnerResponse, TextureId};
use egui_extras::RetainedImage;

mod shape;

const ITEM_SIZE: f32 = 32.0;

fn main() {
    tracing_subscriber::fmt::init();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "runic",
        options,
        Box::new(|cc| {
            let icon = RetainedImage::from_image_bytes(
                "potion-icon-24.png",
                include_bytes!("../potion-icon-24.png",),
            )
            .unwrap();

            //let icon = RetainedImage::from_color_image("example", egui::ColorImage::example());

            // this should pull from a data source only when it changes,
            // and only for open containers?

            let mut container1 = Container::new(1, 4, 4);
            container1.add(0, Item::new(2, icon.texture_id(&cc.egui_ctx)));

            let mut container2 = Container::new(3, 2, 2);
            container2.add(0, Item::new(4, icon.texture_id(&cc.egui_ctx)));

            Box::new(Runic {
                icon,
                drag_item: None,
                container1,
                container2,
            })
        }),
    )
}

struct Runic {
    icon: RetainedImage,
    drag_item: Option<DragItem>,
    container1: Container,
    container2: Container,
}

type ContainerId = usize;
// item -> old container
type DragItem = (Item, ContainerId);
// new container -> new slot
type ContainerData = (ContainerId, usize);

/// source item -> target container
#[derive(Debug)]
struct MoveData {
    item: Option<DragItem>,
    container: Option<ContainerData>,
}

impl MoveData {
    // we could just use zip
    fn merge(self, other: Self) -> Self {
        if self.item.is_some() && other.item.is_some() {
            tracing::error!("multiple items! ({:?} and {:?})", &self.item, &other.item);
        }
        if self.container.is_some() && other.container.is_some() {
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
}

impl eframe::App for Runic {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some((item, container)) =
                ContainerSpace::show(&mut self.drag_item, ui, |drag_item, ui| {
                    // need to resolve how these results are merged with a
                    // hierarchy of containers and items
                    self.container1
                        .ui(drag_item, ui)
                        .inner
                        .merge(self.container2.ui(drag_item, ui).inner)
                })
            {
                tracing::info!("moving item {:?} -> container {:?}", item, container);

                let (item, prev) = item;

                if let Some(item) = match prev {
                    1 => self.container1.remove(item.id),
                    3 => self.container2.remove(item.id),
                    _ => None,
                } {
                    match container {
                        (1, slot) => self.container1.add(slot, item), // FIX,
                        (3, slot) => self.container2.add(slot, item),
                        _ => (),
                    }
                }

                tracing::info!("container1 items: {:?}", self.container1.items.len());
                tracing::info!("container2 items: {:?}", self.container2.items.len());
            }
        });
    }
}

struct ContainerSpace;

impl ContainerSpace {
    // not a widget since it doesn't return a Response
    fn show(
        drag_item: &mut Option<DragItem>,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&Option<DragItem>, &mut egui::Ui) -> MoveData,
    ) -> Option<(DragItem, ContainerData)> {
        // what about a handler for the container that had an item
        // removed?
        // do something w/ inner state, i.e. move items
        let MoveData { item, container } = add_contents(drag_item, ui);
        if let Some(item) = item {
            // assert!(drag_item.is_none());
            //*drag_item = Some(item);
            assert!(drag_item.replace(item).is_none());
        }
        //let dragged = ui.memory().is_anything_being_dragged();
        // tracing::info!(
        //     "dragging: {} pointer released: {} drag_item: {} container: {}",
        //     dragged,
        //     ui.input().pointer.any_released(),
        //     drag_item.is_some(),
        //     container.is_some()
        // );
        ui.input()
            .pointer
            .any_released()
            .then(|| drag_item.take().zip(container))
            .flatten()
    }
}

// this is a struct because it'll eventually be a trait?
struct Container {
    id: usize,
    // returned from item
    //drag_item: Option<DragItem>,
    items: Vec<(usize, Item)>,
    shape: shape::Shape,
    //slot/type: ?
}

impl Container {
    fn new(id: usize, width: usize, height: usize) -> Self {
        Self {
            id,
            items: Vec::new(),
            shape: shape::Shape::new((width, height), false),
        }
    }

    fn pos(&self, slot: usize) -> egui::Vec2 {
        egui::Vec2::new(
            (slot % self.shape.width()) as f32 * ITEM_SIZE,
            (slot / self.shape.width()) as f32 * ITEM_SIZE,
        )
    }

    /// Returns container slot given a point inside the container's
    /// shape. Returns invalid results if outside.
    fn slot(&self, p: egui::Vec2) -> usize {
        let p = p / ITEM_SIZE;
        p.x as usize + p.y as usize * self.shape.width()
    }

    fn add(&mut self, slot: usize, item: Item) {
        self.shape.paint(&item.shape, self.shape.pos(slot));
        self.items.push((slot, item));
    }

    fn remove(&mut self, id: usize) -> Option<Item> {
        let idx = self.items.iter().position(|(_, item)| item.id == id);
        idx.map(|i| self.items.remove(i)).map(|(slot, item)| {
            self.shape.unpaint(&item.shape, self.shape.pos(slot));
            item
        })
    }

    fn body(
        &self,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>> {
        // allocate the full container size
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(
                self.shape.width() as f32 * ITEM_SIZE,
                self.shape.height() as f32 * ITEM_SIZE,
            ),
            egui::Sense::hover(),
        );

        let mut new_drag = None;

        if ui.is_rect_visible(rect) {
            let item_size = egui::vec2(ITEM_SIZE, ITEM_SIZE);
            for (slot, item) in self.items.iter() {
                let item_rect =
                    egui::Rect::from_min_size(ui.min_rect().min + self.pos(*slot), item_size);
                // item returns its id if it's being dragged
                if let Some(id) = ui
                    .allocate_ui_at_rect(item_rect, |ui| item.ui(drag_item, ui))
                    .inner
                {
                    // add the container id
                    new_drag = Some((id, self.id))
                }
            }
        }
        InnerResponse::new(new_drag, response)
    }

    // this is drop_target
    fn ui(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> egui::InnerResponse<MoveData> {
        let margin = egui::Vec2::splat(4.0);

        let outer_rect_bounds = ui.available_rect_before_wrap();
        let inner_rect = outer_rect_bounds.shrink2(margin);
        let where_to_put_background = ui.painter().add(egui::Shape::Noop);
        let mut content_ui = ui.child_ui(inner_rect, *ui.layout());

        let egui::InnerResponse { inner, response } = self.body(drag_item, &mut content_ui);
        let mut dragging = false;
        let mut accepts = false;
        let mut slot = None;
        let mut fits = false;

        // need to remove this if anything else utilizes drag (e.g. a slider)
        if ui.memory().is_anything_being_dragged() != drag_item.is_some() {
            tracing::error!(
                "container: {} dragging: {} drag_item: {:?}",
                self.id,
                ui.memory().is_anything_being_dragged(),
                drag_item
            );
        }

        // we need to pass back the full data from items since they
        // can be containers too, container is a super type of item?

        //if let Some((item, _container)) = &inner {
        if let Some((item, container)) = drag_item {
            dragging = true;
            // tarkov also checks if containers are full, even if not
            // hovering -- maybe track min size free?
            accepts = true; // check item type TODO

            let grid_rect = content_ui.min_rect();
            slot = response
                .hover_pos()
                // the hover includes the outer_rect?
                .filter(|p| grid_rect.contains(*p))
                .map(|p| self.slot(p - grid_rect.min));

            // check if the shape fits here
            // TODO unpaint shape if same container move

            if let Some(slot) = slot {
                // When moving within one container, unpaint the shape
                // first.
                fits = if *container == self.id {
                    let mut shape = self.shape.clone();
                    let p = shape.pos(slot);
                    shape.unpaint(&item.shape, p);
                    shape.fits(&item.shape, p)
                } else {
                    self.shape.fits(&item.shape, self.shape.pos(slot))
                };

                let color = if fits {
                    egui::color::Color32::GREEN
                } else {
                    egui::color::Color32::RED
                };
                let color = egui::color::tint_color_towards(color, ui.visuals().window_fill());

                // paint slot
                let slot_rect = egui::Rect::from_min_size(
                    grid_rect.min + self.pos(slot),
                    egui::Vec2::new(ITEM_SIZE, ITEM_SIZE),
                );
                ui.painter()
                    .rect(slot_rect, 0., color, egui::Stroke::none())
            }
        }

        let outer_rect =
            egui::Rect::from_min_max(outer_rect_bounds.min, content_ui.min_rect().max + margin);
        let (rect, response) = ui.allocate_at_least(outer_rect.size(), egui::Sense::hover());

        let style = if dragging && accepts && response.hovered() {
            ui.visuals().widgets.active
        } else {
            ui.visuals().widgets.inactive
        };

        let mut fill = style.bg_fill;
        let mut stroke = style.bg_stroke;
        if dragging && accepts {
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
                container: (dragging && accepts && fits).then(|| (self.id, slot.unwrap())),
            },
            response,
        )
    }
}

#[derive(Clone, Debug)]
struct Item {
    id: usize,
    rotation: ItemRotation,
    // lieu of size?
    //mask: ItemMask,
    // width/height in units
    shape: shape::Shape,
    icon: TextureId,
}

impl Item {
    fn new(id: usize, icon: TextureId) -> Self {
        Self {
            id,
            rotation: Default::default(),
            shape: shape::Shape::new((1, 1), true),
            icon,
        }
    }

    fn body(&self, ui: &mut egui::Ui) -> egui::Response {
        // the demo adds a context menu here for removing items
        // check the response id is the item id?
        //ui.add(egui::Label::new(format!("item {}", self.id)).sense(egui::Sense::click()))

        ui.add(egui::Image::new(self.icon, (ITEM_SIZE, ITEM_SIZE)).sense(egui::Sense::click()))
    }

    // this combines drag_source and the body, need to separate again
    fn ui(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> Option<Item> {
        // egui::InnerResponse<DragItem> {
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

            // make sure there is no existing drag_item or it matches
            // our id
            assert!(
                drag_item.is_none() || drag_item.as_ref().map(|(item, _)| item.id) == Some(self.id)
            );

            // only send back a clone if this is a new drag (drag_item
            // is empty)
            if drag_item.is_none() {
                Some(self.clone())
            } else {
                None
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
enum ItemRotation {
    #[default]
    Up,
    //Left,
    //Down,
    //Right,
}

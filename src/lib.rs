use egui::{InnerResponse, TextureId};

pub mod shape;

pub const ITEM_SIZE: f32 = 32.0;

pub type ContainerId = usize;
// item -> old container -> old slot -> old container shape minus the
// drag shape
pub type DragItem = (Item, ContainerId, usize, shape::Shape);
// new container -> new slot
pub type ContainerData = (ContainerId, usize);

/// source item -> target container
#[derive(Debug, Default)]
pub struct MoveData {
    pub item: Option<DragItem>,
    pub container: Option<ContainerData>,
}

impl MoveData {
    // we could just use zip
    pub fn merge(self, other: Self) -> Self {
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

pub struct ContainerSpace;

impl ContainerSpace {
    // not a widget since it doesn't return a Response
    pub fn show(
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

        // Rotate the dragged item.
        if ui.input().key_pressed(egui::Key::R) {
            if let Some((item, _, _, _)) = drag_item.as_mut() {
                item.rotation = item.rotation.increment();
                item.shape = item.shape.rotate90();
            }
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

pub fn paint_shape(
    shape: &shape::Shape,
    grid_rect: egui::Rect,
    offset: egui::Vec2,
    color: egui::Color32,
    ui: &mut egui::Ui,
) {
    let offset = grid_rect.min + offset;
    shape
        .slots()
        .map(|slot| offset + shape.pos_f32(slot, ITEM_SIZE).into())
        .filter(|p| grid_rect.contains(*p + egui::vec2(1., 1.)))
        .for_each(|p| {
            let slot_rect = egui::Rect::from_min_size(p, egui::Vec2::new(ITEM_SIZE, ITEM_SIZE));
            ui.painter()
                .rect(slot_rect, 0., color, egui::Stroke::none())
        })
}

// this is a struct because it'll eventually be a trait?
pub struct Container {
    pub id: usize,
    // returned from item
    //drag_item: Option<DragItem>,
    pub items: Vec<(usize, Item)>,
    pub shape: shape::Shape,
    //slot/type: ?
}

impl Container {
    pub fn new(id: usize, width: usize, height: usize) -> Self {
        Self {
            id,
            items: Vec::new(),
            shape: shape::Shape::new((width, height), false),
        }
    }

    pub fn pos(&self, slot: usize) -> egui::Vec2 {
        self.shape.pos_f32(slot, ITEM_SIZE).into()
    }

    /// Returns container slot given a point inside the container's
    /// shape. Returns invalid results if outside.
    pub fn slot(&self, p: egui::Vec2) -> usize {
        let p = p / ITEM_SIZE;
        p.x as usize + p.y as usize * self.shape.width()
    }

    pub fn add(&mut self, slot: usize, item: Item) {
        self.shape.paint(&item.shape(), self.shape.pos(slot));
        self.items.push((slot, item));
    }

    pub fn remove(&mut self, id: usize) -> Option<Item> {
        let idx = self.items.iter().position(|(_, item)| item.id == id);
        idx.map(|i| self.items.remove(i)).map(|(slot, item)| {
            self.shape.unpaint(&item.shape(), self.shape.pos(slot));
            item
        })
    }

    pub fn body(
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
                    // add the container id and current slot and
                    // container shape w/ the item unpainted
                    let mut shape = self.shape.clone();
                    shape.unpaint(&item.shape(), self.shape.pos(*slot));
                    new_drag = Some((id, self.id, *slot, shape))
                }
            }
        }
        InnerResponse::new(new_drag, response)
    }

    // this is drop_target
    pub fn ui(
        &self,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData> {
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
        if let Some((item, container, _curr_slot, cshape)) = drag_item {
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

            if let Some(slot) = slot {
                // Check if the shape fits here. When moving within
                // one container, use the cached shape with the
                // dragged item (and original rotation) unpainted.
                let shape = if *container == self.id {
                    cshape
                } else {
                    &self.shape
                };

                // debug paint the container "shape" (filled slots)
                paint_shape(
                    shape,
                    grid_rect,
                    egui::Vec2::ZERO,
                    egui::color::Color32::DARK_BLUE,
                    ui,
                );

                fits = shape.fits(&item.shape, self.shape.pos(slot));

                let color = if fits {
                    egui::color::Color32::GREEN
                } else {
                    egui::color::Color32::RED
                };
                let color = egui::color::tint_color_towards(color, ui.visuals().window_fill());

                // paint item slots
                paint_shape(&item.shape, grid_rect, self.pos(slot), color, ui);
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
            egui::epaint::RectShape {
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
pub struct Item {
    pub id: usize,
    pub rotation: ItemRotation,
    pub shape: shape::Shape,
    pub icon: TextureId,
}

impl Item {
    pub fn new(id: usize, icon: TextureId, shape: shape::Shape) -> Self {
        Self {
            id,
            rotation: Default::default(),
            shape,
            icon,
        }
    }

    pub fn body(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> egui::Response {
        // the demo adds a context menu here for removing items
        // check the response id is the item id?
        //ui.add(egui::Label::new(format!("item {}", self.id)).sense(egui::Sense::click()))

        ui.add(
            egui::Image::new(
                self.icon,
                (
                    ITEM_SIZE * self.shape.width() as f32,
                    ITEM_SIZE * self.shape.height() as f32,
                ),
            )
            .rotate(
                drag_item
                    .as_ref()
                    .filter(|item| item.0.id == self.id)
                    .map_or(self.rotation, |item| item.0.rotation)
                    .angle(),
                egui::Vec2::splat(0.5),
            )
            .sense(egui::Sense::click()),
        )
    }

    // this combines drag_source and the body, need to separate again
    pub fn ui(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> Option<Item> {
        // egui::InnerResponse<DragItem> {
        let id = egui::Id::new(self.id);
        let drag = ui.memory().is_being_dragged(id);
        if !drag {
            let response = ui.scope(|ui| self.body(drag_item, ui)).response;
            let response = ui.interact(response.rect, id, egui::Sense::drag());
            if response.hovered() {
                ui.output().cursor_icon = egui::CursorIcon::Grab;
            }
            None
        } else {
            ui.output().cursor_icon = egui::CursorIcon::Grabbing;

            let layer_id = egui::LayerId::new(egui::Order::Tooltip, id);
            let response = ui
                .with_layer_id(layer_id, |ui| self.body(drag_item, ui))
                .response;

            if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
                let delta = pointer_pos - response.rect.center();
                ui.ctx().translate_layer(layer_id, delta);
            }

            // make sure there is no existing drag_item or it matches
            // our id
            assert!(
                drag_item.is_none() || drag_item.as_ref().map(|item| item.0.id) == Some(self.id)
            );

            // only send back a clone if this is a new drag (drag_item
            // is empty)
            if drag_item.is_none() {
                // This clones the shape twice...
                Some(self.clone().rotate())
            } else {
                None
            }
        }
    }

    // This returns a clone every time, even if not rotated.
    fn shape(&self) -> shape::Shape {
        match self.rotation {
            ItemRotation::None => self.shape.clone(),
            ItemRotation::R90 => self.shape.rotate90(),
            ItemRotation::R180 => self.shape.rotate180(),
            ItemRotation::R270 => self.shape.rotate270(),
        }
    }

    // Rotate the (dragged) shape to match the item's rotation.
    fn rotate(mut self) -> Self {
        self.shape = self.shape();
        self
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum ItemRotation {
    #[default]
    None,
    R90,
    R180,
    R270,
}

impl ItemRotation {
    pub fn increment(&self) -> Self {
        match self {
            Self::None => Self::R90,
            Self::R90 => Self::R180,
            Self::R180 => Self::R270,
            _ => Self::None,
        }
    }

    pub fn angle(&self) -> f32 {
        match *self {
            Self::None => 0.0,
            Self::R90 => 90.0_f32.to_radians(),
            Self::R180 => 180.0_f32.to_radians(),
            Self::R270 => 270.0_f32.to_radians(),
        }
    }
}

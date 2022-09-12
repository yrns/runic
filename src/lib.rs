use egui::{InnerResponse, TextureId};
use itertools::Itertools;

pub mod shape;

pub const ITEM_SIZE: f32 = 32.0;

pub fn item_size() -> egui::Vec2 {
    egui::vec2(ITEM_SIZE, ITEM_SIZE)
}

pub type ContainerId = usize;
// item -> old container -> old slot -> old container shape minus the
// drag shape
pub type DragItem = (Item, ContainerId, usize, Option<shape::Shape>);
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

    pub fn map_slots<F>(self, f: F) -> Self
    where
        F: Fn(usize) -> usize,
    {
        let Self { item, container } = self;
        Self {
            item: item.map(|mut item| {
                item.2 = f(item.2);
                item
            }),
            container: container.map(|mut c| {
                c.1 = f(c.1);
                c
            }),
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
        .map(|slot| offset + xy(slot, shape.width()) * ITEM_SIZE)
        .filter(|p| grid_rect.contains(*p + egui::vec2(1., 1.)))
        .for_each(|p| {
            let slot_rect = egui::Rect::from_min_size(p, item_size());
            ui.painter()
                .rect(slot_rect, 0., color, egui::Stroke::none())
        })
}

// Is this a trait or generic struct?
// pub trait Item {
//     type Id;
// }

// Are slots intrinsic to contents?
/// A widget to display the contents of a container.
pub trait Contents: std::fmt::Debug {
    //type Item;
    //type Items: for<'a> Iterator<Item = &'a (usize, Item)>;

    fn id(&self) -> usize;

    /// Returns an egui id based on the contents id. Unused, except
    /// for loading state.
    fn eid(&self) -> egui::Id {
        // Containers are items, so we need a unique id for the contents.
        egui::Id::new("contents").with(self.id())
    }

    /// Number of slots this container holds.
    fn len(&self) -> usize;

    // Maybe these would be more generally useful if they were passed
    // an egui context?

    /// Notifies the contents when an item is added.
    fn add(&mut self, _ctx: &egui::Context, _slot: usize, _item: &Item) {}

    /// Notifies the contents when an item is removed.
    //fn remove(&mut self, id: <Self::Item as Item>::Id, item: Self::Item)
    fn remove(&mut self, _ctx: &egui::Context, _slot: usize, _item: &Item) {}

    /// Returns a position for a given slot relative to the contents' origin.
    fn pos(&self, slot: usize) -> egui::Vec2;

    /// Returns a container slot for a given offset. May return
    /// invalid results if the offset is outside the container.
    fn slot(&self, offset: egui::Vec2) -> usize;

    fn accepts(&self, _item: &Item) -> bool {
        true
    }

    // What about fits anywhere/any slot?
    fn fits(&self, _ctx: &egui::Context, _item: &DragItem, _slot: usize) -> bool {
        true
    }

    // Draw contents.
    fn body(
        &mut self,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>>;

    // Default impl should handle everything including grid/sectioned/expanding containers.
    fn ui(
        &mut self,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)], //impl Iterator<Item = &'a (usize, Item)>,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData> {
        let margin = egui::Vec2::splat(4.0);
        let outer_rect_bounds = ui.available_rect_before_wrap();
        let inner_rect = outer_rect_bounds.shrink2(margin);
        // reserve a shape for the background so it draws first
        let bg = ui.painter().add(egui::Shape::Noop);
        let mut content_ui = ui.child_ui(inner_rect, *ui.layout());

        let egui::InnerResponse { inner, response } = self.body(drag_item, items, &mut content_ui);

        // tarkov also checks if containers are full, even if not
        // hovering -- maybe track min size free?
        let dragging = drag_item.is_some();
        let accepts = drag_item
            .as_ref()
            .map(|item| self.accepts(&item.0))
            .unwrap_or_default();

        let r = content_ui.min_rect();
        let slot = response
            .hover_pos()
            // the hover includes the outer_rect?
            .filter(|p| r.contains(*p))
            .map(|p| self.slot(p - r.min));

        let fits = drag_item
            .as_ref()
            .zip(slot)
            .map(|(item, slot)| self.fits(ui.ctx(), item, slot))
            .unwrap_or_default();

        assert!(match drag_item {
            Some(item) => ui.memory().is_being_dragged(item.0.eid()),
            _ => true, // we could be dragging something else
        });

        //self.drag(drag_item, slot, fits, content_ui);

        if let Some((item, _container, _curr_slot, _shape)) = drag_item {
            if let Some(slot) = slot {
                let color = if fits {
                    egui::color::Color32::GREEN
                } else {
                    egui::color::Color32::RED
                };
                let color = egui::color::tint_color_towards(color, ui.visuals().window_fill());

                // paint item slots, need to reserve shapes so this
                // draws w/ the background
                paint_shape(&item.shape, r, self.pos(slot), color, ui);
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
            bg,
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
                container: (dragging && accepts && fits).then(|| (self.id(), slot.unwrap())),
            },
            response,
        )
    }
}

// TODO rename GridContents? Grid shares some meaning w/ egui::Grid
#[derive(Debug)]
pub struct Container {
    // This shares w/ items, but the eid is unique.
    pub id: usize,
    pub size: shape::Vec2,
    //slot/type: ?
}

impl Container {
    pub fn new(id: usize, width: usize, height: usize) -> Self {
        Self {
            id,
            size: (width, height).into(),
        }
    }
}

pub fn xy(slot: usize, width: usize) -> egui::Vec2 {
    egui::Vec2::new((slot % width) as f32, (slot / width) as f32)
}

impl Contents for Container {
    fn id(&self) -> usize {
        self.id
    }

    fn len(&self) -> usize {
        self.size.len()
    }

    fn add(&mut self, ctx: &egui::Context, slot: usize, item: &Item) {
        // There is no get_temp_mut... If the shape doesn't exist we
        // don't care since it will be regenerated next time the
        // container is shown.
        let shape = ctx.data().get_temp::<shape::Shape>(self.eid());
        if let Some(mut shape) = shape {
            shape.paint(&item.shape(), slot);
            ctx.data().insert_temp(self.eid(), shape);
        }
    }

    fn remove(&mut self, ctx: &egui::Context, slot: usize, item: &Item) {
        // See above.
        let shape = ctx.data().get_temp::<shape::Shape>(self.eid());
        if let Some(mut shape) = shape {
            shape.unpaint(&item.shape(), slot);
            ctx.data().insert_temp(self.eid(), shape);
        }
    }

    fn pos(&self, slot: usize) -> egui::Vec2 {
        xy(slot, self.size.x as usize) * ITEM_SIZE
    }

    fn slot(&self, p: egui::Vec2) -> usize {
        let p = p / ITEM_SIZE;
        p.x as usize + p.y as usize * self.size.x as usize
    }

    fn fits(&self, ctx: &egui::Context, item: &DragItem, slot: usize) -> bool {
        // Must be careful with the type inference here since it will
        // never fetch anything if it thinks it's a reference.
        match ctx.data().get_temp(self.eid()) {
            Some(shape) => {
                // Check if the shape fits here. When moving within
                // one container, use the cached shape with the
                // dragged item (and original rotation) unpainted.
                let shape = match (item.1 == self.id(), &item.3) {
                    // (true, None) should never happen...
                    (true, Some(shape)) => shape,
                    _ => &shape,
                };

                shape.fits(&item.0.shape, slot)
            }
            None => {
                // TODO remove this
                tracing::error!("shape {:?} not found!", self.eid());
                false
            }
        }
    }

    fn body(
        &mut self,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>> {
        // allocate the full container size
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(
                self.size.x as f32 * ITEM_SIZE,
                self.size.y as f32 * ITEM_SIZE,
            ),
            egui::Sense::hover(),
        );

        // Skip this if the container is empty? Only if dragging into
        // this container? Only if visible? What if we are dragging to
        // a container w/o the contents visible/open? Is it possible
        // to have an empty shape without a bitvec allocated until
        // painted?  [`fits`] also checks the boundaries even if the
        // container is empty...
        let shape = ui
            .data()
            .get_temp_mut_or_insert_with(self.eid(), || {
                items.iter().fold(
                    shape::Shape::new(self.size, false),
                    |mut shape, (slot, item)| {
                        shape.paint(&item.shape, *slot);
                        shape
                    },
                )
            })
            // We have to clone the shape here otherwise we're holding
            // a reference into ui...
            .clone();

        // TODO make debug option

        // Use the cached shape if the dragged item is ours. This
        // rehashes what's in `fits`.
        let shape = drag_item
            .as_ref()
            .filter(|item| self.id == item.1)
            .and_then(|item| item.3.as_ref())
            .unwrap_or(&shape);

        // debug paint the container "shape" (filled slots)
        paint_shape(
            shape,
            ui.min_rect(),
            egui::Vec2::ZERO,
            egui::color::Color32::DARK_BLUE,
            ui,
        );

        let mut new_drag = None;

        if ui.is_rect_visible(rect) {
            let item_size = item_size();
            for (slot, item) in items {
                let item_rect =
                    egui::Rect::from_min_size(ui.min_rect().min + self.pos(*slot), item_size);
                // item returns a clone if it's being dragged
                if let Some(item) = ui
                    .allocate_ui_at_rect(item_rect, |ui| item.ui(drag_item, ui))
                    .inner
                {
                    // add the container id and current slot and
                    // container shape w/ the item unpainted
                    let mut shape = shape.clone();
                    // We've already cloned the item and we're cloning
                    // the shape again to rotate? Isn't it already rotated?
                    shape.unpaint(&item.shape(), *slot);
                    new_drag = Some((item, self.id, *slot, Some(shape)))
                }
            }
        }
        InnerResponse::new(new_drag, response)
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

    /// Returns an egui id based on the item id.
    pub fn eid(&self) -> egui::Id {
        egui::Id::new(self.id)
    }

    /// Size of the item in pixels.
    pub fn size(&self) -> egui::Vec2 {
        egui::Vec2::new(
            self.shape.width() as f32 * ITEM_SIZE,
            self.shape.height() as f32 * ITEM_SIZE,
        )
    }

    pub fn body(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> egui::Response {
        // the demo adds a context menu here for removing items
        // check the response id is the item id?
        //ui.add(egui::Label::new(format!("item {}", self.id)).sense(egui::Sense::click()))

        ui.add(
            egui::Image::new(self.icon, self.size())
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

    pub fn ui(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> Option<Item> {
        let id = self.eid();
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

// A sectioned container is a set of smaller containers displayed as
// one. Like pouches on a belt or different pockets in a jacket. It's
// one item than holds many fixed containers. This should have the
// same interface as [Container].
#[derive(Debug)]
pub struct SectionContainer {
    pub id: usize,
    pub layout: SectionLayout,
    pub sections: Vec<Container>, // Vec<Box<dyn Contents>>?
}

#[derive(Debug)]
pub enum SectionLayout {
    Grid(usize),
    // Fixed(Vec<(usize, egui::Pos2))
    // Columns?
    // Other(Fn?)
}

impl SectionContainer {
    fn section_slot(&self, slot: usize) -> Option<(usize, usize)> {
        self.section_ranges()
            .enumerate()
            .find_map(|(i, (start, end))| (slot < end).then(|| (i, slot - start)))
    }

    fn section_ranges(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let mut end = 0;
        self.sections.iter().map(move |s| {
            let start = end;
            end = end + s.len();
            (start, end)
        })
    }
}

impl Contents for SectionContainer {
    fn id(&self) -> usize {
        self.id
    }

    fn len(&self) -> usize {
        self.sections.iter().map(|s| s.len()).sum()
    }

    fn add(&mut self, ctx: &egui::Context, slot: usize, item: &Item) {
        if let Some((i, slot)) = self.section_slot(slot) {
            self.sections[i].add(ctx, slot, item)
        }
    }

    fn remove(&mut self, ctx: &egui::Context, slot: usize, item: &Item) {
        if let Some((i, slot)) = self.section_slot(slot) {
            self.sections[i].remove(ctx, slot, item)
        }
    }

    fn pos(&self, _slot: usize) -> egui::Vec2 {
        todo!()
    }

    fn slot(&self, _offset: egui::Vec2) -> usize {
        todo!()
    }

    fn body(
        &mut self,
        _drag_item: &Option<DragItem>,
        _items: &[(usize, Item)],
        _ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>> {
        unimplemented!()
    }

    // Since the items list is unsorted and we only work with slices,
    // we have to sort into a new collection and take slices of
    // it. This needs revisiting.
    fn ui(
        &mut self,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData> {
        // map (slot, item) -> (section, (slot, item))
        let ranges = self.section_ranges().collect_vec();
        let items = items
            .iter()
            .filter_map(|(slot, item)| {
                ranges
                    .iter()
                    .enumerate()
                    .find_map(|(section, (start, end))| {
                        // We have to clone the item here since the
                        // contents wants a slice of items, not a
                        // slice of refs. Maybe change the trait?
                        (slot < end).then(|| (section, ((slot - start), item.clone())))
                    })
            })
            .into_group_map();

        let empty = Vec::new();

        match self.layout {
            SectionLayout::Grid(width) => {
                egui::Grid::new(self.id).num_columns(width).show(ui, |ui| {
                    self.sections
                        .iter_mut()
                        .zip(ranges.iter())
                        .enumerate()
                        .map(|(i, (section, (start, _end)))| {
                            let data = section
                                .ui(
                                    drag_item,
                                    items.get(&i).unwrap_or_else(|| &empty).as_slice(),
                                    ui,
                                )
                                .inner;

                            if (i + 1) % width == 0 {
                                ui.end_row();
                            }

                            // Remap slots.
                            data.map_slots(|slot| slot + start)
                        })
                        .reduce(|acc, a| acc.merge(a))
                        .unwrap_or_default()
                })
            }
        }
    }
}

// An expanding container fits only one item but it can be any size up
// to a maximum size. This is useful for equipment slots where only
// one item can go and the size varies.
#[derive(Debug)]
pub struct ExpandingContainer {
    pub id: usize,
    pub max_size: shape::Vec2,
    // This won't be valid until body is called.
    pub filled: bool,
}

impl ExpandingContainer {
    pub fn new(id: usize, max_size: impl Into<shape::Vec2>) -> Self {
        Self {
            id,
            max_size: max_size.into(),
            filled: false,
        }
    }
}

impl Contents for ExpandingContainer {
    fn id(&self) -> usize {
        self.id
    }

    fn len(&self) -> usize {
        1
    }

    fn add(&mut self, ctx: &egui::Context, slot: usize, _item: &Item) {
        assert!(slot == 0);
        ctx.data().insert_temp(self.eid(), true);
    }

    fn remove(&mut self, ctx: &egui::Context, slot: usize, _item: &Item) {
        assert!(slot == 0);
        ctx.data().insert_temp(self.eid(), false);
    }

    fn pos(&self, _slot: usize) -> egui::Vec2 {
        egui::Vec2::ZERO
    }

    fn slot(&self, _offset: egui::Vec2) -> usize {
        0
    }

    // How do we visually show if the item is too big?
    fn fits(&self, _ctx: &egui::Context, item: &DragItem, slot: usize) -> bool {
        slot == 0 && !self.filled && item.0.shape.size.le(&self.max_size)
    }

    fn body(
        &mut self,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>> {
        assert!(items.len() <= 1);

        let item = items.iter().next();
        self.filled = item.is_some();

        // is_rect_visible?
        let (new_drag, response) = match item {
            Some((slot, item)) => {
                assert!(*slot == 0);
                let InnerResponse { inner, response } =
                    ui.allocate_ui(item.size(), |ui| item.ui(drag_item, ui));
                (inner.map(|item| (item, self.id, *slot, None)), response)
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

// An expanding container that contains another container, the
// contents of which are displayed inline when present.
pub struct InlineContainer {
    pub container: ExpandingContainer,
    pub contents: Container,
}

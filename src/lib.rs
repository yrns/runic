use egui::{InnerResponse, TextureId};
use flagset::{flags, FlagSet};
use itertools::Itertools;

pub mod shape;

pub const ITEM_SIZE: f32 = 48.0;

// static?
pub fn item_size() -> egui::Vec2 {
    egui::vec2(ITEM_SIZE, ITEM_SIZE)
}

pub type ContainerId = usize;

/// Target container id and slot.
pub type ContainerData = (ContainerId, usize);

pub type ResolveFn = Box<dyn FnMut(&egui::Context, &DragItem, ContainerData)>;

pub struct DragItem {
    /// A clone of the original item with rotation applied.
    pub item: Item,
    /// Source container id and slot.
    pub container: ContainerData,
    /// Source container shape with item unpainted, used for fit
    /// checking if dragged within the source container.
    pub cshape: Option<shape::Shape>,
    pub remove_fn: Option<ResolveFn>,
}

/// source item -> target container
#[derive(Default)]
pub struct MoveData {
    pub drag: Option<DragItem>,
    pub target: Option<ContainerData>,
    pub add_fn: Option<ResolveFn>,
}

impl MoveData {
    // we could just use zip
    pub fn merge(self, other: Self) -> Self {
        //let Self { item, container } = self;
        if let (Some(drag), Some(other)) = (self.drag.as_ref(), other.drag.as_ref()) {
            tracing::error!(
                "multiple items! ({:?} and {:?})",
                drag.item.id,
                other.item.id
            )
        }
        if let (Some((c, _)), Some((other, _))) = (self.target.as_ref(), other.target.as_ref()) {
            tracing::error!("multiple containers! ({:?} and {:?})", c, other)
        }
        Self {
            drag: self.drag.or(other.drag),
            target: self.target.or(other.target),
            add_fn: self.add_fn.or(other.add_fn),
        }
    }

    pub fn map_slots<F>(self, f: F) -> Self
    where
        F: Fn(usize) -> usize,
    {
        let Self {
            drag,
            target,
            add_fn,
        } = self;
        Self {
            drag: drag.map(|mut drag| {
                drag.container.1 = f(drag.container.1);
                drag
            }),
            target: target.map(|mut t| {
                t.1 = f(t.1);
                t
            }),
            add_fn,
        }
    }

    pub fn zip(&self) -> Option<(&DragItem, &ContainerData)> {
        self.drag.as_ref().zip(self.target.as_ref())
    }

    pub fn resolve(mut self, ctx: &egui::Context) {
        match (self.drag.take(), self.target.take()) {
            (Some(mut drag), Some(target)) => {
                if let Some(mut f) = drag.remove_fn.take() {
                    f(ctx, &drag, target)
                }
                if let Some(mut f) = self.add_fn.take() {
                    f(ctx, &drag, target)
                }
            }
            _ => tracing::warn!("resolve failed"),
        }
    }
}

pub struct ContainerSpace;

impl ContainerSpace {
    // Not a widget since it doesn't return a Response, but we can use
    // ui.scope just to get a response.
    pub fn show(
        drag_item: &mut Option<DragItem>,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&Option<DragItem>, &mut egui::Ui) -> MoveData,
    ) -> Option<MoveData> {
        // do something w/ inner state, i.e. move items
        let mut data = add_contents(drag_item, ui);

        if let Some(drag) = data.drag.take() {
            // assert!(drag_item.is_none());
            //*drag_item = Some(item);
            assert!(drag_item.replace(drag).is_none());
        }

        // Rotate the dragged item.
        if ui.input().key_pressed(egui::Key::R) {
            if let Some(DragItem { item, .. }) = drag_item.as_mut() {
                item.rotation = item.rotation.increment();
                item.shape = item.shape.rotate90();
            }
        }

        // If the pointer is released, take drag_item.
        ui.input()
            .pointer
            .any_released()
            // If we have both a dragged item and a target, put the
            // item back into the move data and return it.
            .then(|| match (drag_item.take(), data.target.is_some()) {
                (Some(drag), true) => {
                    assert!(data.drag.replace(drag).is_none());
                    Some(data)
                }
                _ => None,
            })
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

/// A widget to display the contents of a container.
pub trait Contents {
    fn id(&self) -> usize;

    /// Returns an egui id based on the contents id. Unused, except
    /// for loading state.
    fn eid(&self) -> egui::Id {
        // Containers are items, so we need a unique id for the contents.
        egui::Id::new("contents").with(self.id())
    }

    fn boxed(self) -> Box<dyn Contents>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }

    /// Number of slots this container holds.
    fn len(&self) -> usize;

    /// Creates a thunk that is resolved after a move when an item is
    /// added. The contents won't exist after a move so we use this to
    /// update internal state in lieu of a normal trait method. `slot`
    /// is used for sectioned contents only.
    fn add(&self, _slot: usize) -> Option<ResolveFn> {
        None
    }

    /// Returns a thunk that is resolved after a move when an item is removed.
    fn remove(&self, _slot: usize) -> Option<ResolveFn> {
        None
    }

    /// Returns a position for a given slot relative to the contents' origin.
    fn pos(&self, slot: usize) -> egui::Vec2;

    /// Returns a container slot for a given offset. May return
    /// invalid results if the offset is outside the container.
    fn slot(&self, offset: egui::Vec2) -> usize;

    fn accepts(&self, item: &Item, slot: usize) -> bool;

    // What about fits anywhere/any slot?
    fn fits(&self, _ctx: &egui::Context, _item: &DragItem, _slot: usize) -> bool;

    // Draw contents.
    fn body(
        &mut self,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>>;

    // Default impl should handle everything including grid/sectioned/expanding containers.
    fn ui(
        &mut self,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData> {
        let margin = egui::Vec2::splat(4.0);
        let outer_rect_bounds = ui.available_rect_before_wrap();
        let inner_rect = outer_rect_bounds.shrink2(margin);
        // reserve a shape for the background so it draws first
        let bg = ui.painter().add(egui::Shape::Noop);
        let mut content_ui = ui.child_ui(inner_rect, *ui.layout());

        let egui::InnerResponse { inner, response } = self.body(drag_item, &mut content_ui);

        // tarkov also checks if containers are full, even if not
        // hovering -- maybe track min size free?
        let dragging = drag_item.is_some();

        let r = content_ui.min_rect();
        let slot = response
            .hover_pos()
            // the hover includes the outer_rect?
            .filter(|p| r.contains(*p))
            .map(|p| self.slot(p - r.min));

        let accepts = drag_item
            .as_ref()
            .zip(slot)
            // `accepts` takes a slot for sectioned contents.
            .map(|(drag, slot)| self.accepts(&drag.item, slot))
            .unwrap_or_default();

        let fits = drag_item
            .as_ref()
            .zip(slot)
            .map(|(item, slot)| self.fits(ui.ctx(), item, slot))
            .unwrap_or_default();

        assert!(match drag_item {
            Some(drag) => ui.memory().is_being_dragged(drag.item.eid()),
            _ => true, // we could be dragging something else
        });

        if let Some(drag) = drag_item {
            if let Some(slot) = slot {
                let color = if fits {
                    egui::color::Color32::GREEN
                } else {
                    egui::color::Color32::RED
                };
                let color = egui::color::tint_color_towards(color, ui.visuals().window_fill());

                // paint item slots, need to reserve shapes so this
                // draws w/ the background
                paint_shape(&drag.item.shape, r, self.pos(slot), color, ui);
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

        // Only send target on release?
        let released = ui.input().pointer.any_released();
        if released && fits && !accepts {
            tracing::info!(
                "container {:?} does not accept item {:?}!",
                self.id(),
                drag_item.as_ref().map(|drag| drag.item.flags)
            );
        }

        // accepts ⇒ dragging, fits ⇒ dragging, fits ⇒ slot

        InnerResponse::new(
            MoveData {
                drag: inner,
                target: (dragging && accepts && fits).then(|| (self.id(), slot.unwrap())),
                add_fn: (accepts && fits).then(|| self.add(slot.unwrap())).flatten(),
            },
            response,
        )
    }
}

#[derive(Debug)]
pub struct GridContents<I> {
    // This shares w/ items, but the eid is unique.
    pub id: usize,
    pub size: shape::Vec2,
    pub items: Option<I>,
    pub flags: FlagSet<ItemFlags>,
}

impl<'a, I> GridContents<I>
where
    I: Iterator<Item = &'a (usize, Item)>,
{
    pub fn new<J>(id: usize, size: impl Into<shape::Vec2>, items: Option<J>) -> Self
    where
        J: IntoIterator<IntoIter = I>,
    {
        Self {
            id,
            size: size.into(),
            items: items.map(|items| items.into_iter()),
            flags: Default::default(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<FlagSet<ItemFlags>>) -> Self {
        self.flags = flags.into();
        self
    }
}

// What is this for?
impl<I> Clone for GridContents<I> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            size: self.size,
            items: None,
            flags: self.flags,
        }
    }
}

pub fn xy(slot: usize, width: usize) -> egui::Vec2 {
    egui::Vec2::new((slot % width) as f32, (slot / width) as f32)
}

fn update_state<T: 'static + Clone + Send + Sync>(
    ctx: &egui::Context,
    id: egui::Id,
    mut f: impl FnMut(T) -> T,
) {
    let t = ctx.data().get_temp::<T>(id);
    if let Some(t) = t {
        ctx.data().insert_temp(id, f(t));
    }
}

// There is no get_temp_mut... If the shape doesn't exist we don't
// care since it will be regenerated next time the container is shown.
fn add_shape(ctx: &egui::Context, id: egui::Id, slot: usize, shape: &shape::Shape) {
    update_state(ctx, id, |mut fill: shape::Shape| {
        fill.paint(shape, slot);
        fill
    })
}

fn remove_shape(ctx: &egui::Context, id: egui::Id, slot: usize, shape: &shape::Shape) {
    update_state(ctx, id, |mut fill: shape::Shape| {
        fill.unpaint(shape, slot);
        fill
    })
}

impl<'a, I> Contents for GridContents<I>
where
    I: Iterator<Item = &'a (usize, Item)>,
{
    fn id(&self) -> usize {
        self.id
    }

    fn len(&self) -> usize {
        self.size.len()
    }

    fn add(&self, _slot: usize) -> Option<ResolveFn> {
        let eid = self.eid();
        Some(Box::new(move |ctx, drag, (_c, slot)| {
            add_shape(ctx, eid, slot, &drag.item.shape())
        }))
    }

    fn remove(&self, _slot: usize) -> Option<ResolveFn> {
        let eid = self.eid();
        Some(Box::new(move |ctx, drag, _| {
            remove_shape(ctx, eid, drag.container.1, &drag.item.shape())
        }))
    }

    fn pos(&self, slot: usize) -> egui::Vec2 {
        xy(slot, self.size.x as usize) * ITEM_SIZE
    }

    fn slot(&self, p: egui::Vec2) -> usize {
        let p = p / ITEM_SIZE;
        p.x as usize + p.y as usize * self.size.x as usize
    }

    fn accepts(&self, item: &Item, _slot: usize) -> bool {
        self.flags.contains(item.flags)
    }

    fn fits(&self, ctx: &egui::Context, drag: &DragItem, slot: usize) -> bool {
        // Must be careful with the type inference here since it will
        // never fetch anything if it thinks it's a reference.
        match ctx.data().get_temp(self.eid()) {
            Some(shape) => {
                // Check if the shape fits here. When moving within
                // one container, use the cached shape with the
                // dragged item (and original rotation) unpainted.
                let shape = match (drag.container.0 == self.id(), &drag.cshape) {
                    // (true, None) should never happen...
                    (true, Some(shape)) => shape,
                    _ => &shape,
                };

                shape.fits(&drag.item.shape, slot)
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
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>> {
        // allocate the full container size
        let (rect, response) = ui.allocate_exact_size(
            egui::Vec2::from(self.size) * ITEM_SIZE,
            egui::Sense::hover(),
        );

        let new_drag = if ui.is_rect_visible(rect) {
            // Skip this if the container is empty? Only if dragging into
            // this container? Only if visible? What if we are dragging to
            // a container w/o the contents visible/open? Is it possible
            // to have an empty shape without a bitvec allocated until
            // painted?  [`fits`] also checks the boundaries even if the
            // container is empty...
            let mut fill = false;
            let eid = self.eid();
            let mut shape = ui.data().get_temp(eid).unwrap_or_else(|| {
                // We don't need to fill if we aren't dragging currently...
                fill = true;
                shape::Shape::new(self.size, false)
            });

            // TODO make debug option
            if !fill {
                // Use the cached shape if the dragged item is ours. This
                // rehashes what's in `fits`.
                let shape = drag_item
                    .as_ref()
                    .filter(|drag| self.id == drag.container.0)
                    .and_then(|drag| drag.cshape.as_ref())
                    .unwrap_or(&shape);

                // debug paint the container "shape" (filled slots)
                paint_shape(
                    shape,
                    ui.min_rect(),
                    egui::Vec2::ZERO,
                    egui::color::Color32::DARK_BLUE,
                    ui,
                );
            }

            let item_size = item_size();

            // I should be movable? Make it an option? We need to be
            // able to call self methods in the adapters. I should be
            // Iterator<Item = (usize, &Item)? FIX:
            let items = self.items.take();

            let new_drag = items
                .map(|items|
                // Paint each item and fill our shape if needed.
                items.map(|(slot, item)| {
                    if fill {
                        shape.paint(&item.shape, *slot);
                    }

                    let item_rect =
                        egui::Rect::from_min_size(ui.min_rect().min + self.pos(*slot), item_size);
                    // item returns a clone if it's being dragged
                    ui.allocate_ui_at_rect(item_rect, |ui| item.ui(drag_item, ui))
                        .inner
                        .map(|new_drag| (slot, new_drag))
                })
                // Reduce down to one new_drag.
                .reduce(|a, b| {
                    if a.is_some() && b.is_some() {
                        // This will only happen if the items overlap?
                        tracing::error!("multiple drag items! ({:?} and {:?})", &a, &b);
                    }
                    a.or(b)
                })
                .flatten()
                // Add the contents id, current slot and
                // container shape w/ the item unpainted.
                .map(|(slot, item)| {
                    let mut cshape = shape.clone();
                    // We've already cloned the item and we're cloning
                    // the shape again to rotate? Isn't it already rotated?
                    cshape.unpaint(&item.shape(), *slot);
                    //let item_shape = item.shape();
                    DragItem {
                        item,
                        container: (self.id, *slot),
                        cshape: Some(cshape),
                        remove_fn: self.remove(*slot),
                    }
                }))
                .flatten();

            // Write out the new shape.
            if fill {
                ui.data().insert_temp(eid, shape);
            }
            new_drag
        } else {
            None
        };

        InnerResponse::new(new_drag, response)
    }
}

// Maybe this should be a trait instead of requiring flagset. Or maybe
// `Item` itself is a trait that encompasses flags. We only care about
// accepting items and whether or not something is a container. At a
// minimum `Item` should be generic over flags. TODO?
flags! {
    // What about slots?
    pub enum ItemFlags: u32 {
        Weapon,
        Armor,
        Potion,
        TradeGood,
        Container,
    }
}

// Rename "simple item"?
#[derive(Clone, Debug)]
pub struct Item {
    pub id: usize,
    pub rotation: ItemRotation,
    pub shape: shape::Shape,
    pub icon: TextureId,
    pub flags: FlagSet<ItemFlags>,
    pub cflags: FlagSet<ItemFlags>,
    //pub layout: ContentsLayout,
}

// pub fn item(
//     id: usize,
//     icon: TextureId,
//     shape: shape::Shape,
//     drag_item: &Option<DragItem>,
// ) -> impl egui::Widget + '_ {
//     // Widget will never work since we need to return things other
//     // than a response.
//     move |ui: &mut egui::Ui| ui.horizontal(|ui| Item::new(id, icon, shape).ui(drag_item, ui))
// }

impl Item {
    pub fn new(id: usize, icon: TextureId, shape: shape::Shape) -> Self {
        Self {
            id,
            rotation: Default::default(),
            shape,
            icon,
            flags: FlagSet::default(),
            cflags: FlagSet::default(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<FlagSet<ItemFlags>>) -> Self {
        self.flags = flags.into();
        self
    }

    pub fn with_cflags(mut self, cflags: impl Into<FlagSet<ItemFlags>>) -> Self {
        self.cflags = cflags.into();
        self
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
                        .filter(|drag| drag.item.id == self.id)
                        .map_or(self.rotation, |drag| drag.item.rotation)
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
                drag_item.is_none() || drag_item.as_ref().map(|drag| drag.item.id) == Some(self.id)
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
// one item than holds many fixed containers.
#[derive(Clone, Debug)]
pub struct SectionContents<I> {
    pub id: usize,
    pub layout: SectionLayout,
    // This should be inside section layout...?
    pub sections: Vec<shape::Vec2>,
    pub items: Option<I>,
    // Should each section have its own flags?
    pub flags: FlagSet<ItemFlags>,
}

#[derive(Clone, Debug)]
pub enum SectionLayout {
    Grid(usize),
    // Fixed(Vec<(usize, egui::Pos2))
    // Columns?
    // Other(Fn?)
}

impl<'a, I> SectionContents<I>
where
    I: Iterator<Item = &'a (usize, Item)>,
{
    pub fn new<J>(
        id: usize,
        layout: SectionLayout,
        sections: Vec<shape::Vec2>,
        items: Option<J>,
    ) -> Self
    where
        J: IntoIterator<IntoIter = I>,
    {
        Self {
            id,
            layout,
            sections,
            items: items.map(|items| items.into_iter()),
            flags: Default::default(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<FlagSet<ItemFlags>>) -> Self {
        self.flags = flags.into();
        self
    }

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

    fn section_eid(&self, idx: usize) -> egui::Id {
        egui::Id::new(self.eid().with("section").with(idx))
    }
}

impl<'a, I> Contents for SectionContents<I>
where
    I: Iterator<Item = &'a (usize, Item)>,
{
    fn id(&self) -> usize {
        self.id
    }

    fn len(&self) -> usize {
        self.sections.iter().map(|s| s.len()).sum()
    }

    fn add(&self, slot: usize) -> Option<ResolveFn> {
        match self.section_slot(slot) {
            Some((i, slot)) => {
                let seid = self.section_eid(i);
                Some(Box::new(move |ctx, drag, _c| {
                    add_shape(ctx, seid, slot, &drag.item.shape())
                }))
            }
            None => None,
        }
    }

    fn remove(&self, slot: usize) -> Option<ResolveFn> {
        match self.section_slot(slot) {
            Some((i, slot)) => {
                let seid = self.section_eid(i);
                Some(Box::new(move |ctx, drag, _c| {
                    remove_shape(ctx, seid, slot, &drag.item.shape())
                }))
            }
            None => None,
        }
    }

    fn pos(&self, _slot: usize) -> egui::Vec2 {
        todo!()
    }

    fn slot(&self, _offset: egui::Vec2) -> usize {
        todo!()
    }

    // Is this even needed? We'd have to generate sections inside Self::new.
    fn accepts(&self, _item: &Item, _slot: usize) -> bool {
        // if let Some((section, slot)) = self.section_slot(slot) {
        //     self.sections[section].accepts(item, slot)
        // } else {
        //     false
        // }
        unimplemented!()
    }

    // Unused. We can only fit things in sections.
    fn fits(&self, _ctx: &egui::Context, _item: &DragItem, _slot: usize) -> bool {
        false
    }

    fn body(
        &mut self,
        _drag_item: &Option<DragItem>,
        _ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>> {
        unimplemented!()
    }

    fn ui(
        &mut self,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData> {
        // map (slot, item) -> (section, (slot, item))
        let ranges = self.section_ranges().collect_vec();

        // TODO We have to clone the items here to produce the proper
        // iterator type `&(usize, Item)`. Need to figure out a way to
        // be more flexible with the input, probably via trait. If we
        // know the input is sorted there is also probably a way to do
        // this w/o collecting into a hash map.
        let items = self
            .items
            .take()
            .map(|items| {
                items
                    .filter_map(|(slot, item)| {
                        // Find section for each slot.
                        ranges
                            .iter()
                            .enumerate()
                            .find_map(|(section, (start, end))| {
                                (slot < end).then(|| (section, ((slot - start), item.clone())))
                            })
                    })
                    .into_group_map()
            })
            .unwrap_or_default();

        match self.layout {
            SectionLayout::Grid(width) => {
                egui::Grid::new(self.id).num_columns(width).show(ui, |ui| {
                    self.sections
                        .iter()
                        .zip(ranges.iter())
                        .enumerate()
                        .map(|(i, (size, (start, _end)))| {
                            let items = items.get(&i);
                            let data = Section::new(
                                self.section_eid(i),
                                GridContents::new(self.id(), *size, items).with_flags(self.flags),
                            )
                            .ui(drag_item, ui)
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

/// Section wraps GridContents to provide a unique egui::Id from the
/// actual (parent) container.
#[derive(Clone, Debug)]
pub struct Section<C> {
    pub eid: egui::Id,
    // This could be Box<dyn Contents>?
    pub contents: C,
}

impl<C> Section<C> {
    pub fn new(eid: egui::Id, contents: C) -> Self {
        Self { eid, contents }
    }
}

impl<C> Contents for Section<C>
where
    C: Contents,
{
    fn id(&self) -> usize {
        self.contents.id()
    }

    fn eid(&self) -> egui::Id {
        self.eid
    }

    fn len(&self) -> usize {
        self.contents.len()
    }

    fn pos(&self, slot: usize) -> egui::Vec2 {
        self.contents.pos(slot)
    }

    fn slot(&self, offset: egui::Vec2) -> usize {
        self.contents.slot(offset)
    }

    fn accepts(&self, item: &Item, slot: usize) -> bool {
        self.contents.accepts(item, slot)
    }

    fn fits(&self, ctx: &egui::Context, item: &DragItem, slot: usize) -> bool {
        self.contents.fits(ctx, item, slot)
    }

    fn body(
        &mut self,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>> {
        self.contents.body(drag_item, ui)
    }
}

// An expanding container fits only one item but it can be any size up
// to a maximum size. This is useful for equipment slots where only
// one item can go and the size varies.
#[derive(Debug)]
pub struct ExpandingContents<I> {
    pub id: usize,
    pub max_size: shape::Vec2,
    // This won't be valid until body is called.
    pub filled: bool,
    pub items: Option<I>,
    pub flags: FlagSet<ItemFlags>,
}

impl<I> ExpandingContents<I> {
    pub fn new<J>(id: usize, max_size: impl Into<shape::Vec2>, items: Option<J>) -> Self
    where
        J: IntoIterator<IntoIter = I>,
    {
        Self {
            id,
            max_size: max_size.into(),
            filled: false,
            items: items.map(|items| items.into_iter()),
            flags: Default::default(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<FlagSet<ItemFlags>>) -> Self {
        self.flags = flags.into();
        self
    }
}

impl<I> Clone for ExpandingContents<I> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            max_size: self.max_size,
            filled: self.filled,
            items: None,
            flags: self.flags,
        }
    }
}

impl<'a, I> Contents for ExpandingContents<I>
where
    I: Iterator<Item = &'a (usize, Item)>,
{
    fn id(&self) -> usize {
        self.id
    }

    fn len(&self) -> usize {
        1
    }

    // fn add(&self, _slot: usize) {
    //     // Using self.filled...
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

    fn accepts(&self, item: &Item, slot: usize) -> bool {
        assert!(slot == 0);
        self.flags.contains(item.flags)
    }

    // How do we visually show if the item is too big?
    fn fits(&self, _ctx: &egui::Context, drag: &DragItem, slot: usize) -> bool {
        slot == 0 && !self.filled && drag.item.shape.size.le(&self.max_size)
    }

    fn body(
        &mut self,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>> {
        let mut items = self.items.take().into_iter().flatten();
        let item = items.next();
        self.filled = item.is_some();
        // Make sure items is <= 1:
        assert!(items.next().is_none());

        // is_rect_visible?
        let (new_drag, response) = match item {
            Some((slot, item)) => {
                assert!(*slot == 0);
                let InnerResponse { inner, response } =
                    ui.allocate_ui(item.size(), |ui| item.ui(drag_item, ui));
                (
                    inner.map(|item| DragItem {
                        item,
                        container: (self.id(), *slot),
                        cshape: None,
                        remove_fn: None,
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

// An expanding container that contains another container, the
// contents of which are displayed inline when present.
pub struct InlineContents<I, C> {
    pub container: ExpandingContents<I>,
    pub contents: Option<C>,
}

impl<'a, I, C: Contents> InlineContents<I, C>
where
    I: Iterator<Item = &'a (usize, Item)>,
{
    pub fn new(container: ExpandingContents<I>, contents: Option<C>) -> Self {
        Self {
            container,
            contents,
        }
    }

    pub fn ui(
        self,
        drag_item: &Option<DragItem>,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData> {
        ui.horizontal(|ui| {
            let Self {
                mut container,
                contents,
            } = self;

            let data = container.ui(drag_item, ui).inner;

            if let Some(mut contents) = contents {
                data.merge(contents.ui(drag_item, ui).inner)
            } else {
                data
            }
        })
    }
}

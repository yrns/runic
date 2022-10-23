use ambassador::{delegatable_trait, Delegate};
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

/// Target container id, slot, and egui::Id (which is unique to sections).
pub type ContainerData = (ContainerId, usize, egui::Id);

pub type ResolveFn = Box<dyn FnMut(&egui::Context, &DragItem, ContainerData)>;

pub struct DragItem {
    /// A clone of the original item with rotation applied.
    pub item: Item,
    /// Source location.
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
        if let (Some((c, _, _)), Some((other, _, _))) =
            (self.target.as_ref(), other.target.as_ref())
        {
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

        // Toggle debug.
        if ui.input().key_pressed(egui::Key::D) {
            ui.ctx().set_debug_on_hover(!ui.ctx().debug_on_hover());
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

pub type Context = (usize, egui::Id);

pub trait IntoContext {
    fn into_ctx(self) -> Context;
}

impl IntoContext for usize {
    fn into_ctx(self) -> Context {
        (self, egui::Id::new("contents").with(self))
    }
}

impl IntoContext for Context {
    fn into_ctx(self) -> Context {
        self
    }
}

/// A widget to display the contents of a container.
#[delegatable_trait]
pub trait Contents {
    /// Returns an egui id based on the contents id. Unused, except
    /// for loading state.
    // fn eid(&self, id: usize) -> egui::Id {
    //     // Containers are items, so we need a unique id for the contents.
    //     egui::Id::new("contents").with(id)
    // }

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
    fn add(&self, _ctx: Context, _slot: usize) -> Option<ResolveFn> {
        None
    }

    /// Returns a thunk that is resolved after a move when an item is removed.
    fn remove(&self, _ctx: Context, _slot: usize) -> Option<ResolveFn> {
        None
    }

    /// Returns a position for a given slot relative to the contents' origin.
    fn pos(&self, slot: usize) -> egui::Vec2;

    /// Returns a container slot for a given offset. May return
    /// invalid results if the offset is outside the container.
    fn slot(&self, offset: egui::Vec2) -> usize;

    fn accepts(&self, item: &Item, slot: usize) -> bool;

    // What about fits anywhere/any slot?
    fn fits(&self, ctx: Context, egui_ctx: &egui::Context, item: &DragItem, slot: usize) -> bool;

    // Draw contents.
    fn body<'a, I>(
        &self,
        ctx: Context,
        drag_item: &Option<DragItem>,
        items: I,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>>
    where
        I: Iterator<Item = (usize, &'a Item)>,
        Self: Sized;

    // Default impl should handle everything including
    // grid/sectioned/expanding containers. Iterator type changed to
    // (usize, &Item) so section contents can rewrite slots.
    fn ui<'a, I, Q>(
        &self,
        ctx: Context,
        _q: &'a Q,
        drag_item: &Option<DragItem>,
        // This used to be an option but we're generally starting with
        // show_contents at the root which implies items. (You can't
        // have items w/o a layout or vice-versa).
        items: I,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData>
    where
        I: IntoIterator<Item = (usize, &'a Item)>,
        Q: ContentsQuery<'a>,
        Self: Sized,
    {
        let margin = egui::Vec2::splat(4.0);
        let outer_rect_bounds = ui.available_rect_before_wrap();
        let inner_rect = outer_rect_bounds.shrink2(margin);
        // reserve a shape for the background so it draws first
        let bg = ui.painter().add(egui::Shape::Noop);
        let mut content_ui = ui.child_ui(inner_rect, *ui.layout());

        let items = items.into_iter();
        let egui::InnerResponse { inner, response } =
            self.body(ctx, drag_item, items, &mut content_ui);

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

        let (id, eid) = ctx;

        let fits = drag_item
            .as_ref()
            .zip(slot)
            .map(|(item, slot)| self.fits(ctx, ui.ctx(), item, slot))
            .unwrap_or_default();

        assert!(match drag_item {
            Some(drag) => ui.memory().is_being_dragged(drag.item.eid()),
            _ => true, // we could be dragging something else
        });

        if let Some(drag) = drag_item {
            if let Some(slot) = slot {
                let color = if !accepts {
                    egui::color::Color32::GRAY
                } else if fits {
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
                id,
                drag_item.as_ref().map(|drag| drag.item.flags)
            );
        }

        // accepts ⇒ dragging, fits ⇒ dragging, fits ⇒ slot

        InnerResponse::new(
            MoveData {
                drag: inner,
                // The target eid is unused..?
                target: (dragging && accepts && fits).then(|| (id, slot.unwrap(), eid)),
                add_fn: (accepts && fits)
                    .then(|| self.add(ctx, slot.unwrap()))
                    .flatten(),
            },
            response,
        )
    }
}

#[derive(Clone, Debug)]
pub struct GridContents {
    pub size: shape::Vec2,
    pub flags: FlagSet<ItemFlags>,
}

impl GridContents {
    pub fn new(size: impl Into<shape::Vec2>) -> Self {
        Self {
            size: size.into(),
            flags: Default::default(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<FlagSet<ItemFlags>>) -> Self {
        self.flags = flags.into();
        self
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

impl Contents for GridContents {
    fn len(&self) -> usize {
        self.size.len()
    }

    // ctx and target are the same...
    fn add(&self, _ctx: Context, _slot: usize) -> Option<ResolveFn> {
        Some(Box::new(move |ctx, drag, (_c, slot, eid)| {
            add_shape(ctx, eid, slot, &drag.item.shape())
        }))
    }

    fn remove(&self, (_id, eid): Context, slot: usize) -> Option<ResolveFn> {
        Some(Box::new(move |ctx, drag, _target| {
            //remove_shape(ctx, eid, drag.container.1, &drag.item.shape())
            remove_shape(ctx, eid, slot, &drag.item.shape())
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

    fn fits(&self, (_id, eid): Context, ctx: &egui::Context, drag: &DragItem, slot: usize) -> bool {
        // Must be careful with the type inference here since it will
        // never fetch anything if it thinks it's a reference.
        match ctx.data().get_temp(eid) {
            Some(shape) => {
                // Check if the shape fits here. When moving within
                // one container, use the cached shape with the
                // dragged item (and original rotation) unpainted.
                let shape = match (drag.container.2 == eid, &drag.cshape) {
                    // (true, None) should never happen...
                    (true, Some(shape)) => shape,
                    _ => &shape,
                };

                shape.fits(&drag.item.shape, slot)
            }
            None => {
                // TODO remove this
                tracing::error!("shape {:?} not found!", eid);
                false
            }
        }
    }

    fn body<'a, I>(
        &self,
        ctx: Context,
        drag_item: &Option<DragItem>,
        items: I,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>>
    where
        I: Iterator<Item = (usize, &'a Item)>,
    {
        // allocate the full container size
        let (rect, response) = ui.allocate_exact_size(
            egui::Vec2::from(self.size) * ITEM_SIZE,
            egui::Sense::hover(),
        );

        let (id, eid) = ctx;

        let new_drag = if ui.is_rect_visible(rect) {
            // Skip this if the container is empty? Only if dragging into
            // this container? Only if visible? What if we are dragging to
            // a container w/o the contents visible/open? Is it possible
            // to have an empty shape without a bitvec allocated until
            // painted?  [`fits`] also checks the boundaries even if the
            // container is empty...
            let mut fill = false;
            let mut shape = ui.data().get_temp(eid).unwrap_or_else(|| {
                // We don't need to fill if we aren't dragging currently...
                fill = true;
                shape::Shape::new(self.size, false)
            });

            if ui.ctx().debug_on_hover() {
                if !fill {
                    // Use the cached shape if the dragged item is ours. This
                    // rehashes what's in `fits`.
                    let shape = drag_item
                        .as_ref()
                        .filter(|drag| eid == drag.container.2)
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
            }

            let item_size = item_size();

            let new_drag = items
                .map(|(slot, item)| {
                    // Paint each item and fill our shape if needed.
                    if fill {
                        shape.paint(&item.shape, slot);
                    }

                    let item_rect =
                        egui::Rect::from_min_size(ui.min_rect().min + self.pos(slot), item_size);
                    // item returns a clone if it's being dragged
                    ui.allocate_ui_at_rect(item_rect, |ui| item.ui(drag_item, ui))
                        .inner
                        .map(|new_drag| (slot, new_drag))
                })
                // Reduce down to one new_drag.
                .reduce(|a, b| {
                    if a.as_ref().and(b.as_ref()).is_some() {
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
                    cshape.unpaint(&item.shape(), slot);
                    //let item_shape = item.shape();
                    DragItem {
                        item,
                        container: (id, slot, eid),
                        cshape: Some(cshape),
                        remove_fn: self.remove(ctx, slot),
                    }
                });

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

/// ContentsQuery allows Contents impls to recursively query the
/// contents of subcontents (InlineContents specifically). This allows
/// SectionContents to use InlineContents as sections, for example.
pub trait ContentsQuery<'a> {
    type Items: IntoIterator<Item = (usize, &'a Item)>;

    fn query(&'a self, id: usize) -> Option<(&'a ContentsLayout, Self::Items)>;
}

// This gets around having to manually specify the iterator type when
// implementing ContentsQuery. Maybe just get rid of the trait?
impl<'a, F, I> ContentsQuery<'a> for F
where
    F: Fn(usize) -> Option<(&'a ContentsLayout, I)>,
    I: Iterator<Item = (usize, &'a Item)> + 'a,
{
    type Items = I;

    fn query(&'a self, id: usize) -> Option<(&'a ContentsLayout, Self::Items)> {
        self(id)
    }
}

// The contents id is not relevant to the layout, just like items,
// which we removed. In particular, the ids of sections are always the
// parent container id. Maybe split Contents into two elements?
#[derive(Clone, Debug, Delegate)]
#[delegate(Contents)]
pub enum ContentsLayout {
    Expanding(ExpandingContents),
    Inline(InlineContents),
    Grid(GridContents),
    Section(SectionContents),
}

// Use ContentsQuery to query a layout and contents, then show it.
pub fn show_contents<'a, Q>(
    q: &'a Q,
    id: usize,
    drag_item: &Option<DragItem>,
    ui: &mut egui::Ui,
) -> Option<egui::InnerResponse<MoveData>>
where
    Q: ContentsQuery<'a>,
{
    q.query(id)
        .map(|(layout, items)| layout.ui(id.into_ctx(), q, drag_item, items, ui))
}

impl From<ExpandingContents> for ContentsLayout {
    fn from(c: ExpandingContents) -> Self {
        Self::Expanding(c)
    }
}

impl From<InlineContents> for ContentsLayout {
    fn from(c: InlineContents) -> Self {
        Self::Inline(c)
    }
}

impl From<GridContents> for ContentsLayout {
    fn from(c: GridContents) -> Self {
        Self::Grid(c)
    }
}

impl From<SectionContents> for ContentsLayout {
    fn from(c: SectionContents) -> Self {
        Self::Section(c)
    }
}

#[derive(Clone, Debug)]
pub struct Item {
    pub id: usize,
    pub rotation: ItemRotation,
    pub shape: shape::Shape,
    pub icon: TextureId,
    pub flags: FlagSet<ItemFlags>,
    pub name: String, // WidgetText?
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
            name: Default::default(),
        }
    }

    pub fn with_id(mut self, id: usize) -> Self {
        self.id = id;
        self
    }

    pub fn with_flags(mut self, flags: impl Into<FlagSet<ItemFlags>>) -> Self {
        self.flags = flags.into();
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
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

        // Scale down slightly when dragged to see the background.
        let dragging = drag_item
            .as_ref()
            .map(|d| d.item.id == self.id)
            .unwrap_or_default();
        let size = self.size() * if dragging { 0.8 } else { 1.0 };

        let image = egui::Image::new(self.icon, size).rotate(
            drag_item
                .as_ref()
                .filter(|drag| drag.item.id == self.id)
                .map_or(self.rotation, |drag| drag.item.rotation)
                .angle(),
            egui::Vec2::splat(0.5),
        );
        // let image = if dragging {
        //     image.tint(egui::Rgba::from_rgba_premultiplied(1.0, 1.0, 1.0, 0.5))
        // } else {
        //     image
        // };
        ui.add(image).on_hover_text(format!("{}", self))
    }

    pub fn ui(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> Option<Item> {
        let id = self.eid();
        let drag = ui.memory().is_being_dragged(id);
        if !drag {
            // This does not work.
            // ui.push_id(self.id, |ui| self.body(drag_item, ui))
            //     .response
            //     .interact(egui::Sense::drag())
            //     .on_hover_cursor(egui::CursorIcon::Grab);

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

            // pos - pos = vec
            // pos + pos = error
            // pos +/- vec = pos
            // vec +/- pos = error

            // The cursor is placing the first slot (upper left) when
            // dragging, so draw the dragged item in roughly the same place.
            if let Some(p) = ui.ctx().pointer_interact_pos() {
                let delta = (p - response.rect.min) - item_size() * 0.25;
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

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)?;
        f.write_str(" ")?;
        f.debug_list().entries(self.flags.into_iter()).finish()
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

// This might be useful to get generic over item iterators, but it
// would require adding another parameter to `Contents::ui`.
pub trait SlotItem {
    fn slot_item(&self) -> (usize, &Item);
}

impl SlotItem for (usize, Item) {
    fn slot_item(&self) -> (usize, &Item) {
        (self.0, &self.1)
    }
}

impl SlotItem for (usize, &Item) {
    fn slot_item(&self) -> (usize, &Item) {
        *self
    }
}

// A sectioned container is a set of smaller containers displayed as
// one. Like pouches on a belt or different pockets in a jacket. It's
// one item than holds many fixed containers.
#[derive(Clone, Debug)]
pub struct SectionContents {
    pub layout: SectionLayout,
    // This should be generic over Contents but then ContentsLayout
    // will cycle on itself.
    pub sections: Vec<ContentsLayout>,
}

#[derive(Clone, Debug)]
pub enum SectionLayout {
    Grid(usize),
    // Fixed(Vec<(usize, egui::Pos2))
    // Columns?
    // Other(Fn?)
}

impl SectionContents {
    pub fn new(layout: SectionLayout, sections: Vec<ContentsLayout>) -> Self {
        Self { layout, sections }
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

    fn section_eid(&self, (_id, eid): Context, sid: usize) -> egui::Id {
        egui::Id::new(eid.with("section").with(sid))
    }
}

impl Contents for SectionContents {
    fn len(&self) -> usize {
        self.sections.iter().map(|s| s.len()).sum()
    }

    fn add(&self, (_id, eid): Context, slot: usize) -> Option<ResolveFn> {
        match self.section_slot(slot) {
            Some((_i, slot)) => Some(Box::new(move |ctx, drag, _target| {
                add_shape(ctx, eid, slot, &drag.item.shape())
            })),
            None => None,
        }
    }

    fn remove(&self, (_id, eid): Context, slot: usize) -> Option<ResolveFn> {
        match self.section_slot(slot) {
            Some((_i, slot)) => Some(Box::new(move |ctx, drag, _target| {
                remove_shape(ctx, eid, slot, &drag.item.shape())
            })),
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
    fn fits(
        &self,
        _ctx: Context,
        _egui_ctx: &egui::Context,
        _item: &DragItem,
        _slot: usize,
    ) -> bool {
        false
    }

    fn body<'a, I>(
        &self,
        _ctx: Context,
        _drag_item: &Option<DragItem>,
        _items: I,
        _ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>>
    where
        I: Iterator<Item = (usize, &'a Item)>,
    {
        unimplemented!()
    }

    fn ui<'a, I, Q>(
        &self,
        ctx: Context,
        q: &'a Q,
        drag_item: &Option<DragItem>,
        items: I,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData>
    where
        I: IntoIterator<Item = (usize, &'a Item)>,
        Q: ContentsQuery<'a>,
        Self: Sized,
    {
        // map (slot, item) -> (section, (slot, item))
        let ranges = self.section_ranges().collect_vec();

        // TODO We have to clone the items here to produce the proper
        // iterator type `&(usize, Item)`. Need to figure out a way to
        // be more flexible with the input, probably via trait. If we
        // know the input is sorted there is also probably a way to do
        // this w/o collecting into a hash map.
        let mut items = items
            .into_iter()
            // Find section for each item.
            .filter_map(|(slot, item)| {
                ranges
                    .iter()
                    .enumerate()
                    .find_map(|(section, (start, end))| {
                        (slot < *end).then(|| (section, ((slot - start), item)))
                    })
            })
            .into_group_map();

        let sections = self.sections.iter().zip(ranges.iter()).enumerate().map(
            |(i, (layout, (start, _end)))| (i, layout, start, items.remove(&i).unwrap_or_default()),
        );

        let id = ctx.0;

        match self.layout {
            SectionLayout::Grid(width) => {
                egui::Grid::new(id).num_columns(width).show(ui, |ui| {
                    sections
                        .map(|(i, layout, start, items)| {
                            let data = layout
                                .ui((id, self.section_eid(ctx, i)), q, drag_item, items, ui)
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
// #[derive(Clone, Debug, Delegate)]
// #[delegate(Contents, target = "contents")]
// pub struct Section<C> {
//     pub eid: egui::Id,
//     pub contents: C,
// }

// impl<C> Section<C> {
//     pub fn new(eid: egui::Id, contents: C) -> Self {
//         Self { eid, contents }
//     }

//     // This overrides the delegate.
//     pub fn eid(&self) -> egui::Id {
//         dbg!("section eid", self.eid);
//         self.eid
//     }
// }

// An expanding container fits only one item but it can be any size up
// to a maximum size. This is useful for equipment slots where only
// one item can go and the size varies.
#[derive(Clone, Debug)]
pub struct ExpandingContents {
    pub max_size: shape::Vec2,
    // This won't be valid until body is called.
    //pub filled: bool,
    pub flags: FlagSet<ItemFlags>,
}

impl ExpandingContents {
    pub fn new(max_size: impl Into<shape::Vec2>) -> Self {
        Self {
            max_size: max_size.into(),
            //filled: false,
            flags: Default::default(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<FlagSet<ItemFlags>>) -> Self {
        self.flags = flags.into();
        self
    }
}

impl Contents for ExpandingContents {
    fn len(&self) -> usize {
        1
    }

    // We don't need these since it's reset in body and only used after...

    // fn add(&self, _slot: usize) {
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
    fn fits(&self, (_id, eid): Context, ctx: &egui::Context, drag: &DragItem, slot: usize) -> bool {
        let filled: bool = ctx.data().get_temp(eid).unwrap_or_default();
        slot == 0 && !filled && drag.item.shape.size.le(&self.max_size)
    }

    fn body<'a, I>(
        &self,
        (id, eid): Context,
        drag_item: &Option<DragItem>,
        mut items: I,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>>
    where
        I: Iterator<Item = (usize, &'a Item)>,
    {
        let item = items.next();

        ui.ctx().data().insert_temp(eid, item.is_some());

        assert!(items.next().is_none());

        // is_rect_visible?
        let (new_drag, response) = match item {
            Some((slot, item)) => {
                assert!(slot == 0);
                let InnerResponse { inner, response } =
                    ui.allocate_ui(item.size(), |ui| item.ui(drag_item, ui));
                (
                    inner.map(|item| DragItem {
                        item,
                        container: (id, slot, eid),
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
        todo!() // 1?
    }

    fn pos(&self, _slot: usize) -> egui::Vec2 {
        todo!()
    }

    fn slot(&self, _offset: egui::Vec2) -> usize {
        todo!()
    }

    fn accepts(&self, _item: &Item, _slot: usize) -> bool {
        todo!()
    }

    fn fits(
        &self,
        _ctx: Context,
        _egui_ctx: &egui::Context,
        _item: &DragItem,
        _slot: usize,
    ) -> bool {
        todo!()
    }

    fn body<'a, I>(
        &self,
        _ctx: Context,
        _drag_item: &Option<DragItem>,
        _items: I,
        _ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<DragItem>>
    where
        I: Iterator<Item = (usize, &'a Item)>,
        Self: Sized,
    {
        unimplemented!()
    }

    fn ui<'a, I, Q>(
        &self,
        ctx: Context,
        q: &'a Q,
        drag_item: &Option<DragItem>,
        items: I,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData>
    where
        I: IntoIterator<Item = (usize, &'a Item)>,
        Q: ContentsQuery<'a>,
    {
        // get the layout and contents of the contained item (if any)
        let mut items = items.into_iter().peekable();
        let inline_id = items.peek().map(|(_, item)| item.id);

        // TODO: InlineLayout?
        ui.horizontal(|ui| {
            let data = self.0.ui(ctx, q, drag_item, items, ui).inner;

            // Don't add contents if the container is being dragged?

            match inline_id.and_then(|id| show_contents(q, id, drag_item, ui)) {
                Some(resp) => data.merge(resp.inner),
                None => data,
            }
        })
    }
}

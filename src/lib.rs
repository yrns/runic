use ambassador::{delegatable_trait, Delegate};
use egui::{InnerResponse, TextureId};
use flagset::{flags, FlagSet};
use itertools::Itertools;

pub mod shape;

pub const ITEM_SIZE: f32 = 48.0;

// static? rename slot_size?
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

impl std::fmt::Debug for DragItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DragItem {{ item: {:?}, container: {:?} }}",
            self.item, self.container
        )
    }
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

    pub fn map_slots<F>(self, id: usize, f: F) -> Self
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
                if drag.container.0 == id {
                    drag.container.1 = f(drag.container.1);
                }
                drag
            }),
            target: target.map(|mut t| {
                if t.0 == id {
                    t.1 = f(t.1);
                }
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
    idxs: Vec<egui::layers::ShapeIdx>,
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
        // It does not matter if we don't use all the shape indices.
        .zip(idxs.iter())
        .for_each(|(p, idx)| {
            let slot_rect = egui::Rect::from_min_size(p, item_size());
            // ui.painter()
            //     .rect(slot_rect, 0., color, egui::Stroke::none())
            ui.painter()
                .set(*idx, egui::epaint::RectShape::filled(slot_rect, 0., color));
        })
}

// Replaces `paint_shape` and uses only one shape index, so we don't
// have to reserve multiple. There is Shape::Vec, too.
pub fn shape_mesh(
    shape: &shape::Shape,
    grid_rect: egui::Rect,
    offset: egui::Vec2,
    color: egui::Color32,
    //texture_id: egui::TextureId,
    scale: f32,
) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();

    // TODO share vertices in grid
    let offset = grid_rect.min + offset;
    shape
        .slots()
        .map(|slot| offset + xy(slot, shape.width()) * scale)
        // TODO use clip rect instead of remaking vertices every frame
        .filter(|p| grid_rect.contains(*p + egui::vec2(1., 1.)))
        .map(|p| egui::Rect::from_min_size(p, egui::Vec2::splat(scale)))
        .for_each(|rect| {
            mesh.add_colored_rect(rect, color);
        });
    mesh
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

// Add contents inside a styled background with a margin. The style is
// mutable so it can be modified based on the contents.
pub fn with_bg<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::style::WidgetVisuals, &mut egui::Ui) -> R,
) -> InnerResponse<R> {
    let margin = egui::Vec2::splat(4.0);
    let outer_rect_bounds = ui.available_rect_before_wrap();
    let inner_rect = outer_rect_bounds.shrink2(margin);

    // Reserve a shape for the background so it draws first.
    let bg = ui.painter().add(egui::Shape::Noop);
    let mut content_ui = ui.child_ui(inner_rect, *ui.layout());

    // Draw contents.
    let mut style = ui.visuals().widgets.active;
    let inner = add_contents(&mut style, &mut content_ui);

    let outer_rect =
        egui::Rect::from_min_max(outer_rect_bounds.min, content_ui.min_rect().max + margin);
    let (rect, response) = ui.allocate_at_least(outer_rect.size(), egui::Sense::hover());

    ui.painter().set(
        bg,
        egui::epaint::RectShape {
            rounding: style.rounding,
            fill: style.bg_fill,
            stroke: style.bg_stroke,
            rect,
        },
    );

    InnerResponse::new(inner, response)
}

pub fn find_slot_default<'a, C, I>(
    contents: &C,
    ctx: Context,
    egui_ctx: &egui::Context,
    drag: &DragItem,
    _items: I,
) -> Option<(usize, usize, egui::Id)>
where
    C: Contents,
    I: IntoIterator<Item = (usize, &'a Item)>,
{
    // TODO test multiple rotations (if non-square) and return it?
    contents.accepts(&drag.item).then(|| true).and(
        (0..contents.len())
            .find(|slot| contents.fits(ctx, egui_ctx, drag, *slot))
            .map(|slot| (ctx.0, slot, ctx.1)),
    )
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
    /// is used for sectioned contents only. SectionContents needs to
    /// be updated...
    fn add(&self, _ctx: Context, _slot: usize) -> Option<ResolveFn> {
        None
    }

    /// Returns a thunk that is resolved after a move when an item is removed.
    fn remove(&self, _ctx: Context, _slot: usize, _shape: shape::Shape) -> Option<ResolveFn> {
        None
    }

    /// Returns a position for a given slot relative to the contents' origin.
    fn pos(&self, slot: usize) -> egui::Vec2;

    /// Returns a container slot for a given offset. May return
    /// invalid results if the offset is outside the container.
    fn slot(&self, offset: egui::Vec2) -> usize;

    fn accepts(&self, item: &Item) -> bool;

    /// Returns true if the dragged item will fit at the specified slot.
    fn fits(&self, ctx: Context, egui_ctx: &egui::Context, item: &DragItem, slot: usize) -> bool;

    /// Finds the first available slot for the dragged item.
    fn find_slot<'a, I>(
        &self,
        ctx: Context,
        egui_ctx: &egui::Context,
        item: &DragItem,
        items: I,
    ) -> Option<(usize, usize, egui::Id)>
    where
        I: IntoIterator<Item = (usize, &'a Item)>,
        //Q: ContentsQuery<'a>,
        Self: Sized,
    {
        find_slot_default(self, ctx, egui_ctx, item, items)
    }

    fn shadow_color(&self, accepts: bool, fits: bool, ui: &egui::Ui) -> egui::Color32 {
        let color = if !accepts {
            egui::color::Color32::GRAY
        } else if fits {
            egui::color::Color32::GREEN
        } else {
            egui::color::Color32::RED
        };
        egui::color::tint_color_towards(color, ui.visuals().window_fill())
    }

    // Draw contents.
    fn body<'a, I>(
        &self,
        _ctx: Context,
        _drag_item: &Option<DragItem>,
        _items: I,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<ItemResponse>>
    where
        I: Iterator<Item = (usize, &'a Item)>,
        Self: Sized,
    {
        // never used
        InnerResponse::new(None, ui.label("❓"))
    }

    // Default impl should handle everything including
    // grid/sectioned/expanding containers. Iterator type changed to
    // (usize, &Item) so section contents can rewrite slots.
    fn ui<'a, I, Q>(
        &self,
        ctx: Context,
        q: &'a Q,
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
        assert!(match drag_item {
            Some(drag) => ui.memory().is_being_dragged(drag.item.eid()),
            _ => true, // we could be dragging something else
        });

        // We have all the information we need to set the style from
        // the MoveData/drag_item, so we could do that internal to
        // with_bg... ItemResponse::Item would need to be
        // preserved. Also, with_item_shadow()?
        with_bg(ui, |style, mut ui| {
            // The item shadow becomes the target item, not the dragged
            // item, for drag-to-item?

            // Reserve shape for the dragged item's shadow.
            let shadow = ui.painter().add(egui::Shape::Noop);

            let egui::InnerResponse { inner, response } =
                self.body(ctx, drag_item, items.into_iter(), &mut ui);

            let min_rect = ui.min_rect();

            // If we are dragging onto another item, check to see if
            // the dragged item will fit anywhere within its contents.
            match (drag_item, inner.as_ref()) {
                // hover ⇒ dragging
                (Some(drag), Some(ItemResponse::Hover((slot, item)))) => {
                    if let Some((contents, items)) = q.query(item.id) {
                        let ctx = item.id.into_ctx();
                        let target = contents.find_slot(ctx, ui.ctx(), drag, items);

                        // TODO fits && !accepts?
                        let color = self.shadow_color(true, target.is_some(), ui);
                        let mut mesh = egui::Mesh::default();
                        mesh.add_colored_rect(
                            // TODO make sure slot is correct for sections
                            egui::Rect::from_min_size(
                                min_rect.min + self.pos(*slot),
                                item.size_rotated(),
                            ),
                            color,
                        );
                        ui.painter().set(shadow, mesh);

                        return MoveData {
                            drag: None,
                            target, //: (id, slot, eid),
                            add_fn: target.and_then(|t| contents.add(ctx, t.1)),
                        };
                    }
                }
                _ => (),
            }

            // tarkov also checks if containers are full, even if not
            // hovering -- maybe track min size free? TODO just do
            // accepts, and only check fits for hover
            let dragging = drag_item.is_some();

            let slot = response
                .hover_pos()
                // the hover includes the outer_rect?
                .filter(|p| min_rect.contains(*p))
                .map(|p| self.slot(p - min_rect.min));

            let accepts = drag_item
                .as_ref()
                .map(|drag| self.accepts(&drag.item))
                .unwrap_or_default();

            let (id, eid) = ctx;

            let fits = drag_item
                .as_ref()
                .zip(slot)
                .map(|(item, slot)| self.fits(ctx, ui.ctx(), item, slot))
                .unwrap_or_default();

            // Paint the dragged item's shadow, showing which slots will
            // be filled.
            if let Some(drag) = drag_item {
                if let Some(slot) = slot {
                    let color = self.shadow_color(accepts, fits, ui);
                    ui.painter().set(
                        shadow,
                        shape_mesh(&drag.item.shape, min_rect, self.pos(slot), color, ITEM_SIZE),
                    );
                }
            }

            if !(dragging && accepts && response.hovered()) {
                *style = ui.visuals().widgets.inactive;
            };

            if dragging && accepts {
                // gray out:
                style.bg_fill =
                    egui::color::tint_color_towards(style.bg_fill, ui.visuals().window_fill());
                style.bg_stroke.color = egui::color::tint_color_towards(
                    style.bg_stroke.color,
                    ui.visuals().window_fill(),
                );
            }

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

            MoveData {
                drag: match inner {
                    Some(ItemResponse::Drag(drag)) => Some(drag),
                    _ => None,
                },
                // The target eid is unused..? dragging implied...
                target: (dragging && accepts && fits).then(|| (id, slot.unwrap(), eid)),
                add_fn: (accepts && fits)
                    .then(|| self.add(ctx, slot.unwrap()))
                    .flatten(),
            }
        })
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
            add_shape(ctx, eid, slot, &drag.item.shape)
        }))
    }

    fn remove(&self, (_id, eid): Context, slot: usize, shape: shape::Shape) -> Option<ResolveFn> {
        Some(Box::new(move |ctx, _drag, _target| {
            remove_shape(ctx, eid, slot, &shape)
        }))
    }

    fn pos(&self, slot: usize) -> egui::Vec2 {
        xy(slot, self.size.x as usize) * ITEM_SIZE
    }

    fn slot(&self, p: egui::Vec2) -> usize {
        let p = p / ITEM_SIZE;
        p.x as usize + p.y as usize * self.size.x as usize
    }

    fn accepts(&self, item: &Item) -> bool {
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

    fn find_slot<'a, I>(
        &self,
        ctx: Context,
        egui_ctx: &egui::Context,
        item: &DragItem,
        items: I,
    ) -> Option<(usize, usize, egui::Id)>
    where
        I: IntoIterator<Item = (usize, &'a Item)>,
        Self: Sized,
    {
        // Prime the container shape. Normally `body` does this.
        let shape: Option<shape::Shape> = egui_ctx.data().get_temp(ctx.1);
        if shape.is_none() {
            let shape = items.into_iter().fold(
                shape::Shape::new(self.size, false),
                |mut shape, (slot, item)| {
                    shape.paint(&item.shape, slot);
                    shape
                },
            );
            egui_ctx.data().insert_temp(ctx.1, shape);
        }

        // This will reclone the shape every turn of the loop...
        find_slot_default(self, ctx, egui_ctx, item, None)
    }

    fn body<'a, I>(
        &self,
        ctx: Context,
        drag_item: &Option<DragItem>,
        items: I,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<ItemResponse>>
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

                    // debug paint the container "shape" (filled
                    // slots)
                    ui.painter().add(shape_mesh(
                        shape,
                        ui.min_rect(),
                        egui::Vec2::ZERO,
                        egui::color::Color32::DARK_BLUE,
                        ITEM_SIZE,
                    ));
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
                // Reduce down to one new_drag. At some point change
                // the above to find_map.
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
                    match item {
                        ItemResponse::NewDrag(item) => {
                            // The dragged item shape is already rotated. We
                            // clone it to retain the original rotation for
                            // removal.
                            let item_shape = item.shape.clone();
                            let mut cshape = shape.clone();
                            // We've already cloned the item and we're cloning
                            // the shape again to rotate? Isn't it already rotated?
                            cshape.unpaint(&item_shape, slot);
                            ItemResponse::Drag(DragItem {
                                item,
                                // FIX just use ctx?
                                container: (id, slot, eid),
                                cshape: Some(cshape),
                                remove_fn: self.remove(ctx, slot, item_shape),
                            })
                        }
                        // Update the slot.
                        ItemResponse::Hover((_, item)) => ItemResponse::Hover((slot, item)),
                        _ => item,
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

#[derive(Debug)]
pub enum ItemResponse {
    Hover((usize, Item)),
    NewDrag(Item),
    Drag(DragItem),
}

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

    pub fn with_rotation(mut self, r: ItemRotation) -> Self {
        self.rotation = r;
        self
    }

    /// Returns an egui id based on the item id.
    pub fn eid(&self) -> egui::Id {
        egui::Id::new(self.id)
    }

    /// Size of the (unrotated?) item in pixels.
    // TODO check uses of this and make sure the rotation is right
    pub fn size(&self) -> egui::Vec2 {
        egui::Vec2::new(
            self.shape.width() as f32 * ITEM_SIZE,
            self.shape.height() as f32 * ITEM_SIZE,
        )
    }

    // DragItem is already rotated, so this should never be used in
    // that case. It would be nice to enforce this via type.
    pub fn size_rotated(&self) -> egui::Vec2 {
        match (self.size(), self.rotation) {
            (egui::Vec2 { x, y }, ItemRotation::R90 | ItemRotation::R270) => egui::vec2(y, x),
            (size, _) => size,
        }
    }

    // The width of the item with rotation.
    pub fn width(&self) -> usize {
        match self.rotation {
            ItemRotation::R90 | ItemRotation::R270 => self.shape.height(),
            _ => self.shape.width(),
        }
    }

    pub fn body(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> egui::Vec2 {
        // the demo adds a context menu here for removing items
        // check the response id is the item id?
        //ui.add(egui::Label::new(format!("item {}", self.id)).sense(egui::Sense::click()))

        let (dragging, rot) = match drag_item.as_ref() {
            Some(drag) if drag.item.id == self.id => (true, drag.item.rotation),
            _ => (false, self.rotation),
        };

        // let image = if dragging {
        //     image.tint(egui::Rgba::from_rgba_premultiplied(1.0, 1.0, 1.0, 0.5))
        // } else {
        //     image
        // };

        // Allocate the original size so the contents draws
        // consistenly when the dragged item is scaled.
        let size = rot.size(self.size());
        let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());

        // Image::rotate was problematic for non-square images. Rather
        // than rotate the mesh, reassign uvs.
        if ui.is_rect_visible(rect) {
            let mut mesh = egui::Mesh::with_texture(self.icon);
            // Scale down slightly when dragged to see the
            // background. Animate this and the drag-shift to cursor?
            let drag_scale = if dragging { 0.8 } else { 1.0 };

            mesh.add_rect_with_uv(
                egui::Rect::from_min_size(rect.min, size * drag_scale),
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            for (v, uv) in mesh.vertices.iter_mut().zip(rot.uvs().iter()) {
                v.uv = *uv;
            }

            ui.painter().add(egui::Shape::mesh(mesh));
        }

        size
    }

    pub fn ui(&self, drag_item: &Option<DragItem>, ui: &mut egui::Ui) -> Option<ItemResponse> {
        let id = self.eid();
        let drag = ui.memory().is_being_dragged(id);
        if !drag {
            // This does not work.
            // ui.push_id(self.id, |ui| self.body(drag_item, ui))
            //     .response
            //     .interact(egui::Sense::drag())
            //     .on_hover_cursor(egui::CursorIcon::Grab);

            let response = ui.scope(|ui| self.body(drag_item, ui)).response;

            // Figure out what slot we're in, see if it's filled,
            // don't sense drag if not.
            let filled = ui
                .ctx()
                .pointer_interact_pos()
                .filter(|p| response.rect.contains(*p))
                .map(|p| {
                    // This is roughly <GridContents as Contents>::slot?
                    let p = (p - response.rect.min) / ITEM_SIZE;
                    let slot = p.x as usize + p.y as usize * self.width();
                    self.shape.fill.get(slot).map(|b| *b).unwrap_or_else(|| {
                        tracing::error!("point {:?} slot {} out of shape fill", p, slot);
                        false
                    })
                })
                .unwrap_or_default();

            if filled {
                let response = ui.interact(response.rect, id, egui::Sense::drag());
                let response = response.on_hover_text_at_pointer(format!("{}", self));
                if response.hovered() {
                    ui.output().cursor_icon = egui::CursorIcon::Grab;
                }
                drag_item
                    .as_ref()
                    .map(|_| ItemResponse::Hover((0, self.clone())))
            } else {
                None
            }
        } else {
            ui.output().cursor_icon = egui::CursorIcon::Grabbing;

            // pos - pos = vec
            // pos + pos = error
            // pos +/- vec = pos
            // vec +/- pos = error

            // Draw the dragged item in a new area so it does not
            // affect the size of the contents, which could occur with
            // a large item rotated outside the bounds of the contents.
            match ui.ctx().pointer_interact_pos() {
                Some(p) => {
                    // from egui::containers::show_tooltip_area_dyn
                    let _resp = egui::containers::Area::new(id)
                        .order(egui::Order::Tooltip)
                        // The cursor is placing the first slot (upper
                        // left) when dragging, so draw the dragged
                        // item in roughly the same place.
                        .fixed_pos(p - item_size() * 0.25)
                        .interactable(false)
                        // Restrict to ContainerShape?
                        .drag_bounds(egui::Rect::EVERYTHING)
                        .show(ui.ctx(), |ui| self.body(drag_item, ui));

                    // Still allocate the original size for expanding
                    // contents. The response size can be rotated
                    // (since it's being dragged), so use our
                    // (rotated) size.
                    ui.allocate_exact_size(self.rotation.size(self.size()), egui::Sense::hover());
                }
                _ => tracing::error!("no interact position for drag?"),
            }

            // make sure there is no existing drag_item or it matches
            // our id
            assert!(
                drag_item.is_none() || drag_item.as_ref().map(|drag| drag.item.id) == Some(self.id)
            );

            // Only send back a clone if this is a new drag (drag_item
            // is empty):
            drag_item
                .is_none()
                // This clones the shape twice...
                .then(|| ItemResponse::NewDrag(self.clone().rotate()))
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
    pub const R0_UVS: [egui::Pos2; 4] = [
        egui::pos2(0.0, 0.0),
        egui::pos2(1.0, 0.0),
        egui::pos2(0.0, 1.0),
        egui::pos2(1.0, 1.0),
    ];

    pub const R90_UVS: [egui::Pos2; 4] = [
        egui::pos2(0.0, 1.0),
        egui::pos2(0.0, 0.0),
        egui::pos2(1.0, 1.0),
        egui::pos2(1.0, 0.0),
    ];

    pub const R180_UVS: [egui::Pos2; 4] = [
        egui::pos2(1.0, 1.0),
        egui::pos2(0.0, 1.0),
        egui::pos2(1.0, 0.0),
        egui::pos2(0.0, 0.0),
    ];

    pub const R270_UVS: [egui::Pos2; 4] = [
        egui::pos2(1.0, 0.0),
        egui::pos2(1.0, 1.0),
        egui::pos2(0.0, 0.0),
        egui::pos2(0.0, 1.0),
    ];

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

    pub fn rot2(&self) -> egui::emath::Rot2 {
        egui::emath::Rot2::from_angle(self.angle())
    }

    // Move this into item? TODO
    pub fn size(&self, size: egui::Vec2) -> egui::Vec2 {
        match *self {
            ItemRotation::R90 | ItemRotation::R270 => egui::Vec2::new(size.y, size.x),
            _ => size,
        }
    }

    pub fn uvs(&self) -> &[egui::Pos2; 4] {
        match *self {
            ItemRotation::None => &Self::R0_UVS,
            ItemRotation::R90 => &Self::R90_UVS,
            ItemRotation::R180 => &Self::R180_UVS,
            ItemRotation::R270 => &Self::R270_UVS,
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

    // (ctx, slot) -> (section, section ctx, section slot)
    fn section(&self, ctx: Context, slot: usize) -> Option<(&ContentsLayout, Context, usize)> {
        self.section_slot(slot)
            .map(|(i, slot)| (&self.sections[i], (ctx.0, self.section_eid(ctx, i)), slot))
    }

    fn section_items<'a, I>(
        &self,
        items: I,
    ) -> impl Iterator<Item = (usize, ContentsLayout, usize, Vec<(usize, &'a Item)>)>
    where
        I: IntoIterator<Item = (usize, &'a Item)>,
    {
        // map (slot, item) -> (section, (slot, item))
        let ranges = self.section_ranges().collect_vec();

        // If we know the input is sorted there is probably a way to
        // do this w/o collecting into a hash map.
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

        // TODO should be a way to do this without cloning sections
        self.sections
            .clone()
            .into_iter()
            .zip(ranges.into_iter())
            .enumerate()
            .map(move |(i, (layout, (start, _end)))| {
                (i, layout, start, items.remove(&i).unwrap_or_default())
            })
    }
}

// pub struct SectionItems<I> {
//     curr: usize,
//     // keep a ref to section contents or clone sections?
//     items: itertools::GroupingMap<I>,
// }

impl Contents for SectionContents {
    fn len(&self) -> usize {
        self.sections.iter().map(|s| s.len()).sum()
    }

    // Forward to section.
    fn add(&self, ctx: Context, slot: usize) -> Option<ResolveFn> {
        self.section(ctx, slot)
            .and_then(|(a, ctx, slot)| a.add(ctx, slot))
    }

    fn remove(&self, ctx: Context, slot: usize, shape: shape::Shape) -> Option<ResolveFn> {
        self.section(ctx, slot)
            .and_then(|(a, ctx, slot)| a.remove(ctx, slot, shape))
    }

    fn pos(&self, _slot: usize) -> egui::Vec2 {
        todo!()
    }

    fn slot(&self, _offset: egui::Vec2) -> usize {
        todo!()
    }

    /// Returns true if any section can accept this item.
    fn accepts(&self, item: &Item) -> bool {
        self.sections.iter().any(|a| a.accepts(item))
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

    fn find_slot<'a, I>(
        &self,
        ctx: Context,
        egui_ctx: &egui::Context,
        item: &DragItem,
        items: I,
        // id, slot, ...
    ) -> Option<(usize, usize, egui::Id)>
    where
        I: IntoIterator<Item = (usize, &'a Item)>,
        Self: Sized,
    {
        self.section_items(items)
            .find_map(|(i, layout, start, items)| {
                let ctx = (ctx.0, self.section_eid(ctx, i));
                layout
                    .find_slot(ctx, egui_ctx, item, items)
                    .map(|(id, slot, eid)| (id, (slot + start), eid))
            })
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
        let sections = self.section_items(items);

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

                            // Remap slots. Only if we are the subject
                            // of the drag or target. Nested contents
                            // will have a different id.
                            data.map_slots(id, |slot| slot + start)
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
#[derive(Clone, Debug)]
pub struct ExpandingContents {
    pub max_size: shape::Vec2,
    pub flags: FlagSet<ItemFlags>,
}

impl ExpandingContents {
    pub fn new(max_size: impl Into<shape::Vec2>) -> Self {
        Self {
            max_size: max_size.into(),
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

    fn accepts(&self, item: &Item) -> bool {
        self.flags.contains(item.flags)
    }

    // How do we visually show if the item is too big? What if the
    // item is rotated and oblong, and only fits one way?
    fn fits(&self, (_id, eid): Context, ctx: &egui::Context, drag: &DragItem, slot: usize) -> bool {
        // Allow rotating in place.
        let current_item = eid == drag.container.2;
        let filled: bool = !current_item && ctx.data().get_temp(eid).unwrap_or_default();
        slot == 0 && !filled && drag.item.shape.size.le(&self.max_size)
    }

    fn body<'a, I>(
        &self,
        (id, eid): Context,
        drag_item: &Option<DragItem>,
        mut items: I,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<ItemResponse>>
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
                    // item.size() isn't rotated... TODO: test
                    // non-square containers, review item.size() everywhere
                    ui.allocate_ui(item.size(), |ui| item.ui(drag_item, ui));
                (
                    inner.map(|item| match item {
                        ItemResponse::NewDrag(item) => ItemResponse::Drag(DragItem {
                            item,
                            container: (id, slot, eid),
                            cshape: None,
                            remove_fn: None,
                        }),
                        // We don't need to update ItemResponse::Hover(...)
                        // since the default slot is 0.
                        _ => item,
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

#[cfg(test)]
mod tests {
    use super::*;

    // 1234, rotate x times, swap the last two to match the quad uvs:
    fn gen_uvs(r: usize) -> [egui::Pos2; 4] {
        let mut uvs = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        uvs.rotate_right(r); // not const yet
        uvs.swap(2, 3); // not const yet
        uvs.map(|(x, y)| egui::pos2(x, y)) // never?
    }

    #[test]
    fn uvs() {
        assert_eq!(gen_uvs(0), ItemRotation::R0_UVS);
        assert_eq!(gen_uvs(1), ItemRotation::R90_UVS);
        assert_eq!(gen_uvs(2), ItemRotation::R180_UVS);
        assert_eq!(gen_uvs(3), ItemRotation::R270_UVS);
    }
}

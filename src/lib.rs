pub use contents::*;
use egui::{InnerResponse, TextureId};
use flagset::{flags, FlagSet};
use itertools::Itertools;

pub mod contents;
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
    let outer_rect = ui.available_rect_before_wrap();
    let inner_rect = outer_rect.shrink2(margin);

    // Reserve a shape for the background so it draws first.
    let bg = ui.painter().add(egui::Shape::Noop);
    let mut content_ui = ui.child_ui(inner_rect, *ui.layout());

    // Draw contents.
    let mut style = ui.visuals().widgets.active;
    let inner = add_contents(&mut style, &mut content_ui);

    let outer_rect = content_ui.min_rect().expand2(margin);
    let (rect, response) = ui.allocate_exact_size(outer_rect.size(), egui::Sense::hover());

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

#[derive(Clone, Debug)]
pub struct HeaderContents {
    // Box<dyn Fn(...)> ? Not clonable.
    pub header: String,
    pub contents: Box<ContentsLayout>,
}

impl HeaderContents {
    pub fn new(header: impl Into<String>, contents: impl Into<ContentsLayout>) -> Self {
        Self {
            header: header.into(),
            contents: Box::new(contents.into()),
        }
    }
}

impl Contents for HeaderContents {
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
        // Is InnerResponse really useful?
        let InnerResponse { inner, response } = ui.vertical(|ui| {
            ui.label(&self.header);
            self.contents.ui(ctx, q, drag_item, items, ui)
        });
        InnerResponse::new(inner.inner, response)
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

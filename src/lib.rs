mod contents;
mod item;
mod shape;

use std::collections::HashMap;

use egui::InnerResponse;
use flagset::{flags, FlagSet};

pub use contents::*;
pub use item::*;
pub use shape::*;

pub const ITEM_SIZE: f32 = 48.0;

// static? rename slot_size?
pub fn item_size() -> egui::Vec2 {
    egui::vec2(ITEM_SIZE, ITEM_SIZE)
}

pub type ContainerId = usize;

/// Target container id, slot, and egui::Id (which is unique to sections).
pub type ContainerData = (ContainerId, usize, egui::Id);

pub type ResolveFn =
    Box<dyn FnMut(&egui::Context, &DragItem, ContainerData) + Send + Sync + 'static>;

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
        if ui.input(|i| i.key_pressed(egui::Key::R)) {
            if let Some(DragItem { item, .. }) = drag_item.as_mut() {
                item.rotation = item.rotation.increment();
                item.shape = item.shape.rotate90();
            }
        }

        // Toggle debug.
        if ui.input(|i| i.key_pressed(egui::Key::D)) {
            ui.ctx().set_debug_on_hover(!ui.ctx().debug_on_hover());
        }

        // If the pointer is released, take drag_item.
        ui.input(|i| i.pointer.any_released())
            // If we have both a dragged item and a target, put the
            // item back into the move data and return it.
            .then(|| match (drag_item.take(), data.target.is_some()) {
                (Some(drag), true) => {
                    assert!(data.drag.replace(drag).is_none());
                    Some(data)
                }
                (drag_item, target) => {
                    tracing::warn!(?target, drag_item = drag_item.map(|i| i.item.name));
                    None
                }
            })
            .flatten()
    }
}

pub fn xy(slot: usize, width: usize) -> egui::Vec2 {
    egui::Vec2::new((slot % width) as f32, (slot / width) as f32)
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

// Container id, egui id, item slot offset (for sectioned containers).
pub type Context = (usize, egui::Id, usize);

pub trait IntoContext {
    fn into_ctx(self) -> Context;
}

impl IntoContext for usize {
    fn into_ctx(self) -> Context {
        (self, egui::Id::new("contents").with(self), 0)
    }
}

impl IntoContext for Context {
    fn into_ctx(self) -> Context {
        self
    }
}

pub fn find_slot_default<'a, T>(
    contents: &T,
    ctx: Context,
    egui_ctx: &egui::Context,
    drag: &DragItem,
    _items: &[(usize, Item)],
) -> Option<(usize, usize, egui::Id)>
where
    T: Contents + ?Sized,
{
    if !contents.accepts(&drag.item) {
        return None;
    }

    // TODO test multiple rotations (if non-square) and return it?
    (0..contents.len())
        .find(|slot| contents.fits(ctx, egui_ctx, drag, *slot))
        .map(|slot| (ctx.0, slot, ctx.1))
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
// pub trait ContentsQuery<'a, T: Contents> {
//     fn query(&self, id: usize) -> Option<(&'a T, T::Items<'a>)>;
// }

// This gets around having to manually specify the iterator type when
// implementing ContentsQuery. Maybe just get rid of the trait?
// impl<'a, T, F> ContentsQuery<'a, T> for F
// where
//     T: Contents + 'a,
//     F: Fn(usize) -> Option<(&'a T, T::Items<'a>)>,
//     // I: Iterator<Item = (usize, &'a Item)> + 'a,
// {
//     // type Items = I;

//     fn query(&self, id: usize) -> Option<(&'a T, T::Items<'a>)> {
//         self(id)
//     }
// }

// Use ContentsQuery to query a layout and contents, then show it.
pub fn show_contents(
    q: &ContentsStorage,
    id: usize,
    drag_item: &Option<DragItem>,
    ui: &mut egui::Ui,
) -> Option<InnerResponse<MoveData>> {
    q.get(&id)
        .map(|(layout, items)| layout.ui(id.into_ctx(), q, drag_item, items, ui))
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

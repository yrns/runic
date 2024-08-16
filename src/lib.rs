mod contents;
mod item;
mod min_frame;
mod shape;

use bevy_ecs::prelude::Entity;
use egui::{InnerResponse, Vec2};

pub use contents::*;
pub use item::*;
pub use shape::*;

pub const SLOT_SIZE: f32 = 48.0;

/// Single slot dimensions in pixels.
pub const fn slot_size() -> egui::Vec2 {
    egui::Vec2::splat(SLOT_SIZE)
}

#[derive(Debug)]
pub struct DragItem<T> {
    pub id: Entity,
    /// A clone of the original item (such that it can be rotated while dragging without affecting the original).
    pub item: Item<T>,
    /// Source location container id, slot, and shape with the dragged item unpainted, used for fit-checking if dragged within the source container.
    pub source: Option<(Entity, usize, Shape)>,
    // TODO:?
    // pub target: Option<(Entity, usize)>,
    /// Slot and offset inside the item when drag started. FIX: Is the slot used?
    pub offset: (usize, Vec2),
}

mod drag {
    macro_rules! item {
        ($drag_item:ident, $id:expr, $item:expr) => {
            $drag_item
                .as_ref()
                .filter(|d| d.id == $id)
                .map(|d| (true, &d.item))
                .unwrap_or((false, $item))
        };
    }

    pub(crate) use item;
}

/// source item -> target container
pub struct MoveData<T> {
    pub drag: Option<DragItem<T>>,
    pub target: Option<(Entity, usize)>,
}

impl<T> Default for MoveData<T> {
    fn default() -> Self {
        Self {
            drag: None,
            target: None,
        }
    }
}

impl<T> MoveData<T> {
    // TODO: remove
    pub fn merge(self, other: Self) -> Self {
        //let Self { item, container } = self;
        if let (Some(drag), Some(other)) = (self.drag.as_ref(), other.drag.as_ref()) {
            tracing::error!("multiple items! ({:?} and {:?})", drag.id, other.id)
        }
        if let (Some((c, ..)), Some((other, ..))) = (self.target.as_ref(), other.target.as_ref()) {
            tracing::error!("multiple containers! ({:?} and {:?})", c, other)
        }
        Self {
            drag: self.drag.or(other.drag),
            target: self.target.or(other.target),
        }
    }
}

pub struct ContainerSpace;

impl ContainerSpace {
    // Not a widget since it doesn't return a Response, but we can use
    // ui.scope just to get a response.
    pub fn show<T>(
        drag_item: &mut Option<DragItem<T>>,
        ui: &mut egui::Ui,
        add_contents: impl FnOnce(&Option<DragItem<T>>, &mut egui::Ui) -> MoveData<T>,
    ) -> Option<MoveData<T>> {
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
            let b = !ui.ctx().debug_on_hover();
            ui.ctx().style_mut(|s| {
                s.debug.debug_on_hover = b;
                s.debug.hover_shows_next = b;
                // s.debug.show_expand_width = b;
                // s.debug.show_expand_height = b;
                // s.debug.show_widget_hits = b;
            });
        }

        // If the pointer is released, take drag_item. TODO: do first?
        ui.input(|i| i.pointer.any_released())
            // If we have both a dragged item and a target, put the
            // item back into the move data and return it.
            .then(|| match (drag_item.take(), data.target.is_some()) {
                (Some(drag), true) => {
                    assert!(data.drag.replace(drag).is_none());
                    Some(data)
                }
                (Some(drag_item), false) => {
                    // FIX name?
                    tracing::warn!(drag_item = ?drag_item.id, "no target");
                    None
                }
                _ => None,
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
        .map(|slot| offset + xy(slot, shape.width()) * SLOT_SIZE)
        .filter(|p| grid_rect.contains(*p + egui::vec2(1., 1.)))
        // It does not matter if we don't use all the shape indices.
        .zip(idxs.iter())
        .for_each(|(p, idx)| {
            let slot_rect = egui::Rect::from_min_size(p, slot_size());
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

// Container id, egui id.
#[derive(Clone, Debug)]
pub struct Context {
    container_id: Entity,
}

impl From<Entity> for Context {
    fn from(container_id: Entity) -> Self {
        Self {
            container_id,
            // container_eid: egui::Id::new("contents").with(container_id),
        }
    }
}

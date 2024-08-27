pub mod grid;

use crate::*;
pub use grid::*;

use bevy_core::Name;
use bevy_ecs::{prelude::*, system::SystemParam};
use egui::{
    ecolor::{tint_color_towards, Color32},
    InnerResponse, Response, Ui, Vec2,
};

pub const SLOT_SIZE: f32 = 48.0;

/// Single slot dimensions in pixels.
pub const fn slot_size() -> egui::Vec2 {
    egui::Vec2::splat(SLOT_SIZE)
}

pub type BoxedContents<T> = Box<dyn Contents<T> + Send + Sync + 'static>;

// Generic over contents?
#[derive(Component)]
pub struct ContentsLayout<T>(pub BoxedContents<T>);

#[derive(Component, Default)]
pub struct ContentsItems(pub Vec<(usize, Entity)>);

// TODO layout per contents?
#[derive(Component)]
pub struct Sections(pub egui::Layout, pub Vec<Entity>);

// #[derive(Component)]
// pub struct ItemFlags<T: Accepts + 'static>(T);

// #[derive(Component)]
// pub struct ContainerFlags<T: Accepts + 'static>(T);

#[derive(Debug)]
pub struct DragItem<T> {
    pub id: Entity,
    /// A clone of the original item (such that it can be rotated while dragging without affecting the original).
    pub item: Item<T>,
    /// Source location container id, slot, and shape with the dragged item unpainted, used for fit-checking if dragged within the source container.
    pub source: Option<(Entity, usize, Shape)>,
    /// Target container id and slot.
    pub target: Option<(Entity, usize)>,
    /// Slot and offset inside the item when drag started. FIX: Is the slot used?
    pub offset: (usize, egui::Vec2),
}

pub trait Accepts: Send + Sync + 'static {
    fn accepts(&self, other: Self) -> bool;
    fn all() -> Self;
}

impl<T> Accepts for T
where
    T: bitflags::Flags + Send + Sync + 'static,
{
    fn accepts(&self, other: Self) -> bool {
        self.contains(other)
    }

    fn all() -> Self {
        Self::all()
    }
}

#[derive(SystemParam)]
pub struct ContentsStorage<'w, 's, T: Send + Sync + 'static> {
    pub contents: Query<
        'w,
        's,
        (
            &'static mut ContentsLayout<T>, // Change to Contents parameter?
            &'static mut ContentsItems,
            // &'static ContainerFlags<T>,
            // TODO?
            // Option<&'static mut Sections>,
        ),
    >,
    pub items: Query<'w, 's, (&'static Name, &'static mut Item<T>)>,
    pub sections: Query<'w, 's, &'static Sections>,
    // pub container_flags: Query<'w, 's, &'static ContainerFlags<T>>,
    // pub item_flags: Query<'w, 's, &'static ItemFlags<T>>,
    pub drag: Local<'s, Option<DragItem<T>>>,
}

impl<'w, 's, T: Accepts + Clone> ContentsStorage<'w, 's, T> {
    pub fn update(&mut self, ctx: &mut egui::Context) {
        // If the pointer is released, resolve drag, if any.
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(drag) = self.drag.take() {
                self.resolve_drag(drag);
            }
        }

        // Clear the target every frame.
        if let Some(drag) = self.drag.as_mut() {
            drag.target = None;

            // Rotate the dragged item.
            if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                // Make this a method.
                drag.item.rotation = drag.item.rotation.increment();
                drag.item.shape = drag.item.shape.rotate90();
            }
        }

        // Toggle debug.
        if ctx.input(|i| i.key_pressed(egui::Key::D)) {
            let b = !ctx.debug_on_hover();
            ctx.style_mut(|s| {
                s.debug.debug_on_hover = b;
                s.debug.hover_shows_next = b;
                // s.debug.show_expand_width = b;
                // s.debug.show_expand_height = b;
                // s.debug.show_widget_hits = b;
            });
        }
    }

    /// Show contents for container `id` and update the current drag.
    pub fn show(&mut self, id: Entity, ui: &mut Ui) -> Option<Response> {
        let InnerResponse { inner, response } = self.show_contents(id, ui)?;
        match inner {
            Some(ItemResponse::NewTarget(id, slot)) => match self.drag.as_mut() {
                Some(drag) => drag.target = Some((id, slot)),
                None => (),
            },
            Some(ItemResponse::NewDrag(new_drag)) => *self.drag = Some(new_drag),
            None => (),
        }
        Some(response)
    }

    pub fn show_contents(
        &self,
        id: Entity,
        ui: &mut Ui,
    ) -> Option<InnerResponse<Option<ItemResponse<T>>>> {
        self.get(id)
            .map(|(layout, items)| layout.0.ui(id, self, items, ui))
    }

    pub fn get(
        &self,
        id: Entity,
    ) -> Option<(
        &ContentsLayout<T>,
        &ContentsItems,
        // &ContainerFlags<T>,
    )> {
        self.contents.get(id).ok()
    }

    pub fn items<'a>(
        &'a self,
        contents_items: &'a ContentsItems,
    ) -> impl Iterator<Item = ((usize, Entity), (&Name, &Item<T>))> + 'a {
        let q_items = self.items.iter_many(contents_items.0.iter().map(|i| i.1));
        contents_items.0.iter().copied().zip(q_items)
    }

    /// Inserts item with `id` into `container`. Returns final container id and slot.
    pub fn insert(&mut self, container: Entity, id: Entity) -> Option<(Entity, usize)> {
        let item = self.items.get(id).ok()?.1;

        // fits/find_slot only accept DragItem...
        let drag = DragItem {
            id,
            item: item.clone(),
            source: None,
            target: None,
            // unused...
            offset: Default::default(),
        };

        // This is fetching twice...
        let (container, slot) = self.find_slot(container, &drag)?;
        let (mut layout, mut items) = self.contents.get_mut(container).ok()?;

        items.insert(slot, id);
        let DragItem { item, .. } = drag;
        layout.0.insert(slot, &item);
        Some((container, slot))
    }

    pub fn is_container(&self, id: Entity) -> bool {
        self.contents.contains(id)
    }

    // Check sections first or last? Last is less recursion.
    pub fn find_slot(&self, id: Entity, drag_item: &DragItem<T>) -> Option<(Entity, usize)> {
        let find_slot = |id| {
            self.contents
                .get(id)
                .ok()
                .and_then(|(contents, _items)| contents.0.find_slot(id, drag_item))
        };

        find_slot(id).or_else(|| {
            self.sections
                .get(id)
                .ok()
                .and_then(|s| s.1.iter().find_map(|id| find_slot(*id)))
        })
    }

    pub fn resolve_drag(&mut self, drag: DragItem<T>) {
        let DragItem {
            id,
            item: Item {
                shape, rotation, ..
            },
            source: Some((container_id, container_slot, _)),
            target: Some((target_id, slot, ..)),
            ..
        } = drag
        else {
            tracing::info!("no target for drag");
            return;
        };

        // TODO better check for cycles?
        assert_ne!(
            id, container_id,
            "cannot move an item inside itself: {}",
            id
        );

        let (name, mut item) = self.items.get_mut(id).expect("item exists");

        // We can't fetch the source and destination container mutably if they're the same.
        let (mut src, dest) = if container_id == target_id {
            (
                self.contents
                    .get_mut(container_id)
                    .expect("src container exists"),
                None,
            )
        } else {
            let [src, dest] = self.contents.many_mut([container_id, target_id]);
            (src, Some(dest))
        };

        // Remove from source container.
        src.1.remove(container_slot, id);
        (src.0).0.remove(container_slot, item.as_ref());

        // Insert into destination container (or source if same).
        let (mut dest_layout, mut dest) = dest.unwrap_or(src);

        assert!(
            slot < dest_layout.0.len(),
            "destination slot in container range"
        );

        // Copy rotation and shape from the dragged item. Do this before inserting so the shape is painted correctly.
        if item.rotation != rotation {
            item.shape = shape;
            item.rotation = rotation;
        }

        // TODO: put slot_item back on error?
        dest.insert(slot, id);
        dest_layout.0.insert(slot, item.as_ref());

        tracing::info!("moved item {name} {id} {rotation:?} -> container {target_id} slot {slot}");
    }
}

impl ContentsItems {
    pub fn insert(&mut self, slot: usize, id: Entity) {
        // Multiple items can share the same slot if they fit together.
        let i = self
            .0
            .binary_search_by(|(k, _)| k.cmp(&slot))
            // .expect_err("item slot free");
            .unwrap_or_else(|i| i);
        self.0.insert(i, (slot, id));
    }

    pub fn remove(&mut self, slot: usize, id: Entity) {
        self.0
            .iter()
            .position(|slot_item| *slot_item == (slot, id))
            //.position(|(_, item)| item == id)
            .map(|i| self.0.remove(i))
            .expect("item exists");
    }
}

/// A widget to display the contents of a container.
pub trait Contents<T: Accepts> {
    fn boxed(self) -> Box<dyn Contents<T> + Send + Sync>
    where
        Self: Sized + Send + Sync + 'static,
    {
        Box::new(self)
    }

    /// Number of slots this container holds.
    fn len(&self) -> usize;

    fn insert(&mut self, slot: usize, item: &Item<T>);

    fn remove(&mut self, slot: usize, item: &Item<T>);

    /// Returns a position for a given slot relative to the contents' origin.
    fn pos(&self, slot: usize) -> egui::Vec2;

    /// Returns a container slot for a given offset. May return
    /// invalid results if the offset is outside the container.
    fn slot(&self, offset: egui::Vec2) -> usize;

    fn accepts(&self, item: &Item<T>) -> bool;

    /// Returns true if the dragged item will fit at the specified slot.
    fn fits(&self, id: Entity, item: &DragItem<T>, slot: usize) -> bool;

    /// Finds the first available slot for the dragged item.
    fn find_slot(&self, id: Entity, item: &DragItem<T>) -> Option<(Entity, usize)>;

    fn shadow_color(&self, accepts: bool, fits: bool, ui: &egui::Ui) -> egui::Color32 {
        let color = if !accepts {
            Color32::GRAY
        } else if fits {
            Color32::GREEN
        } else {
            Color32::RED
        };
        tint_color_towards(color, ui.visuals().window_fill())
    }

    /// Draw contents.
    fn body(
        &self,
        id: Entity,
        contents: &ContentsStorage<T>,
        items: &ContentsItems,
        ui: &mut egui::Ui,
    ) -> InnerResponse<Option<ItemResponse<T>>>;

    /// Draw container.
    fn ui(
        &self,
        id: Entity,
        contents: &ContentsStorage<T>,
        items: &ContentsItems,
        ui: &mut egui::Ui,
    ) -> InnerResponse<Option<ItemResponse<T>>>;
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

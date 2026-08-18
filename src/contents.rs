// mod builder;
mod grid;

use bevy_ecs::{prelude::*, system::SystemParam};
use bevy_egui::egui::{
    self,
    ecolor::{tint_color_towards, Color32},
    Align, Direction, Id, InnerResponse, Pos2, Rect, Response, Ui, Vec2,
};
use bevy_reflect::{Reflect, ReflectDeserialize, ReflectSerialize};
use serde::{Deserialize, Serialize};

use crate::*;
// pub use builder::*;
pub use grid::*;

/// The slot this item occupies in its parent container.
// This really shouldn't derive `Default`, but it has to for BSN?
#[derive(Component, Clone, Default, Reflect)]
#[reflect(Component)]
pub struct Slot(pub usize);

// TODO: It is now.
/// Optional layout overrides the default in `Options`. egui::Layout is not serializable (egui::Direction is). Furthermore, some of the alignment values just don't work well (e.g. centering). So we just make our own struct with only direction and wrapping.
#[derive(Component, Copy, Clone, Debug, PartialEq, Reflect, Deserialize, Serialize)]
#[reflect(opaque)]
#[reflect(Component, PartialEq, Debug, Deserialize, Serialize)]
pub struct Layout {
    pub direction: egui::Direction,
    pub wrap: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            direction: Direction::LeftToRight,
            wrap: false,
        }
    }
}

impl Layout {
    pub fn new(direction: Direction, wrap: bool) -> Self {
        Self { direction, wrap }
    }

    pub fn to_egui_layout(&self) -> egui::Layout {
        egui::Layout {
            main_dir: self.direction,
            main_wrap: self.wrap,
            main_align: Align::Min,
            cross_align: Align::Min,
            ..Default::default()
        }
    }
}

/// Response (inner) returned from `Contents::ui` and `Item::ui`. Sets new drag or current drag target.
#[derive(Debug)]
pub enum ContentsResponse<T> {
    NewTarget((Entity, usize, Id)),
    NewDrag(DragItem<T>),
    // Ctrl-click sends then item to container's default target.
    SendItem(DragItem<T>),
    Open(Entity),
}

/// Source container id, slot, and shape with the dragged item unpainted, used for fit-checking if dragged within the source container.
// TODO: Use Slot.
pub type DragSource = Option<(Entity, usize, Shape)>;

/// An item being dragged.
#[derive(Debug)]
pub struct DragItem<T> {
    /// Dragged item id.
    pub id: Entity,
    /// A clone of the original item (such that it can be rotated while dragging without affecting the original).
    pub item: Item,
    /// Flags.
    pub flags: Flags<T>,
    /// Source location.
    pub source: DragSource,
    /// Target container id and slot, and the egui Id of the widget who set the target.
    pub target: Option<(Entity, usize, Id)>,
    /// Relative offset inside the item where the drag started.
    pub offset: Vec2,
    /// Relative offset outside the item, close to the inner offset.
    pub outer_offset: Vec2,
    /// Screen space origin of the original location.
    pub origin: Pos2,
    // The slot is not used currently.
    pub offset_slot: usize,
}

pub const OUTER_DISTANCE: f32 = 6.0;

impl<T> DragItem<T> {
    pub fn new(id: Entity, item: Item, flags: Flags<T>) -> Self {
        Self {
            id,
            item,
            flags,
            source: None,
            target: None,
            offset: Vec2::ZERO,
            outer_offset: Vec2::ZERO,
            origin: Pos2::ZERO,
            offset_slot: 0,
        }
    }

    fn rotate90(&mut self) {
        self.item.rotation = self.item.rotation.increment();
        self.item.shape = self.item.shape.rotate90();

        // This is close but not quite right. This also leaves the slot incorrect...
        if !self.item.shape.is_square() {
            self.offset = self.offset.yx();
            // We need the slot dimensions to recalculate. This works?
            self.outer_offset = self.outer_offset.yx();
        }
    }
}

/// `Accepts` must be `Clone` because items are cloned.
// TODO Indicate textually why something does't accept another?
pub trait Accepts: Copy + Clone + Default + std::fmt::Display + Send + Sync + 'static {
    fn accepts(&self, other: &Self) -> bool;
}

impl<T> Accepts for T
where
    T: bitflags::Flags + Copy + Clone + Default + std::fmt::Display + Send + Sync + 'static,
{
    fn accepts(&self, other: &Self) -> bool {
        self.contains(*other)
    }
}

/// Contents layout options.
#[derive(Clone, Debug, Resource)]
pub struct Options {
    /// Controls the layout of contents relative to sections. The default is vertical, sections last.
    pub layout: Layout,
    /// Default layout for sections.
    pub section_layout: Layout,
    /// Inline contents layout.
    pub inline_layout: Layout,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            // Align center does not work due to limitations w/ egui.
            layout: Layout::new(Direction::TopDown, false),
            section_layout: Default::default(),
            inline_layout: Default::default(),
        }
    }
}

/// Bit flags used to determine compatibility between containers and items.
#[derive(Component, Copy, Clone, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct Flags<T>(pub T);

impl<T: Accepts> Flags<T> {
    pub fn accepts(&self, flags: &Flags<T>) -> bool {
        self.0.accepts(&flags.0)
    }
}

/// Contents storage.
#[derive(SystemParam)]
pub struct ContentsStorage<'w, 's, T: Send + Sync + 'static> {
    pub commands: Commands<'w, 's>,
    pub contents: Query<
        'w,
        's,
        (
            Entity,
            &'static mut GridContents,
            &'static Flags<T>,
            Option<&'static Children>,
        ),
        // TODO?
        // Option<&'static mut Sections>,
    >,
    pub items: Query<
        'w,
        's,
        (
            Entity,
            &'static Slot,
            &'static Name,
            &'static mut Item,
            &'static Flags<T>,
            Option<&'static IconId>,
        ),
    >,
    // FIX: This should be an option part of the item and not grid contents.
    pub sections: Query<'w, 's, (Option<&'static Layout>, &'static Children)>,

    // pub container_flags: Query<'w, 's, &'static ContainerFlags<T>>,
    // pub item_flags: Query<'w, 's, &'static ItemFlags<T>>,

    // TODO: This should probably be a Resource in case you are showing containers from multiple different systems.
    pub drag: Local<'s, Option<DragItem<T>>>,

    // Target container for sending items directly (via control click, etc.). TODO: Parameter to Self::show?
    pub target: Local<'s, Option<Entity>>,

    pub options: Res<'w, Options>,
}

impl<'w, 's, T: Accepts> ContentsStorage<'w, 's, T> {
    pub fn update(&mut self, ctx: &mut egui::Context) {
        // If the pointer is released, resolve drag, if any.
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(drag) = self.drag.take() {
                if let Some((entity, slot, _)) = drag.target {
                    self.commands.trigger(ItemDragEnd {
                        entity,
                        slot,
                        item: drag.id,
                    })
                }

                self.resolve_drag(drag);
            }
        }

        if let Some(drag) = self.drag.as_mut() {
            // Rotate the dragged item.
            if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                drag.rotate90();
            }
        }

        // Toggle debug.
        if ctx.input(|i| i.key_pressed(egui::Key::D)) {
            let b = !ctx.debug_on_hover();
            ctx.global_style_mut(|s| {
                s.debug.debug_on_hover = b;
                s.debug.hover_shows_next = b;
                // s.debug.show_expand_width = b;
                // s.debug.show_expand_height = b;
                // s.debug.show_widget_hits = b;
            });
        }
    }

    pub fn set_drag_target(&mut self, target: Option<(Entity, usize, Id)>) {
        if let Some(drag) = self.drag.as_mut() {
            // set_if_neq?
            if drag.target != target {
                drag.target = target;

                if let Some((entity, slot, _)) = target {
                    self.commands.trigger(ItemDragOver {
                        entity,
                        slot,
                        item: drag.id,
                    });
                }
            }
        }
    }

    /// Show contents for container `id` and update the current drag.
    pub fn show(&mut self, id: Entity, ui: &mut Ui) -> Option<Response> {
        let InnerResponse { inner, response } = self.show_contents(id, ui)?;

        match inner {
            Some(ContentsResponse::NewTarget((id, slot, _))) => {
                // Overwrite the egui id. The original is effectively unused.
                self.set_drag_target(Some((id, slot, ui.id())))
            }
            Some(ContentsResponse::NewDrag(new_drag)) => {
                *self.drag = Some(new_drag);

                if let Some(DragItem {
                    id: item,
                    source: Some((id, slot, _)),
                    ..
                }) = &*self.drag
                {
                    self.commands.trigger(ItemDragStart {
                        // source contents
                        entity: *id,
                        slot: *slot,
                        item: *item,
                    });
                }
            }
            Some(ContentsResponse::SendItem(mut item)) => {
                item.target = self.target.and_then(|t| {
                    self.find_section_slot(t, &item.item, &item.flags, &item.source)
                        .map(|(id, slot)| (id, slot, ui.id()))
                });
                self.resolve_drag(item);
            }
            Some(ContentsResponse::Open(item)) => {
                if self.is_container(item) {
                    self.commands.trigger(ContainerOpen(item));
                }
            }
            None => {
                // If we contain the pointer and we didn't get a new target, clear the target. Or if we don't contain the pointer and we originally set the target, then clear it. We check the widget id rather than the contents because the same contents may be shown twice (due to inline contents and open contents).
                if response.contains_pointer() || self.target_eid().is_some_and(|id| id == ui.id())
                {
                    self.set_drag_target(None);
                }
            }
        }

        Some(response)
    }

    pub fn target_eid(&self) -> Option<Id> {
        Some(self.drag.as_ref()?.target?.2)
    }

    /// Show sections.
    // Items no longer have contents, just sections.
    pub fn show_contents(
        &self,
        id: Entity,
        ui: &mut Ui,
    ) -> Option<InnerResponse<Option<ContentsResponse<T>>>> {
        use itertools::Itertools;

        // Sections. TODO: The entity mapping issue indicates there's a bad case here that needs to be caught? Like if the `id` isn't a container at all or doesn't exist?

        let (layout, sections) = self.sections.get(id).ok()?;
        let layout = layout
            .unwrap_or(&self.options.section_layout)
            .to_egui_layout();

        Some(ui.with_layout(layout, |ui| {
            // TODO faster to fetch many first?
            sections
                .iter()
                .filter_map(|id| {
                    self.contents
                        .get(id)
                        .ok()
                        .and_then(|(_, contents, flags, items)| {
                            if let Some(items) = items {
                                contents.ui(id, flags, self, items, ui)
                            } else {
                                contents.ui(id, flags, self, &[], ui)
                            }
                            .inner
                        })
                })
                .at_most_one()
                .unwrap_or_else(|mut e| {
                    tracing::error!("at most one item response");
                    e.next()
                })
        }))
    }

    /// Inserts item with `id` into `container`. Returns final container id and slot. This for an untargeted insert (no target slot). It will find the first available section and slot.
    // Since we're not using relationships, there's no way to easily guarantee that this item isn't in another container already... FIX when/if we tie items to UI components (Bevy)
    // TODO: more checking and return a Result? same for removal
    // NOTE: Multiple items can share the same slot if they fit together.
    pub fn insert(&mut self, container: Entity, id: Entity) -> Option<(Entity, usize)> {
        let (_, _slot, _, item, flags, _) = self.items.get(id).ok()?;

        // Find (sub-)container and free slot. This is fetching twice...
        let (container, slot) = self.find_section_slot(container, &item, flags, &None)?;
        let (_, mut contents, _flags, _items) = self.contents.get_mut(container).ok()?;

        assert!(slot < contents.slots(), "slot in contents length");
        self.commands.entity(container).add_child(id);
        contents.insert(slot, item);

        // Assign slot.
        self.commands.entity(id).insert(Slot(slot));

        Some((container, slot))
    }

    // Containers and items are now always separate entities (each with separate flags). And the contents entities are contained in the section entity of the item. This means every item that's a container is always two entities...
    // TODO: Remove?
    pub fn is_container(&self, id: Entity) -> bool {
        self.sections.contains(id)
    }

    /// Returns true if the sections of `a` contains `b`.
    pub fn contains(&self, a: Entity, b: Entity) -> bool {
        self.sections
            .get(a)
            .is_ok_and(|(_, s)| s.iter().any(|s| s == b || self.contains(s, b)))
    }

    /// Search all sections of container `id` for an available slot.
    pub fn find_section_slot(
        &self,
        id: Entity,
        item: &Item,
        flags: &Flags<T>,
        source: &DragSource,
    ) -> Option<(Entity, usize)> {
        // Pass in sections since we're probably already fetching it?
        // Consider layout in the order?
        self.contents
            .iter_many(self.sections.get(id).ok()?.1.iter())
            .filter(|(_, _, f, _)| f.accepts(&flags))
            .find_map(|(id, contents, ..)| contents.find_slot(id, item, source))
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

        if id == target_id || self.contains(id, target_id) {
            return tracing::info!("cannot move an item inside itself");
        }

        let (.., mut item, _flags, _) = self.items.get_mut(id).expect("item exists");

        // We can't fetch the source and destination container mutably if they're the same.
        let ((_, mut contents, _flags, _items), dest) = if container_id == target_id {
            (
                self.contents
                    .get_mut(container_id)
                    .expect("src container exists"),
                None,
            )
        } else if let Ok([src, dest]) = self.contents.get_many_mut([container_id, target_id]) {
            (src, Some(dest))
        } else {
            return tracing::error!(
                "no contents for source ({}) or destination ({})",
                container_id,
                target_id,
            );
        };

        // Remove from source container.
        {
            // Hierarchy makes this automatic.
            // let index = items.0.iter().position(|a| *a == id).unwrap();
            // items.0.swap_remove(index);
            contents.remove(container_slot, &item);
        }

        // Copy rotation and shape from the dragged item. Do this before inserting so the shape is painted correctly.
        if item.rotation != rotation {
            item.shape = shape;
            item.rotation = rotation;
        }

        // Insert into destination container (or source if same). TODO: put slot_item back on error? There is no error?
        self.commands.entity(id).insert(Slot(slot));
        {
            let mut contents = match dest {
                Some((_, c, ..)) => c,
                None => contents,
            };
            self.commands.entity(target_id).add_child(id);
            contents.insert(slot, &item);
        }

        // Fire events.
        if container_id == target_id {
            self.commands.trigger(ItemMove {
                entity: container_id,
                old_slot: container_slot,
                new_slot: slot,
                item: id,
            });
        } else {
            self.commands.trigger(ItemRemove {
                entity: container_id,
                slot: container_slot,
                item: id,
            });

            self.commands.trigger(ItemInsert {
                entity: target_id,
                slot,
                item: id,
            });
        }
    }
}

/// A widget to display the contents of a container.
pub trait Contents {
    /// Number of slots this container holds.
    fn slots(&self) -> usize;

    fn insert(&mut self, slot: usize, item: &Item);

    fn remove(&mut self, slot: usize, item: &Item);

    /// Returns a position for a given slot relative to the contents' origin.
    fn pos(&self, slot: usize) -> Vec2;

    /// Returns a container slot for a given offset. May return
    /// invalid results if the offset is outside the container.
    fn slot(&self, offset: Vec2) -> usize;

    // fn accepts(&self, item: &Item) -> bool;

    /// Returns true if the dragged item will fit at the specified slot.
    fn fits(&self, id: Entity, item: &Item, slot: usize, source: &DragSource) -> bool;

    /// Finds the first available slot for the dragged item.
    fn find_slot(&self, id: Entity, item: &Item, source: &DragSource) -> Option<(Entity, usize)>;

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
    fn body<T: Accepts>(
        &self,
        id: Entity,
        contents: &ContentsStorage<T>,
        items: &[Entity],
        ui: &mut egui::Ui,
    ) -> InnerResponse<Option<ContentsResponse<T>>>;

    /// Draw container.
    fn ui<T: Accepts>(
        &self,
        id: Entity,
        flags: &Flags<T>,
        contents: &ContentsStorage<T>,
        items: &[Entity],
        ui: &mut egui::Ui,
    ) -> InnerResponse<Option<ContentsResponse<T>>>;
}

pub fn xy(slot: usize, width: usize) -> Vec2 {
    Vec2::new((slot % width) as f32, (slot / width) as f32)
}

// pub fn paint_shape(
//     idxs: Vec<egui::layers::ShapeIdx>,
//     shape: &shape::Shape,
//     grid_rect: egui::Rect,
//     offset: egui::Vec2,
//     color: egui::Color32,
//     ui: &mut egui::Ui,
// ) {
//     let offset = grid_rect.min + offset;
//     shape
//         .slots()
//         .map(|slot| offset + xy(slot, shape.width()) * SLOT_SIZE)
//         .filter(|p| grid_rect.contains(*p + egui::vec2(1., 1.)))
//         // It does not matter if we don't use all the shape indices.
//         .zip(idxs.iter())
//         .for_each(|(p, idx)| {
//             let slot_rect = egui::Rect::from_min_size(p, slot_size());
//             // ui.painter()
//             //     .rect(slot_rect, 0., color, egui::Stroke::none())
//             ui.painter()
//                 .set(*idx, egui::epaint::RectShape::filled(slot_rect, 0., color));
//         })
// }

// Replaces `paint_shape` and uses only one shape index, so we don't
// have to reserve multiple. There is Shape::Vec, too.
pub fn shape_mesh(
    shape: &shape::Shape,
    grid_rect: Rect,
    offset: Vec2,
    color: Color32,
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
        .map(|p| Rect::from_min_size(p, Vec2::splat(scale)))
        .for_each(|rect| {
            mesh.add_colored_rect(rect, color);
        });
    mesh
}

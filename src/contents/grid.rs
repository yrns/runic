use bevy_egui::egui::{self, style::WidgetVisuals, Rect, Ui};
use itertools::Itertools;

use super::*;

/// Contains items in a 2d grid.
#[derive(Clone, Debug, Reflect)]
pub struct GridContents<T, const N: usize = 64> {
    /// If true, this grid only holds one item, but the size of that item can be any up to the maximum size.
    pub expands: bool,
    /// If true, show inline contents for the contained item.
    pub inline: bool,
    pub header: Option<String>, // Use Name?
    /// The shape describes the dimensions of the container and which slots are filled.
    pub shape: Shape,
    /// Flags determine what kinds of items will be accepted (see `Accepts`).
    pub flags: T,
}

impl<T, const N: usize> GridContents<T, N>
where
    T: Accepts,
{
    pub fn new(size: impl Into<Size>) -> Self {
        Self {
            expands: false,
            inline: false,
            header: None,
            shape: Shape::new(size.into(), false),
            flags: T::default(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<T>) -> Self {
        self.flags = flags.into();
        self
    }

    pub fn with_expands(mut self, expands: bool) -> Self {
        self.expands = expands;
        self
    }

    pub fn with_inline(mut self, inline: bool) -> Self {
        self.inline = inline;
        self
    }

    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.header = Some(header.into());
        self
    }

    /// Single slot dimensions in pixels.
    pub const fn slot_size() -> egui::Vec2 {
        egui::Vec2::splat(N as f32)
    }

    /// Grid dimensions in pixels.
    pub fn grid_size(&self, size: Size) -> egui::Vec2 {
        (size.as_vec2() * N as f32).as_ref().into()
    }

    /// Grid lines shape.
    pub fn grid_shape(&self, style: &egui::Style, size: Size) -> egui::Shape {
        let stroke1 = style.visuals.widgets.noninteractive.bg_stroke;
        let mut stroke2 = stroke1;
        stroke2.color = tint_color_towards(stroke1.color, style.visuals.extreme_bg_color);
        let stroke2 = egui::epaint::PathStroke::from(stroke2);

        let pixel_size = self.grid_size(size);
        let egui::Vec2 { x: w, y: h } = pixel_size;

        let mut lines = vec![];

        // Don't draw the outside edge.
        lines.extend((1..(size.x)).map(|x| {
            let x = x as f32 * N as f32;
            egui::Shape::LineSegment {
                points: [egui::Pos2::new(x, 0.0), egui::Pos2::new(x, h)],
                stroke: stroke2.clone(),
            }
        }));

        lines.extend((1..(size.y)).map(|y| {
            let y = y as f32 * N as f32;
            egui::Shape::LineSegment {
                points: [egui::Pos2::new(0.0, y), egui::Pos2::new(w, y)],
                stroke: stroke2.clone(),
            }
        }));

        lines.push(egui::Shape::Rect(egui::epaint::RectShape::new(
            Rect::from_min_size(egui::Pos2::ZERO, pixel_size),
            style.visuals.widgets.noninteractive.rounding,
            // style.visuals.window_rounding,
            Color32::TRANSPARENT, // fill covers the grid
            // style.visuals.window_fill,
            stroke1,
        )));

        egui::Shape::Vec(lines)
    }
}

impl<T: Accepts, const N: usize> Contents<T> for GridContents<T, N> {
    fn slots(&self) -> usize {
        if self.expands {
            1
        } else {
            self.shape.area()
        }
    }

    fn insert(&mut self, slot: usize, item: &Item<T>) {
        self.shape.paint(&item.shape, slot);
    }

    fn remove(&mut self, slot: usize, item: &Item<T>) {
        self.shape.unpaint(&item.shape, slot);
    }

    fn pos(&self, slot: usize) -> egui::Vec2 {
        // Expanding only ever has one slot.
        if self.expands {
            egui::Vec2::ZERO
        } else {
            xy(slot, self.shape.size.x as usize) * N as f32
        }
    }

    fn slot(&self, p: egui::Vec2) -> usize {
        // Expanding only ever has one slot.
        if self.expands {
            0
        } else {
            self.shape.slot(to_size(p / N as f32))
        }
    }

    fn accepts(&self, item: &Item<T>) -> bool {
        self.flags.accepts(&item.flags)
    }

    fn fits(&self, id: Entity, item: &Item<T>, slot: usize, source: &DragSource) -> bool {
        // Check if the shape fits here. When moving within
        // one container, use the cached shape with the
        // dragged item (and original rotation) unpainted.
        let shape = match source {
            Some((source_id, _, shape)) if id == *source_id => shape,
            _ => &self.shape,
        };

        shape.fits(&item.shape, slot)
    }

    fn find_slot(
        &self,
        id: Entity,
        item: &Item<T>,
        source: &DragSource,
    ) -> Option<(Entity, usize)> {
        if !self.accepts(item) {
            return None;
        }

        // TODO test multiple rotations (if non-square) and return it?
        (0..self.slots())
            .find(|slot| self.fits(id, item, *slot, source))
            .map(|slot| (id, slot))
    }

    fn body(
        &self,
        id: Entity,
        contents: &ContentsStorage<T>,
        items: &[(usize, Entity)],
        ui: &mut Ui,
    ) -> InnerResponse<Option<ContentsResponse<T>>> {
        assert!(items.len() <= self.slots());

        // For expanding contents we need to see the size of the first item before looping.
        let mut items = contents.items(items).peekable();

        let grid_size = if self.expands {
            items
                .peek()
                .map(|(_, (_, item, _))| item.shape.size)
                .unwrap_or(Size::ONE)
        } else {
            self.shape.size
        };

        // Allocate the full grid size. Note ui.min_rect() may differ from from the allocated rect
        // due to layout. So position items based on the latter.
        let (rect, response) =
            ui.allocate_exact_size(self.grid_size(grid_size), egui::Sense::hover());

        let new_drag = if ui.is_rect_visible(rect) {
            let grid_shape = ui.painter().add(egui::Shape::Noop);

            let new_drag = items
                .filter_map(|((slot, item_id), (name, item, icon))| {
                    // If this item is being dragged, we want to use the dragged rotation. Everything else should be the same.
                    let item = contents
                        .drag
                        .as_ref()
                        .filter(|d| d.id == item_id)
                        .map_or(item, |d| &d.item);

                    // Only allocate the slot otherwise we'll blow out the contents if it doesn't fit.
                    let item_rect =
                        Rect::from_min_size(rect.min + self.pos(slot), Self::slot_size());

                    // item returns a clone if it's being dragged
                    ui.allocate_ui_at_rect(item_rect, |ui| {
                        item.ui(
                            slot,
                            item_id,
                            name,
                            contents.drag.as_ref(),
                            // TODO A better default texture.
                            contents
                                .textures
                                .image_id(icon.handle())
                                .unwrap_or_default(),
                            N as f32,
                            ui,
                        )
                    })
                    .inner
                    .map(|mut cr| {
                        match cr {
                            // Set source. Contents id, current slot and container shape w/ the item unpainted.
                            ContentsResponse::NewDrag(ref mut drag)
                            | ContentsResponse::SendItem(ref mut drag) => {
                                let mut cshape = self.shape.clone();
                                cshape.unpaint(&drag.item.shape, slot);
                                drag.source = Some((id, slot, cshape));
                            }
                            _ => (),
                        }
                        cr
                    })
                })
                .at_most_one()
                //.expect("at most one item response");
                .unwrap_or_else(|mut e| {
                    tracing::warn!("more than one item response");
                    e.next()
                });

            let mut grid = self.grid_shape(ui.style(), grid_size);
            grid.translate(rect.min.to_vec2());
            ui.painter().set(grid_shape, grid);

            // debug paint the container "shape" (filled slots)
            if ui.ctx().debug_on_hover() {
                // Use the cached shape if the dragged item is ours. This rehashes what's in `fits`.
                let shape = contents
                    .drag
                    .as_ref()
                    .and_then(|d| d.source.as_ref())
                    .filter(|s| id == s.0)
                    .map(|d| &d.2)
                    .unwrap_or(&self.shape);

                ui.painter().add(shape_mesh(
                    shape,
                    rect,
                    egui::Vec2::ZERO,
                    Color32::GREEN.gamma_multiply(0.8),
                    N as f32,
                ));
            }

            new_drag
        } else {
            None
        };

        InnerResponse::new(new_drag, response)
    }

    fn ui(
        &self,
        id: Entity,
        contents: &ContentsStorage<T>,
        items: &[(usize, Entity)],
        ui: &mut Ui,
    ) -> InnerResponse<Option<ContentsResponse<T>>> {
        // This no longer works because `drag_item` is a frame behind `dragged_id`. In other words, the
        // dragged_id will be unset before drag_item for one frame.

        // match drag_item.as_ref().map(|d| d.item.eid()) {
        //     Some(id) => {
        //         assert_eq!(ui.ctx().dragged_id(), Some(id));
        //         // if ui.ctx().dragged_id() != Some(id) {
        //         //     tracing::warn!(
        //         //         "drag_item eid {:?} != dragged_id {:?}",
        //         //         id,
        //         //         ui.ctx().dragged_id()
        //         //     )
        //         // }
        //     }
        //     _ => (), // we could be dragging something else
        // }

        let header_frame = |ui: &mut Ui, add_contents| {
            ui.with_layout(contents.options.layout.to_egui_layout(), |ui| {
                // Sections.
                let section_ir = contents.sections.get(id).ok().and_then(|s| {
                    ui.with_layout(
                        s.0.unwrap_or(contents.options.section_layout)
                            .to_egui_layout(),
                        |ui| {
                            // TODO faster to fetch many first?
                            s.1.iter()
                                .filter_map(|id| contents.show_contents(*id, ui))
                                .filter_map(|ir| ir.inner)
                                .at_most_one()
                                .unwrap_or_else(|mut e| {
                                    tracing::error!("at most one item response");
                                    e.next()
                                })
                        },
                    )
                    .inner
                });

                // TODO? The header should always be above the contents that it describes (i.e. use Ui::vertical here)?
                if let Some(header) = self.header.as_ref() {
                    _ = ui.label(header)
                }

                let ir = ui
                    .with_layout(contents.options.inline_layout.to_egui_layout(), |ui| {
                        // Go back to with_bg/min_frame since egui::Frame takes up all available space.
                        let ir: Option<ContentsResponse<T>> =
                            crate::min_frame::min_frame(ui, add_contents).inner;

                        ir.or(
                            // Show inline contents.
                            self.inline
                                .then(|| {
                                    let drag_id = contents.drag.as_ref().map(|d| d.id);
                                    items
                                        .iter()
                                        .map(|(_, id)| *id)
                                        // Don't add contents if the container is being dragged.
                                        .filter(|id| drag_id != Some(*id))
                                        .filter_map(|id| contents.show_contents(id, ui))
                                        .filter_map(|ir| ir.inner)
                                        .at_most_one()
                                        .unwrap_or_else(|mut e| {
                                            tracing::error!("at most one item response");
                                            e.next()
                                        })
                                })
                                .flatten(),
                        )
                    })
                    .inner;

                section_ir.or(ir)
            })
        };

        header_frame(ui, |style: &mut WidgetVisuals, ui: &mut Ui| {
            // Reserve shape for the dragged item's shadow.
            let shadow = ui.painter().add(egui::Shape::Noop);

            let InnerResponse { inner, response } = self.body(id, contents, items, ui);
            let min_rect = response.rect;

            let inner = match (contents.drag.as_ref(), inner) {
                // We are dragging onto another item, check to see if the dragged item will fit anywhere within its contents.
                (Some(drag), Some(ContentsResponse::NewTarget((id, slot, _)))) => {
                    if contents.is_container(id) {
                        // Rather than cloning the item every frame on hover, we just refetch it. This probably could be eliminated by clarifying some lifetimes and just passing an item ref back.
                        let item = contents.items.get(id).expect("item exists").1;
                        let target = contents.find_slot(id, &drag.item, &drag.source);

                        // The item shadow is the target item for drag-to-item, not the dragged item.
                        let color = self.shadow_color(true, target.is_some(), ui);
                        let mesh =
                            shape_mesh(&item.shape, min_rect, self.pos(slot), color, N as f32);
                        ui.painter().set(shadow, mesh);

                        target
                            .map(|(item, slot)| ContentsResponse::NewTarget((item, slot, ui.id())))
                    } else {
                        // Don't set target to non-contents.
                        None
                    }
                }

                // Dragging over an empty slot.
                (Some(drag), None) => {
                    // tarkov also checks if containers are full, even if not
                    // hovering -- maybe track min size free? TODO just do
                    // accepts, and only check fits for hover

                    let accepts = self.accepts(&drag.item);

                    // Highlight the contents border if we can accept the dragged item.
                    if accepts {
                        // TODO move this to settings?
                        style.bg_stroke = ui.visuals().widgets.hovered.bg_stroke;
                    } else {
                        // This does nothing.
                        ui.disable()
                    }
                    // This is ugly w/ the default theme.
                    // *style = ui.style().interact_selectable(&response, accepts);

                    let slot = ui
                        .ctx()
                        .pointer_latest_pos()
                        .filter(|_| response.contains_pointer())
                        // Add (inset) a bit so it's easier to target from the upper left. TODO: Fix the weird clamping on the top and left?
                        // Shape::slot needs to return an option
                        // FIX expanding does not work well w/ the offset
                        .map(|p| {
                            self.slot(p - min_rect.min - drag.offset + Self::slot_size() * 0.5)
                        });

                    let fits = slot
                        .map(|slot| self.fits(id, &drag.item, slot, &drag.source))
                        .unwrap_or_default();

                    // Paint the dragged item's shadow, showing which slots will be filled.
                    if let Some(slot) = slot {
                        let color = self.shadow_color(accepts, fits, ui);
                        let shape = &drag.item.shape;
                        let mesh = shape_mesh(shape, min_rect, self.pos(slot), color, N as f32);
                        ui.painter().set(shadow, mesh);
                    }

                    // This no longer works since we resolve before we even get here.
                    // let released = ui.input(|i| i.pointer.any_released());
                    // if released && fits && !accepts {
                    //     tracing::info!(
                    //         "container {:?} does not accept item {}!",
                    //         id,
                    //         drag.item.flags
                    //     );
                    // }

                    slot.filter(|_| accepts && fits)
                        .map(|slot| ContentsResponse::NewTarget((id, slot, ui.id())))
                }

                (_, inner) => inner,
            };

            InnerResponse::new(inner, response)
        })
    }
}

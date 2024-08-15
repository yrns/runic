use egui::style::WidgetVisuals;

use super::*;

#[derive(Clone, Debug)]
pub struct GridContents {
    /// If true, this grid only holds one item, but the size of that item can be any up to the maximum size.
    pub expands: bool,
    /// If true, show inline contents for the contained item.
    pub inline: bool,
    pub header: Option<String>,
    pub shape: Shape,
    pub flags: ItemFlags,
}

impl GridContents {
    pub fn new(size: impl Into<Size>) -> Self {
        Self {
            expands: false,
            inline: false,
            header: None,
            shape: Shape::new(size.into(), false),
            flags: ItemFlags::all(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<ItemFlags>) -> Self {
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

    pub fn grid_size(&self, size: Size) -> egui::Vec2 {
        (size.as_vec2() * SLOT_SIZE).as_ref().into()
    }

    // Grid lines shape.
    pub fn grid_shape(&self, style: &egui::Style, size: Size) -> egui::Shape {
        let stroke1 = style.visuals.widgets.noninteractive.bg_stroke;
        let mut stroke2 = stroke1.clone();
        stroke2.color = tint_color_towards(stroke1.color, style.visuals.extreme_bg_color);
        let stroke2 = egui::epaint::PathStroke::from(stroke2);

        let pixel_size = (size.as_vec2() * SLOT_SIZE).as_ref().into();
        let egui::Vec2 { x: w, y: h } = pixel_size;

        let mut lines = vec![];

        // Don't draw the outside edge.
        lines.extend((1..(size.x)).map(|x| {
            let x = x as f32 * SLOT_SIZE;
            egui::Shape::LineSegment {
                points: [egui::Pos2::new(x, 0.0), egui::Pos2::new(x, h)],
                stroke: stroke2.clone(),
            }
        }));

        lines.extend((1..(size.y)).map(|y| {
            let y = y as f32 * SLOT_SIZE;
            egui::Shape::LineSegment {
                points: [egui::Pos2::new(0.0, y), egui::Pos2::new(w, y)],
                stroke: stroke2.clone(),
            }
        }));

        lines.push(egui::Shape::Rect(egui::epaint::RectShape::new(
            egui::Rect::from_min_size(egui::Pos2::ZERO, pixel_size),
            style.visuals.widgets.noninteractive.rounding,
            // style.visuals.window_rounding,
            Color32::TRANSPARENT, // fill covers the grid
            // style.visuals.window_fill,
            stroke1,
        )));

        egui::Shape::Vec(lines)
    }
}

impl Contents for GridContents {
    fn len(&self) -> usize {
        if self.expands {
            1
        } else {
            self.shape.area()
        }
    }

    fn insert(&mut self, slot: usize, item: &Item) {
        self.shape.paint(&item.shape, slot);
    }

    fn remove(&mut self, slot: usize, item: &Item) {
        self.shape.unpaint(&item.shape, slot);
    }

    fn pos(&self, slot: usize) -> egui::Vec2 {
        // Expanding only ever has one slot.
        if self.expands {
            egui::Vec2::ZERO
        } else {
            xy(slot, self.shape.size.x as usize) * SLOT_SIZE
        }
    }

    fn slot(&self, p: egui::Vec2) -> usize {
        // Expanding only ever has one slot.
        if self.expands {
            0
        } else {
            self.shape.slot(to_size(p / SLOT_SIZE))
        }
    }

    fn accepts(&self, item: &Item) -> bool {
        self.flags.contains(item.flags)
    }

    fn fits(&self, ctx: &Context, drag: &DragItem, slot: usize) -> bool {
        // Check if the shape fits here. When moving within
        // one container, use the cached shape with the
        // dragged item (and original rotation) unpainted.
        let shape = match &drag.source {
            Some((id, _, shape)) if ctx.container_id == *id => shape,
            _ => &self.shape,
        };

        shape.fits(&drag.item.shape, slot)
    }

    fn find_slot(&self, ctx: &Context, drag: &DragItem) -> Option<(Entity, usize)> {
        if !self.accepts(&drag.item) {
            return None;
        }

        // TODO test multiple rotations (if non-square) and return it?
        (0..self.len())
            .find(|slot| self.fits(ctx, drag, *slot))
            .map(|slot| (ctx.container_id, slot))
    }

    fn body(
        &self,
        ctx: &Context,
        q: &ContentsStorage,
        drag_item: &Option<DragItem>,
        items: &ContentsItems,
        ui: &mut egui::Ui,
    ) -> InnerResponse<Option<ItemResponse>> {
        assert!(items.0.len() <= self.len());

        // For expanding contents we need to see the size of the first item before looping.
        let mut items = q.items(items).peekable();

        let grid_size = if self.expands {
            items
                .peek()
                .map(|(_, (_, item))| item.shape.size)
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
                .map(|((slot, id), (name, item))| {
                    // If this item is being dragged, we want to use the dragged rotation.
                    // Everything else should be the same.
                    let (dragged, item) = drag::item!(drag_item, id, item);

                    let item_rect = egui::Rect::from_min_size(
                        rect.min + self.pos(slot),
                        if dragged {
                            // Only allocate the slot otherwise we'll blow out the contents if it
                            // doesn't fit.
                            slot_size()
                        } else {
                            item.size()
                        },
                    );

                    // item returns a clone if it's being dragged
                    ui.allocate_ui_at_rect(item_rect, |ui| item.ui(slot, id, name, drag_item, ui))
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
                // Set drag source. Contents id, current slot and container shape w/ the item unpainted.
                .map(|(slot, ir)| match ir {
                    ItemResponse::NewDrag(mut drag) => {
                        let mut cshape = self.shape.clone();
                        cshape.unpaint(&drag.item.shape, slot);
                        drag.source = Some((ctx.container_id, slot, cshape));
                        ItemResponse::NewDrag(drag)
                    }
                    _ => ir,
                });

            let mut grid = self.grid_shape(ui.style(), grid_size);
            grid.translate(rect.min.to_vec2());
            ui.painter().set(grid_shape, grid);

            // debug paint the container "shape" (filled slots)
            if ui.ctx().debug_on_hover() {
                // Use the cached shape if the dragged item is ours. This rehashes what's in `fits`.
                let shape = drag_item
                    .as_ref()
                    .and_then(|d| d.source.as_ref())
                    .filter(|s| ctx.container_id == s.0)
                    .map(|d| &d.2)
                    .unwrap_or(&self.shape);

                ui.painter().add(shape_mesh(
                    shape,
                    rect,
                    egui::Vec2::ZERO,
                    Color32::GREEN.gamma_multiply(0.8),
                    SLOT_SIZE,
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
        ctx: &Context,
        q: &ContentsStorage,
        drag_item: &Option<DragItem>,
        items: &ContentsItems,
        ui: &mut egui::Ui,
    ) -> InnerResponse<MoveData> {
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

        let header_frame = |ui: &mut egui::Ui, add_contents| {
            ui.vertical(|ui| {
                // Sections.
                let section_data = if let Ok(Sections(layout, sections)) =
                    q.sections.get(ctx.container_id)
                {
                    ui.with_layout(*layout, |ui| {
                        // TODO faster to fetch many first?
                        sections
                            .iter()
                            .filter_map(|id| q.show_contents(*id, drag_item, ui).map(|ir| ir.inner))
                            .reduce(|acc, data| acc.merge(data))
                    })
                    .inner
                } else {
                    None
                };

                match self.header.as_ref() {
                    Some(header) => _ = ui.label(header),
                    _ => (),
                }

                // TODO: Make the order and layout configurable.
                ui.horizontal_top(|ui| {
                    let data: MoveData = crate::min_frame::min_frame(ui, add_contents).inner;

                    let data = match section_data {
                        Some(section_data) => section_data.merge(data),
                        None => data,
                    };

                    // Show inline contents.
                    if self.inline {
                        // What if there's more than one item?
                        if let Some((_slot, id)) = items.0.first() {
                            // Don't add contents if the container is being dragged.
                            if !drag_item.as_ref().is_some_and(|d| d.id == *id) {
                                if let Some(inline_data) = q.show_contents(*id, drag_item, ui) {
                                    return data.merge(inline_data.inner);
                                }
                            }
                        }
                    }

                    data
                })
            })
            .inner
        };

        // Go back to with_bg/min_frame since egui::Frame takes up all available space.
        header_frame(ui, |style: &mut WidgetVisuals, ui: &mut egui::Ui| {
            // Reserve shape for the dragged item's shadow.
            let shadow = ui.painter().add(egui::Shape::Noop);

            let InnerResponse { inner, response } = self.body(ctx, q, drag_item, items, ui);
            let min_rect = response.rect;

            // If we are dragging onto another item, check to see if the dragged item will fit anywhere within its contents.
            let move_data = match (drag_item, inner) {
                (Some(drag), Some(ItemResponse::SetTarget((slot, id)))) if q.is_container(id) => {
                    let target = q.find_slot(id, drag);
                    // The item shadow becomes the target item, not the dragged item, for
                    // drag-to-item. TODO just use rect
                    let color = self.shadow_color(true, target.is_some(), ui);
                    let mut mesh = egui::Mesh::default();
                    // Rather than cloning the item every frame on hover, we just refetch it. This probably could be eliminated by clarifying some lifetimes and just passing an item ref back.
                    let item = q.items.get(id).expect("item exists").1;
                    mesh.add_colored_rect(
                        egui::Rect::from_min_size(min_rect.min + self.pos(slot), item.size()),
                        color,
                    );
                    ui.painter().set(shadow, mesh);

                    MoveData { drag: None, target }
                }
                // Unless the drag is released and pressed in the same frame, we should never have a new drag while dragging?
                (None, ir) => MoveData {
                    drag: ir.map(|ir| match ir {
                        ItemResponse::SetTarget(_) => unreachable!(),
                        ItemResponse::NewDrag(drag) => drag,
                    }),
                    target: None,
                },
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

                    // `contains_pointer` does not work for the target because only the dragged items'
                    // response will contain the pointer.
                    let slot = ui
                        .ctx()
                        .pointer_latest_pos()
                        // the hover includes the outer_rect?
                        .filter(|p| min_rect.contains(*p))
                        .map(|p| self.slot(p - min_rect.min));

                    let fits = slot
                        .map(|slot| self.fits(ctx, drag, slot))
                        .unwrap_or_default();

                    // Paint the dragged item's shadow, showing which slots will be filled.
                    if let Some(slot) = slot {
                        let color = self.shadow_color(accepts, fits, ui);
                        let shape = &drag.item.shape;
                        let mesh = shape_mesh(&shape, min_rect, self.pos(slot), color, SLOT_SIZE);
                        ui.painter().set(shadow, mesh);
                    }

                    // Only send target on release?
                    let released = ui.input(|i| i.pointer.any_released());
                    if released && fits && !accepts {
                        tracing::info!(
                            "container {:?} does not accept item {:?}!",
                            ctx.container_id,
                            drag.item.flags,
                        );
                    }

                    MoveData {
                        // So this gets merged?
                        drag: None,
                        target: slot
                            .filter(|_| accepts && fits)
                            .map(|slot| (ctx.container_id, slot)),
                    }
                }
                _ => MoveData::default(),
            };

            InnerResponse::new(move_data, response)
        })
    }
}

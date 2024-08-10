use egui::style::WidgetVisuals;

use super::*;

#[derive(Clone, Debug)]
pub struct GridContents {
    pub header: Option<String>,
    pub size: shape::Vec2,
    pub flags: ItemFlags,
}

impl GridContents {
    pub fn new(size: impl Into<shape::Vec2>) -> Self {
        Self {
            header: None,
            size: size.into(),
            flags: ItemFlags::all(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<ItemFlags>) -> Self {
        self.flags = flags.into();
        self
    }

    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.header = Some(header.into());
        self
    }

    pub fn grid_size(&self) -> egui::Vec2 {
        (self.size.as_vec2() * SLOT_SIZE).as_ref().into()
    }

    // Grid lines shape.
    pub fn shape(&self, style: &egui::Style) -> egui::Shape {
        let stroke1 = style.visuals.widgets.noninteractive.bg_stroke;
        let mut stroke2 = stroke1.clone();
        stroke2.color = tint_color_towards(stroke1.color, style.visuals.extreme_bg_color);
        let stroke2 = egui::epaint::PathStroke::from(stroke2);

        let size = self.grid_size();
        let egui::Vec2 { x: w, y: h } = size;

        let mut lines = vec![];

        // Don't draw the outside edge.
        lines.extend((1..(self.size.x)).map(|x| {
            let x = x as f32 * SLOT_SIZE;
            egui::Shape::LineSegment {
                points: [egui::Pos2::new(x, 0.0), egui::Pos2::new(x, h)],
                stroke: stroke2.clone(),
            }
        }));

        lines.extend((1..(self.size.y)).map(|y| {
            let y = y as f32 * SLOT_SIZE;
            egui::Shape::LineSegment {
                points: [egui::Pos2::new(0.0, y), egui::Pos2::new(w, y)],
                stroke: stroke2.clone(),
            }
        }));

        lines.push(egui::Shape::Rect(egui::epaint::RectShape::new(
            egui::Rect::from_min_size(egui::Pos2::ZERO, size),
            style.visuals.widgets.noninteractive.rounding,
            // style.visuals.window_rounding,
            Color32::TRANSPARENT,
            // style.visuals.window_fill,
            stroke1,
        )));

        egui::Shape::Vec(lines)
    }
}

fn update_state<T: 'static + Clone + Send + Sync>(
    ctx: &egui::Context,
    id: egui::Id,
    mut f: impl FnMut(T) -> T,
) {
    if let Some(t) = ctx.data(|d| d.get_temp::<T>(id)) {
        ctx.data_mut(|d| d.insert_temp(id, f(t)));
    }
}

// There is no get_temp_mut... If the shape doesn't exist we don't
// care since it will be regenerated next time the container is shown.
fn add_shape(ctx: &egui::Context, id: egui::Id, slot: usize, shape: &Shape) {
    update_state(ctx, id, |mut fill: Shape| {
        fill.paint(shape, slot);
        fill
    })
}

fn remove_shape(ctx: &egui::Context, id: egui::Id, slot: usize, shape: &Shape) {
    update_state(ctx, id, |mut fill: Shape| {
        fill.unpaint(shape, slot);
        fill
    })
}

impl Contents for GridContents {
    fn len(&self) -> usize {
        self.size.element_product() as usize
    }

    // `slot` is remapped for sections. The target is not...
    fn add(&self, _ctx: &Context, slot: LocalSlot) -> Option<ResolveFn> {
        Some(Box::new(move |ctx, drag, (.., eid)| {
            add_shape(ctx, eid, slot.0, &drag.item.shape())
        }))
    }

    fn remove(&self, ctx: &Context, slot: LocalSlot, shape: shape::Shape) -> Option<ResolveFn> {
        let container_eid = ctx.container_eid;
        Some(Box::new(move |ctx, _, _| {
            remove_shape(ctx, container_eid, slot.0, &shape)
        }))
    }

    fn pos(&self, slot: LocalSlot) -> egui::Vec2 {
        xy(slot.0, self.size.x as usize) * SLOT_SIZE
    }

    fn slot(&self, p: egui::Vec2) -> LocalSlot {
        LocalSlot(slot(p, self.size.x as usize))
    }

    fn accepts(&self, item: &Item) -> bool {
        self.flags.contains(item.flags)
    }

    fn fits(
        &self,
        &Context {
            container_eid: eid, ..
        }: &Context,
        ctx: &egui::Context,
        drag: &DragItem,
        slot: LocalSlot,
    ) -> bool {
        // Must be careful with the type inference here since it will
        // never fetch anything if it thinks it's a reference.
        match ctx.data(|d| d.get_temp::<Shape>(eid)) {
            Some(shape) => {
                // Check if the shape fits here. When moving within
                // one container, use the cached shape with the
                // dragged item (and original rotation) unpainted.
                let shape = match (drag.container.2 == eid, &drag.cshape) {
                    // (true, None) should never happen...
                    (true, Some(shape)) => shape,
                    _ => &shape,
                };

                shape.fits(&drag.item.shape(), slot.0)
            }
            None => {
                // TODO remove this
                tracing::error!("shape {:?} not found!", eid);
                false
            }
        }
    }

    fn find_slot(
        &self,
        ctx: &Context,
        egui_ctx: &egui::Context,
        item: &DragItem,
        items: Items,
    ) -> Option<(Entity, usize, egui::Id)> {
        let new_shape = || {
            items.into_iter().fold(
                Shape::new(self.size, false),
                |mut shape, ((slot, _), (_, item))| {
                    shape.paint(&item.shape(), slot - ctx.slot_offset);
                    shape
                },
            )
        };

        // Prime the container shape. Normally `body` does this. This is here so we can call `fits`,
        // which requires a filled shape, before we draw the contents (drag to item).
        egui_ctx.data_mut(|d| _ = d.get_temp_mut_or_insert_with(ctx.container_eid, new_shape));

        find_slot_default(self, ctx, egui_ctx, item, items)
    }

    fn body(
        &self,
        ctx: &Context,
        drag_item: &Option<DragItem>,
        items: Items,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<ItemResponse>> {
        // Allocate the full grid size. Note ui.min_rect() may differ from from the allocated rect
        // due to layout. So position items based on the latter.

        let (rect, response) = ui.allocate_exact_size(self.grid_size(), egui::Sense::hover());

        let new_drag = if ui.is_rect_visible(rect) {
            let &Context {
                container_id: id,
                container_eid: eid,
                slot_offset: offset,
            } = ctx;
            let grid_shape = ui.painter().add(egui::Shape::Noop);

            // Skip this if the container is empty? Only if dragging into
            // this container? Only if visible? What if we are dragging to
            // a container w/o the contents visible/open? Is it possible
            // to have an empty shape without a bitvec allocated until
            // painted?  [`fits`] also checks the boundaries even if the
            // container is empty...
            let mut fill = false;
            let mut shape = ui.data(|d| d.get_temp::<Shape>(eid)).unwrap_or_else(|| {
                // We don't need to fill if we aren't dragging currently...
                fill = true;
                shape::Shape::new(self.size, false)
            });

            // Debug container "shape", AKA filled slots.
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
                        rect,
                        egui::Vec2::ZERO,
                        Color32::DARK_BLUE,
                        SLOT_SIZE,
                    ));
                }
            }

            let new_drag = items
                .iter()
                .map(|&((slot, id), (name, item))| {
                    let local_slot = LocalSlot(slot - offset);

                    // If this item is being dragged, we want to use the dragged rotation.
                    // Everything else should be the same.
                    let (dragged, item) = drag::item!(drag_item, id, item);

                    // Paint each item and fill our shape if needed.
                    if !dragged && fill {
                        shape.paint(&item.shape(), local_slot.0);
                    }

                    let item_rect = egui::Rect::from_min_size(
                        rect.min + self.pos(local_slot),
                        if dragged {
                            // Only allocate the slot otherwise we'll blow out the contents if it
                            // doesn't fit.
                            slot_size()
                        } else {
                            item.size_rotated()
                        },
                    );

                    // item returns a clone if it's being dragged
                    ui.allocate_ui_at_rect(item_rect, |ui| item.ui(id, name, drag_item, ui))
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
                    let local_slot = LocalSlot(slot - offset);
                    match item {
                        ItemResponse::NewDrag(drag_id, item) => {
                            // The dragged item shape is already rotated. We
                            // clone it to retain the original rotation for
                            // removal. FIX:?
                            let item_shape = item.shape();
                            let mut cshape = shape.clone();
                            // We've already cloned the item and we're cloning
                            // the shape again to rotate? Isn't it already rotated?
                            cshape.unpaint(&item_shape, local_slot.0);
                            ItemResponse::Drag(DragItem {
                                id: drag_id,
                                item,
                                // FIX just use ctx?
                                container: (id, slot, eid),
                                cshape: Some(cshape),
                                remove_fn: self.remove(ctx, local_slot, item_shape),
                            })
                        }
                        // Update the slot.
                        ItemResponse::Hover((_slot, id, item)) => {
                            ItemResponse::Hover((local_slot, id, item))
                        }
                        _ => item,
                    }
                });

            let mut grid = self.shape(ui.style());
            grid.translate(rect.min.to_vec2());
            ui.painter().set(grid_shape, grid);

            // Write out the new shape.
            if fill {
                ui.data_mut(|d| d.insert_temp(eid, shape));
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
        // TODO: fetch in body?
        items: Items<'_>,
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
                match self.header.as_ref() {
                    Some(header) => _ = ui.label(header),
                    _ => (),
                }
                crate::min_frame::min_frame(ui, add_contents)
            })
            .inner
        };

        // Go back to with_bg/min_frame since egui::Frame takes up all available space.
        header_frame(ui, |style: &mut WidgetVisuals, ui: &mut egui::Ui| {
            // Reserve shape for the dragged item's shadow.
            let shadow = ui.painter().add(egui::Shape::Noop);

            let InnerResponse { inner, response } = self.body(ctx, drag_item, items, ui);
            let min_rect = response.rect;

            // TODO move everything into the match

            // If we are dragging onto another item, check to see if
            // the dragged item will fit anywhere within its contents.
            match (drag_item, inner.as_ref()) {
                (Some(drag), Some(ItemResponse::Hover((slot, id, item)))) => {
                    if let Some((contents, items)) = q.get(*id) {
                        let ctx = Context::from(*id);
                        let target = contents.0.find_slot(&ctx, ui.ctx(), drag, &items);

                        // The item shadow becomes the target item, not the dragged item, for
                        // drag-to-item. TODO just use rect
                        let color = self.shadow_color(true, target.is_some(), ui);
                        let mut mesh = egui::Mesh::default();
                        mesh.add_colored_rect(
                            egui::Rect::from_min_size(
                                min_rect.min + self.pos(*slot),
                                item.size_rotated(),
                            ),
                            color,
                        );
                        ui.painter().set(shadow, mesh);

                        return InnerResponse::new(
                            MoveData {
                                drag: None,
                                target, //: (id, slot, eid),
                                add_fn: target.and_then(|(_, slot, ..)| {
                                    contents.0.add(&ctx, ctx.local_slot(slot))
                                }),
                            },
                            response,
                        );
                    }
                }
                _ => (),
            }

            // tarkov also checks if containers are full, even if not
            // hovering -- maybe track min size free? TODO just do
            // accepts, and only check fits for hover
            let dragging = drag_item.is_some();

            let mut move_data = MoveData {
                drag: match inner {
                    // TODO NewDrag?
                    Some(ItemResponse::Drag(drag)) => Some(drag),
                    _ => None,
                },
                ..Default::default()
            };

            if !dragging {
                return InnerResponse::new(move_data, response);
            }

            let accepts = drag_item
                .as_ref()
                .map(|drag| self.accepts(&drag.item))
                .unwrap_or_default();

            // Highlight the contents border if we can accept the dragged item.
            if accepts {
                // TODO move this to settings?
                style.bg_stroke = ui.visuals().widgets.hovered.bg_stroke;
            }

            // `contains_pointer` does not work for the target because only the dragged items'
            // response will contain the pointer.
            let slot = ui
                .ctx()
                .pointer_latest_pos()
                // the hover includes the outer_rect?
                .filter(|p| min_rect.contains(*p))
                .map(|p| self.slot(p - min_rect.min));

            let Context {
                container_id: id,
                container_eid: eid,
                ..
            } = *ctx;

            let fits = drag_item
                .as_ref()
                .zip(slot)
                .map(|(item, slot)| self.fits(ctx, ui.ctx(), item, slot))
                .unwrap_or_default();

            // Paint the dragged item's shadow, showing which slots will
            // be filled.
            if let Some((drag, slot)) = drag_item.as_ref().zip(slot) {
                let color = self.shadow_color(accepts, fits, ui);
                // Use the rotated shape.
                let shape = drag.item.shape();
                let mesh = shape_mesh(&shape, min_rect, self.pos(slot), color, SLOT_SIZE);
                ui.painter().set(shadow, mesh);
            }

            // Only send target on release?
            let released = ui.input(|i| i.pointer.any_released());
            if released && fits && !accepts {
                tracing::info!(
                    "container {:?} does not accept item {:?}!",
                    id,
                    drag_item.as_ref().map(|drag| drag.item.flags)
                );
            }

            // accepts ⇒ dragging, fits ⇒ dragging, fits ⇒ slot

            match slot {
                Some(slot) if accepts && fits => {
                    // The target eid is unused?
                    // dbg!(slot.0, ctx.slot_offset);
                    move_data.target = Some((id, slot.0 + ctx.slot_offset, eid));
                    move_data.add_fn = self.add(ctx, slot);
                }
                _ => (),
            }
            InnerResponse::new(move_data, response)
        })
    }
}

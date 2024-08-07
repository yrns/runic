use super::*;

#[derive(Clone, Debug)]
pub struct GridContents {
    pub size: shape::Vec2,
    pub flags: ItemFlags,
}

impl GridContents {
    pub fn new(size: impl Into<shape::Vec2>) -> Self {
        Self {
            size: size.into(),
            flags: ItemFlags::all(),
        }
    }

    pub fn with_flags(mut self, flags: impl Into<ItemFlags>) -> Self {
        self.flags = flags.into();
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
    fn add(&self, _ctx: Context, slot: usize) -> Option<ResolveFn> {
        Some(Box::new(move |ctx, drag, (.., eid)| {
            add_shape(ctx, eid, slot, &drag.item.shape())
        }))
    }

    fn remove(&self, (_, eid, _): Context, slot: usize, shape: shape::Shape) -> Option<ResolveFn> {
        Some(Box::new(move |ctx, _, _| {
            remove_shape(ctx, eid, slot, &shape)
        }))
    }

    fn pos(&self, slot: usize) -> egui::Vec2 {
        xy(slot, self.size.x as usize) * SLOT_SIZE
    }

    fn slot(&self, p: egui::Vec2) -> usize {
        slot(p, self.size.x as usize)
    }

    fn accepts(&self, item: &Item) -> bool {
        self.flags.contains(item.flags)
    }

    fn fits(
        &self,
        (_, eid, _): Context,
        ctx: &egui::Context,
        drag: &DragItem,
        slot: usize,
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

                shape.fits(&drag.item.shape(), slot)
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
        ctx: Context,
        egui_ctx: &egui::Context,
        item: &DragItem,
        items: &[(usize, Item)],
    ) -> Option<(usize, usize, egui::Id)> {
        let new_shape = || {
            items
                .into_iter()
                .fold(Shape::new(self.size, false), |mut shape, (slot, item)| {
                    shape.paint(&item.shape(), *slot);
                    shape
                })
        };

        // Prime the container shape. Normally `body` does this. This is here so we can call `fits`,
        // which requires a filled shape, before we draw the contents (drag to item).
        egui_ctx.data_mut(|d| _ = d.get_temp_mut_or_insert_with(ctx.1, new_shape));

        find_slot_default(self, ctx, egui_ctx, item, items)
    }

    fn body(
        &self,
        ctx: Context,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<ItemResponse>> {
        // Allocate the full grid size. Note ui.min_rect() may differ from from the allocated rect
        // due to layout. So position items based on the latter.

        let (rect, response) = ui.allocate_exact_size(self.grid_size(), egui::Sense::hover());

        let new_drag = if ui.is_rect_visible(rect) {
            let (id, eid, offset) = ctx;
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
                .map(|(slot, item)| {
                    let slot = slot - offset;

                    // If this item is being dragged, we want to use the dragged rotation.
                    // Everything else should be the same.
                    let (dragged, item) = drag::item!(drag_item, item);

                    // Paint each item and fill our shape if needed.
                    if !dragged && fill {
                        shape.paint(&item.shape(), slot);
                    }

                    let item_rect = egui::Rect::from_min_size(
                        rect.min + self.pos(slot),
                        if dragged {
                            // Only allocate the slot otherwise we'll blow out the contents if it
                            // doesn't fit.
                            slot_size()
                        } else {
                            item.size_rotated()
                        },
                    );

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
                    // let slot = slot - offset;
                    match item {
                        ItemResponse::NewDrag(item) => {
                            // The dragged item shape is already rotated. We
                            // clone it to retain the original rotation for
                            // removal.
                            let item_shape = item.shape();
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
}

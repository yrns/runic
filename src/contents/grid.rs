use crate::*;

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

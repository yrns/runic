use crate::*;

// A sectioned container is a set of smaller containers displayed as
// one. Like pouches on a belt or different pockets in a jacket. It's
// one item than holds many fixed containers.
#[derive(Clone, Debug)]
pub struct SectionContents {
    pub layout: SectionLayout,
    // This should be generic over Contents but then ContentsLayout
    // will cycle on itself.
    pub sections: Vec<ContentsLayout>,
}

#[derive(Clone)]
pub enum SectionLayout {
    Grid(usize),
    // Fixed(Vec<(usize, egui::Pos2))
    // Columns?
    // This isn't clonable...
    //Other(Box<dyn Fn(&mut egui::Ui) -> ...>),
    Other(fn(&mut egui::Ui) -> InnerResponse<MoveData>),
}

impl std::fmt::Debug for SectionLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Grid(cols) => write!(f, "Grid({})", cols),
            Self::Other(_) => write!(f, "Other(...)"),
        }
    }
}

impl SectionContents {
    pub fn new(layout: SectionLayout, sections: Vec<ContentsLayout>) -> Self {
        Self { layout, sections }
    }

    fn section_slot(&self, slot: usize) -> Option<(usize, usize)> {
        self.section_ranges()
            .enumerate()
            .find_map(|(i, (start, end))| (slot < end).then(|| (i, slot - start)))
    }

    fn section_ranges(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        let mut end = 0;
        self.sections.iter().map(move |s| {
            let start = end;
            end = end + s.len();
            (start, end)
        })
    }

    fn section_eid(&self, (_id, eid): Context, sid: usize) -> egui::Id {
        egui::Id::new(eid.with("section").with(sid))
    }

    // (ctx, slot) -> (section, section ctx, section slot)
    fn section(&self, ctx: Context, slot: usize) -> Option<(&ContentsLayout, Context, usize)> {
        self.section_slot(slot)
            .map(|(i, slot)| (&self.sections[i], (ctx.0, self.section_eid(ctx, i)), slot))
    }

    fn section_items<'a, I>(
        &self,
        items: I,
    ) -> impl Iterator<Item = (usize, ContentsLayout, usize, Vec<(usize, &'a Item)>)>
    where
        I: IntoIterator<Item = (usize, &'a Item)>,
    {
        // map (slot, item) -> (section, (slot, item))
        let ranges = self.section_ranges().collect_vec();

        // If we know the input is sorted there is probably a way to
        // do this w/o collecting into a hash map.
        let mut items = items
            .into_iter()
            // Find section for each item.
            .filter_map(|(slot, item)| {
                ranges
                    .iter()
                    .enumerate()
                    .find_map(|(section, (start, end))| {
                        (slot < *end).then(|| (section, ((slot - start), item)))
                    })
            })
            .into_group_map();

        // TODO should be a way to do this without cloning sections
        self.sections
            .clone()
            .into_iter()
            .zip(ranges.into_iter())
            .enumerate()
            .map(move |(i, (layout, (start, _end)))| {
                (i, layout, start, items.remove(&i).unwrap_or_default())
            })
    }
}

// pub struct SectionItems<I> {
//     curr: usize,
//     // keep a ref to section contents or clone sections?
//     items: itertools::GroupingMap<I>,
// }

impl Contents for SectionContents {
    fn len(&self) -> usize {
        self.sections.iter().map(|s| s.len()).sum()
    }

    // Forward to section.
    fn add(&self, ctx: Context, slot: usize) -> Option<ResolveFn> {
        self.section(ctx, slot)
            .and_then(|(a, ctx, slot)| a.add(ctx, slot))
    }

    fn remove(&self, ctx: Context, slot: usize, shape: shape::Shape) -> Option<ResolveFn> {
        self.section(ctx, slot)
            .and_then(|(a, ctx, slot)| a.remove(ctx, slot, shape))
    }

    fn pos(&self, _slot: usize) -> egui::Vec2 {
        todo!()
    }

    fn slot(&self, _offset: egui::Vec2) -> usize {
        todo!()
    }

    /// Returns true if any section can accept this item.
    fn accepts(&self, item: &Item) -> bool {
        self.sections.iter().any(|a| a.accepts(item))
    }

    // Unused. We can only fit things in sections.
    fn fits(
        &self,
        _ctx: Context,
        _egui_ctx: &egui::Context,
        _item: &DragItem,
        _slot: usize,
    ) -> bool {
        false
    }

    fn find_slot<'a, I>(
        &self,
        ctx: Context,
        egui_ctx: &egui::Context,
        item: &DragItem,
        items: I,
        // id, slot, ...
    ) -> Option<(usize, usize, egui::Id)>
    where
        I: IntoIterator<Item = (usize, &'a Item)>,
        Self: Sized,
    {
        self.section_items(items)
            .find_map(|(i, layout, start, items)| {
                let ctx = (ctx.0, self.section_eid(ctx, i));
                layout
                    .find_slot(ctx, egui_ctx, item, items)
                    .map(|(id, slot, eid)| (id, (slot + start), eid))
            })
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
        let id = ctx.0;

        match self.layout {
            SectionLayout::Grid(width) => {
                egui::Grid::new(id).num_columns(width).show(ui, |ui| {
                    self.section_items(items)
                        .map(|(i, layout, start, items)| {
                            let data = layout
                                .ui((id, self.section_eid(ctx, i)), q, drag_item, items, ui)
                                .inner;

                            if (i + 1) % width == 0 {
                                ui.end_row();
                            }

                            // Remap slots. Only if we are the subject
                            // of the drag or target. Nested contents
                            // will have a different id.
                            data.map_slots(id, |slot| slot + start)
                        })
                        .reduce(|acc, a| acc.merge(a))
                        .unwrap_or_default()
                })
            }
            SectionLayout::Other(f) => f(ui),
        }
    }
}

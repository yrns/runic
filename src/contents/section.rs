use std::ops::Range;

// use itertools::Itertools;

use crate::*;

// A sectioned container is a set of smaller containers displayed as one. Like pouches on a belt or
// different pockets in a jacket. It's one item than holds many fixed containers. It is considered
// one container, so we have to remap slots from the subcontents to the main container.
pub struct SectionContents {
    pub layout: SectionLayout,
    // This should be generic over Contents but then ContentsLayout
    // will cycle on itself.
    pub sections: Vec<BoxedContents>,
}

#[derive(Clone)]
pub enum SectionLayout {
    // Number of columns...
    Grid(usize),
    Horizontal,
    Vertical,
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
            Self::Vertical => write!(f, "Vertical"),
            Self::Horizontal => write!(f, "Horizontal"),
            Self::Other(_) => write!(f, "Other(...)"),
        }
    }
}

impl SectionContents {
    pub fn new(layout: SectionLayout, sections: Vec<BoxedContents>) -> Self {
        Self { layout, sections }
    }

    /// Returns (section index, section slot) for `slot`.
    fn section_slot(&self, slot: usize) -> Option<(usize, usize)> {
        self.section_ranges()
            .enumerate()
            .find_map(|(i, r)| (slot < r.end).then(|| (i, slot - r.start)))
    }

    fn section_ranges(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        let mut end = 0;
        self.sections.iter().map(move |s| {
            let start = end;
            end = end + s.len();
            start..end
        })
    }

    fn section_eid(&self, (_id, eid, _): Context, sid: usize) -> egui::Id {
        egui::Id::new(eid.with("section").with(sid))
    }

    // (ctx, slot) -> (section, section ctx, section slot)
    fn section(
        &self,
        ctx: Context,
        slot: usize,
    ) -> Option<(&(dyn Contents + Send + Sync), Context, usize)> {
        self.section_slot(slot).map(|(i, slot)| {
            (
                self.sections[i].as_ref(),
                (ctx.0, self.section_eid(ctx, i), ctx.2),
                slot,
            )
        })
    }

    // TODO: enforce sortedness w/ https://github.com/rklaehn/sorted-iter or our own items collection type
    fn split_items<'a>(
        &'a self,
        offset: usize,
        mut items: &'a [(usize, Item)],
    ) -> impl Iterator<Item = (usize, &'a [(usize, Item)])> {
        let mut ranges = self.section_ranges();

        // This is more complicated than it needs to be, but we want to catch the assertions.
        std::iter::from_fn(move || {
            let Some(r) = ranges.next() else {
                assert_eq!(
                    items.len(),
                    0,
                    "all items inside section ranges (is `items` sorted by slot?)"
                );
                return None;
            };

            // `split_once` is nightly...
            let l = items
                .iter()
                .position(|(slot, _)| (slot - offset) >= r.end)
                .unwrap_or_else(|| items.len());
            let (head, tail) = items.split_at(l);

            items = tail;

            // TODO `is_sorted` is nightly...
            assert!(
                head.iter().all(|(slot, _)| r.contains(&(slot - offset))),
                "item slot in range (is `items` sorted by slot?)"
            );
            Some((r.start, head))
        })
    }

    fn section_items<'a>(
        &'a self,
        offset: usize,
        items: &'a [(usize, Item)],
    ) -> impl Iterator<Item = (&(dyn Contents + Send + Sync), usize, &'a [(usize, Item)])> {
        self.split_items(offset, items)
            .zip(&self.sections)
            .map(|((start, items), l)| (l.as_ref(), start, items))
    }
}

#[allow(unused)]
fn split_lengths<'a, T>(
    mut slice: &'a [T],
    lens: impl IntoIterator<Item = usize> + 'a,
) -> impl Iterator<Item = &[T]> + 'a {
    lens.into_iter().map(move |l| {
        let (head, tail) = slice.split_at(l);
        slice = tail;
        head
    })
}

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

    // Never called.
    fn pos(&self, _slot: usize) -> egui::Vec2 {
        unimplemented!()
    }

    // Never called.
    fn slot(&self, _offset: egui::Vec2) -> usize {
        unimplemented!()
    }

    // Never called.
    fn accepts(&self, _item: &Item) -> bool {
        // self.sections.iter().any(|a| a.accepts(item)))
        unimplemented!()
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

    // Add start slot to find_slot or move slot into item. We need to modify the slot without the item reference being changed.
    fn find_slot(
        &self,
        ctx: Context,
        egui_ctx: &egui::Context,
        item: &DragItem,
        items: &[(usize, Item)],
        // id, slot, ...
    ) -> Option<(usize, usize, egui::Id)> {
        self.section_items(ctx.2, items)
            .enumerate()
            .find_map(|(i, (layout, offset, items))| {
                let ctx = (ctx.0, self.section_eid(ctx, i), offset);
                layout
                    .find_slot(ctx, egui_ctx, item, items)
                    .map(|(id, slot, eid)| (id, (slot + offset), eid))
            })
    }

    fn ui(
        &self,
        ctx: Context,
        q: &ContentsStorage,
        drag_item: &Option<DragItem>,
        items: &[(usize, Item)],
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<MoveData> {
        let id = ctx.0;

        // if !items.is_empty() {
        //     items
        //         .iter()
        //         .for_each(|(slot, item)| print!("[{} {} {}] ", slot, item.id, item.name));
        //     println!("offset: {}", ctx.2);
        // }

        match self.layout {
            SectionLayout::Grid(width) => {
                egui::Grid::new(id).num_columns(width).show(ui, |ui| {
                    self.section_items(ctx.2, items)
                        .enumerate()
                        .map(|(i, (layout, offset, items))| {
                            let data = layout
                                .ui(
                                    (id, self.section_eid(ctx, i), offset + ctx.2),
                                    q,
                                    drag_item,
                                    items,
                                    ui,
                                )
                                .inner;

                            if (i + 1) % width == 0 {
                                ui.end_row();
                            }

                            // Remap slots. Only if we are the subject
                            // of the drag or target. Nested containers
                            // will have a different id.
                            data.map_slots(id, |slot| slot + offset)
                        })
                        .reduce(|acc, a| acc.merge(a))
                        .unwrap_or_default()
                })
            }
            SectionLayout::Vertical => ui.vertical(|ui| {
                self.section_items(ctx.2, items)
                    .enumerate()
                    .map(|(i, (contents, offset, items))| {
                        let section_ctx = (id, self.section_eid(ctx, i), offset + ctx.2);
                        let data = contents.ui(section_ctx, q, drag_item, items, ui).inner;
                        data.map_slots(id, |slot| slot + offset)
                    })
                    .reduce(|data, a| data.merge(a))
                    .unwrap_or_default()
            }),
            SectionLayout::Horizontal => ui.horizontal_top(|ui| {
                self.section_items(ctx.2, items)
                    .enumerate()
                    .map(|(i, (contents, offset, items))| {
                        let section_ctx = (id, self.section_eid(ctx, i), offset + ctx.2);
                        let data = contents.ui(section_ctx, q, drag_item, items, ui).inner;
                        data.map_slots(id, |slot| slot + offset)
                    })
                    .reduce(|data, a| data.merge(a))
                    .unwrap_or_default()
            }),
            SectionLayout::Other(f) => f(ui),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn split_lengths() {
        let a = [1, 2, 3, 4, 5];

        // let (x, y) = a.split_at(6.min(a.len()));
        // assert_eq!(x.len(), 5);
        // assert_eq!(y.len(), 0);
        // assert_eq!(a.len(), 5);

        let b: Vec<_> = super::split_lengths(&a, [2usize, 3].into_iter()).collect();
        assert_eq!(b, [&vec![1, 2], &vec![3, 4, 5]]);
    }
}

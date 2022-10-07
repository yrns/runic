use std::{collections::HashMap, path::PathBuf};

use eframe::egui;
use egui_extras::RetainedImage;
use flagset::FlagSet;
use runic::*;

use bitvec::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "runic",
        options,
        Box::new(|cc| {
            let mut contents = HashMap::new();
            let mut images = HashMap::new();

            let boomerang = Item::new(
                3,
                load_image(&mut images, "boomerang").texture_id(&cc.egui_ctx),
                shape::Shape::from_bits(2, bits![1, 1, 1, 0]),
            )
            // this item is a weapon
            .with_flags(ItemFlags::Weapon);

            let pouch = Item::new(
                8,
                load_image(&mut images, "pouch").texture_id(&cc.egui_ctx),
                shape::Shape::new((2, 2), true),
            )
            // this item is a container
            .with_flags(ItemFlags::Container);

            let short_sword = Item::new(
                9,
                load_image(&mut images, "short-sword").texture_id(&cc.egui_ctx),
                shape::Shape::new((3, 1), true),
            )
            // this item is a weapon
            .with_flags(ItemFlags::Weapon);

            let potion = Item::new(
                4,
                load_image(&mut images, "potion").texture_id(&cc.egui_ctx),
                shape::Shape::new((1, 1), true),
            )
            .with_flags(ItemFlags::Potion);

            contents.insert(
                8,
                (
                    GridContents::new(8, (3, 2))
                        // it only holds potions
                        .with_flags(ItemFlags::Potion)
                        .into(),
                    vec![],
                ),
            );

            contents.insert(
                1,
                (
                    // this id is redundant
                    GridContents::new(1, (4, 4))
                        // accepts any item
                        .with_flags(FlagSet::full())
                        .into(),
                    vec![(0, boomerang), (2, pouch), (8, short_sword)],
                ),
            );

            contents.insert(
                2,
                (
                    GridContents::new(2, (2, 2))
                        // accepts only potions
                        .with_flags(ItemFlags::Potion)
                        .into(),
                    vec![(0, potion)],
                ),
            );

            contents.insert(
                5,
                (
                    SectionContents::new(
                        5,
                        // grid w/ two columns
                        SectionLayout::Grid(2),
                        vec![(1, 2).into(), (1, 2).into()],
                    )
                    // accepts any item
                    .with_flags(FlagSet::full())
                    .into(),
                    vec![],
                ),
            );

            contents.insert(
                6,
                (
                    ExpandingContents::new(6, (2, 2))
                        // accepts only weapons
                        .with_flags(ItemFlags::Weapon)
                        .into(),
                    vec![],
                ),
            );

            contents.insert(
                7,
                (
                    ContentsLayout::Inline(
                        ExpandingContents::new(7, (2, 2))
                            // we only accept containers
                            .with_flags(ItemFlags::Container),
                    ),
                    vec![],
                ),
            );

            Box::new(Runic {
                images,
                drag_item: None,
                contents,
            })
        }),
    )
}

//#[derive(Default)]
struct Runic {
    #[allow(dead_code)]
    images: HashMap<&'static str, RetainedImage>,
    drag_item: Option<DragItem>,
    contents: HashMap<usize, (ContentsLayout, Vec<(usize, Item)>)>,
}

fn load_image<'a>(
    images: &'a mut HashMap<&'static str, RetainedImage>,
    name: &'static str,
) -> &'a RetainedImage {
    images.entry(name).or_insert_with(|| {
        let mut path = PathBuf::from(name);
        path.set_extension("png");
        std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|buf| RetainedImage::from_image_bytes(name, &buf))
            .unwrap_or_else(|e| {
                tracing::error!("failed to load image at: {} ({})", path.display(), e);
                RetainedImage::from_color_image(name, egui::ColorImage::example())
            })
    })
}

impl eframe::App for Runic {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let move_data = ContainerSpace::show(&mut self.drag_item, ui, |drag_item, ui| {
                ui.label("Grid contents 4x4:");
                let data = match self.contents.get(&1) {
                    Some((ContentsLayout::Grid(layout), items)) => {
                        // Option on items not needed anymore?
                        layout.ui(drag_item, Some(items), ui).inner
                    }
                    _ => Default::default(),
                };

                ui.label("Grid contents 2x2:");
                let data = match self.contents.get(&2) {
                    Some((ContentsLayout::Grid(layout), items)) => {
                        data.merge(layout.ui(drag_item, Some(items), ui).inner)
                    }
                    _ => data,
                };

                ui.label("Section contents 2x1x2:");
                let data = match self.contents.get(&5) {
                    Some((ContentsLayout::Section(layout), items)) => {
                        data.merge(layout.ui(drag_item, Some(items), ui).inner)
                    }
                    _ => data,
                };

                ui.label("Expanding container 2x2:");
                let data = match self.contents.get(&6) {
                    Some((ContentsLayout::Expanding(layout), items)) => {
                        data.merge(layout.ui(drag_item, Some(items), ui).inner)
                    }
                    _ => data,
                };

                ui.label("Inline contents 2x2:");
                let data = match self.contents.get(&7) {
                    Some((ContentsLayout::Inline(layout), items)) => {
                        // get the layout and contents of the
                        // contained item (if any)
                        let (inline_layout, inline_items) = match items
                            .get(0)
                            .map(|(_, item)| item.id)
                            .and_then(|id| self.contents.get(&id))
                        {
                            // this is unzip
                            Some((a, b)) => (Some(a), Some(b)),
                            None => (None, None),
                        };

                        data.merge(
                            InlineContents::new(
                                layout,
                                // FIX this mess
                                inline_layout.map(|layout| match layout {
                                    ContentsLayout::Expanding(_) => todo!(),
                                    ContentsLayout::Inline(_) => todo!(),
                                    ContentsLayout::Grid(g) => g,
                                    ContentsLayout::Section(_) => todo!(),
                                }),
                            )
                            .ui(drag_item, Some(items), inline_items, ui)
                            .inner,
                        )
                    }
                    _ => data,
                };

                data
            });

            if let Some(move_data) = move_data {
                let mut resolve = false;
                if let MoveData {
                    drag: Some(ref drag),
                    target: Some((container, slot)),
                    ..
                } = move_data
                {
                    tracing::info!("moving item {:?} -> container {:?}", drag.item, container);

                    // Using indexmap or something else to get two mutable
                    // refs would make this transactable.
                    match self
                        .contents
                        .get_mut(&drag.container.0)
                        .and_then(|(_, items)| {
                            let idx = items.iter().position(|(_, item)| item.id == drag.item.id);
                            idx.map(|idx| items.remove(idx).1)
                        }) {
                        Some(mut item) => {
                            tracing::info!(
                                "new rot {:?} --> {:?}",
                                item.rotation,
                                drag.item.rotation
                            );
                            // Copy the rotation.
                            item.rotation = drag.item.rotation;

                            // Insert item. The contents must exist
                            // already to insert an item?
                            match self.contents.get_mut(&container) {
                                Some((_, items)) => items.push((slot, item)),
                                None => tracing::error!(
                                    "could not find container {} to add to",
                                    container
                                ),
                            }

                            resolve = true;
                        }
                        None => tracing::error!(
                            "could not find container {} to remove from",
                            drag.container.0
                        ),
                    }
                }
                if resolve {
                    move_data.resolve(ctx);
                }
            }
        });
    }
}

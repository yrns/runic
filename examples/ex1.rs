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

            contents.insert(
                1,
                vec![(
                    0,
                    Item::new(
                        3,
                        load_image(&mut images, "boomerang").texture_id(&cc.egui_ctx),
                        shape::Shape::from_bits(2, bits![1, 1, 1, 0]),
                    )
                    // this item is a weapon
                    .with_flags(ItemFlags::Weapon),
                )],
            );

            contents.insert(
                2,
                vec![(
                    0,
                    Item::new(
                        4,
                        load_image(&mut images, "potion").texture_id(&cc.egui_ctx),
                        shape::Shape::new((1, 1), true),
                    )
                    // this item is a a container
                    .with_flags(ItemFlags::Container | ItemFlags::Potion)
                    // this item only accepts potions
                    .with_cflags(ItemFlags::Potion),
                )],
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
    contents: HashMap<usize, Vec<(usize, Item)>>,
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
                let data = GridContents::new(1, (4, 4), self.contents.get(&1))
                    // accepts any item, maybe this should be the default
                    .with_flags(FlagSet::full())
                    .ui(drag_item, ui)
                    .inner;

                ui.label("Grid contents 2x2:");
                let data = data.merge(
                    GridContents::new(2, (2, 2), self.contents.get(&2))
                        // accepts only potions
                        .with_flags(ItemFlags::Potion)
                        .ui(drag_item, ui)
                        .inner,
                );

                ui.label("Section contents 2x1x2:");
                let data = data.merge(
                    SectionContainer::new(
                        5,
                        SectionLayout::Grid(2),
                        vec![(1, 2).into(), (1, 2).into()],
                        self.contents.get(&5),
                    ) // accepts any item
                    .with_flags(FlagSet::full())
                    .ui(drag_item, ui)
                    .inner,
                );

                ui.label("Expanding container 2x2:");
                let data = data.merge(
                    ExpandingContainer::new(6, (2, 2), self.contents.get(&6))
                        // accepts any item
                        .with_flags(FlagSet::full())
                        .ui(drag_item, ui)
                        .inner,
                );

                ui.label("Inline contents 2x2:");
                let contents = self.contents.get(&7);
                let inline_contents = contents
                    .and_then(|items| items.get(0))
                    .map(|item| item.1.id);
                let data = data.merge(
                    InlineContents::new(
                        ExpandingContainer::new(7, (2, 2), contents)
                            // we only accept containers
                            .with_flags(ItemFlags::Container),
                        // TODO the layout of contents is fixed here,
                        // but should depend on the item somehow
                        inline_contents.map(|id| {
                            GridContents::new(id, (3, 2), self.contents.get(&id))
                                // accepts any item
                                .with_flags(FlagSet::full())
                        }),
                    )
                    .ui(drag_item, ui)
                    .inner,
                );

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
                    match self.contents.get_mut(&drag.container.0).and_then(|items| {
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

                            // Insert item.
                            self.contents
                                .entry(container)
                                .or_insert_with(Vec::new)
                                .push((slot, item));
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

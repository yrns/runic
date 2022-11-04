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
            // let mut next_id = {
            //     let mut id = 0;
            //     move || {
            //         id += 1;
            //         id
            //     }
            // };

            let mut contents = HashMap::new();
            let mut images = HashMap::new();

            let boomerang = Item::new(
                3, //next_id(),
                load_image(&mut images, "boomerang").texture_id(&cc.egui_ctx),
                shape::Shape::from_bits(2, bits![1, 1, 1, 0]),
            )
            // this item is a weapon
            .with_flags(ItemFlags::Weapon)
            .with_name("Boomerang");

            let pouch = Item::new(
                8,
                load_image(&mut images, "pouch").texture_id(&cc.egui_ctx),
                shape::Shape::new((2, 2), true),
            )
            // this item is a container
            .with_flags(ItemFlags::Container)
            .with_name("Pouch");

            let short_sword = Item::new(
                9,
                load_image(&mut images, "short-sword").texture_id(&cc.egui_ctx),
                shape::Shape::new((3, 1), true),
            )
            // this item is a weapon
            .with_flags(ItemFlags::Weapon)
            //.with_rotation(ItemRotation::R90);
            .with_name("Short sword");

            let potion = Item::new(
                4,
                load_image(&mut images, "potion").texture_id(&cc.egui_ctx),
                shape::Shape::new((1, 1), true),
            )
            .with_flags(ItemFlags::Potion)
            .with_name("Potion");

            let potion2 = potion.clone().with_id(10).with_name("Potion 2");

            contents.insert(
                8,
                (
                    GridContents::new((3, 2))
                        // it only holds potions
                        .with_flags(ItemFlags::Potion)
                        // checking cycles
                        //.with_flags(FlagSet::full())
                        .into(),
                    vec![],
                ),
            );

            contents.insert(
                1,
                (
                    // this id is redundant
                    GridContents::new((4, 4))
                        // accepts any item
                        .with_flags(FlagSet::full())
                        .into(),
                    vec![(0, boomerang), (2, pouch), (8, short_sword)],
                ),
            );

            contents.insert(
                2,
                (
                    GridContents::new((2, 2))
                        // accepts only potions
                        .with_flags(ItemFlags::Potion)
                        .into(),
                    vec![(0, potion), (1, potion2)],
                ),
            );

            contents.insert(
                5,
                (
                    SectionContents::new(
                        SectionLayout::Grid(3),
                        vec![
                            GridContents::new((1, 2))
                                .with_flags(FlagSet::full()) // accepts any item
                                .into(),
                            GridContents::new((1, 2))
                                .with_flags(FlagSet::full()) // accepts any item
                                .into(),
                            GridContents::new((1, 2))
                                .with_flags(FlagSet::full()) // accepts any item
                                .into(),
                        ],
                    )
                    .into(),
                    vec![],
                ),
            );

            contents.insert(
                6,
                (
                    ExpandingContents::new((2, 2))
                        // accepts only weapons
                        .with_flags(ItemFlags::Weapon)
                        .into(),
                    vec![],
                ),
            );

            contents.insert(
                7,
                (
                    InlineContents::new(
                        ExpandingContents::new((2, 2))
                            // we only accept containers
                            .with_flags(ItemFlags::Container),
                    )
                    .into(),
                    vec![],
                ),
            );

            Box::new(Runic {
                images,
                drag_item: None,
                contents: ContentsStorage(contents),
            })
        }),
    )
}

struct ContentsStorage(HashMap<usize, (ContentsLayout, Vec<(usize, Item)>)>);

//#[derive(Default)]
struct Runic {
    #[allow(dead_code)]
    images: HashMap<&'static str, RetainedImage>,
    drag_item: Option<DragItem>,
    contents: ContentsStorage,
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

// impl<'a> ContentsQuery<'a> for ContentsStorage {
//     type Items = ???
//     fn query(&self, id: usize) -> Option<(&ContentsLayout, Self::Items)> {
//         self.0
//             .get(&id)
//             .map(|(c, i)| (c, i.iter().map(|(slot, item)| (*slot, item))))
//     }
// }

impl eframe::App for Runic {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let drag_item = &mut self.drag_item;
            let q = &self.contents;
            let q = |id: usize| {
                q.0.get(&id)
                    .map(|(c, i)| (c, i.iter().map(|(slot, item)| (*slot, item))))
            };

            let move_data = ContainerSpace::show(drag_item, ui, |drag_item, ui| {
                ui.label("Grid contents 4x4:");
                let data = show_contents(&q, 1, drag_item, ui).unwrap().inner;

                ui.label("Grid contents 2x2:");
                let data = data.merge(show_contents(&q, 2, drag_item, ui).unwrap().inner);

                ui.label("Section contents 2x1x2:");
                let data = data.merge(show_contents(&q, 5, drag_item, ui).unwrap().inner);

                ui.label("Expanding container 2x2:");
                let data = data.merge(show_contents(&q, 6, drag_item, ui).unwrap().inner);

                ui.label("Inline contents 2x2:");
                let data = data.merge(show_contents(&q, 7, drag_item, ui).unwrap().inner);

                data
            });

            if let Some(move_data) = move_data {
                let mut resolve = false;
                if let MoveData {
                    drag: Some(ref drag),
                    target: Some((container, slot, _eid)),
                    ..
                } = move_data
                {
                    // In lieu of an efficient way to do an exhaustive check for cycles:
                    if drag.item.id == container {
                        tracing::info!("cannot move an item inside itself: {}", drag.item.id);
                    } else {
                        tracing::info!(
                            "moving item {} {:?} -> container {} slot {}",
                            drag.item.id,
                            drag.item.rotation,
                            container,
                            slot
                        );

                        // Using indexmap or something else to get two mutable
                        // refs would make this transactable.
                        match self
                            .contents
                            .0
                            .get_mut(&drag.container.0)
                            .and_then(|(_, items)| {
                                let idx =
                                    items.iter().position(|(_, item)| item.id == drag.item.id);
                                idx.map(|idx| items.remove(idx).1)
                            }) {
                            Some(mut item) => {
                                //tracing::info!("new rot {:?} --> {:?}", item.rotation, drag.item.rotation);

                                // Copy the rotation.
                                item.rotation = drag.item.rotation;

                                // Insert item. The contents must exist
                                // already to insert an item?
                                match self.contents.0.get_mut(&container) {
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
                }
                if resolve {
                    move_data.resolve(ctx);
                }
            }
        });
    }
}

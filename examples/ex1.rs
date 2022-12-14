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
            let mut next_id = {
                let mut id = 0;
                move || {
                    id += 1;
                    id
                }
            };

            let mut contents = HashMap::new();
            let mut images = HashMap::new();

            let boomerang = Item::new(
                next_id(),
                load_image(&mut images, "boomerang").texture_id(&cc.egui_ctx),
                shape::Shape::from_bits(2, bits![1, 1, 1, 0]),
            )
            // this item is a weapon
            .with_flags(ItemFlags::Weapon)
            .with_name("Boomerang");

            let pouch = Item::new(
                next_id(),
                load_image(&mut images, "pouch").texture_id(&cc.egui_ctx),
                shape::Shape::new((2, 2), true),
            )
            // this item is a container
            .with_flags(ItemFlags::Container)
            .with_name("Pouch");

            let short_sword = Item::new(
                next_id(),
                load_image(&mut images, "short-sword").texture_id(&cc.egui_ctx),
                shape::Shape::new((3, 1), true),
            )
            // this item is a weapon
            .with_flags(ItemFlags::Weapon)
            //.with_rotation(ItemRotation::R90);
            .with_name("Short sword");

            let potion = Item::new(
                next_id(),
                load_image(&mut images, "potion").texture_id(&cc.egui_ctx),
                shape::Shape::new((1, 1), true),
            )
            .with_flags(ItemFlags::Potion)
            .with_name("Potion");

            let potion2 = potion.clone().with_id(next_id()).with_name("Potion 2");

            let paper_doll_id = next_id();
            let paper_doll = SectionContents::new(
                SectionLayout::Grid(1),
                vec![
                    HeaderContents::new(
                        "Bag of any! 4x4:",
                        GridContents::new((4, 4))
                            // accepts any item
                            .with_flags(FlagSet::full()),
                    )
                    .into(),
                    HeaderContents::new(
                        "Only potions! 2x2:",
                        GridContents::new((2, 2))
                            // accepts only potions
                            .with_flags(ItemFlags::Potion),
                    )
                    .into(),
                    HeaderContents::new(
                        "Weapon here:",
                        ExpandingContents::new((2, 2))
                            // accepts only weapons
                            .with_flags(ItemFlags::Weapon),
                    )
                    .into(),
                    HeaderContents::new(
                        "Section contents 3x1x2:",
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
                        ),
                    )
                    .into(),
                    HeaderContents::new(
                        "Holds a container:",
                        InlineContents::new(
                            ExpandingContents::new((2, 2))
                                // we only accept containers
                                .with_flags(ItemFlags::Container),
                        ),
                    )
                    .into(),
                ],
            );
            contents.insert(paper_doll_id, (paper_doll.into(), vec![]));

            let pouch_contents = SectionContents::new(
                SectionLayout::Grid(4),
                std::iter::repeat(
                    GridContents::new((1, 1))
                        .with_flags(ItemFlags::Potion)
                        .into(),
                )
                .take(4)
                .collect(),
            );
            contents.insert(pouch.id, (pouch_contents.into(), vec![]));

            let ground_id = next_id();
            let ground = GridContents::new((10, 10)).with_flags(FlagSet::full());
            contents.insert(
                ground_id,
                (
                    ground.into(),
                    vec![
                        (0, boomerang),
                        (2, pouch),
                        (4, short_sword),
                        (7, potion),
                        (8, potion2),
                    ],
                ),
            );

            Box::new(Runic {
                images,
                drag_item: None,
                contents: ContentsStorage(contents),
                paper_doll_id,
                ground_id,
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
    paper_doll_id: usize,
    ground_id: usize,
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
                let data = MoveData::default();

                let data = ui.columns(2, |cols| {
                    cols[0].label("Paper doll:");
                    let data = data.merge(
                        show_contents(&q, self.paper_doll_id, drag_item, &mut cols[0])
                            .unwrap()
                            .inner,
                    );

                    cols[1].label("Ground 10x10:");
                    let data = data.merge(
                        show_contents(&q, self.ground_id, drag_item, &mut cols[1])
                            .unwrap()
                            .inner,
                    );
                    data
                });

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

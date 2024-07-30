use eframe::egui;
use flagset::FlagSet;
use runic::*;

use bitvec::prelude::*;

fn main() -> eframe::Result {
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

            egui_extras::install_image_loaders(&cc.egui_ctx);

            macro_rules! load_icon {
                ($path:literal) => {{
                    let icon = egui::include_image!($path).load(
                        &cc.egui_ctx,
                        egui::TextureOptions::default(),
                        egui::SizeHint::default(),
                    )?;

                    match icon {
                        egui::load::TexturePoll::Ready { texture } => texture.id,
                        _ => panic!("no image"),
                    }
                }};
            }

            let boomerang = Item::new(
                next_id(),
                load_icon!("../boomerang.png"),
                Shape::from_bits(2, bits![1, 1, 1, 0]),
            )
            // this item is a weapon
            .with_flags(ItemFlags::Weapon)
            .with_name("Boomerang");

            let pouch = Item::new(
                next_id(),
                load_icon!("../pouch.png"),
                Shape::new((2, 2), true),
            )
            // this item is a container
            .with_flags(ItemFlags::Container)
            .with_name("Pouch");

            let short_sword = Item::new(
                next_id(),
                load_icon!("../short-sword.png"),
                Shape::new((3, 1), true),
            )
            // this item is a weapon
            .with_flags(ItemFlags::Weapon)
            //.with_rotation(ItemRotation::R90);
            .with_name("Short sword");

            let potion = Item::new(
                next_id(),
                load_icon!("../potion.png"),
                Shape::new((1, 1), true),
            )
            .with_flags(ItemFlags::Potion)
            .with_name("Potion");

            let potion2 = potion.clone().with_id(next_id()).with_name("Potion 2");

            // Setup containers. It's important to note here that there are only three containers,
            // the paper doll, the ground, and the pouch. Sectioned contents is one container split
            // into many sections.
            let paper_doll_id = next_id();
            let paper_doll = SectionContents::new(
                SectionLayout::Grid(1),
                vec![
                    HeaderContents::new(
                        "Bag of any! 4x4:",
                        GridContents::new((4, 4)).with_flags(FlagSet::full()), // accepts any item
                    )
                    .boxed(),
                    HeaderContents::new(
                        "Only potions! 2x2:",
                        GridContents::new((2, 2)).with_flags(ItemFlags::Potion), // accepts only potions
                    )
                    .boxed(),
                    HeaderContents::new(
                        "Weapon (2x2 MAX):",
                        ExpandingContents::new((2, 2)).with_flags(ItemFlags::Weapon), // accepts only weapons
                    )
                    .boxed(),
                    HeaderContents::new(
                        "Section contents 3x1x2:",
                        SectionContents::new(
                            SectionLayout::Grid(3),
                            std::iter::repeat(
                                GridContents::new((1, 2)).with_flags(FlagSet::full()), // accepts any item
                            )
                            .take(3)
                            .map(Contents::boxed)
                            .collect(),
                        ),
                    )
                    .boxed(),
                    HeaderContents::new(
                        "Holds a container:",
                        InlineContents::new(
                            ExpandingContents::new((2, 2)).with_flags(ItemFlags::Container), // we only accept containers
                        ),
                    )
                    .boxed(),
                ],
            );

            let mut contents = ContentsStorage::new();
            contents.insert(paper_doll_id, (paper_doll.boxed(), vec![]));

            let pouch_contents = SectionContents::new(
                SectionLayout::Grid(4),
                std::iter::repeat(GridContents::new((1, 1)).with_flags(ItemFlags::Potion))
                    .take(4)
                    .map(Contents::boxed)
                    .collect(),
            );
            contents.insert(pouch.id, (Box::new(pouch_contents), vec![]));

            let ground_id = next_id();
            let ground = GridContents::new((10, 10)).with_flags(FlagSet::full());
            contents.insert(
                ground_id,
                (
                    Box::new(ground),
                    // MUST BE SORTED BY SLOT
                    vec![
                        (0, boomerang),
                        (2, pouch),
                        (4, short_sword),
                        (7, potion),
                        (8, potion2),
                    ],
                ),
            );

            Ok(Box::new(Runic {
                drag_item: None,
                contents,
                paper_doll_id,
                ground_id,
            }))
        }),
    )
}

//#[derive(Default)]
struct Runic {
    drag_item: Option<DragItem>,
    contents: ContentsStorage,
    paper_doll_id: usize,
    ground_id: usize,
}

impl eframe::App for Runic {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let drag_item = &mut self.drag_item;
            let q = &self.contents;

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
                                match self.contents.get_mut(&container) {
                                    Some((_, items)) => {
                                        // Items must be ordered by slot in order for section contents to work.
                                        let i = items
                                            .binary_search_by_key(&slot, |&(slot, _)| slot)
                                            .expect_err("item slot free");
                                        items.insert(i, (slot, item));
                                    }
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

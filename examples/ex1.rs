use bevy::{prelude::*, window::RequestRedraw, winit::WinitSettings};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiUserTextures};
use egui::Ui;
use runic::*;

#[derive(Resource)]
struct Runic {
    drag_item: Option<DragItem>,
    contents: ContentsStorage,
    paper_doll_id: usize,
    ground_id: usize,
}

fn main() {
    App::new()
        .insert_resource(WinitSettings::default())
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        // .insert_resource(Runic::new())
        .add_systems(Startup, setup)
        .add_systems(Update, update)
        // .add_systems(
        //     Last,
        //     redraw
        //         //.run_if(on_event::<AssetEvent<Image>>())
        //         .after(Assets::<Image>::asset_events),
        // )
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut textures: ResMut<EguiUserTextures>,
) {
    commands.insert_resource(Runic::new(&*asset_server, &mut *textures));
}

// This isn't actually reliable.
#[allow(unused)]
fn redraw(mut events: EventReader<AssetEvent<Image>>, mut redraw: EventWriter<RequestRedraw>) {
    for _e in events.read() {
        // dbg!(e);
        redraw.send(RequestRedraw);
    }
}

fn update(mut contexts: EguiContexts, mut runic: ResMut<Runic>, mut _move_data: Local<MoveData>) {
    //egui::CentralPanel::default().show(ctx, |ui| {});
    egui::Window::new("runic - ex1").show(contexts.ctx_mut(), |ui| {
        runic.update(ui); //, &mut *move_data);
    });
}

impl Runic {
    fn new(asset_server: &AssetServer, textures: &mut EguiUserTextures) -> Self {
        let mut next_id = {
            let mut id = 0;
            move || {
                id += 1;
                id
            }
        };

        let boomerang = Item::new(next_id(), ItemFlags::Weapon)
            .with_icon(textures.add_image(asset_server.load("boomerang.png")))
            .with_shape(Shape::from_ones(2, [1, 1, 1, 0]))
            .with_name("Boomerang");

        let pouch = Item::new(next_id(), ItemFlags::Container)
            .with_icon(textures.add_image(asset_server.load("pouch.png")))
            .with_shape((2, 2))
            .with_name("Pouch");

        let short_sword = Item::new(next_id(), ItemFlags::Weapon)
            .with_icon(textures.add_image(asset_server.load("short-sword.png")))
            .with_shape((3, 1))
            .with_rotation(ItemRotation::R90)
            .with_name("Short sword");

        let potion = Item::new(next_id(), ItemFlags::Potion)
            .with_icon(textures.add_image(asset_server.load("potion.png")))
            .with_shape((1, 1))
            .with_name("Potion");

        let potion2 = potion.clone().with_id(next_id()).with_name("Potion 2");

        // Setup containers. It's important to note here that there are only three containers,
        // the paper doll, the ground, and the pouch. Sectioned contents is one container split
        // into many sections.
        let paper_doll_id = next_id();
        let paper_doll = SectionContents::new(
            SectionLayout::Grid(1),
            vec![
                // accepts any item
                HeaderContents::new("Bag of any! 4x4:", GridContents::new((4, 4))).boxed(),
                HeaderContents::new(
                    "Only potions! 2x2:",
                    GridContents::new((2, 2)).with_flags(ItemFlags::Potion),
                )
                .boxed(),
                HeaderContents::new(
                    "Weapon (3x2 MAX):",
                    ExpandingContents::new((3, 2)).with_flags(ItemFlags::Weapon),
                )
                .boxed(),
                HeaderContents::new(
                    "Section contents 3x1x2:",
                    SectionContents::new(
                        SectionLayout::Grid(3),
                        vec![
                            GridContents::new((1, 2)).boxed(),
                            GridContents::new((1, 2)).boxed(),
                            // the last section only accepts weapons
                            GridContents::new((1, 2))
                                .with_flags(ItemFlags::Weapon)
                                .boxed(),
                        ],
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
            SectionLayout::Grid(2),
            vec![
                // Test inline contents with differing flags...
                GridContents::new((3, 2))
                    .with_flags(ItemFlags::Weapon)
                    .boxed(),
                GridContents::new((1, 1))
                    .with_flags(ItemFlags::Potion)
                    .boxed(),
            ],
        );
        contents.insert(pouch.id, (pouch_contents.boxed(), vec![]));

        let ground_id = next_id();
        let ground = GridContents::new((10, 10));
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

        Runic {
            drag_item: None,
            contents,
            paper_doll_id,
            ground_id,
        }
    }

    fn update(&mut self, ui: &mut Ui) {
        // No go.
        //let drag_item = ui.ctx().data_mut(|d| d.get_temp_mut_or_default(ui.id()));

        let drag_item = &mut self.drag_item;
        let q = &self.contents;

        let move_data = ContainerSpace::show(drag_item, ui, |drag_item, ui| {
            let data = MoveData::default();

            let data = ui.columns(2, |cols| {
                cols[0].label("Paper doll:");
                let data = data.merge(
                    show_contents(q, self.paper_doll_id, drag_item, &mut cols[0])
                        .unwrap()
                        .inner,
                );

                cols[1].label("Ground 10x10:");
                let data = data.merge(
                    show_contents(q, self.ground_id, drag_item, &mut cols[1])
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
                            let idx = items.iter().position(|(_, item)| item.id == drag.item.id);
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
                move_data.resolve(ui.ctx());
            }
        }
    }
}

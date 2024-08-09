use bevy::{prelude::*, window::RequestRedraw, winit::WinitSettings};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiUserTextures};
use egui::Ui;
use runic::*;

#[derive(Resource)]
struct Runic {
    drag_item: Option<DragItem>,
    paper_doll: Entity,
    ground: Entity,
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
    let runic = Runic::new(&mut commands, &*asset_server, &mut *textures);
    commands.insert_resource(runic);
}

// This isn't actually reliable.
#[allow(unused)]
fn redraw(mut events: EventReader<AssetEvent<Image>>, mut redraw: EventWriter<RequestRedraw>) {
    for _e in events.read() {
        // dbg!(e);
        redraw.send(RequestRedraw);
    }
}

fn update(
    mut contexts: EguiContexts,
    mut runic: ResMut<Runic>,
    contents: ContentsStorage,
    // mut _move_data: Local<MoveData>,
) {
    //egui::CentralPanel::default().show(ctx, |ui| {});
    egui::Window::new("runic - ex1")
        .resizable(false)
        .show(contexts.ctx_mut(), |ui| {
            runic.update(contents, ui); //, &mut *move_data);
        });
}

impl Runic {
    fn new(
        commands: &mut Commands,
        asset_server: &AssetServer,
        textures: &mut EguiUserTextures,
    ) -> Self {
        let boomerang = commands
            .spawn((
                Item::new(ItemFlags::Weapon)
                    .with_icon(textures.add_image(asset_server.load("boomerang.png")))
                    .with_shape(Shape::from_ones(2, [1, 1, 1, 0])),
                Name::from("Boomerang"),
            ))
            .id();

        let pouch = commands
            .spawn((
                Item::new(ItemFlags::Container)
                    .with_icon(textures.add_image(asset_server.load("pouch.png")))
                    .with_shape((2, 2)),
                Name::from("Pouch"),
            ))
            .id();

        let short_sword = commands
            .spawn((
                Item::new(ItemFlags::Weapon)
                    .with_icon(textures.add_image(asset_server.load("short-sword.png")))
                    .with_shape((3, 1))
                    .with_rotation(ItemRotation::R90),
                Name::from("Short sword"),
            ))
            .id();

        let potion = Item::new(ItemFlags::Potion)
            .with_icon(textures.add_image(asset_server.load("potion.png")))
            .with_shape((1, 1));

        let potion1 = commands
            .spawn((potion.clone(), Name::from("Potion 1")))
            .id();
        let potion2 = commands.spawn((potion, Name::from("Potion 2"))).id();

        // Setup containers. It's important to note here that there are only three containers,
        // the paper doll, the ground, and the pouch. Sectioned contents is one container split
        // into many sections.
        let paper_doll = SectionContents::new(
            SectionLayout::Vertical,
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
                        SectionLayout::Horizontal,
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

        let paper_doll = commands
            .spawn((ContentsLayout(paper_doll.boxed()), ContentsItems(vec![])))
            .id();

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
        commands.entity(pouch).insert((
            ContentsLayout(pouch_contents.boxed()),
            ContentsItems(vec![]),
        ));

        let ground = commands
            .spawn((
                ContentsLayout(GridContents::new((10, 10)).boxed()),
                ContentsItems(vec![
                    // MUST BE SORTED BY SLOT
                    (0, boomerang),
                    (2, pouch),
                    (4, short_sword),
                    (7, potion1),
                    (8, potion2),
                ]),
            ))
            .id();

        Runic {
            drag_item: None,
            paper_doll,
            ground,
        }
    }

    fn update(&mut self, mut q: ContentsStorage, ui: &mut Ui) {
        // No go.
        //let drag_item = ui.ctx().data_mut(|d| d.get_temp_mut_or_default(ui.id()));

        let drag_item = &mut self.drag_item;

        let move_data = ContainerSpace::show(drag_item, ui, |drag_item, ui| {
            let data = MoveData::default();

            let data = ui.columns(2, |cols| {
                cols[0].label("Paper doll:");
                let data = data.merge(
                    q.show_contents(self.paper_doll, drag_item, &mut cols[0])
                        .unwrap()
                        .inner,
                );

                cols[1].label("Ground 10x10:");
                let data = data.merge(
                    q.show_contents(self.ground, drag_item, &mut cols[1])
                        .unwrap()
                        .inner,
                );
                data
            });

            data
        });

        if let Some(data) = move_data {
            q.resolve_move(data, ui.ctx())
        }
    }
}

use bevy::{prelude::*, window::RequestRedraw, winit::WinitSettings};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiUserTextures};
use runic::*;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Flags: u32 {
        const Weapon = 1;
        const Armor = 1 << 1;
        const Potion = 1 << 2;
        const TradeGood = 1 << 3;
        const Container = 1 << 4;
    }
}

#[derive(Resource)]
struct PaperDoll(Entity);

#[derive(Resource)]
struct Ground(Entity);

fn main() {
    App::new()
        .insert_resource(WinitSettings::default())
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        .add_systems(Startup, (setup_items, insert_items).chain())
        .add_systems(Update, update)
        // .add_systems(
        //     Last,
        //     redraw
        //         //.run_if(on_event::<AssetEvent<Image>>())
        //         .after(Assets::<Image>::asset_events),
        // )
        .run();
}

// This isn't actually reliable.
#[allow(unused)]
fn redraw(mut events: EventReader<AssetEvent<Image>>, mut redraw: EventWriter<RequestRedraw>) {
    for _e in events.read() {
        // dbg!(e);
        redraw.send(RequestRedraw);
    }
}

// We do this first so that we can call ContentStorage::insert in `insert_items`.
fn setup_items(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut textures: ResMut<EguiUserTextures>,
) {
    // TODO plugin
    commands.insert_resource(Options::default());

    let _boomerang = commands
        .spawn((
            Item::new(Flags::Weapon)
                .with_icon(textures.add_image(asset_server.load("boomerang.png")))
                .with_shape(Shape::from_ones(2, [1, 1, 1, 0])),
            Name::from("Boomerang"),
        ))
        .id();

    let pouch = commands
        .spawn((
            Item::new(Flags::Container)
                .with_icon(textures.add_image(asset_server.load("pouch.png")))
                .with_shape((2, 2)),
            Name::from("Pouch"),
        ))
        .id();

    let potion_section1 = commands
        .spawn((
            ContentsLayout(
                GridContents::<Flags>::new((1, 1))
                    .with_header("P1:")
                    .with_flags(Flags::Potion)
                    .boxed(),
            ),
            ContentsItems(vec![]),
        ))
        .id();

    let potion_section2 = commands
        .spawn((
            ContentsLayout(
                GridContents::<Flags>::new((1, 1))
                    .with_header("P2:")
                    .with_flags(Flags::Potion)
                    .boxed(),
            ),
            ContentsItems(vec![]),
        ))
        .id();

    commands.entity(pouch).insert((
        ContentsLayout(
            GridContents::<Flags>::new((3, 2))
                .with_header("Weapons:")
                .with_flags(Flags::Weapon)
                .boxed(),
        ),
        ContentsItems::default(),
        Sections(
            // This only works for sections, not the main container. So in this case, the main container will still be below the sections.
            Some(egui::Layout::left_to_right(egui::Align::Min)),
            vec![potion_section1, potion_section2],
        ),
    ));

    let _short_sword = commands
        .spawn((
            Item::new(Flags::Weapon)
                .with_icon(textures.add_image(asset_server.load("short-sword.png")))
                .with_shape((3, 1))
                .with_rotation(ItemRotation::R90),
            Name::from("Short sword"),
        ))
        .id();

    let potion = Item::new(Flags::Potion)
        .with_icon(textures.add_image(asset_server.load("potion.png")))
        .with_shape((1, 1));

    let _potion1 = commands
        .spawn((potion.clone(), Name::from("Potion 1")))
        .id();
    let _potion2 = commands.spawn((potion, Name::from("Potion 2"))).id();

    // Setup sections.
    let section1 = commands
        .spawn((
            ContentsLayout(
                GridContents::<Flags>::new((2, 2))
                    .with_header("Only potions! 2x2:")
                    .with_flags(Flags::Potion)
                    .boxed(),
            ),
            ContentsItems(vec![]),
        ))
        .id();

    let section2 = commands
        .spawn((
            ContentsLayout(
                GridContents::<Flags>::new((3, 2))
                    .with_expands(true)
                    .with_header("Weapon (3x2 MAX):")
                    .with_flags(Flags::Weapon)
                    .boxed(),
            ),
            ContentsItems(vec![]),
        ))
        .id();

    let sub_sections = [
        GridContents::<Flags>::new((1, 2)).with_header("A1"),
        GridContents::<Flags>::new((1, 2)).with_header("A2"),
        // the last section only accepts weapons
        GridContents::<Flags>::new((1, 2))
            .with_header("W1")
            .with_flags(Flags::Weapon),
    ]
    .map(|s| {
        commands
            .spawn((ContentsLayout(s.boxed()), ContentsItems::default()))
            .id()
    });

    let section3 = commands
        .spawn((
            ContentsLayout(
                GridContents::<Flags>::new((2, 2))
                    .with_header("Holds a container:")
                    .with_expands(true)
                    .with_inline(true)
                    .with_flags(Flags::Container)
                    .boxed(),
            ),
            ContentsItems(vec![]),
            Sections(None, sub_sections.to_vec()),
        ))
        .id();

    let paper_doll = commands
        .spawn((
            ContentsLayout(
                GridContents::<Flags>::new((4, 4))
                    .with_header("Bag of any! 4x4:")
                    .boxed(),
            ),
            ContentsItems(vec![]),
            Sections(
                Some(egui::Layout::top_down(egui::Align::Min)), // Center does not work.
                vec![section1, section2, section3],
            ),
        ))
        .id();

    let ground = commands
        .spawn((
            ContentsLayout(GridContents::<Flags>::new((10, 10)).boxed()),
            ContentsItems::default(),
        ))
        .id();

    commands.insert_resource(PaperDoll(paper_doll));
    commands.insert_resource(Ground(ground));
}

fn insert_items(
    items: Query<Entity, With<Item<Flags>>>,
    mut contents: ContentsStorage<Flags>,
    ground: Res<Ground>,
) {
    for item in &items {
        contents.insert(ground.0, item).expect("item fits");
    }
}

fn update(
    mut contexts: EguiContexts,
    mut contents: ContentsStorage<Flags>,
    paper_doll: Res<PaperDoll>,
    ground: Res<Ground>,
) {
    contents.update(contexts.ctx_mut());

    egui::Window::new("Paper doll:")
        .resizable(false)
        .movable(true)
        .max_width(512.0)
        .show(contexts.ctx_mut(), |ui| {
            contents.show(paper_doll.0, ui);
        });

    egui::Window::new("Ground 10x10:")
        .resizable(false)
        .movable(true)
        .show(contexts.ctx_mut(), |ui| {
            contents.show(ground.0, ui);
        });
}

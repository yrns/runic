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
        // TODO plugin
        .insert_resource(Options::default())
        .add_systems(Startup, setup_items)
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
    mut storage: ContentsStorage<Flags>,
) {
    // Spawn a bunch of items on the ground.
    let ground = storage.spawn(
        GridContents::<_>::new((10, 10))
            .builder()
            .with_name("Ground".into())
            .with_items([
                ContentsBuilder::item(
                    Item::new(Flags::Weapon)
                        .with_icon(textures.add_image(asset_server.load("boomerang.png")))
                        .with_shape(Shape::from_ones(2, [1, 1, 1, 0])),
                )
                .with_name("Boomerang".into()),
                ContentsBuilder::item(
                    Item::new(Flags::Container)
                        .with_icon(textures.add_image(asset_server.load("pouch.png")))
                        .with_shape((2, 2)),
                )
                .with_name("Pouch".into())
                .with_contents(
                    GridContents::<_>::new((3, 2))
                        .with_header("Weapons:")
                        .with_flags(Flags::Weapon),
                )
                // This only works for sections, not the main container. So in this case, the main container will still be below the sections.
                .with_section_layout(egui::Layout::left_to_right(egui::Align::Min))
                .with_sections([
                    GridContents::new((1, 1))
                        .with_header("P1:")
                        .with_flags(Flags::Potion),
                    GridContents::new((1, 1))
                        .with_header("P2:")
                        .with_flags(Flags::Potion),
                ]),
                ContentsBuilder::item(
                    Item::new(Flags::Weapon)
                        .with_icon(textures.add_image(asset_server.load("short-sword.png")))
                        .with_shape((3, 1))
                        .with_rotation(ItemRotation::R90),
                )
                .with_name(Name::from("Short sword")),
                ContentsBuilder::item(
                    Item::new(Flags::Potion)
                        .with_icon(textures.add_image(asset_server.load("potion.png")))
                        .with_shape((1, 1)),
                )
                .with_name(Name::from("Potion 1")),
                ContentsBuilder::item(
                    Item::new(Flags::Potion)
                        .with_icon(textures.add_image(asset_server.load("potion.png")))
                        .with_shape((1, 1)),
                )
                .with_name(Name::from("Potion 2")),
            ]),
    );

    // Setup paper doll sections.
    let sub_sections = [
        GridContents::new((1, 2)).with_header("A1"),
        GridContents::new((1, 2)).with_header("A2"),
        // the last section only accepts weapons
        GridContents::new((1, 2))
            .with_header("W1")
            .with_flags(Flags::Weapon),
    ];

    let sections = [
        GridContents::<_>::new((2, 2))
            .with_header("Only potions! 2x2:")
            .with_flags(Flags::Potion)
            .builder(),
        GridContents::<_>::new((3, 2))
            .with_expands(true)
            .with_header("Weapon (3x2 MAX):")
            .with_flags(Flags::Weapon)
            .builder(),
        GridContents::<_>::new((2, 2))
            .with_header("Holds a container:")
            .with_expands(true)
            .with_inline(true)
            .with_flags(Flags::Container)
            .builder()
            .with_sections(sub_sections),
    ];

    let paper_doll = storage.spawn(
        GridContents::<_>::new((4, 4))
            .with_header("Bag of any! 4x4:")
            .builder()
            .with_name("Paper doll".into())
            .with_section_layout(egui::Layout::top_down(egui::Align::Min)) // Center does not work.
            .with_sections(sections),
    );

    commands.insert_resource(PaperDoll(paper_doll));
    commands.insert_resource(Ground(ground));
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

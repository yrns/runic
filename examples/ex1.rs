use bevy::{
    prelude::*,
    window::{PrimaryWindow, RequestRedraw},
    winit::WinitSettings,
};
use bevy_egui::{egui, EguiContext, EguiPlugin, EguiUserTextures};
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
        .add_systems(Update, (item_icon_changed::<Flags>, update).chain())
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
                        .with_icon(asset_server.load("boomerang.png"))
                        .with_shape(Shape::from_ones(2, [1, 1, 1, 0])),
                )
                .with_name("Boomerang".into()),
                ContentsBuilder::item(
                    Item::new(Flags::Container)
                        .with_icon(asset_server.load("pouch.png"))
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
                        .with_icon(asset_server.load("short-sword.png"))
                        .with_shape((3, 1))
                        .with_rotation(ItemRotation::R90),
                )
                .with_name(Name::from("Short sword")),
                ContentsBuilder::item(
                    Item::new(Flags::Potion)
                        .with_icon(asset_server.load("potion.png"))
                        .with_shape((1, 1)),
                )
                .with_name(Name::from("Potion 1")),
                ContentsBuilder::item(
                    Item::new(Flags::Potion)
                        .with_icon(asset_server.load("potion.png"))
                        .with_shape((1, 1)),
                )
                .with_name(Name::from("Potion 2")),
                // ContentsBuilder::item(
                //     Item::new(Flags::TradeGood)
                //         .with_icon(textures.add_image(asset_server.load("artifact.png")))
                //         .with_shape((1, 1)),
                // )
                // .with_name(Name::from("Artifact")),
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

fn item_icon_changed<T: Accepts>(
    items: Query<&Item<T>, Changed<Item<T>>>,
    mut textures: ResMut<EguiUserTextures>,
) {
    for item in &items {
        _ = textures
            .image_id(&item.icon)
            .unwrap_or_else(|| textures.add_image(item.icon.clone_weak()));
    }
}

fn update(
    mut egui_ctx: Query<&mut EguiContext, With<PrimaryWindow>>,
    mut storage: ContentsStorage<Flags>,
    paper_doll: Res<PaperDoll>,
    ground: Res<Ground>,
) {
    let mut egui_ctx = egui_ctx.single_mut();
    let ctx = egui_ctx.get_mut();

    storage.update(ctx);

    // Control-clicking items in the inventory will send them to ground.
    *storage.target = Some(ground.0);

    egui::Window::new("Paper doll:")
        .resizable(false)
        .movable(true)
        .max_width(512.0)
        .anchor(egui::Align2::LEFT_TOP, egui::Vec2::splat(16.0))
        .show(ctx, |ui| {
            storage.show(paper_doll.0, ui);
        });

    // Control-clicking items on the ground will send them to the inventory.
    *storage.target = Some(paper_doll.0);

    egui::Window::new("Ground 10x10:")
        .resizable(false)
        .movable(true)
        .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-16.0, 16.0))
        .show(ctx, |ui| {
            storage.show(ground.0, ui);
        });
}

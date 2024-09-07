use bevy::{
    asset::AssetLoadFailedEvent,
    ecs::system::SystemId,
    prelude::*,
    tasks::IoTaskPool,
    window::{PrimaryWindow, RequestRedraw},
    winit::WinitSettings,
};
use bevy_egui::{
    egui::{self, Direction},
    EguiContext, EguiPlugin, EguiUserTextures,
};
use runic::*;
use serde::{Deserialize, Serialize};

// You can get flags to serialize with the reflect serialization if you derive reflect outside the bitflags! macro (and NOT use reflect_value) as described here (https://docs.rs/bitflags/latest/bitflags/#custom-derives). This serializes as a struct tuple containing a u32. If you use reflect_value you're pretty much required to implement Serialize yourself. The serde flag for bitflags enables the fancy serialization with flag names.
bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Deserialize, Serialize)]
    #[serde(transparent)]
    #[reflect_value(Hash, PartialEq, Debug, Deserialize, Serialize)]
    pub struct Flags: u32 {
        const Weapon = 1;
        const Armor = 1 << 1;
        const Potion = 1 << 2;
        const TradeGood = 1 << 3;
        const Container = 1 << 4;
    }
}

// TODO work this out, remove all from trait?
impl Default for Flags {
    fn default() -> Self {
        Self::all()
    }
}

// These need to be reflectable to be written to the contents scene, as well as the type registered. An alternative would be to show windows for all root level contents that aren't items.
#[derive(Resource, Reflect)]
#[reflect(Resource)]
struct PaperDoll(Entity);

#[derive(Resource, Reflect)]
#[reflect(Resource)]
struct Ground(Entity);

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum AppState {
    #[default]
    Loading,
    Running,
}

fn main() {
    App::new()
        .insert_resource(WinitSettings::default())
        .register_type::<PaperDoll>()
        .register_type::<Ground>()
        .add_plugins((DefaultPlugins, RunicPlugin::<Flags>::default()))
        .init_state::<AppState>()
        .add_plugins(EguiPlugin)
        // TODO plugin
        .insert_resource(Options::default())
        .add_systems(OnEnter(AppState::Loading), load_items)
        .add_systems(Update, wait_for_items.run_if(in_state(AppState::Loading)))
        .add_systems(
            Update,
            spawn_items
                .run_if(in_state(AppState::Loading))
                .run_if(on_event::<AssetLoadFailedEvent<DynamicScene>>()),
        )
        .add_systems(
            Update,
            (
                save_items,
                (item_icon_changed::<Flags>, update)
                    .chain()
                    .run_if(in_state(AppState::Running)),
            ),
        )
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

#[derive(Resource)]
struct SaveItems(SystemId);

const CONTENTS_FILE_PATH: &str = "contents.scn.ron";

fn load_items(mut commands: Commands, asset_server: Res<AssetServer>) {
    let id = commands.register_one_shot_system(save_items_scene);
    commands.insert_resource(SaveItems(id));

    commands.spawn((
        Name::new("contents scene"),
        DynamicSceneBundle {
            scene: asset_server.load(CONTENTS_FILE_PATH),
            ..default()
        },
    ));
}

fn wait_for_items(
    mut asset_events: EventReader<AssetEvent<DynamicScene>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for event in asset_events.read() {
        match event {
            AssetEvent::LoadedWithDependencies { id: _ } => {
                info!("contents loaded!");
                next_state.set(AppState::Running);
            }
            _ => (),
        }
    }
}

fn save_items(
    mut commands: Commands,
    save_items_system: Res<SaveItems>,
    input: Res<ButtonInput<KeyCode>>,
) {
    let ctrl = input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);

    if ctrl && input.just_pressed(KeyCode::KeyS) {
        info!("saving contents...");
        commands.run_system(save_items_system.0);
    }
}

fn save_items_scene(world: &mut World) {
    // Turn all icons to paths.
    let mut query = world.query::<&mut Icon>();
    for mut icon in query.iter_mut(world) {
        // TODO This makes the icons flicker.
        icon.to_path()
        // icon.bypass_change_detection().to_path()
    }

    let mut query =
        world.query_filtered::<Entity, Or<(With<Item<Flags>>, With<ContentsItems<Flags>>)>>();
    let scene = DynamicSceneBuilder::from_world(&world)
        // .deny_all_resources()
        .allow_resource::<Ground>()
        .allow_resource::<PaperDoll>()
        // Bevy does not serialize handles.
        // .deny::<Handle<Image>>()
        .extract_resources()
        .extract_entities(query.iter(&world))
        .build();

    assert!(!scene.resources.is_empty());

    let type_registry = world.resource::<AppTypeRegistry>();
    let type_registry = type_registry.read();
    let serialized_scene = scene
        .serialize(&type_registry)
        .expect("error serializing scene!");

    // info!("{}", serialized_scene);

    #[cfg(not(target_arch = "wasm32"))]
    IoTaskPool::get()
        .spawn(async move {
            std::fs::write(
                format!("assets/{CONTENTS_FILE_PATH}"),
                serialized_scene.as_bytes(),
            )
            .expect("error writing contents to file");
        })
        .detach();
}

fn spawn_items(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut storage: ContentsStorage<Flags>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    next_state.set(AppState::Running);

    // Spawn a bunch of items on the ground.
    let ground = storage.spawn(
        GridContents::<_>::new((10, 10))
            .builder()
            .with_name("Ground".into())
            .with_items([
                ContentsBuilder::item(
                    Item::new(Flags::Weapon).with_shape(Shape::from_ones(2, [1, 1, 1, 0])),
                )
                .with_icon(asset_server.load("boomerang.png"))
                .with_name("Boomerang".into()),
                ContentsBuilder::item(Item::new(Flags::Container).with_shape((2, 2)))
                    .with_icon(asset_server.load("pouch.png"))
                    .with_name("Pouch".into())
                    .with_contents(
                        GridContents::<_>::new((3, 2))
                            .with_header("Weapons:")
                            .with_flags(Flags::Weapon),
                    )
                    // This only works for sections, not the main container. So in this case, the main container will still be below the sections.
                    .with_section_layout(Layout::new(Direction::LeftToRight, false))
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
                        .with_shape((3, 1))
                        .with_rotation(ItemRotation::R90),
                )
                .with_icon(asset_server.load("short-sword.png"))
                .with_name(Name::from("Short sword")),
                ContentsBuilder::item(Item::new(Flags::Potion).with_shape((1, 1)))
                    .with_icon(asset_server.load("potion.png"))
                    .with_name(Name::from("Potion 1")),
                ContentsBuilder::item(Item::new(Flags::Potion).with_shape((1, 1)))
                    .with_icon(asset_server.load("potion.png"))
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
            .with_section_layout(Layout::new(Direction::TopDown, false))
            .with_sections(sections),
    );

    commands.insert_resource(PaperDoll(paper_doll));
    commands.insert_resource(Ground(ground));
}

fn item_icon_changed<T: Accepts>(
    mut items: Query<&mut Icon, Changed<Icon>>,
    mut textures: ResMut<EguiUserTextures>,
    asset_server: Res<AssetServer>,
) {
    for mut icon in &mut items {
        // if !matches!(icon.as_ref(), Icon::Path(_)) {
        //     continue;
        // }
        match std::mem::take(icon.as_mut()) {
            Icon::Path(p) => {
                let h = asset_server.load(p);
                *icon = Icon::Handle(h);
            }
            Icon::Handle(h) => *icon = Icon::Handle(h),
            Icon::None => continue,
        };

        // Add the icon to egui textures if it doesn't already exist.
        let h = icon.handle();
        _ = textures
            .image_id(&h)
            .unwrap_or_else(|| textures.add_image(h.clone_weak()));
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

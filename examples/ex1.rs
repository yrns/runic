use bevy::{
    asset::AssetLoadFailedEvent,
    ecs::{
        entity::MapEntities, reflect::ReflectMapEntities, resource::IsResource, system::SystemId,
    },
    prelude::*,
    tasks::IoTaskPool,
    window::RequestRedraw,
    winit::WinitSettings,
    world_serialization::DynamicWorld,
};
use bevy_egui::{
    egui::{self, Direction},
    EguiContexts, EguiPlugin, EguiPrimaryContextPass, EguiTextureHandle, EguiUserTextures,
};
use runic::*;
use serde::{Deserialize, Serialize};

// NOTE reflect_value is now #[reflect(opaque)]
// You can get flags to serialize with the reflect serialization if you derive reflect outside the bitflags! macro (and NOT use reflect_value) as described here (https://docs.rs/bitflags/latest/bitflags/#custom-derives). This serializes as a struct tuple containing a u32. If you use reflect_value you're pretty much required to implement Serialize yourself. The serde flag for bitflags enables the fancy serialization with flag names.
bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Deserialize, Serialize)]
    #[serde(transparent)]
    #[reflect(opaque)]
    #[reflect(Hash, PartialEq, Debug, Deserialize, Serialize)]
    pub struct Flags: u32 {
        const Weapon = 1;
        const Armor = 1 << 1;
        const Potion = 1 << 2;
        const TradeGood = 1 << 3;
        const Container = 1 << 4;
    }
}

// By default, containers can contain any item. The derived default (0) does not work well, see https://docs.rs/bitflags/latest/bitflags/index.html#zero-bit-flags. This is why items require flags.
impl Default for Flags {
    fn default() -> Self {
        Self::all()
    }
}

impl std::fmt::Display for Flags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names = self.iter_names().map(|(n, _)| n);
        f.write_str(&itertools::join(names, "|"))
    }
}

// FIX? The migration guide explicitly mentions that it is no longer necessary to add derive MapEntities for a resource (<https://bevy.org/learn/migration-guides/0-18-to-0-19/#miscellaneous>)...

// These need to be reflectable to be written to the contents scene, as well as the type registered. An alternative would be to show windows for all root level contents that aren't items. Or add a separate marker component(s).
#[derive(Debug, Resource, MapEntities, Reflect)]
#[reflect(Debug, Resource, MapEntities)]
struct PaperDoll(#[entities] Entity);

#[derive(Debug, Resource, MapEntities, Reflect)]
#[reflect(Debug, Resource, MapEntities)]
struct Ground(#[entities] Entity);

// Remembers which containers are opened.
#[derive(Component, Reflect)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
struct Open;

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
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, startup)
        .add_systems(OnEnter(AppState::Loading), load_items)
        .add_systems(Update, wait_for_items.run_if(in_state(AppState::Loading)))
        .add_systems(
            Update,
            spawn_items
                .run_if(in_state(AppState::Loading))
                .run_if(on_message::<AssetLoadFailedEvent<DynamicWorld>>),
        )
        .add_systems(
            EguiPrimaryContextPass,
            (item_icon_changed::<Flags>, update)
                .chain()
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(Update, save_items.run_if(in_state(AppState::Running)))
        // .add_systems(
        //     Last,
        //     redraw
        //         //.run_if(on_event::<AssetEvent<Image>>())
        //         .after(Assets::<Image>::asset_events),
        // )
        .add_observer(item_insert)
        .add_observer(item_remove)
        .add_observer(item_move)
        .add_observer(drag_start)
        // .observe(drag_end)
        .add_observer(drag_over)
        .add_observer(container_open)
        .run();
}

fn startup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn item_insert(
    event: On<ItemInsert>,
    mut commands: Commands,
    names: Query<Option<&Name>>,
    asset_server: Res<AssetServer>,
) -> Result {
    let insert = event.event();
    let [target, item] = names.get_many([event.event_target(), insert.item])?;
    let target = target.map(|n| n.as_str()).unwrap_or("section");
    info!(
        target,
        item = item.unwrap().as_str(),
        slot = insert.slot,
        "insert"
    );

    // PlaybackSettings::REMOVE? Or is ONCE fine?
    commands
        .entity(event.event_target())
        .insert(AudioPlayer::new(asset_server.load("sfx100v2_wood_03.ogg")))
        .remove::<AudioSink>();

    Ok(())
}

fn item_remove(event: On<ItemRemove>, names: Query<Option<&Name>>) -> Result {
    let remove = event.event();
    let [target, item] = names.get_many([event.event_target(), remove.item])?;
    let target = target.map(|n| n.as_str()).unwrap_or("section");
    info!(
        target,
        item = item.unwrap().as_str(),
        slot = remove.slot,
        "remove"
    );
    Ok(())
}

fn item_move(
    event: On<ItemMove>,
    mut commands: Commands,
    names: Query<Option<&Name>>,
    asset_server: Res<AssetServer>,
) -> Result {
    let moved = event.event();
    let [target, item] = names.get_many([event.event_target(), moved.item])?;
    let target = target.map(|n| n.as_str()).unwrap_or("section");
    info!(
        target,
        item = item.unwrap().as_str(),
        "move slot {} -> {}",
        moved.old_slot,
        moved.new_slot
    );

    commands
        .entity(event.event_target())
        .insert(AudioPlayer::new(asset_server.load("sfx100v2_wood_03.ogg")))
        .remove::<AudioSink>();

    Ok(())
}

fn drag_start(event: On<ItemDragStart>, mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .entity(event.event_target())
        .insert(AudioPlayer::new(asset_server.load("sfx100v2_wood_03.ogg")))
        .remove::<AudioSink>();
}

fn drag_over(
    event: On<ItemDragOver>,
    mut commands: Commands,
    names: Query<Option<&Name>>,
    asset_server: Res<AssetServer>,
) -> Result {
    let drag_over = event.event();
    let [target, item] = names.get_many([event.event_target(), drag_over.item])?;
    let target = target.map(|n| n.as_str()).unwrap_or("section");
    info!(
        target,
        item = item.unwrap().as_str(),
        slot = drag_over.slot,
        "drag over"
    );

    commands
        .entity(event.event_target())
        .insert(AudioPlayer::new(asset_server.load("sfx100v2_wood_03.ogg")))
        // This restarts the audio. Maybe we should detach first.
        .remove::<AudioSink>();

    Ok(())
}

fn container_open(
    event: On<ContainerOpen>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.entity(event.event_target()).insert(Open);

    commands
        .entity(event.event_target())
        .insert(AudioPlayer::new(asset_server.load("sfx100v2_wood_03.ogg")))
        .remove::<AudioSink>();
}

// This isn't actually reliable.
#[allow(unused)]
fn redraw(mut events: MessageReader<AssetEvent<Image>>, mut redraw: MessageWriter<RequestRedraw>) {
    for _e in events.read() {
        // dbg!(e);
        redraw.write(RequestRedraw);
    }
}

#[derive(Resource)]
struct SaveItems(SystemId);

const CONTENTS_FILE_PATH: &str = "contents.scn.ron";

fn load_items(mut commands: Commands, asset_server: Res<AssetServer>) {
    let id = commands.register_system(save_items_scene);
    commands.insert_resource(SaveItems(id));

    commands.spawn((
        Name::new("contents scene"),
        DynamicWorldRoot(asset_server.load(CONTENTS_FILE_PATH)),
    ));
}

fn wait_for_items(
    mut asset_events: MessageReader<AssetEvent<DynamicWorld>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for event in asset_events.read() {
        match event {
            AssetEvent::LoadedWithDependencies { id: _ } => {
                info!("contents loaded!");
                next_state.set(AppState::Running);
            }
            _ => warn!(?event),
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
    let mut query =
        world.query_filtered::<Entity, Or<(With<Item<Flags>>, With<ContentsItems<Flags>>)>>();
    let type_registry = world.resource::<AppTypeRegistry>().read();
    let scene = DynamicWorldBuilder::from_world(&world, &type_registry)
        // .deny_all_resources()
        .allow_resource::<Ground>()
        .allow_resource::<PaperDoll>()
        .deny_component::<PlaybackSettings>()
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
    info!("spawning items!");

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
                        GridContents::<_>::new((3, 2)).with_header("Any:"), // .with_flags(Flags::Container),
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
    mut commands: Commands,
    mut icons: Query<(Entity, &Icon), Changed<Icon>>,
    mut textures: ResMut<EguiUserTextures>,
    names: Query<&Name>,
) {
    for (item, icon) in &mut icons {
        info!(
            "icon changed: {:?} item: {item} name: {}",
            icon.0.path(),
            names.get(item).unwrap()
        );
        commands.entity(item).insert(IconId(
            textures.add_image(EguiTextureHandle::Weak(icon.0.id())),
        ));
    }
}

fn update(
    mut contexts: EguiContexts,
    mut storage: ContentsStorage<Flags>,
    paper_doll: Res<PaperDoll>,
    ground: Res<Ground>,
    opened: Query<(Entity, &Name), (With<Open>, Without<IsResource>)>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

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

    // TODO Should containers opened in a window auto-raise, when dragged to? They can end up behind the fixed contents (ground, etc.).

    // Show all open containers.
    for (c, name, ..) in &opened {
        let mut open = true;
        egui::Window::new(name.as_str())
            .resizable(false)
            .movable(true)
            .open(&mut open)
            // .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-16.0, 16.0))
            .show(ctx, |ui| {
                storage.show(c, ui);
            });
        if !open {
            storage.commands.entity(c).remove::<Open>();
            // .trigger(ContainerClose); // 14.2 doesn't have this yet?
            storage.commands.trigger(ContainerClose(c));
        }
    }

    Ok(())
}

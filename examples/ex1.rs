use bevy::{
    asset::AssetLoadFailedEvent,
    color::palettes::basic::*,
    ecs::{resource::IsResource, system::SystemId},
    prelude::*,
    tasks::IoTaskPool,
    window::RequestRedraw,
    winit::WinitSettings,
    world_serialization::DynamicWorld,
};
#[allow(unused)]
use bevy_ecs::template::*;
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
    pub struct ExFlags: u32 {
        const WEAPON = 1;
        const ARMOR = 1 << 1;
        const POTION = 1 << 2;
        const TRADE_GOOD = 1 << 3;
        const CONTAINER = 1 << 4;
    }
}

// By default, containers can contain any item. The derived default (0) does not work well, see https://docs.rs/bitflags/latest/bitflags/index.html#zero-bit-flags. This is why items require flags.
impl Default for ExFlags {
    fn default() -> Self {
        Self::all()
    }
}

impl std::fmt::Display for ExFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names = self.iter_names().map(|(n, _)| n);
        f.write_str(&itertools::join(names, "|"))
    }
}

// FIX? The migration guide explicitly mentions that it is no longer necessary to add derive MapEntities for a resource (<https://bevy.org/learn/migration-guides/0-18-to-0-19/#miscellaneous>)...

// Remembers which containers are opened. TODO: move to lib?
#[derive(Component, Clone, Default, Reflect)]
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
        .add_plugins((DefaultPlugins, RunicPlugin::<ExFlags>::default()))
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
            (item_icon_changed::<ExFlags>, update)
                .chain()
                .run_if(in_state(AppState::Running)),
        )
        .add_systems(
            Update,
            (styling, save_items).run_if(in_state(AppState::Running)),
        )
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

// We have to modify the node to change the border width?
fn styling(
    mut commands: Commands,
    contents: Query<Entity, Added<GridContents>>,
    opened: Query<Entity, With<Open>>,
    items: Query<Entity, Added<Item>>,
    parents: Query<&ChildOf>,
) {
    // FIX we have decouple the item contents from the item, because the item will be parented to the contents and we want the the inner contents to be unparented (in a window or otherwise) unless it's inline
    for contents in &contents {
        if let Some(_) = parents
            .iter_ancestors(contents)
            .find(|a| opened.contains(*a))
        {
            commands
                .entity(contents)
                .insert((BorderColor::from(FUCHSIA),));
        }
    }

    for item in &items {
        commands.entity(item).insert((BorderColor::from(WHITE),));
    }
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
        slot = ?insert.slot,
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
        slot = ?remove.slot,
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
        "move slot {:?} -> {:?}",
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
        slot = ?drag_over.slot,
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
    opened: Query<Entity, With<Open>>,
) {
    let ctrl = input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);

    if ctrl && input.just_pressed(KeyCode::KeyS) {
        info!("saving contents...");
        commands.run_system(save_items_system.0);
    }

    if input.just_pressed(KeyCode::Escape) {
        for open in &opened {
            commands.entity(open).remove::<Open>();
        }
    }
}

fn save_items_scene(world: &mut World) {
    let mut query = world.query_filtered::<Entity, Or<(With<Item>, With<GridContents>)>>();
    let type_registry = world.resource::<AppTypeRegistry>().read();
    let scene = DynamicWorldBuilder::from_world(&world, &type_registry)
        // .deny_all_resources()
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

fn items() -> impl SceneList {
    bsn_list! [
        #Boomerang
        // This really shouldn't be Default.
        Slot({(0, 0)})
        Icon("boomerang.png")
        Item {
            shape: { [[1, 1], [1, 0]] },
        }
        Flags<ExFlags>(ExFlags::WEAPON),

        #Pouch
        Slot({(2, 0)})
        Icon("pouch.png")
        Item {
            shape: { Shape::new((2, 2), true) }
        }
        Flags<ExFlags>(ExFlags::CONTAINER)
        Layout { direction: Direction::LeftToRight }
        Children [
            #PouchAny
            GridContents {
                header: { "Any:".to_owned() },
                shape: {(3, 2)},
            }
            Flags<ExFlags>({ ExFlags::all() }),

            #PouchP1,
            GridContents {
                header: { "P1:".to_owned() },
                shape: {(1, 1)},
            }
            Flags<ExFlags>({ ExFlags::POTION }),

            #PouchP2,
            GridContents {
                header: { "P2:".to_owned() },
                shape: {(1, 1)},
            }
            Flags<ExFlags>({ ExFlags::POTION })
        ],

        #ShortSword
        Name("Short-sword")
        Slot({(4, 0)})
        Icon("short-sword.png")
        Item {
            shape: { Shape::new((3, 1), true) }
        }
        ItemRotation::R90
        Flags<ExFlags>(ExFlags::WEAPON),

        // Potion 1 & 2 are almost the same?
        #Potion1
        Name("Potion 1")
        Slot({(5, 0)})
        Icon("potion.png")
        Item
        Flags<ExFlags>(ExFlags::POTION),

        #Potion2
        Name("Potion 2")
        Slot({(6, 0)})
        Icon("potion.png")
        Item
        Flags<ExFlags>(ExFlags::POTION),
    ]
}

fn spawn_items(
    mut commands: Commands,
    _asset_server: Res<AssetServer>,
    mut _storage: ContentsStorage<ExFlags>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    info!("spawning items!");

    next_state.set(AppState::Running);

    let scene_list = bsn_list![
        #Ground
        Node
        Children [
            #Ground0
            GridContents {
                shape: { (10, 10) },
                header: { "Ground 10x10".to_owned() },
            }
            Flags<ExFlags>({ ExFlags::all() })
            Children [{ items() }]
        ]
        Open,

        // There is no longer a "main" contents which always appears at the bottom of the other sections. Which means now there's no way to have alternating layouts, and we don't want to do recursive sections just for layout purposes. The layout stuff we'll have to redo later anyway, once we switch to Bevy's native UI.
        #PaperDoll
        Node
        Layout { direction: Direction::TopDown }
        Children [
            #A1
            GridContents {
                shape: { (1, 2) },
                header: { "A1".to_owned() },
            }
            Flags<ExFlags>({ ExFlags::all() }),

            #A2
            GridContents {
                shape: { (1, 2) },
                header: { "A2".to_owned() },
            }
            Flags<ExFlags>({ ExFlags::all() }),

            #W1
            GridContents {
                shape: { (1, 2) },
                header: { "W1".to_owned() },
            }
            Flags<ExFlags>({ ExFlags::WEAPON }),

            #PX
            GridContents {
                shape: { (2, 2) },
                header: { "Only potions! 2x2:".to_owned() },
            }
            Flags<ExFlags>({ ExFlags::POTION }),

            #Weapon
            GridContents {
                shape: { (3, 2) },
                header: { "Weapon (3x2 MAX):".to_owned() },
                expands: true,
            }
            Flags<ExFlags>({ ExFlags::WEAPON }),

            #Belt
            GridContents {
                shape: { (2, 2) },
                header: { "Holds a container:".to_owned() },
                expands: true,
                inline: true,
            }
            Flags<ExFlags>({ ExFlags::CONTAINER }),

            #Bag
            GridContents {
                shape: { (4, 4) },
                header: { "Bag of any! 4x4:".to_owned() },
            }
            Flags<ExFlags>({ ExFlags::all() }),

        ]
        // Open
    ];

    commands.spawn_scene_list(scene_list);
}

// TODO lib
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
    mut storage: ContentsStorage<ExFlags>,
    opened: Query<(Entity, &Name), (With<Open>, Without<IsResource>)>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    storage.update(ctx);

    // TODO component
    // Control-clicking items in the inventory will send them to ground.
    //*storage.target = Some(ground.0);

    // TODO: Titles stored in ECS?

    // egui::Window::new("Paper doll:")
    //     .resizable(false)
    //     .movable(true)
    //     .max_width(512.0)
    //     .anchor(egui::Align2::LEFT_TOP, egui::Vec2::splat(16.0))
    //     .show(ctx, |ui| {
    //         storage.show(paper_doll.0, ui);
    //     });

    // Control-clicking items on the ground will send them to the inventory.
    //*storage.target = Some(paper_doll.0);

    // egui::Window::new("Ground 10x10:")
    //     .resizable(false)
    //     .movable(true)
    //     .anchor(egui::Align2::RIGHT_TOP, egui::Vec2::new(-16.0, 16.0))
    //     .show(ctx, |ui| {
    //         storage.show(ground.0, ui);
    //     });

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

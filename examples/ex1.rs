use std::time::*;

use bevy::{
    asset::AssetLoadFailedEvent, color::palettes::basic::*, ecs::system::SystemId,
    input::common_conditions::*, prelude::*, tasks::IoTaskPool, window::RequestRedraw,
    winit::WinitSettings, world_serialization::DynamicWorld,
};
use runic::*;
use serde::{Deserialize, Serialize};

// NOTE reflect_value is now #[reflect(opaque)]
// You can get flags to serialize with the reflect serialization if you derive reflect outside the bitflags! macro (and NOT use reflect_value) as described here (https://docs.rs/bitflags/latest/bitflags/#custom-derives). This serializes as a struct tuple containing a u32. If you use reflect_value you're pretty much required to implement Serialize yourself. The serde flag for bitflags enables the fancy serialization with flag names.
bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Deserialize, Serialize)]
    #[serde(transparent)]
    #[reflect(opaque)]
    #[reflect(Hash, PartialEq, Debug, Deserialize, Serialize)]
    pub struct Flags: u32 {
        const WEAPON = 1;
        const ARMOR = 1 << 1;
        const POTION = 1 << 2;
        const TRADE_GOOD = 1 << 3;
        const CONTAINER = 1 << 4;
    }
}

impl Accepts for Flags {
    fn accepts(&self, other: &Self) -> bool {
        self.contains(*other)
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

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
enum AppState {
    #[default]
    Loading,
    Running,
}

fn main() {
    App::new()
        .insert_resource(WinitSettings::desktop_app())
        .add_plugins((DefaultPlugins, RunicPlugin::<Flags>::default()))
        .init_state::<AppState>()
        // .add_plugins(EguiPlugin::default())
        .add_systems(Startup, startup)
        .add_systems(OnEnter(AppState::Loading), load_items)
        .add_systems(Update, wait_for_items.run_if(in_state(AppState::Loading)))
        .add_systems(
            Update,
            spawn_contents
                .run_if(in_state(AppState::Loading))
                .run_if(on_message::<AssetLoadFailedEvent<DynamicWorld>>),
        )
        .add_systems(
            Update,
            (
                (styling, save_items).run_if(in_state(AppState::Running)),
                rotate_items.run_if(input_just_pressed(KeyCode::KeyR)),
            ),
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
        .add_observer(open_container)
        .add_observer(container_opened)
        .run();
}

fn startup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

// We have to modify the node to change the border width?
fn styling(
    mut commands: Commands,
    sections: Query<(NameOrEntity, &GridContents)>,
    items: Query<(NameOrEntity, &Item)>,
    mut nodes: Query<
        (
            NameOrEntity,
            Option<&BorderColor>,
            Option<&Viewing>,
            &mut Node,
        ),
        Added<Node>,
    >,
) {
    for (n, border_color, v, mut node) in &mut nodes {
        if let Some(&Viewing(v)) = v {
            if let Ok((_s, _)) = sections.get(v) {
                node.border = px(1.).all();
                commands
                    .entity(n.entity)
                    .insert((BackgroundColor::from(BLACK), BorderColor::from(FUCHSIA)));
            } else if let Ok((_i, _)) = items.get(v) {
                node.border = px(1.).all();
                commands
                    .entity(n.entity)
                    .insert((BorderColor::from(WHITE),));
            }
        } else {
            if border_color.is_none() {
                node.border = px(1.).all();
                commands.entity(n.entity).insert((BorderColor::from(GRAY),));
            }
        }
    }
}

// Increment the currently dragged item's rotation.
fn rotate_items(mut items: Query<&mut DragRotation>) {
    for mut r in &mut items {
        r.0 = r.0.increment();
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
    // names: Query<Option<&Name>>,
    asset_server: Res<AssetServer>,
) -> Result {
    // let drag_over = event.event();
    // let [target, item] = names.get_many([event.event_target(), drag_over.item])?;
    // let target = target.map(|n| n.as_str()).unwrap_or("section");
    // info!(
    //     target,
    //     item = item.unwrap().as_str(),
    //     slot = ?drag_over.slot,
    //     "drag over"
    // );

    commands
        .entity(event.event_target())
        .insert(AudioPlayer::new(asset_server.load("sfx100v2_wood_03.ogg")))
        // This restarts the audio. Maybe we should detach first.
        .remove::<AudioSink>();

    Ok(())
}

#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
struct LastClick(Instant);

// Checks for a double click on an item to open it.
fn open_container(
    event: On<Pointer<Release>>,
    mut commands: Commands,
    views: Query<&Viewing>,
    // world: &World,
    mut items: Query<(NameOrEntity, Option<&mut LastClick>), With<Item>>,
    // This does not work in desktop app mode...
    // time: Res<Time<Real>>,
) {
    // dbg!(world.entity(event.event_target()).spawned_by());

    if event.button == PointerButton::Primary {
        if let Ok(&Viewing(v)) = views.get(event.event_target())
            && let Ok((id, last)) = items.get_mut(v)
        {
            match last {
                Some(mut last) => {
                    if last.0.elapsed() < Duration::from_millis(300) {
                        info!("open: {id}");
                        commands
                            .entity(id.entity)
                            .remove::<LastClick>()
                            .insert(Open(
                                event.pointer_location.position + Vec2::new(-40.0, 40.0),
                            ));
                    }
                    last.0 = Instant::now();
                }
                _ => _ = commands.entity(id.entity).insert(LastClick(Instant::now())),
            }
        }
    }
}

fn container_opened(
    event: On<ContainerOpen>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
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

fn save_items_scene(names: Query<(Entity, &Name)>, children: Query<&Children>, world: &World) {
    use bevy::ui::*;

    let iter = names
        .iter()
        .filter(|(_, n)| n.as_str() == "Inventory" || n.as_str() == "InventoryView")
        .flat_map(|(e, _)| children.iter_descendants(e));

    let type_registry = world.resource::<AppTypeRegistry>().read();
    let scene = DynamicWorldBuilder::from_world(&world, &type_registry)
        .deny_all_resources()
        .deny_component::<PlaybackSettings>()
        .deny_component::<ComputedStackIndex>()
        .deny_component::<ComputedNode>()
        .deny_component::<ComputedUiRenderTargetInfo>()
        .deny_component::<ComputedUiTargetCamera>()
        .deny_component::<UiGlobalTransform>()
        .deny_component::<ScrollPosition>()
        .deny_component::<bevy::ui::widget::ImageNodeSize>()
        .deny_component::<bevy::picking::hover::PickingInteraction>()
        .extract_resources()
        .extract_entities(iter)
        .build();

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

fn pouch_view(sections: Vec<Entity>) -> Box<dyn Scene> {
    use itertools::Itertools;

    let (a1, p1, p2) = sections
        .into_iter()
        .map(|e| bsn! { Viewing(e) })
        .collect_tuple()
        .unwrap();

    Box::new(bsn! {
        Node
        Children [
            a1,
            Node {
                flex_direction: FlexDirection::Column,
            }
            Children [p1, p2],
        ]
    })
}

fn items() -> impl SceneList {
    let pouch_view = pouch_view as fn(sections: Vec<Entity>) -> Box<dyn Scene>;

    bsn_list! [
        #Boomerang
        // This really shouldn't be Default.
        Slot({(0, 0)})
        Icon("boomerang.png")
        Item {
            shape: { [[1, 1], [1, 0]] },
        }
        Flags::WEAPON,

        #Pouch
        Slot({(2, 0)})
        Icon("pouch.png")
        Item {
            shape: { Shape::new((2, 2), true) }
        }
        Flags::CONTAINER
        ContentsView(pouch_view)
        Children [
            (
                #PouchAny
                GridContents {
                    header: { "Any:".to_owned() },
                    shape: {(3, 2)},
                }
                Flags
            ),

            (
                #PouchP1
                GridContents {
                    header: { "P1:".to_owned() },
                    shape: {(1, 1)},
                }
                Flags::POTION
            ),

            (
                #PouchP2 GridContents {
                    header: { "P2:".to_owned() },
                    shape: {(1, 1)},
                }
                Flags::POTION
            ),
        ],

        #ShortSword
        Name("Short-sword")
        Slot({(4, 0)})
        Icon("short-sword.png")
        Item {
            shape: { Shape::new((3, 1), true) }
        }
        ItemRotation::R90
        Flags::WEAPON,

        // Potion 1 & 2 are almost the same?
        #Potion1
        Name("Potion 1")
        Slot({(5, 0)})
        Icon("potion.png")
        Item
        Flags::POTION,

        #Potion2
        Name("Potion 2")
        Slot({(6, 0)})
        Icon("potion.png")
        Item
        Flags::POTION,
    ]
}

/// Note the paper doll and ground are fixed containers and don't use `ContentsView` since they are only spawned once. It's also easier to create the views with entity references.
fn spawn_contents(mut commands: Commands, mut next_state: ResMut<NextState<AppState>>) {
    info!("spawning contents!");

    let scene = bsn_list![
        #Inventory
        Children [
            #PaperDoll
            Target(#Ground)
            Children [
                #A1
                GridContents {
                    shape: { (1, 2) },
                    header: { "A1".to_owned() },
                }
                Flags,

                #A2
                GridContents {
                    shape: { (1, 2) },
                    header: { "A2".to_owned() },
                }
                Flags,

                #W1
                GridContents {
                    shape: { (1, 2) },
                    header: { "W1".to_owned() },
                }
                Flags::WEAPON,

                #PX
                GridContents {
                    shape: { (2, 2) },
                    header: { "Only potions! 2x2:".to_owned() },
                }
                Flags::POTION,

                #Weapon
                GridContents {
                    shape: { (3, 2) },
                    header: { "Weapon (3x2 MAX):".to_owned() },
                    expands: true,
                }
                Flags::WEAPON,

                #Belt
                GridContents {
                    shape: { (2, 2) },
                    header: { "Holds a container:".to_owned() },
                    expands: true,
                    inline: true,
                }
                Flags::CONTAINER,

                #Bag
                GridContents {
                    shape: { (4, 4) },
                    header: { "Bag of any! 4x4:".to_owned() },
                }
                Flags,
            ],

            #Ground
            Target(#PaperDoll)
            Children [
                #Ground0
                GridContents {
                    shape: { (10, 10) },
                    header: { "Ground 10x10".to_owned() },
                }
                Flags
                Children [{ items() }]
            ]
        ],

        #InventoryView
        Node {
            flex_direction: FlexDirection::Row,
            width: percent(100.0),
            // height: percent(100.0),
            // border: px(4.),
            padding: px(8),
            // align_items: AlignItems::Center,
        }
        // BorderColor::all(WHITE)
        Children [
            #PaperDollView
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 0.0,
                // flex_shrink: 0.0,
                // flex_basis: Val::ZERO,
                min_width: Val::ZERO,
                // width: percent(50.0),
                border: px(4.),
                padding: px(8),
                // This was for expands but we can set it in the section node.
                // align_items: AlignItems::FlexStart,
            }
            Pickable::IGNORE
            BorderColor::all(GREEN)
            Children [
                (
                    #PaperDollText
                    Text::new("Paper Doll")
                    Pickable::IGNORE
                ),

                Node {
                    flex_direction: FlexDirection::Row,
                }
                Children [
                    Viewing(#A1),
                    Viewing(#A2),
                ],

                Viewing(#W1),
                Viewing(#PX),
                Viewing(#Weapon),
                Viewing(#Belt),
                Viewing(#Bag),
            ],

            #GroundView
            Node {
                justify_self: JustifySelf::End,
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                // flex_shrink: 1.0,
                // flex_basis: Val::ZERO,
                min_width: Val::ZERO,
                // Clipping causes problems w/ dragging to other sections.
                // overflow: Overflow::clip(),
                // width: percent(50.0),
                border: px(4.),
                padding: px(8),
            }
            BorderColor::all(BLUE)
            Pickable::IGNORE
            Children [
                (
                    #GroundText
                    Text::new("Ground")
                    Pickable::IGNORE
                ),

                Viewing(#Ground0)
            ]
        ]
    ];

    commands.spawn_scene_list(scene);
    //.insert((Name::new("Root"), Pickable::IGNORE));

    next_state.set(AppState::Running);
}

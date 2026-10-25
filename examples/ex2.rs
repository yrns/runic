use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, startup)
        .run();
}

fn startup(mut commands: Commands) {
    commands.spawn(Camera2d);

    let left = |event: On<PointerDragLeave>, names: Query<&Name>| {
        info!(
            "left: {} <- {}/{} ({})",
            names.get(event.dragged).unwrap(),
            names.get(event.event_target()).unwrap(),
            names.get(event.entity).unwrap(),
            names.get(event.original_event_target()).unwrap()
        );
    };

    let enter = |event: On<PointerDragEnter>, names: Query<&Name>| {
        info!(
            "enter: {} -> {}/{} ({})",
            names.get(event.dragged).unwrap(),
            names.get(event.event_target()).unwrap(),
            names.get(event.entity).unwrap(),
            names.get(event.original_event_target()).unwrap()
        );
    };

    commands
        .spawn_scene(bsn! {
            #SceneRoot
            Node
            Children [
                #Parent
                Node {
                    position_type: PositionType::Absolute
                    left: px(0),
                    top: px(0),
                    width: px(200),
                    height: px(200),
                }
                BackgroundColor(bevy::color::palettes::css::PINK)
                on(left)
                on(enter)
                Children [
                    #Child
                    Node {
                        position_type: PositionType::Absolute
                        left: px(0),
                        top: px(0),
                        width: px(100),
                        height: px(100),
                    }
                    BackgroundColor(bevy::color::palettes::css::BROWN)
                    Pickable {
                        should_block_lower: false,
                        is_hoverable: true,
                    }
                    on(left)
                    on(enter)
                ]
            ]
        })
        .insert(Name::from("Root"));
}

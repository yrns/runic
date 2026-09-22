use std::marker::PhantomData;

use bevy_app::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_reflect::*;

use crate::*;

#[derive(Default)]
pub struct RunicPlugin<T>(PhantomData<T>);

impl<T: Reflect + FromReflect + GetTypeRegistration + TypePath + Typed + Accepts> Plugin
    for RunicPlugin<T>
{
    fn build(&self, app: &mut App) {
        // TODO: separate options per T?
        app.init_resource::<Options>()
            .register_type::<Flags<T>>()
            .register_type::<GridContents>()
            .register_type::<Item>()
            .register_type::<Icon>()
            .add_systems(
                PostUpdate,
                (
                    contents::insert_item,
                    item::update_drag_rotation,
                    (
                        // view::spawn_item_views,
                        view::contents_spawned,
                        view::item_moved,
                        item::update_nodes,
                    )
                        .chain(),
                ),
            )
            .add_observer(view::on_open_container)
            .add_observer(item::on_item_drag_start)
            .add_observer(item::on_item_drag)
            .add_observer(item::on_item_drag_enter::<T>)
            .add_observer(item::on_item_drag_over)
            .add_observer(item::on_item_drag_drop::<T>)
            .add_observer(item::on_send_item::<T>)
            .add_observer(item::on_item_drag_end)
            .add_observer(item::on_item_drag_leave)
            .add_observer(item::on_item_drag_cancel);
    }
}

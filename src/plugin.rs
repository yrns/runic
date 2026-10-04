use std::marker::PhantomData;

use bevy_app::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
#[cfg(feature = "reflect")]
use bevy_reflect::*;

use crate::*;

#[derive(Default)]
pub struct RunicPlugin<T>(PhantomData<T>);

impl<T> Plugin for RunicPlugin<T>
where
    T: Accepts + GetTypeRegistration,
{
    fn build(&self, app: &mut App) {
        #[cfg(feature = "reflect")]
        app.register_type::<T>()
            .register_type::<GridContents>()
            .register_type::<Item>()
            .register_type::<Icon>();

        app.init_resource::<Options>()
            .add_systems(
                Update,
                (
                    contents::insert_item,
                    item::update_drag_rotation,
                    view::open_container,
                    (
                        // view::spawn_item_views,
                        view::contents_spawned,
                        view::item_moved,
                        item::item_moved_or_rotated,
                        item::item_view_changed,
                    )
                        .chain(),
                ),
            )
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

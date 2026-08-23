use std::marker::PhantomData;

use bevy_app::*;
use bevy_reflect::*;

use crate::*;

#[derive(Default)]
pub struct RunicPlugin<T>(PhantomData<T>);

impl<T: Reflect + FromReflect + GetTypeRegistration + TypePath + Typed> Plugin for RunicPlugin<T> {
    fn build(&self, app: &mut App) {
        // TODO: separate options per T?
        app.init_resource::<Options>()
            .register_type::<Flags<T>>()
            .register_type::<GridContents>()
            .register_type::<Item>()
            .register_type::<Icon>()
            .add_systems(
                Update,
                (
                    contents::add_item,
                    contents::update_contents,
                    item::update_rotation,
                    item::update_items,
                ),
            );
    }
}

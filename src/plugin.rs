use std::marker::PhantomData;

use bevy_app::*;
use bevy_reflect::*;

use crate::*;

#[derive(Default)]
pub struct RunicPlugin<T>(PhantomData<T>);

impl<T: Reflect + FromReflect + GetTypeRegistration + TypePath> Plugin for RunicPlugin<T> {
    fn build(&self, app: &mut App) {
        // TODO: separate options per T?
        app.init_resource::<Options>()
            .register_type::<ContentsItems<T>>()
            .register_type::<Sections>()
            .register_type::<Item<T>>()
            .register_type::<Icon>();
    }
}

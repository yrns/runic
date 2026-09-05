use bevy_asset::prelude::*;
use bevy_ecs::prelude::*;
use bevy_image::Image;
use bevy_reflect::*;

#[derive(Component, Clone, Debug, Reflect, FromTemplate)]
#[reflect(Component, Debug)]
pub struct Icon(pub Handle<Image>);

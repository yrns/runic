use bevy_asset::prelude::*;
use bevy_ecs::prelude::*;
use bevy_egui::egui::TextureId;
use bevy_image::Image;
use bevy_reflect::*;

#[derive(Component, Debug, Reflect)]
#[reflect(Component, Debug)]
pub struct Icon(pub Handle<Image>);

#[derive(Component, Debug)]
pub struct IconId(pub TextureId);

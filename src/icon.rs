use bevy_asset::{prelude::*, AssetPath};
use bevy_ecs::prelude::*;
use bevy_reflect::*;
use bevy_render::texture::Image;

#[derive(Component, Debug, Default, Reflect)]
#[reflect(Component, Debug)]
pub enum Icon {
    #[default]
    None,
    Path(AssetPath<'static>),
    #[reflect(ignore)]
    Handle(Handle<Image>),
}

impl Icon {
    pub fn to_path(&mut self) {
        match &self {
            Icon::Handle(h) => *self = Icon::Path(h.path().expect("icon has a path").clone()),
            _ => (),
        }
    }

    pub fn handle(&self) -> &Handle<Image> {
        let Icon::Handle(h) = &self else {
            panic!("no handle");
        };
        h
    }
}

impl From<Handle<Image>> for Icon {
    fn from(h: Handle<Image>) -> Self {
        Icon::Handle(h)
    }
}

impl From<&'static str> for Icon {
    fn from(path: &'static str) -> Self {
        Icon::Path(AssetPath::parse(path))
    }
}

impl From<AssetPath<'static>> for Icon {
    fn from(path: AssetPath<'static>) -> Self {
        Icon::Path(path)
    }
}

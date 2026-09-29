//! The same container and its items can be visible on screen more than once. Instead of adding nodes directly to the contents hierarchy we spawn these view nodes to display contents, which reference the original items and contents. This module handles keeping the view nodes in sync with the original contents.

// What about dragging items? We only draw one? What if somehow the contents gets destroyed or closed while dragging?

use bevy_asset::Handle;
use bevy_ecs::prelude::*;
use bevy_image::Image;
use bevy_math::*;
use bevy_picking::{Pickable, events::*};
#[cfg(feature = "reflect")]
use bevy_reflect::std_traits::ReflectDefault;
#[cfg(all(feature = "serialize", feature = "reflect"))]
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};
use bevy_scene::*;
use bevy_ui::{widget::*, *};
use tracing::*;

use crate::*;

/// This entity is viewing a specific item or section. This decouples the inventory entities from the UI nodes (both of which already use `ChildOf`).
#[derive(Component, FromTemplate, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(
    feature = "reflect",
    reflect(Component, PartialEq, Debug, FromWorld, Clone)
)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    all(feature = "serialize", feature = "reflect"),
    reflect(Serialize, Deserialize)
)]
#[relationship(relationship_target = ViewedBy)]
#[require(Node)]
pub struct Viewing(pub Entity);

// See comments for ChildOf...
impl FromWorld for Viewing {
    #[inline(always)]
    fn from_world(_world: &mut World) -> Self {
        Self(Entity::PLACEHOLDER)
    }
}

/// Entities viewing this item or section.
// TODO: Test views despawning with linked_spawn.
#[derive(Component, Default, Debug, PartialEq, Eq)]
#[relationship_target(relationship = Viewing, linked_spawn)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, FromWorld, Default))]
pub struct ViewedBy(Vec<Entity>);

// Copied from Children.
impl<'a> IntoIterator for &'a ViewedBy {
    type Item = <Self::IntoIter as Iterator>::Item;

    type IntoIter = core::slice::Iter<'a, Entity>;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

// Copied from Children.
impl std::ops::Deref for ViewedBy {
    type Target = [Entity];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

fn update_node(contents: &GridContents, node: &mut Node) {
    let UVec2 { x, y } = contents.size();

    // If the section expands, we use 1x1 until an item is placed here, then it expands to the size of the item's node.
    if contents.expands {
        node.display = Display::Flex;
        node.min_width = px(48);
        node.min_height = px(48);
        node.width = Val::Auto;
        node.height = Val::Auto;
        node.grid_template_columns = Vec::new();
        node.grid_template_rows = Vec::new();
        // The parent default seems to be stretch even though it's AlignItems::Default?
        node.align_self = AlignSelf::FlexStart;
    } else {
        node.display = Display::Grid;
        node.width = px(x * 48);
        node.height = px(y * 48);
        node.grid_template_columns = RepeatedGridTrack::px(x as u16, 48.0);
        node.grid_template_rows = RepeatedGridTrack::px(y as u16, 48.0);
    }
    // TEMP styling remove
    node.margin = px(2.).all();
    node.padding = px(2.).all();
    node.align_items = AlignItems::Center;
    node.justify_content = JustifyContent::Center;
}

/// Returns an item view scene.
pub fn item_view(
    // How?
    // name: Option<&Name>,
    icon: Handle<Image>,
    section_view: Entity,
    item: Entity,
) -> impl Scene {
    // let name: Box<dyn Scene> = Box::new(match name {
    //     Some(name) => {
    //         let name = name.clone();
    //         bsn![{ name }]
    //     }
    //     None => bsn! [#ItemView],
    // });

    bsn! [
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        // The item view is a child of the section view.
        ChildOf(section_view)
        Viewing(item)
        ImageNode { image: icon }
        // This is also default behavior and is only needed when dragging?
        Pickable::default()
    ]
}

// Paint shape and set slots here?
// TODO: There is no longer a way to specify the layout of an item container. There never was?
// Despawn existing items on view change?
pub fn contents_spawned(
    mut commands: Commands,
    mut views: Query<(NameOrEntity, &Viewing, &mut Node), Changed<Viewing>>,
    icons: Query<(NameOrEntity, &Icon)>,
    sections: Query<(NameOrEntity, &GridContents, Option<&Children>)>,
) {
    // Node is a required component of Viewing, so we can count on it existing on spawn.
    for (v, &Viewing(s), mut node) in &mut views {
        if let Ok((s, section, items)) = sections.get(s) {
            info!("section view changed: {v} section: {s}");
            update_node(section, &mut *node);

            // Copy the section name.
            if let Some(name) = s.name {
                commands.entity(v.entity).insert(name.clone());
            }

            if let Some(items) = items {
                // TODO: This will silently omit an item if the icon is missing.
                for (i, Icon(icon)) in icons.iter_many(items) {
                    let mut item_view =
                        commands.spawn_scene(item_view(icon.clone(), v.entity, i.entity));

                    // Copy the item's name.
                    if let Some(name) = i.name {
                        item_view.insert(name.clone());
                    }
                }
            }
        }
    }
}

/// When items change sections, we need to update the corresponding views.
pub fn item_moved(
    mut commands: Commands,
    items: Query<(NameOrEntity, &Icon, &ViewedBy, &ChildOf), (Changed<ChildOf>, With<Item>)>,
    sections: Query<&ViewedBy, With<GridContents>>,
    views: Query<NameOrEntity, With<Viewing>>,
) {
    for (i, Icon(icon), item_views, &ChildOf(s)) in &items {
        if let Ok(section_views) = sections.get(s) {
            // We may actually care about the ordering here, meaning matching pairs of item and section views.
            let mut iter = views.iter_many(item_views);
            for s in views.iter_many(section_views) {
                match iter.next() {
                    Some(i) => {
                        commands.entity(i.entity).insert(ChildOf(s.entity));
                        info!("item view moved: {i} -> {s}");
                    }
                    None => {
                        // Create a new item view.
                        let mut view =
                            commands.spawn_scene(item_view(icon.clone(), s.entity, i.entity));
                        if let Some(name) = i.name {
                            view.insert(name.clone());
                        } else {
                            // #ItemView?
                        }
                        info!("new item view: {i} -> {s}");
                    }
                }
            }

            // If we have more item views than section views we despawn the excess.
            for i in iter {
                commands.entity(i.entity).despawn();
            }
        }
    }
}

/// Stores which container this view belongs to.
#[derive(Component, Debug, FromTemplate)]
pub struct ContainerView(pub Entity);

// TODO: Check for an already opened container and then raise it!
// Rely on Open? Update drag position of window...
pub fn open_container(
    mut commands: Commands,
    sections: Query<(Entity, &GridContents)>,
    containers: Query<(NameOrEntity, &Open, &Children), Added<Open>>,
) {
    for (c, &Open(p), children) in &containers {
        let sections = sections
            .iter_many(children)
            // section_view?
            .map(|(e, _)| bsn! { Viewing(e) })
            .collect::<Vec<_>>();

        let entity = c.entity;
        commands.entity(entity).trigger(ContainerOpen);

        let window = bsn![
            #Window
            ContainerView(entity)
            Node {
                // TODO Find empty screen position.
                position_type: PositionType::Absolute,
                left: px(p.x),
                top: px(p.y),
                flex_direction: FlexDirection::Column,
            }
            // Should cover the default UI, but be under the dragged item.
            GlobalZIndex(1)
            Children [
                #Header
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    width: percent(100.0)
                }
                on(drag_by_header)
                on(drag_by_header_end)
                Children [
                    Node Text("Contents"),
                    Node { right: px(0.0) } Button Text("X") on(close_window),
                ],

                {sections}
            ]
        ];

        info!("new window for {c}: {}", commands.spawn_scene(window).id());
    }
}

fn close_window(
    event: On<Pointer<Release>>,
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    views: Query<&ContainerView>,
) {
    let root = child_of.root_ancestor(event.event_target());
    if let Ok(&ContainerView(v)) = views.get(root) {
        commands.entity(v).remove::<Open>().trigger(ContainerClose);
        commands.entity(root).despawn();
    }
}

/// Move contents.
fn drag_by_header(
    mut event: On<Pointer<Drag>>,
    mut nodes: Query<&mut Node>,
    parents: Query<&ChildOf>,
) {
    let entity = event.entity;
    if let Ok(&ChildOf(p)) = parents.get(entity) {
        event.propagate(false);
        if let Ok(mut node) = nodes.get_mut(p) {
            let Vec2 { x, y } = event.delta;
            let Node { top, left, .. } = &mut *node;
            match (top, left) {
                (Val::Px(top), Val::Px(left)) => {
                    *top += y;
                    *left += x;
                }
                _ => (),
            }
        }
    }
}

/// Update `Open` to represent the new position.
pub fn drag_by_header_end(
    event: On<Pointer<DragEnd>>,
    views: Query<(&Node, &ContainerView)>,
    parents: Query<&ChildOf>,
    mut containers: Query<&mut Open>,
) {
    let root = parents.root_ancestor(event.event_target());
    if let Ok((node, &ContainerView(v))) = views.get(root)
        && let Ok(mut open) = containers.get_mut(v)
    {
        match (node.left, node.top) {
            (Val::Px(x), Val::Px(y)) => open.0 = Vec2::new(x, y),
            _ => {
                error!("contents position not in pixels");
            }
        }
    }
}

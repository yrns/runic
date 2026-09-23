//! The same container and its items can be visible on screen more than once. Instead of adding nodes directly to the contents hierarchy we spawn these view nodes to display contents, which reference the original items and contents. This module handles keeping the view nodes in sync with the original contents.

// What about dragging items? We only draw one? What if somehow the contents gets destroyed or closed while dragging?

use bevy_ecs::prelude::*;
use bevy_math::*;
use bevy_picking::{Pickable, events::*};
use bevy_scene::*;
use bevy_ui::{widget::*, *};
use tracing::*;

use crate::*;

#[derive(Component, Debug, Clone, FromTemplate)]
#[relationship(relationship_target = ViewedBy)]
#[require(Node)]
pub struct Viewing(pub Entity);

#[derive(Component, Debug, Clone, FromTemplate)]
#[relationship_target(relationship = Viewing)]
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
    let UVec2 { x, y } = contents.shape.size;

    node.display = Display::Grid;
    // TEMP styling remove
    node.margin = px(2.).all();
    node.padding = px(2.).all();
    node.width = px(x * 48);
    node.height = px(y * 48);
    node.grid_template_columns = RepeatedGridTrack::px(x as u16, 48.0);
    node.grid_template_rows = RepeatedGridTrack::px(y as u16, 48.0);
    node.align_items = AlignItems::Center;
    node.justify_content = JustifyContent::Center;
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
                for (i, icon) in icons.iter_many(items) {
                    let mut item_view = commands.spawn((
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..Default::default()
                        },
                        // The item view is a child of the section view.
                        ChildOf(v.entity),
                        Viewing(i.entity),
                        ImageNode::new(icon.0.clone()).with_mode(NodeImageMode::Auto),
                        // This is also default behavior and is only needed when dragging?
                        Pickable::default(),
                    ));

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
    items: Query<(NameOrEntity, &ViewedBy, &ChildOf), (Changed<ChildOf>, With<Item>)>,
    sections: Query<&ViewedBy, With<GridContents>>,
    views: Query<NameOrEntity, With<Viewing>>,
) {
    for (i, item_views, &ChildOf(s)) in &items {
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
                        commands.spawn((Viewing(i.entity), ChildOf(s.entity)));
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

// TODO: Check for an already opened container and then raise it.
pub fn on_open_container(
    event: On<OpenContainer>,
    mut commands: Commands,
    // views: Query<&Viewing>,
    sections: Query<(Entity, &GridContents)>,
    children: Query<&Children, With<Item>>,
) {
    // This is duplicating is_container()...
    let t = event.event_target();
    if let Ok(c) = children.get(t) {
        let sections = sections
            .iter_many(c)
            .map(|(e, _)| bsn! { Viewing(e) })
            .collect::<Vec<_>>();

        commands.entity(t).insert(Open);

        let window = bsn![
            #Window
            Node {
                position_type: PositionType::Absolute,
                top: px(32),
                left: px(32),
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
                Children [
                    Node Text("Contents"),
                    Node { right: px(0.0) } Button Text("X") on(close_window),
                ],

                {sections}
            ]
        ];

        info!("new window: {}", commands.spawn_scene(window).id());
    }
}

fn close_window(event: On<Pointer<Release>>, mut commands: Commands, child_of: Query<&ChildOf>) {
    if let Ok(header) = child_of.get(event.event_target())
        && let Ok(window) = child_of.get(header.0)
    {
        commands.entity(window.0).despawn();
    }
}

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

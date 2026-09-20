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
    icons: Query<&Icon>,
    sections: Query<(NameOrEntity, &GridContents, Option<&Children>)>,
) {
    // Node is a required component of Viewing, so we can count on it existing on spawn.
    for (v, &Viewing(s), mut node) in &mut views {
        if let Ok((s, section, items)) = sections.get(s) {
            info!("section view changed: {v} section: {s}");
            update_node(section, &mut *node);

            if let Some(items) = items {
                for &item in items {
                    // FIX: unwrap
                    let icon = icons.get(item).unwrap();
                    commands.spawn((
                        Name::new("Foo"),
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..Default::default()
                        },
                        // The item view is a child of the section view.
                        ChildOf(v.entity),
                        Viewing(item),
                        ImageNode::new(icon.0.clone()).with_mode(NodeImageMode::Auto),
                        // This is also default behavior and is only needed when dragging?
                        Pickable::default(),
                        // Remove? This is only needed when dragging?
                        GlobalZIndex::default(),
                    ));
                }
            }
        }
    }
}

pub fn item_moved(
    mut commands: Commands,
    items: Query<(NameOrEntity, &ChildOf), (Changed<ChildOf>, With<Item>)>,
    views: Query<&ViewedBy>,
) {
    for (id, &ChildOf(s)) in &items {
        // In what case are there differing numbers of item views and section views? Never, I think.
        match (views.get(id.entity), views.get(s)) {
            (Ok(i), Ok(s)) if i.len() == s.len() => {
                // We may actually care about the ordering here, meaning matching pairs of items and sections. TODO test multiple views
                for (i, s) in i.iter().zip(s.iter()) {
                    info!("item_moved: {i} -> {s}");
                    commands.entity(i).insert(ChildOf(s));
                }
            }
            (Err(_), Err(_)) => (),
            _ => panic!("item views and parent section views do not match"),
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
            // Interpolate view name from section name?
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

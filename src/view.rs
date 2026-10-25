//! The same container and its items can be visible on screen more than once. Instead of adding nodes directly to the contents hierarchy we spawn these view nodes to display contents, which reference the original items and contents. This module handles keeping the view nodes in sync with the original contents.

// What about dragging items? We only draw one? What if somehow the contents gets destroyed or closed while dragging?

use bevy_asset::Handle;
use bevy_derive::*;
use bevy_ecs::{entity::EntityHashMap, prelude::*};
use bevy_image::Image;
use bevy_math::*;
use bevy_picking::{Pickable, events::*, hover::Hovered};
#[cfg(feature = "reflect")]
use bevy_reflect::std_traits::ReflectDefault;
use bevy_scene::*;
use bevy_ui::{
    widget::{ImageNode, Text},
    *,
};
use bevy_ui_widgets::{Activate, Button};
use tracing::*;

use crate::*;

/// This entity is viewing a specific item or section. This decouples the inventory entities from the UI nodes (both of which already use `ChildOf`).
#[derive(Component, FromTemplate, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(
    feature = "reflect",
    reflect(Component, PartialEq, Debug, FromWorld, Clone)
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

/// When the specified section is filled, the contents of those items (if any) will be displayed as child nodes of this entity.
#[derive(Component, Debug, FromTemplate)]
#[require(Node)]
pub struct InlineContents(pub Entity);

impl From<Entity> for InlineContents {
    fn from(section: Entity) -> Self {
        Self(section)
    }
}

/// Updates a section view's node to match.
fn update_section_view(section: &GridContents, node: &mut Node) {
    let UVec2 { x, y } = section.size();

    // If the section expands, we use 1x1 until an item is placed here, then it expands to the size of the item's node.
    if section.expands {
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

pub const ITEM_PICKABLE: Pickable = Pickable {
    should_block_lower: false,
    is_hoverable: true,
};

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

    // let x = 2u32;
    // let y = 2u32;

    // let cols: Vec<RepeatedGridTrack> = RepeatedGridTrack::px(2, 48.0);
    // let rows: Vec<RepeatedGridTrack> = RepeatedGridTrack::px(2, 48.0);
    // RepeatedGridTrack::percent(GridTrackRepetition::Count(2), 0.5);

    fn _cell(x: i16, y: i16) -> impl Scene {
        bsn! {
            Node {
                grid_row: GridPlacement::start(x),
                grid_column: GridPlacement::start(y),
                width: px(48.0),
                height: px(48.0),
            }
            Outline
        }
    }

    bsn! {
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            // display: Display::Grid,
            // width: px(48.0 * 4.0),
            // height: px(48.0 * 4.0),
            // grid_template_columns: cols,
            // grid_template_rows: rows,
            // overflow: Overflow::clip(),
        }
        // The item view is a child of the section view.
        ChildOf(section_view)
        Viewing(item)
        ImageNode { image: icon }
        ITEM_PICKABLE
        // Children [
        //     @cell(1, 1)
        //     --
        //     @cell(1, 2)
        //     --
        //     @cell(2, 1)
        //     --
        //     @cell(2, 2)
        //     Children [ Text("Nope") Pickable::IGNORE ]
        //     Pickable::IGNORE
        // ]
    }
}

/// This updates a section view when spawned or updated and spawns its contained items' views.
// Paint shape and set slots here?
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
            update_section_view(section, &mut *node);

            // Copy the section name.
            if let Some(name) = s.name {
                commands.entity(v.entity).insert(name.clone());
            }

            if let Some(items) = items {
                // TODO: This will silently omit an item if the icon is missing.
                for (i, Icon(icon)) in icons.iter_many(items).matched() {
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
    // Can `viewed_by` be None?
    for (i, Icon(icon), item_views, &ChildOf(s)) in &items {
        // We may actually care about the ordering here of matching item and section view pairs. Sort?
        let mut iter = views.iter_many(item_views).matched();

        if let Ok(section_views) = sections.get(s) {
            for s in views.iter_many(section_views).matched() {
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
        }

        // If we have more item views than section views we despawn the excess.
        for i in iter {
            commands.entity(i.entity).despawn();
        }
    }
}

/// Stores the container this view belongs to. We can't use `Viewing`/`ViewedBy` because this is usually an item which may have its own view already. This is only to connect pointer events to the original container (item).
#[derive(Component, Debug, FromTemplate)]
pub struct ContainerView(pub Entity);

/// For containers which can be opened, this provides the contents layout. Since this view is spawned well after the actual item and contets are spawned we cannot resue the entity references (if any) that were used, so we have to rely on the number and ordering of the entities.
// We could also store a lookup table?
#[derive(Component, Clone)]
pub struct ContentsView(pub fn(Vec<Entity>) -> Box<dyn Scene>);

/// Default section (row) layout.
pub fn default_contents(sections: Vec<Entity>) -> Box<dyn Scene> {
    let sections: Vec<_> = sections.into_iter().map(|e| bsn! { Viewing(e) }).collect();

    Box::new(bsn! {
        Node Children [{sections}]
    })
}

// Just for BSN.
impl Default for ContentsView {
    fn default() -> Self {
        Self(default_contents)
    }
}

/// When the `Open` component is added to a container, this displays a draggable window with the contents.
pub fn open_container(
    mut commands: Commands,
    sections: Query<(Entity, &GridContents)>,
    containers: Query<(NameOrEntity, &Open, Option<&ContentsView>, &Children), Added<Open>>,
) {
    for (c, &Open(p), contents_view, children) in &containers {
        let sections = sections
            .iter_many(children)
            .matched()
            .map(|(e, _)| e)
            .collect();
        let sections = match contents_view {
            Some(v) => v.0(sections),
            None => default_contents(sections),
        };

        let entity = c.entity;
        commands.entity(entity).trigger(ContainerOpen);

        let window = bsn! {
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
                    Node Text("Contents")
                    --
                    Node { right: px(0.0) }
                    Button
                    Hovered
                    ImageNode { image: "close.png" }
                    on(close_window)
                ]
                --
                @sections
            ]
        };

        info!("new window for {c}: {}", commands.spawn_scene(window).id());
    }
}

/// Reverse map of section entity to inline contents.
#[derive(Resource, Debug, Default, Clone, Deref, DerefMut)]
pub(crate) struct InlinedContentsMap(EntityHashMap<Entity>);

pub(crate) fn update_inline_contents_map(
    mut commands: Commands,
    changed: Query<(Entity, &InlineContents), Changed<InlineContents>>,
    mut removed: RemovedComponents<InlineContents>,
    mut inlined: ResMut<InlinedContentsMap>,
) {
    for r in removed.read() {
        inlined.remove(&r);
        if let Ok(mut r) = commands.get_entity(r) {
            r.despawn_children();
        }
    }

    for (c, &InlineContents(section)) in changed {
        inlined.insert(section, c);
    }
}

pub(crate) fn update_inline_contents(
    mut commands: Commands,
    mut removed: RemovedComponents<Children>,
    sections: Query<(NameOrEntity, &Children), (With<GridContents>, Changed<Children>)>,
    items: Query<(NameOrEntity, Option<&ContentsView>, &Children), With<Item>>,
    inlined: Res<InlinedContentsMap>,
    // sections: Query<(Entity, &GridContents)>,
) {
    // Remove inlined contents when the `Children` component is removed from the source section.
    for r in removed.read() {
        if let Some(&i) = inlined.get(&r) {
            commands.entity(i).despawn_children();
        }
    }

    // Inlined contents relate to a single section.
    // The section contains items.
    // Each item (usually just one) correlates to a child node of the inlined contents entity.
    // The children of that child node are section views, much like that of an opened container.

    // When the children of a section are updated, we look for inlined contents.
    for (section, children) in &sections {
        info!("section contents changed: {section}");
        if let Some(&i) = inlined.get(&section.entity) {
            info!("inlined!");
            commands.entity(i).despawn_children();
            for (item, contents_view, sections) in items.iter_many(children).matched() {
                info!("updating inline contents for: {section}: {item}");
                let sections = sections.into_iter().copied().collect();
                let scene = match contents_view {
                    Some(v) => v.0(sections),
                    None => default_contents(sections),
                };
                commands.spawn_scene(scene).insert(ChildOf(i));
            }
        }
    }
}

fn close_window(
    event: On<Activate>,
    mut commands: Commands,
    child_of: Query<&ChildOf>,
    views: Query<&ContainerView>,
) {
    // Does this propagate up?
    let root = child_of.root_ancestor(event.event_target());
    if let Ok(&ContainerView(v)) = views.get(root) {
        commands.entity(v).remove::<Open>().trigger(ContainerClose);
        commands.entity(root).despawn();
    }
}

/// Move contents.
fn drag_by_header(
    mut event: On<PointerDrag>,
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
    event: On<PointerDragEnd>,
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

use bevy_asset::Handle;
use bevy_camera::visibility::Visibility;
use bevy_ecs::prelude::*;
use bevy_image::Image;
use bevy_input::{keyboard::KeyCode, *};
use bevy_math::*;
use bevy_picking::prelude::*;
use bevy_ui::{widget::ImageNode, *};
use tracing::*;

use crate::*;

/// `Item` icon.
#[derive(Component, Clone, Debug, FromTemplate)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Debug))]
pub struct Icon(pub Handle<Image>);

/// An item.
#[derive(Component, Clone, Debug)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Debug))]
#[require(ItemRotation)]
pub struct Item {
    /// The shape represents this items dimensions (and filled "slots" in case it is not rectangular).
    pub shape: Shape,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            shape: Shape::new([1, 1], true),
        }
    }
}

impl Item {
    /// Apply `rotation` to shape.
    // This should really only be used for temporary items. The actual item entities are stored unrotated and the rotation is applied when we need to check to see if it'll fit somewhere, or other operations where the rotation is pertinent.
    pub fn with_rotation(mut self, rotation: ItemRotation) -> Self {
        match rotation {
            ItemRotation::None => (),
            ItemRotation::R90 => self.shape = self.shape.rotate90(),
            ItemRotation::R180 => self.shape = self.shape.rotate180(),
            ItemRotation::R270 => self.shape = self.shape.rotate270(),
        }
        self
    }
}

/// Update the dragged item's transform when rotated or removed.
pub fn update_drag_rotation(
    mut commands: Commands,
    mut views: Query<&mut UiTransform>,
    items: Query<(NameOrEntity, &Item, &DragRotation, &ViewedBy), Changed<DragRotation>>,
    mut rm: RemovedComponents<DragRotation>,
    item_rotations: Query<(&Item, &ItemRotation, &ViewedBy)>,
) {
    for (i, Item { shape }, DragRotation(r), vs) in &items {
        let mut iter = views.iter_many_mut(vs).matched();
        while let Some(mut t) = iter.fetch_next() {
            t.rotation = r.rot2();
            let Vec2 { x, y } = r.offset(shape.size.as_vec2() * 48.0);
            t.translation = Val2::px(x, y);

            commands.entity(i.entity).trigger(ItemDragRotate);
        }
    }

    for e in rm.read() {
        if let Ok((Item { shape }, r, vs)) = item_rotations.get(e) {
            let mut iter = views.iter_many_mut(vs).matched();
            while let Some(mut t) = iter.fetch_next() {
                t.rotation = r.rot2();
                let Vec2 { x, y } = r.offset(shape.size.as_vec2() * 48.0);
                t.translation = Val2::px(x, y);
            }
        }
    }
}

/// Update the item view's node and transform when the item slot or size (via rotation) changes.
pub fn item_moved_or_rotated(
    items: Query<
        (NameOrEntity, &Item, &ItemRotation, &Slot, &ViewedBy),
        Or<(Changed<ItemRotation>, Changed<Slot>, Changed<ChildOf>)>,
    >,
    mut views: Query<(&mut Node, &mut UiTransform)>,
) {
    for (i, Item { shape }, rotation, Slot(slot), vs) in &items {
        let mut iter = views.iter_many_mut(vs).matched();
        while let Some((node, transform)) = iter.fetch_next() {
            info!("updated: {i}");
            update_item_view(shape, rotation, slot, node, transform);
        }
    }
}

/// Update the node and transform when an item view is added or changed.
pub fn item_view_changed(
    items: Query<(NameOrEntity, &Item, &ItemRotation, &Slot)>,
    mut views: Query<(&Viewing, &mut Node, &mut UiTransform), Changed<Viewing>>,
) {
    // We also need to update item views that are added (when opening containers). In theory there could be duplicates with the above system, but not in practice?
    for (&Viewing(v), node, transform) in &mut views {
        if let Ok((i, Item { shape }, rotation, Slot(slot))) = items.get(v) {
            info!("view changed: {i}");
            update_item_view(shape, rotation, slot, node, transform);
        }
    }
}

/// Updates an item's view based on its rotation and slot. Note the shape is the item's base (unrotated) shape.
pub fn update_item_view(
    &Shape { size, .. }: &Shape,
    rotation: &ItemRotation,
    slot: &UVec2,
    mut node: Mut<'_, Node>,
    mut transform: Mut<'_, UiTransform>,
) {
    // We don't need to apply the rotation to the size because the transform does.
    // let size = match rotation {
    //     ItemRotation::R90 | ItemRotation::R270 => size.yx(),
    //     _ => size,
    // };

    node.grid_row = GridPlacement::start_span((slot.y + 1) as i16, size.y as u16);
    node.grid_column = GridPlacement::start_span((slot.x + 1) as i16, size.x as u16);

    // Grid tracks are fixed, is this needed?
    let size = size.as_vec2() * 48.0;

    // The icons don't stretch in Bevy by default. Specifying the fixed grid tracks isn't enough to get the border at the right size, the cells weirdly take up more room when the neighboring cells aren't filled...
    node.width = px(size.x);
    node.height = px(size.y);
    // node.max_width = px(size.x as f32 * 48.0);
    // node.max_height = px(size.y as f32 * 48.0);
    // node.min_width = node.max_width;
    // node.min_height = node.max_height;

    // Update transform. What about scale?
    transform.rotation = rotation.rot2();
    let Vec2 { x, y } = rotation.offset(size);
    transform.translation = Val2::px(x, y);
}

/// Start dragging an item.
pub fn on_item_drag_start(
    event: On<PointerDragStart>,
    mut commands: Commands,
    views: Query<&Viewing>,
    sections: Query<&GridContents>,
    items: Query<(&Item, &ItemRotation, &Slot, &ViewedBy, &ChildOf)>,
) {
    let view = event.event_target();

    let Ok(&Viewing(id)) = views.get(view) else {
        return;
    };

    let Ok((item, rotation, &slot, vs, &ChildOf(section))) = items.get(id) else {
        return;
    };

    if let Ok(contents) = sections.get(section) {
        let item = item.clone().with_rotation(*rotation);
        let mut shape = contents.shape.clone();
        shape.unpaint(&item.shape, shape.index(slot.0));
        commands.entity(section).insert(DragShape(shape));

        commands
            .entity(view)
            .insert((GlobalZIndex(2), Pickable::IGNORE, FixedNode));
        commands
            .entity(id)
            .insert(DragRotation(*rotation))
            .trigger(|entity| ItemDragStart {
                entity,
                section,
                slot,
            });

        // Hide other item views.
        for &v in vs {
            if v != view {
                commands.entity(v).insert(Visibility::Hidden);
            }
        }
    }
}

pub fn on_item_drag(
    event: On<PointerDrag>,
    mut views: Query<(&mut Node, &Viewing)>,
    items: Query<Entity, With<Item>>,
) {
    if let Ok((mut node, &Viewing(id))) = views.get_mut(event.event_target())
        && items.contains(id)
    {
        // Update fixed node position.
        let d = event.pointer.position;
        node.position_type = PositionType::Absolute;
        node.left = px(d.x);
        node.top = px(d.y);
    }
}

fn pointer_slot(
    position: Vec2,
    section: &GridContents,
    transform: &UiGlobalTransform,
    node: &ComputedNode,
) -> UVec2 {
    // Get local position for the node.
    let p = transform.affine().inverse().transform_point2(position);

    // The transform origin is at the center of the node, so we add 0.5 to get it in the range of (0..1) in each axis.
    let p = p / node.size + Vec2::splat(0.5);

    // Note the pointer can be just outside the node's contents (in the border, for example) and thus we clamp the maximum edge in order to not return a slot outside the shape. We saturate to zero in the negative case so we don't need to clamp it.
    (section.size().as_vec2() * p)
        .as_uvec2()
        .min(section.shape.size - UVec2::ONE)
}

/// Inserts `DragSlot` in a target container if it accepts the dragged item.
pub fn on_item_drag_enter<T: Accepts>(
    mut event: On<PointerDragEnter>,
    mut commands: Commands,
    views: Query<(NameOrEntity, &Viewing)>,
    items: Query<(NameOrEntity, &Icon, &T, Option<&Children>), With<Item>>,
    sections: Query<(NameOrEntity, &T), With<GridContents>>,
) {
    if let Ok([(v, &Viewing(target)), (_, &Viewing(dragged))]) =
        views.get_many([event.event_target(), event.dragged])
        && let Ok((item, icon, item_flags, _)) = items.get(dragged)
    {
        // All items overlap the section that they're in. And we don't want to be inserting a drag slot in the parent section. Unless it's not a container!
        if let Ok((target, .., Some(xs))) = items.get(target)
            && sections.iter_many(xs).matched().next().is_some()
        {
            info!("drag enter item: {item} -> {target}");
            // TODO hit check for empty slots
            event.propagate(false);
        } else if let Ok((s, section_flags)) = sections.get(target) {
            // TODO: Visually indicate "no accepts". Gray/green/red?
            if section_flags.accepts(item_flags) {
                // Spawn item shadow.
                let shadow = commands
                    .spawn((
                        // Use item_view? We're not using Viewing...
                        Node::default(),
                        ImageNode::new(icon.0.clone()),
                        ChildOf(v.entity), // target section view
                        Pickable::IGNORE,
                        Visibility::Hidden, // unhide later
                    ))
                    .id();

                // Insert the drag slot and item shadow.
                commands
                    .entity(target)
                    .insert((DragSlot::default(), DragShadow(shadow)));

                event.propagate(false);
                info!("drag enter: {item} -> {s}");
            }
        }
    }
}

/// Sets the `DragSlot` for the currently hovered section.
pub fn on_item_drag_over(
    event: On<PointerDragOver>,
    mut commands: Commands,
    items: Query<(NameOrEntity, &Item, &ItemRotation, Option<&DragRotation>)>,
    mut sections: Query<(
        NameOrEntity,
        &GridContents,
        Option<&DragShape>,
        &mut DragSlot,
        &DragShadow,
    )>,
    views: Query<(&UiGlobalTransform, &ComputedNode, &Viewing)>,
    mut shadows: Query<(&mut Node, &mut UiTransform, &mut ImageNode, &mut Visibility)>,
) {
    if let Ok([(.., &Viewing(dragged)), (transform, node, &Viewing(target))]) =
        views.get_many([event.dragged, event.event_target()])
    {
        // Fetch the item we are dragging, and the target section we are hovering.
        if let Ok((i, item, rotation, drag_rotation)) = items.get(dragged)
            && let Ok((s, section, drag_shape, mut drag_slot, &DragShadow(shadow))) =
                sections.get_mut(target)
        {
            // Apply (drag) rotation.
            let item = item
                .clone()
                .with_rotation(drag_rotation.map_or(*rotation, |r| r.0));

            // Use the cached shape with the item unpainted when moving within the same container.
            let section_shape = drag_shape.map_or(&section.shape, |DragShape(s)| &s);
            let slot = pointer_slot(event.pointer.position, section, transform, node);
            let index = section_shape.index(slot);
            let new_slot = DragSlot(slot, section_shape.fits(&item.shape, index));

            if drag_slot.replace_if_neq(new_slot).is_some() {
                info!("drag over: {i} -> {s} slot: {new_slot}");

                commands.trigger(ItemDragOver {
                    entity: i.entity,
                    section: s.entity,
                    slot: Slot(slot),
                });

                // Update the item shadow.
                if let Ok((node, t, mut _image, mut vis)) = shadows.get_mut(shadow) {
                    update_item_view(&item.shape, rotation, &slot, node, t);
                    // FIX no color crate
                    // image.color.set_alpha(0.65);
                    vis.set_if_neq(Visibility::Inherited);
                }
            } //  else {
            //     warn!("does not fit: {slot}\n{}", &drag_shape.unwrap().0);
            // }
        }
    }
}

/// If dropped on an item, we attempt to find a section and slot for the item. If dropped on a suitable section slot we move it there.
pub fn on_item_drag_drop<T: Accepts>(
    event: On<PointerDragDrop>,
    views: Query<&Viewing>,
    mut items: Items<T>,
    drag_slot: Query<(Entity, &DragSlot)>,
    mut contents: Contents<T>,
) -> Result {
    // We only care about the original target? What if someone spawns something (text/icon?) inside the item? Then they'd have to be unpickable.
    let t = event.original_event_target();
    if t != event.event_target() {
        return Ok(());
    }

    // Fetch target section.
    let Ok(&Viewing(target)) = views.get(t) else {
        return Ok(());
    };

    // Fetch the dropped item.
    let Ok(&Viewing(v)) = views.get(event.dropped) else {
        return Ok(());
    };
    let Ok((id, slot, item, item_rotation, drag_rotation, &ChildOf(section), flags)) =
        items.get_mut(v)
    else {
        return Ok(());
    };

    // We need to check if this is an item we're dropping onto or contents. Should this check just be moved to resolve_drag? We're fetching the contents twice?
    if let Some(target) = if let Ok((id, &DragSlot(slot, fits))) = drag_slot.get(target) {
        fits.then_some((id, Slot(slot)))
    } else {
        // Item. Find a target.
        contents.find_section_slot(target, item, flags)
    } {
        if contents
            .resolve_drag(
                target,
                &id,
                slot,
                item,
                item_rotation,
                Some(drag_rotation.0),
                section,
            )
            .is_ok()
        {
            info!("drag resolved");
        }
    }

    Ok(())
}

// TODO: move key input to example only?
/// Send item to target container.
pub fn on_send_item<T: Accepts>(
    event: On<PointerClick>,
    input: Res<ButtonInput<KeyCode>>,
    views: Query<(NameOrEntity, &Viewing)>,
    targets: Query<&Target>,
    mut items: Query<(
        NameOrEntity,
        &mut Slot,
        &Item,
        &mut ItemRotation,
        &ChildOf,
        &T,
    )>,
    parents: Query<&ChildOf>,
    mut contents: Contents<T>,
) -> Result {
    let t = event.original_event_target();
    if t != event.event_target() {
        return Ok(());
    }

    match event.button {
        PointerButton::Primary
            if input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) =>
        {
            // Fetch the clicked item.
            let Ok((_id, &Viewing(v))) = views.get(t) else {
                return Ok(());
            };

            let Ok((i, slot, item, item_rotation, &ChildOf(section), flags)) = items.get_mut(v)
            else {
                return Ok(());
            };

            for p in parents.iter_ancestors(i.entity) {
                if let Ok(&Target(target)) = targets.get(p) {
                    if let Some(target) = contents.find_section_slot(
                        target,
                        // Apply the rotation when checking.
                        &item.clone().with_rotation(*item_rotation),
                        flags,
                    ) {
                        contents.resolve_drag(
                            target,
                            &i,
                            slot,
                            item,
                            item_rotation,
                            None,
                            section,
                        )?;
                        info!("sent item: {i}");
                    }
                    break;
                }
            }
            warn!("no target set in parent containers");
        }
        _ => (),
    }

    Ok(())
}

pub fn on_item_drag_end(
    event: On<PointerDragEnd>,
    mut commands: Commands,
    mut views: Query<(NameOrEntity, &Viewing, &mut Node)>,
    items: Query<(NameOrEntity, &ViewedBy, &ChildOf), With<Item>>,
    sections: Query<&DragSlot>,
) {
    let t = event.event_target();
    if let Ok((v, &Viewing(i), mut node)) = views.get_mut(t)
        && let Ok((i, vs, &ChildOf(s))) = items.get(i)
    {
        info!("drag end: {i}");
        commands
            .entity(v.entity)
            .remove::<(GlobalZIndex, Pickable, FixedNode)>();

        // Restore relative grid position.
        node.position_type = PositionType::Relative;
        node.left = Val::Auto;
        node.top = Val::Auto;

        let target = sections
            .get(s)
            .ok()
            .and_then(|&DragSlot(slot, fits)| fits.then_some((s, Slot(slot))));

        commands
            .entity(i.entity)
            .remove::<DragRotation>()
            .trigger(|entity| ItemDragEnd { entity, target });

        // Unhide other item views.
        for &v in vs {
            if v != t {
                commands.entity(v).insert(Visibility::Inherited);
            }
        }
    }
}

pub fn on_item_drag_leave(
    event: On<PointerDragLeave>,
    mut commands: Commands,
    views: Query<&Viewing>,
    // items: Query<NameOrEntity, With<Item>>,
    sections: Query<(NameOrEntity, &DragShadow), With<DragSlot>>,
) {
    // if let Ok(id) = items.get(event.event_target()) {
    //     info!("drag leave item: {id}");
    //     // event.propagate(false);
    // } else
    if let Ok(&Viewing(s)) = views.get(event.event_target())
        && let Ok((s, &DragShadow(shadow))) = sections.get(s)
    {
        info!("drag leave: {s}");
        commands
            .entity(s.entity)
            .remove::<(DragShape, DragSlot, DragShadow)>();

        // Despawn shadow entity.
        commands.entity(shadow).despawn();

        // event.propagate(false);
    }
}

// How do we trigger this?
pub fn on_item_drag_cancel(event: On<PointerCancel>) {
    warn!("cancel! {}", event.event_target());
}

// TODO: rename?
/// Clockwise rotation.
#[derive(Component, Copy, Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(feature = "reflect", reflect(Component, Debug))]
pub enum ItemRotation {
    #[default]
    None,
    R90,
    R180,
    R270,
}

impl ItemRotation {
    pub fn increment(&self) -> Self {
        match self {
            Self::None => Self::R90,
            Self::R90 => Self::R180,
            Self::R180 => Self::R270,
            _ => Self::None,
        }
    }

    pub fn angle(&self) -> f32 {
        match *self {
            Self::None => 0.0,
            Self::R90 => 90.0_f32.to_radians(),
            Self::R180 => 180.0_f32.to_radians(),
            Self::R270 => 270.0_f32.to_radians(),
        }
    }

    /// Returns a `Rot2` for the current rotation. `ItemRotation` is clockwise relative to the screen and user (+Y is down), so the rotation this returns is inverted.
    /// ```
    /// # use runic::ItemRotation;
    /// # use bevy_math::Rot2;
    /// assert_eq!(ItemRotation::R90.rot2().as_degrees(), 90.0);
    /// assert_eq!(ItemRotation::R180.rot2().as_degrees(), 180.0);
    /// assert_eq!(ItemRotation::R270.rot2().as_degrees(), -90.0);
    /// ```
    pub const fn rot2(&self) -> Rot2 {
        match self {
            Self::None => Rot2::IDENTITY,
            Self::R90 => Rot2::FRAC_PI_2,
            Self::R180 => Rot2::PI,
            Self::R270 => Rot2::FRAC_PI_2.inverse(),
        }
    }

    /// Bevy rotates from the center. We're not just rotating the item, we're trying to maintain its upper left corner in the current slot. So non-square items will need to be offset. This function returns that offset.
    fn offset(&self, size: Vec2) -> Vec2 {
        use bevy_math::Vec2Swizzles;

        match self {
            ItemRotation::R90 | ItemRotation::R270 => (size.yx() - size) * 0.5,
            // Center pivot is fine.
            _ => Vec2::ZERO,
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
// }

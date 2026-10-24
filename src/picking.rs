//! Picking filter for items with partially-filled grids.

use bevy_ecs::prelude::*;
use bevy_math::*;
use bevy_picking::backend::PointerHits;

use crate::{Item, ItemRotation, Viewing};

pub(crate) fn filter_item_hits(
    mut hits: MessageMutator<PointerHits>,
    names: Query<(Entity, Option<&Name>)>,
    views: Query<(NameOrEntity, &Viewing)>,
    items: Query<(NameOrEntity, &Item, &ItemRotation)>,
) {
    for pointer_hits in hits.read() {
        let picks = &mut pointer_hits.picks;

        for (i, (e, n)) in names
            .iter_many(picks.iter().map(|(e, _)| e))
            .matched()
            .enumerate()
        {
            let n = n.map(|n| n.as_str()).unwrap_or("-");
            tracing::info!("picking {:?} {i}: {e} {n}", pointer_hits.order);
        }

        picks.retain(|(entity, hit)| {
            if let Ok((_i, &Viewing(v))) = views.get(*entity)
                && let Ok((_, item, rotation)) = items.get(v)
                && let Some(p) = hit.position
            {
                // Apply rotation to item. We can't just use the view's computed node since it doesn't have the fill.
                let item = item.clone().with_rotation(*rotation);
                let shape = &item.shape;

                // Find slot from pointer location. DRY?
                let slot = ((p.truncate() + Vec2::splat(0.5)) * shape.size.as_vec2())
                    .floor()
                    .as_uvec2()
                    .min(shape.size - UVec2::ONE);

                // Retain if the slot is filled.
                shape.fill[shape.index(slot)]
            } else {
                true
            }
        });
    }
}

use std::{collections::HashMap, path::PathBuf};

use eframe::egui;
use egui_extras::RetainedImage;
use runic::*;

use bitvec::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "runic",
        options,
        Box::new(|cc| {
            let mut contents: HashMap<usize, (Box<dyn Contents>, Vec<(usize, Item)>)> =
                HashMap::new();
            let mut images = HashMap::new();

            contents.insert(
                1,
                (
                    Box::new(GridContents::new(1, 4, 4)),
                    vec![(
                        0,
                        Item::new(
                            3,
                            load_image(&mut images, "pipe").texture_id(&cc.egui_ctx),
                            shape::Shape::from_bits(2, bits![1, 1, 1, 0]),
                        ),
                    )],
                ),
            );

            contents.insert(
                2,
                (
                    Box::new(GridContents::new(2, 2, 2)),
                    vec![(
                        0,
                        Item::new(
                            4,
                            load_image(&mut images, "potion-icon-24").texture_id(&cc.egui_ctx),
                            shape::Shape::new((1, 1), true),
                        ),
                    )],
                ),
            );

            contents.insert(
                5,
                (
                    Box::new(SectionContainer {
                        id: 5,
                        layout: SectionLayout::Grid(2),
                        sections: vec![
                            // uses the same id?
                            GridContents::new(5, 1, 2),
                            GridContents::new(5, 1, 2),
                        ],
                    }),
                    Vec::new(), // empty
                ),
            );

            contents.insert(
                6,
                (
                    Box::new(ExpandingContainer::new(6, (2, 2))),
                    Vec::new(), // empty
                ),
            );

            Box::new(Runic {
                images,
                drag_item: None,
                contents,
            })
        }),
    )
}

//#[derive(Default)]
struct Runic {
    #[allow(dead_code)]
    images: HashMap<&'static str, RetainedImage>,
    drag_item: Option<DragItem>,
    contents: HashMap<usize, (Box<dyn Contents>, Vec<(usize, Item)>)>,
}

fn load_image<'a>(
    images: &'a mut HashMap<&'static str, RetainedImage>,
    name: &'static str,
) -> &'a RetainedImage {
    images.entry(name).or_insert_with(|| {
        let mut path = PathBuf::from(name);
        path.set_extension("png");
        std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|buf| RetainedImage::from_image_bytes(name, &buf))
            .unwrap_or_else(|e| {
                tracing::error!("failed to load image at: {} ({})", path.display(), e);
                RetainedImage::from_color_image(name, egui::ColorImage::example())
            })
    })
}

impl eframe::App for Runic {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(((drag_item, prev, _slot, _), (container, slot))) =
                ContainerSpace::show(&mut self.drag_item, ui, |drag_item, ui| {
                    // need to resolve how these results are merged with a
                    // hierarchy of containers and items
                    self.contents
                        .values_mut()
                        .map(|(c, items)| {
                            // we are ignoring all but the last
                            // response...
                            ui.label(format!("Container {}", c.id()));
                            c.ui(drag_item, items, ui).inner
                        })
                        .reduce(|acc, d| acc.merge(d))
                        .unwrap_or_default()
                })
            {
                tracing::info!("moving item {:?} -> container {:?}", drag_item, container);

                // FIX add/remove makes no sense if contents are builders

                match self.contents.get_mut(&prev).and_then(|(c, items)| {
                    let idx = items.iter().position(|(_, item)| item.id == drag_item.id);
                    idx.map(|i| {
                        let (slot, item) = items.remove(i);
                        c.remove(ctx, slot, &item); // unpaint
                        item
                    })
                }) {
                    Some(mut item) => {
                        tracing::info!("new rot {:?} --> {:?}", item.rotation, drag_item.rotation);
                        // Copy the rotation.
                        item.rotation = drag_item.rotation;

                        match self.contents.get_mut(&container) {
                            Some((c, items)) => {
                                c.add(ctx, slot, &item); // paint
                                items.push((slot, item));
                            }
                            None => {
                                tracing::error!("could not find container {} to add to", container)
                            }
                        }
                    }
                    None => tracing::error!("could not find container {} to remove from", prev),
                }
            }
        });
    }
}

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
            let mut runic = Runic::default();

            // this should pull from a data source only when it changes,
            // and only for open containers?

            let mut container = Container::new(1, 4, 4);
            container.add(
                0,
                Item::new(
                    2,
                    runic.load_image("pipe").texture_id(&cc.egui_ctx),
                    shape::Shape::from_bits(2, bits![1, 1, 1, 0]),
                ),
            );
            runic.containers.push(container);

            let mut container = Container::new(3, 2, 2);
            container.add(
                0,
                Item::new(
                    4,
                    runic.load_image("potion-icon-24").texture_id(&cc.egui_ctx),
                    shape::Shape::new((1, 1), true),
                ),
            );
            runic.containers.push(container);

            Box::new(runic)
        }),
    )
}

#[derive(Default)]
struct Runic {
    images: HashMap<&'static str, RetainedImage>,
    drag_item: Option<DragItem>,
    containers: Vec<Container>,
}

impl Runic {
    fn load_image(&mut self, name: &'static str) -> &RetainedImage {
        self.images.entry(name).or_insert_with(|| {
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
}

impl eframe::App for Runic {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(((drag_item, prev, _slot, _), (container, slot))) =
                ContainerSpace::show(&mut self.drag_item, ui, |drag_item, ui| {
                    // need to resolve how these results are merged with a
                    // hierarchy of containers and items
                    self.containers.iter().fold(MoveData::default(), |data, c| {
                        // we are ignoring all but the last response...
                        data.merge(c.ui(drag_item, ui).inner)
                    })
                })
            {
                tracing::info!("moving item {:?} -> container {:?}", drag_item, container);

                if let Some(mut item) = self
                    .containers
                    .iter_mut()
                    .find(|c| c.id == prev)
                    .and_then(|c| c.remove(drag_item.id))
                {
                    tracing::info!("new rot {:?} --> {:?}", item.rotation, drag_item.rotation);
                    // Copy the rotation.
                    item.rotation = drag_item.rotation;

                    if let Some(c) = self.containers.iter_mut().find(|c| c.id == container) {
                        c.add(slot, item);
                    }
                }
            }
        });
    }
}

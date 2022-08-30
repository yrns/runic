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
            // this should pull from a data source only when it changes,
            // and only for open containers?

            let mut images = HashMap::new();
            let mut containers = Vec::new();
            let mut container = Container::new(1, 4, 4);
            container.add(
                0,
                Item::new(
                    3,
                    load_image(&mut images, "pipe").texture_id(&cc.egui_ctx),
                    shape::Shape::from_bits(2, bits![1, 1, 1, 0]),
                ),
            );
            containers.push(container);

            let mut container = Container::new(2, 2, 2);
            container.add(
                0,
                Item::new(
                    4,
                    load_image(&mut images, "potion-icon-24").texture_id(&cc.egui_ctx),
                    shape::Shape::new((1, 1), true),
                ),
            );
            containers.push(container);

            Box::new(Runic {
                images,
                drag_item: None,
                containers,
                section_container: SectionContainer {
                    id: 5,
                    layout: SectionLayout::Grid(2),
                    sections: vec![
                        Container::new(6, 1, 1),
                        Container::new(7, 1, 1),
                        Container::new(8, 1, 1),
                        Container::new(9, 1, 1),
                    ],
                },
            })
        }),
    )
}

//#[derive(Default)]
struct Runic {
    #[allow(dead_code)]
    images: HashMap<&'static str, RetainedImage>,
    drag_item: Option<DragItem>,
    // Vec<Box<impl Container>>?
    containers: Vec<Container>,
    section_container: SectionContainer,
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
                    let data = self
                        .containers
                        .iter()
                        .map(|c| {
                            // we are ignoring all but the last
                            // response...
                            ui.label(format!("Container {}", c.id));
                            c.ui(drag_item, ui).inner
                        })
                        .reduce(|acc, d| acc.merge(d))
                        .unwrap_or_default();

                    ui.label("Sectioned Container");
                    data.merge(self.section_container.ui(drag_item, ui).inner)
                })
            {
                tracing::info!("moving item {:?} -> container {:?}", drag_item, container);

                match self
                    .containers
                    .iter_mut()
                    .chain(self.section_container.sections.iter_mut())
                    .find(|c| c.id == prev)
                    .and_then(|c| c.remove(drag_item.id))
                {
                    Some(mut item) => {
                        tracing::info!("new rot {:?} --> {:?}", item.rotation, drag_item.rotation);
                        // Copy the rotation.
                        item.rotation = drag_item.rotation;

                        match self
                            .containers
                            .iter_mut()
                            .chain(self.section_container.sections.iter_mut())
                            .find(|c| c.id == container)
                        {
                            Some(c) => c.add(slot, item),
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

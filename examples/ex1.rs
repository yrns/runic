use eframe::egui;
use egui_extras::RetainedImage;
use runic::*;

fn main() {
    tracing_subscriber::fmt::init();
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "runic",
        options,
        Box::new(|cc| {
            let icon = RetainedImage::from_image_bytes(
                "potion-icon-24.png",
                include_bytes!("../potion-icon-24.png",),
            )
            .unwrap();

            //let icon = RetainedImage::from_color_image("example", egui::ColorImage::example());

            // this should pull from a data source only when it changes,
            // and only for open containers?

            let mut container1 = Container::new(1, 4, 4);
            container1.add(
                0,
                Item::new(
                    2,
                    icon.texture_id(&cc.egui_ctx),
                    shape::Shape::new((2, 2), true),
                ),
            );

            let mut container2 = Container::new(3, 2, 2);
            container2.add(
                0,
                Item::new(
                    4,
                    icon.texture_id(&cc.egui_ctx),
                    shape::Shape::new((1, 1), true),
                ),
            );

            Box::new(Runic {
                icon,
                drag_item: None,
                container1,
                container2,
            })
        }),
    )
}

struct Runic {
    #[allow(dead_code)]
    icon: RetainedImage,
    drag_item: Option<DragItem>,
    container1: Container,
    container2: Container,
}

impl eframe::App for Runic {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(((drag_item, prev, _slot), container)) =
                ContainerSpace::show(&mut self.drag_item, ui, |drag_item, ui| {
                    // need to resolve how these results are merged with a
                    // hierarchy of containers and items
                    self.container1
                        .ui(drag_item, ui)
                        .inner
                        .merge(self.container2.ui(drag_item, ui).inner)
                })
            {
                tracing::info!("moving item {:?} -> container {:?}", drag_item, container);

                if let Some(mut item) = match prev {
                    1 => self.container1.remove(drag_item.id),
                    3 => self.container2.remove(drag_item.id),
                    _ => None,
                } {
                    // Copy the rotation.
                    item.rotation = drag_item.rotation;
                    match container {
                        (1, slot) => self.container1.add(slot, item), // FIX,
                        (3, slot) => self.container2.add(slot, item),
                        _ => (),
                    }
                }

                tracing::info!("container1 items: {:?}", self.container1.items.len());
                tracing::info!("container2 items: {:?}", self.container2.items.len());
            }
        });
    }
}

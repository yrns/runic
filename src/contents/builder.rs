use super::*;

pub struct ContentsBuilder<T> {
    name: Option<Name>,
    item: Option<Item<T>>,
    contents: Option<BoxedContents<T>>,
    section_layout: Option<egui::Layout>,
    sections: Vec<ContentsBuilder<T>>,
    items: Vec<ContentsBuilder<T>>,
}

impl<T> Default for ContentsBuilder<T> {
    fn default() -> Self {
        Self {
            name: None,
            item: None,
            contents: None,
            section_layout: None,
            sections: Vec::new(),
            items: Vec::new(),
        }
    }
}

// Too bad we can't do contents generically.
impl<T> From<BoxedContents<T>> for ContentsBuilder<T> {
    fn from(contents: BoxedContents<T>) -> Self {
        Self {
            contents: Some(contents),
            ..Default::default()
        }
    }
}

impl<T> From<GridContents<T>> for ContentsBuilder<T>
where
    T: Accepts + Copy + std::fmt::Debug,
{
    fn from(contents: GridContents<T>) -> Self {
        Self {
            contents: Some(contents.boxed()),
            ..Default::default()
        }
    }
}

pub trait ContentsExt<T> {
    fn builder(self) -> ContentsBuilder<T>;
}

impl<C, T> ContentsExt<T> for C
where
    T: Accepts,
    C: Contents<T> + Send + Sync + 'static,
{
    fn builder(self) -> ContentsBuilder<T> {
        ContentsBuilder::contents(self)
    }
}

impl<T> ContentsBuilder<T> {
    pub fn name(name: Name) -> Self {
        Self {
            name: Some(name),
            ..Default::default()
        }
    }

    pub fn with_name(mut self, name: Name) -> Self {
        self.name = Some(name);
        self
    }

    pub fn item(item: Item<T>) -> Self {
        Self::default().with_item(item)
    }

    pub fn with_item(mut self, item: Item<T>) -> Self {
        self.item = Some(item);
        self
    }

    pub fn contents<C>(contents: C) -> Self
    where
        T: Accepts,
        C: Contents<T> + Send + Sync + 'static,
    {
        Self::default().with_contents(contents)
    }

    pub fn with_contents<C>(mut self, contents: C) -> Self
    where
        T: Accepts,
        C: Contents<T> + Send + Sync + 'static,
    {
        self.contents = Some(contents.boxed());
        self
    }

    pub fn with_section_layout(mut self, layout: egui::Layout) -> Self {
        self.section_layout = Some(layout);
        self
    }

    pub fn with_sections<C: Into<ContentsBuilder<T>>, I: IntoIterator<Item = C>>(
        mut self,
        sections: I,
    ) -> Self {
        self.sections = sections.into_iter().map(|c| c.into()).collect();
        self
    }

    pub fn with_items(mut self, items: impl IntoIterator<Item = ContentsBuilder<T>>) -> Self {
        self.items = items.into_iter().collect();
        self
    }
}

impl<T: Accepts + Clone> ContentsStorage<'_, '_, T> {
    pub fn spawn(&mut self, contents: impl Into<ContentsBuilder<T>>) -> Entity {
        let ContentsBuilder {
            name,
            item,
            contents,
            section_layout,
            sections,
            items,
        } = contents.into();

        assert!(item.is_some() || contents.is_some(), "item and/or contents");

        assert!(
            sections.is_empty() || contents.is_some(),
            "sections => contents"
        );

        let sections = Sections(
            section_layout,
            sections.into_iter().map(|i| self.spawn(i)).collect(),
        );

        assert!(items.is_empty() || contents.is_some(), "items => contents");

        let contents_items = contents.map(|contents| {
            let mut contents_items = ContentsItems {
                contents,
                items: Vec::new(),
            };

            // This is convoluted because we can't fetch anything. None of these items exist yet.
            for item in items {
                // This clone is not ideal.
                let Some(item_clone) = item.item.clone() else {
                    panic!("not an item")
                };

                // Recursively spawn the item (and its contents, if any).
                let id = self.spawn(item);

                // Fake drag again...
                let drag = DragItem::new(id, item_clone);
                let Some((_id, slot)) = contents_items.contents.find_slot(id, &drag) else {
                    panic!("no slot for item");
                };

                // The item might fit in a sub-container, but we don't have access to place it there. It would otherwise be viable. Fix?
                assert_eq!(id, _id, "item fits in current container");

                let DragItem { item, .. } = drag;
                contents_items.insert(slot, id, &item);
            }

            contents_items
        });

        let mut e = self.commands.spawn_empty();

        // Insert contents (and items).
        if let Some(contents_items) = contents_items {
            e.insert(contents_items);
        }

        // Insert sections.
        if !sections.1.is_empty() {
            e.insert(sections);
        }

        // Insert item.
        if let Some(item) = item {
            // TODO: name optional?
            e.insert((name.expect("item has a name"), item));
        }

        e.id()
    }
}

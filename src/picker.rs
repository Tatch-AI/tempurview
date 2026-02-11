/// A single item in a picker overlay
pub struct PickerItem {
    /// Shortcut key to quick-select this item
    pub key: char,
    /// Display text shown in the picker
    pub label: String,
}

/// State for a picker overlay modal
pub struct PickerState {
    /// Available items to choose from
    pub items: Vec<PickerItem>,
    /// Currently highlighted item index
    pub selected: usize,
    /// Title displayed at the top of the picker
    pub title: String,
}

impl PickerState {
    pub fn new(title: impl Into<String>, items: Vec<PickerItem>) -> Self {
        Self {
            items,
            selected: 0,
            title: title.into(),
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    pub fn selected_key(&self) -> Option<char> {
        self.items.get(self.selected).map(|item| item.key)
    }
}

use chromiumoxide::page::Page;

/// Manages multiple browser tabs/pages.
pub struct TabPool {
    pages: Vec<Page>,
    active_index: usize,
}

impl TabPool {
    pub fn new(initial_page: Page) -> Self {
        Self {
            pages: vec![initial_page],
            active_index: 0,
        }
    }

    pub fn active_page(&self) -> &Page {
        &self.pages[self.active_index]
    }

    pub fn add_page(&mut self, page: Page) {
        self.pages.push(page);
        self.active_index = self.pages.len() - 1;
    }

    pub fn select_page(&mut self, index: usize) -> Option<&Page> {
        if index < self.pages.len() {
            self.active_index = index;
            Some(&self.pages[self.active_index])
        } else {
            None
        }
    }

    pub fn select_by_target_id(&mut self, target_id: &str) -> Option<&Page> {
        for (i, page) in self.pages.iter().enumerate() {
            if page.target_id().as_ref() == target_id {
                self.active_index = i;
                return Some(page);
            }
        }
        None
    }

    pub fn remove_page(&mut self, target_id: &str) -> bool {
        if let Some(pos) = self
            .pages
            .iter()
            .position(|p| p.target_id().as_ref() == target_id)
        {
            self.pages.remove(pos);
            if self.active_index >= self.pages.len() && !self.pages.is_empty() {
                self.active_index = self.pages.len() - 1;
            }
            true
        } else {
            false
        }
    }

    pub fn list_pages(&self) -> &[Page] {
        &self.pages
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

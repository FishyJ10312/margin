use crate::app_models::{CatalogHistory, PhotoSelection, RemovedPhotos, REMOVAL_HISTORY_LIMIT};
use std::collections::{HashMap, HashSet};

impl PhotoSelection {
    pub fn single(id: String) -> Self {
        Self {
            active_id: id.clone(),
            ids: if id.is_empty() {
                vec![]
            } else {
                vec![id.clone()]
            },
            anchor_id: id,
        }
    }

    pub fn restored(&self, order: &[String], aliases: &HashMap<String, String>) -> Self {
        let map = |id: &String| aliases.get(id).unwrap_or(id).clone();
        let wanted: HashSet<String> = self.ids.iter().map(map).collect();
        let ids: Vec<String> = order
            .iter()
            .filter(|id| wanted.contains(*id))
            .cloned()
            .collect();
        let active = map(&self.active_id);
        let active_id = if order.contains(&active) {
            active
        } else {
            ids.first().or(order.first()).cloned().unwrap_or_default()
        };
        let anchor = map(&self.anchor_id);
        Self {
            anchor_id: if order.contains(&anchor) {
                anchor
            } else {
                active_id.clone()
            },
            active_id,
            ids,
        }
    }

    pub fn after_removal(&self, order: &[String], removed: &HashSet<String>) -> Self {
        let remaining: Vec<String> = order
            .iter()
            .filter(|id| !removed.contains(*id))
            .cloned()
            .collect();
        if remaining.is_empty() {
            return Self::default();
        }
        let previous_index = order
            .iter()
            .position(|id| id == &self.active_id)
            .unwrap_or(0);
        let next = order
            .iter()
            .skip(previous_index)
            .find(|id| !removed.contains(*id))
            .unwrap_or(remaining.last().unwrap());
        let wanted: HashSet<&String> = self.ids.iter().collect();
        let ids: Vec<String> = remaining
            .iter()
            .filter(|id| wanted.contains(*id))
            .cloned()
            .collect();
        if ids.is_empty() {
            return Self::single(if remaining.contains(&self.active_id) {
                self.active_id.clone()
            } else {
                next.clone()
            });
        }
        let active_id = if ids.contains(&self.active_id) {
            self.active_id.clone()
        } else {
            order
                .iter()
                .skip(previous_index)
                .find(|id| ids.contains(*id))
                .unwrap_or(ids.last().unwrap())
                .clone()
        };
        Self {
            anchor_id: if remaining.contains(&self.anchor_id) {
                self.anchor_id.clone()
            } else {
                active_id.clone()
            },
            active_id,
            ids,
        }
    }
}

impl CatalogHistory {
    pub fn trim(&mut self) -> Vec<RemovedPhotos> {
        let mut expired = Vec::new();
        while self.undo.len() + self.redo.len() > REMOVAL_HISTORY_LIMIT {
            // Undo is chronological; redo is reverse chronological.
            expired.push(
                self.undo
                    .pop_front()
                    .or_else(|| self.redo.pop_back())
                    .unwrap(),
            );
        }
        expired
    }

    pub fn sanitize(&mut self) -> bool {
        let mut ids = HashSet::new();
        let invalid = self.recovered
            || self.undo.iter().chain(&self.redo).any(|r| {
                let mut photos = HashSet::new();
                r.id.is_empty()
                    || !ids.insert(r.id.clone())
                    || r.photos.is_empty()
                    || r.photos.iter().any(|p| {
                        p.id.is_empty()
                            || p.path.is_empty()
                            || p.width == 0
                            || p.height == 0
                            || !photos.insert(p.id.clone())
                    })
            });
        if invalid {
            self.undo.clear();
            self.redo.clear();
        }
        self.recovered = false;
        self.trim();
        for record in self.undo.iter_mut().chain(&mut self.redo) {
            for photo in &mut record.photos {
                photo.thumbnail = String::new();
            }
        }
        invalid
    }
}

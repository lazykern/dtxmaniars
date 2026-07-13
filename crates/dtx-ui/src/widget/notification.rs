use std::collections::VecDeque;

use bevy::prelude::{Component, Resource};

/// Marker for the screen-independent notification text entities.
#[derive(Component, Debug, Clone, Copy)]
pub struct GlobalNotificationRoot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationTone {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub message: String,
    pub tone: NotificationTone,
    age_ms: u64,
    lifetime_ms: u64,
}

impl Notification {
    fn new(message: impl Into<String>, tone: NotificationTone, lifetime_ms: u64) -> Self {
        Self {
            message: message.into(),
            tone,
            age_ms: 0,
            lifetime_ms,
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message, NotificationTone::Info, 3_500)
    }
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message, NotificationTone::Success, 3_500)
    }
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message, NotificationTone::Warning, 5_000)
    }
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, NotificationTone::Error, 7_000)
    }
    pub const fn lifetime_ms(&self) -> u64 {
        self.lifetime_ms
    }
}

impl From<String> for Notification {
    fn from(message: String) -> Self {
        Self::info(message)
    }
}

impl From<&str> for Notification {
    fn from(message: &str) -> Self {
        Self::info(message)
    }
}

#[derive(Resource, Debug, Clone)]
pub struct NotificationQueue {
    capacity: usize,
    entries: VecDeque<Notification>,
}

impl Default for NotificationQueue {
    fn default() -> Self {
        Self::with_capacity(4)
    }
}

impl NotificationQueue {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            entries: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, notification: impl Into<Notification>) {
        if self.capacity == 0 {
            return;
        }
        while self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(notification.into());
    }

    pub fn tick(&mut self, delta_ms: u64) {
        for entry in &mut self.entries {
            entry.age_ms = entry.age_ms.saturating_add(delta_ms);
        }
        self.entries
            .retain(|entry| entry.age_ms < entry.lifetime_ms);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Notification> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_are_bounded_and_errors_live_long_enough() {
        let mut queue = NotificationQueue::with_capacity(4);
        for n in 0..5 {
            queue.push(Notification::info(n.to_string()));
        }
        assert_eq!(queue.len(), 4);
        assert!(Notification::error("save failed").lifetime_ms() >= 5_000);
    }
}

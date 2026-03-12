use std::sync::Mutex;

use boltffi::*;

/// A bounded collection of named items.
///
/// Shows multiple constructor patterns: a default constructor,
/// a parameterized one, and a fallible one that returns Result.
pub struct Inventory {
    items: Mutex<Vec<String>>,
    capacity: u32,
}

#[export]
impl Inventory {
    pub fn new() -> Self {
        Self {
            items: Mutex::new(Vec::new()),
            capacity: 100,
        }
    }

    pub fn with_capacity(capacity: u32) -> Self {
        Self {
            items: Mutex::new(Vec::new()),
            capacity,
        }
    }

    /// Creates an inventory, or fails if capacity is zero.
    pub fn try_new(capacity: u32) -> Result<Self, String> {
        if capacity == 0 {
            Err("capacity must be greater than zero".to_string())
        } else {
            Ok(Self {
                items: Mutex::new(Vec::new()),
                capacity,
            })
        }
    }

    pub fn count(&self) -> u32 {
        self.items.lock().unwrap().len() as u32
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn add(&self, item: String) -> bool {
        let mut items = self.items.lock().unwrap();
        if (items.len() as u32) < self.capacity {
            items.push(item);
            true
        } else {
            false
        }
    }

    pub fn remove(&self, index: u32) -> Option<String> {
        let mut items = self.items.lock().unwrap();
        if (index as usize) < items.len() {
            Some(items.remove(index as usize))
        } else {
            None
        }
    }

    pub fn get_all(&self) -> Vec<String> {
        self.items.lock().unwrap().clone()
    }
}

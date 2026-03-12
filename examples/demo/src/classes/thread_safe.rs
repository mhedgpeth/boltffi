use std::sync::Mutex;

use boltffi::*;

pub struct SharedCounter {
    value: Mutex<i32>,
}

impl Default for SharedCounter {
    fn default() -> Self {
        Self::new(0)
    }
}

#[export]
impl SharedCounter {
    pub fn new(initial: i32) -> Self {
        Self {
            value: Mutex::new(initial),
        }
    }

    pub fn get(&self) -> i32 {
        *self.value.lock().unwrap()
    }

    pub fn set(&self, value: i32) {
        *self.value.lock().unwrap() = value;
    }

    pub fn increment(&self) -> i32 {
        let mut guard = self.value.lock().unwrap();
        *guard += 1;
        *guard
    }

    pub fn add(&self, amount: i32) -> i32 {
        let mut guard = self.value.lock().unwrap();
        *guard += amount;
        *guard
    }

    pub async fn async_get(&self) -> i32 {
        *self.value.lock().unwrap()
    }

    pub async fn async_add(&self, amount: i32) -> i32 {
        let mut guard = self.value.lock().unwrap();
        *guard += amount;
        *guard
    }
}

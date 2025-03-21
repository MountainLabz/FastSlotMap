use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Key {
    index: u32,
    generation: u32,
}

pub struct FastSlotMap<T> {
    values: Vec<T>,               // Packed storage for values
    generations: Vec<u32>,        // Tracks slot validity
    next_free: Vec<AtomicU32>,    // Atomic free-list for lock-free operations
    free_head: AtomicU32,         // Head of free list (lock-free)
    len: AtomicU32,               // Number of active elements
}

impl<T: Default + Copy> FastSlotMap<T> {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            generations: Vec::new(),
            next_free: Vec::new(),
            free_head: AtomicU32::new(u32::MAX),
            len: AtomicU32::new(0),
        }
    }

    pub fn insert(&mut self, value: T) -> Key {
        let index;
        let generation;

        loop {
            let free_index = self.free_head.load(Ordering::Acquire);

            if free_index != u32::MAX {
                // Try to pop from the free list
                let next_free = self.next_free[free_index as usize].load(Ordering::Relaxed);
                if self.free_head.compare_exchange(free_index, next_free, Ordering::Release, Ordering::Relaxed).is_ok() {
                    index = free_index;
                    generation = self.generations[index as usize];
                    self.values[index as usize] = value;
                    self.len.fetch_add(1, Ordering::Relaxed);
                    return Key { index, generation };
                }
            } else {
                // Allocate a new slot
                index = self.values.len() as u32;
                self.values.push(value);
                self.generations.push(0);
                self.next_free.push(AtomicU32::new(u32::MAX));
                self.len.fetch_add(1, Ordering::Relaxed);
                return Key { index, generation: 0 };
            }
        }
    }

    pub fn get(&self, key: Key) -> Option<&T> {
        self.values.get(key.index as usize).filter(|_| self.generations[key.index as usize] == key.generation)
    }

    pub fn get_mut(&mut self, key: Key) -> Option<&mut T> {
        self.values.get_mut(key.index as usize).filter(|_| self.generations[key.index as usize] == key.generation)
    }

    pub fn remove(&mut self, key: Key) -> Option<T> {
        if self.generations[key.index as usize] == key.generation {
            self.generations[key.index as usize] = self.generations[key.index as usize].wrapping_add(1);
            self.len.fetch_sub(1, Ordering::Relaxed);
            let value = self.values[key.index as usize];

            // Push this slot to the freelist
            let mut free_head = self.free_head.load(Ordering::Acquire);
            loop {
                self.next_free[key.index as usize].store(free_head, Ordering::Relaxed);
                if self.free_head.compare_exchange(free_head, key.index, Ordering::Release, Ordering::Relaxed).is_ok() {
                    return Some(value);
                }
                free_head = self.free_head.load(Ordering::Acquire);
            }
        }
        None
    }

    pub fn contains(&self, key: Key) -> bool {
        self.get(key).is_some()
    }

    pub fn len(&self) -> u32 {
        self.len.load(Ordering::Relaxed)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

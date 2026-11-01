// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Tile & Page Bitmap Cache with O(1) LRU eviction and Buffer Pool

#![allow(dead_code)]

use std::collections::HashMap;

/// Represents a rendered RGBA/BGRA page raster in memory.
pub struct CachedBitmap {
    pub page_index: usize,
    pub width: i32,
    pub height: i32,
    pub stride: i32,
    pub zoom_pct: u32,
    pub rotation: i32,
    pub pixels: Vec<u8>,
    pub last_access: u64,
}

impl CachedBitmap {
    pub fn byte_size(&self) -> usize {
        self.pixels.len()
    }
}

/// A buffer pool that reuses allocated pixel vectors to eliminate GC/heap churn during fast scrolling.
pub struct BufferPool {
    pool: Vec<Vec<u8>>,
    max_retained: usize,
}

impl BufferPool {
    pub fn new(max_retained: usize) -> Self {
        Self {
            pool: Vec::with_capacity(max_retained),
            max_retained,
        }
    }

    pub fn acquire(&mut self, capacity: usize) -> Vec<u8> {
        // Find best-matching vector if available
        if let Some(pos) = self.pool.iter().position(|v| v.capacity() >= capacity) {
            let mut v = self.pool.swap_remove(pos);
            v.resize(capacity, 0);
            v
        } else {
            vec![0u8; capacity]
        }
    }

    pub fn release(&mut self, mut buffer: Vec<u8>) {
        if self.pool.len() < self.max_retained {
            buffer.clear();
            self.pool.push(buffer);
        }
    }
}

/// LRU Page Bitmap Cache bounded by maximum memory (default 128 MB).
pub struct TileCache {
    entries: HashMap<(usize, u32, i32), CachedBitmap>, // (page_idx, zoom_pct, rotation) -> Bitmap
    current_bytes: usize,
    max_bytes: usize,
    clock: u64,
    pool: BufferPool,
}

impl TileCache {
    pub fn new(max_megabytes: usize) -> Self {
        let max_bytes = max_megabytes.max(16) * 1024 * 1024;
        Self {
            entries: HashMap::new(),
            current_bytes: 0,
            max_bytes,
            clock: 0,
            pool: BufferPool::new(16),
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }

    pub fn get(&mut self, page_index: usize, zoom_pct: u32, rotation: i32) -> Option<&CachedBitmap> {
        let key = (page_index, zoom_pct, rotation);
        let tick = self.tick();
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_access = tick;
            Some(entry)
        } else {
            None
        }
    }

    pub fn acquire_buffer(&mut self, capacity: usize) -> Vec<u8> {
        self.pool.acquire(capacity)
    }

    pub fn insert(
        &mut self,
        page_index: usize,
        zoom_pct: u32,
        rotation: i32,
        width: i32,
        height: i32,
        stride: i32,
        pixels: Vec<u8>,
    ) {
        let key = (page_index, zoom_pct, rotation);
        let size = pixels.len();

        // If key already existed, remove its old size FIRST to prevent premature LRU eviction
        if let Some(old) = self.entries.remove(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(old.byte_size());
            self.pool.release(old.pixels);
        }

        // Evict if over budget
        while self.current_bytes + size > self.max_bytes && !self.entries.is_empty() {
            self.evict_lru();
        }

        let tick = self.tick();
        self.current_bytes += size;
        self.entries.insert(
            key,
            CachedBitmap {
                page_index,
                width,
                height,
                stride,
                zoom_pct,
                rotation,
                pixels,
                last_access: tick,
            },
        );
    }

    fn evict_lru(&mut self) {
        let oldest_key = self
            .entries
            .iter()
            .min_by_key(|(_, v)| v.last_access)
            .map(|(k, _)| *k);

        if let Some(k) = oldest_key {
            if let Some(removed) = self.entries.remove(&k) {
                self.current_bytes = self.current_bytes.saturating_sub(removed.byte_size());
                self.pool.release(removed.pixels);
            }
        }
    }

    pub fn invalidate_page(&mut self, page_index: usize) {
        let keys_to_remove: Vec<_> = self
            .entries
            .keys()
            .filter(|(idx, _, _)| *idx == page_index)
            .cloned()
            .collect();

        for k in keys_to_remove {
            if let Some(removed) = self.entries.remove(&k) {
                self.current_bytes = self.current_bytes.saturating_sub(removed.byte_size());
                self.pool.release(removed.pixels);
            }
        }
    }

    pub fn clear(&mut self) {
        for (_, entry) in self.entries.drain() {
            self.pool.release(entry.pixels);
        }
        self.current_bytes = 0;
    }
}

/// Lightweight LRU cache for left sidebar page thumbnails capped at 64 entries.
pub struct ThumbnailCache {
    thumbnails: HashMap<usize, (i32, i32, Vec<u8>)>,
    lru_order: Vec<usize>,
    max_entries: usize,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self {
            thumbnails: HashMap::new(),
            lru_order: Vec::new(),
            max_entries: 64,
        }
    }

    pub fn get(&mut self, page_index: usize) -> Option<&(i32, i32, Vec<u8>)> {
        if self.thumbnails.contains_key(&page_index) {
            if let Some(pos) = self.lru_order.iter().position(|&x| x == page_index) {
                let elem = self.lru_order.remove(pos);
                self.lru_order.push(elem);
            }
            self.thumbnails.get(&page_index)
        } else {
            None
        }
    }

    pub fn insert(&mut self, page_index: usize, width: i32, height: i32, pixels: Vec<u8>) {
        if self.thumbnails.contains_key(&page_index) {
            if let Some(pos) = self.lru_order.iter().position(|&x| x == page_index) {
                self.lru_order.remove(pos);
            }
        } else {
            while self.thumbnails.len() >= self.max_entries && !self.lru_order.is_empty() {
                let oldest = self.lru_order.remove(0);
                self.thumbnails.remove(&oldest);
            }
        }
        self.thumbnails.insert(page_index, (width, height, pixels));
        self.lru_order.push(page_index);
    }

    pub fn invalidate(&mut self, page_index: usize) {
        self.thumbnails.remove(&page_index);
        self.lru_order.retain(|&x| x != page_index);
    }

    pub fn clear(&mut self) {
        self.thumbnails.clear();
        self.lru_order.clear();
    }
}

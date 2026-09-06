//! Текстурный атлас тамбнейлов 2048² с LRU-вытеснением (SPEC §6.4, T6).
//!
//! Чистая логика размещения (`ThumbSlots`) не зависит от GPU и тестируется
//! без устройства; GPU-обвязка (текстура, write_texture) — в `thumbs.rs`.

use std::collections::HashMap;

/// Сторона атласа, px (SPEC §6.4).
pub const ATLAS_SIZE: u32 = 2048;
/// Сторона ячейки = класс размера тамбнейла (SPEC §5.2).
pub const CELL_SIZE: u32 = 256;
/// Ячеек в строке атласа.
pub const CELLS_PER_ROW: u32 = ATLAS_SIZE / CELL_SIZE;
/// Всего слотов в атласе (8×8 = 64).
pub const SLOT_COUNT: u32 = CELLS_PER_ROW * CELLS_PER_ROW;

/// Размещение тамбнейла ноды: ячейка, реальные размеры растра, тик LRU.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    /// Номер ячейки (0..SLOT_COUNT), row-major.
    pub cell: u32,
    /// Реальные размеры загруженного растра (≤ CELL_SIZE).
    pub width: u32,
    pub height: u32,
    /// Тик последнего обращения (LRU).
    tick: u64,
}

impl Slot {
    /// UV-координаты занятой части ячейки в атласе.
    pub fn uv(&self) -> ([f32; 2], [f32; 2]) {
        let (col, row) = (self.cell % CELLS_PER_ROW, self.cell / CELLS_PER_ROW);
        let origin = [
            (col * CELL_SIZE) as f32 / ATLAS_SIZE as f32,
            (row * CELL_SIZE) as f32 / ATLAS_SIZE as f32,
        ];
        let extent = [
            origin[0] + self.width as f32 / ATLAS_SIZE as f32,
            origin[1] + self.height as f32 / ATLAS_SIZE as f32,
        ];
        (origin, extent)
    }

    /// Пиксельное смещение ячейки в атласе (для write_texture).
    pub fn origin_px(&self) -> [u32; 2] {
        let (col, row) = (self.cell % CELLS_PER_ROW, self.cell / CELLS_PER_ROW);
        [col * CELL_SIZE, row * CELL_SIZE]
    }
}

/// Распределитель ячеек атласа с LRU-вытеснением. Не содержит GPU-объектов.
#[derive(Default)]
pub struct ThumbSlots {
    slots: HashMap<usize, Slot>,
    /// Свободные ячейки (пока атлас не заполнен — вытеснений нет).
    free: Vec<u32>,
    tick: u64,
}

impl ThumbSlots {
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            free: (0..SLOT_COUNT).rev().collect(),
            tick: 0,
        }
    }

    /// Занять ячейку под ноду; вернуть (слот, вытесненная нода).
    /// Повторный alloc той же ноды переиспользует её ячейку.
    pub fn alloc(&mut self, node: usize, width: u32, height: u32) -> (Slot, Option<usize>) {
        self.tick += 1;
        if let Some(slot) = self.slots.get_mut(&node) {
            slot.width = width;
            slot.height = height;
            slot.tick = self.tick;
            return (*slot, None);
        }
        let (cell, evicted) = match self.free.pop() {
            Some(cell) => (cell, None),
            None => {
                // Вытеснение самой давно неиспользуемой ноды; её CPU-копия
                // остаётся в SQLite-кэше, повторная загрузка дешевая (SPEC §6.4)
                let (&victim, _) = self
                    .slots
                    .iter()
                    .min_by_key(|(_, slot)| slot.tick)
                    .expect("атлас полон — есть что вытеснить");
                let cell = self.slots.remove(&victim).map(|slot| slot.cell);
                (cell.unwrap_or(0), Some(victim))
            }
        };
        let slot = Slot {
            cell,
            width,
            height,
            tick: self.tick,
        };
        self.slots.insert(node, slot);
        (slot, evicted)
    }

    /// Слот ноды с обновлением LRU-тика (обращение = использование в кадре).
    pub fn touch(&mut self, node: usize) -> Option<Slot> {
        self.tick += 1;
        let slot = self.slots.get_mut(&node)?;
        slot.tick = self.tick;
        Some(*slot)
    }

    pub fn contains(&self, node: usize) -> bool {
        self.slots.contains_key(&node)
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Атлас вмещает ровно SLOT_COUNT нод без вытеснений.
    #[test]
    fn fills_without_eviction() {
        let mut slots = ThumbSlots::new();
        for node in 0..SLOT_COUNT as usize {
            let (slot, evicted) = slots.alloc(node, 256, 256);
            assert_eq!(evicted, None);
            assert_eq!(slot.cell, node as u32);
        }
        assert_eq!(slots.len(), SLOT_COUNT as usize);
    }

    /// Вытесняется именно LRU-нода, её ячейка переиспользуется.
    #[test]
    fn evicts_least_recently_used() {
        let mut slots = ThumbSlots::new();
        for node in 0..SLOT_COUNT as usize {
            slots.alloc(node, 256, 256);
        }
        // Нода 0 — самая старая; "трогаем" её, теперь LRU — нода 1
        assert!(slots.touch(0).is_some());
        let (slot, evicted) = slots.alloc(1000, 128, 128);
        assert_eq!(evicted, Some(1));
        assert_eq!(slot.cell, 1, "ячейка вытесненной ноды переиспользуется");
        assert!(slots.contains(0));
        assert!(!slots.contains(1));
        assert!(slots.contains(1000));
    }

    /// Повторный alloc той же ноды не занимает новую ячейку и обновляет размеры.
    #[test]
    fn realloc_reuses_cell() {
        let mut slots = ThumbSlots::new();
        let (first, _) = slots.alloc(7, 256, 256);
        let (second, evicted) = slots.alloc(7, 100, 50);
        assert_eq!(evicted, None);
        assert_eq!(first.cell, second.cell);
        assert_eq!((second.width, second.height), (100, 50));
        assert_eq!(slots.len(), 1);
    }

    /// UV-координаты: начало ячейки + реальный размер растра.
    #[test]
    fn uv_matches_cell() {
        let mut slots = ThumbSlots::new();
        let (slot, _) = slots.alloc(0, 128, 256);
        let (min, max) = slot.uv();
        assert_eq!(min, [0.0, 0.0]);
        assert!((max[0] - 128.0 / 2048.0).abs() < 1e-6);
        assert!((max[1] - 256.0 / 2048.0).abs() < 1e-6);

        let (slot, _) = slots.alloc(1, 256, 256);
        let (min, max) = slot.uv();
        assert!((min[0] - 256.0 / 2048.0).abs() < 1e-6);
        assert_eq!(min[1], 0.0);
        assert!((max[0] - 512.0 / 2048.0).abs() < 1e-6);
        assert_eq!(slot.origin_px(), [256, 0]);
    }

    /// Ячейка 63 — последняя строка атласа.
    #[test]
    fn last_cell_position() {
        let slot = Slot {
            cell: SLOT_COUNT - 1,
            width: 256,
            height: 256,
            tick: 0,
        };
        assert_eq!(
            slot.origin_px(),
            [ATLAS_SIZE - CELL_SIZE, ATLAS_SIZE - CELL_SIZE]
        );
    }
}

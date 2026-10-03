// # Cache-Conscious Spatial Index (SoA vs. AoS Vector Arena)
//
// ## Prompt:
// Design a structure for storing and updating 100,000 active tracked dynamic radar entities (position 
// `x, y, z`, velocity `vx, vy, vz`, status flags).  
//
// ## Key Focus:
// * Contrast **Array of Structures (AoS)** with **Structure of Arrays (SoA)** for cache locality.
// * Implementation of a generation-indexed arena / slot map to avoid pointer-chasing and allocation overhead.
// * SIMD alignment considerations for batch arithmetic

// ## Solution
//
//   * AoS (`AosStore`, `Vec<Entity>`): one 32-byte `Entity` per tracked object, so two fit in a
//     64-byte cache line. Good when an operation touches most fields of one entity at a time.
//   * SoA (`SoaArena`): one `Vec` per field. A pass over one field (`count_above` reads only
//     `z`) streams exactly the bytes it needs, while AoS drags in all 32 bytes per entity. Each
//     column is also a flat `f32` run that the compiler can auto-vectorize.
//   * `SoaArena` is a slot map: a `Handle` is `{index, generation}`. `remove` bumps the slot's
//     generation and pushes the index on a free list; `insert` reuses freed slots first, so
//     there is no allocation churn and no pointer chasing. A stale handle fails the generation
//     check instead of silently aliasing whoever reused the slot (the ABA problem).
//   * Free slots are zeroed, so the batch loops (`integrate`, `count_above`) run over every
//     slot with no `if alive` branch, which keeps them vectorizable.
//   * SIMD alignment: `Entity` is `align(32)`. The SoA columns are only 4-byte aligned `Vec<f32>`;
//     the compiler uses unaligned vector loads, which are cheap unless a load straddles a cache
//     line. Forcing alignment would need aligned chunk types or `std::simd`.
//   * `main` benchmarks both layouts. SoA wins on the single-field scan and on integrate.

use std::hint::black_box;
use std::time::Instant;

#[derive(Clone, Copy, Default, Debug, PartialEq)]
#[repr(C, align(32))]
struct Entity {
    x: f32,
    y: f32,
    z: f32,
    vx: f32,
    vy: f32,
    vz: f32,
    flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Handle {
    index: u32,
    generation: u32,
}

struct AosStore {
    items: Vec<Entity>,
}

impl AosStore {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    fn push(&mut self, entity: Entity) {
        self.items.push(entity);
    }

    fn integrate(&mut self, dt: f32) {
        for e in &mut self.items {
            e.x += e.vx * dt;
            e.y += e.vy * dt;
            e.z += e.vz * dt;
        }
    }

    fn count_above(&self, z_min: f32) -> usize {
        self.items.iter().filter(|e| e.z > z_min).count()
    }
}

#[derive(Default)]
struct SoaArena {
    x: Vec<f32>,
    y: Vec<f32>,
    z: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
    vz: Vec<f32>,
    flags: Vec<u32>,
    generation: Vec<u32>,
    free: Vec<u32>,
    len: usize,
}

fn advance(position: &mut [f32], velocity: &[f32], dt: f32) {
    for (p, v) in position.iter_mut().zip(velocity) {
        *p += v * dt;
    }
}

impl SoaArena {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            x: Vec::with_capacity(capacity),
            y: Vec::with_capacity(capacity),
            z: Vec::with_capacity(capacity),
            vx: Vec::with_capacity(capacity),
            vy: Vec::with_capacity(capacity),
            vz: Vec::with_capacity(capacity),
            flags: Vec::with_capacity(capacity),
            generation: Vec::with_capacity(capacity),
            free: Vec::new(),
            len: 0,
        }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn insert(&mut self, entity: Entity) -> Handle {
        let index = self.free.pop().unwrap_or_else(|| self.grow());
        self.store(index as usize, entity);
        self.len += 1;
        Handle {
            index,
            generation: self.generation[index as usize],
        }
    }

    fn get(&self, handle: Handle) -> Option<Entity> {
        let i = handle.index as usize;
        if self.generation.get(i) != Some(&handle.generation) {
            return None;
        }
        Some(Entity {
            x: self.x[i],
            y: self.y[i],
            z: self.z[i],
            vx: self.vx[i],
            vy: self.vy[i],
            vz: self.vz[i],
            flags: self.flags[i],
        })
    }

    fn remove(&mut self, handle: Handle) -> Option<Entity> {
        let removed = self.get(handle)?;
        let i = handle.index as usize;
        self.store(i, Entity::default());
        self.generation[i] = self.generation[i].wrapping_add(1);
        self.free.push(handle.index);
        self.len -= 1;
        Some(removed)
    }

    // Free slots are zeroed (position, velocity, flags) so the batch loops below can run
    // branch-free over every slot, which is what lets them auto-vectorize.
    fn integrate(&mut self, dt: f32) {
        advance(&mut self.x, &self.vx, dt);
        advance(&mut self.y, &self.vy, dt);
        advance(&mut self.z, &self.vz, dt);
    }

    fn count_above(&self, z_min: f32) -> usize {
        self.z.iter().filter(|&&z| z > z_min).count()
    }

    fn grow(&mut self) -> u32 {
        self.x.push(0.0);
        self.y.push(0.0);
        self.z.push(0.0);
        self.vx.push(0.0);
        self.vy.push(0.0);
        self.vz.push(0.0);
        self.flags.push(0);
        self.generation.push(0);
        (self.generation.len() - 1) as u32
    }

    fn store(&mut self, i: usize, e: Entity) {
        self.x[i] = e.x;
        self.y[i] = e.y;
        self.z[i] = e.z;
        self.vx[i] = e.vx;
        self.vy[i] = e.vy;
        self.vz[i] = e.vz;
        self.flags[i] = e.flags;
    }
}

fn entity(i: usize) -> Entity {
    let f = i as f32;
    Entity {
        x: f,
        y: f * 0.5,
        z: 1_000.0 + f * 0.25,
        vx: 1.0,
        vy: -0.5,
        vz: 0.1,
        flags: 1,
    }
}

fn time<T>(label: &str, mut work: impl FnMut() -> T) {
    let start = Instant::now();
    black_box(work());
    println!("{label:<24}{:?}", start.elapsed());
}

fn main() {
    const ENTITIES: usize = 100_000;
    const FRAMES: usize = 500;
    const DT: f32 = 0.016;

    let mut aos = AosStore::with_capacity(ENTITIES);
    let mut soa = SoaArena::with_capacity(ENTITIES);
    let handles: Vec<Handle> = (0..ENTITIES)
        .map(|i| {
            aos.push(entity(i));
            soa.insert(entity(i))
        })
        .collect();

    time("AoS integrate x500", || {
        for _ in 0..FRAMES {
            aos.integrate(black_box(DT));
        }
    });
    time("SoA integrate x500", || {
        for _ in 0..FRAMES {
            soa.integrate(black_box(DT));
        }
    });
    time("AoS count_above x500", || {
        (0..FRAMES).map(|_| aos.count_above(black_box(1_000.0))).sum::<usize>()
    });
    time("SoA count_above x500", || {
        (0..FRAMES).map(|_| soa.count_above(black_box(1_000.0))).sum::<usize>()
    });

    for &h in &handles[..1_000] {
        soa.remove(h);
    }
    println!(
        "tracked: {}, first handle live: {}",
        soa.len(),
        soa.get(handles[0]).is_some()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_is_half_a_cache_line() {
        assert_eq!(std::mem::size_of::<Entity>(), 32);
        assert_eq!(std::mem::align_of::<Entity>(), 32);
    }

    #[test]
    fn insert_get_remove_round_trip() {
        let mut arena = SoaArena::default();
        let h = arena.insert(entity(3));
        assert_eq!(arena.get(h), Some(entity(3)));
        assert_eq!(arena.remove(h), Some(entity(3)));
        assert_eq!(arena.len(), 0);
        assert_eq!(arena.get(h), None);
        assert_eq!(arena.remove(h), None, "double remove is rejected");
    }

    #[test]
    fn stale_handle_does_not_alias_reused_slot() {
        let mut arena = SoaArena::default();
        let old = arena.insert(entity(1));
        arena.remove(old);
        let new = arena.insert(entity(2));
        assert_eq!(new.index, old.index, "freed slot is reused");
        assert_ne!(new.generation, old.generation);
        assert_eq!(arena.get(old), None);
        assert_eq!(arena.get(new), Some(entity(2)));
        assert_eq!(arena.remove(old), None);
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn integrate_matches_aos_and_ignores_free_slots() {
        let mut aos = AosStore::with_capacity(8);
        let mut soa = SoaArena::default();
        let handles: Vec<Handle> = (0..8)
            .map(|i| {
                aos.push(entity(i));
                soa.insert(entity(i))
            })
            .collect();

        soa.remove(handles[3]);
        aos.items.remove(3);
        aos.integrate(0.5);
        soa.integrate(0.5);

        let live = handles.iter().filter_map(|&h| soa.get(h));
        for (a, s) in aos.items.iter().zip(live) {
            assert_eq!(*a, s);
        }
        assert_eq!(soa.x[3], 0.0, "free slot stays at rest");
        assert_eq!(soa.count_above(1_001.0), aos.count_above(1_001.0));
    }
}

use super::*;

#[test]
fn chebyshev_basic() {
    assert_eq!(Cell::new(0, 0).chebyshev(Cell::new(3, 4)), 4);
    assert_eq!(Cell::new(0, 0).chebyshev(Cell::new(5, 5)), 5);
    assert_eq!(Cell::new(0, 0).chebyshev(Cell::new(0, 0)), 0);
    assert_eq!(Cell::new(-2, 3).chebyshev(Cell::new(1, -1)), 4);
}

#[test]
fn entity_walks_to_target() {
    let mut g = Grid::new(16, 8);
    g.spawn(Entity::idle(1, Cell::new(0, 0)).with_walk(Cell::new(5, 3), 1000));
    for _ in 0..5 {
        assert!(g.step_toward_target(0));
    }
    assert_eq!(g.entities[0].pos, Cell::new(5, 3));
    assert!(!g.step_toward_target(0));
}

#[test]
fn entity_respects_occupancy() {
    let mut g = Grid::new(16, 8);
    g.spawn(Entity::idle(1, Cell::new(0, 0)).with_walk(Cell::new(5, 0), 1000));
    // Block path with stationary entity.
    g.spawn(Entity::idle(2, Cell::new(1, 0)));
    assert!(g.step_toward_target(0));
    // Sidestep down: (1, -1) out of bounds → (1, 1).
    assert_eq!(g.entities[0].pos, Cell::new(1, 1));
}

#[test]
fn entity_walks_over_dead_body() {
    let mut g = Grid::new(16, 8);
    g.spawn(Entity::idle(1, Cell::new(0, 0)).with_walk(Cell::new(5, 0), 1000));
    let mut corpse = Entity::idle(2, Cell::new(1, 0));
    corpse.alive = false;
    g.spawn(corpse);
    // Dead entity does NOT block — should step straight to (1, 0).
    assert!(g.step_toward_target(0));
    assert_eq!(g.entities[0].pos, Cell::new(1, 0));
}

#[test]
fn bfs_prefers_straight_step_when_equal_length() {
    // From (5,5) to (5,8) all three south neighbors (4,6), (5,6),
    // (6,6) sit on a shortest path. BFS_NEIGHBOR_ORDER pushes
    // straight (5,6) ahead of the diagonals, so the entity picks
    // the orthogonal step — visually cleaner than zig-zagging.
    let mut g = Grid::new(16, 16);
    g.spawn(Entity::idle(1, Cell::new(5, 5)).with_walk(Cell::new(5, 8), 1000));
    g.step_toward_target(0);
    assert_eq!(g.entities[0].pos, Cell::new(5, 6));
}

#[test]
fn idx_of_id_works() {
    let mut g = Grid::new(16, 8);
    g.spawn(Entity::idle(7, Cell::new(0, 0)));
    g.spawn(Entity::idle(13, Cell::new(1, 0)));
    assert_eq!(g.idx_of_id(7), Some(0));
    assert_eq!(g.idx_of_id(13), Some(1));
    assert_eq!(g.idx_of_id(99), None);
}

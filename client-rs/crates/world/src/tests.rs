use super::*;
use bsk_proto::{GroundTile, ObjectStatus, WorldPos};

const FLOOR: u16 = 1;
const WATER: u16 = 2;
const WALL: u16 = 0x10;
const PLAYER: u16 = 0x20;

fn data() -> GameData {
    let mut d = GameData::default();
    d.add_grounds(
        r#"<Grounds>
            <Ground type="0x01" id="Floor"><Texture><File>t</File><Index>0</Index></Texture></Ground>
            <Ground type="0x02" id="Water"><NoWalk/></Ground>
            <Ground type="0xff" id="Empty"><NoWalk/></Ground>
        </Grounds>"#,
    )
    .unwrap();
    d.add_objects(
        r#"<Objects>
            <Object type="0x10" id="Wall"><Class>Wall</Class><Static/><OccupySquare/><FullOccupy/></Object>
            <Object type="0x20" id="Hero"><Class>Player</Class><Player/></Object>
        </Objects>"#,
    )
    .unwrap();
    d
}

fn obj(id: i32, object_type: u16, x: f32, y: f32, speed: i32) -> bsk_proto::ObjectData {
    let stats = vec![StatData { id: 22, value: StatValue::Int(speed) }];
    ObjectData { object_type: object_type as i32, status: ObjectStatus { object_id: id, pos: WorldPos { x, y }, stats: stats.into() } }
}

/// A 10Ã—10 floor with a wall at (6, 5) and water at (5, 7); our player at (5.5, 5.5).
fn world(d: &GameData) -> World {
    let mut w = World::new(10, 10, "test", 1);
    let mut tiles = Vec::new();
    for y in 0..10 {
        for x in 0..10 {
            tiles.push(GroundTile { x, y, tile_type: if (x, y) == (5, 7) { WATER } else { FLOOR } as i32 });
        }
    }
    let update = Update {
        tiles: tiles.into(),
        new_objs: vec![obj(1, WALL, 6.5, 5.5, 0), obj(2, PLAYER, 5.5, 5.5, 75)].into(),
        drops: Default::default(),
    };
    w.apply_update(&update, d, 0);
    w.me = Some(2);
    w
}

fn walk(w: &mut World, d: &GameData, x: f64, y: f64, ms: i32) -> (f64, f64) {
    let input = MoveInput { x, y, camera_angle: 0.0 };
    for t in 0..ms / 10 {
        w.update(t * 10, 10, &input, d);
    }
    let p = w.player().unwrap();
    (p.x, p.y)
}

#[test]
fn speed_formula() {
    // 75 speed: the maximum, 0.0096 tiles/ms; 0 speed: 0.004.
    assert!((move_speed(75, [0, 0], 1.0) - 0.0096).abs() < 1e-12);
    assert!((move_speed(0, [0, 0], 1.0) - 0.004).abs() < 1e-12);
    // Slowed ignores the stat.
    assert!((move_speed(75, [1 << 3, 0], 0.5) - 0.002).abs() < 1e-12);
    // Speedy: Ã—1.35.
    assert!((move_speed(0, [1 << 14, 0], 1.0) - 0.0054).abs() < 1e-12);
}

#[test]
fn walking_east_stops_at_the_wall() {
    let d = data();
    let mut w = world(&d);
    let (x, y) = walk(&mut w, &d, 1.0, 0.0, 1000);
    // The wall in square 6 is FullOccupy, so isValidPosition refuses any x past the middle of
    // our square: we can't get closer than half a tile to it.
    assert_eq!((x, y), (5.5, 5.5));
    // From further away we walk up to that same point.
    let mut w = world(&d);
    w.apply_goto(2, 3.2, 5.5, 0, &d);
    let (x, _) = walk(&mut w, &d, 1.0, 0.0, 1000);
    assert_eq!(x, 5.5);
}

#[test]
fn walking_west_is_free() {
    let d = data();
    let mut w = world(&d);
    let (x, _) = walk(&mut w, &d, -1.0, 0.0, 200);
    // 200 ms at 0.0096 tiles/ms.
    assert!((x - (5.5 - 1.92)).abs() < 1e-9, "x = {x}");
}

#[test]
fn water_blocks_walking_south() {
    let d = data();
    let mut w = world(&d);
    let (_, y) = walk(&mut w, &d, 0.0, 1.0, 1000);
    assert!((y - 6.99).abs() < 1e-9, "y = {y}");
}

#[test]
fn others_glide_to_their_tick_position() {
    let d = data();
    let mut w = world(&d);
    let mut other = obj(3, PLAYER, 2.0, 2.0, 0);
    other.status.stats = Default::default();
    w.apply_update(&Update { new_objs: vec![other].into(), ..Default::default() }, &d, 0);
    let status = ObjectStatus { object_id: 3, pos: WorldPos { x: 3.0, y: 2.0 }, stats: Default::default() };
    w.apply_new_tick(&NewTick { tick_id: 1, tick_time: 200, statuses: vec![status].into() }, 1000);
    w.update(1100, 100, &MoveInput::default(), &d);
    let o = &w.objects[&3];
    assert!((o.x - 2.5).abs() < 1e-9 && o.moving, "halfway after half the tick: {}", o.x);
    w.update(1200, 100, &MoveInput::default(), &d);
    assert!((w.objects[&3].x - 3.0).abs() < 1e-9);
}

#[test]
fn our_own_status_moves_nothing() {
    let d = data();
    let mut w = world(&d);
    let status = ObjectStatus {
        object_id: 2,
        pos: WorldPos { x: 1.0, y: 1.0 },
        stats: vec![StatData { id: 22, value: StatValue::Int(10) }].into(),
    };
    w.apply_new_tick(&NewTick { tick_id: 1, tick_time: 200, statuses: vec![status].into() }, 100);
    let p = w.player().unwrap();
    assert_eq!((p.x, p.y), (5.5, 5.5));
    assert_eq!(p.stat_int(22), Some(10));
}

#[test]
fn dropped_objects_free_their_square() {
    let d = data();
    let mut w = world(&d);
    assert!(w.occupant(6, 5).is_some());
    w.apply_update(&Update { drops: vec![1].into(), ..Default::default() }, &d, 0);
    assert!(w.occupant(6, 5).is_none());
    let (x, _) = walk(&mut w, &d, 1.0, 0.0, 100);
    assert!(x > 6.0, "the wall is gone, so we cross into square 6: {x}");
}

use boltffi::*;

/// Lifecycle status of an entity.
#[data]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Active,
    Inactive,
    Pending,
}

#[export]
pub fn echo_status(s: Status) -> Status {
    s
}

#[export]
pub fn status_to_string(s: Status) -> String {
    match s {
        Status::Active => "active".to_string(),
        Status::Inactive => "inactive".to_string(),
        Status::Pending => "pending".to_string(),
    }
}

#[export]
pub fn is_active(s: Status) -> bool {
    matches!(s, Status::Active)
}

#[export]
pub fn echo_vec_status(values: Vec<Status>) -> Vec<Status> {
    values
}

#[data]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    North,
    South,
    East,
    West,
}

#[export]
pub fn echo_direction(d: Direction) -> Direction {
    d
}

#[export]
pub fn opposite_direction(d: Direction) -> Direction {
    match d {
        Direction::North => Direction::South,
        Direction::South => Direction::North,
        Direction::East => Direction::West,
        Direction::West => Direction::East,
    }
}

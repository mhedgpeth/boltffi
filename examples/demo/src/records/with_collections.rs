use boltffi::*;

use crate::records::blittable::Point;
use crate::records::with_strings::Person;

#[data]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Polygon {
    pub points: Vec<Point>,
}

#[export]
pub fn echo_polygon(p: Polygon) -> Polygon {
    p
}

#[export]
pub fn make_polygon(points: Vec<Point>) -> Polygon {
    Polygon { points }
}

#[export]
pub fn polygon_vertex_count(p: Polygon) -> u32 {
    p.points.len() as u32
}

#[export]
pub fn polygon_centroid(p: Polygon) -> Point {
    if p.points.is_empty() {
        return Point { x: 0.0, y: 0.0 };
    }
    let count = p.points.len() as f64;
    let sum_x: f64 = p.points.iter().map(|pt| pt.x).sum();
    let sum_y: f64 = p.points.iter().map(|pt| pt.y).sum();
    Point {
        x: sum_x / count,
        y: sum_y / count,
    }
}

#[data]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Team {
    pub name: String,
    pub members: Vec<String>,
}

#[export]
pub fn echo_team(t: Team) -> Team {
    t
}

#[export]
pub fn make_team(name: String, members: Vec<String>) -> Team {
    Team { name, members }
}

#[export]
pub fn team_size(t: Team) -> u32 {
    t.members.len() as u32
}

#[data]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Classroom {
    pub students: Vec<Person>,
}

#[export]
pub fn echo_classroom(c: Classroom) -> Classroom {
    c
}

#[export]
pub fn make_classroom(students: Vec<Person>) -> Classroom {
    Classroom { students }
}

#[data]
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TaggedScores {
    pub label: String,
    pub scores: Vec<f64>,
}

#[export]
pub fn echo_tagged_scores(ts: TaggedScores) -> TaggedScores {
    ts
}

#[export]
pub fn average_score(ts: TaggedScores) -> f64 {
    if ts.scores.is_empty() {
        return 0.0;
    }
    let sum: f64 = ts.scores.iter().sum();
    sum / ts.scores.len() as f64
}

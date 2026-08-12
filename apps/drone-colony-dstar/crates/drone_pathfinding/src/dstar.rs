use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use rand::Rng;
use tracing::{info, debug};

/// Represents a point in the grid.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Node {
    pub x: i32,
    pub y: i32,
}

/// Represents a pathfinding state for the priority queue.
#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: i32,
    node: Node,
}

/// Custom ordering for the priority queue (min-heap).
impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost) // Min-heap based on cost
            .then_with(|| self.node.x.cmp(&other.node.x))
            .then_with(|| self.node.y.cmp(&other.node.y))
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A* pathfinding with Ant Colony heuristic influence.
pub fn double_astar_find_path(
    start: Node,
    goal: Node,
    grid: &Vec<Vec<u8>>,
    pheromones: &mut HashMap<Node, f64>,
) -> Vec<Node> {
    let mut open_set = BinaryHeap::new();
    let mut came_from: HashMap<Node, Node> = HashMap::new();
    let mut g_score: HashMap<Node, i32> = HashMap::new();
    let mut f_score: HashMap<Node, i32> = HashMap::new();
    
    g_score.insert(start, 0);
    f_score.insert(start, heuristic(start, goal));

    open_set.push(State { cost: 0, node: start });

    while let Some(State { node, cost }) = open_set.pop() {
        if node == goal {
            return reconstruct_path(came_from, node);
        }

        for neighbor in get_neighbors(node, grid) {
            let tentative_g_score = g_score.get(&node).unwrap_or(&i32::MAX) + 1;
            let pheromone_bonus = pheromones.get(&neighbor).unwrap_or(&0.0) * 10.0; // Ant Colony boost
            let new_cost = tentative_g_score as f64 - pheromone_bonus; 

            if new_cost < *g_score.get(&neighbor).unwrap_or(&i32::MAX) as f64 {
                came_from.insert(neighbor, node);
                g_score.insert(neighbor, new_cost as i32);
                f_score.insert(neighbor, new_cost as i32 + heuristic(neighbor, goal));
                open_set.push(State { cost: *f_score.get(&neighbor).unwrap(), node: neighbor });
            }
        }
    }
    vec![] // Return empty if no path found
}

/// Euclidean heuristic for A*.
fn heuristic(node: Node, goal: Node) -> i32 {
    let dx = (node.x - goal.x).abs();
    let dy = (node.y - goal.y).abs();
    dx + dy
}

/// Returns neighboring nodes in a 4-way grid system.
fn get_neighbors(node: Node, grid: &Vec<Vec<u8>>) -> Vec<Node> {
    let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];
    let mut neighbors = vec![];

    for (dx, dy) in directions.iter() {
        let new_x = node.x + dx;
        let new_y = node.y + dy;

        if new_x >= 0 && new_x < grid.len() as i32 && new_y >= 0 && new_y < grid[0].len() as i32 {
            if grid[new_x as usize][new_y as usize] == 0 {
                neighbors.push(Node { x: new_x, y: new_y });
            }
        }
    }
    neighbors
}

/// Reconstructs the path after A* completion.
fn reconstruct_path(came_from: HashMap<Node, Node>, mut current: Node) -> Vec<Node> {
    let mut path = vec![current];

    while let Some(prev) = came_from.get(&current) {
        path.push(*prev);
        current = *prev;
    }
    
    path.reverse();
    path
}

/// Updates pheromone levels based on successful paths (Ant Colony Optimization).
pub fn update_pheromones(pheromones: &mut HashMap<Node, f64>, path: &Vec<Node>, decay_factor: f64) {
    for node in path {
        let entry = pheromones.entry(*node).or_insert(0.0);
        *entry += 1.0;
    }

    for (_, pheromone) in pheromones.iter_mut() {
        *pheromone *= decay_factor;
    }
}

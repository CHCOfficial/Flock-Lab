use glam::Vec3;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SpatialHash {
    cell_size: f32,
    cells: HashMap<[i32; 3], Vec<usize>>,
}

impl SpatialHash {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(0.001),
            cells: HashMap::new(),
        }
    }

    pub fn set_cell_size(&mut self, cell_size: f32) {
        self.cell_size = cell_size.max(0.001);
    }

    pub fn clear(&mut self) {
        self.cells.clear();
    }

    pub fn rebuild(&mut self, positions: impl Iterator<Item = (usize, Vec3)>) {
        self.clear();
        for (index, position) in positions {
            self.insert(index, position);
        }
    }

    pub fn insert(&mut self, index: usize, position: Vec3) {
        self.cells
            .entry(self.cell_for(position))
            .or_default()
            .push(index);
    }

    pub fn cell_for(&self, position: Vec3) -> [i32; 3] {
        [
            (position.x / self.cell_size).floor() as i32,
            (position.y / self.cell_size).floor() as i32,
            (position.z / self.cell_size).floor() as i32,
        ]
    }

    pub fn nearby_indices(&self, position: Vec3) -> impl Iterator<Item = usize> + '_ {
        let origin = self.cell_for(position);
        (-1..=1).flat_map(move |x| {
            (-1..=1).flat_map(move |y| {
                (-1..=1).flat_map(move |z| {
                    let key = [origin[0] + x, origin[1] + y, origin[2] + z];
                    self.cells
                        .get(&key)
                        .into_iter()
                        .flat_map(|indices| indices.iter().copied())
                })
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_negative_and_positive_positions() {
        let hash = SpatialHash::new(4.0);
        assert_eq!(hash.cell_for(Vec3::new(0.0, 3.9, -0.1)), [0, 0, -1]);
        assert_eq!(hash.cell_for(Vec3::new(8.2, -4.1, 4.0)), [2, -2, 1]);
    }

    #[test]
    fn query_returns_adjacent_cell_indices() {
        let mut hash = SpatialHash::new(5.0);
        hash.insert(7, Vec3::new(1.0, 0.0, 0.0));
        hash.insert(9, Vec3::new(6.0, 0.0, 0.0));
        hash.insert(11, Vec3::new(25.0, 0.0, 0.0));

        let mut found = hash.nearby_indices(Vec3::ZERO).collect::<Vec<_>>();
        found.sort_unstable();
        assert_eq!(found, vec![7, 9]);
    }
}

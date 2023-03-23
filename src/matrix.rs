use crate::{Column, VecColumn};

/// Unused.
pub trait IndexableMatrix<C: Column> {
    fn col(&self, index: usize) -> &C;
    fn set_col(&mut self, index: usize, col: C);
    fn push_col(&mut self, col: C);
    fn width(&self) -> usize;
    fn height(&self) -> usize;
}

pub struct VecMatrix<C> {
    pub cols: Vec<C>,
    height: usize,
}

impl<C: Column> IndexableMatrix<C> for VecMatrix<C> {
    fn col(&self, index: usize) -> &C {
        &self.cols[index]
    }

    fn set_col(&mut self, index: usize, col: C) {
        self.cols[index] = col;
    }

    fn push_col(&mut self, col: C) {
        self.cols.push(col);
    }

    fn width(&self) -> usize {
        self.cols.len()
    }

    fn height(&self) -> usize {
        self.height
    }
}

impl<C: Column> From<Vec<C>> for VecMatrix<C> {
    fn from(cols: Vec<C>) -> Self {
        Self {
            height: cols.len(),
            cols,
        }
    }
}

impl<C: Column> From<(Vec<C>, Option<usize>)> for VecMatrix<C> {
    fn from((cols, height): (Vec<C>, Option<usize>)) -> Self {
        match height {
            Some(height) => Self { height, cols },
            None => Self {
                height: cols.len(),
                cols,
            },
        }
    }
}

pub fn anti_transpose(matrix: &Vec<VecColumn>, matrix_height: Option<usize>) -> Vec<VecColumn> {
    let matrix_width = matrix.len();
    let matrix_height = matrix_height.unwrap_or(matrix_width);
    let mut return_matrix: Vec<VecColumn> = vec![VecColumn::default(); matrix_height];
    for (j, col) in matrix.iter().enumerate() {
        for i in col.boundary.iter() {
            return_matrix[matrix_height - 1 - i].add_entry(matrix_width - 1 - j);
        }
    }
    return_matrix
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::column::VecColumn;

    fn build_sphere_triangulation() -> Vec<VecColumn> {
        vec![
            (0, vec![]),
            (0, vec![]),
            (0, vec![]),
            (0, vec![]),
            (1, vec![0, 1]),
            (1, vec![0, 2]),
            (1, vec![1, 2]),
            (1, vec![0, 3]),
            (1, vec![1, 3]),
            (1, vec![2, 3]),
            (2, vec![4, 7, 8]),
            (2, vec![5, 7, 9]),
            (2, vec![6, 8, 9]),
            (2, vec![4, 5, 6]),
        ]
        .into_iter()
        .map(|col| col.into())
        .collect()
    }

    fn build_sphere_triangulation_at() -> Vec<VecColumn> {
        vec![
            (0, vec![]),
            (0, vec![]),
            (0, vec![]),
            (0, vec![]),
            (1, vec![1, 2]),
            (1, vec![1, 3]),
            (1, vec![2, 3]),
            (1, vec![0, 1]),
            (1, vec![0, 2]),
            (1, vec![0, 3]),
            (2, vec![4, 5, 6]),
            (2, vec![4, 7, 8]),
            (2, vec![5, 7, 9]),
            (2, vec![6, 8, 9]),
        ]
        .into_iter()
        .map(|col| col.into())
        .collect()
    }

    #[test]
    fn sphere_triangulation_at() {
        let matrix = build_sphere_triangulation();
        let matrix_at = build_sphere_triangulation_at();
        let at: Vec<VecColumn> = anti_transpose(&matrix, None);
        assert_eq!(at, matrix_at);
    }
    use proptest::collection::hash_set;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn at_at_is_identity( matrix in sut_matrix(100) ) {
            let at: Vec<VecColumn> = anti_transpose(&matrix, None);
            let at_at: Vec<VecColumn> = anti_transpose(&at, None);
            assert_eq!(matrix, at_at);
        }
    }

    // Generates a strict upper triangular matrix of VecColumns with given size
    fn sut_matrix(size: usize) -> impl Strategy<Value = Vec<VecColumn>> {
        let mut matrix = vec![];
        for i in 1..size {
            matrix.push(veccolum_with_idxs_below(i));
        }
        matrix
    }

    fn veccolum_with_idxs_below(mut max_idx: usize) -> impl Strategy<Value = VecColumn> {
        // Avoid empty range problem
        // Always returns empty Vec because size is in 0..1 == { 0 }
        if max_idx == 0 {
            max_idx = 1;
        }
        hash_set(0..max_idx, 0..max_idx).prop_map(|set| {
            let mut col: Vec<_> = set.into_iter().collect();
            col.sort();
            VecColumn::from((0, col))
        })
    }
}

use crate::Column;
use crate::DiagramReadOff;
use crate::LoPhatOptions;
use crate::PersistenceDiagram;
use crate::RVDecomposition;

use crossbeam::atomic::AtomicCell;
use hashbrown::HashSet;
use pinboard::NonEmptyPinboard;
use rayon::prelude::*;
#[cfg(feature = "local_thread_pool")]
use rayon::ThreadPoolBuilder;

enum LoPhatThreadPool {
    #[cfg(not(feature = "local_thread_pool"))]
    Global(),
    #[cfg(feature = "local_thread_pool")]
    Local(rayon::ThreadPool),
}

impl LoPhatThreadPool {
    fn install<OP, R>(&self, op: OP) -> R
    where
        OP: FnOnce() -> R + Send,
        R: Send,
    {
        match self {
            #[cfg(not(feature = "local_thread_pool"))]
            LoPhatThreadPool::Global() => op(),
            #[cfg(feature = "local_thread_pool")]
            LoPhatThreadPool::Local(pool) => pool.install(op),
        }
    }
}

/// Stores the matrix and pivot vector behind appropriate atomic data types, as well as the algorithm options.
/// Provides methods for reducing the matrix in parallel.
pub struct LockFreeAlgorithm<'a, C: Column + 'static> {
    matrix: Vec<NonEmptyPinboard<(C, Option<C>)>>,
    pivots: Vec<AtomicCell<Option<usize>>>,
    options: &'a LoPhatOptions,
    thread_pool: LoPhatThreadPool,
    max_dim: usize,
}

impl<'a, C: Column + 'static> LockFreeAlgorithm<'a, C> {
    /// Initialise atomic data structure with provided `matrix`, store algorithm options and init thread pool.
    pub fn new(matrix: impl Iterator<Item = C>, options: &'a LoPhatOptions) -> Self {
        let mut max_dim = 0;
        let matrix: Vec<_> = matrix
            .enumerate()
            .map(|(idx, r_col)| {
                max_dim = max_dim.max(r_col.dimension());
                if options.maintain_v {
                    let mut v_col = C::new_with_dimension(r_col.dimension());
                    v_col.add_entry(idx);
                    NonEmptyPinboard::new((r_col, Some(v_col)))
                } else {
                    NonEmptyPinboard::new((r_col, None))
                }
            })
            .collect();
        let column_height = options.column_height.unwrap_or(matrix.len());
        let pivots: Vec<_> = (0..column_height).map(|_| AtomicCell::new(None)).collect();
        // Setup thread pool
        #[cfg(feature = "local_thread_pool")]
        let thread_pool = LoPhatThreadPool::Local(
            ThreadPoolBuilder::new()
                .num_threads(options.num_threads)
                .build()
                .expect("Failed to build thread pool"),
        );
        #[cfg(not(feature = "local_thread_pool"))]
        let thread_pool = {
            if options.num_threads != 0 {
                panic!(
                    "To specify a number of threads, please enable the local_thread_pool feature"
                );
            }
            LoPhatThreadPool::Global()
        };
        // Return options
        Self {
            matrix,
            pivots,
            options,
            thread_pool,
            max_dim,
        }
    }

    /// Return a column with index `l`, if one exists.
    /// If found, returns `(col_idx, col)`, where col is a tuple consisting of the corresponding column in R and V.
    /// If not maintaining V, second entry of tuple is `None`.
    pub fn get_col_with_pivot(&self, l: usize) -> Option<(usize, (C, Option<C>))> {
        loop {
            let piv = self.pivots[l].load();
            if let Some(piv) = piv {
                let cols = self.matrix[piv].read();
                if cols.0.pivot() != Some(l) {
                    // Got a column but it now has the wrong pivot; loop again.
                    continue;
                };
                // Get column with correct pivot, return to caller.
                return Some((piv, cols));
            } else {
                // There is not yet a column with this pivot, inform caller.
                return None;
            }
        }
    }

    /// Reduces the `j`th column of the matrix as far as possible.
    /// If a pivot is found to the right of `j` (e.g. redued by another thread)
    /// then will switch to reducing that column.
    /// It is safe to reduce all columns in parallel.
    pub fn reduce_column(&self, j: usize) {
        let mut working_j = j;
        'outer: loop {
            //println!("{:?}", working_j);
            let mut curr_column = self.matrix[working_j].read();
            while let Some(l) = (&curr_column).0.pivot() {
                let piv_with_column_opt = self.get_col_with_pivot(l);
                if let Some((piv, piv_column)) = piv_with_column_opt {
                    // Lines 17-24
                    if piv < working_j {
                        curr_column.0.add_col(&piv_column.0);
                        // Only add V columns if we need to
                        if self.options.maintain_v {
                            let curr_v_col = curr_column.1.as_mut().unwrap();
                            curr_v_col.add_col(&piv_column.1.unwrap());
                        }
                    } else if piv > working_j {
                        self.matrix[working_j].set(curr_column);
                        if self.pivots[l]
                            .compare_exchange(Some(piv), Some(working_j))
                            .is_ok()
                        {
                            working_j = piv;
                        }
                        continue 'outer;
                    } else {
                        panic!()
                    }
                } else {
                    // piv = -1 case
                    self.matrix[working_j].set(curr_column);
                    if self.pivots[l]
                        .compare_exchange(None, Some(working_j))
                        .is_ok()
                    {
                        return;
                    } else {
                        continue 'outer;
                    }
                }
            }
            // Lines 25-27 (curr_column = 0 clause)
            if (&curr_column.0).is_cycle() {
                self.matrix[working_j].set(curr_column);
                return;
            }
        }
    }

    /// Uses the boundary built up in column `boudary_idx` to clear the column corresponding to its pivot
    pub fn clear_with_column(&self, boudary_idx: usize) {
        let (boundary_r, _boundary_v) = self.matrix[boudary_idx].read();
        let clearing_idx = boundary_r
            .pivot()
            .expect("Attempted to clear using cycle column");
        let clearing_dimension = self.matrix[clearing_idx].read().0.dimension();
        // The cleared R column is empty
        let r_col = C::new_with_dimension(clearing_dimension);
        // The corresponding R column should be the R column of the boundary
        let v_col = self
            .options
            .maintain_v
            .then_some(boundary_r.with_dimension(clearing_dimension));
        self.matrix[clearing_idx].set((r_col, v_col));
    }

    /// Reduce all columns of given dimension in parallel, according to `options`.
    pub fn reduce_dimension(&self, dimension: usize) {
        // Reduce matrix for columns of that dimension
        self.thread_pool.install(|| {
            (0..self.matrix.len())
                .into_par_iter()
                .with_min_len(self.options.min_chunk_len)
                .filter(|&j| self.matrix[j].read().0.dimension() == dimension)
                .for_each(|j| self.reduce_column(j));
        });
    }

    /// Clear all columns of given dimension in parallel
    pub fn clear_dimension(&self, dimension: usize) {
        // Reduce matrix for columns of that dimension
        self.thread_pool.install(|| {
            (0..self.matrix.len())
                .into_par_iter()
                .with_min_len(self.options.min_chunk_len)
                .filter(|&j| self.matrix[j].read().0.dimension() == dimension)
                .filter(|&j| self.matrix[j].read().0.is_boundary())
                .for_each(|j| self.clear_with_column(j));
        });
    }

    /// Reduce all columns in parallel, according to `options`.
    pub fn reduce(&self) {
        for dimension in (0..=self.max_dim).rev() {
            self.reduce_dimension(dimension);
            if self.options.clearing && dimension > 0 {
                self.clear_dimension(dimension)
            }
        }
    }
}

impl<'a, C: Column + 'static> DiagramReadOff for LockFreeAlgorithm<'a, C> {
    fn diagram(&self) -> crate::PersistenceDiagram {
        let paired: HashSet<(usize, usize)> = self
            .matrix
            .par_iter()
            .enumerate()
            .filter_map(|(idx, col)| {
                let lowest_idx = col.read().0.pivot()?;
                Some((lowest_idx, idx))
            })
            .collect();
        let mut unpaired: HashSet<usize> = (0..self.matrix.len()).collect();
        for (birth, death) in paired.iter() {
            unpaired.remove(birth);
            unpaired.remove(death);
        }
        PersistenceDiagram { unpaired, paired }
    }
}

impl<'a, C: Column + 'static> From<LockFreeAlgorithm<'a, C>> for RVDecomposition<C> {
    fn from(algo: LockFreeAlgorithm<C>) -> Self {
        let (r, v) = if algo.options.maintain_v {
            let (r_sub, v_sub) = algo
                .matrix
                .into_iter()
                .map(|pinboard| pinboard.read())
                .map(|(r_col, v_col)| (r_col, v_col.unwrap()))
                .unzip();
            (r_sub, Some(v_sub))
        } else {
            (
                algo.matrix
                    .into_iter()
                    .map(|pinboard| pinboard.read())
                    .map(|(r_col, _v_col)| r_col)
                    .collect(),
                None,
            )
        };
        RVDecomposition { r, v }
    }
}

/// Decomposes the input matrix, using the lockfree, parallel algoirhtm of Morozov and Nigmetov.
///
/// * `matrix` - iterator over columns of the matrix you wish to decompose.
/// * `options` - additional options to control decompositon, see [`LoPhatOptions`].
pub fn rv_decompose_lock_free<C: Column + 'static>(
    matrix: impl Iterator<Item = C>,
    options: &LoPhatOptions,
) -> LockFreeAlgorithm<C> {
    let algo = LockFreeAlgorithm::new(matrix, options);
    algo.reduce();
    algo
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::column::VecColumn;
    use crate::rv_decompose_serial;
    use crate::DiagramReadOff;
    use proptest::collection::hash_set;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn lockfree_agrees_with_serial( matrix in sut_matrix(100) ) {
            let options = LoPhatOptions::default();
            let serial_dgm = rv_decompose_serial(matrix.iter().cloned(), &options).diagram();
            let parallel_dgm = rv_decompose_lock_free(matrix.into_iter(), &options).diagram();
            assert_eq!(serial_dgm, parallel_dgm);
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

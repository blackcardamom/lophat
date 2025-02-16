Python Bindings
==================================

The current bindings are exposed in the Python lophat package.
For more information, please see `the repository <https://github.com/tomchaplin/lophat>`_.

.. py:currentmodule:: lophat

.. py:function:: compute_pairings(matrix, anti_transpose = True, options= None)

    Decomposes the input matrix, using the lockfree or standard algorithm (according to options).

    :param matrix: The boundary matrix, provided in sparse column format. Each column is a tuple of (dimension, boundary) where boundary is the list of non-zero indices.
    :type matrix: List[Tuple[int, List[int]]] | Iterator[Tuple[int, List[int]]]
    :param anti_transpose: Whether to anti-transpose the matrix first; best left True with clearing on. Set to False if input matrix non-square.
    :type anti_transpose: bool
    :param options: Options to control the R=DV decomposition algorithm.
    :type options: LoPhatOptions
    :returns: The persistence pairings read off from the R=DV decomposition.
    :rtype: PersistenceDiagram

.. py:class:: LoPhatOptions(maintain_v = False,num_threads= 0,column_height= None,max_chunk_len= 1, clearing = True)

    A class representing the persistence diagram computed by LoPHAT.
    Each column index in the input matrix appears exactly once, either in a pairing or as unpaired.

    :param maintain_v: Whether to maintain_v during decompositon, usually best left False.
    :type maintain_v: bool 
    :param num_threads: Max number of threads to use. Set at 0 to use all threads. Set at 1 to use standard, serial algorithm.
    :type num_threads: int
    :param column_height: Optional hint to height of columns. If None, assumed that matrix is square.
    :type column_height: int | None
    :param min_chunk_len: When splitting work, don't reduce chunks to smaller than this size.
    :type min_chunk_len: int
    :param clearing: Whether to employ the clearing optimisation. Cannot use if input non-square.
    :type clearing: bool

.. py:class:: PersistenceDiagram()

    A class representing the persistence diagram computed by LoPHAT.
    Each column index in the input matrix appears exactly once, either in a pairing or as unpaired.

    :param unpaired: The set of input column indices that were not paired in the R=DV decomposition.
    :type unpaired: Set[int]
    :param paired: The set of (birth, death) pairs of column indices that were paired in the R=DV decomposition.
    :type paired: Set[Tuple[int, int]]


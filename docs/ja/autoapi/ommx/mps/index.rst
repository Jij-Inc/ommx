ommx.mps
========

.. py:module:: ommx.mps


Functions
---------

.. autoapisummary::

   ommx.mps.load_file
   ommx.mps.write_file


Module Contents
---------------

.. py:function:: load_file(path: str) -> ommx.v1.Instance

.. py:function:: write_file(instance: ommx.v1.Instance, path: str)

   Outputs the instance as an MPS file.

   - The outputted file is compressed by gzip.
   - Only linear problems are supported.
   - Various forms of metadata, like problem description and variable/constraint names, are not preserved.



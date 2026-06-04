ommx.testing
============

.. py:module:: ommx.testing


Submodules
----------

.. toctree::
   :maxdepth: 1

   /autoapi/ommx/testing/placement/index


Classes
-------

.. autoapisummary::

   ommx.testing.DataType
   ommx.testing.SingleFeasibleLPGenerator


Package Contents
----------------

.. py:class:: DataType



   Generic enumeration.

   Derive from this class to define new enumerations.


   .. py:attribute:: FLOAT


   .. py:attribute:: INT


.. py:class:: SingleFeasibleLPGenerator(n: int, data_type: DataType)

   .. py:method:: get_v1_instance() -> ommx.v1.Instance

      Get an instance of a linear programming problem with a unique solution.

      Examples:
          >>> from ommx.testing import DataType, SingleFeasibleLPGenerator
          >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
          >>> ommx_instance = generator.get_v1_instance()



   .. py:method:: get_v1_state() -> ommx.v1.State

      Get the solution state of the generated instance.

      Examples:
          >>> from ommx.testing import DataType, SingleFeasibleLPGenerator
          >>> generator = SingleFeasibleLPGenerator(3, DataType.INT)
          >>> ommx_state = generator.get_v1_state()



   .. py:attribute:: FLOAT_LOWER_BOUND
      :value: -100.0



   .. py:attribute:: FLOAT_UPPER_BOUND
      :value: 100.0



   .. py:attribute:: INT_LOWER_BOUND
      :value: -100



   .. py:attribute:: INT_UPPER_BOUND
      :value: 100




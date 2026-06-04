ommx_python_mip_adapter.python_mip_to_ommx
==========================================

.. py:module:: ommx_python_mip_adapter.python_mip_to_ommx


Classes
-------

.. autoapisummary::

   ommx_python_mip_adapter.python_mip_to_ommx.OMMXInstanceBuilder


Functions
---------

.. autoapisummary::

   ommx_python_mip_adapter.python_mip_to_ommx.model_to_instance


Module Contents
---------------

.. py:class:: OMMXInstanceBuilder

   Build ommx.v1.Instance from Python-MIP Model.


   .. py:method:: as_ommx_function(lin_expr: mip.LinExpr) -> ommx.v1.Function


   .. py:method:: build() -> ommx.v1.Instance


   .. py:method:: constraints() -> dict[int, ommx.v1.Constraint]


   .. py:method:: decision_variables() -> list[ommx.v1.DecisionVariable]

      Gather decision variables from Python-MIP Model as ommx.v1.DecisionVariable.



   .. py:method:: objective() -> ommx.v1.Function


   .. py:method:: sense()


   .. py:attribute:: model
      :type:  mip.Model


.. py:function:: model_to_instance(model: mip.Model) -> ommx.v1.Instance

   The function to convert Python-MIP Model to ommx.v1.Instance.

   Examples
   =========

   .. doctest::
       >>> import mip
       >>> import ommx_python_mip_adapter as adapter

       >>> model = mip.Model()
       >>> x1=model.add_var(name="1", var_type=mip.INTEGER, lb=0, ub=5)
       >>> x2=model.add_var(name="2", var_type=mip.CONTINUOUS, lb=0, ub=5)

       >>> model.objective = - x1 - 2 * x2
       >>> constr = model.add_constr(x1 + x2 - 6 <= 0)

       >>> ommx_instance = adapter.model_to_instance(model)



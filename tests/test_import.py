def test_import():
    import ommx_python_mip_adapter as adapter
    from ommx_python_mip_adapter import instance_to_model
    from ommx_python_mip_adapter import model_to_solution

    assert adapter.instance_to_model
    assert adapter.model_to_solution
    assert instance_to_model
    assert model_to_solution

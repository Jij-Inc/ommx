import pytest
from statistics import median

@pytest.mark.benchmark
def test_median_performance():
    return median([1, 2, 3, 4, 5])

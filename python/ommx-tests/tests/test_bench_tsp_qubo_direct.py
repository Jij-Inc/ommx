import pytest
import numpy as np
import ommx.v1


def generate_distance_matrix(num_city: int) -> np.ndarray:
    """Generate a random TSP distance matrix."""
    np.random.seed(42)  # Fixed seed for reproducibility
    x_pos = np.random.rand(num_city)
    y_pos = np.random.rand(num_city)
    XX, XX_T = np.meshgrid(x_pos, x_pos)
    YY, YY_T = np.meshgrid(y_pos, y_pos)
    distance = np.sqrt((XX - XX_T) ** 2 + (YY - YY_T) ** 2)
    return distance


@pytest.fixture(params=[2, 4, 8, 16, 32])
def tsp_distance_matrix(request):
    """Fixture to generate TSP distance matrices of different sizes."""
    num_city = request.param
    return generate_distance_matrix(num_city)


def make_tsp_qubo_by_ommx(
    distance: np.ndarray, lambda1: float = 1.0, lambda2: float = 1.0
):
    """
    Generate TSP QUBO using OMMX directly.

    This creates a QUBO formulation for the Traveling Salesman Problem where:
    - x[i][j] = 1 if city i is visited at time j
    - Objective: minimize total distance
    - Constraints: each city visited exactly once, each time slot has exactly one city

    Args:
        distance: Distance matrix between cities
        lambda1: Penalty weight for one-city-per-time constraint
        lambda2: Penalty weight for one-time-per-city constraint
    """
    num_city = distance.shape[0]

    # Create binary decision variables x[i][j]
    x = [
        [
            ommx.v1.DecisionVariable.binary(i * num_city + j, name=f"x_({i},{j})")
            for j in range(num_city)
        ]
        for i in range(num_city)
    ]

    # Objective: sum of distances between consecutive cities in the tour
    # Fixed logic: x[i][t] * x[j][t+1] means city i at time t followed by city j at time t+1
    objective = sum(
        distance[i, j] * x[i][t] * x[j][(t + 1) % num_city]
        for i in range(num_city)
        for j in range(num_city)
        for t in range(num_city)
    )

    # Constraint: each city must be visited exactly once
    one_city_const = sum(
        (sum(x[i][j] for j in range(num_city)) - 1)
        * (sum(x[i][j] for j in range(num_city)) - 1)
        for i in range(num_city)
    )

    # Constraint: each time slot must have exactly one city
    one_time_const = sum(
        (sum(x[i][j] for i in range(num_city)) - 1)
        * (sum(x[i][j] for i in range(num_city)) - 1)
        for j in range(num_city)
    )

    # Total Hamiltonian with configurable penalty weights
    return objective + lambda1 * one_city_const + lambda2 * one_time_const


@pytest.mark.benchmark
def test_tsp_qubo_direct_generation(tsp_distance_matrix: np.ndarray):
    """Benchmark the direct TSP QUBO generation using OMMX."""
    # Generate QUBO with custom penalty weights
    make_tsp_qubo_by_ommx(tsp_distance_matrix, lambda1=10.0, lambda2=10.0)

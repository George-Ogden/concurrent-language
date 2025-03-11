import argparse
import json
import os

import numpy as np
import pandas as pd
from fit import GROUPS, fit, get_final_coefficients, group_data, load_data, main


def test_load():
    df = load_data(os.path.join(os.path.dirname(__file__), "test_data", "vector.tsv"))
    target_df = pd.DataFrame(
        [
            [1, 2, 1, 0, 0, 1, 0, 0, 2, 100.0],
            [2, 1, 2, 0, 1, 0, 0, 0, 2, 50.0],
            [0, 0, 0, 2, 3, 1, 4, 1, 0, 30.0],
            [1, 0, 0, 8, 2, 0, 3, 9, 0, 35.0],
            [1, 0, 0, 8, 2, 0, 3, 9, 0, 25.0],
        ],
        columns=["builtin_bool", "builtin_int", "other", "==", "<=", "&", "|", "^", "**", "time"],
    )
    pd.testing.assert_frame_equal(df, target_df)


def test_group():
    df = pd.DataFrame(
        [
            [1, 2, 1, 0, 0, 1, 0, 0, 2, 100.0],
            [2, 1, 2, 0, 1, 0, 0, 0, 2, 50.0],
            [0, 0, 0, 2, 3, 1, 4, 1, 0, 30.0],
            [1, 0, 0, 8, 2, 0, 3, 9, 0, 35.0],
            [1, 0, 0, 8, 2, 0, 3, 9, 0, 25.0],
        ],
        columns=["builtin_bool", "builtin_int", "other", "==", "<=", "&", "|", "^", "**", "time"],
    )
    group_titles = [
        ["builtin_bool", "builtin_int"],
        ["==", "<="],
        ["&", "|", "^"],
    ]
    grouped_df = group_data(df, group_titles)
    target_df = pd.DataFrame(
        [
            [3, 1, 0, 1, 2, 100.0],
            [3, 2, 1, 0, 2, 50.0],
            [0, 0, 5, 6, 0, 30.0],
            [1, 0, 10, 12, 0, 35.0],
            [1, 0, 10, 12, 0, 25.0],
        ],
        columns=["0", "other", "1", "2", "**", "time"],
    )
    target_df = target_df[["other", "**", "time", "0", "1", "2"]]

    pd.testing.assert_frame_equal(grouped_df, target_df)


def test_fit_known():
    # time = 2*X0 + 3*X1 + 4*X2 + 5 + epsilon
    n = 1000
    [X0, X1, X2] = np.random.rand(3, n)
    y = 2 * X0 + 3 * X1 + 4 * X2 + 5 + np.random.randn() * 0.01

    df = pd.DataFrame(
        {
            "X0": X0,
            "X1": X1,
            "X2": X2,
            "time": y,
        }
    )

    coefficients = fit(df)
    expected = {"X0": 2, "X1": 3, "X2": 4, "_constant": 5}

    assert coefficients.keys() == expected.keys()
    for k, v in expected.items():
        assert np.isclose(v, coefficients[k], atol=0.1)


def test_fit_positive():
    # time = 2*X0 - 3*X1 + 4*X2 + 7 + epsilon
    n = 1000
    [X0, X1, X2] = np.random.rand(3, n)
    y = 2 * X0 - 3 * X1 + 4 * X2 + 7 + np.random.randn() * 0.01

    df = pd.DataFrame(
        {
            "X0": X0,
            "X1": X1,
            "X2": X2,
            "time": y,
        }
    )

    coefficients = fit(df)
    expected_keys = {"X0", "X1", "X2", "_constant"}

    assert set(coefficients.keys()) == expected_keys
    for v in coefficients.values():
        assert v >= 0


def test_final_coefficients():
    coefficients = {"0": 2.0, "1": 3.0, "X2": 4.0, "_constant": 5.0}

    groups = [
        ["X00", "X01"],
        ["X10", "X11", "X12", "X13"],
    ]

    expected_coefficients = {
        "X00": 2.0,
        "X01": 2.0,
        "X10": 3.0,
        "X11": 3.0,
        "X12": 3.0,
        "X13": 3.0,
        "X2": 4.0,
        "_constant": 5.0,
    }

    final_coefficients = get_final_coefficients(coefficients, groups)

    assert final_coefficients == expected_coefficients


def test_main(capsys):
    args = argparse.Namespace(
        log_filename=os.path.join(os.path.dirname(__file__), "test_data", "vector.tsv")
    )

    old_groups = GROUPS
    GROUPS[:] = [
        ["<=", "=="],
        ["&", "|", "^"],
    ]
    main(args)
    GROUPS[:] = old_groups

    captured = capsys.readouterr()
    result = json.loads(captured.out)

    assert set(result.keys()) == {
        "builtin_bool",
        "builtin_int",
        "other",
        "==",
        "<=",
        "&",
        "|",
        "^",
        "**",
        "_constant",
    }
    for v in result.values():
        assert v >= 0

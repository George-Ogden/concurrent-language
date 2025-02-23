import os.path

import pandas as pd
from benchmark_visualization import load_directory, merge_logs, normalize


def test_load_directory():
    df = load_directory(os.path.join(os.path.dirname(__file__), "test"))
    target_df = pd.DataFrame(
        [
            {"function": "fn100(1)", "duration": 100},
            {"function": "fn100(2)", "duration": 200},
            {"function": "fn100(3)", "duration": 300},
            {"function": "fn100(1)", "duration": 101},
            {"function": "fn100(2)", "duration": 202},
            {"function": "fn100(3)", "duration": 303},
            {"function": "fn100(1)", "duration": 110},
            {"function": "fn100(2)", "duration": 220},
            {"function": "fn100(3)", "duration": 330},
            {"function": "fn100(1)", "duration": float("nan")},
            {"function": "fn100(3)", "duration": float("nan")},
            {"function": "fn1000(1,1)", "duration": 101},
            {"function": "fn1000(2,3)", "duration": 203},
            {"function": "fn1000(1,1)", "duration": 111},
            {"function": "fn1000(2,3)", "duration": 213},
            {"function": "fn1000(1,1)", "duration": 121},
            {"function": "fn1000(2,3)", "duration": 223},
            {"function": "fn1000(2,3)", "duration": float("nan")},
        ]
    )
    target_df["title"] = "test example"
    pd.testing.assert_frame_equal(df, target_df)


def test_merge_logs():
    df1 = pd.DataFrame(
        [
            {"function": "fn100(1)", "duration": 100.0, "title": "df1"},
            {"function": "fn100(2)", "duration": 200.0, "title": "df1"},
            {"function": "fn100(3)", "duration": 300.0, "title": "df1"},
            {"function": "fn100(1)", "duration": 101.0, "title": "df1"},
            {"function": "fn100(2)", "duration": 202.0, "title": "df1"},
            {"function": "fn100(3)", "duration": 303.0, "title": "df1"},
        ]
    )
    df2 = pd.DataFrame(
        [
            {"function": "fn100(1)", "duration": 1000.0, "title": "df2"},
            {"function": "fn100(2)", "duration": 2000.0, "title": "df2"},
            {"function": "fn100(3)", "duration": 3000.0, "title": "df2"},
            {"function": "fn100(1)", "duration": 1010.0, "title": "df2"},
            {"function": "fn100(2)", "duration": 2020.0, "title": "df2"},
            {"function": "fn100(3)", "duration": 3030.0, "title": "df2"},
        ]
    )
    df3 = pd.DataFrame(
        [
            {"function": "fn100(1)", "duration": 1000.0, "title": "df3"},
            {"function": "fn100(2)", "duration": 2000.0, "title": "df3"},
            {"function": "fn100(1)", "duration": float("nan"), "title": "df3"},
            {"function": "fn100(2)", "duration": float("nan"), "title": "df3"},
        ]
    )
    target_df = pd.DataFrame(
        [
            {"function": "fn100(1)", "duration": 100.0, "title": "df1"},
            {"function": "fn100(2)", "duration": 200.0, "title": "df1"},
            {"function": "fn100(3)", "duration": 300.0, "title": "df1"},
            {"function": "fn100(1)", "duration": 101.0, "title": "df1"},
            {"function": "fn100(2)", "duration": 202.0, "title": "df1"},
            {"function": "fn100(3)", "duration": 303.0, "title": "df1"},
            {"function": "fn100(1)", "duration": 1000.0, "title": "df2"},
            {"function": "fn100(2)", "duration": 2000.0, "title": "df2"},
            {"function": "fn100(3)", "duration": 3000.0, "title": "df2"},
            {"function": "fn100(1)", "duration": 1010.0, "title": "df2"},
            {"function": "fn100(2)", "duration": 2020.0, "title": "df2"},
            {"function": "fn100(3)", "duration": 3030.0, "title": "df2"},
            {"function": "fn100(1)", "duration": 1000.0, "title": "df3"},
            {"function": "fn100(2)", "duration": 2000.0, "title": "df3"},
            {"function": "fn100(1)", "duration": float("nan"), "title": "df3"},
            {"function": "fn100(2)", "duration": float("nan"), "title": "df3"},
        ]
    )
    merged_df = merge_logs(df1, df2, df3)
    pd.testing.assert_frame_equal(merged_df, target_df)


def test_normalize():
    df = pd.DataFrame(
        [
            {"function": "fn100(1)", "duration": 10.0, "title": "title"},
            {"function": "fn100(2)", "duration": 20.0, "title": "title"},
            {"function": "fn100(3)", "duration": 3.0, "title": "title"},
            {"function": "fn100(1)", "duration": 100.0, "title": "title"},
            {"function": "fn100(2)", "duration": 20.0, "title": "title"},
            {"function": "fn100(3)", "duration": 3000.0, "title": "title2"},
            {"function": "fn100(1)", "duration": 1000.0, "title": "title2"},
            {"function": "fn100(2)", "duration": float("nan"), "title": "title2"},
            {"function": "fn100(3)", "duration": float("nan"), "title": "title2"},
        ]
    )
    target_df = pd.DataFrame(
        [
            {
                "function": "fn100(1)",
                "duration": 10.0,
                "title": "title",
                "normalized_performance": 10.0,
            },
            {
                "function": "fn100(2)",
                "duration": 20.0,
                "title": "title",
                "normalized_performance": 1.0,
            },
            {
                "function": "fn100(3)",
                "duration": 3.0,
                "title": "title",
                "normalized_performance": 10**1.5,
            },
            {
                "function": "fn100(1)",
                "duration": 100.0,
                "title": "title",
                "normalized_performance": 1.0,
            },
            {
                "function": "fn100(2)",
                "duration": 20.0,
                "title": "title",
                "normalized_performance": 1.0,
            },
            {
                "function": "fn100(3)",
                "duration": 3000.0,
                "title": "title2",
                "normalized_performance": 10**-1.5,
            },
            {
                "function": "fn100(1)",
                "duration": 1000.0,
                "title": "title2",
                "normalized_performance": 0.1,
            },
            {
                "function": "fn100(2)",
                "duration": float("nan"),
                "title": "title2",
                "normalized_performance": float("nan"),
            },
            {
                "function": "fn100(3)",
                "duration": float("nan"),
                "title": "title2",
                "normalized_performance": float("nan"),
            },
        ]
    )
    normalized_df = normalize(df)
    pd.testing.assert_frame_equal(normalized_df, target_df)

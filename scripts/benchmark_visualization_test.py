import os.path

import pandas as pd
from benchmark_visualization import load_directory, merge_logs


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
    print(df)
    print(target_df)
    assert df.equals(target_df)


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
    print(merged_df)
    print(target_df)
    assert merged_df.equals(target_df)

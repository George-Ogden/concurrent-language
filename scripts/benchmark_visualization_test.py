import os.path

import pandas as pd
from benchmark_visualization import load_directory


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

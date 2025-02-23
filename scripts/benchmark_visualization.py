import os.path

import pandas as pd


def convert_float_or_nan(x: str) -> float:
    try:
        return float(x)
    except ValueError:
        return float("nan")


def load_directory(directory: str) -> pd.DataFrame:
    log_filename = os.path.join(directory, "log.tsv")
    title_filename = os.path.join(directory, "title.txt")

    df = pd.read_csv(
        log_filename, sep="\t", index_col=False, converters={"duration": convert_float_or_nan}
    )
    df["function"] = df["name"] + "(" + df["args"] + ")"

    with open(title_filename) as f:
        title = f.read().strip()
    df["title"] = title

    return df[["function", "duration", "title"]]


def merge_logs(*logs: pd.DataFrame) -> pd.DataFrame:
    return pd.concat(logs, ignore_index=True)

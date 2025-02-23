import argparse
import os.path

import numpy as np
import pandas as pd
import plotly.express as px
import plotly.graph_objects as go


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


def normalize(df: pd.DataFrame) -> pd.DataFrame:
    df["log_duration"] = np.log10(df.duration)
    grouped_df = df.groupby("function").log_duration
    mean = grouped_df.transform("mean")
    df["normalized_performance"] = 10 ** (mean - df["log_duration"])
    del df["log_duration"]
    return df


def plot(data: pd.DataFrame) -> go.Figure:
    return px.strip(data, x="function", color="title", y="normalized_performance", log_y=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("directories", nargs="+")
    return parser.parse_args()


def main(args: argparse.Namespace):
    directories = args.directories
    data = normalize(merge_logs(*(load_directory(directory) for directory in directories)))

    fig = plot(data)
    fig.show()


if __name__ == "__main__":
    args = parse_args()
    main(args)

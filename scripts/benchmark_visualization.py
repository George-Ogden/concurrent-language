import argparse
import os.path
import warnings

import numpy as np
import pandas as pd
import plotly.express as px
import plotly.graph_objects as go
import plotly.io as pio

pio.kaleido.scope.mathjax = None


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
    # Write function as though it is a function call.
    df["function"] = df.name + "(" + df.args + ")"

    with open(title_filename) as f:
        title = f.read().strip()
    df["title"] = title

    return df[["function", "duration", "title"]]


def merge_logs(*logs: pd.DataFrame) -> pd.DataFrame:
    return pd.concat(logs, ignore_index=True)


def normalize(df: pd.DataFrame) -> pd.DataFrame:
    """Normalize by dividing by the geometric mean of timings fo r a given function."""
    df["log_duration"] = np.log10(df.duration)
    grouped_df = df.groupby("function").log_duration
    mean = grouped_df.transform("mean")
    df["normalized_performance"] = 10 ** (mean - df.log_duration)
    del df["log_duration"]
    return df


def plot(data: pd.DataFrame) -> go.Figure:
    fig = px.strip(
        data,
        x="function",
        y="normalized_performance",
        color="title",
        log_y=True,
        title="Normalized Function Performance across Versions",
        labels={
            "function": "Function and Arguments",
            "normalized_performance": "Normalized Performance",
            "title": "Version",
        },
    )
    fig.update_layout(legend_title_text="Version")
    return fig


def save_plot(fig: go.Figure, filepath: str) -> str:
    if not filepath.endswith(".pdf"):
        filepath += ".pdf"
    fig.write_image(file=filepath, height=1080, width=1920, format="pdf")
    return filepath


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("directories", nargs="+")
    parser.add_argument("--output-filename", "-o", required=False)
    parser.add_argument("--show-web-version", "-w", action="store_true")
    return parser.parse_args()


def main(args: argparse.Namespace):
    directories = args.directories
    show_web_version = args.show_web_version
    output_filename = args.output_filename

    if output_filename is None and not show_web_version:
        warnings.warn(
            "--output-filename and --show-web-version are False - this script will produce no output"
        )

    data = normalize(merge_logs(*(load_directory(directory) for directory in directories)))

    fig = plot(data)
    if show_web_version:
        fig.show()
    if output_filename is not None:
        save_plot(fig, output_filename)
        print(output_filename)


if __name__ == "__main__":
    args = parse_args()
    main(args)

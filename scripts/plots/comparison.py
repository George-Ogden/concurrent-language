import argparse
import os.path
import re
import sys
import warnings
from typing import Optional

import numpy as np
import pandas as pd
import plotly.express as px
import plotly.graph_objects as go
import plotly.io as pio
from plotly.subplots import make_subplots

pio.kaleido.scope.mathjax = None


def convert_float_or_nan(x: str) -> float:
    try:
        return float(x)
    except ValueError:
        return float("nan")


def load_directory(directory: str, extra_cols: Optional[list[str]] = None) -> pd.DataFrame:
    log_filename = os.path.join(directory, "log.tsv")
    title_filename = os.path.join(directory, "title.txt")

    df = pd.read_csv(
        log_filename,
        sep="\t",
        index_col=False,
        converters={"duration": convert_float_or_nan},
    )
    df["function"] = df.name + "(" + df.args + ")"

    with open(title_filename) as f:
        title = f.read().strip()
    df["title"] = title
    dirname, basename = os.path.split(directory)
    if basename == "":
        basename = os.path.basename(dirname)
    df["directory"] = basename

    if extra_cols is None:
        extra_cols = []
    return df[["function", "duration", "title", "directory", *extra_cols]]


def merge_logs(*logs: pd.DataFrame) -> pd.DataFrame:
    return pd.concat(logs, ignore_index=True)


def clean(df: pd.DataFrame) -> pd.DataFrame:
    df.dropna(subset=["duration"], inplace=True, ignore_index=True)

    def trim_group(group: pd.Series) -> pd.Series:
        return group.sort_values("duration").iloc[1:-1]

    grouped_df = df.groupby(["function", "title", "directory"], group_keys=False, sort=False)
    filtered_df = grouped_df.apply(trim_group)
    return filtered_df.reset_index(drop=True)


def normalize(df: pd.DataFrame, normalize_first_directory: bool = False) -> pd.DataFrame:
    grouped_df = df.groupby(["function", "title", "directory"], group_keys=False, sort=False).agg(
        {"duration": ["mean", "std"], "function": "count"}
    )
    grouped_df.columns = ["_".join(column) for column in grouped_df.columns.to_flat_index()]
    if normalize_first_directory:
        # Use first column as baseline.
        first_directory = df.loc[0, "directory"]
        reindexed_grouped_df = grouped_df.reset_index()
        baseline_map = (
            reindexed_grouped_df[reindexed_grouped_df.directory == first_directory]
            .set_index("function")
            .duration_mean.to_dict()
        )
        baseline_duration = reindexed_grouped_df.function.map(baseline_map)
        baseline_duration.index = grouped_df.index
    else:
        # Use mean duration as baseline.
        baseline_duration = grouped_df.groupby("function").duration_mean.transform("mean")

    grouped_df["normalized_performance"] = baseline_duration / grouped_df.duration_mean
    grouped_df["performance_lower"] = grouped_df.normalized_performance - baseline_duration / (
        grouped_df.duration_mean + grouped_df.duration_std / np.sqrt(grouped_df.function_count)
    )
    grouped_df["performance_upper"] = (
        baseline_duration
        / (grouped_df.duration_mean - grouped_df.duration_std / np.sqrt(grouped_df.function_count))
        - grouped_df.normalized_performance
    )
    df = grouped_df.reset_index()
    df["function_name"] = df["function"].str.extract(r"^([a-z\-]+)")
    columns = []
    i = 0
    while True:
        digit_pattern = r"\d+," * i + r"(\d+)"
        argument = df["function"].str.extract(r"^.*\(" + digit_pattern)
        if np.all(argument.isna()):
            break
        else:
            column_name = f"argument{i}"
            columns.append(column_name)
            df[column_name] = argument.fillna(0).astype(int)
            i += 1
    df.sort_values(by=["function_name", *columns], inplace=True, ignore_index=True)
    return df


def neaten(fig: go.Figure) -> go.Figure:
    fig.update_layout(paper_bgcolor="white", plot_bgcolor="white")
    fig.update_layout(
        legend=dict(
            itemsizing="constant",
            font=dict(size=24),
        )
    )
    for update in [fig.update_xaxes, fig.update_yaxes]:
        update(
            tickfont=dict(size=20),
            title_font=dict(size=24),
            showline=True,
            linewidth=1,
            linecolor="black",
            mirror=True,
        )

    for annotation in fig.layout.annotations:
        annotation.font.size = 30

    for trace in fig.data:
        trace.marker.size = 10
        trace.line.width = 5

    return fig


def plot(df: pd.DataFrame) -> go.Figure:
    functions = pd.unique(df.function)
    function_map = {f: i for i, f in enumerate(functions)}
    titles = pd.unique(df.title)
    title_map = {t: (i / (len(titles) - 1) - 0.5) / 2 for i, t in enumerate(titles)}
    df["x_base"] = df.function.map(function_map)
    df["jitter"] = df.title.map(title_map)
    df["x_jittered"] = df.x_base + df.jitter
    function_names = pd.unique(df.function_name)
    fig = make_subplots(
        rows=2,
        cols=5,
        subplot_titles=function_names,
        shared_yaxes=False,
        shared_xaxes=False,
    )

    for i, function in enumerate(function_names):
        row = i // 5 + 1
        col = i % 5 + 1
        subset = df[df["function_name"] == function]

        scatter = px.scatter(
            subset,
            x="x_jittered",
            y="normalized_performance",
            error_y="performance_upper",
            error_y_minus="performance_lower",
            color="title",
            log_y=True,
            labels={
                "function": "Function and Arguments",
                "normalized_performance": "Relative Performance",
                "title": "Version",
            },
        )

        for trace in scatter.data:
            if row == 1 and col == 1:
                trace.showlegend = True
            else:
                trace.showlegend = False
            trace.marker.symbol = "x"
            fig.add_trace(trace, row=row, col=col)

    for k in fig.layout:
        if re.search(r"xaxis\d*", k):
            fig.layout[k].update(
                matches=None,
                tickmode="array",
                tickvals=list(function_map.values()),
                ticktext=list(function_map.keys()),
                tickangle=45,
            )

    fig.update_layout(
        legend=dict(
            yanchor="bottom",
            xanchor="center",
            orientation="h",
            y=1.1,
            x=0.5,
        )
    )
    fig = neaten(fig)

    return fig


def save_plot(fig: go.Figure, directory: str, filename: str = "plot.pdf") -> str:
    filepath = os.path.join(directory, filename)
    os.makedirs(directory, exist_ok=True)
    fig.write_image(file=filepath, height=1080, width=1920, format="pdf")
    with open(os.path.join(directory, "args"), "w") as f:
        f.write(" ".join(sys.argv))
    return filepath


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("directories", nargs="+")
    parser.add_argument("--output-folder", "-o", required=False)
    parser.add_argument("--show-web-version", "-w", action="store_true")
    parser.add_argument("--normalize-first-directory", "-n", action="store_true")
    return parser.parse_args()


def main(args: argparse.Namespace):
    directories = args.directories
    show_web_version = args.show_web_version
    output_folder = args.output_folder
    normalize_first_directory = args.normalize_first_directory

    if output_folder is None and not show_web_version:
        warnings.warn(
            "--output-folder and --show-web-version are False - this script will produce no output"
        )

    data = normalize(
        clean(merge_logs(*(load_directory(directory) for directory in directories))),
        normalize_first_directory=normalize_first_directory,
    )

    fig = plot(data)
    if show_web_version:
        fig.show()
    if output_folder is not None:
        filename = save_plot(fig, output_folder)
        print(filename)


if __name__ == "__main__":
    args = parse_args()
    main(args)

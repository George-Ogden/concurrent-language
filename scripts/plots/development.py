import argparse
import warnings

import numpy as np
import pandas as pd
import plotly.express as px
import plotly.graph_objects as go
from comparison import (
    clean,
    load_directory,
    merge_logs,
    neaten,
    normalize,
    parse_args,
    save_plot,
)


def plot(df: pd.DataFrame) -> go.Figure:
    iterations = {dir: i for i, dir in enumerate(reversed(np.sort(pd.unique(df.directory))))}
    df["iteration"] = df.directory.map(iterations)

    pivot_df = pd.pivot_table(
        df,
        index=["function_name", "function"],
        columns="iteration",
        values="normalized_performance",
        sort=False,
    )
    functions = []
    for function_name in pivot_df.index.get_level_values(0):
        filtered_df = pivot_df.loc[function_name]
        filtered_df.dropna(how="all", axis=1, inplace=True)
        filtered_df.dropna(how="any", axis=0, inplace=True)
        filtered_df.reset_index(inplace=True)
        functions.append(filtered_df.function.loc[filtered_df.index[-1]])

    df = df.loc[df.function.isin(functions)].copy()
    base = df.groupby("function").normalized_performance.transform(np.nanmin)
    df.normalized_performance /= base
    df.performance_upper /= base
    df.performance_lower /= base
    df.sort_values(by=["iteration", "function"], inplace=True, ignore_index=True)
    fig = px.line(
        df,
        x="iteration",
        y="normalized_performance",
        color="function",
        log_y=True,
        labels={
            "function": "Function and Arguments",
            "normalized_performance": "Normalized Performance",
            "iteration": "Update",
        },
        markers=True,
    )
    titles = df.groupby("iteration").title.first()
    fig.update_xaxes(
        tickmode="array",
        tickvals=list(range(len(titles))),
        ticktext=titles,
        tickangle=45,
    )
    fig.update_traces(marker=dict(symbol="cross"))

    fig = neaten(fig)
    fig.update_layout(
        legend=dict(
            font=dict(size=32),
        )
    )
    for update in [fig.update_xaxes, fig.update_yaxes]:
        update(
            tickfont=dict(size=32),
            title_font=dict(size=40),
        )

    for trace in fig.data:
        trace.marker.size = 20
        trace.line.width = 8

    return fig


def main(args: argparse.Namespace):
    directories = args.directories
    show_web_version = args.show_web_version
    output_folder = args.output_folder

    if output_folder is None and not show_web_version:
        warnings.warn(
            "--output-folder and --show-web-version are False - this script will produce no output"
        )

    data = normalize(clean(merge_logs(*(load_directory(directory) for directory in directories))))

    fig = plot(data)
    if show_web_version:
        fig.show()
    if output_folder is not None:
        filename = save_plot(fig, output_folder)
        print(filename)


if __name__ == "__main__":
    args = parse_args()
    main(args)

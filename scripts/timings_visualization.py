import argparse
import json

import pandas as pd
import plotly.express as px
import plotly.graph_objects as go


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("log_filename")
    parser.add_argument("json_filename", nargs="?")
    return parser.parse_args()


def main(args: argparse.Namespace):
    filename = args.log_filename
    json_filename = args.json_filename

    df = pd.read_csv(filename, sep="\t", index_col=False)

    fig = px.box(
        df,
        y="time",
        color="sample",
        x="sample",
        title="Distribution of runtimes across programs",
        labels={
            "sample": "Sample",
            "time": "Runtime (ns)",
        },
        points="all",
        hover_data={"time": ":.0f"},
    )
    if json_filename is not None:
        with open(json_filename) as f:
            coefficients = json.load(f)
        unique_samples = pd.unique(df["sample"])

        predicted_times = []

        for sample in unique_samples:
            example = df.loc[df["sample"] == sample].iloc[0]
            predicted_time = coefficients["_constant"] + sum(
                example[k] * v for k, v in coefficients.items() if k != "_constant"
            )
            predicted_times.append(predicted_time)

        fig.add_trace(
            go.Scatter(
                x=unique_samples,
                y=predicted_times,
                mode="markers",
                marker=dict(color="black", size=5, symbol="x"),
                name="Predicted Time",
            )
        )

    fig.update_layout(legend_title_text="Sample")
    fig.show()


if __name__ == "__main__":
    args = parse_args()
    main(args)

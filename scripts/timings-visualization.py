import argparse

import pandas as pd
import plotly.express as px


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("log_filename")
    return parser.parse_args()


def main(args: argparse.Namespace):
    filename = args.log_filename

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
    fig.update_layout(legend_title_text="Sample")
    fig.show()


if __name__ == "__main__":
    args = parse_args()
    main(args)

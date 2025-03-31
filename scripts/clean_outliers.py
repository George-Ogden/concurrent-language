import argparse
import functools
import os.path

import numpy as np
import pandas as pd


def load_data(directory: str) -> pd.DataFrame:
    filename = os.path.join(directory, "vector.tsv")
    df = pd.read_csv(filename, sep="\t", index_col=False)
    return df.dropna(how="any", axis=0, ignore_index=True)


def remove_outliers_iteration(df: pd.DataFrame, threshold: float) -> pd.DataFrame:
    """Remove timings more than `threshold` times the standard deviation outside the mean for a sample program."""
    mean = df.groupby("sample").time.transform("mean")
    std = df.groupby("sample").time.transform(functools.partial(np.nanstd, ddof=0))
    z = np.abs(df.time - mean) / std
    return df.loc[z < threshold].reset_index(drop=True)


def remove_outliers(df: pd.DataFrame, threshold: float) -> pd.DataFrame:
    """Remove outliers iteratively until there are no updates."""
    length = None
    while length is None or length != len(df):
        length = len(df)
        df = remove_outliers_iteration(df, threshold)

    return df


def save_data(df: pd.DataFrame, directory: str) -> None:
    filename = os.path.join(directory, "clean_vector.tsv")
    df.to_csv(filename, index=False, sep="\t")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("log_directory")
    parser.add_argument("--z-threshold", "-t", "-z", default=2.5, type=float)
    return parser.parse_args()


def main(args: argparse.Namespace):
    directory = args.log_directory
    threshold = args.z_threshold

    df = load_data(directory)
    df = remove_outliers(df, threshold)
    save_data(df, directory)


if __name__ == "__main__":
    args = parse_args()
    main(args)

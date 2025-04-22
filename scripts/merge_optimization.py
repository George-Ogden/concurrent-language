import argparse
import os
import re

import pandas as pd


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("folders", nargs="+", help="Folders with optimization results.")
    parser.add_argument("--output-folder", required=True, help="Folder to save combined results.")
    return parser.parse_args()


def load_folder(folder: str) -> pd.DataFrame:
    log_filename = os.path.join(folder, "log.tsv")
    title_filename = os.path.join(folder, "title.txt")
    df = pd.read_csv(log_filename, sep="\t", index_col=False)
    with open(title_filename) as f:
        optimization, num_cpus, cpu = f.read().strip().split()
    assert re.match(r"cpus?", cpu)
    df["optimization"] = optimization
    df["num_cpus"] = int(num_cpus)
    return df


def merge_folders(folders: list[str]) -> pd.DataFrame:
    data = []
    for folder in folders:
        df = load_folder(folder)
        data.append(df)
    return pd.concat(data, ignore_index=True)


def save_df(df: pd.DataFrame, output_folder: str) -> None:
    output_filename = os.path.join(output_folder, "log.tsv")
    os.makedirs(output_folder, exist_ok=True)
    df.to_csv(output_filename, sep="\t", index=False)


def main(args: argparse.Namespace) -> None:
    folders = args.folders
    output_folder = args.output_folder
    df = merge_folders(folders)
    save_df(df, output_folder)


if __name__ == "__main__":
    args = parse_args()
    main(args)

import argparse
import json

import pandas as pd
from sklearn import linear_model

GROUPS = [
    ["==", "<="],
    ["&", "|", "^"],
]


def load_data(filename: str) -> pd.DataFrame:
    df = pd.read_csv(filename, sep="\t", index_col=False)
    df.drop(["sample"], inplace=True, axis=1)
    return df


def group_data(df: pd.DataFrame, groups: list[list[str]]) -> pd.DataFrame:
    df = df.copy()
    for i, group in enumerate(groups):
        combined_values = df[group].sum(axis=1)
        df[str(i)] = combined_values
        df.drop(group, axis=1, inplace=True)
    return df


def fit(df: pd.DataFrame) -> dict[str, float]:
    target = df.pop("time")

    reg = linear_model.LinearRegression(positive=True)
    reg.fit(df.values, target.values)

    coefficients = {column: coef for column, coef in zip(df.columns, reg.coef_, strict=True)}
    coefficients["_constant"] = reg.intercept_

    return coefficients


def get_final_coefficients(
    coefficients: dict[str, float], groups: list[list[str]]
) -> dict[str, float]:
    final_coefficients = {}
    for k, v in coefficients.items():
        try:
            idx = int(k)
        except ValueError:
            final_coefficients[k] = v
        else:
            for k in groups[idx]:
                final_coefficients[k] = v
    return final_coefficients


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("log_filename")
    return parser.parse_args()


def main(args: argparse.Namespace):
    filename = args.log_filename
    df = load_data(filename)
    grouped_df = group_data(df, groups=GROUPS)
    coefficients = fit(grouped_df)
    final_coefficients = get_final_coefficients(coefficients, groups=GROUPS)
    print(json.dumps(final_coefficients, indent=4, sort_keys=True))


if __name__ == "__main__":
    args = parse_args()
    main(args)

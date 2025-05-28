#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "polars",
#   "seaborn",
#   "rich",
# ]
# ///

import json
import glob
import re
import io
import os
from os import path
from statistics import mean
import math
from pathlib import Path
from rich.console import Console

import polars as pl # type: ignore
import seaborn as sns # type: ignore

os.environ['REQUESTS_CA_BUNDLE'] = "/etc/ssl/certs/ca-certificates.crt"
console = Console()

if "VIRTUAL_ENV" not in os.environ:
    exit("Run this script from a venv to avoid polluting your system.")
else:
    console.print(f"You're running from venv: {os.environ["VIRTUAL_ENV"]}")

data_dir = '../../out/or_coins'

all_files = glob.glob(path.join(data_dir, "gen_*.json"))
gen_pattern = 'gen_0*([0-9]*).json$'

def extract_gen_number(filename):
    return int(re.search(gen_pattern, filename).group(1))

all_files.sort(key=extract_gen_number)
all_files


particles = []
meta = []
for idx,file in enumerate(all_files):
    with open(file) as f:
        # 1. Read the JSON
        # 2. Tear out the sections we want from each array entry and...
        # 3. ... dump them back into JSON arrays
        # 4. Load them into a data frame
        
        # 1.
        data = json.load(f)
        
        # 2. & 3.
        parameter_sets = json.dumps([x["parameters"] for x in data["pop"]["normalised_particles"]])
        scores = [x["score"] for x in data["pop"]["normalised_particles"]]
        gen_number = extract_gen_number(file)
        
        # 4.
        particles.append(
            pl.read_json(io.StringIO(parameter_sets))
            .with_columns(pl.lit(gen_number).alias("gen_number"))
            .with_columns(pl.Series("score", scores))
        )
        meta.append(
            pl.DataFrame()
            .with_columns(pl.lit(gen_number).alias("generation"))
            .with_columns(pl.lit(data["next_gen_tolerance"]).alias("tolerance"))
            .with_columns(pl.lit(data["pop"]["acceptance"]).alias("acceptance"))
        )
        
particle_df = pl.concat(particles)
meta_df = pl.concat(meta)

print(particle_df.head(3))
print(meta_df.head(3))

meta_df = meta_df.with_columns(
    pl.Series(
        name='log_tolerance', 
        values=meta_df['tolerance'].map_elements(lambda x: math.log(1 + min(x, 0.5)), return_dtype=pl.Float64)
    )
)
meta_melted = meta_df.melt('generation', variable_name='statistic', value_vars=['log_tolerance', 'acceptance'])

latest_gen = meta_df.get_column("generation").max()
print("latest gen is", latest_gen)

def plot_meta(meta):
    facet_grid = sns.FacetGrid(meta.melt('generation', variable_name='statistic', value_vars=[
        'tolerance', 'acceptance']),
        row="statistic",
        aspect=4,
        sharey=False
    )

    facet_grid.map_dataframe(
        sns.barplot,
        x='generation',
        y='value',
        alpha= 0.8,
        palette = "viridis_r",
        # hue='generation'
    )
    fig = facet_grid
    fig.savefig( f'out/meta_gen{latest_gen:03}.pdf')

def plot_posterior(particles):
    g = sns.FacetGrid(
        particles
            .lazy()
            .select(pl.exclude(["score"]))
            .collect()
            .melt("gen_number"),   
        col="variable",
        hue="gen_number",
        palette="viridis_r",
        col_wrap=3,
        sharex=False,
        sharey=False,
        legend_out=True,
        despine=False
    )
    g.map(
        sns.kdeplot,
        "value",
        fill=True,
        alpha=.1,
        linewidth=1,
        bw_adjust=.8,
        cut=0,
    )
    g.set(ylabel=None)
    g.set(xlabel=None)
    g.set(yticklabels=[])
    g.tight_layout()
    g.add_legend()
    g.savefig( f'out/posterior_gen{latest_gen:03}.pdf')

def plot_correlations(particles):
    fig = sns.PairGrid(
        particles
        .lazy()
        .filter(pl.col("gen_number") == latest_gen)
        .select(pl.exclude(["score", "gen_number"]))
        .collect(),
        diag_sharey=False
    )
    fig.map_lower(sns.kdeplot, cmap="Blues",
                  fill=True, bw_adjust=0.75, thresh=0)
    fig.map_upper(sns.scatterplot, s=15)
    fig.map_diag(sns.kdeplot, cut=0, bw_adjust=0.6, fill=True)
    fig.savefig(f'out/correlations_gen{latest_gen:03}.pdf')

plot_meta(meta_df)
plot_posterior(particle_df)
plot_correlations(particle_df)

# TODO lots of deprecation warnings - make them go away

# # Score plot # CG Do we still want this? 
# fig, ax = plt.subplots(figsize=(8,4))
# score_max = max(particle_df['score'])
# ax.set_xlim(0, score_max)
# sns.kdeplot(
#     data=particle_df, 
#     x='score', 
#     hue='gen_number',
#     fill=True, 
#     palette="rocket_r",
#     alpha=.1, 
#     linewidth=1,
#     bw_adjust=.8, 
#     cut=0,
# ).set(title='Acceptance Rate')
# plt.savefig("plot_2.png", format='png', dpi=300)
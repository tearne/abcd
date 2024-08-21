import json
import glob
import re
import dpath.util
import io
import polars as pl
import os
from os import path
from statistics import mean
import seaborn as sns
import matplotlib.pyplot as plt
import math
from pathlib import Path

if 'VIRTUAL_ENV' not in os.environ:
    exit("Run this script from a venv to avoid polluting your system.")

data_dir = '../../../out/unfair_coin'

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

# Metadata plot
sns.FacetGrid(
    meta_melted,
    row="statistic",
    aspect=4,
).map_dataframe(
    sns.barplot,
    x='generation', 
    y='value',
    color='grey'
)
plt.savefig("plot_0.png", format='png', dpi=300)


# Posterior distribution
fig, ax = plt.subplots(figsize=(8,4))
ax.set_xlim(0, 1)
sns.kdeplot(
   data=particle_df,
   x="heads", 
   hue='gen_number',
   fill=True, 
   palette="viridis_r",
   alpha=.1, 
   linewidth=1,
   bw_adjust=0.8, 
   cut=0, 
).set(title='Posterior Heads')
sns.move_legend(ax, "upper left")
plt.savefig("plot_1.png", format='png', dpi=300)


# Score plot
fig, ax = plt.subplots(figsize=(8,4))
score_max = max(particle_df['score'])
ax.set_xlim(0, score_max)
sns.kdeplot(
    data=particle_df, 
    x='score', 
    hue='gen_number',
    fill=True, 
    palette="rocket_r",
    alpha=.1, 
    linewidth=1,
    bw_adjust=.8, 
    cut=0,
).set(title='Acceptance Rate')
plt.savefig("plot_2.png", format='png', dpi=300)

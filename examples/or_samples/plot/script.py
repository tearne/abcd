#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.12"
# dependencies = [
#   "polars",
#   "seaborn",
# ]
# ///

import json
import io
import polars as pl # type: ignore
import os
import seaborn as sns # type: ignore
import matplotlib.pyplot as plt # type: ignore
import matplotlib # type: ignore
from pathlib import Path

this_dir = os.path.dirname(os.path.realpath(__file__))
os.chdir(this_dir)

project_dir = Path("..", "..", "..")
particles_dir = project_dir / "out/or_coins"

all_gens = list(Path(particles_dir).glob('*.json'))

particles_file = max(all_gens)

samples_1 = project_dir / "out/or_coins/samples/samples_1.json"
samples_2 = project_dir / "out/or_coins/samples/samples_2.json"
samples_3 = project_dir / "out/or_coins/samples/samples_3.json"

assert particles_file.exists()
with open(particles_file) as f:
    raw = json.load(f)
params = [p["parameters"] for p in raw["pop"]["normalised_particles"]]
gen = pl.read_json(io.StringIO(json.dumps(params)))
print(gen)

assert samples_1.exists()
with open(samples_1) as f:
    raw = json.load(f)
samples_1 = pl.read_json(io.StringIO(json.dumps(raw["samples"])))
print("samples:", samples_1)

assert samples_2.exists()
with open(samples_2) as f:
    raw = json.load(f)
samples_2 = pl.read_json(io.StringIO(json.dumps(raw["samples"])))
print("samples:", samples_2)


assert samples_3.exists()
with open(samples_3) as f:
    raw = json.load(f)
samples_3 = pl.read_json(io.StringIO(json.dumps(raw["samples"])))
print("samples:", samples_3)

fig = plt.figure()
sns.scatterplot(data=gen, x="alpha", y="beta")
sns.kdeplot(data=samples_1, x="alpha", y="beta", color='black', linewidths=0.5, bw_adjust=2.5).set_title("Proposal density given previous particles")
# sns.kdeplot(data=samples_2, x="alpha", y="beta", color='blue', linewidths=0.5, bw_adjust=2.5).set_title("Proposal density given previous particles")
# sns.kdeplot(data=samples_3, x="alpha", y="beta", color='red', linewidths=0.5, bw_adjust=2.5).set_title("Proposal density given previous particles")

fig.tight_layout()
plt.savefig("plot.png", format='png', dpi=300)
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

project_dir = Path("..", "..")
particles_dir = project_dir / "out/or_coins_olcm"
print("particles dir", particles_dir.resolve())

all_gens = list(Path(particles_dir).glob('*.json'))

particles_file = max(all_gens)

trivial = project_dir / "out/samples/trivial.json"

olcm_1 = project_dir / "out/samples/olcm_1.json"
olcm_2 = project_dir / "out/samples/olcm_2.json"
olcm_3 = project_dir / "out/samples/olcm_3.json"

assert particles_file.exists()
with open(particles_file) as f:
    raw = json.load(f)
params = [p["parameters"] for p in raw["pop"]["normalised_particles"]]
gen = pl.read_json(io.StringIO(json.dumps(params)))
print(gen)

assert trivial.exists()
with open(trivial) as f:
    raw = json.load(f)
trivial = pl.read_json(io.StringIO(json.dumps(raw["samples"])))
print("samples:", trivial)


assert olcm_1.exists()
with open(olcm_1) as f:
    raw = json.load(f)
olcm_1 = pl.read_json(io.StringIO(json.dumps(raw["samples"])))
print("samples:", olcm_1)

assert olcm_2.exists()
with open(olcm_2) as f:
    raw = json.load(f)
olcm_2 = pl.read_json(io.StringIO(json.dumps(raw["samples"])))
print("samples:", olcm_2)


assert olcm_3.exists()
with open(olcm_3) as f:
    raw = json.load(f)
olcm_3 = pl.read_json(io.StringIO(json.dumps(raw["samples"])))
print("samples:", olcm_3)

fig = plt.figure(figsize=(10,5))
plt.subplot(1,2,1)
sns.scatterplot(data=gen, x="alpha", y="beta").set_title("Proposal density with trivial kernel")
sns.kdeplot(data=trivial, x="alpha", y="beta", color='black', linewidths=0.5, bw_adjust=2.5)
plt.xlim((-0.25,1.25))
plt.ylim((-0.25,1.25))

plt.subplot(1,2,2)
sns.scatterplot(data=gen, x="alpha", y="beta").set_title("Proposal density with OLCM kernel")
sns.kdeplot(data=olcm_1, x="alpha", y="beta", color='black', linewidths=0.5, bw_adjust=2.5)
# sns.kdeplot(data=olcm_2, x="alpha", y="beta", color='blue', linewidths=0.5, bw_adjust=2.5)
# sns.kdeplot(data=olcm_3, x="alpha", y="beta", color='red', linewidths=0.5, bw_adjust=2.5)
plt.xlim((-0.25,1.25))
plt.ylim((-0.25,1.25))

fig.tight_layout()
plt.savefig("plot.png", format='png', dpi=300)
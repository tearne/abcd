import json
import glob
import re
import io
import polars as pl
import os
import seaborn as sns
import matplotlib.pyplot as plt
import math
import matplotlib

this_dir = os.path.dirname(os.path.realpath(__file__))
os.chdir(this_dir)

with open("../particles.json") as f:
    raw = json.load(f)
params = [p["parameters"] for p in raw["pop"]["normalised_particles"]]
gen = pl.read_json(io.StringIO(json.dumps(params)))
print(gen)

with open("../../../../out/samples.json") as f:
    raw = json.load(f)
samples = pl.read_json(io.StringIO(json.dumps(raw["samples"])))
mean = pl.read_json(io.StringIO(json.dumps(raw["mean"])))
print("samples:", samples)
print("mean:", mean)

fig = plt.figure()
sns.scatterplot(data=gen, x="x", y="y")
sns.kdeplot(data=samples, x="x", y="y", color='black', linewidths=0.5, bw_adjust=2.5).set_title("Proposal density given previous particles")
sns.scatterplot(data=mean, x="x", y="y", color='red')
fig.tight_layout()
plt.savefig("../../../../out/plot.png", format='png', dpi=300)
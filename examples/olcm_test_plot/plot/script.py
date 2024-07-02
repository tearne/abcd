import json
import io
import polars as pl
import os
import seaborn as sns
import matplotlib.pyplot as plt
import matplotlib
from pathlib import Path

this_dir = os.path.dirname(os.path.realpath(__file__))
os.chdir(this_dir)

project_dir = Path("..", "..", "..")

particles_file = project_dir / "resources/test/olcm/particles.json"
samples_file = project_dir / "out/samples.json"

assert particles_file.exists()
with open(particles_file) as f:
    raw = json.load(f)
params = [p["parameters"] for p in raw["pop"]["normalised_particles"]]
gen = pl.read_json(io.StringIO(json.dumps(params)))
print(gen)

assert samples_file.exists()
with open(samples_file) as f:
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
plt.savefig("plot.png", format='png', dpi=300)
import json
import glob
import re
import pandas as pd
from os import path

data_dir = 'out'

all_files = glob.glob(path.join(data_dir, "gen_*.json"))
gen_pattern = 'gen_0*([0-9]*).json$'

def extract_gen_number(filename):
    return int(re.search(gen_pattern, filename).group(1))

all_files.sort(key=extract_gen_number)

print(all_files)


dfs = []
for filename in all_files:
    gen_number = extract_gen_number(filename)
    print("Gen number", gen_number, "from filename", filename)
    
    with open(filename) as f:
        data = json.load(f)
    new_df = pd.DataFrame(data["pop"]["normalised_particles"])
    new_df['generation'] = gen_number
    
    dfs.append(new_df)


df = pd.concat(dfs)
print(df)

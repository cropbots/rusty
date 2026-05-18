import json
import os

puzzles = [
    {
        "id": "basic_lock",
        "prompt": "Lock Puzzle: write code that scans then turns right.",
        "starter_code": "function solve()\n  scan()\n  turn.right()\nend",
        "validator_contains": "scan()"
    }
]

# Let's generate a bunch of simple lock puzzles
verbs = [
    ("move.forward(1)", "moves forward 1"),
    ("move.forward(2)", "moves forward 2"),
    ("move.forward(3)", "moves forward 3"),
    ("move.back(1)", "moves back 1"),
    ("move.back(2)", "moves back 2"),
    ("turn.left()", "turns left"),
    ("turn.right()", "turns right"),
    ("scan()", "scans"),
    ("harvest()", "harvests"),
    ("wait(1)", "waits 1 second"),
]

import random
random.seed(42)

for i in range(1, 50):
    num_steps = random.randint(2, 4)
    steps = random.sample(verbs, k=num_steps)
    
    prompt = "Lock Puzzle: write code that " + ", then ".join([s[1] for s in steps]) + "."
    starter_code = "function solve()\n"
    for s in steps:
        starter_code += "  " + s[0] + "\n"
    starter_code += "end"
    
    # Randomly pick one line to be missing, but require it in validator
    missing_idx = random.randint(0, num_steps - 1)
    validator = steps[missing_idx][0]
    
    starter_code_lines = starter_code.split("\n")
    # remove the missing line
    del starter_code_lines[missing_idx + 1]
    starter_code = "\n".join(starter_code_lines)
    
    puzzles.append({
        "id": f"lock_{i}",
        "prompt": prompt,
        "starter_code": starter_code,
        "validator_contains": validator
    })

os.makedirs("/home/rustle/Documents/@project/cropbot/src/puzzle", exist_ok=True)
index = []

for p in puzzles:
    filename = p['id'] + ".json"
    index.append(filename)
    with open(f"/home/rustle/Documents/@project/cropbot/src/puzzle/{filename}", "w") as f:
        json.dump(p, f, indent=2)

with open("/home/rustle/Documents/@project/cropbot/src/puzzle/index.json", "w") as f:
    json.dump(index, f, indent=2)

print("Generated", len(puzzles), "puzzles.")

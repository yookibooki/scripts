#!/usr/bin/env python3
import re
import urllib.request

URL = "https://artificialanalysis.ai/leaderboards/models?status=all"
OUTPUT = "table.md"

html = urllib.request.urlopen(URL).read().decode()

pattern = r'slug\\":\\"([^"]+)\\"[^}]{0,500}?intelligenceIndex\\":([0-9.]+)'
matches = re.findall(pattern, html)
matches.sort(key=lambda x: float(x[1]), reverse=True)

with open(OUTPUT, "w") as f:
    f.write("| Name | Score |\n|------|-------|\n")
    for slug, score in matches:
        f.write(f"| {slug} | {round(float(score), 2)} |\n")

print(f"Saved {len(matches)} models to {OUTPUT}")

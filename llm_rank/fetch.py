import json, os, re, urllib.request

URL = "https://artificialanalysis.ai/leaderboards/models"
PUSH = re.compile(
    r"self\.__next_f\.push\(\s*\[\s*1\s*,\s*\"((?:\\.|[^\"\\])*)\"\s*\]\s*\)\s*;?",
    re.DOTALL,
)

def label(m):
    for k in ("name_and_creator_label", "name", "short_name", "slug"):
        v = m.get(k)
        if isinstance(v, str) and v.strip():
            return v.strip()
    return ""

def score(m):
    v = m.get("estimated_intelligence_index")
    try:
        return None if v is None else float(v)
    except Exception:
        return None

def main():
    html = urllib.request.urlopen(
        urllib.request.Request(URL, headers={"User-Agent": "update_models.py"}),
        timeout=60,
    ).read().decode("utf-8", "replace")

    chunks = PUSH.findall(html)
    if not chunks:
        raise SystemExit("No __next_f chunks found")
    payload = "".join(json.loads('"' + c + '"') for c in chunks)

    k = payload.find('"models":')
    if k < 0:
        raise SystemExit('Could not find "models":')
    start = payload.find("[", k)
    if start < 0:
        raise SystemExit("Could not find models array")

    depth = 0
    in_s = False
    esc = False
    q = ""
    end = -1
    for i in range(start, len(payload)):
        ch = payload[i]
        if in_s:
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == q:
                in_s = False
            continue
        if ch in ('"', "'"):
            in_s = True
            q = ch
        elif ch == "[":
            depth += 1
        elif ch == "]":
            depth -= 1
            if depth == 0:
                end = i + 1
                break
    if end < 0:
        raise SystemExit("Unterminated models array")

    models = json.loads(payload[start:end])
    models = [m for m in models if isinstance(m, dict) and not m.get("deleted")]
    models.sort(key=lambda m: (score(m) is None, -(score(m) or 0.0), label(m).casefold()))

    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "models.md")
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        for i, m in enumerate(models, 1):
            f.write(f"{i} {label(m)}\n")

if __name__ == "__main__":
    main()

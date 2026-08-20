#!/usr/bin/env python3
"""Golden corpus: every schema-valid config, kept beside the SVG it renders.

    test/golden/<sha512 of the SVG>/<sha512 of the JSON>_parameters.json
                                   /<sha512 of the SVG>_background.svg

One rule covers both files: each is named by the sha512 of its own bytes,
exactly as written. So `sha512sum` reproduces every name in the corpus, and
the SVG additionally reproduces the directory holding it -- nothing has to
trust this program to check its own work:

    sha512sum test/golden/<D>/*                             -> <F>, <D>
    nix run .#bgsvg -- test/golden/<D>/<F>_parameters.json
    sha512sum out/trihex-*.svg                              -> D

`cargo test` asserts the invariants a render must satisfy; this asserts the
render did not change at all, and parses each one as XML on the way past.
Keeping the SVG rather than only its hash is what lets a failure say *what*
moved instead of just *that* something did.

    cargo build --release && python3 test/golden.py    # verify
    python3 test/golden.py --regen                     # rewrite after an intended change
"""
import argparse
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
from xml.dom.minidom import parseString

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def bgsvg():
    """The renderer under test: $BGSVG, else a cargo target, else PATH."""
    for c in (os.environ.get("BGSVG"),
              os.path.join(REPO, "target", "release", "bgsvg"),
              os.path.join(REPO, "target", "debug", "bgsvg")):
        if c and os.access(c, os.X_OK):
            return c
    found = shutil.which("bgsvg")
    if found:
        return found
    sys.exit("no bgsvg binary: run `cargo build` or set $BGSVG")


def bgsvg_run(args, cwd=None):
    p = subprocess.run([bgsvg(), *args], cwd=cwd, capture_output=True, text=True)
    if p.returncode:
        sys.exit(f"bgsvg {' '.join(args)} exited {p.returncode}: {p.stderr.strip()}")
    return p.stdout


def configs():
    """The corpus's contract with the renderer. The enumeration lives in Rust so
    that a new enum value grows this corpus and the cargo tests together --
    describing the axes twice is how one surface silently stops covering them."""
    return [line for line in bgsvg_run(["--configs"]).splitlines() if line]


ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "golden")
JSON_SUFFIX = "_parameters.json"
SVG_SUFFIX = "_background.svg"
SIZE = (1920, 1080)              # the schema default; the goldens carry no output key


def canon(line):
    """The bytes that land on disk: exactly what --configs printed, plus the
    trailing newline that keeps it a POSIX text file. The name hashes these
    bytes and not some other rendering of them."""
    return (line + "\n").encode()


def sha(blob):
    return hashlib.sha512(blob).hexdigest()


def read(path):
    with open(path, "rb") as f:
        return f.read()


def text(blob):
    """A config as you would read it -- short enough that every problem below
    names the config itself instead of the hash it happens to live under."""
    return blob.decode().strip()


def render(line):
    """Run the binary on this config in a scratch directory. The config carries
    no output key, so the default sink writes one SVG under ./out and prints the
    path it wrote."""
    with tempfile.TemporaryDirectory() as d:
        cfg = os.path.join(d, "parameters.json")
        with open(cfg, "w") as f:
            f.write(line)
        out = bgsvg_run([cfg], cwd=d).strip()
        svg = read(os.path.join(d, out))
    parseString(svg)          # well-formed, not merely unchanged
    return svg


def corpus():
    """{JSON hash: (JSON bytes, SVG bytes)} -- the corpus as it should exist."""
    out, n = {}, 0
    for line in configs():
        blob = canon(line)
        out[sha(blob)] = (blob, render(line))
        n += 1
    assert len(out) == n, "two configs serialised to the same JSON"
    return out


def first_diff(old, new):
    """Where two SVGs first part company. A line diff says nothing here -- the
    whole document is one line -- so point at the byte and quote around it."""
    i = next((i for i, (a, b) in enumerate(zip(old, new)) if a != b),
             min(len(old), len(new)))
    window = slice(max(0, i - 30), i + 40)
    return (f"      first differs at byte {i}, {len(old)} -> {len(new)} bytes\n"
            f"      was ...{text(old[window])}...\n"
            f"      now ...{text(new[window])}...")


def scan():
    """Read the corpus off disk -> ({JSON hash: (dir, JSON bytes, SVG bytes)},
    problems). Enforces the shape here so the comparison below only has to care
    about content: one directory per render, holding one config and the one SVG
    it renders, every file named by its own hash."""
    found, bad = {}, []
    if not os.path.isdir(ROOT):
        return found, [f"{os.path.relpath(ROOT)} does not exist; run --regen"]

    for entry in sorted(os.listdir(ROOT)):
        d = os.path.join(ROOT, entry)
        tag = entry[:16] + "..."
        if not os.path.isdir(d):
            bad.append(f"{entry}: stray file in the corpus root")
            continue
        names = sorted(os.listdir(d))
        blobs = {n: read(os.path.join(d, n)) for n in names
                 if os.path.isfile(os.path.join(d, n))}
        jsons = [n for n in names if n.endswith(JSON_SUFFIX)]
        svgs = [n for n in names if n.endswith(SVG_SUFFIX)]

        for n in names:
            if n not in jsons and n not in svgs:
                bad.append(f"{tag}/{n}: not a golden file")
            elif n.rsplit("_", 1)[0] != sha(blobs[n]):
                bad.append(f"{tag}/{n[:16]}...: contents do not hash to its own "
                           "name (edited by hand?)")
        if len(svgs) != 1:
            bad.append(f"{tag}: holds {len(svgs)} SVGs, expected the 1 it is named after")
        elif svgs[0][:-len(SVG_SUFFIX)] != entry:
            bad.append(f"{tag}: the SVG here does not hash to this directory")
        if len(jsons) > 1:
            bad.append(f"{tag}: {len(jsons)} configs render this one SVG, so an axis "
                       "stopped changing the picture: "
                       + " ".join(text(blobs[n]) for n in jsons))
        elif not jsons:
            bad.append(f"{tag}: holds no config")

        svg = blobs[svgs[0]] if len(svgs) == 1 else None
        for n in jsons:
            found[n[:-len(JSON_SUFFIX)]] = (entry, blobs[n], svg)
    return found, bad


def verify():
    want = corpus()
    found, bad = scan()

    for json_hash, (json_blob, svg_blob) in sorted(want.items(), key=lambda kv: kv[1][0]):
        if json_hash not in found:
            bad.append(f"missing: {text(json_blob)}")
            continue
        entry, _, stored = found[json_hash]
        if stored is not None and stored != svg_blob:
            bad.append(f"changed: {text(json_blob)}\n{first_diff(stored, svg_blob)}")
        elif entry != sha(svg_blob):
            bad.append(f"changed: {text(json_blob)}\n      "
                       f"{entry[:16]}... -> {sha(svg_blob)[:16]}...")
    for json_hash, (_, json_blob, _) in sorted(found.items(), key=lambda kv: kv[1][1]):
        if json_hash not in want:
            bad.append(f"stale: {text(json_blob)}")

    if bad:
        print(f"golden: {len(bad)} problem(s)", file=sys.stderr)
        for line in bad:
            print(f"  {line}", file=sys.stderr)
        print("\nif the picture was meant to change: python3 test/golden.py --regen",
              file=sys.stderr)
        return 1
    print(f"golden ok: {len(want)} configs render byte-identical SVGs at "
          f"{SIZE[0]}x{SIZE[1]}, and every file hashes to its own name")
    return 0


def regen():
    want = corpus()
    shutil.rmtree(ROOT, ignore_errors=True)
    for json_hash, (json_blob, svg_blob) in want.items():
        svg_hash = sha(svg_blob)
        d = os.path.join(ROOT, svg_hash)
        os.makedirs(d, exist_ok=True)
        for name, blob in ((json_hash + JSON_SUFFIX, json_blob),
                           (svg_hash + SVG_SUFFIX, svg_blob)):
            with open(os.path.join(d, name), "wb") as f:
                f.write(blob)
    print(f"regenerated {len(want)} goldens under {os.path.relpath(ROOT)}")
    return verify()   # a fresh corpus still has to pass every check (~1s)


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--regen", action="store_true",
                    help="rewrite the corpus after an intended visual change")
    sys.exit(regen() if ap.parse_args().regen else verify())

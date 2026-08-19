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

`--selftest` asserts the invariants a render must satisfy; this asserts the
render did not change at all. Keeping the SVG rather than only its hash is
what lets a failure say *what* moved instead of just *that* something did.

    nix develop -c python3 test/golden.py            # verify
    nix develop -c python3 test/golden.py --regen    # rewrite after an intended change
"""
import argparse
import hashlib
import json
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import background as bg          # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "golden")
JSON_SUFFIX = "_parameters.json"
SVG_SUFFIX = "_background.svg"
SIZE = (1920, 1080)              # the schema default; the goldens carry no output key
# seed 0 and no output key on purpose: geometry depends only on the seed, and
# the sink picks a destination rather than pixels, so holding both fixed keeps
# the corpus to the axes that actually change the SVG.
SEED = 0


def canon(obj):
    """The bytes that land on disk. Sorted and minified so one message can only
    ever hash one way; enum names written out in full rather than elided as
    proto3 defaults, so a golden still says what it tests when you read it. One
    trailing newline keeps it a POSIX text file without breaking the name, since
    the name hashes these bytes and not some other rendering of them."""
    return (json.dumps(obj, sort_keys=True, separators=(",", ":")) + "\n").encode()


def sha(blob):
    return hashlib.sha512(blob).hexdigest()


def read(path):
    with open(path, "rb") as f:
        return f.read()


def text(blob):
    """A config as you would read it -- short enough that every problem below
    names the config itself instead of the hash it happens to live under."""
    return blob.decode().strip()


def render(obj):
    return bg.build_svg(*SIZE, **bg.resolve(bg.validate(bg.parse(obj)))).encode()


def corpus():
    """{JSON hash: (JSON bytes, SVG bytes)} -- the corpus as it should exist.

    Keyed by the JSON hash because that, not the path, is a config's identity:
    a renderer change moves a config to a new directory, and matching on the
    key is what lets verify() name the one config that changed rather than a
    missing file plus an unrelated stale one. The SVG hash is not stored: it is
    sha(SVG bytes), and keeping a second copy of a derivable value is how the
    two drift apart."""
    out, n = {}, 0
    for cfg in bg.valid_configs(SEED):
        blob = canon(cfg)
        out[sha(blob)] = (blob, render(cfg))
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

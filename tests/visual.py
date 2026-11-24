#!/usr/bin/env python3
"""Render every tests/*.md fixture through bin/md2pdf and compare the pages
against the committed images in tests/golden/.

The engine's unit tests check the Typst markup it emits; only a real render
shows what that markup looks like on the page. Fonts are hermetic
(--no-system-fonts) so the output does not depend on the machine.

    python3 tests/visual.py            # compare; diffs go to tests/golden-diff/
    python3 tests/visual.py --update   # rewrite the goldens
    python3 tests/visual.py cover      # only some fixtures

Needs typst (see bin/md2pdf for the version), pdftoppm (poppler) and Pillow.
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parent.parent
TESTS = ROOT / "tests"
GOLDEN = TESTS / "golden"
DIFF = TESTS / "golden-diff"
DPI = 50
# Renders are pixel-identical across machines and poppler versions, so the
# tolerances are tight: a single changed word fails the page.
CHANNEL_TOLERANCE = 16
PAGE_TOLERANCE = 0.0001


def render(fixture, workdir, env):
    pdf = workdir / (fixture.stem + ".pdf")
    done = subprocess.run(
        [sys.executable, str(ROOT / "bin" / "md2pdf"), "--no-system-fonts", str(fixture), str(pdf)],
        capture_output=True,
        text=True,
        env=env,
    )
    if done.returncode != 0:
        raise RuntimeError(done.stderr.strip() or "md2pdf failed")
    prefix = workdir / fixture.stem
    subprocess.run(["pdftoppm", "-r", str(DPI), "-png", str(pdf), str(prefix)], check=True)
    # pdftoppm pads page numbers to the width of the page count.
    return sorted(workdir.glob(fixture.stem + "-*.png"), key=lambda p: int(p.stem.rsplit("-", 1)[1]))


def changed_share(a, b):
    if a.size != b.size:
        return 1.0
    diff = ImageChops.difference(a.convert("RGB"), b.convert("RGB"))
    mask = diff.convert("L").point(lambda v: 255 if v > CHANNEL_TOLERANCE else 0)
    return mask.histogram()[255] / (a.size[0] * a.size[1])


def save_diff(name, page, expected, actual):
    DIFF.mkdir(exist_ok=True)
    expected.save(DIFF / ("%s-%d-expected.png" % (name, page)))
    actual.save(DIFF / ("%s-%d-actual.png" % (name, page)))
    if expected.size == actual.size:
        ImageChops.difference(expected.convert("RGB"), actual.convert("RGB")).save(
            DIFF / ("%s-%d-diff.png" % (name, page))
        )


def compare(name, pages):
    goldens = sorted(GOLDEN.glob(name + "-*.png"), key=lambda p: int(p.stem.rsplit("-", 1)[1]))
    problems = []
    if len(goldens) != len(pages):
        problems.append("%d page(s), golden has %d" % (len(pages), len(goldens)))
    for number, (page, golden) in enumerate(zip(pages, goldens), start=1):
        actual, expected = Image.open(page), Image.open(golden)
        share = changed_share(actual, expected)
        if share > PAGE_TOLERANCE:
            problems.append("page %d differs (%.2f%% of pixels)" % (number, share * 100))
            save_diff(name, number, expected, actual)
    return problems


def update(name, pages):
    GOLDEN.mkdir(exist_ok=True)
    for stale in GOLDEN.glob(name + "-*.png"):
        stale.unlink()
    for number, page in enumerate(pages, start=1):
        shutil.copyfile(page, GOLDEN / ("%s-%d.png" % (name, number)))


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--update", action="store_true", help="rewrite the goldens")
    parser.add_argument("only", nargs="*", help="fixture names (default: all)")
    args = parser.parse_args()

    fixtures = sorted(TESTS.glob("*.md"))
    if args.only:
        fixtures = [f for f in fixtures if f.stem in args.only]
    shutil.rmtree(DIFF, ignore_errors=True)

    failures = 0
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        # Point Typst at the package in this checkout instead of an installed copy.
        packages = tmp / "packages"
        (packages / "local" / "md2pdf").mkdir(parents=True)
        os.symlink(ROOT / "package", packages / "local" / "md2pdf" / "0.1.0")
        # Remote images must fail the same way everywhere, or the goldens would
        # depend on the network: route every fetch to a closed local port.
        offline = {k: "http://127.0.0.1:9" for k in ("http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY")}
        env = dict(os.environ, TYPST_PACKAGE_PATH=str(packages), no_proxy="", NO_PROXY="", **offline)

        for fixture in fixtures:
            workdir = tmp / fixture.stem
            workdir.mkdir()
            try:
                pages = render(fixture, workdir, env)
            except RuntimeError as e:
                failures += 1
                print("FAIL %s: render failed\n%s" % (fixture.stem, e))
                continue
            if args.update:
                update(fixture.stem, pages)
                print("updated %s (%d page(s))" % (fixture.stem, len(pages)))
                continue
            problems = compare(fixture.stem, pages)
            if problems:
                failures += 1
                print("FAIL %s: %s" % (fixture.stem, "; ".join(problems)))
            else:
                print("ok   %s" % fixture.stem)

    if failures:
        if not args.update:
            print("\n%d fixture(s) differ. Diffs: %s. Intended? Run with --update." % (failures, DIFF))
        raise SystemExit(1)


if __name__ == "__main__":
    main()

"""Extract the inline inputs of the C# snapshot tests in PlannerIssueTest.cs into Rust data.

Usage: python3 -I extract_issue_cases.py <PlannerIssueTest.cs> <out.rs>
"""

import re
import sys

METHODS = [
    "CanPlanCollinearConnectedCenters",
    "AllowsManyTurnsAroundAvoidLocations",
    "CanPlanSinglePumpjackSurrounded",
    "CanPlanTwoPumpjacksWithLotsOfAvoids",
]

NUM = r"(-?\d+(?:\.\d+)?)f?"


def method_body(text: str, name: str) -> str:
    start = text.index(f"public async Task {name}()")
    end = text.find("public ", start + 10)
    return text[start:end if end != -1 else len(text)]


def rust_f32(value: str) -> str:
    return value if "." in value else value + ".0"


def main() -> None:
    text = open(sys.argv[1], encoding="utf-8-sig").read()
    out = [
        "//! Inputs of the snapshot tests in `test/FactorioTools.Test/OilField/PlannerIssueTest.cs`,",
        "//! extracted from that file by a script so the numbers cannot drift by hand. Regenerate",
        "//! rather than edit if the C# test changes.",
        "",
        "/// One test: its name, the options preset, whether beacons are added, the pumpjack",
        "/// positions and the avoided locations.",
        "pub struct IssueCase {",
        "    pub name: &'static str,",
        "    pub preset: &'static str,",
        "    pub add_beacons: bool,",
        "    pub pumpjacks: &'static [(f32, f32)],",
        "    pub avoid: &'static [(f32, f32)],",
        "}",
        "",
        "pub const ISSUE_CASES: &[IssueCase] = &[",
    ]
    for name in METHODS:
        body = method_body(text, name)
        preset = re.search(r"OilFieldOptions\.(For\w+)", body).group(1)
        add_beacons = "options.AddBeacons = false" not in body
        assert "options.ValidateSolution = true" in body, name
        pumpjacks = re.findall(r"new Position \{ X = " + NUM + r", Y = " + NUM + r" \}", body)
        avoid = re.findall(r"new AvoidLocation\(" + NUM + r", " + NUM + r"\)", body)
        out.append("    IssueCase {")
        out.append(f'        name: "{name}",')
        out.append(f'        preset: "{preset}",')
        out.append(f"        add_beacons: {'true' if add_beacons else 'false'},")
        out.append("        pumpjacks: &[")
        out.extend(f"            ({rust_f32(x)}, {rust_f32(y)})," for x, y in pumpjacks)
        out.append("        ],")
        out.append("        avoid: &[")
        out.extend(f"            ({rust_f32(x)}, {rust_f32(y)})," for x, y in avoid)
        out.append("        ],")
        out.append("    },")
        print(f"{name}: {preset}, beacons={add_beacons}, {len(pumpjacks)} pumpjacks, {len(avoid)} avoid")
    out.append("];")
    open(sys.argv[2], "w").write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()

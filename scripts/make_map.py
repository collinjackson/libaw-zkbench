#!/usr/bin/env python3
"""Writes zkvm/workloads/map.json: a synthetic 21x19 two-player map (rotationally symmetric,
so neither seat is favoured) in the AWBW-style JSON libaw::awbw::state_from reads. The
workloads start from it, so no benchmark data comes from a real game.

  python3 -I scripts/make_map.py
"""
import json
from pathlib import Path

# Top half (rows 0-8) and the middle row; the bottom half is the top rotated 180 degrees with
# the armies swapped. O/P/Q are Orange Star city/base/HQ, X/Y/Z Blue Moon's.
TOP = [
    "~~~.f..m..n..f..m.~~~",
    "~~.QPO--n.f..m.n..f.~",
    "~.f.|..f..--n...t...~",
    "..m.|.n..m..|...f.m..",
    ".n..b----..f|.n..b...",
    "..f.|..m..n.|..m..f..",
    "f...|.f...m.-----..n.",
    "..n.|..n.f....f..m...",
    ".m..t...m..a..f..n.f.",
]
MIDDLE = "~..f.n..m.-.m..n.f..~"
SWAP = {"O": "X", "P": "Y", "Q": "Z", "X": "O", "Y": "P", "Z": "Q"}
ROWS = TOP + [MIDDLE] + ["".join(SWAP.get(c, c) for c in reversed(r)) for r in reversed(TOP)]
TERRAIN = {"~": 28, ".": 1, "m": 2, "f": 3, "-": 15, "|": 16}
BUILDING = {"n": (34, None), "b": (35, None), "a": (36, None), "t": (133, None),
            "O": (38, 101), "P": (39, 101), "Q": (42, 101), "X": (43, 102), "Y": (44, 102), "Z": (47, 102)}


def main():
    assert len(ROWS) == 19 and all(len(r) == 21 for r in ROWS), [len(r) for r in ROWS]
    assert MIDDLE == "".join(reversed(MIDDLE)) and not set(MIDDLE) & set(SWAP)
    w, h = 21, 19
    terrain = [[{"tiles_x": x, "tiles_y": y, "terrain_id": TERRAIN.get(ROWS[y][x], BUILDING.get(ROWS[y][x], (1,))[0])}
                for y in range(h)] for x in range(w)]
    buildings = [[{"buildings_x": x, "buildings_y": y, "terrain_id": BUILDING[ROWS[y][x]][0],
                   "buildings_players_id": BUILDING[ROWS[y][x]][1], "buildings_capture": 20}
                  for y in range(h) if ROWS[y][x] in BUILDING] for x in range(w)]

    def player(pid, order, co, country, code):
        return {"players_id": pid, "players_team": str(pid), "co_name": co, "players_order": order,
                "players_funds": 0, "players_income": 0, "players_eliminated": "N", "players_co_power_on": "N",
                "players_co_power": 0, "players_co_max_power": 270000, "players_co_max_spower": 540000,
                "countries_name": country, "countries_code": code, "users_username": f"Player {order}"}

    m = {"day": 1, "libaw": {"map": "zkbench-21x19", "license": "CC0", "rows": ROWS},
         "gameState": {"terrain": terrain, "buildings": buildings, "units": [],
                       "players": [player(101, 1, "Andy", "Orange Star", "os"), player(102, 2, "Grimm", "Blue Moon", "bm")],
                       "gameWeather": {"code": "C"}, "currentTurnPId": 101}}
    out = Path(__file__).resolve().parent.parent / "zkvm/workloads/map.json"
    out.write_text(json.dumps(m))
    print("\n".join(ROWS))


main()

# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Plugin Lane Declarations and Admission Fixtures."""

# === Imports === #

import unittest

from google.protobuf.empty_pb2 import Empty

from harmos import VISIBILITY_PUBLIC
from harmos.plugin import Catalog, Context, Lane, Plugin, operation

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class Lanes(unittest.TestCase):
    def test_lane_capacity_encoding(self) -> None:
        declarations = [Lane("instrument", serial=True), Lane("parallel", concurrent=2), Lane("open")]
        expected = ["0a0a696e737472756d656e741001", "0a08706172616c6c656c1802", "0a046f70656e2001"]
        self.assertEqual([lane.declaration().SerializeToString().hex() for lane in declarations], expected)

    def test_invalid_names_and_capacity(self) -> None:
        for name in ("", "Open", "harmos.resident", "bad\nname"):
            self.assertRaises(ValueError, Lane, name)
        self.assertRaises(ValueError, Lane, "instrument", concurrent=0)

    def test_duplicate_and_undeclared_lanes(self) -> None:
        instrument = Lane("instrument", serial=True)

        class Author:
            @operation("example.measure", request=Empty, response=Empty, visibility=VISIBILITY_PUBLIC, lane=instrument)
            async def measure(self, _: Context, request: Empty) -> Empty:
                return request

        self.assertRaises(
            ValueError, Plugin(id="example", version="1.0.0", register=lambda ctx: ctx.register(Catalog(Author()))).declarations
        )
        self.assertRaises(
            ValueError,
            Plugin(
                id="example", version="1.0.0", register=lambda ctx: ctx.register(Catalog(Author())), lanes=[instrument, instrument]
            ).declarations,
        )
        declarations = Plugin(
            id="example", version="1.0.0", register=lambda ctx: ctx.register(Catalog(Author())), lanes=[instrument]
        ).declarations()
        self.assertEqual(declarations.operations[0].lane, "instrument")


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #

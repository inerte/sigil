#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import pathlib
import sys
import unittest


MODULE_PATH = pathlib.Path(__file__).with_name("checkCanonicalStdlibUsage.py")
SPEC = importlib.util.spec_from_file_location("checkCanonicalStdlibUsage", MODULE_PATH)
assert SPEC is not None
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class CanonicalStdlibUsageTests(unittest.TestCase):
    def test_sum_duplicate_rejected(self) -> None:
        issues = MODULE.analyze_file("language/examples/example.sigil", "λsum(xs:[Int])=>Int=0\n")
        self.assertEqual([issue.replacement for issue in issues], ["stdlib::list.sum"])

    def test_reverse_duplicate_rejected(self) -> None:
        issues = MODULE.analyze_file(
            "language/examples/example.sigil",
            "λreverse[T](xs:[T])=>[T]=[]\n",
        )
        self.assertEqual([issue.replacement for issue in issues], ["stdlib::list.reverse"])

    def test_numeric_max_duplicate_rejected(self) -> None:
        issues = MODULE.analyze_file(
            "projects/demo.sigil",
            "λmax(a:Int,b:Int)=>Int=a\n",
        )
        self.assertEqual([issue.replacement for issue in issues], ["stdlib::numeric.max"])

    def test_string_take_duplicate_rejected(self) -> None:
        issues = MODULE.analyze_file(
            "projects/demo.sigil",
            "λtake(n:Int,s:String)=>String=s\n",
        )
        self.assertEqual([issue.replacement for issue in issues], ["stdlib::string.take"])

    def test_name_match_with_different_shape_allowed(self) -> None:
        issues = MODULE.analyze_file(
            "language/test-fixtures/example.sigil",
            "λsum(acc:Int,x:Int)=>Int=acc+x\n",
        )
        self.assertEqual(issues, [])

    def test_stdlib_path_is_ignored(self) -> None:
        issues = MODULE.analyze_file(
            "language/stdlib/list.lib.sigil",
            "λsum(xs:[Int])=>Int=0\n",
        )
        self.assertEqual(issues, [])


if __name__ == "__main__":
    unittest.main()

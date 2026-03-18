#!/usr/bin/env python3
from __future__ import annotations

import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Rule:
    name: str
    pattern: re.Pattern[str]
    replacement: str


@dataclass(frozen=True)
class Issue:
    path: str
    line: int
    name: str
    replacement: str


LOWER = r"[a-z][A-Za-z0-9]*"
UPPER = r"[A-Z][A-Za-z0-9]*"
END = r"(?==| match)"


RULES: tuple[Rule, ...] = (
    Rule("abs", re.compile(rf"^λabs\(({LOWER}):Int\)=>Int{END}", re.M), "stdlib::numeric.abs"),
    Rule("all", re.compile(rf"^λall\[(?P<t>{UPPER})\]\(({LOWER}):λ\((?P=t)\)=>Bool,{LOWER}:\[(?P=t)\]\)=>Bool{END}", re.M), "stdlib::list.all"),
    Rule("any", re.compile(rf"^λany\[(?P<t>{UPPER})\]\(({LOWER}):λ\((?P=t)\)=>Bool,{LOWER}:\[(?P=t)\]\)=>Bool{END}", re.M), "stdlib::list.any"),
    Rule("charAt", re.compile(rf"^λcharAt\(({LOWER}):Int,{LOWER}:String\)=>String{END}", re.M), "stdlib::string.charAt"),
    Rule("clamp", re.compile(rf"^λclamp\(({LOWER}):Int,{LOWER}:Int,{LOWER}:Int\)=>Int{END}", re.M), "stdlib::numeric.clamp"),
    Rule("contains", re.compile(rf"^λcontains\[(?P<t>{UPPER})\]\(({LOWER}):(?P=t),{LOWER}:\[(?P=t)\]\)=>Bool{END}", re.M), "stdlib::list.contains"),
    Rule("count", re.compile(rf"^λcount\[(?P<t>{UPPER})\]\(({LOWER}):(?P=t),{LOWER}:\[(?P=t)\]\)=>Int{END}", re.M), "stdlib::list.count"),
    Rule("countIf", re.compile(rf"^λcountIf\[(?P<t>{UPPER})\]\(({LOWER}):λ\((?P=t)\)=>Bool,{LOWER}:\[(?P=t)\]\)=>Int{END}", re.M), "stdlib::list.countIf"),
    Rule("divisible", re.compile(rf"^λdivisible\(({LOWER}):Int,{LOWER}:Int\)=>Bool{END}", re.M), "stdlib::numeric.divisible"),
    Rule("divmod", re.compile(rf"^λdivmod\(({LOWER}):Int,{LOWER}:Int\)=>DivMod{END}", re.M), "stdlib::numeric.divmod"),
    Rule("dropList", re.compile(rf"^λdrop\[(?P<t>{UPPER})\]\(({LOWER}):Int,{LOWER}:\[(?P=t)\]\)=>\[(?P=t)\]{END}", re.M), "stdlib::list.drop"),
    Rule("dropString", re.compile(rf"^λdrop\(({LOWER}):Int,{LOWER}:String\)=>String{END}", re.M), "stdlib::string.drop"),
    Rule("endsWith", re.compile(rf"^λendsWith\(({LOWER}):String,{LOWER}:String\)=>Bool{END}", re.M), "stdlib::string.endsWith"),
    Rule("find", re.compile(rf"^λfind\[(?P<t>{UPPER})\]\(({LOWER}):λ\((?P=t)\)=>Bool,{LOWER}:\[(?P=t)\]\)=>Option\[(?P=t)\]{END}", re.M), "stdlib::list.find"),
    Rule("flatMap", re.compile(rf"^λflatMap\[(?P<t>{UPPER}),(?P<u>{UPPER})\]\(({LOWER}):λ\((?P=t)\)=>\[(?P=u)\],{LOWER}:\[(?P=t)\]\)=>\[(?P=u)\]{END}", re.M), "stdlib::list.flatMap"),
    Rule("gcd", re.compile(rf"^λgcd\(({LOWER}):Int,{LOWER}:Int\)=>Int{END}", re.M), "stdlib::numeric.gcd"),
    Rule("inBounds", re.compile(rf"^λinBounds\[(?P<t>{UPPER})\]\(({LOWER}):Int,{LOWER}:\[(?P=t)\]\)=>Bool{END}", re.M), "stdlib::list.inBounds"),
    Rule("inRange", re.compile(rf"^λinRange\(({LOWER}):Int,{LOWER}:Int,{LOWER}:Int\)=>Bool{END}", re.M), "stdlib::numeric.inRange"),
    Rule("indexOf", re.compile(rf"^λindexOf\(({LOWER}):String,{LOWER}:String\)=>Int{END}", re.M), "stdlib::string.indexOf"),
    Rule("intToString", re.compile(rf"^λintToString\(({LOWER}):Int\)=>String{END}", re.M), "stdlib::string.intToString"),
    Rule("isDigit", re.compile(rf"^λisDigit\(({LOWER}):String\)=>Bool{END}", re.M), "stdlib::string.isDigit"),
    Rule("isEven", re.compile(rf"^λisEven\(({LOWER}):Int\)=>Bool{END}", re.M), "stdlib::numeric.isEven"),
    Rule("isNegative", re.compile(rf"^λisNegative\(({LOWER}):Int\)=>Bool{END}", re.M), "stdlib::numeric.isNegative"),
    Rule("isNonNegative", re.compile(rf"^λisNonNegative\(({LOWER}):Int\)=>Bool{END}", re.M), "stdlib::numeric.isNonNegative"),
    Rule("isOdd", re.compile(rf"^λisOdd\(({LOWER}):Int\)=>Bool{END}", re.M), "stdlib::numeric.isOdd"),
    Rule("isPositive", re.compile(rf"^λisPositive\(({LOWER}):Int\)=>Bool{END}", re.M), "stdlib::numeric.isPositive"),
    Rule("join", re.compile(rf"^λjoin\(({LOWER}):String,{LOWER}:\[String\]\)=>String{END}", re.M), "stdlib::string.join"),
    Rule("last", re.compile(rf"^λlast\[(?P<t>{UPPER})\]\(({LOWER}):\[(?P=t)\]\)=>Option\[(?P=t)\]{END}", re.M), "stdlib::list.last"),
    Rule("lcm", re.compile(rf"^λlcm\(({LOWER}):Int,{LOWER}:Int\)=>Int{END}", re.M), "stdlib::numeric.lcm"),
    Rule("lines", re.compile(rf"^λlines\(({LOWER}):String\)=>\[String\]{END}", re.M), "stdlib::string.lines"),
    Rule("maxList", re.compile(rf"^λmax\(({LOWER}):\[Int\]\)=>Option\[Int\]{END}", re.M), "stdlib::list.max"),
    Rule("maxNumeric", re.compile(rf"^λmax\(({LOWER}):Int,{LOWER}:Int\)=>Int{END}", re.M), "stdlib::numeric.max"),
    Rule("minList", re.compile(rf"^λmin\(({LOWER}):\[Int\]\)=>Option\[Int\]{END}", re.M), "stdlib::list.min"),
    Rule("minNumeric", re.compile(rf"^λmin\(({LOWER}):Int,{LOWER}:Int\)=>Int{END}", re.M), "stdlib::numeric.min"),
    Rule("mod", re.compile(rf"^λmod\(({LOWER}):Int,{LOWER}:Int\)=>Int{END}", re.M), "stdlib::numeric.mod"),
    Rule("nth", re.compile(rf"^λnth\[(?P<t>{UPPER})\]\(({LOWER}):Int,{LOWER}:\[(?P=t)\]\)=>Option\[(?P=t)\]{END}", re.M), "stdlib::list.nth"),
    Rule("pow", re.compile(rf"^λpow\(({LOWER}):Int,{LOWER}:Int\)=>Int{END}", re.M), "stdlib::numeric.pow"),
    Rule("product", re.compile(rf"^λproduct\(({LOWER}):\[Int\]\)=>Int{END}", re.M), "stdlib::list.product"),
    Rule("range", re.compile(rf"^λrange\(({LOWER}):Int,{LOWER}:Int\)=>\[Int\]{END}", re.M), "stdlib::numeric.range"),
    Rule("removeFirst", re.compile(rf"^λremoveFirst\[(?P<t>{UPPER})\]\(({LOWER}):(?P=t),{LOWER}:\[(?P=t)\]\)=>\[(?P=t)\]{END}", re.M), "stdlib::list.removeFirst"),
    Rule("repeat", re.compile(rf"^λrepeat\(({LOWER}):Int,{LOWER}:String\)=>String{END}", re.M), "stdlib::string.repeat"),
    Rule("replaceAll", re.compile(rf"^λreplaceAll\(({LOWER}):String,{LOWER}:String,{LOWER}:String\)=>String{END}", re.M), "stdlib::string.replaceAll"),
    Rule("reverseList", re.compile(rf"^λreverse\[(?P<t>{UPPER})\]\(({LOWER}):\[(?P=t)\]\)=>\[(?P=t)\]{END}", re.M), "stdlib::list.reverse"),
    Rule("reverseString", re.compile(rf"^λreverse\(({LOWER}):String\)=>String{END}", re.M), "stdlib::string.reverse"),
    Rule("sign", re.compile(rf"^λsign\(({LOWER}):Int\)=>Int{END}", re.M), "stdlib::numeric.sign"),
    Rule("split", re.compile(rf"^λsplit\(({LOWER}):String,{LOWER}:String\)=>\[String\]{END}", re.M), "stdlib::string.split"),
    Rule("sortedAsc", re.compile(rf"^λsortedAsc\(({LOWER}):\[Int\]\)=>Bool{END}", re.M), "stdlib::list.sortedAsc"),
    Rule("sortedDesc", re.compile(rf"^λsortedDesc\(({LOWER}):\[Int\]\)=>Bool{END}", re.M), "stdlib::list.sortedDesc"),
    Rule("startsWith", re.compile(rf"^λstartsWith\(({LOWER}):String,{LOWER}:String\)=>Bool{END}", re.M), "stdlib::string.startsWith"),
    Rule("substring", re.compile(rf"^λsubstring\(({LOWER}):Int,{LOWER}:String,{LOWER}:Int\)=>String{END}", re.M), "stdlib::string.substring"),
    Rule("sum", re.compile(rf"^λsum\(({LOWER}):\[Int\]\)=>Int{END}", re.M), "stdlib::list.sum"),
    Rule("takeList", re.compile(rf"^λtake\[(?P<t>{UPPER})\]\(({LOWER}):Int,{LOWER}:\[(?P=t)\]\)=>\[(?P=t)\]{END}", re.M), "stdlib::list.take"),
    Rule("takeString", re.compile(rf"^λtake\(({LOWER}):Int,{LOWER}:String\)=>String{END}", re.M), "stdlib::string.take"),
    Rule("toLower", re.compile(rf"^λtoLower\(({LOWER}):String\)=>String{END}", re.M), "stdlib::string.toLower"),
    Rule("toUpper", re.compile(rf"^λtoUpper\(({LOWER}):String\)=>String{END}", re.M), "stdlib::string.toUpper"),
    Rule("trim", re.compile(rf"^λtrim\(({LOWER}):String\)=>String{END}", re.M), "stdlib::string.trim"),
    Rule("unlines", re.compile(rf"^λunlines\(({LOWER}):\[String\]\)=>String{END}", re.M), "stdlib::string.unlines"),
)


def tracked_sigil_files() -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "*.sigil", "*.lib.sigil"],
        capture_output=True,
        check=True,
        text=True,
    )
    return [line for line in result.stdout.splitlines() if line]


def analyze_file(path: str, source: str) -> list[Issue]:
    if path.startswith("language/stdlib/"):
        return []

    issues: list[Issue] = []
    for rule in RULES:
        for match in rule.pattern.finditer(source):
            line = source.count("\n", 0, match.start()) + 1
            issues.append(Issue(path=path, line=line, name=rule.name, replacement=rule.replacement))
    return issues


def main() -> int:
    issues: list[Issue] = []
    for path in tracked_sigil_files():
        source = Path(path).read_text()
        issues.extend(analyze_file(path, source))

    if not issues:
        print("Canonical stdlib usage OK")
        return 0

    print("Non-canonical local stdlib helper redefinitions found:")
    for issue in issues:
        print(
            f"{issue.path}:{issue.line}: local '{issue.name}' duplicates canonical helper "
            f"'{issue.replacement}'; use the qualified stdlib path instead."
        )
    return 1


if __name__ == "__main__":
    sys.exit(main())

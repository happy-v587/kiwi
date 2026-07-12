#!/usr/bin/env python3
# Copyright (c) 2024-present, arana-db Community.  All rights reserved.
#
# Licensed to the Apache Software Foundation (ASF) under one or more
# contributor license agreements.  See the NOTICE file distributed with
# this work for additional information regarding copyright ownership.
# The ASF licenses this file to You under the Apache License, Version 2.0
# (the "License"); you may not use this file except in compliance with
# the License.  You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""Check that client-visible error text is centralized in error-catalog."""

import re
import sys
from pathlib import Path

CATALOG_FILE = Path("src/common/error-catalog/src/lib.rs")

SCAN_DIRS = [
    Path("src/cmd/src"),
    Path("src/storage/src"),
    Path("src/net/src"),
    Path("src/common/runtime/src"),
    Path("src/client/src"),
]

# (name, regex)
PATTERNS = [
    # Any client-visible string literal starting with a Redis error class.
    # These match even when the literal is split across lines from the
    # surrounding `RespData::Error(...)` call.
    ("raw ERR string literal", r'"ERR\s'),
    ("raw WRONGTYPE string literal", r'"WRONGTYPE\s'),
    ("raw NOAUTH string literal", r'"NOAUTH\s'),
    ("raw WRONGPASS string literal", r'"WRONGPASS\s'),
    # format!/write! that prefixes an error class around dynamic content.
    (
        "format ERR/WRONGTYPE/NOAUTH/WRONGPASS wrapping",
        r'format!\s*\(\s*"(?:ERR|WRONGTYPE|NOAUTH|WRONGPASS)\s',
    ),
    # Storage-level RedisErr construction that hard-codes an error class.
    ("raw ERR in RedisErr message", r'RedisErr\s*\{\s*message\s*:\s*"ERR\s'),
    (
        "raw WRONGTYPE in RedisErr message",
        r'RedisErr\s*\{\s*message\s*:\s*"WRONGTYPE\s',
    ),
    # Dead response helpers removed by the unification work.
    ("CmdRes dead code", r'\bCmdRes::'),
    ("set_res dead code", r'\bset_res\s*\('),
]

# Files that are temporarily allowed to contain legacy error patterns while
# migration is in progress. Remove entries as they are migrated.
ALLOWLIST: set[Path] = set()


def _strip_line_comment(line: str) -> str:
    """Remove a trailing Rust line comment while preserving string literals."""
    result = []
    i = 0
    while i < len(line):
        if line[i : i + 2] == "//":
            break
        if line[i] == '"':
            # Copy the string literal as-is so that patterns inside it can match.
            result.append(line[i])
            i += 1
            while i < len(line):
                result.append(line[i])
                if line[i] == "\\":
                    i += 1
                    if i < len(line):
                        result.append(line[i])
                elif line[i] == '"':
                    i += 1
                    break
                i += 1
            continue
        result.append(line[i])
        i += 1
    return "".join(result)


def main() -> int:
    violations = []

    for dir_path in SCAN_DIRS:
        if not dir_path.exists():
            continue
        for file_path in dir_path.rglob("*.rs"):
            if file_path == CATALOG_FILE or file_path in ALLOWLIST:
                continue
            if "tests" in file_path.parts:
                continue

            content = file_path.read_text(encoding="utf-8")
            for name, pattern in PATTERNS:
                for match in re.finditer(pattern, content, re.MULTILINE):
                    line_num = content[: match.start()].count("\n") + 1
                    raw_line = content.splitlines()[line_num - 1]
                    # Skip doc comments; keep regular code comments intact so
                    # that patterns inside them are still linted.
                    if raw_line.lstrip().startswith("///"):
                        continue
                    cleaned = _strip_line_comment(raw_line)
                    # Re-check the pattern after stripping comments to avoid
                    # false positives inside trailing code comments.
                    if not re.search(pattern, cleaned, re.MULTILINE):
                        continue
                    violations.append(
                        f"{file_path}:{line_num}: [{name}] {raw_line.strip()}"
                    )

    if violations:
        print("ERROR: error-catalog violations found:")
        for v in violations:
            print(f"  {v}")
        print(
            "\nAll client-visible error text must come from the error-catalog crate."
        )
        print("See CLAUDE.md 'Error Handling' for the correct pattern.")
        return 1

    print("error-catalog check passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

#! /usr/bin/env python3

import re
import sys

for line in sys.stdin:
    m = re.search(r'startCharCode = (\d+), endCharCode = (\d+),', line)
    if m is not None:
        [start, end] = [int(s) for s in m.groups()]
        for i in range(start, end+1):
            print(i)

#!/usr/bin/env python3
import os
import signal
import sys

if len(sys.argv) < 2:
    sys.exit(2)
target = sys.argv[1]
# Why not leave Python's SIG_IGN: exec preserves it, so EPIPE never becomes 141.
signal.signal(signal.SIGPIPE, signal.SIG_DFL)
read_end, write_end = os.pipe()
os.close(read_end)
os.dup2(write_end, 2)
os.execv(target, [target])

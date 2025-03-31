import gc
import signal
import sys
import time

gc.disable()

program_name = sys.argv[1]
args = sys.argv[2]

args = map(int, args.strip().split())


def handler():
    print("nan")
    exit(0)


# Avoid hitting the recursion limit.
sys.setrecursionlimit(10000)
# Set handler if the program timeouts.
signal.signal(signal.SIGALRM, handler)

with open(program_name) as f:
    # Define `main` and related functions.
    exec(f.read())

try:
    # Timeout after 60s.
    signal.alarm(60)
    start_time = time.time()
    main(*args)
    end_time = time.time()
    signal.alarm(0)
    # Print time in nanoseconds.
    print(int((end_time - start_time) * 10**9))
except Exception as e:
    print("nan")

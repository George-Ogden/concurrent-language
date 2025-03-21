import signal
import sys
import time

program_name = sys.argv[1]
args = sys.argv[2]

args = map(int, args.strip().split())


def handler():
    print("nan")
    exit(0)


sys.setrecursionlimit(10000)
signal.signal(signal.SIGALRM, handler)

with open(program_name) as f:
    exec(f.read())

try:
    signal.alarm(60)
    start_time = time.time()
    main(*args)
    end_time = time.time()
    signal.alarm(0)
    print(int((end_time - start_time) * 10**9))
except Exception as e:
    print("nan")

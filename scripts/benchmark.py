import sys
import time

program_name = sys.argv[1]
args = sys.argv[2]

with open(program_name) as f:
    exec(f.read())

args = map(int, args.strip().split())

try:
    start_time = time.time()
    main(*args)
    end_time = time.time()
    print(int((end_time - start_time) * 10**9))
except Exception as e:
    print("nan")

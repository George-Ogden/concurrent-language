def fib(n):
    if n <= 1:
        if n < 0:
            return 0
        else:
            return 1
    else:
        return fib(n - 1) + fib(n - 2)


def main(n):
    fib(n)

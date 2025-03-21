def fib(n):
    if n <= 1:
        if n < 0:
            return 0
        else:
            return 1
    else:
        if (n & 1) == 0:
            fib_21 = fib(n // 2 - 1)
            fib_2 = fib(n // 2)
            return fib_21**2 + fib_2 * fib_21 + fib_2 * fib(n // 2 - 2)
        else:
            fib_2 = fib(n // 2)
            return fib_2**2 + fib_2 * fib(n // 2 - 1) * 2


def main(n):
    return fib(n)

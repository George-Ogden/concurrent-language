def fib(n):
    if n <= 1:
        if n < 0:
            return 0
        else:
            return 1
    else:

        def fib_inner(m):
            if m == 0:
                return (1, 1)
            else:
                f = fib_inner(m - 1)
                return (f[1], f[0] + f[1])

        return fib_inner(n - 1)[1]


def main(n):
    fib(n)

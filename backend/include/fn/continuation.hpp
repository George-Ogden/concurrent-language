#pragma once

#include <atomic>

struct Continuation {
    std::atomic<unsigned> &remaining;
    std::atomic<unsigned> &counter;
    Continuation(std::atomic<unsigned> &remaining,
                 std::atomic<unsigned> &counter)
        : remaining(remaining), counter(counter) {}
};

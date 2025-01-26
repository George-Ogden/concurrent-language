#pragma once

#include "data_structures/lock.hpp"

#include <atomic>
#include <memory>

struct Continuation {
    std::atomic<unsigned> *remaining;
    std::atomic<unsigned> &counter;
    Locked<bool> &valid;
    Continuation(std::atomic<unsigned> *remaining,
                 std::atomic<unsigned> &counter, Locked<bool> &valid)
        : remaining(remaining), counter(counter), valid(valid) {}
};

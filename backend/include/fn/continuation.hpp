#pragma once

#include "data_structures/lock.hpp"

#include <atomic>
#include <memory>

struct Continuation {
    std::atomic<unsigned> *remaining;
    std::atomic<unsigned> &counter;
    std::shared_ptr<Locked<bool>> valid;
    Continuation(std::atomic<unsigned> *remaining,
                 std::atomic<unsigned> &counter,
                 std::shared_ptr<Locked<bool>> valid)
        : remaining(remaining), counter(counter), valid(valid) {}
};

#pragma once

#include "data_structures/lock.tpp"

#include <atomic>

struct Continuation {
    std::atomic<unsigned> *remaining;
    std::atomic<unsigned> &counter;
    Locked<bool> *valid;
    Continuation(std::atomic<unsigned> *remaining,
                 std::atomic<unsigned> &counter, Locked<bool> *valid);
    void update();
};

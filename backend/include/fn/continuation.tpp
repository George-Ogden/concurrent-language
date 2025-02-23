#pragma once

#include "fn/continuation.hpp"
#include "data_structures/lock.tpp"

#include <atomic>
#include <memory>

Continuation::Continuation(std::atomic<unsigned> *remaining,
                           std::atomic<unsigned> &counter, Locked<bool> *valid)
    : remaining(remaining), counter(counter), valid(valid) {}

void Continuation::update() {
    if (remaining->fetch_sub(1, std::memory_order_relaxed) == 1) {
        delete remaining;
        valid->acquire();
        if (**valid) {
            **valid = false;
            counter.fetch_add(1, std::memory_order_relaxed);
            valid->release();
        } else {
            valid->release();
            delete valid;
        }
    }
}

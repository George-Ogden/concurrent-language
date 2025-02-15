#pragma once

#include "fn/continuation.hpp"
#include "data_structures/lock.tpp"

#include <atomic>
#include <memory>

Continuation::Continuation(std::atomic<unsigned> *remaining,
                           std::atomic<unsigned> &counter, Locked<bool> *valid)
    : remaining(remaining), counter(counter), valid(valid) {}

#pragma once

#include "data_structures/lazy.tpp"
#include "types/utils.hpp"
#include "fn/continuation.tpp"

#include <memory>

template <typename T>
Lazy<T>::Lazy() = default;

template <typename T>
template <typename ...Args>
Lazy<T>::Lazy(Args&&...args):_value(std::forward<Args>(args)...),done_flag(true){}

template <typename T>
bool Lazy<T>::done() const {
    return done_flag.load(std::memory_order_relaxed);
}

template <typename T>
T Lazy<T>::value() const {
    return _value;
}

template <typename T>
void Lazy<T>::update_continuation(Continuation c) {
    if (c.remaining->fetch_sub(1, std::memory_order_relaxed) == 1) {
        delete c.remaining;
        c.valid->acquire();
        if (**c.valid) {
            **c.valid = false;
            c.counter.fetch_add(1, std::memory_order_relaxed);
            c.valid->release();
        } else {
            c.valid->release();
            delete c.valid;
        }
    }
}

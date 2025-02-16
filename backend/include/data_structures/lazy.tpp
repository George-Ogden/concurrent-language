#pragma once

#include "data_structures/lazy.tpp"
#include "types/utils.hpp"
#include "fn/continuation.tpp"
#include "work/work.tpp"

#include <memory>

template <typename T>
Lazy<T>::Lazy() = default;

template <typename T>
template <typename ...Args>
Lazy<T>::Lazy(Args&&...args):_value(std::forward<Args>(args)...){}

template <typename T>
bool Lazy<T>::done() const {
    return work == nullptr || work->done();
}

template <typename T>
T Lazy<T>::value() const {
    return _value;
}

template <typename T>
void Lazy<T>::add_continuation(Continuation c) {
    if (work == nullptr){
        Work::update_continuation(c);
    } else {
        work->add_continuation(c);
    }
}

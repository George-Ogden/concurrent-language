#pragma once

#include "lazy/lazy.hpp"
#include "types/utils.hpp"
#include "fn/continuation.tpp"
#include "work/work.tpp"

#include <memory>

template <typename T>
Lazy<T>::~Lazy() = default;

template <typename T>
LazyWork<T>::LazyWork() = default;

template <typename T>
bool LazyWork<T>::done() const {
    return work == nullptr || work->done();
}

template <typename T>
T LazyWork<T>::value() const {
    return _value;
}

template <typename T>
void LazyWork<T>::add_continuation(Continuation c) {
    if (work == nullptr){
        c.update();
    } else {
        work->add_continuation(c);
    }
}

template <typename T>
template <typename ...Args>
LazyConstant<T>::LazyConstant(Args&&...args):_value(std::forward<Args>(args)...){}

template <typename T>
bool LazyConstant<T>::done() const {
    return true;
}

template <typename T>
T LazyConstant<T>::value() const {
    return _value;
}

template <typename T>
void LazyConstant<T>::add_continuation(Continuation c) {
    c.update();
}

template <typename T>
LazyPlaceholder<T>::LazyPlaceholder(std::shared_ptr<Work> work)
    : work(work) {}

template <typename T>
void LazyPlaceholder<T>::add_continuation(Continuation c) {
        continuations.acquire();
        if (reference == nullptr) {
            continuations->push_back(c);
            continuations.release();
        } else {
            continuations.release();
            reference->add_continuation(c);
        }

}

template <typename T>
void LazyPlaceholder<T>::assign(std::shared_ptr<Lazy<T>> value) {
        continuations.acquire();
        for (Continuation &c : *continuations) {
            value->add_continuation(c);
        }
        continuations->clear();
        reference = value;
        continuations.release();

}

template <typename T>
bool LazyPlaceholder<T>::done() const {
    return work->done() && reference->done();
}

template <typename T>
T LazyPlaceholder<T>::value() const {
    return reference->value();
}

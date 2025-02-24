#pragma once

#include "lazy/lazy.hpp"
#include "types/utils.hpp"
#include "fn/continuation.tpp"
#include "work/work.tpp"
#include "system/work_manager.tpp"

#include <optional>
#include <memory>

template <typename T>
Lazy<T>::~Lazy() = default;

template <typename T>
std::shared_ptr<Lazy<T>> Lazy<T>::as_ref() {
    return nullptr;
}

template <typename T>
void Lazy<T>::prioritize(){}

template <typename T>
template <typename ...Args>
LazyConstant<T>::LazyConstant(Args&&...args):_value(std::forward<Args>(args)...){}

template <typename T>
bool LazyConstant<T>::done() {
    return true;
}

template <typename T>
T LazyConstant<T>::value() {
    return _value;
}

template <typename T>
T& LazyConstant<T>::lvalue() {
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
    auto current_reference = reference.load(std::memory_order_relaxed);
    if (current_reference == nullptr) {
        continuations->push_back(c);
        continuations.release();
    } else {
        continuations.release();
        current_reference->add_continuation(c);
    }
}

template <typename T>
void LazyPlaceholder<T>::assign(std::shared_ptr<Lazy<T>> value) {
    if (required){
        value->prioritize();
    }
    continuations.acquire();
    for (Continuation &c : *continuations) {
        value->add_continuation(c);
    }
    continuations->clear();
    reference.store(value, std::memory_order_relaxed);
    work.store(nullptr, std::memory_order_relaxed);
    continuations.release();
}

template <typename T>
bool LazyPlaceholder<T>::done() {
    auto current_reference = this->as_ref();
    return current_reference != nullptr && current_reference->done();
}

template <typename T>
T LazyPlaceholder<T>::value() {
    return as_ref()->value();
}

template <typename T>
T& LazyPlaceholder<T>::lvalue() {
    return reference.load(std::memory_order_relaxed)->lvalue();
}

template <typename T>
std::shared_ptr<Lazy<T>> LazyPlaceholder<T>::as_ref() {
    auto current_reference = reference.load(std::memory_order_relaxed);
    if (current_reference == nullptr){
        return nullptr;
    }
    std::shared_ptr<Lazy<T>> lazy = current_reference->as_ref();
    if (lazy == nullptr){
        return current_reference;
    } else {
        reference.compare_exchange_weak(current_reference, lazy, std::memory_order_relaxed);
        return lazy;
    }
}

template <typename T>
void LazyPlaceholder<T>::prioritize() {
    required = true;
    auto current_reference = this->as_ref();
    if (current_reference == nullptr) {
        WorkT current_work = work.load(std::memory_order_relaxed);
        if (current_work != nullptr && current_work->status.require() && current_work->status.acquire()){
            WorkManager::enqueue(current_work);
        }
    } else {
        current_reference->prioritize();
    }
}

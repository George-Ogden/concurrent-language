#pragma once

#include "lazy/lazy.hpp"
#include "types/utils.hpp"
#include "work/work.tpp"
#include "system/work_manager.tpp"

#include <optional>
#include <memory>

LazyValue::~LazyValue() = default;


template <typename T>
std::shared_ptr<Lazy<T>> Lazy<T>::as_ref() {
    return nullptr;
}

template <typename T>
std::optional<WorkT> Lazy<T>::get_work() {
    return std::nullopt;
}

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
void LazyConstant<T>::enqueue() {}

template <typename T>
LazyPlaceholder<T>::LazyPlaceholder(std::shared_ptr<Work> work)
    : work(work) {}

template <typename T>
void LazyPlaceholder<T>::assign(std::shared_ptr<Lazy<T>> value) {
    reference.store(value, std::memory_order_relaxed);
    work.store(nullptr, std::memory_order_relaxed);
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
        // Shorten path to reference if possible.
        reference.compare_exchange_weak(current_reference, lazy, std::memory_order_relaxed);
        return lazy;
    }
}

template <typename T>
std::optional<WorkT> LazyPlaceholder<T>::get_work() {
    auto current_reference = this->as_ref();
    if (current_reference == nullptr) {
        WorkT current_work = this->work.load(std::memory_order_relaxed);
        if (current_work != nullptr && !current_work->done()){
            return current_work;
        }
    } else {
        return current_reference->get_work();
    }
    return std::nullopt;
}

template <typename T>
void LazyPlaceholder<T>::enqueue() {
    std::optional<WorkT> work = get_work();
    if (work.has_value()){
        WorkManager::enqueue(work.value());
    }
}

#pragma once

#include "data_structures/lazy.hpp"
#include "data_structures/lock.tpp"
#include "fn/continuation.tpp"
#include "types/utils.hpp"

#include <memory>

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

template <typename T>
template <typename U>
auto Lazy<T>::extract_value(const U &v) {
    return v;
}

template <typename T>
template <typename... Us>
auto Lazy<T>::extract_value(const std::tuple<Us...> &v) {
    return std::apply(
        [](const auto &...args) {
            return std::make_tuple(extract_value(args)...);
        },
        v);
}

template <typename T>
template <typename U>
auto Lazy<T>::extract_value(const std::shared_ptr<Lazy<U>> &v) {
    return v->value();
}

template <typename T>
template <typename U>
requires is_shared_ptr_v<U>
LazyT<T> Lazy<T>::make_placeholders(U fn) {
    if constexpr (is_lazy_v<T>) {
        return std::make_shared<
            LazyPlaceholder<remove_lazy_t<T>, typename U::element_type>>(
            fn);
    } else if constexpr (is_tuple_v<T>) {
        return std::apply(
            [fn](auto &&...args) {
                return std::make_tuple(
                    Lazy<std::decay_t<decltype(args)>>::make_placeholders(
                        fn)...);
            },
            T{});
    } else {
        return nullptr;
    }
}

template <typename T>
void LazyPlaceholderBase<T>::add_continuation(Continuation c) {
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
void LazyPlaceholderBase<T>::assign(LazyT<T> value) {
    continuations.acquire();
    for (Continuation &c : *continuations) {
        value->add_continuation(c);
    }
    continuations->clear();
    reference = value;
    continuations.release();
}

template <typename T>
bool LazyPlaceholderBase<T>::done() const {
    return reference != nullptr && reference->done();
}

template <typename T>
T LazyPlaceholderBase<T>::value() const {
    return reference->value();
}

template <typename T, typename U>
LazyPlaceholder<T, U>::LazyPlaceholder(std::shared_ptr<U> reference)
    : reference(reference) {}

template <typename T>
LazyConstant<T>::LazyConstant() : data() {}

template <typename T>
LazyConstant<T>::LazyConstant(const T &data) : data(data) {}

template <typename T>
LazyConstant<T>::LazyConstant(T &&data) : data(std::forward<T>(data)) {}

template <typename T>
template <std::size_t Index>
LazyConstant<T>::LazyConstant(std::integral_constant<std::size_t, Index>)
    : data(std::integral_constant<std::size_t, Index>{}) {}

template <typename T>
template <std::size_t Index, typename U>
LazyConstant<T>::LazyConstant(std::integral_constant<std::size_t, Index>, U &&value)
    : data(std::integral_constant<std::size_t, Index>{}, std::forward<U>(value)) {}

template <typename T>
bool LazyConstant<T>::done() const {
    return true;
}

template <typename T>
T LazyConstant<T>::value() const {
    return data;
}

template <typename T>
void LazyConstant<T>::add_continuation(Continuation c) {
    Lazy<T>::update_continuation(c);
}

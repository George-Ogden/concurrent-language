#pragma once

#include "fn/continuation.hpp"

#include <type_traits>

template <typename T> struct Lazy {
    virtual bool done() const = 0;
    virtual T value() = 0;
    virtual void add_continuation(Continuation c) = 0;
    virtual ~Lazy(){};
    void update_continuation(Continuation c) {
        if (c.remaining.fetch_sub(1, std::memory_order_relaxed) == 1) {
            c.counter.fetch_add(1, std::memory_order_relaxed);
        }
    }
};

template <typename T> class LazyConstant : public Lazy<T> {
    const T data;

  public:
    // cppcheck-suppress noExplicitConstructor
    LazyConstant(const T &data) : data(data) {}
    bool done() const override { return true; }
    T value() override { return data; }
    void add_continuation(Continuation c) override {
        Lazy<T>::update_continuation(c);
    }
};

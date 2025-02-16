#pragma once

#include "fn/continuation.hpp"
#include "types/utils.hpp"

#include <atomic>
#include <utility>

template <typename T> class Lazy {
    T _value;
    std::atomic<bool> done_flag;

  public:
    Lazy();
    template <typename... Args> Lazy(Args &&...);
    bool done() const;
    T value() const;
    static void update_continuation(Continuation c);
};

template <typename T, typename... Args> LazyT<T> make_lazy(Args &&...args) {
    return std::make_shared<Lazy<T>>(std::forward<Args>(args)...);
}

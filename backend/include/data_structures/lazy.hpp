#pragma once

#include "fn/continuation.hpp"
#include "types/utils.hpp"

#include <utility>

struct Work;
template <typename T> class Lazy {
    friend class Work;
    T _value;
    std::shared_ptr<Work> work;

  public:
    Lazy();
    template <typename... Args> Lazy(Args &&...);
    bool done() const;
    T value() const;
    void add_continuation(Continuation c);
};

template <typename T, typename... Args> LazyT<T> make_lazy(Args &&...args) {
    return std::make_shared<Lazy<T>>(std::forward<Args>(args)...);
}

#pragma once

#include "lazy/lazy.tpp"

#include <memory>

LazyT<Int> inc(LazyT<Int> x, std::shared_ptr<void> env = nullptr) {
    return make_lazy<Int>(x->value() + 1);
}

FnT<Int, Int> inc_fn{inc};

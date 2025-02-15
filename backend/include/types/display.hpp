#pragma once

#include "types/compound.tpp"
#include "types/utils.hpp"

#include <iostream>
#include <memory>
#include <type_traits>

template <typename... Args>
std::ostream &operator<<(std::ostream &os, TupleT<Args...> const &t) {
    bool first = true;
    os << '(';
    apply(
        [&](auto &&...args) {
            ((os << (first ? "" : ", ") << args, first = false), ...);
        },
        t);
    os << ')';
    return os;
}

template <typename T, typename = std::enable_if_t<is_lazy_v<T>>>
std::ostream &operator<<(std::ostream &os, T const &t) {
    os << t->value();
    return os;
}

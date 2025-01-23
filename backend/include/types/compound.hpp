#pragma once

#include <iostream>
#include <tuple>
#include <type_traits>

#include "fn/fn.hpp"

template <typename... Ts> using TupleT = std::tuple<Ts...>;

template <typename... Types> struct VariantT {
    static_assert(sizeof...(Types) > 0, "VariantT must have at least one type");

    using TagType =
        std::conditional_t<(sizeof...(Types) <= 256), std::uint8_t,
                           std::conditional_t<(sizeof...(Types) <= 65536),
                                              std::uint16_t, std::uint32_t>>;
    TagType tag;
    std::aligned_union_t<0, Types...> value;

    VariantT() = default;

    friend std::ostream &operator<<(std::ostream &os,
                                    const VariantT<Types...> &variant) {
        os << '[' << static_cast<Int>(variant.tag) << "; ";
        [&]<std::size_t... I>(std::index_sequence<I...>) {
            ((variant.tag == I
                  ? (os << reinterpret_cast<const std::tuple_element_t<
                               I, std::tuple<Types...>> *>(&variant.value)
                               ->value,
                     void())
                  : void()),
             ...);
            if ((variant.tag >= sizeof...(I))) {
                os << "unknown";
            }
        }
        (std::make_index_sequence<sizeof...(Types)>());
        os << ']';
        return os;
    }
};

template <typename R, typename... As> using FnT = ParametricFn<R, As...> *;

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

template <typename T, typename = std::enable_if_t<std::is_pointer_v<T>>>
std::ostream &operator<<(std::ostream &os, T const &t) {
    os << *t;
    return os;
}

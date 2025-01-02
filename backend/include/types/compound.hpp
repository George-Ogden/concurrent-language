#pragma once

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
};

template <typename R, typename... As> using FnT = ParametricFn<R, As...> *;

#pragma once

#include <cstdint>
#include <type_traits>

typedef int64_t Int;
typedef bool Bool;
template <typename... Ts> using Tuple = std::tuple<Ts...>;

template <typename... Types> struct Variant {
    static_assert(sizeof...(Types) > 0, "Variant must have at least one type");

    using TagType =
        std::conditional_t<(sizeof...(Types) <= 256), std::uint8_t,
                           std::conditional_t<(sizeof...(Types) <= 65536),
                                              std::uint16_t, std::uint32_t>>;
    TagType tag;
    std::aligned_union_t<0, Types...> value;

    Variant() = default;
};

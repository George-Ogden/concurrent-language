#pragma once

#include "fn/fn.hpp"

#include <iostream>
#include <memory>
#include <new>
#include <tuple>
#include <type_traits>

template <typename... Ts> using TupleT = std::tuple<Ts...>;

template <typename... Types> struct VariantT {
    static_assert(sizeof...(Types) > 0, "VariantT must have at least one type");

    using TagType =
        std::conditional_t<(sizeof...(Types) < 256), std::uint8_t,
                           std::conditional_t<(sizeof...(Types) < 65536),
                                              std::uint16_t, std::uint32_t>>;
    TagType tag = sizeof...(Types);
    std::aligned_union_t<0, Types...> value;

    template <std::size_t Index>
    requires(Index < sizeof...(Types)) explicit constexpr VariantT(
        std::integral_constant<std::size_t, Index>);

    template <std::size_t Index, typename T>
    requires(Index < sizeof...(Types)) &&
        std::is_same_v<
            std::tuple_element_t<Index, std::tuple<Types...>>,
            std::decay_t<T>> explicit constexpr VariantT(std::
                                                             integral_constant<
                                                                 std::size_t,
                                                                 Index>,
                                                         T &&value);

    VariantT() = default;
    VariantT(const VariantT &other);
    VariantT &operator=(const VariantT &other);
    ~VariantT();

    static void copy(VariantT &target, const VariantT &source);

    template <typename T>
    static void copy_impl(std::aligned_union_t<0, Types...> &target,
                          const std::aligned_union_t<0, Types...> &source);

    void destroy();

    template <typename T>
    static void destroy_impl(std::aligned_union_t<0, Types...> &data);

    friend std::ostream &operator<<(std::ostream &os, const VariantT &variant) {
        os << '[' << static_cast<int>(variant.tag) << "; ";
        [&]<std::size_t... Is>(std::index_sequence<Is...>) {
            ((variant.tag == Is
                  ? (os << reinterpret_cast<const std::tuple_element_t<
                               Is, std::tuple<Types...>> *>(&variant.value)
                               ->value,
                     void())
                  : void()),
             ...);
            if ((variant.tag >= sizeof...(Is))) {
                os << "unknown";
            }
        }
        (std::make_index_sequence<sizeof...(Types)>());
        os << ']';
        return os;
    }
};

template <typename R, typename... As>
using FnT = TypedFn<LazyT<R>, LazyT<As>...>;

template <typename E, typename R, typename... As>
using ClosureT = TypedClosure<LazyT<E>, LazyT<R>, LazyT<As>...>;

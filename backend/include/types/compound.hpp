#pragma once

#include "data_structures/lazy.hpp"
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
        std::integral_constant<std::size_t, Index>)
        : tag(static_cast<TagType>(Index)) {}

    template <std::size_t Index, typename T>
    requires(Index < sizeof...(Types)) &&
        std::is_same_v<
            std::tuple_element_t<Index, std::tuple<Types...>>,
            std::decay_t<T>> explicit constexpr VariantT(std::
                                                             integral_constant<
                                                                 std::size_t,
                                                                 Index>,
                                                         T &&value)
        : tag(static_cast<TagType>(Index)) {
        new (std::launder(
            reinterpret_cast<std::decay_t<T> *>(std::addressof(this->value))))
            std::remove_reference_t<T>(std::forward<T>(value));
    }

    VariantT() = default;
    VariantT(const VariantT &other) { copy(*this, other); }
    VariantT &operator=(const VariantT &other) {
        if (this != &other) {
            destroy();
            copy(*this, other);
        }
        return *this;
    }
    ~VariantT() { destroy(); }

    static void copy(VariantT &target, const VariantT &source) {
        using CopyFn = void (*)(std::aligned_union_t<0, Types...> &,
                                const std::aligned_union_t<0, Types...> &);

        static constexpr CopyFn copiers[sizeof...(Types)] = {
            &copy_impl<Types>...};

        target.tag = source.tag;
        if (source.tag < sizeof...(Types)) {
            CopyFn copier = copiers[source.tag];
            copier(target.value, source.value);
        }
    }
    template <typename T>
    static void copy_impl(std::aligned_union_t<0, Types...> &target,
                          const std::aligned_union_t<0, Types...> &source) {
        new (&target) T{*reinterpret_cast<const T *>(&source)};
    }

    void destroy() {
        using DestructorFn = void (*)(std::aligned_union_t<0, Types...> &);

        static constexpr DestructorFn destructors[sizeof...(Types)] = {
            &destroy_impl<Types>...};

        if (tag < sizeof...(Types)) {
            DestructorFn destructor = destructors[tag];
            destructor(value);
            tag = sizeof...(Types);
        }
    }
    template <typename T>
    static void destroy_impl(std::aligned_union_t<0, Types...> &data) {
        reinterpret_cast<T *>(&data)->~T();
    }

    friend std::ostream &operator<<(std::ostream &os,
                                    const VariantT<Types...> &variant) {
        os << '[' << static_cast<Int>(variant.tag) << "; ";
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
using FnT = std::shared_ptr<ParametricFn<R, As...>>;

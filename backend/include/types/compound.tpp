#pragma once

#include "types/compound.hpp"

#include <memory>
#include <new>
#include <tuple>
#include <type_traits>

template <typename... Types>
template <std::size_t Index>
requires(Index < sizeof...(Types))
constexpr VariantT<Types...>::VariantT(std::integral_constant<std::size_t, Index>)
    : tag(static_cast<TagType>(Index)) {}

template <typename... Types>
template <std::size_t Index, typename T>
requires(Index < sizeof...(Types)) &&
         std::is_same_v<std::tuple_element_t<Index, std::tuple<Types...>>, std::decay_t<T>>
constexpr VariantT<Types...>::VariantT(std::integral_constant<std::size_t, Index>, T &&value)
    : tag(static_cast<TagType>(Index)) {
    new (std::launder(reinterpret_cast<std::decay_t<T> *>(std::addressof(this->value))))
        std::remove_reference_t<T>(std::forward<T>(value));
}

template <typename... Types>
VariantT<Types...>::VariantT(const VariantT &other) {
    copy(*this, other);
}

template <typename... Types>
VariantT<Types...> &VariantT<Types...>::operator=(const VariantT &other) {
    if (this != &other) {
        destroy();
        copy(*this, other);
    }
    return *this;
}

template <typename... Types>
VariantT<Types...>::~VariantT() {
    destroy();
}

template <typename... Types>
void VariantT<Types...>::copy(VariantT &target, const VariantT &source) {
    using CopyFn = void (*)(std::aligned_union_t<0, Types...> &, const std::aligned_union_t<0, Types...> &);

    // Define copies for all variants.
    static constexpr CopyFn copiers[sizeof...(Types)] = { &copy_impl<Types>... };

    target.tag = source.tag;
    if (source.tag < sizeof...(Types)) {
        // Perform the necessary copy.
        CopyFn copier = copiers[source.tag];
        copier(target.value, source.value);
    }
}

template <typename... Types>
template <typename T>
void VariantT<Types...>::copy_impl(std::aligned_union_t<0, Types...> &target, const std::aligned_union_t<0, Types...> &source) {
    new (&target) T{ *reinterpret_cast<const T *>(&source) };
}

template <typename... Types>
void VariantT<Types...>::destroy() {
    using DestructorFn = void (*)(std::aligned_union_t<0, Types...> &);

    // Define  destructors for all variants.
    static constexpr DestructorFn destructors[sizeof...(Types)] = { &destroy_impl<Types>... };

    if (tag < sizeof...(Types)) {
        // Call the necessary destructor.
        DestructorFn destructor = destructors[tag];
        destructor(value);
        tag = sizeof...(Types);
    }
}

template <typename... Types>
template <typename T>
void VariantT<Types...>::destroy_impl(std::aligned_union_t<0, Types...> &data) {
    reinterpret_cast<T *>(&data)->~T();
}

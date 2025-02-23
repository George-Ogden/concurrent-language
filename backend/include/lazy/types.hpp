#pragma once

#include "lazy/lazy.hpp"

#include <tuple>
#include <type_traits>

template <typename T> struct remove_lazy { using type = T; };

template <typename T> struct remove_lazy<Lazy<T>> { using type = T; };

template <typename T> struct remove_lazy<LazyPlaceholder<T>> {
    using type = T;
};

template <typename T> struct remove_lazy<std::shared_ptr<Lazy<T>>> {
    using type = T;
};

template <typename T> using remove_lazy_t = typename remove_lazy<T>::type;

template <typename... Ts> struct remove_lazy<std::tuple<Ts...>> {
    using type = std::tuple<remove_lazy_t<Ts>...>;
};

template <typename> struct is_lazy : std::false_type {};

template <typename T>
struct is_lazy<std::shared_ptr<Lazy<T>>> : std::true_type {};

template <typename T> inline constexpr bool is_lazy_v = is_lazy<T>::value;

template <typename T> struct lazy_type {
    using type = std::shared_ptr<Lazy<T>>;
};

template <typename T> using LazyT = typename lazy_type<T>::type;

template <typename T> struct lazy_type<std::shared_ptr<Lazy<T>>> {
    using type = std::shared_ptr<Lazy<T>>;
};

template <typename... Ts> struct lazy_type<std::tuple<Ts...>> {
    using type = std::tuple<LazyT<Ts>...>;
};

template <typename T> struct weak_lazy_placeholder_type {
    using type = std::weak_ptr<LazyPlaceholder<T>>;
};

template <typename T>
using WeakLazyPlaceholdersT = typename weak_lazy_placeholder_type<T>::type;

template <typename T>
struct weak_lazy_placeholder_type<std::weak_ptr<Lazy<T>>> {
    using type = std::weak_ptr<LazyPlaceholder<T>>;
};

template <typename... Ts> struct weak_lazy_placeholder_type<std::tuple<Ts...>> {
    using type = std::tuple<WeakLazyPlaceholdersT<Ts>...>;
};

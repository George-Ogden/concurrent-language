#pragma once

#include <memory>
#include <type_traits>

struct Fn;
template <typename> struct Lazy;

template <typename> struct is_tuple : std::false_type {};

template <typename... T> struct is_tuple<std::tuple<T...>> : std::true_type {};

template <typename T> inline constexpr bool is_tuple_v = is_tuple<T>::value;

template <typename Tuple> constexpr auto flatten(Tuple &&t) {
    if constexpr (is_tuple_v<std::remove_reference_t<Tuple>>) {
        return std::apply(
            [](auto &&...args) {
                return std::tuple_cat(
                    flatten(std::forward<decltype(args)>(args))...);
            },
            std::forward<Tuple>(t));
    } else {
        return std::make_tuple(std::forward<Tuple>(t));
    }
}
template <typename T> struct remove_shared_ptr { using type = T; };

template <typename T> struct remove_shared_ptr<std::shared_ptr<T>> {
    using type = T;
};

template <typename T>
using remove_shared_ptr_t = typename remove_shared_ptr<T>::type;

template <typename U = std::shared_ptr<Fn>, typename T> U dynamic_fn_cast(T f) {
    return std::dynamic_pointer_cast<remove_shared_ptr_t<U>>(f);
}

template <typename> struct is_shared_ptr : std::false_type {};

template <typename... T>
struct is_shared_ptr<std::shared_ptr<T...>> : std::true_type {};

template <typename T>
inline constexpr bool is_shared_ptr_v = is_shared_ptr<T>::value;

template <typename T> struct remove_lazy { using type = T; };

template <typename T> struct remove_lazy<std::shared_ptr<Lazy<T>>> {
    using type = T;
};

template <typename T> using remove_lazy_t = typename remove_lazy<T>::type;

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

#include "data_structures/lazy.hpp"

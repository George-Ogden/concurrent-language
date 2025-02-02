#pragma once

#include "data_structures/lazy.hpp"

#include <memory>
#include <type_traits>

template <typename> struct is_tuple : std::false_type {};

template <typename... T> struct is_tuple<std::tuple<T...>> : std::true_type {};

template <typename T> inline constexpr bool is_tuple_v = is_tuple<T>::value;

template <typename Tuple> constexpr auto spill(Tuple &&t) {
    if constexpr (is_tuple_v<std::remove_reference_t<Tuple>>) {
        return std::apply(
            [](auto &&...args) {
                return std::tuple_cat(
                    spill(std::forward<decltype(args)>(args))...);
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

template <typename U, typename T> U dynamic_fn_cast(T f) {
    return std::dynamic_pointer_cast<remove_shared_ptr_t<U>>(f);
}

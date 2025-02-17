#pragma once

#include "types/builtin.hpp"

#include <concepts>
#include <memory>
#include <string>
#include <string_view>
#include <type_traits>
#include <utility>

template <typename... Types> struct VariantT;

class Fn;

template <typename> struct is_tuple : std::false_type {};

template <typename... T> struct is_tuple<std::tuple<T...>> : std::true_type {};

template <typename T> inline constexpr bool is_tuple_v = is_tuple<T>::value;

template <typename> struct is_empty : std::false_type {};

template <> struct is_empty<Empty> : std::true_type {};

template <typename T> inline constexpr bool is_empty_v = is_empty<T>::value;

template <typename> struct is_variant : std::false_type {};

template <typename... T> struct is_variant<VariantT<T...>> : std::true_type {};

template <typename T> inline constexpr bool is_variant_v = is_variant<T>::value;

template <typename T> constexpr auto flatten(T &&t) {
    if constexpr (is_tuple_v<std::remove_reference_t<T>>) {
        return std::apply(
            [](auto &&...args) {
                return std::tuple_cat(
                    flatten(std::forward<decltype(args)>(args))...);
            },
            std::forward<T>(t));
    } else if constexpr (is_empty_v<std::decay_t<T>>) {
        return std::tuple<>{};
    } else {
        return std::make_tuple(std::forward<T>(t));
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

template <typename... T>
struct is_shared_ptr<std::weak_ptr<T...>> : std::true_type {};

template <typename T>
inline constexpr bool is_shared_ptr_v = is_shared_ptr<T>::value;

template <typename T>
Int convert_arg(char *&arg) requires std::same_as<T, Int> {
    return std::stoll(arg);
}

template <typename T>
Bool convert_arg(char *&arg) requires std::same_as<T, Bool> {
    std::string_view str(arg);
    if (str == "true" || str == "True") {
        return true;
    } else if (str == "false" || str == "False") {
        return false;
    } else {
        throw std::invalid_argument("Could not convert " + std::string(arg) +
                                    " to boolean.");
    }
}

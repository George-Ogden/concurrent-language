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
template <typename> struct Lazy;

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

template <typename T>
inline constexpr bool is_shared_ptr_v = is_shared_ptr<T>::value;

template <typename T> struct remove_lazy { using type = T; };

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

template <typename T> struct weak_lazy_type {
    using type = std::weak_ptr<Lazy<T>>;
};

template <typename T> using WeakLazyT = typename weak_lazy_type<T>::type;

template <typename T> struct weak_lazy_type<std::weak_ptr<Lazy<T>>> {
    using type = std::weak_ptr<Lazy<T>>;
};

template <typename... Ts> struct weak_lazy_type<std::tuple<Ts...>> {
    using type = std::tuple<WeakLazyT<Ts>...>;
};

template <typename F, typename T>
auto lazy_map(F f, std::shared_ptr<Lazy<T>> t) {
    if constexpr (std::is_void_v<
                      std::invoke_result_t<F, std::shared_ptr<Lazy<T>>>>) {
        f(t);
        return std::monostate{};
    } else {
        return f(t);
    }
}

template <typename F, typename... Ts> auto lazy_map(F f, std::tuple<Ts...> t) {
    return std::apply(
        [&f](auto... ts) { return std::tuple(lazy_map(f, ts)...); }, t);
}

template <typename F, typename T>
auto lazy_dual_map(F f, std::weak_ptr<Lazy<T>> t, std::shared_ptr<Lazy<T>> u) {
    if constexpr (std::is_void_v<std::invoke_result_t<
                      F, std::weak_ptr<Lazy<T>>, std::shared_ptr<Lazy<T>>>>) {
        f(t, u);
        return std::monostate{};
    } else {
        return f(t, u);
    }
}

template <typename F, typename... Ts, typename... Us>
auto lazy_dual_map(F f, std::tuple<Ts...> t, std::tuple<Us...> u) {
    static_assert(sizeof...(Ts) == sizeof...(Us));
    return [&]<std::size_t... Is>(std::index_sequence<Is...>) {
        return std::tuple(
            lazy_dual_map(f, std::get<Is>(t), std::get<Is>(u))...);
    }
    (std::index_sequence_for<Ts...>{});
}

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

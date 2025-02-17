#pragma once

#include "lazy/lazy.hpp"
#include "lazy/types.hpp"
#include "types/utils.hpp"

#include <type_traits>

template <typename F, typename T>
requires is_shared_ptr_v<T> && std::is_base_of_v<
    Lazy<remove_lazy_t<std::decay_t<typename T::element_type>>>,
    std::decay_t<typename T::element_type>>
auto lazy_map(F f, T t) {
    if constexpr (std::is_void_v<std::invoke_result_t<F, T>>) {
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

template <typename F, typename T, typename U>
requires is_shared_ptr_v<T> && is_shared_ptr_v<U> && std::is_base_of_v<
    Lazy<remove_lazy_t<std::decay_t<typename T::element_type>>>,
    std::decay_t<typename T::element_type>> &&
    std::is_base_of_v<
        Lazy<remove_lazy_t<std::decay_t<typename U::element_type>>>,
        std::decay_t<typename U::element_type>>
auto lazy_dual_map(F f, T t, U u) {
    if constexpr (std::is_void_v<std::invoke_result_t<F, T, U>>) {
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

template <typename T> auto make_lazy_placeholders(std::shared_ptr<Work> work) {
    return lazy_map(
        [&work](const auto &t) {
            return std::make_shared<
                LazyPlaceholder<remove_lazy_t<std::decay_t<decltype(t)>>>>(
                work);
        },
        T{});
}

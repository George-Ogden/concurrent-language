#pragma once

#include "fn/fn_inst.tpp"
#include "fn/types.hpp"
#include "lazy/lazy.hpp"
#include "lazy/types.hpp"
#include "types/utils.hpp"

#include <memory>
#include <type_traits>
#include <utility>

template <typename T> auto ensure_lazy(T arg) {
    if constexpr (is_lazy_v<std::decay_t<T>>) {
        return arg;
    } else {
        return make_lazy<T>(arg);
    }
}

template <typename... Ts> auto ensure_lazy(std::tuple<Ts...> arg) {
    return std::apply(
        [](auto... args) { return std::make_tuple(ensure_lazy(args)...); },
        arg);
}

template <typename T> auto extract_lazy(T arg) {
    if constexpr (is_lazy_v<std::decay_t<T>>) {
        return arg->value();
    } else {
        return arg;
    }
}

template <typename... Ts> auto extract_lazy(std::tuple<Ts...> arg) {
    return std::apply(
        [](auto... args) { return std::make_tuple(extract_lazy(args)...); },
        arg);
}

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

template <typename T> T load_env(T env) { return env; }

template <typename... Ts> auto load_env(std::tuple<Ts...> ts) {
    return std::apply(
        [](auto &...ts) { return std::make_tuple(load_env(ts)...); }, ts);
}

template <typename R, typename... A>
std::shared_ptr<Lazy<FnT<R, A...>>>
load_env(std::shared_ptr<Lazy<WeakFnT<R, A...>>> f) {
    return make_lazy<FnT<R, A...>>(std::shared_ptr(f->value()));
}

template <typename T, typename U> struct StoreEnv {
    static T store(U env) { return env; }
};

template <typename... Ts, typename U> struct StoreEnv<std::tuple<Ts...>, U> {
    static std::tuple<Ts...> store(U env) {
        return std::apply(
            [](auto &...args) {
                return std::make_tuple(
                    StoreEnv<Ts, decltype(args)>::store(args)...);
            },
            env);
    }
};

template <typename R, typename... A, typename U>
struct StoreEnv<std::shared_ptr<Lazy<WeakFnT<R, A...>>>, U> {
    static LazyT<WeakFnT<R, A...>> store(U f) {
        return make_lazy<WeakFnT<R, A...>>(f->value());
    }
};

template <typename T, typename U> T store_env(U env) {
    return StoreEnv<T, U>::store(env);
}

template <typename F> LazyT<std::shared_ptr<typename F::Fn>> setup_closure() {
    std::shared_ptr<typename F::Fn> fn = make_shared<
        ClosureFnT<remove_lazy_t<typename F::EnvT>, typename F::Fn>>(F::init);
    return make_lazy<std::shared_ptr<typename F::Fn>>(fn);
}

template <typename F, typename Y>
LazyT<std::shared_ptr<typename F::Fn>> setup_closure(
    std::shared_ptr<Y> &allocator,
    ClosureFnT<remove_lazy_t<typename F::EnvT>, typename F::Fn> &memory) {
    memory =
        ClosureFnT<remove_lazy_t<typename F::EnvT>, typename F::Fn>(F::init);
    std::shared_ptr<typename F::Fn> fn =
        std::shared_ptr<typename F::Fn>(allocator, &memory);
    return make_lazy<std::shared_ptr<typename F::Fn>>(fn);
}

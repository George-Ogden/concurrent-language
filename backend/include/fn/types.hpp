#pragma once

#include "fn/fn.hpp"

template <typename R, typename... As>
using FnT = TypedFn<LazyT<R>, LazyT<As>...>;

template <typename E, typename R, typename... As>
using ClosureT = TypedClosure<LazyT<E>, LazyT<R>, LazyT<As>...>;

template <typename E, typename T> struct closure_fn {};

template <typename E, typename R, typename... As>
struct closure_fn<E, TypedFn<R, As...>> {
    using type = TypedClosure<E, R, As...>;
};

template <typename E, typename T> using ClosureFnT = closure_fn<E, T>::type;

template <typename T> struct function_traits {};

template <typename Ret, typename... Args>
struct function_traits<Ret (*)(Args...)> {
    using return_type = Ret;

    template <typename H, typename... T> struct remove_last {
        using type = decltype(std::tuple_cat(
            std::tuple<H>{}, typename remove_last<T...>::type{}));
    };

    template <typename L> struct remove_last<L> { using type = std::tuple<>; };

    using args_tuple = typename remove_last<Args...>::type;
};

template <typename T>
using function_args_t = typename function_traits<T>::args_tuple;

template <typename T>
using function_ret_t = typename function_traits<T>::return_type;

template <typename R, typename Args> struct function_equivalent {};

template <typename R, typename... Args>
struct function_equivalent<R, std::tuple<Args...>> {
    using type = FnT<remove_lazy_t<R>, remove_lazy_t<Args>...>;
};

template <typename T>
using function_equivalent_t =
    typename function_equivalent<function_ret_t<T>, function_args_t<T>>::type;

#pragma once

#include "fn/fn_gen.hpp"
#include "lazy/types.hpp"

template <typename E, typename T> struct closure_fn {};

template <typename E, typename R, typename... As>
struct closure_fn<E, TypedFnG<R, As...>> {
    using type = TypedClosureG<E, R, As...>;
};

template <typename E, typename T> using ClosureFnT = closure_fn<E, T>::type;

template <typename R, typename... As>
using FnT = std::shared_ptr<TypedFnG<R, As...>>;
template <typename R, typename... As>
using WeakFnT = std::weak_ptr<TypedFnG<R, As...>>;

template <typename E, typename R, typename... As>
using ClosureT = TypedClosureG<E, R, As...>;

template <typename R, typename Args> struct function_equivalent {};

template <typename R, typename... Args>
struct function_equivalent<R, std::tuple<Args...>> {
    using type = FnT<R, Args...>;
};

template <typename T>
using function_equivalent_t =
    typename function_equivalent<remove_lazy_t<typename T::RetT>,
                                 remove_lazy_t<typename T::ArgsT>>::type;

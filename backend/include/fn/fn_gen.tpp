#pragma once

#include "fn/fn_gen.hpp"

#include <memory>

template <typename Ret, typename ...Args>
TypedFnG<Ret,Args...>::TypedFnG() = default;
template <typename Ret, typename ...Args>
TypedFnG<Ret,Args...>::~TypedFnG() = default;

template <typename E, typename Ret, typename ...Args>
TypedClosureG<E,Ret,Args...>::TypedClosureG(T fn, const EnvT &env):fn(fn),env(env){};
template <typename E, typename Ret, typename ...Args>
TypedClosureG<E,Ret,Args...>::TypedClosureG(T fn):fn(fn){}
template <typename E, typename Ret, typename ...Args>
TypedClosureG<E,Ret,Args...>::TypedClosureG() = default;
template <typename Ret, typename ...Args>
TypedClosureG<Empty,Ret,Args...>::TypedClosureG(T fn):fn(fn){}
template <typename Ret, typename ...Args>
TypedClosureG<Empty,Ret,Args...>::TypedClosureG() = default;

template <typename E, typename Ret, typename ...Args>
typename TypedClosureG<E,Ret,Args...>::U TypedClosureG<E,Ret,Args...>::init(LazyT<std::decay_t<Args>>... args) const {
    return fn(std::make_tuple(args...), env);
}
template <typename Ret, typename ...Args>
typename TypedClosureG<Empty,Ret,Args...>::U TypedClosureG<Empty,Ret,Args...>::init(LazyT<std::decay_t<Args>>... args) const {
    return fn(std::make_tuple(args...));
}

#pragma once

#include "fn/fn_gen.hpp"

#include <bit>
#include <memory>

template <typename Ret, typename ...Args>
TypedFnG<Ret,Args...>::TypedFnG(T fn, std::shared_ptr<void> env):_fn(fn), _env(std::reinterpret_pointer_cast<void>(env)){}
template <typename Ret, typename ...Args>
TypedFnG<Ret,Args...>::TypedFnG(T fn):TypedFnG(fn, nullptr){}
template <typename Ret, typename ...Args>
TypedFnG<Ret,Args...>::TypedFnG() = default;
template <typename Ret, typename ...Args>
TypedFnG<Ret,Args...>::~TypedFnG() = default;

template <typename Ret, typename ...Args>
typename TypedFnG<Ret,Args...>::U TypedFnG<Ret,Args...>::init(LazyT<std::decay_t<Args>>...args) const {
    return _fn(std::make_tuple(args...), _env);
}

template <typename E, typename Ret, typename ...Args>
TypedClosureG<E,Ret,Args...>::TypedClosureG(T fn, EnvT env):TypedFnG<Ret,Args...>(std::bit_cast<typename TypedFnG<Ret, Args...>::T>(fn), std::reinterpret_pointer_cast<void>(std::make_shared<EnvT>(env))){}
template <typename E, typename Ret, typename ...Args>
TypedClosureG<E,Ret,Args...>::TypedClosureG(T fn):TypedFnG<Ret,Args...>(std::bit_cast<typename TypedFnG<Ret, Args...>::T>(fn), std::make_shared<EnvT>()){}

template <typename E, typename Ret, typename ...Args>
typename TypedClosureG<E,Ret,Args...>::EnvT &TypedClosureG<E,Ret,Args...>::env() {
    return *std::reinterpret_pointer_cast<EnvT>(this->_env);
}

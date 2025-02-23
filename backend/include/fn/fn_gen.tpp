#pragma once

#include "fn/fn_gen.hpp"

#include <bit>
#include <memory>

FnG::FnG() = default;
FnG::~FnG() = default;

FnG::FnG(void * fn, std::shared_ptr<void> env):_fn(fn),_env(env){}
FnG::FnG(void * fn):FnG(fn, nullptr){}

template <typename Ret, typename ...Args>
TypedFnG<Ret,Args...>::TypedFnG(T fn, std::shared_ptr<void> env):FnG(std::bit_cast<void*>(fn), env){}
template <typename Ret, typename ...Args>
TypedFnG<Ret,Args...>::TypedFnG(T fn):FnG(std::bit_cast<void*>(fn)){}
template <typename Ret, typename ...Args>
TypedFnG<Ret,Args...>::TypedFnG():FnG(){}

template <typename Ret, typename ...Args>
typename TypedFnG<Ret,Args...>::U TypedFnG<Ret,Args...>::init(LazyT<std::decay_t<Args>>...args) const {
    return std::bit_cast<T>(_fn)(std::make_tuple(args...), _env);
}

template <typename E, typename Ret, typename ...Args>
TypedClosureG<E,Ret,Args...>::TypedClosureG(T fn, EnvT env):TypedFnG<Ret,Args...>(std::bit_cast<typename TypedFnG<Ret, Args...>::T>(fn), std::reinterpret_pointer_cast<void>(std::make_shared<EnvT>(env))){}
template <typename E, typename Ret, typename ...Args>
TypedClosureG<E,Ret,Args...>::TypedClosureG(T fn):TypedFnG<Ret,Args...>(std::bit_cast<typename TypedFnG<Ret, Args...>::T>(fn), std::make_shared<EnvT>()){}

template <typename E, typename Ret, typename ...Args>
typename TypedClosureG<E,Ret,Args...>::EnvT &TypedClosureG<E,Ret,Args...>::env() {
    return *std::reinterpret_pointer_cast<EnvT>(this->_env);
}

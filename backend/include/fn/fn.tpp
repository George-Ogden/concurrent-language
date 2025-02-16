#pragma once

#include "fn/fn.hpp"

#include <bit>

Fn::Fn() = default;
Fn::~Fn() = default;

Fn::Fn(void * fn, std::shared_ptr<void> env):_fn(fn),_env(env){}

template <typename R, typename ...Args>
TypedFn<R,Args...>::TypedFn(T fn, std::shared_ptr<void> env):Fn(std::bit_cast<void*>(fn), env){}

template <typename R, typename ...Args>
typename TypedFn<R,Args...>::T TypedFn<R,Args...>::fn() const {
    return std::bit_cast<T>(_fn);
}

template <typename R, typename ...Args>
R TypedFn<R,Args...>::call(Args...args) const {
    return fn()(args..., _env);
}

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

template <typename E, typename R, typename ...Args>
TypedClosure<E,R,Args...>::TypedClosure(T fn, std::shared_ptr<E>(env)):TypedFn<R,Args...>(std::bit_cast<typename TypedFn<R, Args...>::T>(fn), std::reinterpret_pointer_cast<void>(env)){}

template <typename E, typename R, typename ...Args>
const std::shared_ptr<E> TypedClosure<E,R,Args...>::env() const {
    return std::reinterpret_pointer_cast<E>(this->_env);
}

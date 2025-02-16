#pragma once

#include "fn/fn.hpp"

#include <bit>
#include <memory>

Fn::Fn() = default;
Fn::~Fn() = default;

Fn::Fn(void * fn, std::shared_ptr<void> env):_fn(fn),_env(env){}
Fn::Fn(void * fn):Fn(fn, nullptr){}

template <typename R, typename ...Args>
TypedFn<R,Args...>::TypedFn(T fn, std::shared_ptr<void> env):Fn(std::bit_cast<void*>(fn), env){}
template <typename R, typename ...Args>
TypedFn<R,Args...>::TypedFn(T fn):Fn(std::bit_cast<void*>(fn)){}
template <typename R, typename ...Args>
TypedFn<R,Args...>::TypedFn():Fn(){}

template <typename R, typename ...Args>
typename TypedFn<R,Args...>::T TypedFn<R,Args...>::fn() const {
    return std::bit_cast<T>(_fn);
}

template <typename R, typename ...Args>
R TypedFn<R,Args...>::call(Args...args) const {
    return fn()(args..., _env);
}

template <typename E, typename R, typename ...Args>
TypedClosure<E,R,Args...>::TypedClosure(T fn, E env):TypedFn<R,Args...>(std::bit_cast<typename TypedFn<R, Args...>::T>(fn), std::reinterpret_pointer_cast<void>(std::make_shared<E>(env))){}
template <typename E, typename R, typename ...Args>
TypedClosure<E,R,Args...>::TypedClosure(T fn):TypedFn<R,Args...>(std::bit_cast<typename TypedFn<R, Args...>::T>(fn), std::make_shared<E>()){}

template <typename E, typename R, typename ...Args>
E &TypedClosure<E,R,Args...>::env() {
    return *std::reinterpret_pointer_cast<E>(this->_env);
}

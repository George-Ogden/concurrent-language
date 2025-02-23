#pragma once

#include "fn/fn.hpp"

#include <bit>
#include <memory>

Fn::Fn() = default;
Fn::~Fn() = default;

Fn::Fn(void * fn, std::shared_ptr<void> env):_fn(fn),_env(env){}
Fn::Fn(void * fn):Fn(fn, nullptr){}

template <typename Ret, typename ...Args>
TypedFn<Ret,Args...>::TypedFn(T fn, std::shared_ptr<void> env):Fn(std::bit_cast<void*>(fn), env){}
template <typename Ret, typename ...Args>
TypedFn<Ret,Args...>::TypedFn(T fn):Fn(std::bit_cast<void*>(fn)){}
template <typename Ret, typename ...Args>
TypedFn<Ret,Args...>::TypedFn():Fn(){}

template <typename Ret, typename ...Args>
typename TypedFn<Ret,Args...>::T TypedFn<Ret,Args...>::fn() const {
    return std::bit_cast<T>(_fn);
}

template <typename Ret, typename ...Args>
Ret TypedFn<Ret,Args...>::call(Args...args) const {
    return fn()(args..., _env);
}

template <typename E, typename Ret, typename ...Args>
TypedClosure<E,Ret,Args...>::TypedClosure(T fn, E env):TypedFn<Ret,Args...>(std::bit_cast<typename TypedFn<Ret, Args...>::T>(fn), std::reinterpret_pointer_cast<void>(std::make_shared<E>(env))){}
template <typename E, typename Ret, typename ...Args>
TypedClosure<E,Ret,Args...>::TypedClosure(T fn):TypedFn<Ret,Args...>(std::bit_cast<typename TypedFn<Ret, Args...>::T>(fn), std::make_shared<E>()){}

template <typename E, typename Ret, typename ...Args>
E &TypedClosure<E,Ret,Args...>::env() {
    return *std::reinterpret_pointer_cast<E>(this->_env);
}

WeakFn::WeakFn(Fn f):_fn(f._fn),_env(f._env){}
WeakFn::WeakFn() = default;

Fn WeakFn::lock() const {
    return Fn{_fn, _env.lock()};
}

template <typename Ret, typename... Args>
TypedWeakFn<Ret, Args...>::TypedWeakFn(TypedFn<Ret, Args...> f):WeakFn(f){}
template <typename Ret, typename... Args>
TypedWeakFn<Ret, Args...>::TypedWeakFn():WeakFn(){}

template <typename Ret, typename... Args>
TypedFn<Ret, Args...> TypedWeakFn<Ret, Args...>::lock() const {
    return TypedFn<Ret, Args...>{std::bit_cast<typename TypedFn<Ret,Args...>::T>(_fn), _env.lock()};
}

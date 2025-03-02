#pragma once

#include "fn/fn_inst.hpp"
#include "fn_inst.hpp"

template <typename Ret, typename... Args>
TypedFnI<Ret, Args...>::TypedFnI() = default;

template <typename Ret, typename... Args>
TypedFnI<Ret, Args...>::~TypedFnI() = default;

template <typename Ret, typename... Args>
TypedFnI<Ret, Args...>::TypedFnI(const ArgsT&args)
:args(args){};

template <typename Ret, typename... Args>
typename TypedFnI<Ret, Args...>::RetT TypedFnI<Ret, Args...>::run() {
    return std::apply([this](auto &...t) { return body(t...); }, this->args);
}

template <typename Ret, typename... Args>
void TypedFnI<Ret, Args...>::set_fn(const std::shared_ptr<TypedFnG<Ret,Args...>> &fn) {}

template <typename E, typename Ret, typename... Args>
TypedClosureI<E, Ret, Args...>::TypedClosureI(const ArgsT &args,
                                              const EnvT &env)
    : TypedFnI<Ret, Args...>(args), env(env) {}

template <typename E, typename Ret, typename... Args>
void TypedClosureI<E, Ret, Args...>::set_fn(const std::shared_ptr<TypedFnG<Ret,Args...>> &f) {
    fn = f;
}

#pragma once

#include "fn/fn_inst.hpp"
#include "fn_inst.hpp"
#include "system/work_manager.tpp"
#include "work/work.tpp"

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
void TypedFnI<Ret, Args...>::process(WorkT &work) const {
    if (execute_immediately()) {
        work->run();
    }
}

template <typename Ret, typename... Args>
template <typename ...Ts>
auto TypedFnI<Ret, Args...>::fn_call(Ts... args) const {
    auto [work, res] = Work::fn_call(args...);
    process(work);
    return res;
}

template <typename Ret, typename... Args>
constexpr bool TypedFnI<Ret, Args...>::execute_immediately() const {
    return !is_recursive() && upper_size_bound() < IMMEDIATE_EXECUTION_THRESHOLD;
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

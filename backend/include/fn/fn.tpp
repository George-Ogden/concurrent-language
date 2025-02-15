#pragma once

#include "fn/fn.hpp"
#include "data_structures/lazy.tpp"
#include "system/work_manager.tpp"

#include <memory>

template <typename... Ts>
auto Fn::reference_all(Ts... args)
{
    return std::make_tuple(std::make_shared<LazyConstant<std::decay_t<Ts>>>(args)...);
}

template <typename Ret, typename... Args>
ParametricFn<Ret, Args...>::ParametricFn() = default;

template <typename Ret, typename... Args>
ParametricFn<Ret, Args...>::ParametricFn(LazyT<std::decay_t<Args>>... args)
    requires(sizeof...(Args) > 0)
    : args(args...)
{
}

template <typename Ret, typename... Args>
ParametricFn<Ret, Args...>::ParametricFn(std::add_const_t<std::add_lvalue_reference_t<Args>>... args)
    requires(sizeof...(Args) > 0)
    : args(Fn::reference_all(args...))
{
}

template <typename Ret, typename... Args>
ParametricFn<Ret, Args...>::~ParametricFn()
{
    cleanup_args();
}

template <typename Ret, typename... Args>
std::tuple<std::shared_ptr<ParametricFn<Ret, Args...>>, LazyT<Ret>>
ParametricFn<Ret, Args...>::clone_with_args(LazyT<std::decay_t<Args>>... args) const
{
    std::shared_ptr<ParametricFn<Ret, Args...>> call = this->clone();
    call->args = std::make_tuple(args...);
    call->ret = Lazy<LazyT<R>>::make_placeholders(call);
    return std::make_tuple(call, call->ret);
}

template <typename Ret, typename... Args>
void ParametricFn<Ret, Args...>::run()
{
    auto arguments = this->args;
    if (!done_flag.load(std::memory_order_acquire))
    {
        LazyT<R> return_ = std::apply([this](auto &&...t)
                                      { return body(t...); }, arguments);
        WorkManager::await(return_);
        assign(ret, return_);
    }
    continuations.acquire();
    for (const Continuation &c : *continuations)
    {
        Lazy<R>::update_continuation(c);
    }
    continuations->clear();
    done_flag.store(true, std::memory_order_release);
    cleanup();
    continuations.release();
}

template <typename Ret, typename... Args>
void ParametricFn<Ret, Args...>::await_all()
{
    WorkManager::await_all(ret);
}

template <typename Ret, typename... Args>
void ParametricFn<Ret, Args...>::cleanup()
{
    cleanup_args();
}

template <typename Ret, typename... Args>
void ParametricFn<Ret, Args...>::cleanup_args()
{
    this->args = ArgsT{};
}

template <typename Ret, typename... Args>
bool ParametricFn<Ret, Args...>::done() const
{
    return done_flag.load(std::memory_order_relaxed);
}

template <typename Ret, typename... Args>
typename ParametricFn<Ret, Args...>::R ParametricFn<Ret, Args...>::value() const
{
    return Lazy<R>::extract_value(ret);
}

template <typename Ret, typename... Args>
void ParametricFn<Ret, Args...>::add_continuation(Continuation c)
{
    continuations.acquire();
    if (done())
    {
        continuations.release();
        Lazy<R>::update_continuation(c);
    }
    else
    {
        continuations->push_back(c);
        continuations.release();
    }
}

template <typename F, typename R, typename... A>
std::shared_ptr<ParametricFn<R, A...>> EasyCloneFn<F, R, A...>::clone() const
{
    return std::make_shared<F>();
}

void FinishWork::run() {}

bool FinishWork::done() const
{
    return true;
}

void FinishWork::await_all() {}

template <typename T>
LazyT<T> BlockFn<T>::body()
{
    return body_fn();
}

template <typename T>
BlockFn<T>::BlockFn(std::function<LazyT<T>()> &&f) : body_fn(std::move(f)) {}

template <typename T>
BlockFn<T>::BlockFn(const std::function<LazyT<T>()> &f) : body_fn(f) {}

template <typename T>
std::shared_ptr<ParametricFn<T>> BlockFn<T>::clone() const
{
    return std::make_shared<BlockFn<T>>(body_fn);
}

template <typename E>
ClosureRoot<E>::ClosureRoot(const LazyT<E> &e) : env(e) {}

template <typename E>
ClosureRoot<E>::ClosureRoot() = default;

template <typename E>
ClosureRoot<E>::~ClosureRoot() = default;

template <typename T, typename E, typename R, typename... A>
std::shared_ptr<ParametricFn<R, A...>> Closure<T, E, R, A...>::clone() const
{
    return std::make_shared<T>(this->env);
}

template <typename T, typename E, typename R, typename... A>
Closure<T, E, R, A...>::~Closure() = default;

template <typename T, typename R, typename... A>
Closure<T, Empty, R, A...>::Closure() = default;
template <typename T, typename R, typename... A>
Closure<T, Empty, R, A...>::Closure(const Empty &e) {};
template <typename T, typename R, typename... A>
std::shared_ptr<ParametricFn<R, A...>> Closure<T, Empty, R, A...>::clone() const
{
    return std::make_shared<T>();
}

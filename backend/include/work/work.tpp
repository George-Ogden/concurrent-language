#pragma once

#include "work/work.hpp"
#include "fn/continuation.tpp"
#include "data_structures/lazy.tpp"
#include "types/utils.hpp"

#include <atomic>
#include <memory>
#include <utility>

Work::~Work() = default;

bool Work::done() const {
    return done_flag.load(std::memory_order_relaxed);
}

template <typename Ret, typename ...Args>
std::pair<std::shared_ptr<Work>, LazyT<TupleT<Args...>>> Work::fn_call(TypedFn<Ret, Args...> f, Args...args){
    using RetT = remove_lazy_t<Ret>;
    Ret targets = lazy_map([](const auto& target){
        return std::make_shared<remove_shared_ptr_t<std::decay_t<decltype(target)>>>();
    }, Ret{});
    WeakLazyT<RetT> weak_targets = lazy_map([](const auto& target){
        return std::weak_ptr(target);
    }, targets);
    std::shared_ptr<TypedWork<RetT, remove_lazy_t<Args>...>> work = std::make_shared<TypedWork<RetT, remove_lazy_t<Args>...>>();
    work->targets = weak_targets;
    work->args = std::make_tuple(args...);
    work->fn = f;
    lazy_map([&work](const auto& target){
        target->work = work;
    }, targets);
    return std::make_pair(work, targets);
}

void Work::add_continuation(Continuation c)
{
    continuations.acquire();
    if (done())
    {
        continuations.release();
        update_continuation(c);
    }
    else
    {
        continuations->push_back(c);
        continuations.release();
    }
}

void Work::update_continuation(Continuation c) {
    if (c.remaining->fetch_sub(1, std::memory_order_relaxed) == 1) {
        delete c.remaining;
        c.valid->acquire();
        if (**c.valid) {
            **c.valid = false;
            c.counter.fetch_add(1, std::memory_order_relaxed);
            c.valid->release();
        } else {
            c.valid->release();
            delete c.valid;
        }
    }
}

template <typename T, typename U>
void Work::assign(T &targets, U &results){
    lazy_dual_map([](auto target, auto result){
    target.lock()->_value = result->value();
        },
    targets, results);
}

template <typename Ret, typename ...Args>
void TypedWork<Ret,Args...>::run() {
    if (!this->done_flag.load(std::memory_order_acquire))
    {
        LazyT<Ret> results = std::apply([this](auto&&...args){return fn.call(std::forward<decltype(args)>(args)...);}, args);
        assign(targets, results);
    }
    this->continuations.acquire();
    for (const Continuation &c : *this->continuations)
    {
        Work::update_continuation(c);
    }
    this->continuations->clear();
    this->done_flag.store(true, std::memory_order_release);
    this->continuations.release();
}

#pragma once

#include "work/work.hpp"
#include "work/status.hpp"
#include "fn/continuation.tpp"
#include "lazy/lazy.tpp"
#include "lazy/types.hpp"
#include "lazy/fns.hpp"
#include "types/utils.hpp"
#include "system/work_manager.tpp"

#include <atomic>
#include <memory>
#include <utility>
#include <type_traits>

Work::Work():status(Status::available){}
Work::~Work() = default;

bool Work::done() const
{
    return status.load(std::memory_order_relaxed).done();
}

FinishWork::FinishWork(){
    status.store(Status::finished, std::memory_order_relaxed);
};

void FinishWork::run()
{
    throw finished{};
}

void FinishWork::await_all() {}

template <typename Ret, typename... Args>
std::pair<std::shared_ptr<Work>, Ret> Work::fn_call(TypedFn<Ret, Args...> f, Args... args)
{
    std::shared_ptr<TypedWork<remove_lazy_t<Ret>, remove_lazy_t<Args>...>> work = std::make_shared<TypedWork<remove_lazy_t<Ret>, remove_lazy_t<Args>...>>();
    auto placeholders = make_lazy_placeholders<Ret>(work);
    work->targets = lazy_map([](const auto &t)
                             { return std::weak_ptr(t); }, placeholders);
    work->args = std::make_tuple(args...);
    work->fn = f;
    return std::make_pair(work, placeholders);
}

void Work::add_continuation(Continuation c)
{
    continuations.acquire();
    if (done())
    {
        continuations.release();
        c.update();
    }
    else
    {
        continuations->push_back(c);
        continuations.release();
    }
}

template <typename T, typename U>
void Work::assign(T &targets, U &results)
{
    lazy_dual_map([](auto target, auto result)
                  {
        auto placeholder = target.lock();
        if (placeholder != nullptr){
            placeholder->assign(result);
} },
                  targets, results);
}

template <typename Ret, typename... Args>
void TypedWork<Ret, Args...>::run()
{
    if (!this->status.load(std::memory_order_acquire).done())
    {
        LazyT<Ret> results = std::apply([this](auto &&...args)
                                        { return fn.call(std::forward<decltype(args)>(args)...); }, args);
        assign(targets, results);
    }
    this->continuations.acquire();
    for (Continuation &c : *this->continuations)
    {
        c.update();
    }
    this->continuations->clear();
    this->status.store(Status::finished, std::memory_order_release);
    this->continuations.release();
}

template <typename Ret, typename... Args>
void TypedWork<Ret, Args...>::await_all()
{
    auto vs = lazy_map([](auto target)->LazyT<remove_lazy_t<remove_shared_ptr_t<decltype(target)>>>
                       { return target.lock(); }, targets);
    WorkManager::await_all(vs);
}

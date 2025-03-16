#pragma once

#include "work/work.hpp"
#include "work/status.tpp"
#include "fn/continuation.tpp"
#include "fn/types.hpp"
#include "fn/fn_inst.tpp"
#include "lazy/lazy.tpp"
#include "lazy/types.hpp"
#include "lazy/fns.hpp"
#include "types/utils.hpp"
#include "system/work_manager.tpp"

#include <atomic>
#include <memory>
#include <utility>
#include <type_traits>

Work::Work() = default;
Work::~Work() = default;

bool Work::done() const {
    return status.done();
}

template <typename Ret, typename... Args, typename ... ArgsT>
requires (std::is_same_v<Args,remove_lazy_t<std::decay_t<ArgsT>>> && ...)
std::pair<std::shared_ptr<Work>, LazyT<Ret>>
Work::fn_call(FnT<Ret, Args...> f, ArgsT... args) {
    std::shared_ptr<TypedWork<Ret, Args...>> work = std::make_shared<TypedWork<Ret, Args...>>();
    auto placeholders = make_lazy_placeholders<LazyT<Ret>>(work);
    work->targets = lazy_map([](const auto &t) { return std::weak_ptr(t); }, placeholders);
    work->fn = f->init(ensure_lazy(args)...);
    work->fn->set_fn(f);
    return std::make_pair(work, placeholders);
}

void Work::add_continuation(Continuation c) {
    continuations.acquire();
    if (done()) {
        continuations.release();
        c.update();
    } else {
        continuations->push_back(c);
        continuations.release();
    }
}

template <typename T, typename U>
void Work::assign(T &targets, U &results) {
    lazy_dual_map([](auto target, auto result) {
        auto placeholder = target.lock();
        if (placeholder != nullptr){
            placeholder->assign(result);
        } }, targets, results);
}

template <typename Ret, typename... Args>
void TypedWork<Ret, Args...>::run() {
    if (this->status.done()) {
        return;
    }
    LazyT<Ret> results = fn->run();
    assign(targets, results);
    this->continuations.acquire();
    for (Continuation &c : *this->continuations) {
        c.update();
    }
    this->continuations->clear();
    this->status.finish();
    this->continuations.release();
}

template <typename Ret, typename... Args>
void TypedWork<Ret, Args...>::await_all() {
    auto vs = lazy_map([](auto target) -> LazyT<remove_lazy_t<remove_shared_ptr_t<decltype(target)>>>
                       { return target.lock(); }, targets);
    WorkManager::await_all(vs);
}

bool operator<(const Work& a, const Work& b) {
    return a.size() < b.size();
}

bool Work::can_fulfill_request() const {
    return size() > 50;
}

template <typename Ret, typename... Args>
std::size_t TypedWork<Ret, Args...>::size() const {
    return fn->lower_size_bound() + fn->upper_size_bound();
}

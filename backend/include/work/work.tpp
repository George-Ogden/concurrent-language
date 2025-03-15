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

bool Work::prioritize(){
    if (status.priority()) return false;
    dependencies.acquire();
    if (status.prioritize()){
        for (std::weak_ptr<LazyValue> weak_dependency : *dependencies){
            std::shared_ptr<LazyValue> dependency = weak_dependency.lock();
            if (dependency != nullptr){
                dependency->require();
            }
        }
        dependencies.release();
        return true;
    } else {
        dependencies.release();
        return false;
    }
}

void Work::add_dependencies(std::initializer_list<std::shared_ptr<LazyValue>>&& dependencies){
    if (status.priority()) {
        for (std::shared_ptr<LazyValue> dependency: dependencies){
            dependency->require();
        }
    } else {
        this->dependencies.acquire();
        if (status.priority()){
            for (std::shared_ptr<LazyValue> dependency : dependencies){
                dependency->require();
            }
        } else {
            for (std::shared_ptr<LazyValue> dependency: dependencies){
                this->dependencies->push_back(dependency);
            }
        }
        this->dependencies.release();
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

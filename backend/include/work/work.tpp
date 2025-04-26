#pragma once

#include "work/work.hpp"
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
    bool is_done = static_cast<WorkStatus>(work_status.load<0>(std::memory_order_relaxed)) == WorkStatus::DONE;
    if (is_done){
        std::atomic_thread_fence(std::memory_order_acquire);
        return true;
    } else {
        return false;
    }
}

void Work::finish() {
    work_status.store<0>(WorkStatus::DONE, std::memory_order_release);
}

template <typename Ret, typename... Args, typename ... ArgsT>
requires (std::is_same_v<Args,remove_lazy_t<std::decay_t<ArgsT>>> && ...)
std::pair<std::shared_ptr<Work>, LazyT<Ret>>
Work::fn_call(const FnT<Ret, Args...> &f, const ArgsT&... args) {
    std::shared_ptr<TypedWork<Ret, Args...>> work = std::make_shared<TypedWork<Ret, Args...>>();
    // Make placeholders to store result.
    auto placeholders = make_lazy_placeholders<LazyT<Ret>>(work);
    // Setup the work targets with references to these placeholders.
    work->targets = lazy_map([](const auto &t) { return std::weak_ptr(t); }, placeholders);
    // Initialize with arguments.
    work->fn = f->init(ensure_lazy(args)...);
    // Set the work's fn to avoid closures collapsing.
    work->fn->set_fn(f);
    return std::make_pair(work, placeholders);
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
    if (work_status.compare_exchange<0>(WorkStatus::AVAILABLE, WorkStatus::ACTIVE, std::memory_order_acq_rel)){
        LazyT<Ret> results = fn->run();
        assign(targets, results);
        finish();
    }
}

template <typename Ret, typename... Args>
void TypedWork<Ret, Args...>::await_all() {
    auto vs = lazy_map([](auto target) -> LazyT<remove_lazy_t<remove_shared_ptr_t<decltype(target)>>>
                       { return target.lock(); }, targets);
    WorkManager::await_all(vs);
}

template <typename Ret, typename... Args>
bool TypedWork<Ret, Args...>::can_respond() const {
    /// Determine that the function is moderately large and currently available.
    if (fn->lower_size_bound() > 200 || fn->is_recursive()) {
        if (work_status.load<0>(std::memory_order_relaxed) == WorkStatus::AVAILABLE){
            std::atomic_thread_fence(std::memory_order_acquire);
            return true;
        }
    }
    return false;
}

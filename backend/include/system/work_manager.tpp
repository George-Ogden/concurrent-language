#pragma once

#include "fn/fn.tpp"
#include "system/thread_manager.tpp"
#include "system/work_manager.hpp"
#include "work/work.tpp"
#include "work/runner.tpp"

#include <range/v3/view/iota.hpp>
#include <range/v3/view/transform.hpp>
#include <range/v3/range/conversion.hpp>

#include <atomic>
#include <memory>
#include <utility>

template <typename Ret, typename... Args>
Ret WorkManager::run(TypedFn<Ret, Args...> fn, Args...args) {
    auto [work, result] = Work::fn_call(fn, args...);
    std::atomic<WorkT> ref{work};
    ThreadManager::RunConfig config{ThreadManager::available_concurrency(),
                                    false};
    WorkRunner::shared_work_queue->clear();

    runners = ranges::iota_view(static_cast<unsigned>(0), ThreadManager::available_concurrency())
          | ranges::views::transform([](auto thread_id) { return std::make_unique<WorkRunner>(thread_id); })
          | ranges::to<std::vector>();


    ThreadManager::run_multithreaded(main, &ref, config);
    return result;
}

void WorkManager::enqueue(WorkT work) {
    WorkRunner::enqueue(work);
}

std::monostate WorkManager::main(std::atomic<WorkT> *ref) {
    runners[ThreadManager::get_id()]->main(ref);
    return std::monostate{};
}

template <typename... Vs>
void WorkManager::await(Vs &...vs) {
    runners[ThreadManager::get_id()]->await(vs...);
}

template <typename... Vs>
void WorkManager::await_all(Vs &...vs) {
    runners[ThreadManager::get_id()]->await_all(vs...);
}

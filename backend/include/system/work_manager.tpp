#pragma once

#include "fn/types.hpp"
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
LazyT<Ret> WorkManager::run(FnT<Ret, Args...> fn, Args...args) {
    auto [work, result] = Work::fn_call(fn, args...);
    std::atomic<WorkT> ref{work};
    auto num_cpus = ThreadManager::available_concurrency();
    ThreadManager::RunConfig config{num_cpus, false};
    WorkRunner::setup(num_cpus);

    runners = ranges::iota_view(static_cast<unsigned>(0), WorkRunner::num_cpus)
          | ranges::views::transform([](auto thread_id) { return std::make_unique<WorkRunner>(thread_id); })
          | ranges::to<std::vector>();


    ThreadManager::run_multithreaded(main, &ref, config);
    return result;
}

std::monostate WorkManager::main(std::atomic<WorkT> *ref) {
    runners[ThreadManager::get_id()]->main(ref);
    return std::monostate{};
}

template <typename... Vs>
auto WorkManager::await(Vs &...vs) {
    return runners[ThreadManager::get_id()]->await(vs...);
}

template <typename... Vs>
void WorkManager::await_all(Vs &...vs) {
    runners[ThreadManager::get_id()]->await_all(vs...);
}
